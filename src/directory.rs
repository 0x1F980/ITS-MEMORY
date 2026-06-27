use std::path::{Path, PathBuf};

use crate::error::{MemError, Result};
use crate::pipe;
use crate::vault::{channel_coin_registry, gdir_registry, legacy_coin_registry, normalize_pk};
use crate::wire::{ChannelCoinManifest, GdirCoinManifest};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelSortKey {
    FrameCount,
    LastEpoch,
    MemoryBytes,
    HostedSeconds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GdirSortKey {
    ContribOps,
    ContribBytes,
    ContribSeconds,
}

impl ChannelSortKey {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "frame_count" | "frames" | "activity" => Ok(Self::FrameCount),
            "last_epoch" | "last_pool_epoch" | "epoch" => Ok(Self::LastEpoch),
            "memory_bytes" | "memory" | "storage" => Ok(Self::MemoryBytes),
            "hosted_seconds" | "hosting" | "duration" => Ok(Self::HostedSeconds),
            _ => Err(MemError::Usage(format!(
                "unknown channel sort key: {s} (frame_count|last_epoch|memory_bytes|hosted_seconds)"
            ))),
        }
    }
}

impl GdirSortKey {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "contrib_ops" | "ops" => Ok(Self::ContribOps),
            "contrib_bytes" | "bytes" => Ok(Self::ContribBytes),
            "contrib_seconds" | "seconds" | "duration" => Ok(Self::ContribSeconds),
            _ => Err(MemError::Usage(format!(
                "unknown gdir sort key: {s} (contrib_ops|contrib_bytes|contrib_seconds)"
            ))),
        }
    }
}

/// Legacy alias.
pub type SortKey = ChannelSortKey;

pub fn publish_channel_manifest(manifest_path: &Path, registry: Option<&Path>) -> Result<PathBuf> {
    crate::vault::ensure_layout()?;
    let manifest = ChannelCoinManifest::read_file(manifest_path)?;
    let reg = registry
        .map(|p| p.to_path_buf())
        .unwrap_or_else(channel_coin_registry);
    std::fs::create_dir_all(&reg)?;
    let dest = reg.join(format!(
        "{}.channel.coin.toml",
        normalize_pk(&manifest.room_wire_pk)
    ));
    manifest.write_file(&dest)?;
    println!("Published channel coin -> {}", dest.display());
    Ok(dest)
}

pub fn publish_gdir_manifest(manifest_path: &Path, registry: Option<&Path>) -> Result<PathBuf> {
    crate::vault::ensure_layout()?;
    let manifest = GdirCoinManifest::read_file(manifest_path)?;
    let reg = registry
        .map(|p| p.to_path_buf())
        .unwrap_or_else(gdir_registry);
    std::fs::create_dir_all(&reg)?;
    let dest = reg.join(format!("{}.gdir.coin.toml", manifest.contrib_fp));
    manifest.write_file(&dest)?;
    println!("Published gdir coin -> {}", dest.display());
    Ok(dest)
}

pub fn publish_manifest(manifest_path: &Path, registry: Option<&Path>) -> Result<PathBuf> {
    publish_channel_manifest(manifest_path, registry)
}

pub fn publish_to_pool(manifest_path: &Path, routing_config: &Path, ratchet_seed: &Path) -> Result<()> {
    let manifest = ChannelCoinManifest::read_file(manifest_path)?;
    let wire_path = std::env::temp_dir().join(format!("its_coin_pub_{}", rand_suffix()));
    std::fs::write(&wire_path, manifest.serialize_text())?;
    pipe::its_routing_send(routing_config, &wire_path, ratchet_seed)?;
    let _ = std::fs::remove_file(&wire_path);
    println!(
        "Pool publish sent for room_wire_pk={}…",
        &manifest.room_wire_pk[..8.min(manifest.room_wire_pk.len())]
    );
    Ok(())
}

pub fn publish_gdir_to_pool(
    manifest_path: &Path,
    routing_config: &Path,
    ratchet_seed: &Path,
) -> Result<()> {
    let manifest = GdirCoinManifest::read_file(manifest_path)?;
    let wire_path = std::env::temp_dir().join(format!("its_gdir_pub_{}", rand_suffix()));
    std::fs::write(&wire_path, manifest.serialize_text())?;
    pipe::its_routing_send(routing_config, &wire_path, ratchet_seed)?;
    let _ = std::fs::remove_file(&wire_path);
    println!(
        "Pool publish sent for gdir contrib_fp={}…",
        &manifest.contrib_fp[..8.min(manifest.contrib_fp.len())]
    );
    Ok(())
}

