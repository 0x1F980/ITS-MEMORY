use std::env;
use std::path::PathBuf;

use crate::coin::{mint_coin, validate_coin, MintOptions};
use crate::directory::{
    browse_channel, browse_gdir, browse_gdir_flat, discover_flat_gdir, discover_quiet_channel,
    discover_quiet_flat_channel, publish_channel_manifest, publish_gdir_manifest,
    publish_gdir_to_pool, publish_to_pool, search_channel, ChannelSearchFilters, ChannelSortKey,
    GdirSortKey, SortOrder,
};
use crate::error::{MemError, Result};
use crate::gdir::{mint_gdir_coin, mint_gdir_coin_options, record_contrib, validate_gdir_coin, MintGdirOptions};
use crate::ingest::{ingest_channel_pool, ingest_gdir_pool};
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
        "discover-quiet" => discover_quiet_channel(flag_path(args, "--registry").as_deref()).map(|_| ()),
        "discover-quiet-flat" => {
            let cap = flag_str(args, "--cap-bps")
                .and_then(|s| s.parse().ok())
                .unwrap_or(500);
            discover_quiet_flat_channel(flag_path(args, "--registry").as_deref(), cap).map(|_| ())
        }
        "ingest-pool" => cmd_channel_ingest_pool(&args[1..]),
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
        "discover-flat" => {
            let cap = flag_str(args, "--cap-bps")
                .and_then(|s| s.parse().ok())
                .unwrap_or(500);
            discover_flat_gdir(flag_path(args, "--registry").as_deref(), cap).map(|_| ())
        }
        "ingest-pool" => cmd_gdir_ingest_pool(&args[1..]),
        _ => Err(MemError::Usage(format!("unknown gdir command: {}", args[0]))),
    }
}

fn cmd_channel_mint(args: &[String]) -> Result<()> {
    let room_wire_pk = flag_str(args, "--room-wire-pk")
        .ok_or_else(|| MemError::Usage("--room-wire-pk HEX".into()))?;
    let require_published = args.iter().any(|a| a == "--require-published");
    let registry_visible = !args.iter().any(|a| a == "--registry-hidden");
    let require_global = args.iter().any(|a| a == "--global");
    let manifest = mint_coin(&MintOptions {
        room_wire_pk,
        pin_dir: flag_path(args, "--pin-dir"),
        room_id: flag_str(args, "--room-id"),
        decrypt_pk: flag_path(args, "--decrypt-pk"),
        decrypt_sk: flag_path(args, "--decrypt-sk"),
        quorum_replicas: flag_str(args, "--quorum-replicas").and_then(|s| s.parse().ok()),
        require_quorum: flag_str(args, "--require-quorum").and_then(|s| s.parse().ok()),
        ssc_out: flag_path(args, "--ssc-out"),
        require_published,
        registry_visible,
        require_global,
        timelock_unlock_epoch: flag_str(args, "--timelock-unlock-epoch").and_then(|s| s.parse().ok()),
        room_dir: flag_path(args, "--room-dir"),
        pool_config: flag_path(args, "-c").or_else(|| flag_path(args, "--pool-config")),
        ratchet_seed: flag_path(args, "--ratchet-seed"),
    })?;
    if let Some(path) = flag_path(args, "--out") {
        manifest.write_file(&path)?;
        println!("Minted ITS-CHANNEL-COIN/3 -> {}", path.display());
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
        flag_path(args, "--room-dir").as_deref(),
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

fn cmd_channel_ingest_pool(args: &[String]) -> Result<()> {
    let config = flag_path(args, "-c")
        .ok_or_else(|| MemError::Usage("-c routing.toml".into()))?;
    let ratchet = flag_path(args, "--ratchet-seed")
        .ok_or_else(|| MemError::Usage("--ratchet-seed PATH".into()))?;
    let max = flag_str(args, "--max-messages")
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);
    let timeout = flag_str(args, "--timeout-secs")
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    let n = ingest_channel_pool(
        &config,
        &ratchet,
        flag_path(args, "--registry").as_deref(),
        max,
        timeout,
    )?;
    println!("ingest-pool: {n} channel manifest(s)");
    Ok(())
}

fn cmd_gdir_ingest_pool(args: &[String]) -> Result<()> {
    let config = flag_path(args, "-c")
        .ok_or_else(|| MemError::Usage("-c routing.toml".into()))?;
    let ratchet = flag_path(args, "--ratchet-seed")
        .ok_or_else(|| MemError::Usage("--ratchet-seed PATH".into()))?;
    let max = flag_str(args, "--max-messages")
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);
    let timeout = flag_str(args, "--timeout-secs")
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    let n = ingest_gdir_pool(
        &config,
        &ratchet,
        flag_path(args, "--registry").as_deref(),
        max,
        timeout,
    )?;
    println!("ingest-pool: {n} gdir manifest(s)");
    Ok(())
}

