#!/usr/bin/env bash
# M40 — pin pool ciphertext, fetch, decrypt scroll seq 1..3.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHAT_ROOT="${ITS_CHAT_DIR:-/home/user/ITS-CHAT}"
ROUTING_ROOT="${ITS_ROUTING_DIR:-/home/user/ROUTING}"
ASYM="${ITS_ASYMMETRIC_DIR:-/home/user/ITS-asymmetric}"

TMP="${TMPDIR:-/tmp}/its_memory_pin_$$"
mkdir -p "$TMP"
trap 'rm -rf "$TMP"' EXIT

export ITS_CHAT_HOME="$TMP/chat_home"
export ITS_MEMORY_HOME="$TMP/memory_home"
POOL="$TMP/pool"
mkdir -p "$POOL" "$ITS_CHAT_HOME" "$ITS_MEMORY_HOME"

cargo build --release --manifest-path "$ASYM/Cargo.toml" --bin its_asymmetric \
  --features "${ITS_ASYM_FEATURES:-bundle,parallel,std,compact-wire}" --quiet
cargo build --release --manifest-path "$ROUTING_ROOT/its_routing/Cargo.toml" --quiet
cargo build --release --manifest-path "$CHAT_ROOT/Cargo.toml" --quiet
cargo build --release --manifest-path "$ROOT/Cargo.toml" --quiet

ITS="$ASYM/target/release/its_asymmetric"
CHAT="$CHAT_ROOT/target/release/its-chat"
MEMORY="$ROOT/target/release/its-memory"

export ITS_ASYMMETRIC_BIN="$ITS"
export ITS_ROUTING_BIN="$ROUTING_ROOT/target/release/its-routing"
export ITS_MEMORY_BIN="$MEMORY"

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

echo "== create room =="
"$CHAT" room create --alias hist --type chat
ROOM_DIR="$ITS_CHAT_HOME/rooms/hist"
PK="$(grep '^room_wire_pk' "$ROOM_DIR/room.toml" | head -1 | sed 's/^room_wire_pk *= *"\([^"]*\)".*/\1/')"

pin_one() {
  "$MEMORY" pin --room-wire-pk "$PK" -c "$CFG" --ratchet-seed "$SEED" \
    --max-messages 1 --timeout-secs 90 \
    --filter-pk "$ROOM_DIR/public.key" --filter-sk "$ROOM_DIR/secret.key"
}

echo "== send + pin x3 (pool reset between wires) =="
for msg in "memory one" "memory two" "memory three"; do
  rm -rf "$POOL"/*
  "$CHAT" send --room hist --message "$msg"
  sleep 5
  pin_one
  sleep 2
done

echo "== fetch pins =="
FETCH="$TMP/fetched"
"$MEMORY" fetch --room-wire-pk "$PK" --out "$FETCH" \
  --filter-pk "$ROOM_DIR/public.key" --filter-sk "$ROOM_DIR/secret.key"
WIRE_COUNT="$(find "$FETCH" -name '*.wire' | wc -l)"
[[ "$WIRE_COUNT" -ge 3 ]] || { echo "FAIL: expected >=3 wire files, got $WIRE_COUNT" >&2; exit 1; }

echo "== decrypt + verify seq 1..3 =="
SEQS=""
for w in "$FETCH"/*.wire; do
  FRAME="$TMP/frame_$(basename "$w").txt"
  "$ITS" decrypt --pk "$ROOM_DIR/public.key" --sk "$ROOM_DIR/secret.key" --in "$w" --out "$FRAME"
  SEQ="$(grep '^seq:' "$FRAME" | head -1 | awk '{print $2}')"
  SEQS="$SEQS $SEQ"
done
for want in 1 2 3; do
  echo "$SEQS" | grep -q " $want" || { echo "FAIL: missing seq=$want in:$SEQS" >&2; exit 1; }
done

echo "pipe_its_memory_pin_e2e.sh: OK (M40)"
