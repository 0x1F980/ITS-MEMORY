use std::env;
use std::path::PathBuf;

use crate::coin::{mint_coin, validate_coin, MintOptions};
use crate::directory::{
    browse_channel, browse_gdir, publish_channel_manifest, publish_gdir_manifest,
    publish_gdir_to_pool, publish_manifest, publish_to_pool, search_channel, ChannelSortKey,
    GdirSortKey,
};
use crate::error::{MemError, Result};
use crate::gdir::{mint_gdir_coin, record_contrib, validate_gdir_coin};
use crate::wire::{ChannelCoinManifest, GdirCoinManifest, GdirOp};

pub fn run_cli(program: &str) -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage(program);
        return Ok(());
    }
    match args[1].as_str() {
        "channel" => dispatch_channel(&args[2..], program),
        "gdir" => dispatch_gdir(&args[2..], program),
        // Legacy v1 commands default to channel coin.
        "mint" | "validate" | "publish" | "browse" | "search" => {
            dispatch_channel(&args[1..], program)
        }
        "-h" | "--help" | "help" => {
            print_usage(program);
            Ok(())
        }
        _ => Err(MemError::Usage(format!("unknown command: {}", args[1]))),
    }
}

fn dispatch_channel(args: &[String], _program: &str) -> Result<()> {
    if args.is_empty() {
        return Err(MemError::Usage("channel subcommand required".into()));
    }
    match args[0].as_str() {
        "mint" => cmd_channel_mint(&args[1..]),
        "validate" => cmd_channel_validate(&args[1..]),
        "publish" => cmd_channel_publish(&args[1..]),
        "browse" => cmd_channel_browse(&args[1..]),
        "search" => cmd_channel_search(&args[1..]),
        _ => Err(MemError::Usage(format!("unknown channel command: {}", args[0]))),
    }
}

fn dispatch_gdir(args: &[String], _program: &str) -> Result<()> {
    if args.is_empty() {
        return Err(MemError::Usage("gdir subcommand required".into()));
    }
    match args[0].as_str() {
        "record" => cmd_gdir_record(&args[1..]),
        "mint" => cmd_gdir_mint(&args[1..]),
        "validate" => cmd_gdir_validate(&args[1..]),
        "publish" => cmd_gdir_publish(&args[1..]),
        "browse" => cmd_gdir_browse(&args[1..]),
        _ => Err(MemError::Usage(format!("unknown gdir command: {}", args[0]))),
    }
}

fn cmd_channel_mint(args: &[String]) -> Result<()> {
    let room_wire_pk = flag_str(args, "--room-wire-pk")
        .ok_or_else(|| MemError::Usage("--room-wire-pk HEX".into()))?;
    let require_published = args.iter().any(|a| a == "--require-published");
    let registry_visible = !args.iter().any(|a| a == "--registry-hidden");
    let manifest = mint_coin(&MintOptions {
        room_wire_pk,
        pin_dir: flag_path(args, "--pin-dir"),
        room_id: flag_str(args, "--room-id"),
        decrypt_pk: flag_path(args, "--decrypt-pk"),
        decrypt_sk: flag_path(args, "--decrypt-sk"),
        quorum_replicas: flag_str(args, "--quorum-replicas").and_then(|s| s.parse().ok()),
        ssc_out: flag_path(args, "--ssc-out"),
        require_published,
        registry_visible,
    })?;
    if let Some(path) = flag_path(args, "--out") {
        manifest.write_file(&path)?;
        println!("Minted ITS-CHANNEL-COIN/2 -> {}", path.display());
    } else {
        print!("{}", manifest.serialize_text());
    }
    Ok(())
}

fn cmd_channel_validate(args: &[String]) -> Result<()> {
    let manifest_path = flag_path(args, "--manifest")
        .ok_or_else(|| MemError::Usage("--manifest PATH".into()))?;
    let manifest = ChannelCoinManifest::read_file(&manifest_path)?;
    validate_coin(
        &manifest,
        flag_path(args, "--pin-dir").as_deref(),
        flag_path(args, "--decrypt-pk").as_deref(),
        flag_path(args, "--decrypt-sk").as_deref(),
    )
}

