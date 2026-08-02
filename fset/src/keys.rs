//! Generic `<section>.<key>` addressing for `fset get/set/unset`, matching
//! the schema in fs-config's `Config` 1:1 (fsapp-design-spec.md §6.3/§6.4).
//! This is the single call site that knows how each key's string
//! representation maps to its typed field — everything else in `fset`
//! operates on `Config` values only.

use clap::ValueEnum;
use fs_config::{CompressFormat, Config, OnError, SortOrder};

pub fn get(config: &Config, section: &str, key: &str) -> Result<Option<String>, String> {
    macro_rules! opt {
        ($section_field:expr, $key_field:ident) => {
            $section_field
                .as_ref()
                .and_then(|s| s.$key_field.as_ref())
                .map(|v| display(v))
        };
    }
    let value = match (section, key) {
        ("global", "verbosity") => opt!(config.global, verbosity),
        ("global", "quiet") => opt!(config.global, quiet),

        ("copy", "on-error") => opt!(config.copy, on_error),
        ("copy", "small-file-threshold") => opt!(config.copy, small_file_threshold),
        ("copy", "batch-concurrency") => opt!(config.copy, batch_concurrency),
        ("copy", "preserve-permissions") => opt!(config.copy, preserve_permissions),
        ("copy", "allow-fs-integrity-risk") => opt!(config.copy, allow_fs_integrity_risk),
        ("copy", "overwrite") => opt!(config.copy, overwrite),
        ("copy", "max-bytes-per-batch") => opt!(config.copy, max_bytes_per_batch),
        ("copy", "max-files-per-batch") => opt!(config.copy, max_files_per_batch),
        ("copy", "sort-order") => opt!(config.copy, sort_order),

        ("mv", "on-error") => opt!(config.mv, on_error),
        ("mv", "small-file-threshold") => opt!(config.mv, small_file_threshold),
        ("mv", "batch-concurrency") => opt!(config.mv, batch_concurrency),
        ("mv", "preserve-permissions") => opt!(config.mv, preserve_permissions),
        ("mv", "allow-fs-integrity-risk") => opt!(config.mv, allow_fs_integrity_risk),
        ("mv", "overwrite") => opt!(config.mv, overwrite),

        ("sync", "on-error") => opt!(config.sync, on_error),
        ("sync", "small-file-threshold") => opt!(config.sync, small_file_threshold),
        ("sync", "batch-concurrency") => opt!(config.sync, batch_concurrency),
        ("sync", "preserve-permissions") => opt!(config.sync, preserve_permissions),
        ("sync", "allow-fs-integrity-risk") => opt!(config.sync, allow_fs_integrity_risk),
        ("sync", "no-overwrite") => opt!(config.sync, no_overwrite),
        ("sync", "checksum") => opt!(config.sync, checksum),

        ("watch", "no-recursive") => opt!(config.watch, no_recursive),

        ("compress", "on-error") => opt!(config.compress, on_error),
        ("compress", "small-file-threshold") => opt!(config.compress, small_file_threshold),
        ("compress", "batch-concurrency") => opt!(config.compress, batch_concurrency),
        ("compress", "format") => opt!(config.compress, format),

        _ => return Err(unknown_key(section, key)),
    };
    Ok(value)
}

pub fn set(config: &mut Config, section: &str, key: &str, value: &str) -> Result<(), String> {
    macro_rules! set_field {
        ($section_field:expr, $key_field:ident, $parse:expr) => {{
            let parsed = $parse(value)?;
            $section_field.get_or_insert_with(Default::default).$key_field = Some(parsed);
        }};
    }
    match (section, key) {
        ("global", "verbosity") => set_field!(config.global, verbosity, parse_u8),
        ("global", "quiet") => set_field!(config.global, quiet, parse_bool),

        ("copy", "on-error") => set_field!(config.copy, on_error, parse_enum::<OnError>),
        ("copy", "small-file-threshold") => set_field!(config.copy, small_file_threshold, parse_u64),
        ("copy", "batch-concurrency") => set_field!(config.copy, batch_concurrency, parse_u64),
        ("copy", "preserve-permissions") => set_field!(config.copy, preserve_permissions, parse_bool),
        ("copy", "allow-fs-integrity-risk") => {
            set_field!(config.copy, allow_fs_integrity_risk, parse_bool)
        }
        ("copy", "overwrite") => set_field!(config.copy, overwrite, parse_bool),
        ("copy", "max-bytes-per-batch") => set_field!(config.copy, max_bytes_per_batch, parse_u64),
        ("copy", "max-files-per-batch") => set_field!(config.copy, max_files_per_batch, parse_u64),
        ("copy", "sort-order") => set_field!(config.copy, sort_order, parse_enum::<SortOrder>),

        ("mv", "on-error") => set_field!(config.mv, on_error, parse_enum::<OnError>),
        ("mv", "small-file-threshold") => set_field!(config.mv, small_file_threshold, parse_u64),
        ("mv", "batch-concurrency") => set_field!(config.mv, batch_concurrency, parse_u64),
        ("mv", "preserve-permissions") => set_field!(config.mv, preserve_permissions, parse_bool),
        ("mv", "allow-fs-integrity-risk") => {
            set_field!(config.mv, allow_fs_integrity_risk, parse_bool)
        }
        ("mv", "overwrite") => set_field!(config.mv, overwrite, parse_bool),

        ("sync", "on-error") => set_field!(config.sync, on_error, parse_enum::<OnError>),
        ("sync", "small-file-threshold") => set_field!(config.sync, small_file_threshold, parse_u64),
        ("sync", "batch-concurrency") => set_field!(config.sync, batch_concurrency, parse_u64),
        ("sync", "preserve-permissions") => set_field!(config.sync, preserve_permissions, parse_bool),
        ("sync", "allow-fs-integrity-risk") => {
            set_field!(config.sync, allow_fs_integrity_risk, parse_bool)
        }
        ("sync", "no-overwrite") => set_field!(config.sync, no_overwrite, parse_bool),
        ("sync", "checksum") => set_field!(config.sync, checksum, parse_bool),

        ("watch", "no-recursive") => set_field!(config.watch, no_recursive, parse_bool),

        ("compress", "on-error") => set_field!(config.compress, on_error, parse_enum::<OnError>),
        ("compress", "small-file-threshold") => {
            set_field!(config.compress, small_file_threshold, parse_u64)
        }
        ("compress", "batch-concurrency") => set_field!(config.compress, batch_concurrency, parse_u64),
        ("compress", "format") => set_field!(config.compress, format, parse_enum::<CompressFormat>),

        _ => return Err(unknown_key(section, key)),
    }
    Ok(())
}

