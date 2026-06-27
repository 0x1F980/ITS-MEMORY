#!/usr/bin/env bash
# M49 (MEMORY) — fetch --limit returns latest K pins by pool_epoch.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHAT_ROOT="${ITS_CHAT_DIR:-/home/user/ITS-CHAT}"
ROUTING_ROOT="${ITS_ROUTING_DIR:-/home/user/ROUTING}"
ASYM="${ITS_ASYMMETRIC_DIR:-/home/user/ITS-asymmetric}"

TMP="${TMPDIR:-/tmp}/its_memory_m49_$$"
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

"$CHAT" room create --alias query --type chat
ROOM_DIR="$ITS_CHAT_HOME/rooms/query"
PK="$(grep '^room_wire_pk' "$ROOM_DIR/room.toml" | head -1 | sed 's/^room_wire_pk *= *"\([^"]*\)".*/\1/')"

pin_one() {
  "$MEMORY" pin --room-wire-pk "$PK" -c "$CFG" --ratchet-seed "$SEED" \
    --max-messages 1 --timeout-secs 90 \
    --filter-pk "$ROOM_DIR/public.key" --filter-sk "$ROOM_DIR/secret.key"
}

for i in $(seq 1 10); do
  rm -rf "$POOL"/*
  "$CHAT" send --room query --message "msg-$i"
  sleep 5
  pin_one
  sleep 2
done

ALL="$TMP/all"
"$MEMORY" fetch --room-wire-pk "$PK" --out "$ALL" \
  --filter-pk "$ROOM_DIR/public.key" --filter-sk "$ROOM_DIR/secret.key"
ALL_COUNT="$(find "$ALL" -name '*.wire' | wc -l)"
[[ "$ALL_COUNT" -ge 10 ]] || { echo "FAIL: expected >=10 pins, got $ALL_COUNT" >&2; exit 1; }

LIMIT="$TMP/limit3"
"$MEMORY" fetch --room-wire-pk "$PK" --out "$LIMIT" --limit 3 \
  --filter-pk "$ROOM_DIR/public.key" --filter-sk "$ROOM_DIR/secret.key"
LIMIT_COUNT="$(find "$LIMIT" -name '*.wire' | wc -l)"
[[ "$LIMIT_COUNT" -eq 3 ]] || { echo "FAIL: --limit 3 expected 3 wires, got $LIMIT_COUNT" >&2; exit 1; }

SEQS=""
for w in "$LIMIT"/*.wire; do
  FRAME="$TMP/f_$(basename "$w").txt"
  "$ITS" decrypt --pk "$ROOM_DIR/public.key" --sk "$ROOM_DIR/secret.key" --in "$w" --out "$FRAME"
  SEQ="$(grep '^seq:' "$FRAME" | head -1 | awk '{print $2}')"
  SEQS="$SEQS $SEQ"
done
for want in 8 9 10; do
  echo "$SEQS" | tr ' ' '\n' | grep -qx "$want" || { echo "FAIL: --limit 3 missing seq=$want in:$SEQS" >&2; exit 1; }
done
echo "$SEQS" | tr ' ' '\n' | grep -qx "7" && { echo "FAIL: --limit 3 should not include seq=7 in:$SEQS" >&2; exit 1; }

HINT="$TMP/hint7"
"$MEMORY" fetch --room-wire-pk "$PK" --out "$HINT" --from-seq-hint 7 \
  --filter-pk "$ROOM_DIR/public.key" --filter-sk "$ROOM_DIR/secret.key"
HINT_COUNT="$(find "$HINT" -name '*.wire' | wc -l)"
[[ "$HINT_COUNT" -ge 4 ]] || { echo "FAIL: --from-seq-hint 7 expected >=4 wires, got $HINT_COUNT" >&2; exit 1; }

echo "pipe_its_memory_scroll_query_e2e.sh: OK (M49 fetch --limit; CHAT scroll query pending 6h)"
