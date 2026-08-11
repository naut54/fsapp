//! Shell completion generation and installation, shared by both binaries.
//!
//! `fset` pulls this in with `#[path]` rather than `use`: the two are
//! separate `[[bin]]` targets of one package, so neither can `use` the
//! other's modules, and the logic is identical for both — only the
//! `clap::Command` differs.
//!
//! Scripts are generated from the `clap::Command` we already build, never
//! hand-written, so they cannot drift from the CLI. `update-check` and
//! `--no-update-check` would have appeared in 0.4.0's completions for
//! free. `completions/` in the repo is the committed output; the
//! staleness test in each binary regenerates and compares, so a CLI change
//! that forgets to regenerate fails the suite rather than shipping a
//! stale script.
//!
//! See `docs/completions-design.md`.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Command;
use clap_complete::Shell;

/// Renders the completion script for `shell` into a buffer.
pub fn render(cmd: &mut Command, bin_name: &str, shell: Shell) -> Vec<u8> {
    let mut buf = Vec::new();
    clap_complete::generate(shell, cmd, bin_name, &mut buf);
    buf
}

/// The conventional filename each shell looks for. zsh wants `_name` on
/// `fpath`; bash and fish want an extension. Getting these wrong means the
/// file installs successfully and then is silently never loaded.
pub fn script_name(bin_name: &str, shell: Shell) -> String {
    match shell {
        Shell::Zsh => format!("_{bin_name}"),
        Shell::Bash => format!("{bin_name}.bash"),
        Shell::Fish => format!("{bin_name}.fish"),
        other => format!("{bin_name}.{other}"),
    }
}

/// The committed script for `bin_name`/`shell`, relative to the workspace
/// root. Shipped into every archive via dist's `include`, and into the
/// `.deb` via `cargo-deb` assets.
#[cfg(test)]
pub fn repo_path(bin_name: &str, shell: Shell) -> PathBuf {
    PathBuf::from("completions").join(script_name(bin_name, shell))
}

/// The three shells we commit, package, and can install. `clap_complete`
/// will happily emit PowerShell and Elvish too — `completions <shell>`
/// still prints those — but nothing in our four macOS/Linux targets would
/// install them, so they aren't committed or packaged.
#[cfg(test)]
pub const PACKAGED_SHELLS: [Shell; 3] = [Shell::Zsh, Shell::Bash, Shell::Fish];

/// `$SHELL`'s basename, which is the best signal available without asking.
/// Returns `None` rather than guessing when it isn't one we support — a
/// wrong guess writes a working script into a directory the user's actual
/// shell never reads, which looks like success and behaves like failure.
pub fn detect_shell() -> Option<Shell> {
    shell_from_path(&std::env::var("SHELL").ok()?)
}

/// Split out from `detect_shell` so it is testable without mutating the
/// process-global environment: this module is compiled into both binaries,
/// so its tests run twice, in parallel, in two processes — env-mutating
/// tests raced each other and failed intermittently.
fn shell_from_path(shell_path: &str) -> Option<Shell> {
    match Path::new(shell_path).file_name()?.to_str()? {
        "zsh" => Some(Shell::Zsh),
        "bash" => Some(Shell::Bash),
        "fish" => Some(Shell::Fish),
        _ => None,
    }
}

/// Candidate install directories, most preferred first. The Homebrew
/// prefix leads on a Homebrew install because that's where its shell
/// integration already looks; the system paths are Debian's, matching
/// where the `.deb` puts them; the user paths are the fallback that needs
/// no privileges.
fn candidate_dirs(shell: Shell) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let home = dirs::home_dir();

    if let Some(prefix) = homebrew_prefix() {
        dirs.push(match shell {
            Shell::Zsh => prefix.join("share/zsh/site-functions"),
            Shell::Bash => prefix.join("etc/bash_completion.d"),
            Shell::Fish => prefix.join("share/fish/vendor_completions.d"),
            _ => prefix.join("share"),
        });
    }

    dirs.push(PathBuf::from(match shell {
        Shell::Zsh => "/usr/share/zsh/vendor-completions",
        Shell::Bash => "/usr/share/bash-completion/completions",
        Shell::Fish => "/usr/share/fish/vendor_completions.d",
        _ => "/usr/share",
    }));

    if let Some(home) = home {
        dirs.push(match shell {
            Shell::Zsh => home.join(".zsh/completions"),
            Shell::Bash => home.join(".local/share/bash-completion/completions"),
            Shell::Fish => home.join(".config/fish/completions"),
            _ => home.join(".local/share"),
        });
    }
    dirs
}

