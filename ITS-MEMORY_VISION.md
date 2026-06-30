# ITS-MEMORY / ITS-COIN — Vision, Postulates & Composition Spec

## License: GNU GPLv3 Only
## Target: Reviewers, operators, CHAT integrators

*(Composition specification — not new Shannon math. Wire ITS and SSS algebra are imported from sibling repos.)*

---

## Purpose

ITS-MEMORY and ITS-COIN form a **proof-of-hosting activity directory** for ITS-CHAT rooms:

1. **Replace meaningless mining** with measurable hosting work (bytes hosted, pins published, mirror uptime).
2. **Federate mirrors** — multiple hosts can pin the same `room_wire_pk`; quorum via matching SSS `chain_root` (`link_0`).
3. **Anti-hoard** — local pins without published mirror are not mintable (`--require-published`).
4. **Quiet discovery** — browse/search can rank **least active** open registries first to avoid spam rooms while still finding joinable channels.

**Reviewer task:** Read **§0.1** (worked example), postulates P0–P12, §12 checklist, then Appendices A–C to **confirm** or **reject** that implementation matches this specification.

> **Convention.** Metric names in §6 and postulates use documented semantics. The wire field `hosted_seconds` is documented as **`mirror_listed_seconds`** (channel-level mirror listing time since first publish).

**Related:** [ITS-MEMORY_PIPE.md](ITS-MEMORY_PIPE.md) · [ITS-MEMORY_KEEP_BOUNDARY.md](ITS-MEMORY_KEEP_BOUNDARY.md) · [PROOF_MANIFEST.md](PROOF_MANIFEST.md) · [ITS-CHAT/ITS_CHAT_ROOMS.md](../ITS-CHAT/ITS_CHAT_ROOMS.md)

---

## §0.2 Dual coin model (Fase 7)

**Two coins, two purposes — not the same thing:**

| | **CHANNEL coin** (`ITS-CHANNEL-COIN/3`) | **GDIR coin** (`ITS-GDIR-COIN/1`) |
|---|----------------------------------------|-----------------------------------|
| **Purpose** | **Memory proof** — host preserved channel history (messages as ciphertext pins) | **Blind global directory infra** — phonebook without channel knowledge |
| **Identity** | `room_wire_pk`, `room_id_fp`, SSS `chain_root` over pin bytes | `contrib_fp` only — **no** `room_wire_pk` |
| **Memory value** | Per-channel (`memory_weight_seconds`, bytes, pin lifetime) | Flat infra (`contrib_*`) — **not** channel ranking |
| **Host knows** | Which channel (required for channel mint/validate) | **Intentionally not** which channels shards belong to (GDIR blind role) |
| **Registry** | `coin/channel/registry/` — activity + quiet discovery | `coin/gdir/registry/` — flatten merge, blind assignment |
| **Deletion** | Remove pins → SSS validate fails → coin dysfunctional | Break shard-set → GDIR validate fails |

**Worked example:**

- **CHANNEL:** “plaza has hosted 500 pins for 90 days” → `memory_weight_seconds` = Σ hosted duration per pin.
- **GDIR:** “I helped 12 blind shards without room names” → `contrib_fp` + blind shard receipts.

**Operator rule:** CHANNEL coin ≠ GDIR coin. Join still uses CHAT registry OOB; COIN registries rank **activity** and **infra duty** separately.

---

## §0.1 Worked example (read this first)

**Setup:** Two open chat rooms publish channel coins to the same registry. Both have `registry_visible: true`.

| Room | Pins published | `frame_count` | Role |
|------|----------------|---------------|------|
| **spam-room** | 100 | 100 | High-activity spam channel |
| **quiet-room** | 2 | 2 | Low-activity, still open |

**Discovery (quiet-first):**

```bash
its-coin channel browse --sort frame_count --order asc
# or preset:
its-coin channel discover-quiet
```

**Expected order:** `quiet-room` appears **before** `spam-room` (ascending frame count).

**Join path:** Operator reads `room_id_fp` / `room_wire_pk` prefix from browse output, then uses CHAT registry:

```bash
its-chat room browse          # CHAT room manifest registry (~/.its/chat/registry/)
its-chat room join --room-id <HEX> --alias my-quiet
```

**Anti-spam filter:**

```bash
its-coin channel search --max-frames 5 --order asc
```

Only rooms with ≤5 frames match — `spam-room` (100 frames) is excluded; `quiet-room` remains.

