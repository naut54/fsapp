//! Version check against the GitHub Releases API, in two modes:
//!
//! - `fsapp update-check` — always hits the network, always prints a
//!   verdict, and reports a failure to reach GitHub as an error.
//! - an automatic check alongside a normal command — cached to disk, run
//!   concurrently with the operation, and silent about everything except
//!   "there is a newer version". It must never delay, fail, or otherwise
//!   affect the operation the user actually asked for; every error path
//!   here ends in "say nothing".
//!
//! The cache exists because the alternative is a network round trip on
//! every single invocation of a file-copy tool. GitHub's unauthenticated
//! rate limit is 60 requests/hour/IP, which one impatient user with a
//! shell loop would otherwise exhaust.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use colored::Colorize;
use semver::Version;
use serde::{Deserialize, Serialize};

const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/naut54/fsapp/releases/latest";
const RELEASES_PAGE: &str = "https://github.com/naut54/fsapp/releases/latest";

/// How long a successful answer stays good. A day is short enough that a
/// release is noticed promptly and long enough that the check is invisible.
const CACHE_TTL_OK: Duration = Duration::from_secs(24 * 60 * 60);
/// How long a *failed* attempt suppresses the next one. Much shorter than
/// `CACHE_TTL_OK`: a failure usually means the machine was offline for a
/// moment, and punishing that with a full day of silence would hide a
/// release for a day for anyone who ran the tool at the wrong time.
const CACHE_TTL_FAIL: Duration = Duration::from_secs(60 * 60);

/// Hard ceiling on the request. The automatic check is concurrent with the
/// operation, so this only bounds how long the *task* lives; the operation
/// never waits on it (see `main.rs`).
const NETWORK_TIMEOUT: Duration = Duration::from_secs(3);

const CURRENT: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
pub enum UpdateError {
    Network(String),
    /// A tag that isn't `vMAJOR.MINOR.PATCH`. Reported rather than ignored
    /// in the explicit path: it means the release process changed shape,
    /// which is worth knowing about.
    UnparsableTag(String),
    Current(semver::Error),
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(e) => write!(f, "could not reach GitHub: {e}"),
            Self::UnparsableTag(tag) => write!(f, "unrecognised release tag \"{tag}\""),
            Self::Current(e) => write!(f, "could not parse this build's own version: {e}"),
        }
    }
}

/// What the check concluded. `UpToDate` carries the current version so the
/// explicit path can print it without re-parsing.
pub enum Status {
    UpToDate(Version),
    Newer { latest: Version, current: Version },
}

/// Whether the automatic check should run at all, per §6.2 precedence:
/// `--no-update-check` > `FSAPP_UPDATE_NO_CHECK` > `update.no-check` >
/// default (it runs).
///
/// Suppressed additionally when there's nobody to read it — `--quiet`, a
/// non-tty stderr (the notice would land in a log or a pipe), or `CI` set.
/// A build server has no use for an upgrade suggestion and shouldn't be
/// making the request at all.
pub fn auto_check_enabled(no_check_flag: bool, config_no_check: Option<bool>, quiet: bool) -> bool {
    use std::io::IsTerminal;

    if crate::resolve::resolve_bool(no_check_flag, "update", "no-check", config_no_check) {
        return false;
    }
    if quiet || !std::io::stderr().is_terminal() {
        return false;
    }
    if std::env::var_os("CI").is_some() {
        return false;
    }
    true
}

/// Starts the automatic check on a detached OS thread and hands back the
/// channel its answer will arrive on.
///
/// Deliberately **not** `tokio::task::spawn_blocking`: the runtime waits
/// for blocking tasks during shutdown, so a `spawn_blocking` check against
/// an unreachable GitHub added the full `NETWORK_TIMEOUT` to the user's
/// command *after* the copy had already finished — measured at 3.01s wall
/// clock for a copy that took 0.0s. A plain thread is outside the
/// runtime's control and dies with the process, which is precisely the
/// behaviour wanted: the answer is never worth waiting for.
pub fn start_background_check() -> std::sync::mpsc::Receiver<Option<Status>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(check_cached());
    });
    rx
}

