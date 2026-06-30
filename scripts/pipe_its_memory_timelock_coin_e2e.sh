#!/usr/bin/env bash
# M59 — timelock unlock epoch excludes pre-unlock pins from memory_weight.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHAT_ROOT="${ITS_CHAT_DIR:-/home/user/ITS-CHAT}"
ROUTING_ROOT="${ITS_ROUTING_DIR:-/home/user/ROUTING}"
ASYM="${ITS_ASYMMETRIC_DIR:-/home/user/ITS-asymmetric}"
SSS_ROOT="${SSS_CHAIN_DIR:-/home/user/SSS_CHAIN}"

TMP="${TMPDIR:-/tmp}/its_memory_m59_$$"
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
cargo build --release --manifest-path "$SSS_ROOT/Cargo.toml" --quiet
cargo build --release --manifest-path "$ROOT/Cargo.toml" --quiet

CHAT="$CHAT_ROOT/target/release/its-chat"
MEMORY="$ROOT/target/release/its-memory"
COIN="$ROOT/target/release/its-coin"
export ITS_ASYMMETRIC_BIN="$ASYM/target/release/its_asymmetric"
export ITS_ROUTING_BIN="$ROUTING_ROOT/target/release/its-routing"
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

"$CHAT" room create --alias timelockroom --type chat
ROOM_DIR="$ITS_CHAT_HOME/rooms/timelockroom"
PK="$(grep '^room_wire_pk' "$ROOM_DIR/room.toml" | head -1 | sed 's/^room_wire_pk *= *"\([^"]*\)".*/\1/')"
RID="$(grep '^room_id' "$ROOM_DIR/room.toml" | head -1 | sed 's/^room_id *= *"\([^"]*\)".*/\1/')"

rm -rf "$POOL"/*
"$CHAT" send --room timelockroom --message "pre-unlock"
sleep 5
"$MEMORY" pin --room-wire-pk "$PK" -c "$CFG" --ratchet-seed "$SEED" \
  --max-messages 1 --timeout-secs 90 \
  --filter-pk "$ROOM_DIR/public.key" --filter-sk "$ROOM_DIR/secret.key"
sleep 2
"$MEMORY" publish-pins --room-wire-pk "$PK"

PINS="$TMP/pins"
"$MEMORY" fetch --room-wire-pk "$PK" --out "$PINS" \
  --filter-pk "$ROOM_DIR/public.key" --filter-sk "$ROOM_DIR/secret.key"

# Simulate unlock at pool_epoch 999 — all current pins are "sealed"
UNLOCK=999
MAN="$TMP/coin_tl.toml"
"$COIN" channel mint --room-wire-pk "$PK" --pin-dir "$PINS" --out "$MAN" \
  --require-published --timelock-unlock-epoch "$UNLOCK" \
  --decrypt-pk "$ROOM_DIR/public.key" --decrypt-sk "$ROOM_DIR/secret.key" \
  --room-id "$RID"

WEIGHT="$(grep '^memory_weight_seconds:' "$MAN" | awk '{print $2}')"
SEALED="$(grep '^timelock_sealed_frames:' "$MAN" | awk '{print $2}')"
[[ "${WEIGHT:-0}" -eq 0 ]] || { echo "FAIL: pre-unlock pins should yield memory_weight 0" >&2; exit 1; }
[[ "${SEALED:-0}" -ge 1 ]] || { echo "FAIL: timelock_sealed_frames should be >= 1" >&2; exit 1; }

echo "pipe_its_memory_timelock_coin_e2e.sh: OK (M59)"
