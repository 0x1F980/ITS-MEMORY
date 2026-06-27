#!/usr/bin/env bash
# Optional pool mirror sync for channel + gdir coin registries (Fase 6f).
# Publishes each *.coin.toml under coin/channel/registry and coin/gdir/registry via ROUTING pool.
#
# Usage:
#   export ITS_MEMORY_HOME=~/.its/memory
#   export ITS_ROUTING_CONFIG=~/.its/chat/routing.toml
#   export ITS_RATCHET_SEED=~/.its/memory/ratchet.seed
#   bash scripts/sync_registry_pool.sh [--dry-run]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
COIN="${ITS_COIN_BIN:-$ROOT/target/release/its-coin}"
MEMORY_HOME="${ITS_MEMORY_HOME:-$HOME/.its/memory}"
ROUTING_CFG="${ITS_ROUTING_CONFIG:-$HOME/.its/chat/routing.toml}"
RATCHET="${ITS_RATCHET_SEED:-$MEMORY_HOME/ratchet.seed}"
DRY=0
[[ "${1:-}" == "--dry-run" ]] && DRY=1

sync_dir() {
  local kind=$1
  local reg=$2
  local publish_fn=$3
  [[ -d "$reg" ]] || return 0
  shopt -s nullglob
  for man in "$reg"/*.coin.toml; do
    if [[ "$DRY" -eq 1 ]]; then
      echo "[dry-run] $publish_fn --manifest $man -c $ROUTING_CFG --ratchet-seed $RATCHET"
    else
      "$COIN" "$publish_fn" --manifest "$man" -c "$ROUTING_CFG" --ratchet-seed "$RATCHET"
    fi
  done
}

sync_dir channel "$MEMORY_HOME/coin/channel/registry" channel
sync_dir gdir "$MEMORY_HOME/coin/gdir/registry" gdir

echo "sync_registry_pool.sh: done"