fn cmd_gdir_record(args: &[String]) -> Result<()> {
    let op = flag_str(args, "--op")
        .ok_or_else(|| MemError::Usage("--op mirror|sync|route|blind".into()))?;
    let byte_span = flag_str(args, "--byte-span")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    record_contrib(GdirOp::parse(&op)?, byte_span).map(|_| ())
}

fn cmd_gdir_mint(args: &[String]) -> Result<()> {
    let require_blind = args.iter().any(|a| a == "--require-blind");
    let manifest = if require_blind {
        mint_gdir_coin_options(&MintGdirOptions {
            require_blind_or_infra: true,
        })?
    } else {
        mint_gdir_coin()?
    };
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
    let order = parse_order(args)?;
    if args.iter().any(|a| a == "--flatten") {
        let cap = flag_str(args, "--cap-bps")
            .and_then(|s| s.parse().ok())
            .unwrap_or(500);
        browse_gdir_flat(flag_path(args, "--registry").as_deref(), cap, order).map(|_| ())
    } else {
        browse_gdir(flag_path(args, "--registry").as_deref(), sort, order).map(|_| ())
    }
}

fn cmd_channel_browse(args: &[String]) -> Result<()> {
    let (sort, order) = if flag_str(args, "--discover").as_deref() == Some("quiet") {
        (ChannelSortKey::FrameCount, SortOrder::Asc)
    } else {
        let sort = flag_str(args, "--sort")
            .map(|s| ChannelSortKey::parse(&s))
            .transpose()?
            .unwrap_or(ChannelSortKey::FrameCount);
        (sort, parse_order(args)?)
    };
    browse_channel(flag_path(args, "--registry").as_deref(), sort, order).map(|_| ())
}

fn cmd_channel_search(args: &[String]) -> Result<()> {
    let filters = ChannelSearchFilters {
        min_frames: flag_str(args, "--min-frames").and_then(|s| s.parse().ok()),
        max_frames: flag_str(args, "--max-frames").and_then(|s| s.parse().ok()),
        max_memory_bytes: flag_str(args, "--max-memory-bytes").and_then(|s| s.parse().ok()),
        max_hosted_seconds: flag_str(args, "--max-hosted-seconds")
            .or_else(|| flag_str(args, "--max-mirror-listed-seconds"))
            .and_then(|s| s.parse().ok()),
    };
    let sort = flag_str(args, "--sort")
        .map(|s| ChannelSortKey::parse(&s))
        .transpose()?
        .unwrap_or(ChannelSortKey::FrameCount);
    let order = parse_order(args)?;
    let hits = search_channel(
        flag_path(args, "--registry").as_deref(),
        filters,
        sort,
        order,
    )?;
    println!("search hits: {}", hits.len());
    Ok(())
}

fn parse_order(args: &[String]) -> Result<SortOrder> {
    match flag_str(args, "--order") {
        Some(s) => SortOrder::parse(&s),
        None => Ok(SortOrder::Desc),
    }
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

Channel coin (per room_wire_pk — memory preservation):
  {program} channel mint --room-wire-pk HEX [--pin-dir PATH] [--out PATH] \\
    [--require-published] [--registry-hidden] [--global] [--require-quorum K] \\
    [--timelock-unlock-epoch N] [--room-dir PATH] [--decrypt-pk PATH --decrypt-sk PATH]
  {program} channel validate --manifest PATH [--pin-dir PATH] [--room-dir PATH]
  {program} channel publish --manifest PATH [--registry PATH] [--record-gdir] \\
    [-c routing.toml --ratchet-seed PATH]
  {program} channel ingest-pool -c routing.toml --ratchet-seed PATH [--registry PATH]
  {program} channel browse [--sort frame_count|memory_bytes|memory_weight_seconds|hosted_seconds|last_epoch] \\
    [--order asc|desc] [--discover quiet] [--registry PATH]
  {program} channel discover-quiet [--registry PATH]
  {program} channel discover-quiet-flat [--cap-bps 500] [--registry PATH]
  {program} channel search [--min-frames N] [--max-frames N] [--max-memory-bytes N] \\
    [--max-hosted-seconds N] [--sort ...] [--order asc|desc] [--registry PATH]

Global directory coin (no room_wire_pk — blind infra):
  {program} gdir record --op mirror|sync|route|blind --byte-span N
  {program} gdir mint [--out PATH] [--require-blind]
  {program} gdir validate --manifest PATH
  {program} gdir publish --manifest PATH [--registry PATH] [-c routing.toml --ratchet-seed PATH]
  {program} gdir ingest-pool -c routing.toml --ratchet-seed PATH [--registry PATH]
  {program} gdir browse [--sort contrib_bytes|contrib_seconds|contrib_ops] [--order asc|desc] [--flatten]
  {program} gdir discover-flat [--cap-bps 500] [--registry PATH]

Legacy: mint|validate|publish|browse|search map to channel commands.
"
    );
}
