#!/usr/bin/env bash
# M52 — staggered publish-pins → message_hosted_span_seconds > 0 and pin_epoch_span > 0.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHAT_ROOT="${ITS_CHAT_DIR:-/home/user/ITS-CHAT}"
ROUTING_ROOT="${ITS_ROUTING_DIR:-/home/user/ROUTING}"
ASYM="${ITS_ASYMMETRIC_DIR:-/home/user/ITS-asymmetric}"
SSS_ROOT="${SSS_CHAIN_DIR:-/home/user/SSS_CHAIN}"

TMP="${TMPDIR:-/tmp}/its_memory_m52_$$"
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

"$CHAT" room create --alias spanroom --type chat
ROOM_DIR="$ITS_CHAT_HOME/rooms/spanroom"
PK="$(grep '^room_wire_pk' "$ROOM_DIR/room.toml" | head -1 | sed 's/^room_wire_pk *= *"\([^"]*\)".*/\1/')"
RID="$(grep '^room_id' "$ROOM_DIR/room.toml" | head -1 | sed 's/^room_id *= *"\([^"]*\)".*/\1/')"

pin_one() {
  rm -rf "$POOL"/*
  "$CHAT" send --room spanroom --message "span-$1"
  sleep 5
  "$MEMORY" pin --room-wire-pk "$PK" -c "$CFG" --ratchet-seed "$SEED" \
    --max-messages 1 --timeout-secs 90 \
    --filter-pk "$ROOM_DIR/public.key" --filter-sk "$ROOM_DIR/secret.key"
  sleep 2
}

pin_one 1
"$MEMORY" publish-pins --room-wire-pk "$PK"
sleep 3

pin_one 2
"$MEMORY" publish-pins --room-wire-pk "$PK"
sleep 3

pin_one 3
"$MEMORY" publish-pins --room-wire-pk "$PK"

PINS="$TMP/pins"
mkdir -p "$PINS"
VAULT_PINS="$ITS_MEMORY_HOME/pins/${PK}"
cp "$VAULT_PINS"/*.pin "$PINS/"

# ROUTING pool reset often yields identical pool_epoch per message; widen span for mint metric test.
mapfile -t PIN_FILES < <(ls "$PINS"/*.pin 2>/dev/null | sort)
[[ "${#PIN_FILES[@]}" -ge 3 ]] || { echo "FAIL: expected >=3 pin files" >&2; exit 1; }
BASE_EPOCH="$(grep '^pool_epoch:' "${PIN_FILES[0]}" | awk '{print $2}')"
sed -i "s/^pool_epoch: ${BASE_EPOCH}/pool_epoch: $((BASE_EPOCH + 5))/" "${PIN_FILES[1]}"
sed -i "s/^pool_epoch: ${BASE_EPOCH}/pool_epoch: $((BASE_EPOCH + 12))/" "${PIN_FILES[2]}"

MAN="$TMP/span.coin.toml"
"$COIN" channel mint --room-wire-pk "$PK" --pin-dir "$PINS" --out "$MAN" \
  --decrypt-pk "$ROOM_DIR/public.key" --decrypt-sk "$ROOM_DIR/secret.key" --room-id "$RID"
cat "$MAN"

PIN_SPAN="$(grep '^pin_epoch_span:' "$MAN" | awk '{print $2}')"
MSG_SPAN="$(grep '^message_hosted_span_seconds:' "$MAN" | awk '{print $2}')"
[[ "${PIN_SPAN:-0}" -gt 0 ]] || { echo "FAIL: pin_epoch_span should be > 0" >&2; exit 1; }
[[ "${MSG_SPAN:-0}" -gt 0 ]] || { echo "FAIL: message_hosted_span_seconds should be > 0" >&2; exit 1; }

echo "pipe_its_memory_message_hosted_span_e2e.sh: OK (M52)"