/// Answer from cache when it's fresh, otherwise ask GitHub and record the
/// answer. Every failure returns `None` — an unreachable network, an
/// unwritable cache directory, and a malformed cache file are all "say
/// nothing", never "interrupt the user".
fn check_cached() -> Option<Status> {
    let cache_path = fs_config::update_cache_path().ok()?;
    let cached = read_cache(&cache_path);

    if let Some(cache) = &cached {
        if cache.is_fresh() {
            // A cached failure (no version recorded) is still a fresh
            // answer — it means "don't ask again yet", not "ask again now".
            let latest = cache.latest_version.as_ref()?;
            return compare(latest).ok();
        }
    }

    // Stamped *before* the request, not after. This thread is killed
    // whenever the process exits first, and without a stamp already on
    // disk an interrupted refresh would leave the cache stale — so every
    // subsequent short-lived invocation would open its own connection to
    // GitHub and none would ever live long enough to finish. The stamp
    // caps that at one attempt per `CACHE_TTL_FAIL`, and a successful
    // fetch overwrites it a moment later.
    write_cache(&cache_path, &Cache { last_checked: now_unix(), latest_version: None });

    let fetched = fetch_latest_tag();
    write_cache(
        &cache_path,
        &Cache {
            last_checked: now_unix(),
            latest_version: fetched.as_ref().ok().cloned(),
        },
    );
    compare(&fetched.ok()?).ok()
}

/// The explicit path: always a fresh request, and the cache is refreshed as
/// a side effect so an interactive check also resets the automatic one.
pub fn check_now() -> Result<Status, UpdateError> {
    let fetched = fetch_latest_tag();
    if let Ok(cache_path) = fs_config::update_cache_path() {
        write_cache(
            &cache_path,
            &Cache {
                last_checked: now_unix(),
                latest_version: fetched.as_ref().ok().cloned(),
            },
        );
    }
    compare(&fetched?)
}

fn compare(latest_tag: &str) -> Result<Status, UpdateError> {
    let latest = parse_tag(latest_tag)?;
    let current = Version::parse(CURRENT).map_err(UpdateError::Current)?;
    if latest > current {
        Ok(Status::Newer { latest, current })
    } else {
        Ok(Status::UpToDate(current))
    }
}

/// Tags are `vMAJOR.MINOR.PATCH`; the `v` is ours, not semver's.
fn parse_tag(tag: &str) -> Result<Version, UpdateError> {
    Version::parse(tag.strip_prefix('v').unwrap_or(tag))
        .map_err(|_| UpdateError::UnparsableTag(tag.to_string()))
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

/// `/releases/latest` excludes prereleases and drafts, so a `-rc.1` tag
/// never reaches users as an upgrade prompt.
fn fetch_latest_tag() -> Result<String, UpdateError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(NETWORK_TIMEOUT))
        .build()
        .into();

    let release: Release = agent
        .get(LATEST_RELEASE_URL)
        // GitHub rejects API requests without a User-Agent outright.
        .header("User-Agent", &format!("fsapp/{CURRENT}"))
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| UpdateError::Network(e.to_string()))?
        .body_mut()
        .read_json()
        .map_err(|e| UpdateError::Network(e.to_string()))?;

    Ok(release.tag_name)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Cache {
    last_checked: u64,
    /// `None` records a failed attempt, which is what gives failures their
    /// own shorter TTL instead of retrying on every invocation.
    #[serde(default)]
    latest_version: Option<String>,
}

impl Cache {
    fn is_fresh(&self) -> bool {
        let ttl = if self.latest_version.is_some() {
            CACHE_TTL_OK
        } else {
            CACHE_TTL_FAIL
        };
        match now_unix().checked_sub(self.last_checked) {
            Some(age) => Duration::from_secs(age) < ttl,
            // A `last_checked` in the future means the clock moved
            // backwards. Treat it as stale and re-check rather than
            // trusting it until the clock catches up.
            None => false,
        }
    }
}

