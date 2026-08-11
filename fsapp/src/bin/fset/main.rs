mod keys;

/// The same module `fsapp` uses. `#[path]` rather than `use` because the
/// two are separate `[[bin]]` targets of one package, so neither can reach
/// the other's modules — see its own docs.
#[path = "../../completions.rs"]
mod completions;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use clap_complete::Shell;
use fs_config::Config;

#[derive(Parser)]
#[command(name = "fset", version, about = "Read/write fsapp's shared JSON config file")]
struct Cli {
    /// Override the config file location for this invocation.
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the current value, or "unset" if absent.
    Get { key: String },
    /// Set a value, validated against the same types fsapp's CLI uses.
    Set { key: String, value: String },
    /// Remove a key — reverts to the file-engine builder default.
    Unset { key: String },
    /// Dump current JSON (optionally scoped to one section).
    List { section: Option<String> },
    /// Print the resolved config file path.
    Path,
    /// Open $EDITOR on the file; re-validate before saving.
    Edit,
    /// Reset the whole file (or one section) to {}; always backs up first.
    Reset { section: Option<String> },
    /// Print a shell completion script, or install it with --install.
    Completions {
        /// Detected from $SHELL when omitted.
        shell: Option<Shell>,
        /// Write the script into the shell's completion directory.
        #[arg(long)]
        install: bool,
        /// Install into this directory instead of searching. Implies --install.
        #[arg(long)]
        dir: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // Handled before config resolution: printing a completion script must
    // not fail because the config directory is unavailable.
    if let Command::Completions { shell, install, dir } = cli.command {
        use clap::CommandFactory;
        return ExitCode::from(completions::run(&mut Cli::command(), "fset", shell, install, dir));
    }

    let config_path = match fs_config::resolve_config_path(cli.config.clone()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("fset: error: {e}");
            return ExitCode::from(5);
        }
    };

    let code = match cli.command {
        Command::Get { key } => run_get(&config_path, &key),
        Command::Set { key, value } => run_set(&config_path, &key, &value),
        Command::Unset { key } => run_unset(&config_path, &key),
        Command::List { section } => run_list(&config_path, section.as_deref()),
        Command::Path => {
            println!("{}", config_path.display());
            0
        }
        Command::Edit => run_edit(&config_path),
        Command::Reset { section } => run_reset(&config_path, section.as_deref()),
        Command::Completions { .. } => unreachable!("handled above"),
    };

    ExitCode::from(code as u8)
}

fn split_key(key: &str) -> Result<(&str, &str), i32> {
    match key.split_once('.') {
        Some((section, k)) if !section.is_empty() && !k.is_empty() => Ok((section, k)),
        _ => {
            eprintln!("fset: error: expected \"<section>.<key>\", got \"{key}\"");
            Err(2)
        }
    }
}

fn run_get(config_path: &std::path::Path, key: &str) -> i32 {
    let (section, k) = match split_key(key) {
        Ok(pair) => pair,
        Err(code) => return code,
    };
    let config = match fs_config::load_with_recovery(config_path, "fset") {
        Ok(c) => c,
        Err(code) => return code,
    };
    match keys::get(&config, section, k) {
        Ok(Some(value)) => {
            println!("{value}");
            0
        }
        Ok(None) => {
            println!("unset");
            0
        }
        Err(e) => {
            eprintln!("fset: error: {e}");
            2
        }
    }
}

fn run_set(config_path: &std::path::Path, key: &str, value: &str) -> i32 {
    let (section, k) = match split_key(key) {
        Ok(pair) => pair,
        Err(code) => return code,
    };
    let mut config = match fs_config::load_with_recovery(config_path, "fset") {
        Ok(c) => c,
        Err(code) => return code,
    };
    if let Err(e) = keys::set(&mut config, section, k, value) {
        eprintln!("fset: error: {e}");
        return 2;
    }
    if let Err(e) = config.validate() {
        eprintln!("fset: error: {e}");
        return 2;
    }
    if let Err(e) = fs_config::save_config(&config, config_path) {
        eprintln!("fset: error: {e}");
        return 5;
    }
    0
}

fn run_unset(config_path: &std::path::Path, key: &str) -> i32 {
    let (section, k) = match split_key(key) {
        Ok(pair) => pair,
        Err(code) => return code,
    };
    let mut config = match fs_config::load_with_recovery(config_path, "fset") {
        Ok(c) => c,
        Err(code) => return code,
    };
    if let Err(e) = keys::unset(&mut config, section, k) {
        eprintln!("fset: error: {e}");
        return 2;
    }
    if let Err(e) = fs_config::save_config(&config, config_path) {
        eprintln!("fset: error: {e}");
        return 5;
    }
    0
}

