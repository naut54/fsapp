use serde::{Deserialize, Serialize};

use crate::enums::{CompressFormat, OnError, SortOrder};
use crate::error::ConfigError;

/// The full config file, §6.3. Every section is optional; every key within a
/// section is optional; `{}` is a fully valid file meaning "all builder
/// defaults everywhere".
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
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