fn read_cache(path: &Path) -> Option<Cache> {
    let contents = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

/// Best-effort. A read-only or missing config directory costs us the cache,
/// not the check, and definitely not the operation.
fn write_cache(path: &PathBuf, cache: &Cache) {
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    if let Ok(json) = serde_json::to_string(cache) {
        let _ = std::fs::write(path, json);
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// How this copy of fsapp was installed, inferred from where the binary
/// actually lives, so the notice can name the one command that will work
/// rather than listing every channel. Homebrew's `bin/` entries are
/// symlinks into `Cellar`, hence the canonicalize.
fn upgrade_command() -> String {
    let exe = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .unwrap_or_default();
    let path = exe.to_string_lossy();

    if path.contains("/Cellar/") || path.contains("/homebrew/") {
        "brew update && brew upgrade naut54/tap/fsapp".to_string()
    } else if path.contains("/.cargo/") {
        format!("curl --proto '=https' --tlsv1.2 -LsSf {RELEASES_PAGE}/download/fsapp-installer.sh | sh")
    } else if path.starts_with("/usr/bin/") {
        format!("download the .deb from {RELEASES_PAGE}, then: sudo dpkg -i fsapp_*.deb")
    } else {
        format!("see {RELEASES_PAGE}")
    }
}

/// The automatic notice: two lines on stderr, so it never contaminates
/// piped stdout, printed after the operation's own summary.
pub fn print_notice(latest: &Version, current: &Version) {
    eprintln!();
    eprintln!(
        "{} fsapp {latest} is available (you have {current})",
        "\u{2191}".yellow()
    );
    eprintln!("  {}", upgrade_command());
}

/// The explicit `update-check` verdict, on stdout — here the version *is*
/// the output, so it should survive a pipe.
pub fn print_verdict(status: &Status) {
    match status {
        Status::UpToDate(current) => {
            println!("{} fsapp {current} is the latest version", "\u{2713}".green());
        }
        Status::Newer { latest, current } => {
            println!(
                "{} fsapp {latest} is available (you have {current})",
                "\u{2191}".yellow()
            );
            println!("  {}", upgrade_command());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_parse_with_and_without_the_v_prefix() {
        assert_eq!(parse_tag("v1.2.3").unwrap(), Version::new(1, 2, 3));
        assert_eq!(parse_tag("1.2.3").unwrap(), Version::new(1, 2, 3));
    }

    #[test]
    fn an_unrecognisable_tag_is_an_error_not_a_silent_zero() {
        assert!(matches!(parse_tag("nightly"), Err(UpdateError::UnparsableTag(_))));
    }

    #[test]
    fn version_ordering_is_semantic_not_lexicographic() {
        // The whole reason for depending on `semver`: as strings,
        // "0.9.0" sorts after "0.10.0".
        assert!(parse_tag("v0.10.0").unwrap() > parse_tag("v0.9.0").unwrap());
        assert!(parse_tag("v1.0.0").unwrap() > parse_tag("v0.99.99").unwrap());
    }

    #[test]
    fn a_prerelease_does_not_outrank_the_release_it_precedes() {
        assert!(parse_tag("v1.0.0").unwrap() > parse_tag("v1.0.0-rc.1").unwrap());
    }

    #[test]
    fn this_builds_own_version_is_always_parsable() {
        // Guards against a Cargo.toml version that semver would reject,
        // which would otherwise only surface at runtime as a failed check.
        Version::parse(CURRENT).expect("CARGO_PKG_VERSION must be valid semver");
    }

    #[test]
    fn a_successful_cache_entry_is_fresh_for_a_day_and_stale_after() {
        let fresh = Cache {
            last_checked: now_unix() - 60,
            latest_version: Some("0.1.0".to_string()),
        };
        assert!(fresh.is_fresh());

        let stale = Cache {
            last_checked: now_unix() - CACHE_TTL_OK.as_secs() - 1,
            latest_version: Some("0.1.0".to_string()),
        };
        assert!(!stale.is_fresh());
    }

    #[test]
    fn a_failed_attempt_is_retried_far_sooner_than_a_successful_one() {
        let age = CACHE_TTL_FAIL.as_secs() + 1;
        let failed = Cache { last_checked: now_unix() - age, latest_version: None };
        assert!(!failed.is_fresh(), "a failure older than its own TTL must be retried");

        let succeeded = Cache {
            last_checked: now_unix() - age,
            latest_version: Some("0.1.0".to_string()),
        };
        assert!(succeeded.is_fresh(), "a success of the same age must still be trusted");
    }

    #[test]
    fn a_last_checked_in_the_future_is_treated_as_stale() {
        let skewed = Cache {
            last_checked: now_unix() + 86_400,
            latest_version: Some("0.1.0".to_string()),
        };
        assert!(!skewed.is_fresh());
    }

    #[test]
    fn cache_round_trips_through_its_on_disk_form() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-check.json");
        write_cache(
            &path,
            &Cache { last_checked: 1_700_000_000, latest_version: Some("9.9.9".to_string()) },
        );
        let read = read_cache(&path).expect("just written");
        assert_eq!(read.last_checked, 1_700_000_000);
        assert_eq!(read.latest_version.as_deref(), Some("9.9.9"));
    }

    #[test]
    fn a_corrupt_cache_file_is_ignored_rather_than_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-check.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert!(read_cache(&path).is_none());
    }

    #[test]
    fn writing_the_cache_creates_the_directory_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("deeper").join("update-check.json");
        write_cache(&path, &Cache { last_checked: 1, latest_version: None });
        assert!(path.exists());
    }

    #[test]
    fn an_unwritable_cache_location_is_survivable() {
        // A path whose parent can't be created — must not panic.
        write_cache(
            &PathBuf::from("/dev/null/nope/update-check.json"),
            &Cache { last_checked: 1, latest_version: None },
        );
    }

    #[test]
    fn the_upgrade_command_is_never_empty_whatever_the_install_path() {
        assert!(!upgrade_command().is_empty());
    }
}
