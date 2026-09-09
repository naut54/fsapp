//! First-run nudge for tab completion, in the same spirit as `update.rs`'s
//! automatic notice: no network involved here (it's a local filesystem
//! check), but the same suppression rules apply — quiet, a non-tty stderr,
//! or `CI` all mean nobody's there to read it — and the same "never affect
//! the operation" discipline: every failure path is "say nothing".
//!
//! Exists because most install channels leave completions inert until the
//! user runs `completions --install` themselves (docs/completions-design.md
//! §2/§4.3) — `.deb` is the only exception. A README line doesn't reach
//! someone who installed via Homebrew or the shell script and never opened
//! it, so this makes the ask visible in the one place they're guaranteed to
//! look: their own terminal.

use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::completions;

/// Checked once per run, synchronously — this is a handful of `stat()`
/// calls against a fixed short list of directories, nothing worth
/// backgrounding the way the network-bound update check is.
pub fn maybe_print(quiet: bool) {
    if !enabled(quiet) {
        return;
    }
    let Some(shell) = completions::detect_shell() else { return };

    let fsapp_missing = !completions::is_installed("fsapp", shell);
    let fset_missing = !completions::is_installed("fset", shell);
    if !fsapp_missing && !fset_missing {
        return;
    }

    let cache_path = match fs_config::completions_notice_cache_path() {
        Ok(p) => p,
        Err(_) => return,
    };
    let shell_key = shell.to_string();
    if read_cache(&cache_path).is_some_and(|c| c.shown_for_shell.as_deref() == Some(&shell_key)) {
        return;
    }

    print_notice(shell, fsapp_missing, fset_missing);
    write_cache(&cache_path, &Cache { shown_for_shell: Some(shell_key) });
}

/// Same triggers as `update::auto_check_enabled`'s "nobody's there to read
/// it" checks — deliberately no config/env toggle of its own: this fires
/// at most once per shell ever (the cache), so there's much less to want
/// to suppress repeatedly than the update check.
fn enabled(quiet: bool) -> bool {
    use std::io::IsTerminal;
    if quiet || !std::io::stderr().is_terminal() {
        return false;
    }
    if std::env::var_os("CI").is_some() {
        return false;
    }
    true
}

fn print_notice(shell: clap_complete::Shell, fsapp_missing: bool, fset_missing: bool) {
    eprintln!();
    eprintln!("{} tab completion for {shell} isn't installed yet", "\u{2139}".cyan());
    if fsapp_missing {
        eprintln!("  fsapp completions --install");
    }
    if fset_missing {
        eprintln!("  fset completions --install");
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Cache {
    shown_for_shell: Option<String>,
}

fn read_cache(path: &std::path::Path) -> Option<Cache> {
    let contents = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

/// Best-effort, same as `update.rs`'s `write_cache`: losing this costs us
/// one repeated notice, never the operation.
fn write_cache(path: &std::path::PathBuf, cache: &Cache) {
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    if let Ok(json) = serde_json::to_string(cache) {
        let _ = std::fs::write(path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_round_trips_through_its_on_disk_form() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("completions-notice.json");
        write_cache(&path, &Cache { shown_for_shell: Some("zsh".to_string()) });
        let read = read_cache(&path).expect("just written");
        assert_eq!(read.shown_for_shell.as_deref(), Some("zsh"));
    }

    #[test]
    fn a_corrupt_cache_file_is_ignored_rather_than_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("completions-notice.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert!(read_cache(&path).is_none());
    }

    #[test]
    fn an_unwritable_cache_location_is_survivable() {
        write_cache(
            &std::path::PathBuf::from("/dev/null/nope/completions-notice.json"),
            &Cache { shown_for_shell: None },
        );
    }
}
