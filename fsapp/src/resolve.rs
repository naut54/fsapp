//! §6.2 value precedence: CLI flag explicitly passed > `FSAPP_*` env var >
//! `config.json` > file-engine builder default. The env var name for a
//! given `<section>.<key>` isn't pinned down character-by-character
//! anywhere in the spec (only the `FSAPP_CONFIG` path override is named
//! explicitly) — this uses `FSAPP_<SECTION>_<KEY>` (uppercase, `-` -> `_`),
//! e.g. `FSAPP_COPY_ON_ERROR`, `FSAPP_SYNC_NO_OVERWRITE`.

use std::str::FromStr;

/// For `Option<T>`-shaped flags (numbers, enums): CLI already gave `None`
/// when the flag wasn't passed, so this only needs to look further when
/// that's the case.
pub fn resolve<T: FromStr>(cli: Option<T>, section: &str, key: &str, config: Option<T>) -> Option<T> {
    cli.or_else(|| env_var(section, key).and_then(|s| s.parse().ok()))
        .or(config)
}

/// For presence-only boolean CLI flags (`--overwrite`, `--checksum`, ...):
/// `cli_flag == true` is unambiguously "on" and wins outright. Otherwise
/// fall through to env/config, defaulting to `false` — which for every
/// flag in this schema is also the file-engine builder's own default, so
/// "nothing set anywhere" naturally reproduces the builder default without
/// this function needing to know what that default is.
pub fn resolve_bool(cli_flag: bool, section: &str, key: &str, config: Option<bool>) -> bool {
    if cli_flag {
        return true;
    }
    if let Some(v) = env_var(section, key).and_then(|s| s.parse::<bool>().ok()) {
        return v;
    }
    config.unwrap_or(false)
}

fn env_var(section: &str, key: &str) -> Option<String> {
    let name = format!(
        "FSAPP_{}_{}",
        section.to_uppercase(),
        key.to_uppercase().replace('-', "_")
    );
    std::env::var(name).ok()
}
