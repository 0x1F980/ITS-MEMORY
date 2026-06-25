use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use crate::error::{MemError, Result};

fn env_bin(var: &str, default: &str) -> Result<PathBuf> {
    let name = std::env::var(var).unwrap_or_else(|_| default.to_string());
    which(&name).ok_or_else(|| MemError::Pipe(format!("binary '{name}' not on PATH")))
}

fn which(bin: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(bin);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub struct ReceiveOutput {
    pub stdout: String,
    pub wire_bytes: Vec<u8>,
}

pub fn spawn_routing_follow(
    config: &Path,
    ratchet_seed: &Path,
    out_wire: &Path,
    timeout_secs: u64,
) -> Result<Child> {
    let bin = env_bin("ITS_ROUTING_BIN", "its-routing")?;
    let child = Command::new(bin)
        .arg("-c")
        .arg(config)
        .arg("client-receive")
        .arg("--pool")
        .arg("--follow")
        .arg("--ratchet-seed-file")
        .arg(ratchet_seed)
        .arg("-o")
        .arg(out_wire)
        .arg("--timeout-secs")
        .arg(timeout_secs.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| MemError::Pipe(format!("spawn follow: {e}")))?;
    Ok(child)
}

pub fn its_routing_receive_once(
    config: &Path,
    ratchet_seed: &Path,
    out_wire: &Path,
    timeout_secs: u64,
    from_epoch: u64,
) -> Result<ReceiveOutput> {
    let bin = env_bin("ITS_ROUTING_BIN", "its-routing")?;
    let mut cmd = Command::new(&bin);
    cmd.arg("-c")
        .arg(config)
        .arg("client-receive")
        .arg("--pool")
        .arg("--ratchet-seed-file")
        .arg(ratchet_seed)
        .arg("-o")
        .arg(out_wire)
        .arg("--timeout-secs")
        .arg(timeout_secs.to_string())
        .arg("--continuous");
    if from_epoch > 0 {
        cmd.arg("--from-epoch").arg(from_epoch.to_string());
    }
    let output = cmd
        .output()
        .map_err(|e| MemError::Pipe(format!("spawn receive: {e}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if !output.status.success() && !out_wire.is_file() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MemError::Pipe(format!(
            "receive failed: {stdout}{stderr}"
        )));
    }
    let wire_bytes = std::fs::read(out_wire).unwrap_or_default();
    Ok(ReceiveOutput { stdout, wire_bytes })
}

pub fn parse_next_pool_epoch(stdout: &str) -> Option<u64> {
    let relevant = if let Some(idx) = stdout.find("Wrote ") {
        &stdout[..idx]
    } else {
        stdout
    };
    let mut epochs: Vec<u64> = Vec::new();
    for line in relevant.lines() {
        if let Some(idx) = line.find(" at epoch ") {
            let tail = line[idx + 10..].trim().trim_end_matches('.');
            if let Ok(epoch) = tail.parse::<u64>() {
                epochs.push(epoch);
            }
        }
    }
    if epochs.is_empty() {
        return None;
    }
    epochs.sort_unstable();
    const K: usize = 2;
    let pivot_idx = epochs.len().min(K).saturating_sub(1);
    Some(epochs[pivot_idx].saturating_add(1))
}

pub fn parse_wrote_epoch(stdout: &str) -> Option<u64> {
    for line in stdout.lines() {
        if let Some(idx) = line.find(" at epoch ") {
            let tail = line[idx + 10..].trim().trim_end_matches('.');
            if let Ok(epoch) = tail.parse::<u64>() {
                return Some(epoch);
            }
        }
    }
    parse_next_pool_epoch(stdout).map(|e| e.saturating_sub(1))
}

pub fn its_asymmetric_decrypt(pk: &Path, sk: &Path, wire: &Path, out: &Path) -> Result<()> {
    let bin = env_bin("ITS_ASYMMETRIC_BIN", "its_asymmetric")?;
    let output = Command::new(bin)
        .arg("decrypt")
        .arg("--pk")
        .arg(pk)
        .arg("--sk")
        .arg(sk)
        .arg("--in")
        .arg(wire)
        .arg("--out")
        .arg(out)
        .output()
        .map_err(|e| MemError::Pipe(format!("decrypt spawn: {e}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(MemError::Pipe(format!(
            "decrypt failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

pub fn its_routing_send(config: &Path, wire: &Path, ratchet_seed: &Path) -> Result<()> {
    let bin = env_bin("ITS_ROUTING_BIN", "its-routing")?;
    let output = Command::new(bin)
        .arg("-c")
        .arg(config)
        .arg("client-send")
        .arg("--pool")
        .arg("--file")
        .arg(wire)
        .arg("--ratchet-seed-file")
        .arg(ratchet_seed)
        .output()
        .map_err(|e| MemError::Pipe(format!("send spawn: {e}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(MemError::Pipe(format!(
            "send failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

pub fn try_parse_frame_seq(frame_text: &str) -> Option<u64> {
    for line in frame_text.lines() {
        if let Some(v) = line.strip_prefix("seq:") {
            return v.trim().parse().ok();
        }
    }
    None
}

pub fn try_parse_frame_room_id(frame_text: &str) -> Option<String> {
    for line in frame_text.lines() {
        if let Some(v) = line.strip_prefix("room_id:") {
            return Some(v.trim().to_string());
        }
    }
    None
}

pub fn sss_chain_bin() -> Result<PathBuf> {
    env_bin("SSS_CHAIN_BIN", "sss_chain")
}

pub fn sss_chain_generate(
    root_path: &Path,
    total_bytes: usize,
    link_byte_len: usize,
    out_ssc: &Path,
) -> Result<()> {
    let bin = sss_chain_bin()?;
    let output = Command::new(&bin)
        .arg("generate")
        .arg("--root")
        .arg(root_path)
        .arg("--total-bytes")
        .arg(total_bytes.to_string())
        .arg("--link-byte-len")
        .arg(link_byte_len.to_string())
        .arg("--out")
        .arg(out_ssc)
        .arg("--quiet")
        .output()
        .map_err(|e| MemError::Pipe(format!("sss_chain spawn failed ({bin:?}): {e}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(MemError::Pipe(format!(
            "sss_chain generate failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

pub fn sss_chain_validate(root_path: &Path, ssc_path: &Path) -> Result<bool> {
    let bin = sss_chain_bin()?;
    let output = Command::new(&bin)
        .arg("validate")
        .arg("--root")
        .arg(root_path)
        .arg("--in")
        .arg(ssc_path)
        .arg("--quiet")
        .output()
        .map_err(|e| MemError::Pipe(format!("sss_chain validate: {e}")))?;
    Ok(output.status.success())
}

/// Parse `link_0` hex from an `.ssc` file (first `hex:` block).
pub fn parse_ssc_link0_hex(ssc_text: &str) -> Result<String> {
    let mut seen_block = false;
    for line in ssc_text.lines() {
        let line = line.trim();
        if line == "---" {
            seen_block = true;
            continue;
        }
        if seen_block {
            if let Some(hex) = line.strip_prefix("hex:") {
                let h = hex.trim().replace(' ', "");
                if h.is_empty() {
                    return Err(MemError::Coin("empty link_0 hex in .ssc".into()));
                }
                return Ok(h);
            }
        }
    }
    Err(MemError::Coin("missing link_0 in .ssc".into()))
}

