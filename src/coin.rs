use std::path::{Path, PathBuf};

use crate::error::{MemError, Result};
use crate::pipe::{self, parse_ssc_link0_hex, sss_chain_generate, sss_chain_validate};
use crate::host::{contrib_fp, hosted_seconds, now_unix};
use crate::gdir::has_gdir_receipt;
use crate::ingest::ingest_channel_pool;
use crate::mirror::{list_published_pins, published_at_for_pin, refresh_pin_hosted_markers};
use crate::room_timelock::{room_dir_from_decrypt_pk, timelock_unlock_pool_epoch};
use crate::vault::{has_pool_duty_witness, normalize_pk};
use crate::store::list_pins;
use crate::witness::{count_quorum_witnesses, require_quorum_met};
use crate::wire::{room_id_fingerprint, ChannelCoinManifest, MemoryPin};

pub type CoinManifest = ChannelCoinManifest;

pub const COIN_LINK_BYTE_LEN: usize = 16;

pub struct MintOptions {
    pub room_wire_pk: String,
    pub pin_dir: Option<PathBuf>,
    pub room_id: Option<String>,
    pub decrypt_pk: Option<PathBuf>,
    pub decrypt_sk: Option<PathBuf>,
    pub quorum_replicas: Option<u64>,
    pub require_quorum: Option<u64>,
    pub ssc_out: Option<PathBuf>,
    pub require_published: bool,
    pub registry_visible: bool,
    pub require_global: bool,
    pub timelock_unlock_epoch: Option<u64>,
    pub room_dir: Option<PathBuf>,
    pub pool_config: Option<PathBuf>,
    pub ratchet_seed: Option<PathBuf>,
}

pub fn pin_set_hash(pins: &[MemoryPin]) -> String {
    let mut hashes: Vec<&str> = pins.iter().map(|p| p.wire_hash.as_str()).collect();
    hashes.sort_unstable();
    wire_identity(hashes.join("|").as_bytes())
}

fn wire_identity(bytes: &[u8]) -> String {
    crate::wire::wire_identity(bytes)
}

