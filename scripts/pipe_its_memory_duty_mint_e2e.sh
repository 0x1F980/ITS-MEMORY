#!/usr/bin/env bash
# M60 — mint --global without GDIR receipt fails; M61 — pin delete breaks validate.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHAT_ROOT="${ITS_CHAT_DIR:-/home/user/ITS-CHAT}"
ROUTING_ROOT="${ITS_ROUTING_DIR:-/home/user/ROUTING}"
ASYM="${ITS_ASYMMETRIC_DIR:-/home/user/ITS-asymmetric}"
SSS_ROOT="${SSS_CHAIN_DIR:-/home/user/SSS_CHAIN}"

TMP="${TMPDIR:-/tmp}/its_memory_m60_$$"
mkdir -p "$TMP"
trap 'rm -rf "$TMP"' EXIT

export ITS_CHAT_HOME="$TMP/chat_home"
HOME_A="$TMP/memory_a"
export ITS_MEMORY_HOME="$HOME_A"
REG_A="$HOME_A/coin/channel/registry"
POOL="$TMP/pool"
mkdir -p "$POOL" "$ITS_CHAT_HOME" "$HOME_A"

cargo build --release --manifest-path "$ASYM/Cargo.toml" --bin its_asymmetric \
  --features "${ITS_ASYM_FEATURES:-bundle,parallel,std,compact-wire}" --quiet
cargo build --release --manifest-path "$ROUTING_ROOT/its_routing/Cargo.toml" --quiet
cargo build --release --manifest-path "$CHAT_ROOT/Cargo.toml" --quiet
cargo build --release --manifest-path "$SSS_ROOT/Cargo.toml" --quiet
cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml" --quiet

CHAT="$CHAT_ROOT/target/release/its-chat"
MEMORY="$REPO_ROOT/target/release/its-memory"
COIN="$REPO_ROOT/target/release/its-coin"
export ITS_ASYMMETRIC_BIN="$ASYM/target/release/its_asymmetric"
export ITS_ROUTING_BIN="$ROUTING_ROOT/target/release/its-routing"
export SSS_CHAIN_BIN="$SSS_ROOT/target/release/sss_chain"

dd if=/dev/urandom of="$ITS_CHAT_HOME/ratchet.seed" bs=32 count=1 status=none
dd if=/dev/urandom of="$HOME_A/host.secret" bs=32 count=1 status=none
dd if=/dev/urandom of="$TMP/host_b.secret" bs=32 count=1 status=none
cp "$ITS_CHAT_HOME/ratchet.seed" "$HOME_A/ratchet.seed"
cat > "$ITS_CHAT_HOME/routing.toml" <<EOF
[pool]
transport_mode = "pool"
pool_file = "$POOL"
cell_size_L = 4096
epoch_interval_ms = 100
sss_k = 2
sss_n = 3
fountain_enabled = false
EOF
CFG="$ITS_CHAT_HOME/routing.toml"
SEED="$HOME_A/ratchet.seed"

"$CHAT" room create --alias dutyroom --type chat
ROOM_DIR="$ITS_CHAT_HOME/rooms/dutyroom"
PK="$(grep '^room_wire_pk' "$ROOM_DIR/room.toml" | head -1 | sed 's/^room_wire_pk *= *"\([^"]*\)".*/\1/')"
RID="$(grep '^room_id' "$ROOM_DIR/room.toml" | head -1 | sed 's/^room_id *= *"\([^"]*\)".*/\1/')"

rm -rf "$POOL"/*
"$CHAT" send --room dutyroom --message "duty"
sleep 5
"$MEMORY" pin --room-wire-pk "$PK" -c "$CFG" --ratchet-seed "$SEED" \
  --max-messages 1 --timeout-secs 90 \
  --filter-pk "$ROOM_DIR/public.key" --filter-sk "$ROOM_DIR/secret.key"
sleep 2
"$MEMORY" publish-pins --room-wire-pk "$PK"
PINS="$TMP/pins"
"$MEMORY" fetch --room-wire-pk "$PK" --out "$PINS" \
  --filter-pk "$ROOM_DIR/public.key" --filter-sk "$ROOM_DIR/secret.key"

echo "== M60: --global without GDIR receipt fails =="
if "$COIN" channel mint --room-wire-pk "$PK" --pin-dir "$PINS" --require-published --global \
  --decrypt-pk "$ROOM_DIR/public.key" --decrypt-sk "$ROOM_DIR/secret.key" --room-id "$RID" 2>/dev/null; then
  echo "FAIL: --global should require GDIR receipt" >&2
  exit 1
fi

"$COIN" gdir record --op sync --byte-span 128

# Quorum for --global (≥2 distinct witness_fp)
MAN_PRE="$TMP/pre_man.toml"
"$COIN" channel mint --room-wire-pk "$PK" --pin-dir "$PINS" --out "$MAN_PRE" \
  --require-published --decrypt-pk "$ROOM_DIR/public.key" --decrypt-sk "$ROOM_DIR/secret.key" \
  --room-id "$RID"
CHAIN_ROOT="$(grep '^chain_root:' "$MAN_PRE" | awk '{print $2}')"
"$MEMORY" witness --room-wire-pk "$PK" --chain-root "$CHAIN_ROOT" --pin-dir "$PINS"
HOME_B="$TMP/memory_b"
mkdir -p "$HOME_B"
cp "$ITS_MEMORY_HOME/ratchet.seed" "$HOME_B/"
cp "$TMP/host_b.secret" "$HOME_B/host.secret"
export ITS_MEMORY_HOME="$HOME_B"
"$MEMORY" witness --room-wire-pk "$PK" --chain-root "$CHAIN_ROOT" --pin-dir "$PINS"
PK_NORM="$(echo "$PK" | tr '[:upper:]' '[:lower:]')"
mkdir -p "$HOME_A/witnesses/$PK_NORM"
for w in "$HOME_B/witnesses/$PK_NORM/"*.witness; do
  [[ -f "$w" ]] || continue
  cp "$w" "$HOME_A/witnesses/$PK_NORM/"
done
export ITS_MEMORY_HOME="$HOME_A"

# Pool duty witness for --global (publish + ingest-pool)
rm -rf "$POOL"/*
"$COIN" channel publish --manifest "$MAN_PRE" --registry "$REG_A" -c "$CFG" --ratchet-seed "$SEED"
sleep 3
"$COIN" channel ingest-pool -c "$CFG" --ratchet-seed "$SEED" --max-messages 2 --timeout-secs 30 >/dev/null

MAN="$TMP/duty.toml"
"$COIN" channel mint --room-wire-pk "$PK" --pin-dir "$PINS" --out "$MAN" \
  --require-published --global --require-quorum 2 \
  --decrypt-pk "$ROOM_DIR/public.key" --decrypt-sk "$ROOM_DIR/secret.key" --room-id "$RID"

echo "== M61: pin delete breaks validate =="
"$COIN" channel validate --manifest "$MAN" --pin-dir "$PINS" \
  --decrypt-pk "$ROOM_DIR/public.key" --decrypt-sk "$ROOM_DIR/secret.key"
rm -f "$PINS"/*.pin "$PINS"/*.wire
if "$COIN" channel validate --manifest "$MAN" --pin-dir "$PINS" \
  --decrypt-pk "$ROOM_DIR/public.key" --decrypt-sk "$ROOM_DIR/secret.key" 2>/dev/null; then
  echo "FAIL: validate should fail after pin destruction" >&2
  exit 1
fi

echo "pipe_its_memory_duty_mint_e2e.sh: OK (M60/M61)"