pub fn unset(config: &mut Config, section: &str, key: &str) -> Result<(), String> {
    macro_rules! clear_field {
        ($section_field:expr, $key_field:ident) => {
            if let Some(s) = $section_field.as_mut() {
                s.$key_field = None;
            }
        };
    }
    match (section, key) {
        ("global", "verbosity") => clear_field!(config.global, verbosity),
        ("global", "quiet") => clear_field!(config.global, quiet),

        ("copy", "on-error") => clear_field!(config.copy, on_error),
        ("copy", "small-file-threshold") => clear_field!(config.copy, small_file_threshold),
        ("copy", "batch-concurrency") => clear_field!(config.copy, batch_concurrency),
        ("copy", "preserve-permissions") => clear_field!(config.copy, preserve_permissions),
        ("copy", "allow-fs-integrity-risk") => clear_field!(config.copy, allow_fs_integrity_risk),
        ("copy", "overwrite") => clear_field!(config.copy, overwrite),
        ("copy", "max-bytes-per-batch") => clear_field!(config.copy, max_bytes_per_batch),
        ("copy", "max-files-per-batch") => clear_field!(config.copy, max_files_per_batch),
        ("copy", "sort-order") => clear_field!(config.copy, sort_order),

        ("mv", "on-error") => clear_field!(config.mv, on_error),
        ("mv", "small-file-threshold") => clear_field!(config.mv, small_file_threshold),
        ("mv", "batch-concurrency") => clear_field!(config.mv, batch_concurrency),
        ("mv", "preserve-permissions") => clear_field!(config.mv, preserve_permissions),
        ("mv", "allow-fs-integrity-risk") => clear_field!(config.mv, allow_fs_integrity_risk),
        ("mv", "overwrite") => clear_field!(config.mv, overwrite),

        ("sync", "on-error") => clear_field!(config.sync, on_error),
        ("sync", "small-file-threshold") => clear_field!(config.sync, small_file_threshold),
        ("sync", "batch-concurrency") => clear_field!(config.sync, batch_concurrency),
        ("sync", "preserve-permissions") => clear_field!(config.sync, preserve_permissions),
        ("sync", "allow-fs-integrity-risk") => clear_field!(config.sync, allow_fs_integrity_risk),
        ("sync", "no-overwrite") => clear_field!(config.sync, no_overwrite),
        ("sync", "checksum") => clear_field!(config.sync, checksum),

        ("watch", "no-recursive") => clear_field!(config.watch, no_recursive),

        ("compress", "on-error") => clear_field!(config.compress, on_error),
        ("compress", "small-file-threshold") => clear_field!(config.compress, small_file_threshold),
        ("compress", "batch-concurrency") => clear_field!(config.compress, batch_concurrency),
        ("compress", "format") => clear_field!(config.compress, format),

        _ => return Err(unknown_key(section, key)),
    }
    Ok(())
}

fn unknown_key(section: &str, key: &str) -> String {
    format!("unknown config key \"{section}.{key}\"")
}

fn display<T: serde::Serialize>(value: &T) -> String {
    match serde_json::to_value(value).expect("config field values always serialize") {
        serde_json::Value::String(s) => s,
        other => other.to_string(),
    }
}

fn parse_bool(input: &str) -> Result<bool, String> {
    match input {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!("expected \"true\" or \"false\", got \"{other}\"")),
    }
}

fn parse_u8(input: &str) -> Result<u8, String> {
    input
        .parse::<u8>()
        .map_err(|_| format!("expected an integer 0..=255, got \"{input}\""))
}

fn parse_u64(input: &str) -> Result<u64, String> {
    input
        .parse::<u64>()
        .map_err(|_| format!("expected a non-negative integer, got \"{input}\""))
}

fn parse_enum<T: ValueEnum>(input: &str) -> Result<T, String> {
    T::from_str(input, true).map_err(|_| {
        let variants: Vec<String> = T::value_variants()
            .iter()
            .filter_map(|v| v.to_possible_value())
            .map(|pv| pv.get_name().to_string())
            .collect();
        format!("expected one of [{}], got \"{input}\"", variants.join(", "))
    })
}
