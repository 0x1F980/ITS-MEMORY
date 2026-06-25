use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::pipe;
use crate::store::list_pins;
use crate::vault::{normalize_pk, pin_room_dir};
use crate::wire::wire_filename_tag;

pub struct FetchOptions {
    pub room_wire_pk: String,
    pub out_dir: PathBuf,
    pub from_epoch: u64,
    pub filter_pk: Option<PathBuf>,
    pub filter_sk: Option<PathBuf>,
}

pub fn run_fetch(opts: &FetchOptions) -> Result<usize> {
    std::fs::create_dir_all(&opts.out_dir)?;
    let pk_norm = normalize_pk(&opts.room_wire_pk);
    let pins = list_pins(&pk_norm)?;
    let mut exported = 0usize;
    for pin in pins {
        if pin.pool_epoch < opts.from_epoch {
            continue;
        }
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
            let _ = std::fs::remove_file(&tmp_frame);
        }
        let wire_path = opts.out_dir.join(format!("epoch_{:08}_seq_{}.wire", pin.pool_epoch, pin.seq_hint.unwrap_or(0)));
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
