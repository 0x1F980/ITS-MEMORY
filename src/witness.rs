use std::path::PathBuf;

use crate::coin::pin_set_hash;
use crate::error::{MemError, Result};
use crate::host::{contrib_fp, now_unix};
use crate::vault::{ensure_layout, normalize_pk, witnesses_dir};
use crate::wire::{ChannelCoinManifest, MemoryPin, MemoryWitness};

pub fn write_witness(
    room_wire_pk: &str,
    chain_root: &str,
    pins: &[MemoryPin],
    witness_fp: Option<&str>,
) -> Result<PathBuf> {
    ensure_layout()?;
    let fp = witness_fp
        .map(|s| s.to_string())
        .unwrap_or_else(|| contrib_fp().unwrap_or_default());
    if fp.is_empty() {
        return Err(MemError::Coin("witness_fp unavailable".into()));
    }
    let pk = normalize_pk(room_wire_pk);
    let dir = witnesses_dir().join(&pk);
    std::fs::create_dir_all(&dir)?;
    let witness = MemoryWitness {
        room_wire_pk: pk,
        chain_root: chain_root.to_string(),
        pin_set_hash: pin_set_hash(pins),
        witness_fp: fp.clone(),
        epoch: now_unix(),
    };
    let path = dir.join(format!("{fp}.witness"));
    witness.write_file(&path)?;
    println!(
        "Wrote ITS-MEMORY-WITNESS/1 witness_fp={fp}… pin_set_hash={}…",
        &witness.pin_set_hash[..8.min(witness.pin_set_hash.len())]
    );
    Ok(path)
}

pub fn write_witness_from_manifest(manifest: &ChannelCoinManifest, pins: &[MemoryPin]) -> Result<PathBuf> {
    write_witness(&manifest.room_wire_pk, &manifest.chain_root, pins, None)
}

pub fn list_witnesses(room_wire_pk: &str) -> Result<Vec<MemoryWitness>> {
    let dir = witnesses_dir().join(normalize_pk(room_wire_pk));
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("witness") {
            continue;
        }
        if let Ok(w) = MemoryWitness::read_file(&path) {
            out.push(w);
        }
    }
    Ok(out)
}

pub fn count_quorum_witnesses(
    room_wire_pk: &str,
    chain_root: &str,
    pin_set_hash_hex: &str,
) -> Result<u64> {
    let witnesses = list_witnesses(room_wire_pk)?;
    let mut fps = std::collections::HashSet::new();
    for w in witnesses {
        if w.chain_root == chain_root && w.pin_set_hash == pin_set_hash_hex {
            fps.insert(w.witness_fp);
        }
    }
    Ok(fps.len() as u64)
}

pub fn require_quorum_met(
    room_wire_pk: &str,
    chain_root: &str,
    pins: &[MemoryPin],
    required: u64,
) -> Result<()> {
    if required == 0 {
        return Ok(());
    }
    let hash = pin_set_hash(pins);
    let count = count_quorum_witnesses(room_wire_pk, chain_root, &hash)?;
    if count < required {
        return Err(MemError::Coin(format!(
            "quorum not met: need {required} distinct witness_fp, got {count} (run its-memory witness on peer hosts)"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_set_hash_stable() {
        let pins = vec![
            MemoryPin {
                room_wire_pk: "aa".repeat(32),
                pool_epoch: 1,
                wire_b64: String::new(),
                seq_hint: None,
                wire_hash: "c".repeat(64),
            },
            MemoryPin {
                room_wire_pk: "aa".repeat(32),
                pool_epoch: 2,
                wire_b64: String::new(),
                seq_hint: None,
                wire_hash: "a".repeat(64),
            },
        ];
        assert_eq!(pin_set_hash(&pins), pin_set_hash(&pins));
    }
}
