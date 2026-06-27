#!/usr/bin/env bash
# M44 — local pin without publish-pins → channel mint --require-published fails.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHAT_ROOT="${ITS_CHAT_DIR:-/home/user/ITS-CHAT}"
ROUTING_ROOT="${ITS_ROUTING_DIR:-/home/user/ROUTING}"
ASYM="${ITS_ASYMMETRIC_DIR:-/home/user/ITS-asymmetric}"

TMP="${TMPDIR:-/tmp}/its_memory_m44_$$"
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
SSS_ROOT="${SSS_CHAIN_DIR:-/home/user/SSS_CHAIN}"
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

"$CHAT" room create --alias hoard --type chat
ROOM_DIR="$ITS_CHAT_HOME/rooms/hoard"
PK="$(grep '^room_wire_pk' "$ROOM_DIR/room.toml" | head -1 | sed 's/^room_wire_pk *= *"\([^"]*\)".*/\1/')"

rm -rf "$POOL"
mkdir -p "$POOL"
"$CHAT" send --room hoard --message "one"
sleep 4
"$MEMORY" pin --room-wire-pk "$PK" -c "$CFG" --ratchet-seed "$SEED" --max-messages 1 \
  --filter-pk "$ROOM_DIR/public.key" --filter-sk "$ROOM_DIR/secret.key"

if "$COIN" channel mint --room-wire-pk "$PK" --require-published --out "$TMP/coin.toml" 2>/dev/null; then
  echo "FAIL: mint without publish-pins should fail" >&2
  exit 1
fi

"$MEMORY" publish-pins --room-wire-pk "$PK"
"$COIN" channel mint --room-wire-pk "$PK" --require-published --out "$TMP/coin.toml"
grep -q "ITS-CHANNEL-COIN/2" "$TMP/coin.toml" || grep -q "memory_bytes:" "$TMP/coin.toml"
grep -q "memory_bytes:" "$TMP/coin.toml"

echo "pipe_its_memory_publish_e2e.sh: OK (M44)"
