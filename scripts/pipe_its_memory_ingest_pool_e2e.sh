#!/usr/bin/env bash
# M54 — A publish → B ingest-pool → browse match.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHAT_ROOT="${ITS_CHAT_DIR:-/home/user/ITS-CHAT}"
ROUTING_ROOT="${ITS_ROUTING_DIR:-/home/user/ROUTING}"
ASYM="${ITS_ASYMMETRIC_DIR:-/home/user/ITS-asymmetric}"
SSS_ROOT="${SSS_CHAIN_DIR:-/home/user/SSS_CHAIN}"

TMP="${TMPDIR:-/tmp}/its_memory_m54_$$"
mkdir -p "$TMP"
trap 'rm -rf "$TMP"' EXIT

export ITS_CHAT_HOME="$TMP/chat_home"
HOME_A="$TMP/memory_a"
HOME_B="$TMP/memory_b"
REG_A="$HOME_A/coin/channel/registry"
REG_B="$HOME_B/coin/channel/registry"
POOL="$TMP/pool"
mkdir -p "$POOL" "$ITS_CHAT_HOME" "$HOME_A" "$HOME_B" "$REG_A" "$REG_B"

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
SEED="$HOME_A/ratchet.seed"

"$CHAT" room create --alias ingestroom --type chat
ROOM_DIR="$ITS_CHAT_HOME/rooms/ingestroom"
PK="$(grep '^room_wire_pk' "$ROOM_DIR/room.toml" | head -1 | sed 's/^room_wire_pk *= *"\([^"]*\)".*/\1/')"
RID="$(grep '^room_id' "$ROOM_DIR/room.toml" | head -1 | sed 's/^room_id *= *"\([^"]*\)".*/\1/')"

export ITS_MEMORY_HOME="$HOME_A"
rm -rf "$POOL"/*
"$CHAT" send --room ingestroom --message "ingest-test"
sleep 5
"$MEMORY" pin --room-wire-pk "$PK" -c "$CFG" --ratchet-seed "$SEED" \
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
rm -rf "$POOL"/*
"$COIN" channel publish --manifest "$MAN" --registry "$REG_A" -c "$CFG" --ratchet-seed "$SEED"
sleep 5

export ITS_MEMORY_HOME="$HOME_B"
INGEST_OUT="$("$COIN" channel ingest-pool -c "$CFG" --ratchet-seed "$SEED" --registry "$REG_B" --max-messages 4 --timeout-secs 30)"
echo "$INGEST_OUT"
N="$(echo "$INGEST_OUT" | awk '/Ingested channel coin/ {c++} END {print c+0}')"
echo "ingested: $N"
[[ "$N" -ge 1 ]] || { echo "FAIL: ingest-pool got 0 manifests" >&2; exit 1; }

BROWSE="$("$COIN" channel browse --registry "$REG_B")"
echo "$BROWSE"
echo "$BROWSE" | grep -q "$PK" || { echo "FAIL: ingested registry missing room_wire_pk" >&2; exit 1; }

echo "pipe_its_memory_ingest_pool_e2e.sh: OK (M54)"
