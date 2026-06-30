#!/usr/bin/env bash
# M57 — blind_shards dir has no room_wire_pk; M58 — GDIR mint from blind receipts.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHAT_ROOT="${ITS_CHAT_DIR:-/home/user/ITS-CHAT}"
ROUTING_ROOT="${ITS_ROUTING_DIR:-/home/user/ROUTING}"
ASYM="${ITS_ASYMMETRIC_DIR:-/home/user/ITS-asymmetric}"
SSS_ROOT="${SSS_CHAIN_DIR:-/home/user/SSS_CHAIN}"

TMP="${TMPDIR:-/tmp}/its_memory_m57_$$"
mkdir -p "$TMP"
trap 'rm -rf "$TMP"' EXIT

export ITS_CHAT_HOME="$TMP/chat_home"
export ITS_MEMORY_HOME="$TMP/memory_home"
POOL="$TMP/pool"
BLIND="$ITS_MEMORY_HOME/blind_shards"
mkdir -p "$POOL" "$ITS_CHAT_HOME" "$ITS_MEMORY_HOME" "$BLIND"

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
dd if=/dev/urandom of="$ITS_MEMORY_HOME/host.secret" bs=32 count=1 status=none
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

"$CHAT" room create --alias blindroom --type chat
ROOM_DIR="$ITS_CHAT_HOME/rooms/blindroom"
PK="$(grep '^room_wire_pk' "$ROOM_DIR/room.toml" | head -1 | sed 's/^room_wire_pk *= *"\([^"]*\)".*/\1/')"

rm -rf "$POOL"/*
"$CHAT" send --room blindroom --message "blind-shard-payload"
sleep 5
"$MEMORY" pin --room-wire-pk "$PK" -c "$CFG" --ratchet-seed "$SEED" \
  --max-messages 1 --timeout-secs 90 \
  --filter-pk "$ROOM_DIR/public.key" --filter-sk "$ROOM_DIR/secret.key"
sleep 2
"$MEMORY" publish-pins --room-wire-pk "$PK"
rm -rf "$POOL"/*
"$CHAT" send --room blindroom --message "blind-pool-wire"
sleep 5
"$MEMORY" blind-pull -c "$CFG" --ratchet-seed "$SEED" --max-messages 2 --timeout-secs 90

if grep -r "room_wire_pk" "$BLIND" 2>/dev/null; then
  echo "FAIL: blind_shards must not contain room_wire_pk" >&2
  exit 1
fi
ls "$BLIND"/*.shard >/dev/null 2>&1 || { echo "FAIL: no blind shard stored" >&2; exit 1; }

MAN="$TMP/gdir.toml"
"$COIN" gdir mint --require-blind --out "$MAN"
"$COIN" gdir validate --manifest "$MAN"

echo "pipe_its_memory_blind_gdir_e2e.sh: OK (M57/M58)"
