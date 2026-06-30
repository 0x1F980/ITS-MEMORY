use std::env;
use std::path::PathBuf;

use crate::error::{MemError, Result};
use crate::fetch::{FetchOptions, run_fetch};
use crate::host;
use crate::mirror::publish_pins;
use crate::pin::{run_pin, PinOptions};
use crate::vault::ratchet_seed_path;
use crate::witness::{write_witness, write_witness_from_manifest};
use crate::blind::run_blind_pull;
use crate::wire::ChannelCoinManifest;

pub fn run_cli(program: &str) -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage(program);
        return Ok(());
    }
    match args[1].as_str() {
        "pin" => cmd_pin(&args[2..]),
        "fetch" => cmd_fetch(&args[2..]),
        "publish-pins" => cmd_publish_pins(&args[2..]),
        "host-status" => cmd_host_status(&args[2..]),
        "witness" => cmd_witness(&args[2..]),
        "blind-pull" => cmd_blind_pull(&args[2..]),
        "-h" | "--help" | "help" => {
            print_usage(program);
            Ok(())
        }
        _ => Err(MemError::Usage(format!("unknown command: {}", args[1]))),
    }
}

fn cmd_pin(args: &[String]) -> Result<()> {
    let room_wire_pk = flag_str(args, "--room-wire-pk")
        .ok_or_else(|| MemError::Usage("--room-wire-pk HEX".into()))?;
    let config = flag_path(args, "-c")
        .or_else(|| flag_path(args, "--pool-config"))
        .ok_or_else(|| MemError::Usage("-c routing.toml".into()))?;
    let ratchet = flag_path(args, "--ratchet-seed");
    let follow = args.iter().any(|a| a == "--follow");
    let max = flag_str(args, "--max-messages").and_then(|s| s.parse().ok());
    let timeout = flag_str(args, "--timeout-secs")
        .and_then(|s| s.parse().ok())
        .unwrap_or(120u64);
    let opts = PinOptions {
        room_wire_pk,
        config,
        ratchet_seed: ratchet_seed_path(ratchet.as_deref()),
        follow,
        max_messages: max,
        follow_timeout_secs: timeout,
        filter_pk: flag_path(args, "--filter-pk"),
        filter_sk: flag_path(args, "--filter-sk"),
    };
    let n = run_pin(&opts)?;
    println!("Pinned {n} wire(s) for room_wire_pk={}", opts.room_wire_pk);
    Ok(())
}

fn cmd_fetch(args: &[String]) -> Result<()> {
    let room_wire_pk = flag_str(args, "--room-wire-pk")
        .ok_or_else(|| MemError::Usage("--room-wire-pk HEX".into()))?;
    let out = flag_path(args, "--out")
        .ok_or_else(|| MemError::Usage("--out DIR".into()))?;
    let from_epoch = flag_str(args, "--from-epoch")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let to_epoch = flag_str(args, "--to-epoch").and_then(|s| s.parse().ok());
    let from_seq_hint = flag_str(args, "--from-seq-hint").and_then(|s| s.parse().ok());
    let limit = flag_str(args, "--limit").and_then(|s| s.parse().ok());
    let routing_config = flag_path(args, "--routing-config");
    let _epoch_map = crate::epoch_map::EpochMap::from_env_and_options(
        routing_config.as_deref(),
        flag_str(args, "--epoch-interval-ms").and_then(|s| s.parse().ok()),
    )?;
    let opts = FetchOptions {
        room_wire_pk,
        out_dir: out,
        from_epoch,
        to_epoch,
        from_seq_hint,
        limit,
        filter_pk: flag_path(args, "--filter-pk"),
        filter_sk: flag_path(args, "--filter-sk"),
        mirror_dir: flag_path(args, "--mirror-dir"),
    };
    run_fetch(&opts).map(|_| ())
}

fn cmd_publish_pins(args: &[String]) -> Result<()> {
    let room_wire_pk = flag_str(args, "--room-wire-pk")
        .ok_or_else(|| MemError::Usage("--room-wire-pk HEX".into()))?;
    publish_pins(&room_wire_pk).map(|_| ())
}

