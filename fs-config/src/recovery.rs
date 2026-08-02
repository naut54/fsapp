//! §6.5 invalid-JSON recovery flow, shared by `fsapp` and `fset` — both
//! route a JSON parse failure and a schema validation failure through the
//! same two-option prompt, so this lives here once rather than being
//! duplicated per binary.

use std::io::{IsTerminal, Write};
use std::path::Path;

use crate::error::ConfigError;
use crate::schema::Config;

/// `Ok(config)` on success. `Err(exit_code)` when the caller should abort
/// immediately with that process exit code (3 = unrepaired invalid config,
/// 5 = unrelated I/O failure). `path` is always shown to the user verbatim
/// (§6.5: an explicit `--config <path>` must name that specific path, never
/// the default fsapp path), so callers just pass the already-resolved path.
pub fn load_with_recovery(path: &Path, binary_name: &str) -> Result<Config, i32> {
    match crate::load_config(path) {
        Ok(config) => Ok(config),
        Err(ConfigError::Parse(msg)) | Err(ConfigError::Validation(msg)) => {
            recover(path, binary_name, &msg)
        }
        Err(other) => {
            eprintln!("{binary_name}: error: {other}");
            Err(5)
        }
    }
}

fn recover(path: &Path, binary_name: &str, message: &str) -> Result<Config, i32> {
    let path_display = path.display().to_string();

    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        eprintln!(
            "{binary_name}: error: the config file at {path_display} is invalid:\n  {message}\n\n\
             Not running interactively, so no prompt was shown. To fix this, either:\n  \
             1. Edit {path_display} by hand and re-run, or\n  \
             2. Run `fset reset` to discard it and start from an empty config.\n"
        );
        return Err(3);
    }

    println!("The config file at {path_display} is invalid:");
    println!("  {message}");
    println!();
    println!("What do you want to do?");
    println!("  [1] Exit without touching the file");
    println!("  [2] Reset to default configuration");
    print!("\n> ");
    std::io::stdout().flush().ok();

    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).ok();
    match answer.trim() {
        "2" => {
            if path.exists() {
                if let Err(e) = crate::backup_config(path, "invalid") {
                    eprintln!("{binary_name}: error: {e}");
                    return Err(5);
                }
            }
            let fresh = Config::default();
            if let Err(e) = crate::save_config(&fresh, path) {
                eprintln!("{binary_name}: error: {e}");
                return Err(5);
            }
            Ok(fresh)
        }
        _ => Err(3),
    }
}
