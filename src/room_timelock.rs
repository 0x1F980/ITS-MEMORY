use std::path::Path;

use crate::error::Result;

/// Read timelock unlock pool epoch from ITS-CHAT `room.toml` when timelock is configured.
pub fn timelock_unlock_pool_epoch(room_dir: &Path) -> Result<Option<u64>> {
    let toml_path = room_dir.join("room.toml");
    if !toml_path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&toml_path)?;
    let mut has_bundle = false;
    let mut unlock_after = None;
    let mut valid_from = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("timelock_bundle") {
            has_bundle = true;
        } else if let Some(v) = line.strip_prefix("timelock_unlock_after_epochs:") {
            unlock_after = v.trim().parse().ok();
        } else if let Some(v) = line.strip_prefix("timelock_unlock_after_epochs =") {
            unlock_after = v.trim().parse().ok();
        } else if let Some(v) = line.strip_prefix("valid_from_epoch:") {
            valid_from = v.trim().parse().ok();
        } else if let Some(v) = line.strip_prefix("valid_from_epoch =") {
            valid_from = v.trim().parse().ok();
        }
    }
    if !has_bundle {
        return Ok(None);
    }
    let after: u64 = unlock_after.unwrap_or(0);
    let base: u64 = valid_from.unwrap_or(0);
    Ok(Some(base.saturating_add(after)))
}

pub fn room_dir_from_decrypt_pk(pk: &Path) -> Option<std::path::PathBuf> {
    pk.parent().map(|p| p.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timelock_unlock_from_room_toml() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("room.toml"),
            r#"timelock_bundle: /path/to/bundle.its
timelock_unlock_after_epochs: 50
valid_from_epoch: 100
"#,
        )
        .unwrap();
        assert_eq!(
            timelock_unlock_pool_epoch(tmp.path()).unwrap(),
            Some(150)
        );
    }

    #[test]
    fn no_timelock_without_bundle() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("room.toml"),
            "timelock_unlock_after_epochs: 50\nvalid_from_epoch: 100\n",
        )
        .unwrap();
        assert_eq!(timelock_unlock_pool_epoch(tmp.path()).unwrap(), None);
    }
}
