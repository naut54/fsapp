use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::ConfigError;

/// §6.6 backup naming. `kind` is `"invalid"` (broken/unparseable file, before
/// an interactive or forced reset) or `"bak"` (valid file, backed up before a
/// manual `fset reset`). Same directory as `config.json`, Unix-seconds-UTC
/// timestamp, no milliseconds. On a same-second collision, appends `-2`,
/// `-3`, ... rather than overwriting — a backup is never destroyed by a
/// naming collision.
pub fn backup_config(config_path: &Path, kind: &str) -> Result<PathBuf, ConfigError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_secs();

    let dir = config_path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = config_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "config.json".to_string());

    let mut candidate = dir.join(format!("{file_name}.{kind}-{timestamp}"));
    let mut suffix = 2;
    while candidate.exists() {
        candidate = dir.join(format!("{file_name}.{kind}-{timestamp}-{suffix}"));
        suffix += 1;
    }

    std::fs::copy(config_path, &candidate).map_err(|source| ConfigError::Backup {
        path: candidate.clone(),
        source,
    })?;

    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_copies_content_and_names_it_after_the_original_and_kind() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        std::fs::write(&config_path, r#"{"copy":{"overwrite":true}}"#).unwrap();

        let backup_path = backup_config(&config_path, "bak").unwrap();

        assert_eq!(backup_path.parent().unwrap(), dir.path());
        let name = backup_path.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with("config.json.bak-"), "unexpected name: {name}");
        let timestamp = name.strip_prefix("config.json.bak-").unwrap();
        assert!(timestamp.parse::<u64>().is_ok(), "timestamp segment should be a plain integer: {timestamp}");

        assert_eq!(std::fs::read_to_string(&backup_path).unwrap(), r#"{"copy":{"overwrite":true}}"#);
        // The original is untouched by taking a backup.
        assert!(config_path.exists());
    }

    #[test]
    fn invalid_kind_produces_an_invalid_prefixed_name() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        std::fs::write(&config_path, "{not valid json").unwrap();

        let backup_path = backup_config(&config_path, "invalid").unwrap();
        let name = backup_path.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with("config.json.invalid-"), "unexpected name: {name}");
    }

    #[test]
    fn repeated_backups_in_the_same_second_never_collide_or_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        std::fs::write(&config_path, "{}").unwrap();

        let first = backup_config(&config_path, "bak").unwrap();
        let second = backup_config(&config_path, "bak").unwrap();
        let third = backup_config(&config_path, "bak").unwrap();

        assert_ne!(first, second);
        assert_ne!(second, third);
        assert_ne!(first, third);
        // All three must still exist — a collision must never overwrite
        // an earlier backup.
        assert!(first.exists());
        assert!(second.exists());
        assert!(third.exists());
    }

    #[test]
    fn missing_source_file_is_an_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("does-not-exist.json");

        let err = backup_config(&config_path, "bak").unwrap_err();
        assert!(matches!(err, ConfigError::Backup { .. }));
    }
}
