use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine;

use crate::error::{MemError, Result};
use crate::pipe::{self, parse_next_pool_epoch, parse_wrote_epoch};
use crate::store::{dedup_exists, pin_from_wire, store_pin, update_seq_hint};
use crate::vault::{ensure_layout, normalize_pk, ratchet_seed_path};

pub struct PinOptions {
    pub room_wire_pk: String,
    pub config: PathBuf,
    pub ratchet_seed: PathBuf,
    pub follow: bool,
    pub max_messages: Option<usize>,
    pub follow_timeout_secs: u64,
    pub filter_pk: Option<PathBuf>,
    pub filter_sk: Option<PathBuf>,
}

pub fn run_pin(opts: &PinOptions) -> Result<usize> {
    ensure_layout()?;
    let pk = normalize_pk(&opts.room_wire_pk);
    if opts.follow {
        pin_follow(opts)
    } else {
        pin_poll(opts, pk)
    }
}

fn pin_poll(opts: &PinOptions, room_wire_pk: String) -> Result<usize> {
    let limit = opts.max_messages.unwrap_or(1);
    let deadline = Instant::now() + Duration::from_secs(240);
    let mut from_epoch = 0u64;
    let mut stored = 0usize;
    let tmp_wire = std::env::temp_dir().join(format!("its_mem_wire_{}", rand_suffix()));

    while stored < limit && Instant::now() < deadline {
        let recv = pipe::its_routing_receive_once(
            &opts.config,
            &opts.ratchet_seed,
            &tmp_wire,
            60,
            from_epoch,
        )?;
        if recv.wire_bytes.is_empty() {
            thread::sleep(Duration::from_millis(150));
            continue;
        }
        let hash = crate::wire::wire_identity(&recv.wire_bytes);
        if dedup_exists(&room_wire_pk, &recv.wire_bytes)? {
            if let Some(next) = parse_next_pool_epoch(&recv.stdout) {
                from_epoch = next;
            }
            continue;
        }
        if !wire_matches_room(opts, &recv.wire_bytes)? {
            if let Some(next) = parse_next_pool_epoch(&recv.stdout) {
                from_epoch = next;
            }
            continue;
        }
        let epoch = parse_wrote_epoch(&recv.stdout).unwrap_or(from_epoch);
        let pin = pin_from_wire(&room_wire_pk, epoch, &recv.wire_bytes)?;
        let path = store_pin(&pin)?;
        if let Some(seq) = decrypt_seq_hint(opts, &recv.wire_bytes)? {
            let _ = update_seq_hint(&path, seq);
        }
        stored += 1;
        eprintln!("Pinned wire epoch={epoch} hash={}…", &hash[..8]);
        if let Some(next) = parse_next_pool_epoch(&recv.stdout) {
            from_epoch = next;
            eprintln!("ITS_EPOCH_CURSOR={next}");
        }
    }
    let _ = std::fs::remove_file(&tmp_wire);
    Ok(stored)
}

fn pin_follow(opts: &PinOptions) -> Result<usize> {
    let room_wire_pk = normalize_pk(&opts.room_wire_pk);
    let limit = opts.max_messages.unwrap_or(usize::MAX);
    let wire_path = std::env::temp_dir().join(format!("its_mem_follow_{}", rand_suffix()));
    let _ = std::fs::remove_file(&wire_path);
    let mut child = pipe::spawn_routing_follow(
        &opts.config,
        &opts.ratchet_seed,
        &wire_path,
        opts.follow_timeout_secs,
    )?;
    let deadline = Instant::now() + Duration::from_secs(opts.follow_timeout_secs);
    let mut stored = 0usize;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    while stored < limit && Instant::now() < deadline {
        if wire_path.is_file() {
            let bytes = std::fs::read(&wire_path)?;
            if !bytes.is_empty() {
                let hash = crate::wire::wire_identity(&bytes);
                if !seen.contains(&hash) && !dedup_exists(&room_wire_pk, &bytes)? {
                    if wire_matches_room(opts, &bytes)? {
                        thread::sleep(Duration::from_millis(120));
                        let stable = std::fs::read(&wire_path).unwrap_or_default();
                        if stable.is_empty() {
                            continue;
                        }
                        let hash = crate::wire::wire_identity(&stable);
                        if seen.contains(&hash) || dedup_exists(&room_wire_pk, &stable)? {
                            continue;
                        }
                        if !wire_matches_room(opts, &stable)? {
                            continue;
                        }
                        let pin = pin_from_wire(&room_wire_pk, 0, &stable)?;
                        let path = store_pin(&pin)?;
                        if let Some(seq) = decrypt_seq_hint(opts, &stable)? {
                            let _ = update_seq_hint(&path, seq);
                        }
                        seen.insert(hash.clone());
                        stored += 1;
                        eprintln!("Pinned wire hash={}…", &hash[..8]);
                    }
                }
            }
        }
        if child.try_wait()?.is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(80));
    }
    let _ = child.kill();
    let _ = std::fs::remove_file(&wire_path);
    Ok(stored)
}

fn wire_matches_room(opts: &PinOptions, wire_bytes: &[u8]) -> Result<bool> {
    let (pk, sk) = match (&opts.filter_pk, &opts.filter_sk) {
        (Some(pk), Some(sk)) => (pk, sk),
        _ => return Ok(true),
    };
    let tmp_wire = std::env::temp_dir().join(format!("its_mem_filter_w_{}", rand_suffix()));
    let tmp_frame = std::env::temp_dir().join(format!("its_mem_filter_f_{}", rand_suffix()));
    std::fs::write(&tmp_wire, wire_bytes)?;
    let ok = pipe::its_asymmetric_decrypt(pk, sk, &tmp_wire, &tmp_frame).is_ok();
    let _ = std::fs::remove_file(&tmp_wire);
    let _ = std::fs::remove_file(&tmp_frame);
    Ok(ok)
}

fn decrypt_seq_hint(opts: &PinOptions, wire_bytes: &[u8]) -> Result<Option<u64>> {
    let (pk, sk) = match (&opts.filter_pk, &opts.filter_sk) {
        (Some(pk), Some(sk)) => (pk, sk),
        _ => return Ok(None),
    };
    let tmp_wire = std::env::temp_dir().join(format!("its_mem_seq_w_{}", rand_suffix()));
    let tmp_frame = std::env::temp_dir().join(format!("its_mem_seq_f_{}", rand_suffix()));
    std::fs::write(&tmp_wire, wire_bytes)?;
    if pipe::its_asymmetric_decrypt(pk, sk, &tmp_wire, &tmp_frame).is_err() {
        let _ = std::fs::remove_file(&tmp_wire);
        return Ok(None);
    }
    let frame = std::fs::read_to_string(&tmp_frame).unwrap_or_default();
    let _ = std::fs::remove_file(&tmp_wire);
    let _ = std::fs::remove_file(&tmp_frame);
    Ok(pipe::try_parse_frame_seq(&frame))
}

fn rand_suffix() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}

pub fn default_pin_opts(
    room_wire_pk: &str,
    config: &Path,
    ratchet: Option<&Path>,
) -> PinOptions {
    PinOptions {
        room_wire_pk: room_wire_pk.to_string(),
        config: config.to_path_buf(),
        ratchet_seed: ratchet_seed_path(ratchet),
        follow: false,
        max_messages: None,
        follow_timeout_secs: 120,
        filter_pk: None,
        filter_sk: None,
    }
}