**Content remains ITS:** Browse lists counts and pseudonyms only. Without `room.secret`, Eve learns **0 bits** of message plaintext (imported from ITS-asymmetric wire ITS).

---

## Postulates

| ID | Postulate |
|----|-----------|
| **P0** | **Pin format:** `ITS-MEMORY-PIN/1` stores neutral wire ciphertext (`wire_b64`, `wire_hash`, `pool_epoch`). No ITS-FRAME plaintext in pins. |
| **P1** | **No human identity in PIN:** Pins contain no username, email, or host legal name. Optional `seq_hint` is metadata only. |
| **P2** | **Read capability:** Decrypting pin content requires `room.secret` (ITS-asymmetric). Mirror hosts do not receive room secrets by default. |
| **P3** | **SSS chain root:** `ITS-CHANNEL-COIN/2` `chain_root` = SSS `link_0` hex over concatenated published wire bytes — backward underdetermination anchor (SSS_CHAIN import). |
| **P4** | **Anti-hoard:** `its-coin channel mint --require-published` rejects mint when no `.published` markers exist (gate M44). |
| **P5** | **Open registry:** `registry_visible: true` on coin manifest ⇒ included in `its-coin channel browse/search`. Hidden coins are filtered out (gate M45 via CHAT). |
| **P6** | **Metric semantics** (see §6 table). Doc alias: `hosted_seconds` ≡ `mirror_listed_seconds`. |
| **P7** | **GDIR separation:** `ITS-GDIR-COIN/1` aggregates infra (`contrib_ops`, `contrib_bytes`, `contrib_seconds`) with `contrib_fp` only — **no** `room_wire_pk`. |
| **P8** | **Quiet discovery:** Channel browse/search supports `--order asc|desc` and max/min activity filters (`--max-frames`, `--min-frames`, etc.). Preset: `discover-quiet` / `--discover quiet`. |
| **P9** | **Pseudonym fingerprints:** `host_fp` (channel coin) and `contrib_fp` (GDIR) are 16-hex pseudonyms from local `host.secret` — not “no identity,” but no legal-name binding. |
| **P10** | **Non-claim:** No automatic token reward, payment, or mining payout from coin mint alone. |
| **P11** | **Timelock + CHANNEL coin:** Optional `timelock_epoch_span`, `timelock_sealed_frames`; `memory_weight_seconds` credits only pins with `pool_epoch ≥ unlock_epoch` (gate M59). GDIR unchanged. |
| **P12** | **Pool ingest:** `its-coin channel|gdir ingest-pool` pulls manifests from ROUTING pool; `sync_registry_pool.sh --pull` (gate M54). |
| **P13** | **Dual coin:** CHANNEL = memory preservation proof; GDIR = blind flat phonebook infra. Separate registries, separate mint paths. |
| **P14** | **Quorum witnesses:** `ITS-MEMORY-WITNESS/1`; mint with `--require-quorum K` needs ≥K distinct `witness_fp` agreeing on `chain_root` + `pin_set_hash` (gate M55). |
| **P15** | **Blind GDIR:** `ITS-MEMORY-SHARD/1` stored under `blind_shards/` with **no** `room_wire_pk`; VRF `shard_id` assignment; `its-memory blind-pull` (gates M57/M58). Blindness applies to GDIR role only — CHANNEL host knows room when minting memory coin. |
| **P16** | **Flatten merge:** Cap single entry share in merged browse view (default 5% / `--cap-bps 500`); `discover-quiet-flat`, `gdir discover-flat` (gate M56). |
| **P17** | **Duty mint:** `--global` requires `--require-published`, GDIR receipt, and quorum; rank = directory sort only — no token payout (P10). |
| **P18** | **Memory–coin bind:** Destroying published pins breaks `channel validate` / `chain_root` (gates M53/M61). |

---

## §6 — Metric semantics (P6)

