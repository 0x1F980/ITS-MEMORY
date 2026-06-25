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
    std::fs::create_dir_all(coin_registry())?;
    Ok(())
}

pub fn pins_root() -> PathBuf {
    memory_home().join("pins")
}

pub fn pin_room_dir(room_wire_pk: &str) -> PathBuf {
    pins_root().join(normalize_pk(room_wire_pk))
}

pub fn coin_registry() -> PathBuf {
    memory_home().join("coin").join("registry")
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
