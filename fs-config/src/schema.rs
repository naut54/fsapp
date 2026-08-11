use serde::{Deserialize, Serialize};

use crate::enums::{CompressFormat, OnError, SortOrder};
use crate::error::ConfigError;

/// The full config file, §6.3. Every section is optional; every key within a
/// section is optional; `{}` is a fully valid file meaning "all builder
/// defaults everywhere".
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global: Option<GlobalSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy: Option<CopySection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mv: Option<MvSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync: Option<SyncSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch: Option<WatchSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compress: Option<CompressSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update: Option<UpdateSection>,
}

/// The automatic update check, off-by-absence in the negative form so it
/// resolves through the same `resolve_bool` path as `sync.no-overwrite`
/// and `watch.no-recursive` — "nothing set anywhere" means the check runs,
/// and `FSAPP_UPDATE_NO_CHECK=true` disables it without a bespoke env var.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct UpdateSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_check: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct GlobalSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbosity: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quiet: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CopySection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_error: Option<OnError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_file_threshold: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_concurrency: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preserve_permissions: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_fs_integrity_risk: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overwrite: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes_per_batch: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_files_per_batch: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<SortOrder>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct MvSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_error: Option<OnError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_file_threshold: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_concurrency: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preserve_permissions: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_fs_integrity_risk: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overwrite: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SyncSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_error: Option<OnError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_file_threshold: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_concurrency: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preserve_permissions: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_fs_integrity_risk: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_overwrite: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct WatchSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_recursive: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CompressSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_error: Option<OnError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_file_threshold: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_concurrency: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<CompressFormat>,
}

impl Config {
    /// §6.4 validation rules that aren't already enforced by serde/deny_unknown_fields
    /// (enum strings and bool types are enforced at deserialize time via serde;
    /// this covers numeric ranges, which serde's type system alone can't express).
    pub fn validate(&self) -> Result<(), ConfigError> {
        if let Some(g) = &self.global {
            if let Some(v) = g.verbosity {
                if v > 3 {
                    return Err(ConfigError::Validation(format!(
                        "global.verbosity must be 0..=3, got {v}"
                    )));
                }
            }
        }
        macro_rules! check_batch_fields {
            ($section:expr, $name:literal) => {
                if let Some(s) = &$section {
                    check_gt_zero(s.small_file_threshold, concat!($name, ".small-file-threshold"))?;
                    check_gte_one(s.batch_concurrency, concat!($name, ".batch-concurrency"))?;
                }
            };
        }
        check_batch_fields!(self.copy, "copy");
        check_batch_fields!(self.mv, "mv");
        check_batch_fields!(self.sync, "sync");
        check_batch_fields!(self.compress, "compress");

        if let Some(c) = &self.copy {
            check_gt_zero(c.max_bytes_per_batch, "copy.max-bytes-per-batch")?;
            check_gte_one(c.max_files_per_batch, "copy.max-files-per-batch")?;
        }

        Ok(())
    }
}

fn check_gt_zero(value: Option<u64>, key: &str) -> Result<(), ConfigError> {
    if let Some(v) = value {
        if v == 0 {
            return Err(ConfigError::Validation(format!(
                "{key} must be > 0, got 0"
            )));
        }
    }
    Ok(())
}

fn check_gte_one(value: Option<u64>, key: &str) -> Result<(), ConfigError> {
    if let Some(v) = value {
        if v < 1 {
            return Err(ConfigError::Validation(format!(
                "{key} must be >= 1, got {v}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_object_is_a_valid_config_meaning_all_defaults() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn only_touched_keys_round_trip_through_json() {
        let json = r#"{"copy":{"on-error":"abort","overwrite":true}}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.copy.as_ref().unwrap().on_error, Some(OnError::Abort));
        assert_eq!(config.copy.as_ref().unwrap().overwrite, Some(true));
        assert_eq!(config.copy.as_ref().unwrap().small_file_threshold, None);

        // Serializing back out must omit untouched keys and sections, not
        // dump the whole struct (§6.3).
        let round_tripped = serde_json::to_string(&config).unwrap();
        assert_eq!(round_tripped, json);
    }

    #[test]
    fn unknown_key_in_a_known_section_is_a_validation_error() {
        let err = serde_json::from_str::<Config>(r#"{"copy":{"on-eror":"abort"}}"#).unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn unknown_section_name_is_a_validation_error() {
        // §6.4: a typo must surface, never be silently swallowed — this
        // applies to a mistyped *section* name too, not just a mistyped
        // key within a known section.
        let err = serde_json::from_str::<Config>(r#"{"coppy":{"overwrite":true}}"#).unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn verbosity_in_range_is_valid() {
        let config = Config {
            global: Some(GlobalSection { verbosity: Some(3), quiet: None }),
            ..Config::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn verbosity_above_three_is_invalid() {
        let config = Config {
            global: Some(GlobalSection { verbosity: Some(4), quiet: None }),
            ..Config::default()
        };
        let err = config.validate().unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
        assert!(err.to_string().contains("global.verbosity"));
    }

    #[test]
    fn small_file_threshold_of_zero_is_invalid_in_every_section_that_has_it() {
        let mut config = Config {
            copy: Some(CopySection { small_file_threshold: Some(0), ..Default::default() }),
            ..Config::default()
        };
        assert!(config.validate().is_err());

        config = Config {
            mv: Some(MvSection { small_file_threshold: Some(0), ..Default::default() }),
            ..Config::default()
        };
        assert!(config.validate().is_err());

        config = Config {
            sync: Some(SyncSection { small_file_threshold: Some(0), ..Default::default() }),
            ..Config::default()
        };
        assert!(config.validate().is_err());

        config = Config {
            compress: Some(CompressSection { small_file_threshold: Some(0), ..Default::default() }),
            ..Config::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn batch_concurrency_of_zero_is_invalid() {
        let config = Config {
            copy: Some(CopySection { batch_concurrency: Some(0), ..Default::default() }),
            ..Config::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn max_bytes_per_batch_of_zero_is_invalid_copy_only() {
        let config = Config {
            copy: Some(CopySection { max_bytes_per_batch: Some(0), ..Default::default() }),
            ..Config::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn max_files_per_batch_of_zero_is_invalid_copy_only() {
        let config = Config {
            copy: Some(CopySection { max_files_per_batch: Some(0), ..Default::default() }),
            ..Config::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn positive_batch_values_are_valid() {
        let config = Config {
            copy: Some(CopySection {
                small_file_threshold: Some(1024),
                batch_concurrency: Some(4),
                max_bytes_per_batch: Some(8 * 1024 * 1024),
                max_files_per_batch: Some(100),
                ..Default::default()
            }),
            ..Config::default()
        };
        assert!(config.validate().is_ok());
    }
}