pub fn mint_coin(opts: &MintOptions) -> Result<CoinManifest> {
    let pk_norm = normalize_pk(&opts.room_wire_pk);
    let pins = if opts.require_published {
        if let Some(ref dir) = opts.pin_dir {
            load_pins_from_dir(dir, &pk_norm)?
        } else {
            list_published_pins(&pk_norm)?
        }
    } else if let Some(ref dir) = opts.pin_dir {
        load_pins_from_dir(dir, &pk_norm)?
    } else {
        list_pins(&pk_norm)?
    };
    if pins.is_empty() {
        if opts.require_published {
            return Err(MemError::Coin(
                "no published pins to mint (run its-memory publish-pins first)".into(),
            ));
        }
        return Err(MemError::Coin("no pins to mint".into()));
    }

    if opts.require_global {
        if !opts.require_published {
            return Err(MemError::Coin(
                "--global requires --require-published (duty mint)".into(),
            ));
        }
        let quorum = opts.require_quorum.or(opts.quorum_replicas).unwrap_or(2);
        if quorum < 2 {
            return Err(MemError::Coin("--global requires --require-quorum >= 2".into()));
        }
        if !has_gdir_receipt()? {
            return Err(MemError::Coin(
                "--global requires GDIR receipt (run its-coin gdir record or its-memory blind-pull)".into(),
            ));
        }
        if !has_pool_duty_witness() {
            if let (Some(cfg), Some(ratchet)) = (&opts.pool_config, &opts.ratchet_seed) {
                let n = ingest_channel_pool(cfg, ratchet, None, 2, 20)?;
                if n == 0 && !has_pool_duty_witness() {
                    return Err(MemError::Coin(
                        "--global requires pool ingest witness (publish manifest to pool, then ingest-pool)".into(),
                    ));
                }
            } else {
                return Err(MemError::Coin(
                    "--global requires pool duty witness (run ingest-pool after publish, or pass -c/--ratchet-seed for auto-ingest)".into(),
                ));
            }
        }
    }

    let unlock_epoch = opts
        .timelock_unlock_epoch
        .or_else(|| {
            opts.room_dir
                .as_ref()
                .and_then(|d| timelock_unlock_pool_epoch(d).ok().flatten())
        })
        .or_else(|| {
            opts.decrypt_pk.as_ref().and_then(|pk| {
                room_dir_from_decrypt_pk(pk)
                    .and_then(|d| timelock_unlock_pool_epoch(&d).ok().flatten())
            })
        })
        .unwrap_or(0);
    let weight_pins: Vec<&MemoryPin> = if unlock_epoch > 0 {
        pins.iter()
            .filter(|p| p.pool_epoch >= unlock_epoch)
            .collect()
    } else {
        pins.iter().collect()
    };

    let memory_bytes: u64 = weight_pins
        .iter()
        .map(|p| p.wire_bytes().map(|b| b.len() as u64))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .sum();
    let hosted_secs = hosted_seconds(&pk_norm)?;
    let (pin_epoch_span, message_hosted_span_seconds) = pin_span_metrics(&pk_norm, &pins)?;
    let (memory_weight_seconds, pin_hosted_min_seconds, pin_hosted_max_seconds) =
        memory_weight_metrics(&pk_norm, &weight_pins)?;
    let (timelock_epoch_span, timelock_sealed_frames) =
        timelock_metrics(&pins, unlock_epoch);

    let mut last_seq = 0u64;
    let mut last_pool_epoch = 0u64;
    let mut room_id = opts.room_id.clone();

    for pin in &pins {
        last_pool_epoch = last_pool_epoch.max(pin.pool_epoch);
        let mut seq_part = pin
            .seq_hint
            .map(|s| s.to_string())
            .unwrap_or_default();
        if seq_part.is_empty() {
            if let (Some(pk), Some(sk)) = (&opts.decrypt_pk, &opts.decrypt_sk) {
                if let Some(seq) = decrypt_seq(pk, sk, pin)? {
                    seq_part = seq.to_string();
                    last_seq = last_seq.max(seq);
                }
            }
        } else {
            last_seq = last_seq.max(pin.seq_hint.unwrap_or(0));
        }
        if room_id.is_none() {
            if let (Some(pk), Some(sk)) = (&opts.decrypt_pk, &opts.decrypt_sk) {
                if let Some(rid) = decrypt_room_id(pk, sk, pin)? {
                    room_id = Some(rid);
                }
            }
        }
    }

    let payload = build_coin_payload(&pins)?;
    let (chain_root, ssc_path) = generate_sss_chain_root(&pk_norm, &payload)?;
    if !sss_chain_validate(&coin_root_path(&pk_norm), &ssc_path)? {
        return Err(MemError::Coin("sss_chain validate failed after mint".into()));
    }

    let quorum_required = opts
        .require_quorum
        .or(opts.quorum_replicas)
        .unwrap_or(0);
    if quorum_required > 0 {
        require_quorum_met(&pk_norm, &chain_root, &pins, quorum_required)?;
    }

    let witness_count = count_quorum_witnesses(&pk_norm, &chain_root, &pin_set_hash(&pins))?;
    if let Some(ref out) = opts.ssc_out {
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&ssc_path, out)?;
    }

    let room_id_fp = room_id
        .as_ref()
        .map(|r| room_id_fingerprint(r))
        .unwrap_or_else(|| "0000000000000000".into());

    Ok(CoinManifest {
        room_wire_pk: pk_norm,
        room_id_fp,
        chain_root,
        frame_count: pins.len() as u64,
        last_seq,
        last_pool_epoch,
        memory_bytes,
        hosted_seconds: hosted_secs,
        pin_epoch_span,
        message_hosted_span_seconds,
        memory_weight_seconds,
        pin_hosted_min_seconds,
        pin_hosted_max_seconds,
        timelock_epoch_span,
        timelock_sealed_frames,
        witness_count,
        registry_visible: opts.registry_visible,
        quorum_replicas: opts.quorum_replicas,
        host_fp: Some(contrib_fp()?),
    })
}

