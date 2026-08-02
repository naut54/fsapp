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
