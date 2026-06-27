use std::path::{Path, PathBuf};

use crate::error::{MemError, Result};
use crate::host::{self, now_unix};
use crate::store::list_pins;
use crate::vault::{ensure_layout, mirror_room_dir, normalize_pk, pin_room_dir};
use crate::wire::MemoryPin;

pub const PUBLISHED_SUFFIX: &str = ".published";

pub fn publish_pins(room_wire_pk: &str) -> Result<usize> {
    ensure_layout()?;
    let pk = normalize_pk(room_wire_pk);
    let pins = list_pins(&pk)?;
    if pins.is_empty() {
        return Err(MemError::Store("no pins to publish".into()));
    }
    let mirror_dir = mirror_room_dir(&pk);
    std::fs::create_dir_all(&mirror_dir)?;
    let mut published = 0usize;
    for pin in &pins {
        let src = find_pin_path(&pk, &pin.wire_hash)?;
        let wire = pin.wire_bytes()?;
        let dest_name = format!(
            "epoch_{:08}_{}.pin",
            pin.pool_epoch,
            &pin.wire_hash[..16.min(pin.wire_hash.len())]
        );
        let dest = mirror_dir.join(&dest_name);
        pin.write_file(&dest)?;
        write_published_marker(&dest, wire.len() as u64)?;
        if let Some(local) = src {
            write_published_marker(&local, wire.len() as u64)?;
        }
        host::touch_room_published(&pk, wire.len() as u64)?;
        published += 1;
    }
    println!(
        "Published {published} pin(s) -> {}",
        mirror_dir.display()
    );
    Ok(published)
}

pub fn is_published_pin_path(path: &Path) -> bool {
    published_marker_path(path).is_file()
}

pub fn list_mirror_pins(room_wire_pk: &str) -> Result<Vec<MemoryPin>> {
    list_pins_in_dir(&mirror_room_dir(room_wire_pk))
}

pub fn list_pins_in_dir(dir: &Path) -> Result<Vec<MemoryPin>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut pins = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("pin") {
            continue;
        }
        pins.push(MemoryPin::read_file(&path)?);
    }
    pins.sort_by_key(|p| (p.pool_epoch, p.wire_hash.clone()));
    Ok(pins)
}

pub fn list_published_pins(room_wire_pk: &str) -> Result<Vec<MemoryPin>> {
    let pk = normalize_pk(room_wire_pk);
    let mut pins = list_pins(&pk)?;
    pins.retain(|pin| {
        find_pin_path(&pk, &pin.wire_hash)
            .ok()
            .flatten()
            .map(|p| is_published_pin_path(&p))
            .unwrap_or(false)
    });
    if pins.is_empty() {
        pins = list_mirror_pins(&pk)?;
    }
    Ok(pins)
}

pub fn fetch_from_mirror(room_wire_pk: &str, out_dir: &Path, from_epoch: u64) -> Result<usize> {
    fetch_from_mirror_dir(
        &mirror_room_dir(room_wire_pk),
        out_dir,
        from_epoch,
        None,
        None,
        None,
    )
}

pub fn fetch_from_mirror_dir(
    mirror_dir: &Path,
    out_dir: &Path,
    from_epoch: u64,
    to_epoch: Option<u64>,
    from_seq_hint: Option<u64>,
    limit: Option<usize>,
) -> Result<usize> {
    use crate::fetch::{export_filtered_pins, FetchOptions};
    let pins = list_pins_in_dir(mirror_dir)?;
    let opts = FetchOptions {
        room_wire_pk: String::new(),
        out_dir: out_dir.to_path_buf(),
        from_epoch,
        to_epoch,
        from_seq_hint,
        limit,
        filter_pk: None,
        filter_sk: None,
        mirror_dir: None,
    };
    export_filtered_pins(pins, &opts)
}

fn published_marker_path(pin_path: &Path) -> PathBuf {
    PathBuf::from(format!(
        "{}{}",
        pin_path.display(),
        PUBLISHED_SUFFIX
    ))
}

fn write_published_marker(pin_path: &Path, byte_span: u64) -> Result<()> {
    let marker = published_marker_path(pin_path);
    let text = format!(
        "published_at: {}\nbyte_span: {byte_span}\n",
        now_unix()
    );
    std::fs::write(marker, text)?;
    Ok(())
}

fn find_pin_path(room_wire_pk: &str, wire_hash: &str) -> Result<Option<PathBuf>> {
    let dir = pin_room_dir(room_wire_pk);
    if !dir.is_dir() {
        return Ok(None);
    }
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
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