pub fn validate_coin(
    manifest: &CoinManifest,
    pin_dir: Option<&Path>,
    decrypt_pk: Option<&Path>,
    decrypt_sk: Option<&Path>,
    room_dir: Option<&Path>,
) -> Result<()> {
    let pk_norm = normalize_pk(&manifest.room_wire_pk);
    let pins = if let Some(dir) = pin_dir {
        load_pins_for_witness(dir, &pk_norm)?
    } else if manifest.memory_bytes > 0 || manifest.hosted_seconds > 0 {
        list_published_pins(&pk_norm)?
    } else {
        Vec::new()
    };
    if !pins.is_empty() {
        let _ = refresh_pin_hosted_markers(&pk_norm, &pins);
    }

    let timelock_unlock = room_dir
        .and_then(|d| timelock_unlock_pool_epoch(d).ok().flatten())
        .or_else(|| {
            decrypt_pk
                .and_then(room_dir_from_decrypt_pk)
                .and_then(|d| timelock_unlock_pool_epoch(&d).ok().flatten())
        });

    let recomputed = mint_coin(&MintOptions {
        room_wire_pk: manifest.room_wire_pk.clone(),
        pin_dir: pin_dir.map(|p| p.to_path_buf()),
        room_id: None,
        decrypt_pk: decrypt_pk.map(|p| p.to_path_buf()),
        decrypt_sk: decrypt_sk.map(|p| p.to_path_buf()),
        quorum_replicas: manifest.quorum_replicas,
        require_quorum: manifest.quorum_replicas,
        ssc_out: None,
        require_published: manifest.memory_bytes > 0 || manifest.hosted_seconds > 0,
        registry_visible: manifest.registry_visible,
        require_global: false,
        timelock_unlock_epoch: timelock_unlock,
        room_dir: room_dir.map(|p| p.to_path_buf()),
        pool_config: None,
        ratchet_seed: None,
    })?;
    if recomputed.chain_root != manifest.chain_root {
        return Err(MemError::Coin(format!(
            "chain_root mismatch: expected {} got {}",
            manifest.chain_root, recomputed.chain_root
        )));
    }
    if recomputed.frame_count != manifest.frame_count {
        return Err(MemError::Coin("frame_count mismatch".into()));
    }
    if recomputed.memory_bytes != manifest.memory_bytes && manifest.memory_bytes > 0 {
        return Err(MemError::Coin("memory_bytes mismatch".into()));
    }
    if recomputed.pin_epoch_span != manifest.pin_epoch_span && manifest.pin_epoch_span > 0 {
        return Err(MemError::Coin("pin_epoch_span mismatch".into()));
    }
    if recomputed.message_hosted_span_seconds != manifest.message_hosted_span_seconds
        && manifest.message_hosted_span_seconds > 0
    {
        return Err(MemError::Coin("message_hosted_span_seconds mismatch".into()));
    }
    if recomputed.memory_weight_seconds != manifest.memory_weight_seconds
        && manifest.memory_weight_seconds > 0
    {
        return Err(MemError::Coin("memory_weight_seconds mismatch".into()));
    }
    println!(
        "Validated ITS-CHANNEL-COIN room_wire_pk={}… frames={} memory_bytes={} memory_weight_seconds={} root={}… (SSS link_0)",
        &manifest.room_wire_pk[..8.min(manifest.room_wire_pk.len())],
        manifest.frame_count,
        manifest.memory_bytes,
        manifest.memory_weight_seconds,
        &manifest.chain_root[..8.min(manifest.chain_root.len())]
    );
    Ok(())
}

pub fn coin_root_material(room_wire_pk: &str) -> Vec<u8> {
    let mut out = b"ITS-COIN-sss-root-v1".to_vec();
    out.push(0);
    out.extend_from_slice(room_wire_pk.as_bytes());
    out
}

fn coin_root_path(room_wire_pk: &str) -> PathBuf {
    let short = &room_wire_pk[..16.min(room_wire_pk.len())];
    std::env::temp_dir().join(format!("its_coin_root_{short}"))
}

