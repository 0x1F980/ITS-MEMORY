# ITS-MEMORY / ITS-COIN — proof manifest

## License: GNU GPLv3 Only

| Gate | Script | Claim |
|------|--------|-------|
| M40 | `scripts/pipe_its_memory_pin_e2e.sh` | Pin → fetch → decrypt yields seq 1..3 |
| M42 | `scripts/pipe_its_memory_coin_e2e.sh` | Two pin dirs → same SSS `chain_root` (`link_0`); validate OK |
| M43 | `scripts/pipe_its_memory_directory_e2e.sh` | Publish 2 rooms; browse sorts; no secret → 0 bits |

ITS-CHAT gate:

| Gate | Script | Claim |
|------|--------|-------|
| M41 | `../ITS-CHAT/scripts/pipe_its_chat_scroll_e2e.sh` | Scroll without local journal; sign parity |

## Run all

```bash
export ITS_ASYMMETRIC_DIR=/home/user/ITS-asymmetric \
       ITS_ROUTING_DIR=/home/user/ROUTING \
       ITS_OTM_DIR=/home/user/ITS-OTM_public_attestation \
       ITS_CHAT_DIR=/home/user/ITS-CHAT \
       SSS_CHAIN_DIR=/home/user/SSS_CHAIN
cd /home/user/ITS-MEMORY
cargo build --release && cargo test
bash scripts/pipe_its_memory_pin_e2e.sh
bash scripts/pipe_its_memory_coin_e2e.sh
bash scripts/pipe_its_memory_directory_e2e.sh
bash "$ITS_CHAT_DIR/scripts/pipe_its_chat_scroll_e2e.sh"
```

## Wire formats

- `ITS-MEMORY-PIN/1` — see `src/wire.rs`
- `ITS-COIN/1` — see `src/wire.rs`

No identity fields in MEMORY/COIN layers.
