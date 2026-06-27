use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::mirror::list_pins_in_dir;
use crate::pipe;
use crate::store::list_pins;
use crate::vault::{normalize_pk, pin_room_dir};
use crate::wire::{MemoryPin, wire_filename_tag};

pub struct FetchOptions {
    pub room_wire_pk: String,
    pub out_dir: PathBuf,
    pub from_epoch: u64,
    pub to_epoch: Option<u64>,
    pub from_seq_hint: Option<u64>,
    pub limit: Option<usize>,
    pub filter_pk: Option<PathBuf>,
    pub filter_sk: Option<PathBuf>,
    pub mirror_dir: Option<PathBuf>,
}

pub fn run_fetch(opts: &FetchOptions) -> Result<usize> {
    std::fs::create_dir_all(&opts.out_dir)?;
    let pins = if let Some(ref mirror) = opts.mirror_dir {
        list_pins_in_dir(mirror)?
    } else {
        list_pins(&normalize_pk(&opts.room_wire_pk))?
    };
    let filtered = filter_pins(pins, opts);
    export_pins(&filtered, opts)
}

pub fn export_filtered_pins(pins: Vec<MemoryPin>, opts: &FetchOptions) -> Result<usize> {
    std::fs::create_dir_all(&opts.out_dir)?;
    let filtered = filter_pins(pins, opts);
    export_pins(&filtered, opts)
}

pub fn filter_pins(mut pins: Vec<MemoryPin>, opts: &FetchOptions) -> Vec<MemoryPin> {
    pins.retain(|pin| {
        if pin.pool_epoch < opts.from_epoch {
            return false;
        }
        if let Some(to) = opts.to_epoch {
            if pin.pool_epoch > to {
                return false;
            }
        }
        if let Some(seq_min) = opts.from_seq_hint {
            return pin.seq_hint.is_some_and(|seq| seq >= seq_min);
        }
        true
    });
    pins.sort_by_key(|p| (p.pool_epoch, p.seq_hint.unwrap_or(0), p.wire_hash.clone()));
    if let Some(limit) = opts.limit {
        if pins.len() > limit {
            let start = pins.len().saturating_sub(limit);
            pins = pins.split_off(start);
        }
    }
    pins
}

fn export_pins(pins: &[MemoryPin], opts: &FetchOptions) -> Result<usize> {
    let mut exported = 0usize;
    for pin in pins {
        let wire_bytes = pin.wire_bytes()?;
        if let (Some(pub_key), Some(sec_key)) = (&opts.filter_pk, &opts.filter_sk) {
            let tag = wire_filename_tag(&wire_bytes);
            let tmp_wire = opts.out_dir.join(format!("try_{tag}.wire"));
            std::fs::write(&tmp_wire, &wire_bytes)?;
            let tmp_frame = opts.out_dir.join(format!("try_{tag}.frame"));
            if pipe::its_asymmetric_decrypt(pub_key, sec_key, &tmp_wire, &tmp_frame).is_err() {
                let _ = std::fs::remove_file(&tmp_wire);
                continue;
            }
            let _ = std::fs::remove_file(&tmp_wire);
            let _ = std::fs::remove_file(&tmp_frame);
        }
        let wire_path = opts
            .out_dir
            .join(format!("epoch_{:08}_seq_{}.wire", pin.pool_epoch, pin.seq_hint.unwrap_or(0)));
        std::fs::write(&wire_path, &wire_bytes)?;
        let pin_copy = opts.out_dir.join(format!("epoch_{:08}.pin", pin.pool_epoch));
        pin.write_file(&pin_copy)?;
        exported += 1;
    }
    println!("Fetched {exported} pins -> {}", opts.out_dir.display());
    Ok(exported)
}

pub fn fetch_pin_dir(room_wire_pk: &str, pin_dir: &Path) -> Result<Vec<PathBuf>> {
    let pk_norm = normalize_pk(room_wire_pk);
    if pin_dir.is_dir() {
        let mut wires = Vec::new();
        for entry in std::fs::read_dir(pin_dir)? {
            let path = entry?.path();
            if path.extension().and_then(|s| s.to_str()) == Some("wire") {
                wires.push(path);
            }
        }
        wires.sort();
        return Ok(wires);
    }
    let pins = list_pins(&pk_norm)?;
    Ok(pins
        .iter()
        .filter_map(|p| {
            let wire = p.wire_bytes().ok()?;
            let tag = wire_filename_tag(&wire);
            Some(pin_room_dir(&pk_norm).join(format!("{tag}.pin")))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::pin_from_wire;

    fn pin_at(epoch: u64, seq: Option<u64>, wire: &[u8]) -> MemoryPin {
        let mut p = pin_from_wire("aa".repeat(64).as_str(), epoch, wire).unwrap();
        p.seq_hint = seq;
        p
    }

    #[test]
    fn filter_epoch_window_and_limit() {
        let pins = vec![
            pin_at(1, Some(1), b"a"),
            pin_at(2, Some(2), b"b"),
            pin_at(3, Some(3), b"c"),
            pin_at(4, Some(4), b"d"),
        ];
        let opts = FetchOptions {
            room_wire_pk: "aa".repeat(64),
            out_dir: PathBuf::from("/tmp/out"),
            from_epoch: 2,
            to_epoch: Some(3),
            from_seq_hint: None,
            limit: None,
            filter_pk: None,
            filter_sk: None,
            mirror_dir: None,
        };
        let got = filter_pins(pins, &opts);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].pool_epoch, 2);
        assert_eq!(got[1].pool_epoch, 3);
    }

    #[test]
    fn filter_seq_hint_and_latest_limit() {
        let pins = vec![
            pin_at(1, Some(1), b"a"),
            pin_at(2, Some(2), b"b"),
            pin_at(3, Some(3), b"c"),
            pin_at(4, Some(4), b"d"),
            pin_at(5, None, b"skip"),
        ];
        let opts = FetchOptions {
            room_wire_pk: "aa".repeat(64),
            out_dir: PathBuf::from("/tmp/out"),
            from_epoch: 0,
            to_epoch: None,
            from_seq_hint: Some(3),
            limit: Some(2),
            filter_pk: None,
            filter_sk: None,
            mirror_dir: None,
        };
        let got = filter_pins(pins, &opts);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].pool_epoch, 3);
        assert_eq!(got[1].pool_epoch, 4);
    }
}