fn build_coin_payload(pins: &[MemoryPin]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for pin in pins {
        out.extend_from_slice(&pin.wire_bytes()?);
    }
    if out.is_empty() {
        out.extend_from_slice(b"ITS-COIN-empty");
    }
    Ok(out)
}

fn sss_total_bytes(payload_len: usize) -> usize {
    let raw_len = payload_len.max(COIN_LINK_BYTE_LEN);
    ((raw_len + COIN_LINK_BYTE_LEN - 1) / COIN_LINK_BYTE_LEN) * COIN_LINK_BYTE_LEN
}

fn generate_sss_chain_root(room_wire_pk: &str, payload: &[u8]) -> Result<(String, PathBuf)> {
    let root_path = coin_root_path(room_wire_pk);
    let root_bytes = coin_root_material(room_wire_pk);
    std::fs::write(&root_path, &root_bytes)?;

    let short = &room_wire_pk[..16.min(room_wire_pk.len())];
    let ssc_path = std::env::temp_dir().join(format!("its_coin_{short}.ssc"));
    let total_bytes = sss_total_bytes(payload.len());
    sss_chain_generate(
        &root_path,
        total_bytes,
        COIN_LINK_BYTE_LEN,
        &ssc_path,
    )?;

    let ssc_text = std::fs::read_to_string(&ssc_path)?;
    let link0 = parse_ssc_link0_hex(&ssc_text)?;
    Ok((link0, ssc_path))
}

fn pin_span_metrics(room_wire_pk: &str, pins: &[MemoryPin]) -> Result<(u64, u64)> {
    if pins.is_empty() {
        return Ok((0, 0));
    }
    let min_epoch = pins.iter().map(|p| p.pool_epoch).min().unwrap_or(0);
    let max_epoch = pins.iter().map(|p| p.pool_epoch).max().unwrap_or(0);
    let pin_epoch_span = max_epoch.saturating_sub(min_epoch);

    let mut published_times = Vec::new();
    for pin in pins {
        if let Some(ts) = published_at_for_pin(room_wire_pk, pin)? {
            published_times.push(ts);
        }
    }
    let message_hosted_span_seconds = if published_times.len() >= 2 {
        let min_ts = *published_times.iter().min().unwrap();
        let max_ts = *published_times.iter().max().unwrap();
        max_ts.saturating_sub(min_ts)
    } else {
        0
    };
    Ok((pin_epoch_span, message_hosted_span_seconds))
}

fn memory_weight_metrics(
    room_wire_pk: &str,
    pins: &[&MemoryPin],
) -> Result<(u64, u64, u64)> {
    if pins.is_empty() {
        return Ok((0, 0, 0));
    }
    let now = now_unix();
    let mut per_pin = Vec::new();
    for pin in pins {
        if let Some(published_at) = published_at_for_pin(room_wire_pk, pin)? {
            per_pin.push(now.saturating_sub(published_at));
        }
    }
    if per_pin.is_empty() {
        return Ok((0, 0, 0));
    }
    let memory_weight_seconds: u64 = per_pin.iter().sum();
    let pin_hosted_min_seconds = *per_pin.iter().min().unwrap();
    let pin_hosted_max_seconds = *per_pin.iter().max().unwrap();
    Ok((
        memory_weight_seconds,
        pin_hosted_min_seconds,
        pin_hosted_max_seconds,
    ))
}

fn timelock_metrics(pins: &[MemoryPin], unlock_epoch: u64) -> (u64, u64) {
    if unlock_epoch == 0 {
        return (0, 0);
    }
    let sealed: Vec<&MemoryPin> = pins
        .iter()
        .filter(|p| p.pool_epoch < unlock_epoch)
        .collect();
    if sealed.is_empty() {
        return (0, 0);
    }
    let min_epoch = sealed.iter().map(|p| p.pool_epoch).min().unwrap_or(0);
    let max_epoch = sealed.iter().map(|p| p.pool_epoch).max().unwrap_or(0);
    (
        max_epoch.saturating_sub(min_epoch),
        sealed.len() as u64,
    )
}

