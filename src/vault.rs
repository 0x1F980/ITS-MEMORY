use std::path::{Path, PathBuf};

use crate::error::{MemError, Result};

pub fn memory_home() -> PathBuf {
    std::env::var("ITS_MEMORY_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs_fallback()
                .join(".its")
                .join("memory")
        })
}

fn dirs_fallback() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

pub fn ensure_layout() -> Result<()> {
    std::fs::create_dir_all(pins_root())?;
    std::fs::create_dir_all(channel_coin_registry())?;
    std::fs::create_dir_all(gdir_registry())?;
    std::fs::create_dir_all(gdir_receipts_dir())?;
    std::fs::create_dir_all(mirrors_root())?;
    std::fs::create_dir_all(witnesses_dir())?;
    std::fs::create_dir_all(blind_shards_dir())?;
    std::fs::create_dir_all(pool_ingest_staging())?;
    migrate_legacy_registry()?;
    Ok(())
}

pub fn pins_root() -> PathBuf {
    memory_home().join("pins")
}

pub fn pin_room_dir(room_wire_pk: &str) -> PathBuf {
    pins_root().join(normalize_pk(room_wire_pk))
}

pub fn coin_registry() -> PathBuf {
    channel_coin_registry()
}

pub fn channel_coin_registry() -> PathBuf {
    memory_home().join("coin").join("channel").join("registry")
}

/// Legacy path before channel/gdir split (`coin/registry/`).
pub fn legacy_coin_registry() -> PathBuf {
    memory_home().join("coin").join("registry")
}

pub fn gdir_registry() -> PathBuf {
    memory_home().join("coin").join("gdir").join("registry")
}

pub fn gdir_receipts_dir() -> PathBuf {
    memory_home().join("coin").join("gdir").join("receipts")
}

pub fn mirrors_root() -> PathBuf {
    memory_home().join("mirrors")
}

pub fn witnesses_dir() -> PathBuf {
    memory_home().join("witnesses")
}

pub fn blind_shards_dir() -> PathBuf {
    memory_home().join("blind_shards")
}

pub fn pool_ingest_staging() -> PathBuf {
    memory_home().join("pool_ingest")
}

pub fn pool_duty_witness_path() -> PathBuf {
    pool_ingest_staging().join("duty_pool_witness.txt")
}

pub fn touch_pool_duty_witness() -> Result<()> {
    std::fs::create_dir_all(pool_ingest_staging())?;
    std::fs::write(
        pool_duty_witness_path(),
        format!("witness_at: {}\n", now_unix_for_vault()),
    )?;
    Ok(())
}

pub fn has_pool_duty_witness() -> bool {
    pool_duty_witness_path().is_file()
}

fn now_unix_for_vault() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn mirror_room_dir(room_wire_pk: &str) -> PathBuf {
    mirrors_root().join(normalize_pk(room_wire_pk))
}

pub fn normalize_pk(pk: &str) -> String {
    pk.trim().to_ascii_lowercase()
}

pub fn ratchet_seed_path(explicit: Option<&Path>) -> PathBuf {
    if let Some(p) = explicit {
        return p.to_path_buf();
    }
    if let Ok(p) = std::env::var("ITS_MEMORY_RATCHET_SEED") {
        return PathBuf::from(p);
    }
    memory_home().join("ratchet.seed")
}

pub fn read_ratchet_seed(path: &Path) -> Result<[u8; 32]> {
    let data = std::fs::read(path)?;
    if data.len() != 32 {
        return Err(MemError::Store(format!(
            "ratchet seed must be 32 bytes (got {})",
            data.len()
        )));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&data);
    Ok(out)
}

#[cfg(test)]
mod migration_tests {
    use super::*;

    #[test]
    fn migrate_legacy_registry_copies_to_channel() {
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("ITS_MEMORY_HOME", tmp.path());
        let legacy = legacy_coin_registry();
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("deadbeef.channel.coin.toml"), "stub\n").unwrap();
        ensure_layout().unwrap();
        let canonical = channel_coin_registry();
        assert!(canonical.join("deadbeef.channel.coin.toml").exists());
        std::env::remove_var("ITS_MEMORY_HOME");
    }
}

fn migrate_legacy_registry() -> Result<()> {
    let legacy = legacy_coin_registry();
    if !legacy.is_dir() {
        return Ok(());
    }
    let canonical = channel_coin_registry();
    std::fs::create_dir_all(&canonical)?;
    for entry in std::fs::read_dir(&legacy)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let name = path.file_name().ok_or_else(|| {
            MemError::Store("legacy registry entry missing filename".into())
        })?;
        let dest = canonical.join(name);
        if dest.exists() {
            continue;
        }
        #[cfg(unix)]
        {
            if std::os::unix::fs::symlink(&path, &dest).is_ok() {
                continue;
            }
        }
        std::fs::copy(&path, &dest)?;
    }
    Ok(())
}
