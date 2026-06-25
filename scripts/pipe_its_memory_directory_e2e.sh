#!/usr/bin/env bash
# M43 — publish 2 rooms; browse sorts; without secret → decrypt fails (0 bits).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHAT_ROOT="${ITS_CHAT_DIR:-/home/user/ITS-CHAT}"
ROUTING_ROOT="${ITS_ROUTING_DIR:-/home/user/ROUTING}"
ASYM="${ITS_ASYMMETRIC_DIR:-/home/user/ITS-asymmetric}"

TMP="${TMPDIR:-/tmp}/its_memory_dir_$$"
mkdir -p "$TMP"
trap 'rm -rf "$TMP"' EXIT

export ITS_CHAT_HOME="$TMP/chat_home"
export ITS_MEMORY_HOME="$TMP/memory_home"
REG="$ITS_MEMORY_HOME/coin/registry"
POOL="$TMP/pool"
mkdir -p "$POOL" "$ITS_CHAT_HOME" "$ITS_MEMORY_HOME" "$REG"

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

mint_room() {
  local alias=$1
  local count=$2
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
    "$CHAT" send --room "$alias" --message "msg $i"
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
  "$COIN" mint --room-wire-pk "$pk" --pin-dir "$pins" --out "$man" \
    --decrypt-pk "$room_dir/public.key" --decrypt-sk "$room_dir/secret.key" --room-id "$rid"
  "$COIN" publish --manifest "$man" --registry "$REG"
  echo "$pk"
}

echo "== mint + publish quiet + loud rooms =="
mint_room quiet 1 >/dev/null
LOUD_PK="$(mint_room loud 5)"

echo "== browse sorted by frame_count =="
BROWSE="$("$COIN" browse --sort frame_count --registry "$REG")"
echo "$BROWSE"
FIRST="$(echo "$BROWSE" | head -1)"
echo "$FIRST" | grep -q "$LOUD_PK" || { echo "FAIL: loudest room should sort first" >&2; exit 1; }

echo "== search min-frames 3 =="
SEARCH="$("$COIN" search --min-frames 3 --registry "$REG")"
echo "$SEARCH"
echo "$SEARCH" | grep -q "$LOUD_PK" || { echo "FAIL: loud room missing from search" >&2; exit 1; }

echo "== outsider without secret cannot decrypt pin =="
OUTSIDER="$TMP/outsider"
"$ITS" keygen --out-dir "$OUTSIDER"
WIRE="$(find "$TMP/pins_loud" -name '*.wire' | head -1)"
if "$ITS" decrypt --pk "$OUTSIDER/public.key" --sk "$OUTSIDER/secret.key" --in "$WIRE" --out "$TMP/bad.frame" 2>"$TMP/decrypt.err"; then
  echo "FAIL: decrypt should fail without room capability" >&2
  exit 1
fi
test -s "$TMP/decrypt.err" || true

echo "pipe_its_memory_directory_e2e.sh: OK (M43)"
