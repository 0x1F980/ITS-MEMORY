use std::env;
use std::path::PathBuf;

use crate::error::{MemError, Result};
use crate::fetch::{FetchOptions, run_fetch};
use crate::pin::{run_pin, PinOptions};
use crate::vault::ratchet_seed_path;

pub fn run_cli(program: &str) -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage(program);
        return Ok(());
    }
    match args[1].as_str() {
        "pin" => cmd_pin(&args[2..]),
        "fetch" => cmd_fetch(&args[2..]),
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
    let opts = FetchOptions {
        room_wire_pk,
        out_dir: out,
        from_epoch,
        filter_pk: flag_path(args, "--filter-pk"),
        filter_sk: flag_path(args, "--filter-sk"),
    };
    run_fetch(&opts).map(|_| ())
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
  {program} fetch --room-wire-pk HEX --out DIR [--from-epoch N] \\
    [--filter-pk PATH --filter-sk PATH]
"
    );
}
