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
| **P11** | **Non-claim:** ITS-timelock + coin coupling (Fase 6) not enforced in v1. |
| **P12** | **Non-claim:** Pool ingest of coin manifests is optional (`sync_registry_pool.sh` publish only); `quorum_replicas` field is not enforced by directory code. |

---

## §6 — Metric semantics (P6)

| Field | Semantics | Scope |
|-------|-----------|-------|
| `memory_bytes` | Sum of hosted wire ciphertext bytes across published pins | Channel coin |
| `frame_count` | Count of published pins in manifest | Channel coin |
| `pin_epoch_span` | `max(pool_epoch) − min(pool_epoch)` over pins | Message batch time window (pool epochs) |
| `message_hosted_span_seconds` | `max(published_at) − min(published_at)` from `.published` markers | Per-pin publish span (0 if single pin) |
| `hosted_seconds` *(doc: mirror_listed_seconds)* | `now − first_published` from host ledger | Channel mirror listing duration |
| `last_pool_epoch` | Latest pin pool epoch | Channel coin |
| `contrib_bytes` / `contrib_seconds` / `contrib_ops` | Aggregated GDIR infra ledger | GDIR coin only |

**Distinction:** `mirror_listed_seconds` measures how long the channel has been listed on **this host's mirror** since first publish. `message_hosted_span_seconds` measures the **spread of pin publish timestamps** — closer to “how long messages have been hosted” as a batch.

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
```

### its-coin channel (ITS-CHANNEL-COIN/2)

```
its-coin channel mint --room-wire-pk HEX [--pin-dir PATH] [--out PATH]
  [--require-published] [--registry-hidden]
  [--decrypt-pk PATH --decrypt-sk PATH] [--room-id HEX] [--quorum-replicas N]

its-coin channel validate --manifest PATH [--pin-dir PATH]
its-coin channel publish --manifest PATH [--registry PATH] [--record-gdir]
  [-c routing.toml --ratchet-seed PATH]

its-coin channel browse [--sort frame_count|last_epoch|memory_bytes|hosted_seconds]
  [--order asc|desc] [--discover quiet] [--registry PATH]

its-coin channel discover-quiet [--registry PATH]

its-coin channel search [--min-frames N] [--max-frames N]
  [--max-memory-bytes N] [--max-hosted-seconds N]
  [--sort ...] [--order asc|desc] [--registry PATH]
```

### its-coin gdir (ITS-GDIR-COIN/1)

```
its-coin gdir record --op mirror|sync|route [--byte-span N]
its-coin gdir mint [--out PATH]
its-coin gdir validate --manifest PATH
its-coin gdir publish --manifest PATH [--registry PATH]
its-coin gdir browse [--sort contrib_ops|contrib_bytes|contrib_seconds]
  [--order asc|desc] [--registry PATH]
```

---

## Appendix B — Registry paths (two-registry model)

| Registry | Path | Owner | Contents |
|----------|------|-------|----------|
| **CHAT room registry** | `~/.its/chat/registry/<room_id>/` | ITS-CHAT | `public.key`, `room.toml`, `ITS-ROOM.manifest` — join OOB |
| **COIN channel registry** | `$ITS_MEMORY_HOME/coin/channel/registry/*.channel.coin.toml` | ITS-MEMORY | Activity manifests (`frame_count`, metrics, `chain_root`) |
| **COIN GDIR registry** | `$ITS_MEMORY_HOME/coin/gdir/registry/*.gdir.coin.toml` | ITS-MEMORY | Infra contribution (`contrib_fp`, no room) |
| **Legacy** | `$ITS_MEMORY_HOME/coin/registry/` | ITS-MEMORY | Migrated to channel registry on `ensure_layout()` |

**Operator flow:** CHAT registry for **join keys**; COIN registry for **activity ranking** and quiet discovery.

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

## Planned (out of v1 scope)

- ITS-timelock + coin-kobling (Fase 6)
- Pool **ingest** of coin manifests (publish-only in `sync_registry_pool.sh`)
- Automatic belønning / token economy
- `quorum_replicas` enforcement
