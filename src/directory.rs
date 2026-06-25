use std::path::{Path, PathBuf};

use crate::error::{MemError, Result};
use crate::pipe;
use crate::vault::{coin_registry, ensure_layout, normalize_pk};
use crate::wire::CoinManifest as WireCoin;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    FrameCount,
    LastEpoch,
}

impl SortKey {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "frame_count" | "frames" | "activity" => Ok(Self::FrameCount),
            "last_epoch" | "last_pool_epoch" | "epoch" => Ok(Self::LastEpoch),
            _ => Err(MemError::Usage(format!(
                "unknown sort key: {s} (frame_count|last_epoch)"
            ))),
        }
    }
}

pub fn publish_manifest(manifest_path: &Path, registry: Option<&Path>) -> Result<PathBuf> {
    ensure_layout()?;
    let manifest = WireCoin::read_file(manifest_path)?;
    let reg = registry
        .map(|p| p.to_path_buf())
        .unwrap_or_else(coin_registry);
    std::fs::create_dir_all(&reg)?;
    let dest = reg.join(format!("{}.coin.toml", normalize_pk(&manifest.room_wire_pk)));
    manifest.write_file(&dest)?;
    println!("Published coin -> {}", dest.display());
    Ok(dest)
}

pub fn publish_to_pool(manifest_path: &Path, routing_config: &Path, ratchet_seed: &Path) -> Result<()> {
    let manifest = WireCoin::read_file(manifest_path)?;
    let wire_path = std::env::temp_dir().join(format!("its_coin_pub_{}", rand_suffix()));
    std::fs::write(&wire_path, manifest.serialize_text())?;
    pipe::its_routing_send(routing_config, &wire_path, ratchet_seed)?;
    let _ = std::fs::remove_file(&wire_path);
    println!("Pool publish sent for room_wire_pk={}…", &manifest.room_wire_pk[..8.min(manifest.room_wire_pk.len())]);
    Ok(())
}

pub fn browse(registry: Option<&Path>, sort: SortKey) -> Result<Vec<WireCoin>> {
    let reg = registry
        .map(|p| p.to_path_buf())
        .unwrap_or_else(coin_registry);
    if !reg.is_dir() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&reg)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.ends_with(".coin.toml") || name.ends_with(".toml") {
            if let Ok(coin) = WireCoin::read_file(&path) {
                entries.push(coin);
            }
        }
    }
    match sort {
        SortKey::FrameCount => entries.sort_by(|a, b| b.frame_count.cmp(&a.frame_count)),
        SortKey::LastEpoch => entries.sort_by(|a, b| b.last_pool_epoch.cmp(&a.last_pool_epoch)),
    }
    for coin in &entries {
        println!(
            "room_wire_pk={} room_id_fp={} frames={} last_seq={} last_epoch={} root={}…",
            coin.room_wire_pk,
            coin.room_id_fp,
            coin.frame_count,
            coin.last_seq,
            coin.last_pool_epoch,
            &coin.chain_root[..8.min(coin.chain_root.len())]
        );
    }
    Ok(entries)
}

pub fn search(registry: Option<&Path>, min_frames: u64, sort: SortKey) -> Result<Vec<WireCoin>> {
    let all = browse(registry, sort)?;
    Ok(all
        .into_iter()
        .filter(|c| c.frame_count >= min_frames)
        .collect())
}

fn rand_suffix() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}
