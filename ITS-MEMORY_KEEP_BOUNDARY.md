# ITS-MEMORY: identity boundary

## License: GNU GPLv3 Only

## In scope

| Component | Location | Role |
|-----------|----------|------|
| ITS-MEMORY-PIN/1 | `src/wire.rs`, `src/pin.rs`, `src/store.rs` | Neutral ciphertext mirror |
| Pin / fetch CLI | `src/cli_memory.rs`, `its-memory` bin | Pool `--follow` ingest, export |
| ITS-COIN/1 | `src/wire.rs`, `src/coin.rs` | SSS chain head (`link_0`) over pin payload span |
| Directory | `src/directory.rs`, `its-coin` bin | publish / browse / search |
| Vault | `src/vault.rs` | `~/.its/memory/` pins + coin registry |
| Subprocess glue | `src/pipe.rs` | PATH-only integration |

## Out of scope

| Concern | Owner repo |
|---------|------------|
| ITS-FRAME/1 semantics, send/listen | ITS-CHAT |
| Shannon encrypt/decrypt | ITS-asymmetric |
| OTM sign/verify | ITS-OTM |
| UES pool transport | ROUTING |

## Integration

```
its-memory pin   → its-routing client-receive --pool [--follow]
its-memory fetch → (local pin store; optional its_asymmetric decrypt filter)
its-coin mint    → sss_chain generate (root=room_wire_pk material, total-bytes=payload span)
its-chat scroll  → its-memory fetch (subprocess) → its_asymmetric decrypt
```

No host identity fields in pins or coin manifests.
