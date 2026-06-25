#!/usr/bin/env bash
# M42 — two independent pin dirs → same chain_root; validate OK.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHAT_ROOT="${ITS_CHAT_DIR:-/home/user/ITS-CHAT}"
ROUTING_ROOT="${ITS_ROUTING_DIR:-/home/user/ROUTING}"
ASYM="${ITS_ASYMMETRIC_DIR:-/home/user/ITS-asymmetric}"

TMP="${TMPDIR:-/tmp}/its_memory_coin_$$"
mkdir -p "$TMP"
trap 'rm -rf "$TMP"' EXIT

export ITS_CHAT_HOME="$TMP/chat_home"
export ITS_MEMORY_HOME="$TMP/memory_home"
POOL="$TMP/pool"
PIN_A="$TMP/pins_a"
PIN_B="$TMP/pins_b"
mkdir -p "$POOL" "$ITS_CHAT_HOME" "$ITS_MEMORY_HOME" "$PIN_A" "$PIN_B"

cargo build --release --manifest-path "$ASYM/Cargo.toml" --bin its_asymmetric \
  --features "${ITS_ASYM_FEATURES:-bundle,parallel,std,compact-wire}" --quiet
cargo build --release --manifest-path "$ROUTING_ROOT/its_routing/Cargo.toml" --quiet
cargo build --release --manifest-path "$CHAT_ROOT/Cargo.toml" --quiet
SSS_ROOT="${SSS_CHAIN_DIR:-/home/user/SSS_CHAIN}"
cargo build --release --manifest-path "$SSS_ROOT/Cargo.toml" --quiet
cargo build --release --manifest-path "$ROOT/Cargo.toml" --quiet

ITS="$ASYM/target/release/its_asymmetric"
CHAT="$CHAT_ROOT/target/release/its-chat"
COIN="$ROOT/target/release/its-coin"
MEMORY="$ROOT/target/release/its-memory"

export ITS_ASYMMETRIC_BIN="$ITS"
export ITS_ROUTING_BIN="$ROUTING_ROOT/target/release/its-routing"
export ITS_MEMORY_BIN="$MEMORY"
export SSS_CHAIN_BIN="$SSS_ROOT/target/release/sss_chain"

dd if=/dev/urandom of="$ITS_CHAT_HOME/ratchet.seed" bs=32 count=1 status=none
cp "$ITS_CHAT_HOME/ratchet.seed" "$ITS_MEMORY_HOME/ratchet.seed"
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
SEED="$ITS_MEMORY_HOME/ratchet.seed"

"$CHAT" room create --alias coinroom --type chat
ROOM_DIR="$ITS_CHAT_HOME/rooms/coinroom"
PK="$(grep '^room_wire_pk' "$ROOM_DIR/room.toml" | head -1 | sed 's/^room_wire_pk *= *"\([^"]*\)".*/\1/')"
RID="$(grep '^room_id' "$ROOM_DIR/room.toml" | head -1 | sed 's/^room_id *= *"\([^"]*\)".*/\1/')"

pin_one() {
  "$MEMORY" pin --room-wire-pk "$PK" -c "$CFG" --ratchet-seed "$SEED" \
    --max-messages 1 --timeout-secs 90 \
    --filter-pk "$ROOM_DIR/public.key" --filter-sk "$ROOM_DIR/secret.key"
}

for msg in "coin a" "coin b"; do
  rm -rf "$POOL"/*
  "$CHAT" send --room coinroom --message "$msg"
  sleep 5
  pin_one
  sleep 2
done

"$MEMORY" fetch --room-wire-pk "$PK" --out "$PIN_A" \
  --filter-pk "$ROOM_DIR/public.key" --filter-sk "$ROOM_DIR/secret.key"
mkdir -p "$PIN_B"
cp -a "$PIN_A/." "$PIN_B/"

MAN_A="$TMP/coin_a.toml"
MAN_B="$TMP/coin_b.toml"
"$COIN" mint --room-wire-pk "$PK" --pin-dir "$PIN_A" --out "$MAN_A" \
  --decrypt-pk "$ROOM_DIR/public.key" --decrypt-sk "$ROOM_DIR/secret.key" --room-id "$RID"
"$COIN" mint --room-wire-pk "$PK" --pin-dir "$PIN_B" --out "$MAN_B" \
  --decrypt-pk "$ROOM_DIR/public.key" --decrypt-sk "$ROOM_DIR/secret.key" --room-id "$RID"

ROOT_A="$(grep '^chain_root:' "$MAN_A" | awk '{print $2}')"
ROOT_B="$(grep '^chain_root:' "$MAN_B" | awk '{print $2}')"
[[ "$ROOT_A" == "$ROOT_B" ]] || { echo "FAIL: chain_root mismatch" >&2; exit 1; }

"$COIN" validate --manifest "$MAN_A" --pin-dir "$PIN_A" \
  --decrypt-pk "$ROOM_DIR/public.key" --decrypt-sk "$ROOM_DIR/secret.key"
"$COIN" validate --manifest "$MAN_B" --pin-dir "$PIN_B" \
  --decrypt-pk "$ROOM_DIR/public.key" --decrypt-sk "$ROOM_DIR/secret.key"

echo "pipe_its_memory_coin_e2e.sh: OK (M42)"
