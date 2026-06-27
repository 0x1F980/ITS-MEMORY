use std::path::{Path, PathBuf};

use base64::Engine;

use crate::error::Result;
use crate::host;
use crate::vault::{ensure_layout, normalize_pk, pin_room_dir};
use crate::wire::{wire_filename_tag, wire_identity, MemoryPin};

pub fn store_pin(pin: &MemoryPin) -> Result<PathBuf> {
    ensure_layout()?;
    let dir = pin_room_dir(&pin.room_wire_pk);
    std::fs::create_dir_all(&dir)?;
    if let Some(existing) = find_pin_by_identity(&dir, &pin.wire_hash)? {
        return Ok(existing);
    }
    let wire = pin.wire_bytes()?;
    let tag = wire_filename_tag(&wire);
    let path = dir.join(format!("{tag}.pin"));
    pin.write_file(&path)?;
    let wire = pin.wire_bytes()?;
    let _ = host::touch_room_pin(&pin.room_wire_pk, wire.len() as u64);
    Ok(path)
}

pub fn list_pins(room_wire_pk: &str) -> Result<Vec<MemoryPin>> {
    let dir = pin_room_dir(room_wire_pk);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut pins = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".pin") {
            continue;
        }
        pins.push(MemoryPin::read_file(&entry.path())?);
    }
    pins.sort_by_key(|p| (p.pool_epoch, p.wire_hash.clone()));
    Ok(pins)
}

pub fn pin_from_wire(room_wire_pk: &str, pool_epoch: u64, wire_bytes: &[u8]) -> Result<MemoryPin> {
    let wire_b64 = base64::engine::general_purpose::STANDARD.encode(wire_bytes);
    Ok(MemoryPin {
        room_wire_pk: normalize_pk(room_wire_pk),
        pool_epoch,
        wire_b64,
        seq_hint: None,
        wire_hash: wire_identity(wire_bytes),
    })
}

pub fn pin_count(room_wire_pk: &str) -> Result<usize> {
    Ok(list_pins(room_wire_pk)?.len())
}

pub fn update_seq_hint(path: &Path, seq: u64) -> Result<()> {
    let mut pin = MemoryPin::read_file(path)?;
    pin.seq_hint = Some(seq);
    pin.write_file(path)
}

pub fn dedup_exists(room_wire_pk: &str, wire_bytes: &[u8]) -> Result<bool> {
    let dir = pin_room_dir(room_wire_pk);
    if !dir.is_dir() {
        return Ok(false);
    }
    let want = wire_identity(wire_bytes);
    Ok(find_pin_by_identity(&dir, &want)?.is_some())
}

fn find_pin_by_identity(dir: &Path, wire_hash: &str) -> Result<Option<PathBuf>> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("pin") {
            continue;
        }
        let pin = MemoryPin::read_file(&path)?;
        if pin.wire_hash == wire_hash {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_dedup() {
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("ITS_MEMORY_HOME", tmp.path());
        let pin = pin_from_wire("aa".repeat(64).as_str(), 1, b"wire").unwrap();
        let p1 = store_pin(&pin).unwrap();
        let p2 = store_pin(&pin).unwrap();
        assert_eq!(p1, p2);
        assert_eq!(pin_count(&pin.room_wire_pk).unwrap(), 1);
        std::env::remove_var("ITS_MEMORY_HOME");
    }
}