/// Only when the binary is actually running from a Homebrew prefix. A
/// `.deb` install on a machine that happens to have Linuxbrew shouldn't
/// have its completions diverted there.
fn homebrew_prefix() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let s = exe.to_string_lossy();
    let prefix = if s.starts_with("/opt/homebrew/") {
        "/opt/homebrew"
    } else if s.starts_with("/usr/local/") && s.contains("/Cellar/") {
        "/usr/local"
    } else if let Some(idx) = s.find("/Cellar/") {
        return Some(PathBuf::from(&s[..idx]));
    } else {
        return None;
    };
    Some(PathBuf::from(prefix))
}

pub struct Installed {
    pub path: PathBuf,
    /// Set when the chosen directory isn't one the shell reads
    /// automatically, so the caller can print the line that fixes it.
    pub needs_fpath_hint: bool,
}

/// Writes the script into the first candidate directory that accepts it.
/// `explicit_dir` skips the search entirely.
pub fn install(
    cmd: &mut Command,
    bin_name: &str,
    shell: Shell,
    explicit_dir: Option<PathBuf>,
) -> Result<Installed, String> {
    let script = render(cmd, bin_name, shell);
    let file_name = script_name(bin_name, shell);

    let candidates = match explicit_dir {
        Some(dir) => vec![dir],
        None => candidate_dirs(shell),
    };

    let mut last_error = None;
    for dir in &candidates {
        match try_write(dir, &file_name, &script) {
            Ok(path) => {
                let needs_fpath_hint = shell == Shell::Zsh && is_user_dir(dir);
                return Ok(Installed { path, needs_fpath_hint });
            }
            Err(e) => last_error = Some(format!("{}: {e}", dir.display())),
        }
    }

    Err(format!(
        "no writable completion directory found (tried {}){}",
        candidates
            .iter()
            .map(|d| d.display().to_string())
            .collect::<Vec<_>>()
            .join(", "),
        last_error.map(|e| format!("; last error: {e}")).unwrap_or_default()
    ))
}

/// Creates the directory only for user-owned paths. Creating
/// `/usr/share/...` on a machine that doesn't have it would either fail on
/// permissions anyway or invent a directory no shell was told to read.
fn try_write(dir: &Path, file_name: &str, script: &[u8]) -> std::io::Result<PathBuf> {
    if !dir.exists() {
        if !is_user_dir(dir) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "directory does not exist",
            ));
        }
        std::fs::create_dir_all(dir)?;
    }
    let path = dir.join(file_name);
    let mut file = std::fs::File::create(&path)?;
    file.write_all(script)?;
    Ok(path)
}

fn is_user_dir(dir: &Path) -> bool {
    dirs::home_dir().is_some_and(|home| dir.starts_with(home))
}

