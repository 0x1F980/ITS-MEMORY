use std::path::Path;

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::error::{MemError, Result};

pub const PIN_MAGIC: &str = "ITS-MEMORY-PIN/1";
/// Legacy channel coin (v1).
pub const CHANNEL_COIN_V1_MAGIC: &str = "ITS-COIN/1";
pub const CHANNEL_COIN_V2_MAGIC: &str = "ITS-CHANNEL-COIN/2";
pub const GDIR_RECEIPT_MAGIC: &str = "ITS-GDIR-RECEIPT/1";
pub const GDIR_COIN_MAGIC: &str = "ITS-GDIR-COIN/1";

/// Backward-compatible alias.
pub type CoinManifest = ChannelCoinManifest;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryPin {
    pub room_wire_pk: String,
    pub pool_epoch: u64,
    pub wire_b64: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq_hint: Option<u64>,
    /// Hex encoding of raw ciphertext bytes (exact identity, not a hash function).
    pub wire_hash: String,
}

impl MemoryPin {
    pub fn wire_bytes(&self) -> Result<Vec<u8>> {
        base64::engine::general_purpose::STANDARD
            .decode(self.wire_b64.trim())
            .map_err(|e| MemError::Wire(format!("wire_b64: {e}")))
    }

    pub fn serialize_text(&self) -> String {
        let mut out = String::new();
        out.push_str(PIN_MAGIC);
        out.push('\n');
        out.push_str(&format!("room_wire_pk: {}\n", self.room_wire_pk));
        out.push_str(&format!("pool_epoch: {}\n", self.pool_epoch));
        out.push_str(&format!("wire_hash: {}\n", self.wire_hash));
        if let Some(seq) = self.seq_hint {
            out.push_str(&format!("seq_hint: {seq}\n"));
        }
        out.push_str("wire_b64: ");
        out.push_str(&self.wire_b64);
        out.push('\n');
        out
    }

