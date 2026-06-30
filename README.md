# ITS-MEMORY / ITS-COIN

Neutral wire mirrors and global activity directory for ITS-CHAT rooms.

**Reviewer entry:** [ITS-MEMORY_VISION.md](ITS-MEMORY_VISION.md) — purpose, §0.1 worked example, postulates P0–P12, checklist, CLI manpage.

**ITS-COIN** binds activity with **SSS chain** (`sss_chain` subprocess): `chain_root` is `link_0` hex — information-theoretic backward underdetermination, not SHA/Merkle.

Two coin types (Fase 6):

| Type | CLI | Registry |
|------|-----|----------|
| **ITS-CHANNEL-COIN/2** | `its-coin channel mint\|publish\|browse` | `coin/channel/registry/` |
| **ITS-GDIR-COIN/1** | `its-coin gdir record\|mint\|publish\|browse` | `coin/gdir/registry/` |

## License

GNU GPLv3 Only

## Quick start

```bash
export ITS_ASYMMETRIC_DIR=/home/user/ITS-asymmetric \
       ITS_ROUTING_DIR=/home/user/ROUTING \
       ITS_MEMORY_HOME=/tmp/its_memory_demo
cargo build --release
its-memory pin --room-wire-pk HEX -c routing.toml --follow --max-messages 5
its-memory publish-pins --room-wire-pk HEX
its-memory fetch --room-wire-pk HEX --out /tmp/pins \
  --from-epoch N --to-epoch M --limit K --mirror-dir ~/.its/memory/mirrors/HEX \
  --filter-pk public.key --filter-sk secret.key
its-coin channel mint --room-wire-pk HEX --require-published --out coin.toml
its-coin channel publish --manifest coin.toml
its-coin channel browse --sort frame_count --order asc   # quiet discovery
its-coin channel discover-quiet
its-coin channel search --max-frames 5 --order asc         # exclude spam rooms
its-coin gdir record --op sync --byte-span 4096
its-coin gdir mint --out gdir.toml
its-coin gdir browse --sort contrib_bytes --order asc
```

ITS-CHAT scroll integration (query filters):

```bash
its-chat scroll --room ALIAS --from-seq 1 --to-seq 100 --memory-home "$ITS_MEMORY_HOME"
its-chat scroll --room ALIAS --last 20
its-chat scroll --room ALIAS --after 2026-06-01 --before 2026-06-22
its-chat room create --alias plaza --type chat --registry visible   # default
its-chat room create --alias den --type chat --registry hidden
```

Optional pool registry sync: `bash scripts/sync_registry_pool.sh`.

## Gates

```bash
bash scripts/pipe_its_memory_pin_e2e.sh                    # M40
bash scripts/pipe_its_memory_coin_e2e.sh                   # M42
bash scripts/pipe_its_memory_directory_e2e.sh              # M43
bash scripts/pipe_its_memory_publish_e2e.sh                # M44
bash scripts/pipe_its_memory_browse_sort_e2e.sh            # M46
bash scripts/pipe_its_memory_host_status_e2e.sh            # M47
bash scripts/pipe_its_memory_gdir_e2e.sh                   # M48
bash scripts/pipe_its_memory_scroll_query_e2e.sh           # M49
bash scripts/pipe_its_memory_discover_quiet_e2e.sh         # M50
bash scripts/pipe_its_memory_search_max_frames_e2e.sh      # M51
bash scripts/pipe_its_memory_message_hosted_span_e2e.sh    # M52
```

See [ITS-MEMORY_VISION.md](ITS-MEMORY_VISION.md), [ITS-MEMORY_KEEP_BOUNDARY.md](ITS-MEMORY_KEEP_BOUNDARY.md), and [PROOF_MANIFEST.md](PROOF_MANIFEST.md).
