use std::env;
use std::path::PathBuf;

use crate::coin::{mint_coin, validate_coin, MintOptions};
use crate::directory::{browse, publish_manifest, publish_to_pool, search, SortKey};
use crate::error::{MemError, Result};
use crate::vault::ratchet_seed_path;
use crate::wire::CoinManifest;

pub fn run_cli(program: &str) -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage(program);
        return Ok(());
    }
    match args[1].as_str() {
        "mint" => cmd_mint(&args[2..]),
        "validate" => cmd_validate(&args[2..]),
        "publish" => cmd_publish(&args[2..]),
        "browse" => cmd_browse(&args[2..]),
        "search" => cmd_search(&args[2..]),
        "-h" | "--help" | "help" => {
            print_usage(program);
            Ok(())
        }
        _ => Err(MemError::Usage(format!("unknown command: {}", args[1]))),
    }
}

fn cmd_mint(args: &[String]) -> Result<()> {
    let room_wire_pk = flag_str(args, "--room-wire-pk")
        .ok_or_else(|| MemError::Usage("--room-wire-pk HEX".into()))?;
    let pin_dir = flag_path(args, "--pin-dir");
    let out = flag_path(args, "--out");
    let manifest = mint_coin(&MintOptions {
        room_wire_pk,
        pin_dir,
        room_id: flag_str(args, "--room-id"),
        decrypt_pk: flag_path(args, "--decrypt-pk"),
        decrypt_sk: flag_path(args, "--decrypt-sk"),
        quorum_replicas: flag_str(args, "--quorum-replicas").and_then(|s| s.parse().ok()),
        ssc_out: flag_path(args, "--ssc-out"),
    })?;
    if let Some(path) = out {
        manifest.write_file(&path)?;
        println!("Minted ITS-COIN/1 -> {}", path.display());
    } else {
        print!("{}", manifest.serialize_text());
    }
    Ok(())
}

fn cmd_validate(args: &[String]) -> Result<()> {
    let manifest_path = flag_path(args, "--manifest")
        .ok_or_else(|| MemError::Usage("--manifest PATH".into()))?;
    let manifest = CoinManifest::read_file(&manifest_path)?;
    validate_coin(
        &manifest,
        flag_path(args, "--pin-dir").as_deref(),
        flag_path(args, "--decrypt-pk").as_deref(),
        flag_path(args, "--decrypt-sk").as_deref(),
    )
}

fn cmd_publish(args: &[String]) -> Result<()> {
    let manifest = flag_path(args, "--manifest")
        .ok_or_else(|| MemError::Usage("--manifest PATH".into()))?;
    let registry = flag_path(args, "--registry");
    publish_manifest(&manifest, registry.as_deref())?;
    if let (Some(config), Some(ratchet)) = (flag_path(args, "-c"), flag_path(args, "--ratchet-seed")) {
        publish_to_pool(&manifest, &config, &ratchet)?;
    }
    Ok(())
}

fn cmd_browse(args: &[String]) -> Result<()> {
    let sort = flag_str(args, "--sort")
        .map(|s| SortKey::parse(&s))
        .transpose()?
        .unwrap_or(SortKey::FrameCount);
    browse(flag_path(args, "--registry").as_deref(), sort).map(|_| ())
}

fn cmd_search(args: &[String]) -> Result<()> {
    let min = flag_str(args, "--min-frames")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let sort = flag_str(args, "--sort")
        .map(|s| SortKey::parse(&s))
        .transpose()?
        .unwrap_or(SortKey::FrameCount);
    let hits = search(flag_path(args, "--registry").as_deref(), min, sort)?;
    println!("search hits: {}", hits.len());
    Ok(())
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
{program} — ITS activity directory (ITS-COIN/1)

Usage:
  {program} mint --room-wire-pk HEX [--pin-dir PATH] [--out PATH] [--room-id HEX] \\
    [--decrypt-pk PATH --decrypt-sk PATH] [--quorum-replicas N] [--ssc-out PATH]
  {program} validate --manifest PATH [--pin-dir PATH] [--decrypt-pk PATH --decrypt-sk PATH]
  {program} publish --manifest PATH [--registry PATH] [-c routing.toml --ratchet-seed PATH]
  {program} browse [--sort frame_count|last_epoch] [--registry PATH]
  {program} search --min-frames N [--sort frame_count|last_epoch] [--registry PATH]
"
    );
}
