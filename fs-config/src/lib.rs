mod backup;
mod enums;
mod error;
mod path;
mod recovery;
mod schema;

pub use backup::backup_config;
pub use enums::{CompressFormat, OnError, SortOrder};
pub use error::ConfigError;
pub use path::resolve_config_path;
pub use recovery::load_with_recovery;
pub use schema::{Config, CompressSection, CopySection, GlobalSection, MvSection, SyncSection, WatchSection};

use std::path::{Path, PathBuf};

/// Parses and validates `contents` as a config file. Both a JSON parse
/// failure and a schema validation failure (unknown field, wrong type,
/// out-of-range value, invalid enum string) surface as `ConfigError::Parse`
/// / `ConfigError::Validation` respectively — callers route both into the
/// same §6.5 recovery flow, so the distinction only matters for the message
/// shown to the user.
pub fn parse_config(contents: &str) -> Result<Config, ConfigError> {
    let config: Config =
        serde_json::from_str(contents).map_err(|e| ConfigError::Parse(e.to_string()))?;
    config.validate()?;
    Ok(config)
}

/// Loads the config at `path`. A missing file is not an error — it means
/// "all builder defaults everywhere" (`Config::default()`), and — per §6.1 —
/// fsapp in read-only mode must never touch disk to create it.
pub fn load_config(path: &Path) -> Result<Config, ConfigError> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let contents = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    parse_config(&contents)
}

/// Serializes and writes `config` to `path`, creating the parent directory
/// if needed (only called from `fset set`/`fset reset` — see §6.1, `fsapp`
/// read paths never call this).
pub fn save_config(config: &Config, path: &Path) -> Result<(), ConfigError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|source| ConfigError::CreateDir {
            path: dir.to_path_buf(),
            source,
        })?;
    }
    let contents = serde_json::to_string_pretty(config)
        .expect("Config serialization is infallible for valid data");
    std::fs::write(path, contents + "\n").map_err(|source| ConfigError::Write {
        path: path.to_path_buf(),
        source,
    })
}

/// Convenience: the platform-default config path with no `--config`/env
/// override, for `fset path` and similar diagnostics.
pub fn default_config_path() -> Result<PathBuf, ConfigError> {
    resolve_config_path(None)
}
