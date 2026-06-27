use std::path::Path;

use crate::error::{MemError, Result};

/// Maps pool `pool_epoch` to approximate unix time via `epoch_interval_ms`.
#[derive(Clone, Debug)]
pub struct EpochMap {
    pub interval_ms: u64,
    pub epoch_zero_unix: u64,
}

impl EpochMap {
    pub fn new(interval_ms: u64, epoch_zero_unix: u64) -> Self {
        Self {
            interval_ms: interval_ms.max(1),
            epoch_zero_unix,
        }
    }

    pub fn from_env_and_options(
        routing_config: Option<&Path>,
        epoch_interval_ms: Option<u64>,
    ) -> Result<Self> {
        let interval = epoch_interval_ms
            .or_else(|| std::env::var("ITS_EPOCH_INTERVAL_MS").ok()?.parse().ok())
            .or_else(|| routing_config.and_then(parse_routing_epoch_interval_ms))
            .unwrap_or(100);
        let epoch_zero = std::env::var("ITS_EPOCH_ZERO_UNIX")
            .ok()
            .and_then(|s| s.parse().ok())
            .or_else(|| routing_config.and_then(parse_routing_epoch_zero_unix))
            .unwrap_or(0);
        Ok(Self::new(interval, epoch_zero))
    }

    pub fn pool_epoch_to_unix(&self, pool_epoch: u64) -> u64 {
        self.epoch_zero_unix
            .saturating_add(pool_epoch.saturating_mul(self.interval_ms) / 1000)
    }

    pub fn unix_to_pool_epoch(&self, unix_secs: u64) -> u64 {
        if unix_secs <= self.epoch_zero_unix {
            return 0;
        }
        let delta_ms = (unix_secs - self.epoch_zero_unix).saturating_mul(1000);
        delta_ms / self.interval_ms
    }

    pub fn parse_date_to_unix(date: &str) -> Result<u64> {
        let trimmed = date.trim();
        if trimmed.chars().all(|c| c.is_ascii_digit()) {
            return trimmed
                .parse()
                .map_err(|_| MemError::Usage(format!("invalid unix timestamp: {date}")));
        }
        parse_ymd(trimmed)
    }
}

fn parse_routing_epoch_interval_ms(path: &Path) -> Option<u64> {
    parse_routing_pool_u64(path, "epoch_interval_ms")
}

fn parse_routing_epoch_zero_unix(path: &Path) -> Option<u64> {
    parse_routing_pool_u64(path, "epoch_zero_unix")
}

fn parse_routing_pool_u64(path: &Path, key: &str) -> Option<u64> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut in_pool = false;
    for line in text.lines() {
        let line = line.split('#').next()?.trim();
        if line.is_empty() {
            continue;
        }
        if line == "[pool]" {
            in_pool = true;
            continue;
        }
        if line.starts_with('[') {
            in_pool = false;
            continue;
        }
        if !in_pool {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                return v.trim().parse().ok();
            }
        }
    }
    None
}

fn parse_ymd(date: &str) -> Result<u64> {
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() != 3 {
        return Err(MemError::Usage(format!(
            "date must be YYYY-MM-DD or unix seconds: {date}"
        )));
    }
    let year: i32 = parts[0]
        .parse()
        .map_err(|_| MemError::Usage(format!("invalid year in date: {date}")))?;
    let month: u32 = parts[1]
        .parse()
        .map_err(|_| MemError::Usage(format!("invalid month in date: {date}")))?;
    let day: u32 = parts[2]
        .parse()
        .map_err(|_| MemError::Usage(format!("invalid day in date: {date}")))?;
    days_since_epoch(year, month, day)
        .ok_or_else(|| MemError::Usage(format!("invalid calendar date: {date}")))
}

fn days_since_epoch(year: i32, month: u32, day: u32) -> Option<u64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let mut y = year as i64;
    let mut m = month as i64;
    if m <= 2 {
        y -= 1;
        m += 12;
    }
    let era = y / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m - 3) + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    if days < 0 {
        return None;
    }
    Some((days as u64).saturating_mul(86_400))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_epoch_map() {
        let map = EpochMap::new(1000, 1_700_000_000);
        let epoch = 42;
        let unix = map.pool_epoch_to_unix(epoch);
        assert_eq!(map.unix_to_pool_epoch(unix), epoch);
    }

    #[test]
    fn parse_routing_interval() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("routing.toml");
        std::fs::write(
            &cfg,
            "[pool]\ntransport_mode = \"pool\"\nepoch_interval_ms = 250\n",
        )
        .unwrap();
        let map = EpochMap::from_env_and_options(Some(&cfg), None).unwrap();
        assert_eq!(map.interval_ms, 250);
    }

    #[test]
    fn parse_ymd_date() {
        let unix = EpochMap::parse_date_to_unix("2024-01-01").unwrap();
        assert_eq!(unix, 1_704_067_200);
    }
}