pub fn browse_channel(registry: Option<&Path>, sort: ChannelSortKey) -> Result<Vec<ChannelCoinManifest>> {
    let mut entries = collect_channel_manifests(registry)?;
    sort_channel(&mut entries, sort);
    for coin in &entries {
        println!(
            "channel room_wire_pk={} room_id_fp={} frames={} memory_bytes={} hosted_seconds={} last_epoch={} root={}…",
            coin.room_wire_pk,
            coin.room_id_fp,
            coin.frame_count,
            coin.memory_bytes,
            coin.hosted_seconds,
            coin.last_pool_epoch,
            &coin.chain_root[..8.min(coin.chain_root.len())]
        );
    }
    Ok(entries)
}

pub fn browse_gdir(registry: Option<&Path>, sort: GdirSortKey) -> Result<Vec<GdirCoinManifest>> {
    let reg = registry
        .map(|p| p.to_path_buf())
        .unwrap_or_else(gdir_registry);
    if !reg.is_dir() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&reg)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        if let Ok(coin) = GdirCoinManifest::read_file(&path) {
            entries.push(coin);
        }
    }
    sort_gdir(&mut entries, sort);
    for coin in &entries {
        println!(
            "gdir contrib_fp={} ops={} bytes={} seconds={} root={}…",
            coin.contrib_fp,
            coin.contrib_ops,
            coin.contrib_bytes,
            coin.contrib_seconds,
            &coin.chain_root[..8.min(coin.chain_root.len())]
        );
    }
    Ok(entries)
}

pub fn browse(registry: Option<&Path>, sort: ChannelSortKey) -> Result<Vec<ChannelCoinManifest>> {
    browse_channel(registry, sort)
}

pub fn search_channel(
    registry: Option<&Path>,
    min_frames: u64,
    sort: ChannelSortKey,
) -> Result<Vec<ChannelCoinManifest>> {
    let all = browse_channel(registry, sort)?;
    Ok(all
        .into_iter()
        .filter(|c| c.frame_count >= min_frames)
        .collect())
}

pub fn search(
    registry: Option<&Path>,
    min_frames: u64,
    sort: ChannelSortKey,
) -> Result<Vec<ChannelCoinManifest>> {
    search_channel(registry, min_frames, sort)
}

fn sort_channel(entries: &mut [ChannelCoinManifest], sort: ChannelSortKey) {
    match sort {
        ChannelSortKey::FrameCount => entries.sort_by(|a, b| b.frame_count.cmp(&a.frame_count)),
        ChannelSortKey::LastEpoch => {
            entries.sort_by(|a, b| b.last_pool_epoch.cmp(&a.last_pool_epoch))
        }
        ChannelSortKey::MemoryBytes => entries.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes)),
        ChannelSortKey::HostedSeconds => {
            entries.sort_by(|a, b| b.hosted_seconds.cmp(&a.hosted_seconds))
        }
    }
}

fn sort_gdir(entries: &mut [GdirCoinManifest], sort: GdirSortKey) {
    match sort {
        GdirSortKey::ContribOps => entries.sort_by(|a, b| b.contrib_ops.cmp(&a.contrib_ops)),
        GdirSortKey::ContribBytes => entries.sort_by(|a, b| b.contrib_bytes.cmp(&a.contrib_bytes)),
        GdirSortKey::ContribSeconds => {
            entries.sort_by(|a, b| b.contrib_seconds.cmp(&a.contrib_seconds))
        }
    }
}

fn collect_channel_manifests(registry: Option<&Path>) -> Result<Vec<ChannelCoinManifest>> {
    let dirs: Vec<PathBuf> = if let Some(reg) = registry {
        vec![reg.to_path_buf()]
    } else {
        let mut dirs = vec![channel_coin_registry()];
        let legacy = legacy_coin_registry();
        if legacy.is_dir() {
            dirs.push(legacy);
        }
        dirs
    };
    let mut entries = Vec::new();
    for reg in dirs {
        entries.extend(read_channel_registry_dir(&reg)?);
    }
    entries.retain(|coin| coin.registry_visible);
    entries.sort_by(|a, b| a.room_wire_pk.cmp(&b.room_wire_pk));
    entries.dedup_by(|a, b| a.room_wire_pk == b.room_wire_pk);
    Ok(entries)
}

fn read_channel_registry_dir(reg: &Path) -> Result<Vec<ChannelCoinManifest>> {
    if !reg.is_dir() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(reg)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        if let Ok(coin) = ChannelCoinManifest::read_file(&path) {
            entries.push(coin);
        }
    }
    Ok(entries)
}

fn rand_suffix() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}
