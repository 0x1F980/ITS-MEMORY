#!/usr/bin/env bash
# M50 — spam-room + quiet-room; --order asc → quiet first; both registry_visible.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHAT_ROOT="${ITS_CHAT_DIR:-/home/user/ITS-CHAT}"
ROUTING_ROOT="${ITS_ROUTING_DIR:-/home/user/ROUTING}"
ASYM="${ITS_ASYMMETRIC_DIR:-/home/user/ITS-asymmetric}"
SSS_ROOT="${SSS_CHAIN_DIR:-/home/user/SSS_CHAIN}"

TMP="${TMPDIR:-/tmp}/its_memory_m50_$$"
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

mint_room() {
  local alias=$1
  local count=$2
  "$CHAT" room create --alias "$alias" --type chat --registry visible
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
    "$CHAT" send --room "$alias" --message "msg-$alias-$i"
    sleep 5
    "$MEMORY" pin --room-wire-pk "$pk" -c "$CFG" --ratchet-seed "$ITS_MEMORY_HOME/ratchet.seed" \
      --max-messages 1 --timeout-secs 90 \
      --filter-pk "$room_dir/public.key" --filter-sk "$room_dir/secret.key"
    sleep 2
  done
  "$MEMORY" publish-pins --room-wire-pk "$pk"
  local pins="$TMP/pins_$alias"
  "$MEMORY" fetch --room-wire-pk "$pk" --out "$pins" \
    --filter-pk "$room_dir/public.key" --filter-sk "$room_dir/secret.key"
  local man="$TMP/${alias}.coin.toml"
  "$COIN" channel mint --room-wire-pk "$pk" --pin-dir "$pins" --out "$man" \
    --require-published --decrypt-pk "$room_dir/public.key" --decrypt-sk "$room_dir/secret.key" \
    --room-id "$rid"
  "$COIN" channel publish --manifest "$man" --registry "$REG"
  echo "$pk"
}

echo "== spam-room (10 frames) + quiet-room (2 frames) =="
SPAM_PK="$(mint_room spam-room 10 | tail -1)"
QUIET_PK="$(mint_room quiet-room 2 | tail -1)"

echo "== browse --sort frame_count --order asc: quiet first =="
BY_ASC="$("$COIN" channel browse --sort frame_count --order asc --registry "$REG")"
echo "$BY_ASC"
FIRST="$(echo "$BY_ASC" | head -1)"
echo "$FIRST" | grep -q "$QUIET_PK" || { echo "FAIL: quiet-room should rank first with --order asc" >&2; exit 1; }

echo "== discover-quiet preset: quiet first =="
QUIET_OUT="$("$COIN" channel discover-quiet --registry "$REG")"
echo "$QUIET_OUT"
FIRST_Q="$(echo "$QUIET_OUT" | head -1)"
echo "$FIRST_Q" | grep -q "$QUIET_PK" || { echo "FAIL: discover-quiet should rank quiet-room first" >&2; exit 1; }

echo "== both rooms visible in registry =="
ls "$REG"/*.channel.coin.toml | wc -l | grep -q '^2$' || { echo "FAIL: expected 2 registry entries" >&2; exit 1; }

echo "pipe_its_memory_discover_quiet_e2e.sh: OK (M50)"
