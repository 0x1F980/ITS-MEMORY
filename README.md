# ITS-MEMORY / ITS-COIN

Neutral wire mirrors and global activity directory for ITS-CHAT rooms.

**ITS-COIN** binds activity with **SSS chain** (`sss_chain` subprocess): `chain_root` is `link_0` hex — information-theoretic backward underdetermination, not SHA/Merkle.

## License

GNU GPLv3 Only

## Quick start

```bash
export ITS_ASYMMETRIC_DIR=/home/user/ITS-asymmetric \
       ITS_ROUTING_DIR=/home/user/ROUTING \
       ITS_MEMORY_HOME=/tmp/its_memory_demo
cargo build --release
its-memory pin --room-wire-pk HEX -c routing.toml --follow --max-messages 5
its-memory fetch --room-wire-pk HEX --out /tmp/pins --filter-pk public.key --filter-sk secret.key
its-coin mint --room-wire-pk HEX --pin-dir "$ITS_MEMORY_HOME/pins/HEX" --out coin.toml
its-coin publish --manifest coin.toml
its-coin browse --sort frame_count
```

ITS-CHAT scroll integration:

```bash
its-chat scroll --room ALIAS --from-seq 1 --memory-home "$ITS_MEMORY_HOME"
```

## Gates

```bash
bash scripts/pipe_its_memory_pin_e2e.sh      # M40
bash scripts/pipe_its_memory_coin_e2e.sh     # M42
bash scripts/pipe_its_memory_directory_e2e.sh # M43
```

See [ITS-MEMORY_KEEP_BOUNDARY.md](ITS-MEMORY_KEEP_BOUNDARY.md) and [PROOF_MANIFEST.md](PROOF_MANIFEST.md).
