mod backup;
mod enums;
mod error;
mod path;
mod recovery;
mod schema;

pub use backup::backup_config;
pub use enums::{CompressFormat, OnError, SortOrder};
pub use error::ConfigError;
pub use path::{resolve_config_path, update_cache_path};
pub use recovery::load_with_recovery;
pub use schema::{
    Config, CompressSection, CopySection, GlobalSection, MvSection, SyncSection, UpdateSection,
    WatchSection,
};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_config_rejects_malformed_json() {
        let err = parse_config("{not json").unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn parse_config_rejects_a_value_that_fails_schema_validation() {
        let err = parse_config(r#"{"global":{"verbosity":9}}"#).unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
    }

    #[test]
    fn parse_config_accepts_a_valid_partial_config() {
        let config = parse_config(r#"{"sync":{"checksum":true}}"#).unwrap();
        assert_eq!(config.sync.unwrap().checksum, Some(true));
    }

    #[test]
    fn load_config_on_a_missing_file_returns_defaults_without_creating_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");

        let config = load_config(&path).unwrap();

        assert_eq!(config, Config::default());
        assert!(!path.exists(), "fsapp's read-only load path must never touch disk for a missing file");
    }

    #[test]
    fn save_then_load_round_trips_exactly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("config.json");

        let mut config = Config::default();
        config.copy = Some(CopySection { overwrite: Some(true), ..Default::default() });

        save_config(&config, &path).unwrap();
        assert!(path.exists(), "save_config should create missing parent directories");

        let loaded = load_config(&path).unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn load_config_on_an_invalid_file_surfaces_the_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, "{ this is not json").unwrap();

        let err = load_config(&path).unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)));
    }
}
