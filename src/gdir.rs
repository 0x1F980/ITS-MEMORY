use std::path::{Path, PathBuf};

use crate::error::{MemError, Result};
use crate::host::{self, contrib_fp, load_ledger, now_unix, touch_gdir_contrib};
use crate::pipe::{parse_ssc_link0_hex, sss_chain_generate, sss_chain_validate};
use crate::vault::gdir_receipts_dir;
use crate::wire::{GdirCoinManifest, GdirOp, GdirReceipt};

pub const GDIR_LINK_BYTE_LEN: usize = 16;

pub fn record_contrib(op: GdirOp, byte_span: u64) -> Result<PathBuf> {
    touch_gdir_contrib()?;
    let fp = contrib_fp()?;
    let receipt = GdirReceipt {
        contrib_fp: fp,
        epoch: now_unix(),
        op: op.as_str().to_string(),
        byte_span,
    };
    std::fs::create_dir_all(gdir_receipts_dir())?;
    let path = gdir_receipts_dir().join(format!(
        "receipt_{}_{}.txt",
        receipt.epoch,
        rand_suffix()
    ));
    receipt.write_file(&path)?;
    println!(
        "Recorded ITS-GDIR-RECEIPT/1 op={} byte_span={} -> {}",
        receipt.op,
        receipt.byte_span,
        path.display()
    );
    Ok(path)
}

pub fn list_receipts() -> Result<Vec<GdirReceipt>> {
    let dir = gdir_receipts_dir();
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let fp = contrib_fp()?;
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        if let Ok(receipt) = GdirReceipt::read_file(&path) {
            if receipt.contrib_fp == fp {
                out.push(receipt);
            }
        }
    }
    out.sort_by_key(|r| (r.epoch, r.byte_span));
    Ok(out)
}

pub fn has_gdir_receipt() -> Result<bool> {
    Ok(!list_receipts()?.is_empty())
}

pub fn has_blind_or_infra_receipt() -> Result<bool> {
    Ok(list_receipts()?
        .iter()
        .any(|r| matches!(r.op.as_str(), "blind" | "sync" | "route")))
}

pub struct MintGdirOptions {
    pub require_blind_or_infra: bool,
}

pub fn mint_gdir_coin() -> Result<GdirCoinManifest> {
    mint_gdir_coin_options(&MintGdirOptions {
        require_blind_or_infra: false,
    })
}

pub fn mint_gdir_coin_options(opts: &MintGdirOptions) -> Result<GdirCoinManifest> {
    let receipts = list_receipts()?;
    if receipts.is_empty() {
        return Err(MemError::Coin("no gdir receipts to mint".into()));
    }
    if opts.require_blind_or_infra && !receipts.iter().any(|r| matches!(r.op.as_str(), "blind" | "sync" | "route")) {
        return Err(MemError::Coin(
            "GDIR mint requires blind/sync/route receipt (not channel-only mirror ops)".into(),
        ));
    }
    let fp = contrib_fp()?;
    let payload = build_gdir_payload(&receipts);
    let (chain_root, ssc_path) = generate_gdir_chain_root(&fp, &payload)?;
    if !sss_chain_validate(&gdir_root_path(&fp), &ssc_path)? {
        return Err(MemError::Coin("sss_chain validate failed for gdir coin".into()));
    }
    let ledger = load_ledger()?;
    let now = now_unix();
    let contrib_seconds = ledger
        .gdir_first_seen
        .map(|start| now.saturating_sub(start))
        .unwrap_or(0);
    let contrib_bytes = receipts.iter().map(|r| r.byte_span).sum();
    Ok(GdirCoinManifest {
        contrib_fp: fp,
        chain_root,
        contrib_ops: receipts.len() as u64,
        contrib_bytes,
        contrib_seconds,
    })
}

pub fn validate_gdir_coin(manifest: &GdirCoinManifest) -> Result<()> {
    let recomputed = mint_gdir_coin()?;
    if recomputed.chain_root != manifest.chain_root {
        return Err(MemError::Coin("gdir chain_root mismatch".into()));
    }
    if recomputed.contrib_ops != manifest.contrib_ops {
        return Err(MemError::Coin("gdir contrib_ops mismatch".into()));
    }
    println!(
        "Validated ITS-GDIR-COIN/1 contrib_fp={}… ops={} bytes={} seconds={}",
        &manifest.contrib_fp[..8.min(manifest.contrib_fp.len())],
        manifest.contrib_ops,
        manifest.contrib_bytes,
        manifest.contrib_seconds
    );
    Ok(())
}

fn build_gdir_payload(receipts: &[GdirReceipt]) -> Vec<u8> {
    let mut out = Vec::new();
    for receipt in receipts {
        out.extend_from_slice(&receipt.payload_bytes());
        out.push(0);
    }
    if out.is_empty() {
        out.extend_from_slice(b"ITS-GDIR-empty");
    }
    out
}

fn gdir_root_material(contrib_fp: &str) -> Vec<u8> {
    let mut out = b"ITS-GDIR-sss-root-v1".to_vec();
    out.push(0);
    out.extend_from_slice(contrib_fp.as_bytes());
    out
}

fn gdir_root_path(contrib_fp: &str) -> PathBuf {
    std::env::temp_dir().join(format!("its_gdir_root_{contrib_fp}"))
}

fn sss_total_bytes(payload_len: usize) -> usize {
    let raw_len = payload_len.max(GDIR_LINK_BYTE_LEN);
    ((raw_len + GDIR_LINK_BYTE_LEN - 1) / GDIR_LINK_BYTE_LEN) * GDIR_LINK_BYTE_LEN
}

fn generate_gdir_chain_root(contrib_fp: &str, payload: &[u8]) -> Result<(String, PathBuf)> {
    let root_path = gdir_root_path(contrib_fp);
    std::fs::write(&root_path, gdir_root_material(contrib_fp))?;
    let ssc_path = std::env::temp_dir().join(format!("its_gdir_{contrib_fp}.ssc"));
    let total_bytes = sss_total_bytes(payload.len());
    sss_chain_generate(
        &root_path,
        total_bytes,
        GDIR_LINK_BYTE_LEN,
        &ssc_path,
    )?;
    let ssc_text = std::fs::read_to_string(&ssc_path)?;
    let link0 = parse_ssc_link0_hex(&ssc_text)?;
    Ok((link0, ssc_path))
}

fn rand_suffix() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gdir_receipt_has_no_room_field() {
        let text = GdirReceipt {
            contrib_fp: "abcd1234ef567890".into(),
            epoch: 1,
            op: "sync".into(),
            byte_span: 99,
        }
        .serialize_text();
        assert!(!text.contains("room_wire_pk"));
        assert!(text.contains("ITS-GDIR-RECEIPT/1"));
    }
}
