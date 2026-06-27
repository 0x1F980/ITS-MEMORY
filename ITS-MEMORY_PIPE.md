# ITS-MEMORY / ITS-COIN — subprocess contracts

## Environment

| Variable | Default | Purpose |
|----------|---------|---------|
| `ITS_MEMORY_HOME` | `~/.its/memory` | Pin store + coin registry |
| `ITS_MEMORY_BIN` | `its-memory` | Used by ITS-CHAT scroll |
| `ITS_ROUTING_BIN` | `its-routing` | Pool receive/send |
| `ITS_ASYMMETRIC_BIN` | `its_asymmetric` | Optional decrypt filter |
| `ITS_POOL_EPOCH_ANCHOR_UNIX` | `0` | CHAT scroll date ↔ pool_epoch mapping |

## its-memory pin

```
its-memory pin --room-wire-pk HEX -c routing.toml [--follow] [--max-messages N]
  [--ratchet-seed PATH] [--filter-pk PATH --filter-sk PATH] [--timeout-secs N]
```

Spawns `its-routing -c CFG client-receive --pool [--follow] --ratchet-seed-file SEED`.

Writes `ITS-MEMORY-PIN/1` under `$ITS_MEMORY_HOME/pins/<room_wire_pk>/<tag>.pin`.

Dedup: full `wire_hash = hex(ciphertext)` in pin body; filename tag is short prefix/suffix (not SHA).

## its-memory fetch

```
its-memory fetch --room-wire-pk HEX --out DIR [--from-epoch N] [--to-epoch M] [--limit K]
  [--from-seq-hint N] [--mirror-dir PATH] [--routing-config PATH]
  [--filter-pk PATH --filter-sk PATH]
```

Exports `.wire` + `.pin` files sorted by pool epoch. `--limit K` returns the latest K pins.
`--mirror-dir` reads from published mirror instead of local pin vault.

## its-memory publish-pins / host-status

```
its-memory publish-pins --room-wire-pk HEX
its-memory host-status --room-wire-pk HEX
```

`publish-pins` copies pins to `$ITS_MEMORY_HOME/mirrors/<room_wire_pk>/` with `.published` markers
and updates the local host ledger (`hosted_seconds` after publish).

## its-coin channel (ITS-CHANNEL-COIN/2)

```
its-coin channel mint --room-wire-pk HEX [--pin-dir PATH] [--require-published] [--out PATH]
  [--decrypt-pk PATH --decrypt-sk PATH] [--room-id HEX] [--registry-hidden]
its-coin channel validate --manifest PATH [--pin-dir PATH]
its-coin channel publish --manifest PATH [--registry PATH] [-c routing.toml --ratchet-seed PATH]
its-coin channel browse [--sort frame_count|last_epoch|memory_bytes|hosted_seconds]
its-coin channel search --min-frames N [--sort ...]
```

Registry default: `$ITS_MEMORY_HOME/coin/channel/registry/*.channel.coin.toml`.
Legacy `$ITS_MEMORY_HOME/coin/registry/` is migrated on first `ensure_layout()`.

`chain_root` = SSS `link_0` hex from `sss_chain generate` over concatenated wire ciphertext bytes.

## its-coin gdir (ITS-GDIR-COIN/1)

```
its-coin gdir record --op mirror|sync|route [--byte-span N]
its-coin gdir mint [--out PATH]
its-coin gdir publish --manifest PATH [--registry PATH]
its-coin gdir browse [--sort contrib_ops|contrib_bytes|contrib_seconds]
```

GDIR receipts and coins contain **no** `room_wire_pk` — aggregated directory infra only.

## Pool registry sync (optional)

```
bash scripts/sync_registry_pool.sh [--dry-run]
```

Requires `ITS_ROUTING_CONFIG`, `ITS_RATCHET_SEED`, built `its-coin`.

## ITS-CHAT scroll

```
its-chat scroll --room ALIAS [--from-seq N] [--to-seq M] [--at-seq K]
  [--last K] [--limit K] [--after DATE] [--before DATE]
  [--memory-home PATH] [--mirror-dir PATH] [--fetch-dir PATH] [--no-strict-publish]
its-chat room create --alias NAME --type chat [--registry visible|hidden]
```

Subprocess: `its-memory fetch` (with epoch/limit/mirror prefilter) → local decrypt → ITS-FRAME query filter → display.

DATE = `YYYY-MM-DD` or unix seconds. Wire time from pin `pool_epoch` + `routing.toml` `epoch_interval_ms`.
