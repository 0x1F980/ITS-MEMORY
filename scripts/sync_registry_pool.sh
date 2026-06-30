#!/usr/bin/env bash
# Pool mirror sync for channel + gdir coin registries (Fase 7B).
# Publish mode (default): push local *.coin.toml to ROUTING pool.
# Pull mode (--pull): ingest remote manifests from pool into local registry.
#
# Usage:
#   export ITS_MEMORY_HOME=~/.its/memory
#   export ITS_ROUTING_CONFIG=~/.its/chat/routing.toml
#   export ITS_RATCHET_SEED=~/.its/memory/ratchet.seed
#   bash scripts/sync_registry_pool.sh [--dry-run]
#   bash scripts/sync_registry_pool.sh --pull [--max-messages N]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
COIN="${ITS_COIN_BIN:-$ROOT/target/release/its-coin}"
MEMORY_HOME="${ITS_MEMORY_HOME:-$HOME/.its/memory}"
ROUTING_CFG="${ITS_ROUTING_CONFIG:-$HOME/.its/chat/routing.toml}"
RATCHET="${ITS_RATCHET_SEED:-$MEMORY_HOME/ratchet.seed}"
DRY=0
PULL=0
MAX=8
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY=1; shift ;;
    --pull) PULL=1; shift ;;
    --max-messages) MAX="${2:-8}"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 1 ;;
  esac
done

if [[ "$PULL" -eq 1 ]]; then
  if [[ "$DRY" -eq 1 ]]; then
    echo "[dry-run] $COIN channel ingest-pool -c $ROUTING_CFG --ratchet-seed $RATCHET --max-messages $MAX"
    echo "[dry-run] $COIN gdir ingest-pool -c $ROUTING_CFG --ratchet-seed $RATCHET --max-messages $MAX"
  else
    "$COIN" channel ingest-pool -c "$ROUTING_CFG" --ratchet-seed "$RATCHET" --max-messages "$MAX" --timeout-secs 90
    "$COIN" gdir ingest-pool -c "$ROUTING_CFG" --ratchet-seed "$RATCHET" --max-messages "$MAX" --timeout-secs 90
  fi
  echo "sync_registry_pool.sh: pull done"
  exit 0
fi

sync_dir() {
  local kind=$1
  local reg=$2
  local publish_fn=$3
  [[ -d "$reg" ]] || return 0
  shopt -s nullglob
  for man in "$reg"/*.coin.toml; do
    if [[ "$DRY" -eq 1 ]]; then
      echo "[dry-run] $COIN $publish_fn publish --manifest $man -c $ROUTING_CFG --ratchet-seed $RATCHET"
    else
      "$COIN" "$publish_fn" publish --manifest "$man" -c "$ROUTING_CFG" --ratchet-seed "$RATCHET"
    fi
  done
}

sync_dir channel "$MEMORY_HOME/coin/channel/registry" channel
sync_dir gdir "$MEMORY_HOME/coin/gdir/registry" gdir

echo "sync_registry_pool.sh: publish done"
