use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::gdir::record_contrib;
use crate::host::contrib_fp;
use crate::pipe;
use crate::wire::GdirOp;
use crate::vault::{blind_shards_dir, ensure_layout};
use crate::wire::{BlindShard, MemoryPin, PIN_MAGIC};

const DEFAULT_NETWORK_SALT: &str = "its-memory-network-v1";

pub fn shard_assignment_id(epoch: u64, contrib: Option<&str>) -> Result<String> {
    let fp = contrib
        .map(|s| s.to_string())
        .unwrap_or_else(|| contrib_fp().unwrap_or_default());
    let salt = std::env::var("ITS_NETWORK_SALT").unwrap_or_else(|_| DEFAULT_NETWORK_SALT.into());
    let mut h: u64 = 0xcbf29ce484222325;
    for b in format!("{salt}|{epoch}|{fp}").bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    let mut h2: u64 = h.wrapping_mul(0x100000001b3) ^ epoch;
    for b in fp.as_bytes() {
        h2 ^= *b as u64;
        h2 = h2.wrapping_mul(0x100000001b3);
    }
    Ok(format!("{:016x}{:016x}", h, h2))
}

pub fn store_blind_shard(shard: &BlindShard) -> Result<PathBuf> {
    ensure_layout()?;
    let dir = blind_shards_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("shard_{}_{}.shard", shard.shard_id, shard.epoch));
    shard.write_file(&path)?;
    Ok(path)
}

pub fn blind_pull_once(
    config: &Path,
    ratchet_seed: &Path,
    epoch: u64,
    timeout_secs: u64,
) -> Result<Option<PathBuf>> {
    let tmp = std::env::temp_dir().join(format!("its_blind_wire_{epoch}"));
    let recv = pipe::its_routing_receive_once(config, ratchet_seed, &tmp, timeout_secs, epoch)?;
    if recv.wire_bytes.is_empty() {
        let _ = std::fs::remove_file(&tmp);
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&recv.wire_bytes);
    let shard_id = shard_assignment_id(epoch, None)?;
    let shard = if text.trim_start().starts_with(PIN_MAGIC) {
        let pin = MemoryPin::parse_text(&text)?;
        BlindShard {
            shard_id: shard_id.clone(),
            epoch: pin.pool_epoch,
            wire_b64: pin.wire_b64,
            wire_hash: pin.wire_hash,
        }
    } else if text.trim_start().starts_with("ITS-MEMORY-SHARD/1") {
        let mut s = BlindShard::parse_text(&text)?;
        s.shard_id = shard_id;
        s
    } else {
        BlindShard {
            shard_id,
            epoch,
            wire_b64: base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &recv.wire_bytes,
            ),
            wire_hash: crate::wire::wire_identity(&recv.wire_bytes),
        }
    };
    let path = store_blind_shard(&shard)?;
    let bytes = shard.wire_b64.len() as u64;
    let _ = record_contrib(GdirOp::Blind, bytes.max(1))?;
    let _ = std::fs::remove_file(&tmp);
    Ok(Some(path))
}

pub fn run_blind_pull(
    config: &Path,
    ratchet_seed: &Path,
    max_messages: usize,
    timeout_secs: u64,
) -> Result<usize> {
    let mut stored = 0usize;
    let mut from_epoch = 0u64;
    while stored < max_messages {
        match blind_pull_once(config, ratchet_seed, from_epoch, timeout_secs)? {
            Some(_) => {
                stored += 1;
                from_epoch = from_epoch.saturating_add(1);
            }
            None => break,
        }
    }
    println!("Blind-pulled {stored} shard(s) -> {}", blind_shards_dir().display());
    Ok(stored)
}

pub fn blind_dir_has_room_wire_pk() -> Result<bool> {
    let dir = blind_shards_dir();
    if !dir.is_dir() {
        return Ok(false);
    }
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        if text.contains("room_wire_pk:") {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shard_id_deterministic() {
        let a = shard_assignment_id(7, Some("abcd1234")).unwrap();
        let b = shard_assignment_id(7, Some("abcd1234")).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);
    }
}
