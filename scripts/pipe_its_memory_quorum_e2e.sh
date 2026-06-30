#!/usr/bin/env bash
# M55 — solo --require-quorum 2 fails; two distinct witness_fp OK.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHAT_ROOT="${ITS_CHAT_DIR:-/home/user/ITS-CHAT}"
ROUTING_ROOT="${ITS_ROUTING_DIR:-/home/user/ROUTING}"
ASYM="${ITS_ASYMMETRIC_DIR:-/home/user/ITS-asymmetric}"
SSS_ROOT="${SSS_CHAIN_DIR:-/home/user/SSS_CHAIN}"

TMP="${TMPDIR:-/tmp}/its_memory_m55_$$"
mkdir -p "$TMP"
trap 'rm -rf "$TMP"' EXIT

export ITS_CHAT_HOME="$TMP/chat_home"
HOME_A="$TMP/memory_a"
HOME_B="$TMP/memory_b"
POOL="$TMP/pool"
mkdir -p "$POOL" "$ITS_CHAT_HOME" "$HOME_A" "$HOME_B"

cargo build --release --manifest-path "$ASYM/Cargo.toml" --bin its_asymmetric \
  --features "${ITS_ASYM_FEATURES:-bundle,parallel,std,compact-wire}" --quiet
cargo build --release --manifest-path "$ROUTING_ROOT/its_routing/Cargo.toml" --quiet
cargo build --release --manifest-path "$CHAT_ROOT/Cargo.toml" --quiet
cargo build --release --manifest-path "$SSS_ROOT/Cargo.toml" --quiet
cargo build --release --manifest-path "$ROOT/Cargo.toml" --quiet

CHAT="$CHAT_ROOT/target/release/its-chat"
MEMORY="$ROOT/target/release/its-memory"
COIN="$ROOT/target/release/its-coin"
export ITS_ASYMMETRIC_BIN="$ASYM/target/release/its_asymmetric"
export ITS_ROUTING_BIN="$ROUTING_ROOT/target/release/its-routing"
export SSS_CHAIN_BIN="$SSS_ROOT/target/release/sss_chain"

dd if=/dev/urandom of="$ITS_CHAT_HOME/ratchet.seed" bs=32 count=1 status=none
dd if=/dev/urandom of="$HOME_A/host.secret" bs=32 count=1 status=none
dd if=/dev/urandom of="$HOME_B/host.secret" bs=32 count=1 status=none
cp "$ITS_CHAT_HOME/ratchet.seed" "$HOME_A/ratchet.seed"
cp "$ITS_CHAT_HOME/ratchet.seed" "$HOME_B/ratchet.seed"
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

"$CHAT" room create --alias quorumroom --type chat
ROOM_DIR="$ITS_CHAT_HOME/rooms/quorumroom"
PK="$(grep '^room_wire_pk' "$ROOM_DIR/room.toml" | head -1 | sed 's/^room_wire_pk *= *"\([^"]*\)".*/\1/')"
RID="$(grep '^room_id' "$ROOM_DIR/room.toml" | head -1 | sed 's/^room_id *= *"\([^"]*\)".*/\1/')"

export ITS_MEMORY_HOME="$HOME_A"
rm -rf "$POOL"/*
"$CHAT" send --room quorumroom --message "quorum-a"
sleep 5
"$MEMORY" pin --room-wire-pk "$PK" -c "$CFG" --ratchet-seed "$HOME_A/ratchet.seed" \
  --max-messages 1 --timeout-secs 90 \
  --filter-pk "$ROOM_DIR/public.key" --filter-sk "$ROOM_DIR/secret.key"
sleep 2
"$MEMORY" publish-pins --room-wire-pk "$PK"
PINS="$TMP/pins"
"$MEMORY" fetch --room-wire-pk "$PK" --out "$PINS" \
  --filter-pk "$ROOM_DIR/public.key" --filter-sk "$ROOM_DIR/secret.key"
MAN="$TMP/coin.toml"
"$COIN" channel mint --room-wire-pk "$PK" --pin-dir "$PINS" --out "$MAN" \
  --require-published --decrypt-pk "$ROOM_DIR/public.key" --decrypt-sk "$ROOM_DIR/secret.key" \
  --room-id "$RID"
ROOT="$(grep '^chain_root:' "$MAN" | awk '{print $2}')"

echo "== solo quorum mint must fail =="
if "$COIN" channel mint --room-wire-pk "$PK" --pin-dir "$PINS" --require-published \
  --require-quorum 2 --decrypt-pk "$ROOM_DIR/public.key" --decrypt-sk "$ROOM_DIR/secret.key" \
  --room-id "$RID" 2>/dev/null; then
  echo "FAIL: solo mint with --require-quorum 2 should fail" >&2
  exit 1
fi

echo "== host A self-witness + host B witness =="
export ITS_MEMORY_HOME="$HOME_A"
"$MEMORY" witness --room-wire-pk "$PK" --chain-root "$ROOT" --pin-dir "$PINS"
export ITS_MEMORY_HOME="$HOME_B"
"$MEMORY" witness --room-wire-pk "$PK" --chain-root "$ROOT" --pin-dir "$PINS"
mkdir -p "$HOME_A/witnesses/$(echo "$PK" | tr '[:upper:]' '[:lower:]')"
cp "$HOME_B/witnesses/"*/*.witness "$HOME_A/witnesses/$(echo "$PK" | tr '[:upper:]' '[:lower:]')/" 2>/dev/null || \
  cp "$HOME_B/witnesses/"*/*.witness "$HOME_A/witnesses/"*/ 2>/dev/null || true
WITNESS_DIR="$HOME_B/witnesses"
for w in "$WITNESS_DIR"/*/*.witness; do
  [[ -f "$w" ]] || continue
  PKDIR="$HOME_A/witnesses/$(basename "$(dirname "$w")")"
  mkdir -p "$PKDIR"
  cp "$w" "$PKDIR/"
done

export ITS_MEMORY_HOME="$HOME_A"
"$COIN" channel mint --room-wire-pk "$PK" --pin-dir "$PINS" --out "$TMP/quorum_ok.toml" \
  --require-published --require-quorum 2 \
  --decrypt-pk "$ROOM_DIR/public.key" --decrypt-sk "$ROOM_DIR/secret.key" --room-id "$RID"

echo "pipe_its_memory_quorum_e2e.sh: OK (M55)"