fn run_list(config_path: &std::path::Path, section: Option<&str>) -> i32 {
    let config = match fs_config::load_with_recovery(config_path, "fset") {
        Ok(c) => c,
        Err(code) => return code,
    };
    let value = match section {
        None => serde_json::to_value(&config).unwrap(),
        Some(s) => {
            let whole = serde_json::to_value(&config).unwrap();
            match whole.get(s) {
                Some(v) => v.clone(),
                None if is_known_section(s) => serde_json::json!({}),
                None => {
                    eprintln!("fset: error: unknown config section \"{s}\"");
                    return 2;
                }
            }
        }
    };
    println!("{}", serde_json::to_string_pretty(&value).unwrap());
    0
}

fn is_known_section(section: &str) -> bool {
    matches!(
        section,
        "global" | "copy" | "mv" | "sync" | "watch" | "compress"
    )
}

fn run_reset(config_path: &std::path::Path, section: Option<&str>) -> i32 {
    match section {
        None => {
            if config_path.exists() {
                if let Err(e) = fs_config::backup_config(config_path, "bak") {
                    eprintln!("fset: error: {e}");
                    return 5;
                }
            }
            if let Err(e) = fs_config::save_config(&Config::default(), config_path) {
                eprintln!("fset: error: {e}");
                return 5;
            }
            0
        }
        Some(s) => {
            if !is_known_section(s) {
                eprintln!("fset: error: unknown config section \"{s}\"");
                return 2;
            }
            let mut config = match fs_config::load_with_recovery(config_path, "fset") {
                Ok(c) => c,
                Err(code) => return code,
            };
            if config_path.exists() {
                if let Err(e) = fs_config::backup_config(config_path, "bak") {
                    eprintln!("fset: error: {e}");
                    return 5;
                }
            }
            clear_section(&mut config, s);
            if let Err(e) = fs_config::save_config(&config, config_path) {
                eprintln!("fset: error: {e}");
                return 5;
            }
            0
        }
    }
}

fn clear_section(config: &mut Config, section: &str) {
    match section {
        "global" => config.global = None,
        "copy" => config.copy = None,
        "mv" => config.mv = None,
        "sync" => config.sync = None,
        "watch" => config.watch = None,
        "compress" => config.compress = None,
        _ => unreachable!("checked by is_known_section"),
    }
}

fn run_edit(config_path: &std::path::Path) -> i32 {
    let editor = match std::env::var("EDITOR") {
        Ok(e) if !e.is_empty() => e,
        _ => {
            eprintln!("fset: error: $EDITOR is not set; export EDITOR and try again");
            return 5;
        }
    };

    let starting_contents = match std::fs::read_to_string(config_path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "{}\n".to_string(),
        Err(e) => {
            eprintln!("fset: error: could not read {}: {e}", config_path.display());
            return 5;
        }
    };

    let tmp_path = std::env::temp_dir().join(format!("fset-edit-{}.json", std::process::id()));
    if let Err(e) = std::fs::write(&tmp_path, &starting_contents) {
        eprintln!("fset: error: could not create scratch file: {e}");
        return 5;
    }

    let result = loop {
        let status = std::process::Command::new(&editor).arg(&tmp_path).status();
        let status = match status {
            Ok(s) => s,
            Err(e) => {
                eprintln!("fset: error: could not launch $EDITOR ({editor}): {e}");
                break 5;
            }
        };
        if !status.success() {
            eprintln!("fset: error: $EDITOR exited with a non-zero status");
            break 5;
        }

        let edited = match std::fs::read_to_string(&tmp_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("fset: error: could not read scratch file: {e}");
                break 5;
            }
        };

        match fs_config::parse_config(&edited) {
            Ok(config) => {
                if let Err(e) = fs_config::save_config(&config, config_path) {
                    eprintln!("fset: error: {e}");
                    break 5;
                }
                break 0;
            }
            Err(e) => {
                eprintln!("fset: error: edited config is invalid:\n  {e}");
                print!("Press ENTER to reopen $EDITOR, or Ctrl+C to abort without saving.\n> ");
                use std::io::Write;
                std::io::stdout().flush().ok();
                let mut discard = String::new();
                std::io::stdin().read_line(&mut discard).ok();
            }
        }
    };

    std::fs::remove_file(&tmp_path).ok();
    result
}

#[cfg(test)]
mod completion_tests {
    use clap::CommandFactory;

    #[test]
    fn committed_scripts_match_the_current_cli() {
        crate::completions::assert_committed_scripts_are_current(&mut super::Cli::command(), "fset");
    }

    #[test]
    fn generated_scripts_describe_the_actual_subcommands() {
        for shell in crate::completions::PACKAGED_SHELLS {
            let script = crate::completions::render(&mut super::Cli::command(), "fset", shell);
            let text = String::from_utf8_lossy(&script);
            for expected in ["get", "set", "unset", "completions"] {
                assert!(text.contains(expected), "{shell} script is missing `{expected}`");
            }
        }
    }
}