| Field | Semantics | Scope |
|-------|-----------|-------|
| `memory_bytes` | Sum of hosted wire ciphertext bytes across published pins | Channel coin |
| `frame_count` | Count of published pins in manifest | Channel coin |
| `pin_epoch_span` | `max(pool_epoch) − min(pool_epoch)` over pins | Message batch time window (pool epochs) |
| `message_hosted_span_seconds` | `max(published_at) − min(published_at)` from `.published` markers | Per-pin publish span (0 if single pin) |
| `hosted_seconds` *(doc: mirror_listed_seconds)* | `now − first_published` from host ledger | Channel mirror listing duration |
| `memory_weight_seconds` | Σ `(now − published_at)` per published pin (v3) | **Channel coin only** — memory preservation weight |
| `pin_hosted_min_seconds` / `pin_hosted_max_seconds` | Per-pin hosted duration stats (v3) | Channel coin |
| `timelock_epoch_span` / `timelock_sealed_frames` | Sealed pins below unlock epoch (v3) | Channel coin + ITS-CHAT timelock |
| `witness_count` | Distinct agreeing witnesses (merge/ingest) | Channel coin |
| `last_pool_epoch` | Latest pin pool epoch | Channel coin |
| `contrib_bytes` / `contrib_seconds` / `contrib_ops` | Aggregated GDIR infra ledger | GDIR coin only |

**Distinction:** GDIR does **not** get `memory_weight_seconds`. `mirror_listed_seconds` measures channel listing time on this host; `memory_weight_seconds` measures cumulative per-pin preservation duty.

---

## §12 — Review checklist

Confirm or reject each item against `src/` and gate scripts:

| # | Check | Gate / artifact |
|---|-------|-----------------|
| 1 | Pin roundtrip `ITS-MEMORY-PIN/1` | M40 |
| 2 | SSS `chain_root` stable across pin dirs | M42 |
| 3 | Directory publish + browse | M43 |
| 4 | `--require-published` anti-hoard | M44 |
| 5 | Sort keys DESC default; ASC quiet discovery | M46, M50 |
| 6 | `hosted_seconds` > 0 after publish | M47 |
| 7 | GDIR coin has no `room_wire_pk` | M48 |
| 8 | Fetch `--limit K` | M49 |
| 9 | `--max-frames` excludes spam | M51 |
| 10 | `pin_epoch_span` + `message_hosted_span_seconds` computed | M52 |
| 11 | Hidden registry absent from browse | M45 (CHAT) |
| 12 | No plaintext in coin/pin wire without room secret | KEEP_BOUNDARY |
| 13 | Pool ingest A publish → B browse | M54 |
| 14 | Quorum `--require-quorum K` anti-Eve self-list | M55 |
| 15 | `memory_weight_seconds` + pin delete breaks validate | M53 |
| 16 | Flatten merge anti-centralization | M56 |
| 17 | Blind GDIR shards + mint from blind receipts | M57, M58 |
| 18 | Timelock fields on CHANNEL coin | M59 |
| 19 | Duty mint `--global` + destruction gates | M60, M61 |

---

## Fase 7 roadmap (status)

| Phase | Deliverable | Gate | Status |
|-------|-------------|------|--------|
| **Del 0** | VISION v2 dual-coin, P13–P18, roadmap | — | **done** |
| **7B** | `channel/gdir ingest-pool`, merge, `sync_registry_pool.sh --pull` | M54 | **done** |
| **7C** | `ITS-MEMORY-WITNESS/1`, `--require-quorum` | M55 | **done** |
| **7A** | CHANNEL v3 `memory_weight_seconds` | M53 | **done** |
| **7E** | flatten, `discover-quiet-flat`, `gdir discover-flat` | M56 | **done** |
| **7D** | `ITS-MEMORY-SHARD/1`, `blind-pull` | M57, M58 | **done** |
| **7G** | timelock fields on CHANNEL coin | M59 | **done** |
| **7H** | `--global` duty mint | M60, M61 | **done** |

---

## Appendix A — CLI manpage

### its-memory

```
its-memory pin --room-wire-pk HEX -c routing.toml [--follow] [--max-messages N]
  [--ratchet-seed PATH] [--filter-pk PATH --filter-sk PATH] [--timeout-secs N]

its-memory fetch --room-wire-pk HEX --out DIR [--from-epoch N] [--to-epoch M] [--limit K]
  [--from-seq-hint N] [--mirror-dir PATH] [--filter-pk PATH --filter-sk PATH]

its-memory publish-pins --room-wire-pk HEX
its-memory host-status --room-wire-pk HEX
its-memory witness --room-wire-pk HEX --pin-dir PATH (--chain-root HEX | --manifest PATH)
its-memory blind-pull -c routing.toml [--ratchet-seed PATH] [--max-messages N]
```

### its-coin channel (ITS-CHANNEL-COIN/3)