fn cmd_channel_publish(args: &[String]) -> Result<()> {
    let manifest = flag_path(args, "--manifest")
        .ok_or_else(|| MemError::Usage("--manifest PATH".into()))?;
    let registry = flag_path(args, "--registry");
    publish_channel_manifest(&manifest, registry.as_deref())?;
    if args.iter().any(|a| a == "--record-gdir") {
        let bytes = std::fs::metadata(&manifest)
            .map(|m| m.len())
            .unwrap_or(0) as u64;
        let _ = record_contrib(GdirOp::Sync, bytes.max(1))?;
    }
    if let (Some(config), Some(ratchet)) = (flag_path(args, "-c"), flag_path(args, "--ratchet-seed")) {
        publish_to_pool(&manifest, &config, &ratchet)?;
    }
    Ok(())
}

fn cmd_gdir_record(args: &[String]) -> Result<()> {
    let op = flag_str(args, "--op")
        .ok_or_else(|| MemError::Usage("--op mirror|sync|route".into()))?;
    let byte_span = flag_str(args, "--byte-span")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    record_contrib(GdirOp::parse(&op)?, byte_span).map(|_| ())
}

fn cmd_gdir_mint(args: &[String]) -> Result<()> {
    let manifest = mint_gdir_coin()?;
    if let Some(path) = flag_path(args, "--out") {
        manifest.write_file(&path)?;
        println!("Minted ITS-GDIR-COIN/1 -> {}", path.display());
    } else {
        print!("{}", manifest.serialize_text());
    }
    Ok(())
}

fn cmd_gdir_validate(args: &[String]) -> Result<()> {
    let manifest_path = flag_path(args, "--manifest")
        .ok_or_else(|| MemError::Usage("--manifest PATH".into()))?;
    let manifest = GdirCoinManifest::read_file(&manifest_path)?;
    validate_gdir_coin(&manifest)
}

fn cmd_gdir_publish(args: &[String]) -> Result<()> {
    let manifest = flag_path(args, "--manifest")
        .ok_or_else(|| MemError::Usage("--manifest PATH".into()))?;
    let registry = flag_path(args, "--registry");
    publish_gdir_manifest(&manifest, registry.as_deref())?;
    if let (Some(config), Some(ratchet)) = (flag_path(args, "-c"), flag_path(args, "--ratchet-seed")) {
        publish_gdir_to_pool(&manifest, &config, &ratchet)?;
    }
    Ok(())
}

fn cmd_gdir_browse(args: &[String]) -> Result<()> {
    let sort = flag_str(args, "--sort")
        .map(|s| GdirSortKey::parse(&s))
        .transpose()?
        .unwrap_or(GdirSortKey::ContribBytes);
    browse_gdir(flag_path(args, "--registry").as_deref(), sort).map(|_| ())
}

fn cmd_channel_browse(args: &[String]) -> Result<()> {
    let sort = flag_str(args, "--sort")
        .map(|s| ChannelSortKey::parse(&s))
        .transpose()?
        .unwrap_or(ChannelSortKey::FrameCount);
    browse_channel(flag_path(args, "--registry").as_deref(), sort).map(|_| ())
}

fn cmd_channel_search(args: &[String]) -> Result<()> {
    let min = flag_str(args, "--min-frames")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let sort = flag_str(args, "--sort")
        .map(|s| ChannelSortKey::parse(&s))
        .transpose()?
        .unwrap_or(ChannelSortKey::FrameCount);
    let hits = search_channel(flag_path(args, "--registry").as_deref(), min, sort)?;
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
{program} — ITS activity directory (two coin types)

Channel coin (per room_wire_pk — hosting/activity):
  {program} channel mint --room-wire-pk HEX [--pin-dir PATH] [--out PATH] \\
    [--require-published] [--registry-hidden] [--decrypt-pk PATH --decrypt-sk PATH]
  {program} channel validate --manifest PATH [--pin-dir PATH]
  {program} channel publish --manifest PATH [--registry PATH] [--record-gdir] \\
    [-c routing.toml --ratchet-seed PATH]
  {program} channel browse [--sort frame_count|memory_bytes|hosted_seconds|last_epoch]
  {program} channel search --min-frames N [--sort ...]

Global directory coin (no room_wire_pk — infra contribution):
  {program} gdir record --op mirror|sync|route --byte-span N
  {program} gdir mint [--out PATH]
  {program} gdir validate --manifest PATH
  {program} gdir publish --manifest PATH [--registry PATH] [-c routing.toml --ratchet-seed PATH]
  {program} gdir browse [--sort contrib_bytes|contrib_seconds|contrib_ops]

Legacy: mint|validate|publish|browse|search map to channel commands.
"
    );
}