/// The shared `completions` handler. Returns a process exit code.
pub fn run(
    cmd: &mut Command,
    bin_name: &str,
    shell: Option<Shell>,
    do_install: bool,
    dir: Option<PathBuf>,
) -> u8 {
    let shell = match shell.or_else(detect_shell) {
        Some(s) => s,
        None => {
            eprintln!(
                "{bin_name}: error: could not detect the shell from $SHELL; \
                 pass one explicitly, e.g. `{bin_name} completions zsh`"
            );
            return 2;
        }
    };

    // `--dir` without `--install` is a request to install somewhere
    // specific, not a request to print.
    if !do_install && dir.is_none() {
        let script = render(cmd, bin_name, shell);
        std::io::stdout().write_all(&script).ok();
        return 0;
    }

    match install(cmd, bin_name, shell, dir) {
        Ok(installed) => {
            println!("installed {shell} completions to {}", installed.path.display());
            if installed.needs_fpath_hint {
                let dir = installed.path.parent().unwrap_or(Path::new("")).display();
                println!();
                println!("that directory is not on zsh's fpath by default — add to ~/.zshrc:");
                println!("  fpath=({dir} $fpath)");
                println!("  autoload -Uz compinit && compinit");
            } else {
                println!("start a new shell, or run: exec {shell}");
            }
            0
        }
        Err(e) => {
            eprintln!("{bin_name}: error: {e}");
            5
        }
    }
}

/// Regenerates every packaged script and compares it against the committed
/// copy. Shared by both binaries' staleness tests — the guard that makes
/// committing generated files safe (`docs/completions-design.md` §3.1).
#[cfg(test)]
pub fn assert_committed_scripts_are_current(cmd: &mut Command, bin_name: &str) {
    // Tests run with the package dir as CWD; the scripts live at the
    // workspace root.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root");

    for shell in PACKAGED_SHELLS {
        let path = root.join(repo_path(bin_name, shell));
        let generated = render(cmd, bin_name, shell);
        let committed = std::fs::read(&path).unwrap_or_else(|e| {
            panic!(
                "{} is missing ({e}). Regenerate with: cargo run --bin {bin_name} -- \
                 completions {shell} > {}",
                path.display(),
                path.display()
            )
        });
        assert!(
            generated == committed,
            "{} is stale. The CLI changed without regenerating completions. Run:\n  \
             cargo run --bin {bin_name} -- completions {shell} > {}",
            path.display(),
            path.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zsh_scripts_are_named_for_fpath_and_the_others_by_extension() {
        assert_eq!(script_name("fsapp", Shell::Zsh), "_fsapp");
        assert_eq!(script_name("fsapp", Shell::Bash), "fsapp.bash");
        assert_eq!(script_name("fsapp", Shell::Fish), "fsapp.fish");
    }

    #[test]
    fn an_unsupported_login_shell_is_not_guessed_at() {
        assert!(
            shell_from_path("/bin/ksh").is_none(),
            "a wrong guess installs a script nothing reads"
        );
        assert!(shell_from_path("").is_none());
    }

    #[test]
    fn the_login_shell_is_read_from_its_basename() {
        assert_eq!(shell_from_path("/opt/homebrew/bin/fish"), Some(Shell::Fish));
        assert_eq!(shell_from_path("/bin/zsh"), Some(Shell::Zsh));
        assert_eq!(shell_from_path("/usr/local/bin/bash"), Some(Shell::Bash));
    }

    #[test]
    fn every_packaged_shell_has_a_candidate_directory() {
        for shell in PACKAGED_SHELLS {
            assert!(!candidate_dirs(shell).is_empty(), "{shell} has nowhere to install");
        }
    }

    #[test]
    fn an_explicit_directory_is_used_verbatim() {
        let dir = tempfile::tempdir().unwrap();
        let mut cmd = Command::new("demo").subcommand(Command::new("shipit"));
        let installed =
            install(&mut cmd, "demo", Shell::Zsh, Some(dir.path().to_path_buf())).unwrap();
        assert_eq!(installed.path, dir.path().join("_demo"));
        let written = std::fs::read_to_string(&installed.path).unwrap();
        assert!(written.contains("shipit"), "the script should describe the CLI");
    }

    #[test]
    fn an_unwritable_explicit_directory_is_an_error_not_a_panic() {
        let mut cmd = Command::new("demo");
        let result = install(
            &mut cmd,
            "demo",
            Shell::Zsh,
            Some(PathBuf::from("/dev/null/nope")),
        );
        assert!(result.is_err());
    }
}
