#!/usr/bin/env bash
# M48 — gdir receipt has no room_wire_pk; mint + browse separate from channel coin.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SSS_ROOT="${SSS_CHAIN_DIR:-/home/user/SSS_CHAIN}"

TMP="${TMPDIR:-/tmp}/its_memory_gdir_$$"
mkdir -p "$TMP"
trap 'rm -rf "$TMP"' EXIT

export ITS_MEMORY_HOME="$TMP/memory_home"
GDIR_REG="$ITS_MEMORY_HOME/coin/gdir/registry"
mkdir -p "$GDIR_REG"

cargo build --release --manifest-path "$SSS_ROOT/Cargo.toml" --quiet
cargo build --release --manifest-path "$ROOT/Cargo.toml" --quiet

COIN="$ROOT/target/release/its-coin"
export SSS_CHAIN_BIN="$SSS_ROOT/target/release/sss_chain"

"$COIN" gdir record --op sync --byte-span 4096
"$COIN" gdir record --op mirror --byte-span 8192
"$COIN" gdir mint --out "$TMP/gdir.coin.toml"

grep -q "ITS-GDIR-COIN/1" "$TMP/gdir.coin.toml"
grep -q "contrib_fp:" "$TMP/gdir.coin.toml"
if grep -q "room_wire_pk" "$TMP/gdir.coin.toml"; then
  echo "FAIL: gdir coin must not contain room_wire_pk" >&2
  exit 1
fi

"$COIN" gdir publish --manifest "$TMP/gdir.coin.toml" --registry "$GDIR_REG"
"$COIN" gdir browse --registry "$GDIR_REG" --sort contrib_bytes | grep -q "gdir contrib_fp="

echo "pipe_its_memory_gdir_e2e.sh: OK (M48)"
