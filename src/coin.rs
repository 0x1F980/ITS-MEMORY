use std::path::{Path, PathBuf};

use crate::error::{MemError, Result};
use crate::pipe::{self, parse_ssc_link0_hex, sss_chain_generate, sss_chain_validate};
use crate::store::list_pins;
use crate::vault::normalize_pk;
use crate::wire::{room_id_fingerprint, CoinManifest, MemoryPin};

pub const COIN_LINK_BYTE_LEN: usize = 16;

pub struct MintOptions {
    pub room_wire_pk: String,
    pub pin_dir: Option<PathBuf>,
    pub room_id: Option<String>,
    pub decrypt_pk: Option<PathBuf>,
    pub decrypt_sk: Option<PathBuf>,
    pub quorum_replicas: Option<u64>,
    pub ssc_out: Option<PathBuf>,
}

pub fn mint_coin(opts: &MintOptions) -> Result<CoinManifest> {
    let pk_norm = normalize_pk(&opts.room_wire_pk);
    let pins = if let Some(ref dir) = opts.pin_dir {
        load_pins_from_dir(dir, &pk_norm)?
    } else {
        list_pins(&pk_norm)?
    };
    if pins.is_empty() {
        return Err(MemError::Coin("no pins to mint".into()));
    }

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
        quorum_replicas: opts.quorum_replicas,
    })
}

pub fn validate_coin(
    manifest: &CoinManifest,
    pin_dir: Option<&Path>,
    decrypt_pk: Option<&Path>,
    decrypt_sk: Option<&Path>,
) -> Result<()> {
    let recomputed = mint_coin(&MintOptions {
        room_wire_pk: manifest.room_wire_pk.clone(),
        pin_dir: pin_dir.map(|p| p.to_path_buf()),
        room_id: None,
        decrypt_pk: decrypt_pk.map(|p| p.to_path_buf()),
        decrypt_sk: decrypt_sk.map(|p| p.to_path_buf()),
        quorum_replicas: manifest.quorum_replicas,
        ssc_out: None,
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
    println!(
        "Validated ITS-COIN/1 room_wire_pk={}… frames={} root={}… (SSS link_0)",
        &manifest.room_wire_pk[..8.min(manifest.room_wire_pk.len())],
        manifest.frame_count,
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
    fn sss_total_bytes_aligns() {
        assert_eq!(sss_total_bytes(1), 16);
        assert_eq!(sss_total_bytes(16), 16);
        assert_eq!(sss_total_bytes(17), 32);
    }
}
