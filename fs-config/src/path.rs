use std::path::PathBuf;

use crate::error::ConfigError;

/// §6.1 path resolution, first match wins, no merging across sources:
/// 1. explicit `--config <path>`
/// 2. `FSAPP_CONFIG` env var
/// 3. `dirs::config_dir()` + `/fsapp/config.json` (platform-idiomatic; `dirs`
///    already handles the XDG_CONFIG_HOME / ~/.config fallback on Linux and
///    the macOS/Windows equivalents)
/// 4. `dirs::config_dir()` returning `None` is a fatal `ConfigError::NoConfigDir`
pub fn resolve_config_path(explicit: Option<PathBuf>) -> Result<PathBuf, ConfigError> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    if let Ok(path) = std::env::var("FSAPP_CONFIG") {
        return Ok(PathBuf::from(path));
    }
    dirs::config_dir()
        .map(|dir| dir.join("fsapp").join("config.json"))
        .ok_or(ConfigError::NoConfigDir)
}
