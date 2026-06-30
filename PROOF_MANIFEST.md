# ITS-MEMORY / ITS-COIN — proof manifest

## License: GNU GPLv3 Only

| Gate | Script | Claim |
|------|--------|-------|
| M40 | `scripts/pipe_its_memory_pin_e2e.sh` | Pin → fetch → decrypt yields seq 1..3 |
| M42 | `scripts/pipe_its_memory_coin_e2e.sh` | Two pin dirs → same SSS `chain_root` (`link_0`); validate OK |
| M43 | `scripts/pipe_its_memory_directory_e2e.sh` | Publish 2 rooms; browse sorts; no secret → 0 bits |
| M44 | `scripts/pipe_its_memory_publish_e2e.sh` | Lokal pin uden publish → `--require-published` fejler |
| M46 | `scripts/pipe_its_memory_browse_sort_e2e.sh` | `channel browse --sort memory_bytes` vs `frame_count` rank korrekt |
| M47 | `scripts/pipe_its_memory_host_status_e2e.sh` | `host-status` + mint efter publish → `hosted_seconds` > 0 |
| M48 | `scripts/pipe_its_memory_gdir_e2e.sh` | GDIR coin uden `room_wire_pk`; separat browse |
| M49 | `scripts/pipe_its_memory_scroll_query_e2e.sh` | `fetch --limit K` returnerer seneste K pins |
| M50 | `scripts/pipe_its_memory_discover_quiet_e2e.sh` | `--order asc` / `discover-quiet` → quiet-room før spam-room |
| M51 | `scripts/pipe_its_memory_search_max_frames_e2e.sh` | `--max-frames N` ekskluderer høj-aktivitet kanaler |
| M52 | `scripts/pipe_its_memory_message_hosted_span_e2e.sh` | Staggered publish → `message_hosted_span_seconds` > 0, `pin_epoch_span` > 0 |
| M53 | `scripts/pipe_its_memory_memory_weight_e2e.sh` | `memory_weight_seconds` > 0; pin delete → validate fails |
| M54 | `scripts/pipe_its_memory_ingest_pool_e2e.sh` | A publish → B ingest-pool → browse match |
| M55 | `scripts/pipe_its_memory_quorum_e2e.sh` | Solo `--require-quorum 2` fails; 2+ distinct witness_fp OK |
| M56 | `scripts/pipe_its_memory_flatten_e2e.sh` | Mega + tiny channel; `discover-quiet-flat` shows both |
| M57/M58 | `scripts/pipe_its_memory_blind_gdir_e2e.sh` | Blind shards have no `room_wire_pk`; GDIR mint `--require-blind` |
| M59 | `scripts/pipe_its_memory_timelock_coin_e2e.sh` | Pre-unlock pins excluded from `memory_weight_seconds` |
| M60/M61 | `scripts/pipe_its_memory_duty_mint_e2e.sh` | `--global` without GDIR fails; pin delete breaks validate |

ITS-CHAT gates:

| Gate | Script | Claim |
|------|--------|-------|
| M41 | `../ITS-CHAT/scripts/pipe_its_chat_scroll_e2e.sh` | Scroll without local journal; sign parity |
| M45 | `../ITS-CHAT/scripts/pipe_its_chat_registry_hidden_e2e.sh` | Hidden registry absent from browse |
| M49 | `../ITS-CHAT/scripts/pipe_its_chat_scroll_query_e2e.sh` | Scroll `--last`, `--from-seq/--to-seq`, `--after/--before` |

## Run all

```bash
export ITS_ASYMMETRIC_DIR=/home/user/ITS-asymmetric \
       ITS_ROUTING_DIR=/home/user/ROUTING \
       ITS_OTM_DIR=/home/user/ITS-OTM_public_attestation \
       ITS_CHAT_DIR=/home/user/ITS-CHAT \
       SSS_CHAIN_DIR=/home/user/SSS_CHAIN
cd /home/user/ITS-MEMORY
cargo build --release && cargo test
for s in scripts/pipe_its_memory_*.sh; do bash "$s"; done
bash "$ITS_CHAT_DIR/scripts/pipe_its_chat_scroll_e2e.sh"
bash "$ITS_CHAT_DIR/scripts/pipe_its_chat_registry_hidden_e2e.sh"
bash "$ITS_CHAT_DIR/scripts/pipe_its_chat_scroll_query_e2e.sh"
```

## Wire formats

- `ITS-MEMORY-PIN/1` — see `src/wire.rs`
- `ITS-CHANNEL-COIN/3` — kanal mindebevis (memory_weight_seconds, timelock fields, witness_count; v2 compatible)
- `ITS-MEMORY-WITNESS/1` — quorum witness attestation
- `ITS-MEMORY-SHARD/1` — blind GDIR shard (no room_wire_pk)
- `ITS-GDIR-COIN/1` — global directory infra (ingen room_wire_pk)
- Legacy `ITS-COIN/1` — alias for channel coin v1

No human identity fields in MEMORY/COIN layers. Pseudonym tags: `host_fp`, `contrib_fp` (16-hex from local host.secret).

## Registry layout

| Path | Purpose |
|------|---------|
| `coin/channel/registry/` | Channel coin manifests |
| `coin/gdir/registry/` | GDIR coin manifests |
| `coin/registry/` | Legacy — migrated on `ensure_layout()` |

Optional pool sync: `scripts/sync_registry_pool.sh` (publish) or `--pull` (ingest).
