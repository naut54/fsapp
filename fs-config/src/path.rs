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

/// Where the update check caches its last result. Deliberately **not**
/// derived from `resolve_config_path`: `--config /tmp/one-off.json` is a
/// per-invocation override of which settings to read, and dropping a cache
/// file next to it (or worse, into whatever directory the user pointed at)
/// isn't what they asked for. The cache is machine state, not
/// configuration, so it tracks the platform config dir only.
///
/// `FSAPP_CACHE_DIR` overrides the directory, for tests and for anyone who
/// wants the cache somewhere writable when the config dir isn't.
pub fn update_cache_path() -> Result<PathBuf, ConfigError> {
    if let Ok(dir) = std::env::var("FSAPP_CACHE_DIR") {
        return Ok(PathBuf::from(dir).join("update-check.json"));
    }
    dirs::config_dir()
        .map(|dir| dir.join("fsapp").join("update-check.json"))
        .ok_or(ConfigError::NoConfigDir)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// `cargo test` runs tests in this module concurrently on threads
    /// sharing one process, but `FSAPP_CONFIG`/`FSAPP_CACHE_DIR` are
    /// process-global state — without this, two tests interleaving their
    /// set/read/remove races and either sees the other's value. Every test
    /// below that touches either var takes this lock for its full
    /// set-read-remove sequence; `.unwrap_or_else(|e| e.into_inner())`
    /// recovers from a poisoned lock left by an earlier test panicking
    /// mid-mutation, since the env var itself still gets cleaned up.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn update_cache_lives_beside_the_config_not_at_the_explicit_override() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: test-only env mutation, restored before returning.
        unsafe { std::env::set_var("FSAPP_CONFIG", "/tmp/somewhere-else/config.json") };
        let cache = update_cache_path();
        unsafe { std::env::remove_var("FSAPP_CONFIG") };
        if let Ok(path) = cache {
            assert!(!path.starts_with("/tmp/somewhere-else"));
            assert!(path.ends_with("fsapp/update-check.json") || path.ends_with("fsapp\\update-check.json"));
        }
    }

    #[test]
    fn cache_dir_env_var_overrides_the_platform_directory() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("FSAPP_CACHE_DIR", "/tmp/fsapp-cache-test") };
        let cache = update_cache_path();
        unsafe { std::env::remove_var("FSAPP_CACHE_DIR") };
        assert_eq!(
            cache.unwrap(),
            PathBuf::from("/tmp/fsapp-cache-test/update-check.json")
        );
    }

    #[test]
    fn explicit_path_wins_over_everything() {
        let explicit = PathBuf::from("/tmp/explicit-config.json");
        let resolved = resolve_config_path(Some(explicit.clone())).unwrap();
        assert_eq!(resolved, explicit);
    }

    #[test]
    fn explicit_path_wins_even_when_env_var_is_also_set() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: test-only env mutation, restored before returning; no
        // other test in this crate reads/writes FSAPP_CONFIG.
        unsafe { std::env::set_var("FSAPP_CONFIG", "/tmp/from-env.json") };
        let resolved = resolve_config_path(Some(PathBuf::from("/tmp/explicit.json")));
        unsafe { std::env::remove_var("FSAPP_CONFIG") };
        assert_eq!(resolved.unwrap(), PathBuf::from("/tmp/explicit.json"));
    }

    #[test]
    fn env_var_wins_over_platform_default() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("FSAPP_CONFIG", "/tmp/from-env-only.json") };
        let resolved = resolve_config_path(None);
        unsafe { std::env::remove_var("FSAPP_CONFIG") };
        assert_eq!(resolved.unwrap(), PathBuf::from("/tmp/from-env-only.json"));
    }

    #[test]
    fn falls_back_to_platform_config_dir_ending_in_fsapp_config_json() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::remove_var("FSAPP_CONFIG") };
        let resolved = resolve_config_path(None);
        // `dirs::config_dir()` is platform-provided; only assert the part
        // this crate controls (the `fsapp/config.json` suffix), not the
        // platform-specific prefix.
        if let Ok(path) = resolved {
            assert!(path.ends_with("fsapp/config.json") || path.ends_with("fsapp\\config.json"));
        }
    }
}
