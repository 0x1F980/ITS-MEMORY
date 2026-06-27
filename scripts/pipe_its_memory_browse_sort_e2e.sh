#!/usr/bin/env bash
# M46 — channel browse --sort memory_bytes vs frame_count ranks correctly.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHAT_ROOT="${ITS_CHAT_DIR:-/home/user/ITS-CHAT}"
ROUTING_ROOT="${ITS_ROUTING_DIR:-/home/user/ROUTING}"
ASYM="${ITS_ASYMMETRIC_DIR:-/home/user/ITS-asymmetric}"
SSS_ROOT="${SSS_CHAIN_DIR:-/home/user/SSS_CHAIN}"

TMP="${TMPDIR:-/tmp}/its_memory_m46_$$"
mkdir -p "$TMP"
trap 'rm -rf "$TMP"' EXIT

export ITS_CHAT_HOME="$TMP/chat_home"
export ITS_MEMORY_HOME="$TMP/memory_home"
REG="$ITS_MEMORY_HOME/coin/channel/registry"
POOL="$TMP/pool"
mkdir -p "$POOL" "$ITS_CHAT_HOME" "$ITS_MEMORY_HOME" "$REG"

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
BULK_PAYLOAD="$(python3 -c 'print("X"*3500)')"

mint_room() {
  local alias=$1
  local count=$2
  local msg_fn=$3
  "$CHAT" room create --alias "$alias" --type chat
  local room_dir="$ITS_CHAT_HOME/rooms/$alias"
  local pk rid
  pk="$(grep '^room_wire_pk' "$room_dir/room.toml" | head -1 | sed 's/^room_wire_pk *= *"\([^"]*\)".*/\1/')"
  rid="$(grep '^room_id' "$room_dir/room.toml" | head -1 | sed 's/^room_id *= *"\([^"]*\)".*/\1/')"
  export ITS_MEMORY_HOME="$TMP/memory_$alias"
  mkdir -p "$ITS_MEMORY_HOME"
  cp "$ITS_CHAT_HOME/ratchet.seed" "$ITS_MEMORY_HOME/"
  local i
  for ((i=1; i<=count; i++)); do
    rm -rf "$POOL"/*
    local msg
    msg="$("$msg_fn" "$i")"
    "$CHAT" send --room "$alias" --message "$msg"
    sleep 5
    "$MEMORY" pin --room-wire-pk "$pk" -c "$CFG" --ratchet-seed "$ITS_MEMORY_HOME/ratchet.seed" \
      --max-messages 1 --timeout-secs 90 \
      --filter-pk "$room_dir/public.key" --filter-sk "$room_dir/secret.key"
    sleep 2
  done
  local pins="$TMP/pins_$alias"
  "$MEMORY" fetch --room-wire-pk "$pk" --out "$pins" \
    --filter-pk "$room_dir/public.key" --filter-sk "$room_dir/secret.key"
  local man="$TMP/${alias}.coin.toml"
  "$COIN" channel mint --room-wire-pk "$pk" --pin-dir "$pins" --out "$man" \
    --decrypt-pk "$room_dir/public.key" --decrypt-sk "$room_dir/secret.key" --room-id "$rid"
  "$COIN" channel publish --manifest "$man" --registry "$REG"
  echo "$pk"
}

small_msg() { echo "tiny-$1"; }
big_msg() { echo "${BULK_PAYLOAD}-$1"; }

echo "== chatty (5 small) + bulky (2 large) rooms =="
CHATTY_PK="$(mint_room chatty 5 small_msg)"
BULKY_PK="$(mint_room bulky 2 big_msg)"

echo "== sort by frame_count: chatty first =="
BY_FRAMES="$("$COIN" channel browse --sort frame_count --registry "$REG")"
echo "$BY_FRAMES"
FIRST_FRAME="$(echo "$BY_FRAMES" | head -1)"
echo "$FIRST_FRAME" | grep -q "$CHATTY_PK" || { echo "FAIL: chatty should rank first by frame_count" >&2; exit 1; }

echo "== sort by memory_bytes: bulky first =="
BY_BYTES="$("$COIN" channel browse --sort memory_bytes --registry "$REG")"
echo "$BY_BYTES"
FIRST_BYTES="$(echo "$BY_BYTES" | head -1)"
echo "$FIRST_BYTES" | grep -q "$BULKY_PK" || { echo "FAIL: bulky should rank first by memory_bytes" >&2; exit 1; }

echo "pipe_its_memory_browse_sort_e2e.sh: OK (M46)"