pub fn load_pins_for_witness(dir: &Path, room_wire_pk: &str) -> Result<Vec<MemoryPin>> {
    load_pins_from_dir(dir, room_wire_pk)
}

fn load_pins_from_dir(dir: &Path, room_wire_pk: &str) -> Result<Vec<MemoryPin>> {
    let mut pins = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) == Some("pin") {
            let pin = MemoryPin::read_file(&path)?;
            if seen.insert(pin.wire_hash.clone()) {
                pins.push(pin);
            }
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) == Some("wire") {
            let wire_bytes = std::fs::read(&path)?;
            let pin = crate::store::pin_from_wire(room_wire_pk, 0, &wire_bytes)?;
            if seen.insert(pin.wire_hash.clone()) {
                pins.push(pin);
            }
        }
    }
    pins.sort_by_key(|p| (p.pool_epoch, p.wire_hash.clone()));
    Ok(pins)
}

fn decrypt_seq(pk: &Path, sk: &Path, pin: &MemoryPin) -> Result<Option<u64>> {
    let wire = pin.wire_bytes()?;
    let tmp_w = std::env::temp_dir().join(format!("coin_w_{}_{}", pin.pool_epoch, pin.seq_hint.unwrap_or(0)));
    let tmp_f = std::env::temp_dir().join(format!("coin_f_{}_{}", pin.pool_epoch, pin.seq_hint.unwrap_or(0)));
    std::fs::write(&tmp_w, &wire)?;
    if pipe::its_asymmetric_decrypt(pk, sk, &tmp_w, &tmp_f).is_err() {
        let _ = std::fs::remove_file(&tmp_w);
        return Ok(None);
    }
    let frame = std::fs::read_to_string(&tmp_f).unwrap_or_default();
    let _ = std::fs::remove_file(&tmp_w);
    let _ = std::fs::remove_file(&tmp_f);
    Ok(pipe::try_parse_frame_seq(&frame))
}

fn decrypt_room_id(pk: &Path, sk: &Path, pin: &MemoryPin) -> Result<Option<String>> {
    let wire = pin.wire_bytes()?;
    let tmp_w = std::env::temp_dir().join(format!("coin_rid_w_{}_{}", pin.pool_epoch, pin.seq_hint.unwrap_or(0)));
    let tmp_f = std::env::temp_dir().join(format!("coin_rid_f_{}_{}", pin.pool_epoch, pin.seq_hint.unwrap_or(0)));
    std::fs::write(&tmp_w, &wire)?;
    if pipe::its_asymmetric_decrypt(pk, sk, &tmp_w, &tmp_f).is_err() {
        let _ = std::fs::remove_file(&tmp_w);
        return Ok(None);
    }
    let frame = std::fs::read_to_string(&tmp_f).unwrap_or_default();
    let _ = std::fs::remove_file(&tmp_w);
    let _ = std::fs::remove_file(&tmp_f);
    Ok(pipe::try_parse_frame_room_id(&frame))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coin_root_material_deterministic() {
        let a = coin_root_material("aa".repeat(64).as_str());
        let b = coin_root_material("aa".repeat(64).as_str());
        assert_eq!(a, b);
        assert!(a.starts_with(b"ITS-COIN-sss-root-v1"));
    }

    #[test]
    fn pin_span_metrics_from_pins() {
        let pins = vec![
            MemoryPin {
                room_wire_pk: "aa".repeat(32),
                pool_epoch: 3,
                wire_b64: String::new(),
                seq_hint: None,
                wire_hash: "a".repeat(64),
            },
            MemoryPin {
                room_wire_pk: "aa".repeat(32),
                pool_epoch: 11,
                wire_b64: String::new(),
                seq_hint: None,
                wire_hash: "b".repeat(64),
            },
        ];
        let (epoch_span, _) = pin_span_metrics("aa".repeat(32).as_str(), &pins).unwrap();
        assert_eq!(epoch_span, 8);
    }

    #[test]
    fn sss_total_bytes_aligns() {
        assert_eq!(sss_total_bytes(1), 16);
        assert_eq!(sss_total_bytes(16), 16);
        assert_eq!(sss_total_bytes(17), 32);
    }
}
