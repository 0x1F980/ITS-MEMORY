use std::path::{Path, PathBuf};

use crate::directory::{merge_channel_manifest, merge_gdir_manifest};
use crate::error::Result;
use crate::pipe;
use crate::vault::{channel_coin_registry, gdir_registry, pool_ingest_staging};
use crate::wire::{ChannelCoinManifest, GdirCoinManifest, CHANNEL_COIN_V1_MAGIC, CHANNEL_COIN_V2_MAGIC, CHANNEL_COIN_V3_MAGIC, GDIR_COIN_MAGIC};

pub fn try_parse_coin_wire(bytes: &[u8]) -> Result<Option<IngestedCoin>> {
    let text = String::from_utf8_lossy(bytes);
    let trimmed = text.trim();
    if trimmed.starts_with(CHANNEL_COIN_V1_MAGIC)
        || trimmed.starts_with(CHANNEL_COIN_V2_MAGIC)
        || trimmed.starts_with(CHANNEL_COIN_V3_MAGIC)
    {
        return Ok(Some(IngestedCoin::Channel(
            ChannelCoinManifest::parse_text(trimmed)?,
        )));
    }
    if trimmed.starts_with(GDIR_COIN_MAGIC) {
        return Ok(Some(IngestedCoin::Gdir(
            GdirCoinManifest::parse_text(trimmed)?,
        )));
    }
    Ok(None)
}

pub enum IngestedCoin {
    Channel(ChannelCoinManifest),
    Gdir(GdirCoinManifest),
}

pub fn ingest_channel_pool(
    config: &Path,
    ratchet_seed: &Path,
    registry: Option<&Path>,
    max_messages: usize,
    timeout_secs: u64,
) -> Result<usize> {
    let staging = pool_ingest_staging();
    std::fs::create_dir_all(&staging)?;
    let tmp = staging.join("recv.wire");
    let mut ingested = 0usize;
    let mut from_epoch = 0u64;
    let mut attempts = 0usize;
    let max_attempts = max_messages.saturating_mul(4).max(4);
    let reg = registry
        .map(|p| p.to_path_buf())
        .unwrap_or_else(channel_coin_registry);

    while ingested < max_messages && attempts < max_attempts {
        attempts += 1;
        let recv = pipe::its_routing_receive_once_opts(
            config,
            ratchet_seed,
            &tmp,
            timeout_secs,
            from_epoch,
            false,
        )?;
        if recv.wire_bytes.is_empty() {
            break;
        }
        if let Some(IngestedCoin::Channel(manifest)) = try_parse_coin_wire(&recv.wire_bytes)? {
            std::fs::create_dir_all(&reg)?;
            let dest = reg.join(format!(
                "{}.channel.coin.toml",
                crate::vault::normalize_pk(&manifest.room_wire_pk)
            ));
            if dest.is_file() {
                let existing = ChannelCoinManifest::read_file(&dest)?;
                let merged = merge_channel_manifest(&existing, &manifest);
                merged.write_file(&dest)?;
            } else {
                manifest.write_file(&dest)?;
            }
            ingested += 1;
            let _ = crate::vault::touch_pool_duty_witness();
            println!(
                "Ingested channel coin room_wire_pk={}…",
                &manifest.room_wire_pk[..8.min(manifest.room_wire_pk.len())]
            );
        }
        if let Some(next) = pipe::parse_next_pool_epoch(&recv.stdout) {
            from_epoch = next;
        } else {
            from_epoch = from_epoch.saturating_add(1);
        }
    }
    let _ = std::fs::remove_file(&tmp);
    Ok(ingested)
}

pub fn ingest_gdir_pool(
    config: &Path,
    ratchet_seed: &Path,
    registry: Option<&Path>,
    max_messages: usize,
    timeout_secs: u64,
) -> Result<usize> {
    let staging = pool_ingest_staging();
    std::fs::create_dir_all(&staging)?;
    let tmp = staging.join("recv_gdir.wire");
    let mut ingested = 0usize;
    let mut from_epoch = 0u64;
    let mut attempts = 0usize;
    let max_attempts = max_messages.saturating_mul(4).max(4);
    let reg = registry
        .map(|p| p.to_path_buf())
        .unwrap_or_else(gdir_registry);

    while ingested < max_messages && attempts < max_attempts {
        attempts += 1;
        let recv = pipe::its_routing_receive_once_opts(
            config,
            ratchet_seed,
            &tmp,
            timeout_secs,
            from_epoch,
            false,
        )?;
        if recv.wire_bytes.is_empty() {
            break;
        }
        if let Some(IngestedCoin::Gdir(manifest)) = try_parse_coin_wire(&recv.wire_bytes)? {
            std::fs::create_dir_all(&reg)?;
            let dest = reg.join(format!("{}.gdir.coin.toml", manifest.contrib_fp));
            if dest.is_file() {
                let existing = GdirCoinManifest::read_file(&dest)?;
                let merged = merge_gdir_manifest(&existing, &manifest);
                merged.write_file(&dest)?;
            } else {
                manifest.write_file(&dest)?;
            }
            ingested += 1;
            let _ = crate::vault::touch_pool_duty_witness();
            println!(
                "Ingested gdir coin contrib_fp={}…",
                &manifest.contrib_fp[..8.min(manifest.contrib_fp.len())]
            );
        }
        if let Some(next) = pipe::parse_next_pool_epoch(&recv.stdout) {
            from_epoch = next;
        } else {
            from_epoch = from_epoch.saturating_add(1);
        }
    }
    let _ = std::fs::remove_file(&tmp);
    Ok(ingested)
}
