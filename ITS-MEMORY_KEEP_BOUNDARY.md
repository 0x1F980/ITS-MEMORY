# ITS-MEMORY: identity boundary

## License: GNU GPLv3 Only

## In scope

| Component | Location | Role |
|-----------|----------|------|
| ITS-MEMORY-PIN/1 | `src/wire.rs`, `src/pin.rs`, `src/store.rs` | Neutral ciphertext mirror |
| ITS-MEMORY-WITNESS/1 | `src/witness.rs` | Quorum attestation (distinct `witness_fp`) |
| ITS-MEMORY-SHARD/1 | `src/blind.rs`, `src/wire.rs` | Blind GDIR shard (no `room_wire_pk` on disk) |
| Pin / fetch CLI | `src/cli_memory.rs`, `its-memory` bin | Pool ingest, witness, blind-pull |
| ITS-CHANNEL-COIN/3 | `src/wire.rs`, `src/coin.rs` | SSS `link_0` + memory_weight + timelock fields (v2 compatible) |
| ITS-GDIR-COIN/1 | `src/gdir.rs` | Infra contribution (`contrib_fp` only) |
| Pool ingest | `src/ingest.rs` | Pull manifests from ROUTING pool |
| Directory | `src/directory.rs`, `its-coin` bin | publish / browse / search / quiet-flat discovery |
| Vault | `src/vault.rs` | `~/.its/memory/` pins, mirrors, witnesses, blind_shards, registries |
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
its-memory pin      → its-routing client-receive --pool [--follow]
its-memory witness  → ITS-MEMORY-WITNESS/1 quorum files
its-memory blind-pull → ITS-MEMORY-SHARD/1 (GDIR duty, no room identity)
its-memory fetch    → local pin store; optional its_asymmetric decrypt filter
its-coin mint       → sss_chain generate (root=room_wire_pk material)
its-coin ingest-pool → pull channel/gdir manifests from pool
its-chat scroll     → its-memory fetch → its_asymmetric decrypt
its-chat discover   → its-coin channel discover-quiet-flat (wrapper)
```

## Dual coin boundary (Fase 7)

| Coin | Knows room? | Registry path |
|------|-------------|---------------|
| **CHANNEL** | Yes (`room_wire_pk`) | `coin/channel/registry/` |
| **GDIR** | No (blind shards) | `coin/gdir/registry/` + `blind_shards/` |

CHANNEL host **must** know room for memory-coin mint/validate. GDIR blind role **must not** persist `room_wire_pk` in shard files.

## Pseudonym fingerprints (not “no identity”)

| Field | Source | Semantics |
|-------|--------|-----------|
| `host_fp` | First 16 hex of `host.secret` | Channel coin — pseudonymous host tag |
| `contrib_fp` | Same `host.secret` material | GDIR coin — pseudonymous infra contributor |
| `witness_fp` | Same `host.secret` material | Quorum witness attestation |

These are **local pseudonyms**, not legal-name identity and not room membership proofs.
No human-readable names in pins or coin manifests.

## No plaintext on wire

Pins and coin manifests expose ciphertext counts, epochs, and SSS roots only.
Message content requires `room.secret` (ITS-asymmetric decrypt).

See [ITS-MEMORY_VISION.md](ITS-MEMORY_VISION.md) for full postulates (P0–P18).