fn cmd_host_status(args: &[String]) -> Result<()> {
    let room_wire_pk = flag_str(args, "--room-wire-pk")
        .ok_or_else(|| MemError::Usage("--room-wire-pk HEX".into()))?;
    match host::room_host_status(&room_wire_pk)? {
        Some(entry) => {
            let hosted = host::hosted_seconds(&room_wire_pk)?;
            println!(
                "room_wire_pk={} pin_count={} memory_bytes={} hosted_seconds={} first_seen={} last_seen={} first_published={:?}",
                entry.room_wire_pk,
                entry.pin_count,
                entry.memory_bytes,
                hosted,
                entry.first_seen,
                entry.last_seen,
                entry.first_published
            );
        }
        None => println!("No local host ledger entry for room_wire_pk={room_wire_pk}"),
    }
    Ok(())
}

fn cmd_witness(args: &[String]) -> Result<()> {
    let room_wire_pk = flag_str(args, "--room-wire-pk")
        .ok_or_else(|| MemError::Usage("--room-wire-pk HEX".into()))?;
    if let Some(manifest_path) = flag_path(args, "--manifest") {
        let manifest = ChannelCoinManifest::read_file(&manifest_path)?;
        let pin_dir = flag_path(args, "--pin-dir")
            .ok_or_else(|| MemError::Usage("--pin-dir PATH (with --manifest)".into()))?;
        let pins = crate::coin::load_pins_for_witness(&pin_dir, &manifest.room_wire_pk)?;
        write_witness_from_manifest(&manifest, &pins)?;
    } else {
        let chain_root = flag_str(args, "--chain-root")
            .ok_or_else(|| MemError::Usage("--chain-root HEX or --manifest PATH".into()))?;
        let pin_dir = flag_path(args, "--pin-dir")
            .ok_or_else(|| MemError::Usage("--pin-dir PATH".into()))?;
        let pk_norm = crate::vault::normalize_pk(&room_wire_pk);
        let pins = crate::coin::load_pins_for_witness(&pin_dir, &pk_norm)?;
        write_witness(&pk_norm, &chain_root, &pins, None)?;
    }
    Ok(())
}

fn cmd_blind_pull(args: &[String]) -> Result<()> {
    let config = flag_path(args, "-c")
        .or_else(|| flag_path(args, "--pool-config"))
        .ok_or_else(|| MemError::Usage("-c routing.toml".into()))?;
    let ratchet = ratchet_seed_path(flag_path(args, "--ratchet-seed").as_deref());
    let max = flag_str(args, "--max-messages")
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);
    let timeout = flag_str(args, "--timeout-secs")
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    run_blind_pull(&config, &ratchet, max, timeout).map(|_| ())
}

fn flag_str(args: &[String], name: &str) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == name && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
        i += 1;
    }
    None
}

fn flag_path(args: &[String], name: &str) -> Option<PathBuf> {
    flag_str(args, name).map(PathBuf::from)
}

fn print_usage(program: &str) {
    eprintln!(
        "\
{program} — neutral ITS wire mirror (ITS-MEMORY-PIN/1)

Usage:
  {program} pin --room-wire-pk HEX -c routing.toml [--follow] [--max-messages N] \\
    [--ratchet-seed PATH] [--filter-pk PATH --filter-sk PATH] [--timeout-secs N]
  {program} fetch --room-wire-pk HEX --out DIR [--from-epoch N] [--to-epoch M] [--limit K] \\
    [--from-seq-hint N] [--mirror-dir PATH] [--routing-config PATH] [--epoch-interval-ms MS] \\
    [--filter-pk PATH --filter-sk PATH]
  {program} publish-pins --room-wire-pk HEX
  {program} host-status --room-wire-pk HEX
  {program} witness --room-wire-pk HEX --pin-dir PATH (--chain-root HEX | --manifest PATH)
  {program} blind-pull -c routing.toml [--ratchet-seed PATH] [--max-messages N] [--timeout-secs N]
"
    );
}