```
its-coin channel mint --room-wire-pk HEX [--pin-dir PATH] [--out PATH]
  [--require-published] [--registry-hidden] [--global] [--require-quorum K]
  [--timelock-unlock-epoch N]
  [--decrypt-pk PATH --decrypt-sk PATH] [--room-id HEX] [--quorum-replicas N]

its-coin channel validate --manifest PATH [--pin-dir PATH]
its-coin channel publish --manifest PATH [--registry PATH] [--record-gdir]
  [-c routing.toml --ratchet-seed PATH]
its-coin channel ingest-pool -c routing.toml --ratchet-seed PATH [--registry PATH]

its-coin channel browse [--sort frame_count|memory_bytes|memory_weight_seconds|hosted_seconds|last_epoch]
  [--order asc|desc] [--discover quiet] [--registry PATH]

its-coin channel discover-quiet [--registry PATH]
its-coin channel discover-quiet-flat [--cap-bps 500] [--registry PATH]

its-coin channel search [--min-frames N] [--max-frames N]
  [--max-memory-bytes N] [--max-hosted-seconds N]
  [--sort ...] [--order asc|desc] [--registry PATH]
```

### its-coin gdir (ITS-GDIR-COIN/1)

```
its-coin gdir record --op mirror|sync|route|blind [--byte-span N]
its-coin gdir mint [--out PATH] [--require-blind]
its-coin gdir validate --manifest PATH
its-coin gdir publish --manifest PATH [--registry PATH]
its-coin gdir ingest-pool -c routing.toml --ratchet-seed PATH [--registry PATH]
its-coin gdir browse [--sort contrib_ops|contrib_bytes|contrib_seconds]
  [--order asc|desc] [--flatten] [--cap-bps 500]
its-coin gdir discover-flat [--cap-bps 500] [--registry PATH]
```

---

## Appendix B — Registry paths (three-registry model)

| Registry | Path | Owner | Contents |
|----------|------|-------|----------|
| **CHAT join registry** | `~/.its/chat/registry/<room_id>/` | ITS-CHAT | `public.key`, `room.toml`, `ITS-ROOM.manifest` — join OOB |
| **COIN channel registry** | `$ITS_MEMORY_HOME/coin/channel/registry/*.channel.coin.toml` | ITS-MEMORY | Activity manifests (`frame_count`, `memory_weight_seconds`, `chain_root`) |
| **COIN GDIR registry** | `$ITS_MEMORY_HOME/coin/gdir/registry/*.gdir.coin.toml` | ITS-MEMORY | Infra contribution (`contrib_fp`, no room) |
| **Witness store** | `$ITS_MEMORY_HOME/witnesses/<room_wire_pk>/` | ITS-MEMORY | `ITS-MEMORY-WITNESS/1` files |
| **Blind shards** | `$ITS_MEMORY_HOME/blind_shards/` | ITS-MEMORY | `ITS-MEMORY-SHARD/1` (no `room_wire_pk`) |
| **Legacy** | `$ITS_MEMORY_HOME/coin/registry/` | ITS-MEMORY | Migrated to channel registry on `ensure_layout()` |

**Operator flow:** CHAT registry for **join keys**; CHANNEL coin registry for **memory/activity ranking**; GDIR registry for **blind infra duty**. CHANNEL coin ≠ GDIR coin.

---

## Appendix C — Upstream delegation

| Concern | Owner repo | MEMORY/COIN role |
|---------|------------|------------------|
| Shannon wire ITS | ITS-asymmetric | Subprocess decrypt filter only |
| Pool transport | ROUTING (`its-routing`) | `pin` spawns `client-receive --pool` |
| SSS chain root | SSS_CHAIN | `sss_chain generate` at mint |
| Frame semantics | ITS-CHAT | Scroll subprocess; not in MEMORY |
| Ecosystem constitution | ITS-ROUTING/ITS_ECOSYSTEM.md | MEMORY listed as neutral mirror + directory |

No duplicate wire math or transport proofs in this repo.

---

## Non-claims (remain honest)

- Metadata-layer spam flood cannot be eliminated Shannon-style; flatten + quiet + quorum raise Eve cost.
- CHANNEL host **knows** `room_wire_pk` when minting memory coin — blindness is **GDIR role only**.
- CHAT join still OOB via `~/.its/chat/registry/`.
- No cryptocurrency payout (P10).

---

## Out of scope (post Fase 7)

- Automatic token economy / on-chain payout
- Full global witness federation protocol (witness files copied/merged manually or via ingest today)