    pub fn parse_text(text: &str) -> Result<Self> {
        let lines: Vec<&str> = text.lines().collect();
        if lines.first().map(|l| l.trim()) != Some(PIN_MAGIC) {
            return Err(MemError::Wire("missing ITS-MEMORY-PIN/1".into()));
        }
        let mut room_wire_pk = String::new();
        let mut pool_epoch = 0u64;
        let mut wire_hash = String::new();
        let mut seq_hint = None;
        let mut wire_b64 = String::new();
        let mut in_b64 = false;
        for line in &lines[1..] {
            if in_b64 {
                wire_b64.push_str(line.trim());
                continue;
            }
            let line = line.trim();
            if let Some(v) = line.strip_prefix("room_wire_pk:") {
                room_wire_pk = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("pool_epoch:") {
                pool_epoch = v.trim().parse().map_err(|_| MemError::Wire("pool_epoch".into()))?;
            } else if let Some(v) = line.strip_prefix("wire_hash:") {
                wire_hash = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("seq_hint:") {
                seq_hint = Some(v.trim().parse().map_err(|_| MemError::Wire("seq_hint".into()))?);
            } else if let Some(v) = line.strip_prefix("wire_b64:") {
                wire_b64.push_str(v.trim());
                in_b64 = true;
            }
        }
        if room_wire_pk.is_empty() || wire_b64.is_empty() {
            return Err(MemError::Wire("incomplete pin".into()));
        }
        if wire_hash.is_empty() {
            let wire = base64::engine::general_purpose::STANDARD
                .decode(wire_b64.trim())
                .map_err(|e| MemError::Wire(format!("wire_b64: {e}")))?;
            wire_hash = wire_identity(&wire);
        }
        Ok(Self {
            room_wire_pk,
            pool_epoch,
            wire_b64,
            seq_hint,
            wire_hash,
        })
    }

    pub fn write_file(&self, path: &Path) -> Result<()> {
        std::fs::write(path, self.serialize_text())?;
        Ok(())
    }

    pub fn read_file(path: &Path) -> Result<Self> {
        Self::parse_text(&std::fs::read_to_string(path)?)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelCoinManifest {
    pub room_wire_pk: String,
    pub room_id_fp: String,
    /// SSS chain `link_0` hex (ITS backward-underdetermination anchor).
    pub chain_root: String,
    pub frame_count: u64,
    pub last_seq: u64,
    pub last_pool_epoch: u64,
    #[serde(default)]
    pub memory_bytes: u64,
    #[serde(default)]
    pub hosted_seconds: u64,
    #[serde(default = "default_registry_visible")]
    pub registry_visible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quorum_replicas: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_fp: Option<String>,
}

fn default_registry_visible() -> bool {
    true
}

impl ChannelCoinManifest {
    pub fn serialize_text(&self) -> String {
        let magic = if self.is_v2() {
            CHANNEL_COIN_V2_MAGIC
        } else {
            CHANNEL_COIN_V1_MAGIC
        };
        let mut out = String::new();
        out.push_str(magic);
        out.push('\n');
        out.push_str(&format!("room_wire_pk: {}\n", self.room_wire_pk));
        out.push_str(&format!("room_id_fp: {}\n", self.room_id_fp));
        out.push_str(&format!("chain_root: {}\n", self.chain_root));
        out.push_str(&format!("frame_count: {}\n", self.frame_count));
        out.push_str(&format!("last_seq: {}\n", self.last_seq));
        out.push_str(&format!("last_pool_epoch: {}\n", self.last_pool_epoch));
        if self.is_v2() {
            out.push_str(&format!("memory_bytes: {}\n", self.memory_bytes));
            out.push_str(&format!("hosted_seconds: {}\n", self.hosted_seconds));
            out.push_str(&format!(
                "registry_visible: {}\n",
                if self.registry_visible { "true" } else { "false" }
            ));
        }
        if let Some(q) = self.quorum_replicas {
            out.push_str(&format!("quorum_replicas: {q}\n"));
        }
        if let Some(ref fp) = self.host_fp {
            out.push_str(&format!("host_fp: {fp}\n"));
        }
        out
    }

    fn is_v2(&self) -> bool {
        self.memory_bytes > 0
            || self.hosted_seconds > 0
            || !self.registry_visible
            || self.host_fp.is_some()
    }

    pub fn parse_text(text: &str) -> Result<Self> {
        let lines: Vec<&str> = text.lines().collect();
        let magic = lines.first().map(|l| l.trim()).unwrap_or("");
        if magic != CHANNEL_COIN_V1_MAGIC && magic != CHANNEL_COIN_V2_MAGIC {
            return Err(MemError::Coin(format!(
                "missing {CHANNEL_COIN_V1_MAGIC} or {CHANNEL_COIN_V2_MAGIC}"
            )));
        }
        let mut room_wire_pk = String::new();
        let mut room_id_fp = String::new();
        let mut chain_root = String::new();
        let mut frame_count = 0u64;
        let mut last_seq = 0u64;
        let mut last_pool_epoch = 0u64;
        let mut memory_bytes = 0u64;
        let mut hosted_seconds = 0u64;
        let mut registry_visible = true;
        let mut quorum_replicas = None;
        let mut host_fp = None;
        for line in &lines[1..] {
            let line = line.trim();
            if let Some(v) = line.strip_prefix("room_wire_pk:") {
                room_wire_pk = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("room_id_fp:") {
                room_id_fp = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("chain_root:") {
                chain_root = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("frame_count:") {
                frame_count = v.trim().parse().map_err(|_| MemError::Coin("frame_count".into()))?;
            } else if let Some(v) = line.strip_prefix("last_seq:") {
                last_seq = v.trim().parse().map_err(|_| MemError::Coin("last_seq".into()))?;
            } else if let Some(v) = line.strip_prefix("last_pool_epoch:") {
                last_pool_epoch = v
                    .trim()
                    .parse()
                    .map_err(|_| MemError::Coin("last_pool_epoch".into()))?;
            } else if let Some(v) = line.strip_prefix("memory_bytes:") {
                memory_bytes = v
                    .trim()
                    .parse()
                    .map_err(|_| MemError::Coin("memory_bytes".into()))?;
            } else if let Some(v) = line.strip_prefix("hosted_seconds:") {
                hosted_seconds = v
                    .trim()
                    .parse()
                    .map_err(|_| MemError::Coin("hosted_seconds".into()))?;
            } else if let Some(v) = line.strip_prefix("registry_visible:") {
                registry_visible = matches!(v.trim(), "true" | "1" | "yes");
            } else if let Some(v) = line.strip_prefix("quorum_replicas:") {
                quorum_replicas = Some(v.trim().parse().map_err(|_| MemError::Coin("quorum".into()))?);
            } else if let Some(v) = line.strip_prefix("host_fp:") {
                host_fp = Some(v.trim().to_string());
            }
        }
        if room_wire_pk.is_empty() || chain_root.is_empty() {
            return Err(MemError::Coin("incomplete manifest".into()));
        }
        Ok(Self {
            room_wire_pk,
            room_id_fp,
            chain_root,
            frame_count,
            last_seq,
            last_pool_epoch,
            memory_bytes,
            hosted_seconds,
            registry_visible,
            quorum_replicas,
            host_fp,
        })
    }

    pub fn write_file(&self, path: &Path) -> Result<()> {
        std::fs::write(path, self.serialize_text())?;
        Ok(())
    }

    pub fn read_file(path: &Path) -> Result<Self> {
        Self::parse_text(&std::fs::read_to_string(path)?)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GdirOp {
    Mirror,
    Sync,
    Route,
}

impl GdirOp {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "mirror" => Ok(Self::Mirror),
            "sync" => Ok(Self::Sync),
            "route" => Ok(Self::Route),
            _ => Err(MemError::Coin(format!(
                "unknown gdir op: {s} (mirror|sync|route)"
            ))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mirror => "mirror",
            Self::Sync => "sync",
            Self::Route => "route",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GdirReceipt {
    pub contrib_fp: String,
    pub epoch: u64,
    pub op: String,
    pub byte_span: u64,
}

impl GdirReceipt {
    pub fn serialize_text(&self) -> String {
        format!(
            "{GDIR_RECEIPT_MAGIC}\ncontrib_fp: {}\nepoch: {}\nop: {}\nbyte_span: {}\n",
            self.contrib_fp, self.epoch, self.op, self.byte_span
        )
    }

    pub fn parse_text(text: &str) -> Result<Self> {
        let lines: Vec<&str> = text.lines().collect();
        if lines.first().map(|l| l.trim()) != Some(GDIR_RECEIPT_MAGIC) {
            return Err(MemError::Coin(format!("missing {GDIR_RECEIPT_MAGIC}")));
        }
        let mut contrib_fp = String::new();
        let mut epoch = 0u64;
        let mut op = String::new();
        let mut byte_span = 0u64;
        for line in &lines[1..] {
            let line = line.trim();
            if let Some(v) = line.strip_prefix("contrib_fp:") {
                contrib_fp = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("epoch:") {
                epoch = v.trim().parse().map_err(|_| MemError::Coin("epoch".into()))?;
            } else if let Some(v) = line.strip_prefix("op:") {
                op = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("byte_span:") {
                byte_span = v.trim().parse().map_err(|_| MemError::Coin("byte_span".into()))?;
            }
        }
        if contrib_fp.is_empty() || op.is_empty() {
            return Err(MemError::Coin("incomplete gdir receipt".into()));
        }
        Ok(Self {
            contrib_fp,
            epoch,
            op,
            byte_span,
        })
    }

    pub fn payload_bytes(&self) -> Vec<u8> {
        format!("{}|{}|{}|{}", self.contrib_fp, self.epoch, self.op, self.byte_span).into_bytes()
    }

    pub fn write_file(&self, path: &Path) -> Result<()> {
        std::fs::write(path, self.serialize_text())?;
        Ok(())
    }

    pub fn read_file(path: &Path) -> Result<Self> {
        Self::parse_text(&std::fs::read_to_string(path)?)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GdirCoinManifest {
    pub contrib_fp: String,
    pub chain_root: String,
    pub contrib_ops: u64,
    pub contrib_bytes: u64,
    pub contrib_seconds: u64,
}

impl GdirCoinManifest {
    pub fn serialize_text(&self) -> String {
        format!(
            "{GDIR_COIN_MAGIC}\ncontrib_fp: {}\nchain_root: {}\ncontrib_ops: {}\ncontrib_bytes: {}\ncontrib_seconds: {}\n",
            self.contrib_fp,
            self.chain_root,
            self.contrib_ops,
            self.contrib_bytes,
            self.contrib_seconds
        )
    }

    pub fn parse_text(text: &str) -> Result<Self> {
        let lines: Vec<&str> = text.lines().collect();
        if lines.first().map(|l| l.trim()) != Some(GDIR_COIN_MAGIC) {
            return Err(MemError::Coin(format!("missing {GDIR_COIN_MAGIC}")));
        }
        let mut contrib_fp = String::new();
        let mut chain_root = String::new();
        let mut contrib_ops = 0u64;
        let mut contrib_bytes = 0u64;
        let mut contrib_seconds = 0u64;
        for line in &lines[1..] {
            let line = line.trim();
            if let Some(v) = line.strip_prefix("contrib_fp:") {
                contrib_fp = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("chain_root:") {
                chain_root = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("contrib_ops:") {
                contrib_ops = v.trim().parse().map_err(|_| MemError::Coin("contrib_ops".into()))?;
            } else if let Some(v) = line.strip_prefix("contrib_bytes:") {
                contrib_bytes = v
                    .trim()
                    .parse()
                    .map_err(|_| MemError::Coin("contrib_bytes".into()))?;
            } else if let Some(v) = line.strip_prefix("contrib_seconds:") {
                contrib_seconds = v
                    .trim()
                    .parse()
                    .map_err(|_| MemError::Coin("contrib_seconds".into()))?;
            }
        }
        if contrib_fp.is_empty() || chain_root.is_empty() {
            return Err(MemError::Coin("incomplete gdir coin".into()));
        }
        Ok(Self {
            contrib_fp,
            chain_root,
            contrib_ops,
            contrib_bytes,
            contrib_seconds,
        })
    }

    pub fn write_file(&self, path: &Path) -> Result<()> {
        std::fs::write(path, self.serialize_text())?;
        Ok(())
    }

    pub fn read_file(path: &Path) -> Result<Self> {
        Self::parse_text(&std::fs::read_to_string(path)?)
    }
}

/// Exact ciphertext identity for dedup filenames (hex of raw bytes, not SHA).
pub fn wire_identity(wire_bytes: &[u8]) -> String {
    hex::encode(wire_bytes)
}

/// Short filesystem tag; full equality checked via `wire_identity`.
pub fn wire_filename_tag(wire_bytes: &[u8]) -> String {
    let n = wire_bytes.len();
    let head = hex::encode(&wire_bytes[..8.min(n)]);
    let tail = if n > 8 {
        hex::encode(&wire_bytes[n.saturating_sub(8)..])
    } else {
        String::new()
    };
    format!("{n:08x}_{head}_{tail}")
}

/// Public directory fingerprint: first 16 hex chars of ITS room_id (32 hex).
pub fn room_id_fingerprint(room_id: &str) -> String {
    let trimmed = room_id.trim();
    if trimmed.len() >= 16 {
        trimmed[..16].to_string()
    } else {
        format!("{trimmed:0<16}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_roundtrip() {
        let wire = b"wire";
        let pin = MemoryPin {
            room_wire_pk: "aa".repeat(32),
            pool_epoch: 7,
            wire_b64: base64::engine::general_purpose::STANDARD.encode(wire),
            seq_hint: Some(2),
            wire_hash: wire_identity(wire),
        };
        let parsed = MemoryPin::parse_text(&pin.serialize_text()).unwrap();
        assert_eq!(parsed, pin);
    }

    #[test]
    fn room_id_fp_truncates() {
        assert_eq!(room_id_fingerprint("abcd1234ef567890abcd1234ef567890"), "abcd1234ef567890");
    }
}
