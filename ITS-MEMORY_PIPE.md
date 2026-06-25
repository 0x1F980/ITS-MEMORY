# ITS-MEMORY / ITS-COIN — subprocess contracts

## Environment

| Variable | Default | Purpose |
|----------|---------|---------|
| `ITS_MEMORY_HOME` | `~/.its/memory` | Pin store + coin registry |
| `ITS_MEMORY_BIN` | `its-memory` | Used by ITS-CHAT scroll |
| `ITS_ROUTING_BIN` | `its-routing` | Pool receive/send |
| `ITS_ASYMMETRIC_BIN` | `its_asymmetric` | Optional decrypt filter |

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
its-memory fetch --room-wire-pk HEX --out DIR [--from-epoch N]
  [--filter-pk PATH --filter-sk PATH]
```

Exports `.wire` + `.pin` files sorted by pool epoch. Filter keys drop wires that fail decrypt.

## its-coin mint / validate

```
its-coin mint --room-wire-pk HEX [--pin-dir PATH] [--out PATH]
  [--decrypt-pk PATH --decrypt-sk PATH] [--room-id HEX] [--ssc-out PATH]
its-coin validate --manifest PATH [--pin-dir PATH] [--decrypt-pk PATH --decrypt-sk PATH]
```

`chain_root` = SSS `link_0` hex from `sss_chain generate` over concatenated wire ciphertext bytes
(`--root` = `ITS-COIN-sss-root-v1 || room_wire_pk`, `--total-bytes` = padded payload length).
Requires `sss_chain` on PATH (`SSS_CHAIN_BIN`).

## its-coin directory

```
its-coin publish --manifest PATH [--registry PATH] [-c routing.toml --ratchet-seed PATH]
its-coin browse [--sort frame_count|last_epoch] [--registry PATH]
its-coin search --min-frames N [--sort frame_count|last_epoch]
```

Registry default: `$ITS_MEMORY_HOME/coin/registry/*.coin.toml`.

## ITS-CHAT scroll

```
its-chat scroll --room ALIAS [--from-seq N] [--memory-home PATH] [--no-strict-publish]
```

Subprocess: `its-memory fetch` → local decrypt → ITS-FRAME display (same rules as listen).
