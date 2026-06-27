use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{MemError, Result};
use crate::vault::{ensure_layout, memory_home, normalize_pk};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct HostLedger {
    pub contrib_fp: String,
    #[serde(default)]
    pub rooms: Vec<HostRoomEntry>,
    #[serde(default)]
    pub gdir_first_seen: Option<u64>,
    #[serde(default)]
    pub gdir_last_seen: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HostRoomEntry {
    pub room_wire_pk: String,
    pub first_seen: u64,
    pub last_seen: u64,
    pub pin_count: u64,
    pub memory_bytes: u64,
    pub first_published: Option<u64>,
}

pub fn host_secret_path() -> PathBuf {
    memory_home().join("host.secret")
}

pub fn host_ledger_path() -> PathBuf {
    memory_home().join("host").join("ledger.toml")
}

pub fn ensure_host_secret() -> Result<[u8; 32]> {
    ensure_layout()?;
    let path = host_secret_path();
    if path.is_file() {
        return read_secret(&path);
    }
    let mut secret = [0u8; 32];
    getrandom_bytes(&mut secret)?;
    std::fs::write(&path, &secret)?;
    Ok(secret)
}

pub fn contrib_fp() -> Result<String> {
    let secret = ensure_host_secret()?;
    Ok(hex::encode(&secret)[..16.min(64)].to_string())
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn load_ledger() -> Result<HostLedger> {
    ensure_layout()?;
    let path = host_ledger_path();
    if !path.is_file() {
        return Ok(HostLedger {
            contrib_fp: contrib_fp()?,
            ..Default::default()
        });
    }
    let text = std::fs::read_to_string(&path)?;
    let mut ledger: HostLedger = toml::from_str(&text)
        .map_err(|e| MemError::Store(format!("host ledger: {e}")))?;
    if ledger.contrib_fp.is_empty() {
        ledger.contrib_fp = contrib_fp()?;
    }
    Ok(ledger)
}

pub fn save_ledger(ledger: &HostLedger) -> Result<()> {
    let path = host_ledger_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, toml::to_string_pretty(ledger).unwrap_or_default())?;
    Ok(())
}

pub fn touch_room_pin(room_wire_pk: &str, pin_bytes: u64) -> Result<()> {
    let mut ledger = load_ledger()?;
    let now = now_unix();
    let pk = normalize_pk(room_wire_pk);
    if let Some(entry) = ledger.rooms.iter_mut().find(|r| r.room_wire_pk == pk) {
        entry.last_seen = now;
        entry.pin_count += 1;
        entry.memory_bytes += pin_bytes;
    } else {
        ledger.rooms.push(HostRoomEntry {
            room_wire_pk: pk,
            first_seen: now,
            last_seen: now,
            pin_count: 1,
            memory_bytes: pin_bytes,
            first_published: None,
        });
    }
    save_ledger(&ledger)
}

pub fn touch_room_published(room_wire_pk: &str, pin_bytes: u64) -> Result<()> {
    let mut ledger = load_ledger()?;
    let now = now_unix();
    let pk = normalize_pk(room_wire_pk);
    if let Some(entry) = ledger.rooms.iter_mut().find(|r| r.room_wire_pk == pk) {
        entry.last_seen = now;
        entry.memory_bytes = entry.memory_bytes.saturating_add(pin_bytes);
        if entry.first_published.is_none() {
            entry.first_published = Some(now);
        }
    } else {
        ledger.rooms.push(HostRoomEntry {
            room_wire_pk: pk,
            first_seen: now,
            last_seen: now,
            pin_count: 0,
            memory_bytes: pin_bytes,
            first_published: Some(now),
        });
    }
    save_ledger(&ledger)
}

pub fn touch_gdir_contrib() -> Result<()> {
    let mut ledger = load_ledger()?;
    let now = now_unix();
    if ledger.gdir_first_seen.is_none() {
        ledger.gdir_first_seen = Some(now);
    }
    ledger.gdir_last_seen = Some(now);
    save_ledger(&ledger)
}

pub fn hosted_seconds(room_wire_pk: &str) -> Result<u64> {
    let ledger = load_ledger()?;
    let pk = normalize_pk(room_wire_pk);
    let now = now_unix();
    Ok(ledger
        .rooms
        .iter()
        .find(|r| r.room_wire_pk == pk)
        .and_then(|r| r.first_published)
        .map(|start| now.saturating_sub(start))
        .unwrap_or(0))
}

pub fn room_host_status(room_wire_pk: &str) -> Result<Option<HostRoomEntry>> {
    let ledger = load_ledger()?;
    let pk = normalize_pk(room_wire_pk);
    Ok(ledger.rooms.iter().find(|r| r.room_wire_pk == pk).cloned())
}

fn read_secret(path: &Path) -> Result<[u8; 32]> {
    let data = std::fs::read(path)?;
    if data.len() != 32 {
        return Err(MemError::Store("host.secret must be 32 bytes".into()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&data);
    Ok(out)
}

fn getrandom_bytes(out: &mut [u8; 32]) -> Result<()> {
    use std::io::Read;
    let mut f = std::fs::File::open("/dev/urandom")
        .map_err(|e| MemError::Store(format!("urandom: {e}")))?;
    f.read_exact(out)
        .map_err(|e| MemError::Store(format!("urandom read: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrib_fp_stable() {
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("ITS_MEMORY_HOME", tmp.path());
        let fp1 = contrib_fp().unwrap();
        let fp2 = contrib_fp().unwrap();
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.len(), 16);
        std::env::remove_var("ITS_MEMORY_HOME");
    }
}
