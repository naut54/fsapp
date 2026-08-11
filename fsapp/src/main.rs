mod cli;
mod completions;
mod convert;
mod fatal;
mod progress;
mod resolve;
mod summary;
mod update;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use cli::{BatchArgs, Command, FsSafetyArgs};
use file_engine::FileEngine;
use fs_config::Config;

#[tokio::main]
async fn main() -> ExitCode {
    let cli = cli::Cli::parse();

    let config_path = match fs_config::resolve_config_path(cli.config.clone()) {
        Ok(p) => p,
        Err(e) => {
            fatal::print("fsapp", &e);
            return ExitCode::from(5);
        }
    };
    let config = match fs_config::load_with_recovery(&config_path, "fsapp") {
        Ok(c) => c,
        Err(code) => return ExitCode::from(code as u8),
    };

    let verbosity = if cli.verbose > 0 {
        cli.verbose
    } else {
        resolve::resolve(None, "global", "verbosity", config.global.as_ref().and_then(|g| g.verbosity))
            .unwrap_or(0)
    };
    let quiet = resolve::resolve_bool(cli.quiet, "global", "quiet", config.global.as_ref().and_then(|g| g.quiet));
    init_tracing(verbosity);

    // `update-check` is the explicit path and answers for itself — it must
    // not also fire the automatic one.
    if matches!(cli.command, Command::UpdateCheck) {
        return run_update_check();
    }
    // Completions never touch the filesystem the user asked about and are
    // often piped into a file, so they skip the update notice too.
    if let Command::Completions { shell, install, dir } = cli.command {
        use clap::CommandFactory;
        let code = completions::run(&mut cli::Cli::command(), "fsapp", shell, install, dir);
        return ExitCode::from(code);
    }

    // Spawned before the operation so the request overlaps with the work
    // rather than adding to it. Nothing below awaits it on the critical
    // path — see the timeout after the command finishes.
    let update_task = update::auto_check_enabled(
        cli.no_update_check,
        config.update.as_ref().and_then(|u| u.no_check),
        quiet,
    )
    .then(update::start_background_check);

    let code = match cli.command {
        Command::UpdateCheck | Command::Completions { .. } => {
            unreachable!("handled above, before the automatic check")
        }
        Command::Copy { source, dest, batch, safety, overwrite, max_bytes_per_batch, max_files_per_batch, sort_order } => {
            run_copy(&config, quiet, source, dest, batch, safety, overwrite, max_bytes_per_batch, max_files_per_batch, sort_order).await
        }
        Command::Mv { source, dest, batch, safety, overwrite } => {
            run_mv(&config, quiet, source, dest, batch, safety, overwrite).await
        }
        Command::Sync { source, dest, batch, safety, no_overwrite, checksum } => {
            run_sync(&config, quiet, source, dest, batch, safety, no_overwrite, checksum).await
        }
        Command::Watch { path, no_recursive } => run_watch(&config, path, no_recursive).await,
        Command::Compress { source, dest, batch, format } => {
            run_compress(&config, quiet, source, dest, batch, format).await
        }
    };

    if let Some(rx) = update_task {
        report_update(rx);
    }

    ExitCode::from(code as u8)
}

/// Gives the check whatever time is left after the operation finished, and
/// no more. A fresh cache answers in microseconds; a copy that finished in
/// 40ms against a slow network will not, and the right answer there is to
/// say nothing rather than hold the shell prompt hostage for a version
/// number. The thread is simply abandoned — a late answer still reaches
/// the cache if it beats process exit, and the next run picks it up.
fn report_update(rx: std::sync::mpsc::Receiver<Option<update::Status>>) {
    if let Ok(Some(update::Status::Newer { latest, current })) = rx.recv_timeout(UPDATE_GRACE) {
        update::print_notice(&latest, &current);
    }
}

/// How long the notice is allowed to delay the shell prompt. Long enough
/// for a cache hit and a warm connection, short enough to read as instant.
const UPDATE_GRACE: Duration = Duration::from_millis(300);

/// The explicit check. Exit 5 on a network failure, consistent with §8.1's
/// "I/O error unrelated to the engine" — the check genuinely failed, and a
/// script asking "is there an update?" should be able to tell that apart
/// from "no".
fn run_update_check() -> ExitCode {
    match update::check_now() {
        Ok(status) => {
            update::print_verdict(&status);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("fsapp: error: {e}");
            ExitCode::from(5)
        }
    }
}

fn init_tracing(verbosity: u8) {
    let level = match verbosity {
        0 => tracing::Level::WARN,
        1 => tracing::Level::INFO,
        2 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_writer(std::io::stderr)
        .without_time()
        .init();
}

struct ResolvedBatch {
    on_error: Option<file_engine::ErrorStrategy>,
    small_file_threshold: Option<u64>,
    batch_concurrency: Option<usize>,
}

fn resolve_batch(section: &str, args: &BatchArgs, config: &Config) -> ResolvedBatch {
    let section_config = config_section(config, section);
    ResolvedBatch {
        on_error: resolve::resolve(args.on_error, section, "on-error", section_config.and_then(|s| s.on_error))
            .map(convert::to_error_strategy),
        small_file_threshold: resolve::resolve(
            args.small_file_threshold,
            section,
            "small-file-threshold",
            section_config.and_then(|s| s.small_file_threshold),
        ),
        batch_concurrency: resolve::resolve(
            args.batch_concurrency,
            section,
            "batch-concurrency",
            section_config.and_then(|s| s.batch_concurrency),
        )
        .map(|v| v as usize),
    }
}

struct ResolvedSafety {
    preserve_permissions: bool,
    allow_fs_integrity_risk: bool,
}

fn resolve_safety(section: &str, args: &FsSafetyArgs, config: &Config) -> ResolvedSafety {
    let section_config = config_section(config, section);
    ResolvedSafety {
        preserve_permissions: resolve::resolve_bool(
            args.preserve_permissions,
            section,
            "preserve-permissions",
            section_config.and_then(|s| s.preserve_permissions),
        ),
        allow_fs_integrity_risk: resolve::resolve_bool(
            args.allow_fs_integrity_risk,
            section,
            "allow-fs-integrity-risk",
            section_config.and_then(|s| s.allow_fs_integrity_risk),
        ),
    }
}

/// A tiny shim so `resolve_batch`/`resolve_safety` can read the four
/// fields they need without matching on which section struct it is —
/// every section shares this shape for these particular keys.
#[derive(Clone, Copy)]
struct SectionView {
    on_error: Option<fs_config::OnError>,
    small_file_threshold: Option<u64>,
    batch_concurrency: Option<u64>,
    preserve_permissions: Option<bool>,
    allow_fs_integrity_risk: Option<bool>,
}

fn config_section(config: &Config, section: &str) -> Option<SectionView> {
    match section {
        "copy" => config.copy.as_ref().map(|s| SectionView {
            on_error: s.on_error,
            small_file_threshold: s.small_file_threshold,
            batch_concurrency: s.batch_concurrency,
            preserve_permissions: s.preserve_permissions,
            allow_fs_integrity_risk: s.allow_fs_integrity_risk,
        }),
        "mv" => config.mv.as_ref().map(|s| SectionView {
            on_error: s.on_error,
            small_file_threshold: s.small_file_threshold,
            batch_concurrency: s.batch_concurrency,
            preserve_permissions: s.preserve_permissions,
            allow_fs_integrity_risk: s.allow_fs_integrity_risk,
        }),
        "sync" => config.sync.as_ref().map(|s| SectionView {
            on_error: s.on_error,
            small_file_threshold: s.small_file_threshold,
            batch_concurrency: s.batch_concurrency,
            preserve_permissions: s.preserve_permissions,
            allow_fs_integrity_risk: s.allow_fs_integrity_risk,
        }),
        "compress" => config.compress.as_ref().map(|s| SectionView {
            on_error: s.on_error,
            small_file_threshold: s.small_file_threshold,
            batch_concurrency: s.batch_concurrency,
            preserve_permissions: None,
            allow_fs_integrity_risk: None,
        }),
        _ => None,
    }
}

fn context_message(verb: &str, source: &Path, dest: &Path) -> String {
    format!("could not {verb} \"{}\" to \"{}\"", source.display(), dest.display())
}

async fn run_copy(
    config: &Config,
    quiet: bool,
    source: PathBuf,
    dest: PathBuf,
    batch: BatchArgs,
    safety: FsSafetyArgs,
    overwrite: bool,
    max_bytes_per_batch: Option<u64>,
    max_files_per_batch: Option<u64>,
    sort_order: Option<fs_config::SortOrder>,
) -> i32 {
    let batch_r = resolve_batch("copy", &batch, config);
    let safety_r = resolve_safety("copy", &safety, config);
    let overwrite = resolve::resolve_bool(overwrite, "copy", "overwrite", config.copy.as_ref().and_then(|c| c.overwrite));
    let max_bytes_per_batch = resolve::resolve(
        max_bytes_per_batch,
        "copy",
        "max-bytes-per-batch",
        config.copy.as_ref().and_then(|c| c.max_bytes_per_batch),
    );
    let max_files_per_batch = resolve::resolve(
        max_files_per_batch,
        "copy",
        "max-files-per-batch",
        config.copy.as_ref().and_then(|c| c.max_files_per_batch),
    )
    .map(|v| v as usize);
    let sort_order = resolve::resolve(sort_order, "copy", "sort-order", config.copy.as_ref().and_then(|c| c.sort_order))
        .map(convert::to_sort_order);

    let mut builder = FileEngine::new().copy(&source, &dest).overwrite(overwrite);
    if let Some(v) = batch_r.small_file_threshold {
        builder = builder.small_file_threshold(v);
    }
    if let Some(v) = batch_r.batch_concurrency {
        builder = builder.batch_concurrency(v);
    }
    if let Some(v) = batch_r.on_error {
        builder = builder.on_error(v);
    }
    if let Some(v) = max_bytes_per_batch {
        builder = builder.max_bytes_per_batch(v);
    }
    if let Some(v) = max_files_per_batch {
        builder = builder.max_files_per_batch(v);
    }
    if let Some(v) = sort_order {
        builder = builder.batch_sort_order(v);
    }
    if safety_r.allow_fs_integrity_risk {
        builder = builder.allow_filesystem_integrity_risk(true);
    }
    #[cfg(unix)]
    let builder = if safety_r.preserve_permissions { builder.preserve_permissions(true) } else { builder };
    #[cfg(not(unix))]
    if safety_r.preserve_permissions {
        tracing::warn!("--preserve-permissions is only supported on Unix; ignoring");
    }

    let context = context_message("copy", &source, &dest);
    let handle = match builder.start() {
        Ok(h) => h,
        Err(e) => {
            fatal::print_with_context("fsapp", &context, &e);
            return 4;
        }
    };

    let progress::DriveResult { outcome, cancelled } = progress::drive(handle, quiet).await;
    if cancelled {
        return 130;
    }
    match outcome {
        Ok(o) => {
            if summary::print_operation_block("copied", &o, false) {
                0
            } else {
                1
            }
        }
        Err(e) => {
            fatal::print_with_context("fsapp", &context, &e);
            4
        }
    }
}

async fn run_mv(
    config: &Config,
    quiet: bool,
    source: PathBuf,
    dest: PathBuf,
    batch: BatchArgs,
    safety: FsSafetyArgs,
    overwrite: bool,
) -> i32 {
    let batch_r = resolve_batch("mv", &batch, config);
    let safety_r = resolve_safety("mv", &safety, config);
    let overwrite = resolve::resolve_bool(overwrite, "mv", "overwrite", config.mv.as_ref().and_then(|c| c.overwrite));

    let mut builder = FileEngine::new().move_path(&source, &dest).overwrite(overwrite);
    if let Some(v) = batch_r.small_file_threshold {
        builder = builder.small_file_threshold(v);
    }
    if let Some(v) = batch_r.batch_concurrency {
        builder = builder.batch_concurrency(v);
    }
    if let Some(v) = batch_r.on_error {
        builder = builder.on_error(v);
    }
    if safety_r.allow_fs_integrity_risk {
        builder = builder.allow_filesystem_integrity_risk(true);
    }
    #[cfg(unix)]
    let builder = if safety_r.preserve_permissions { builder.preserve_permissions(true) } else { builder };
    #[cfg(not(unix))]
    if safety_r.preserve_permissions {
        tracing::warn!("--preserve-permissions is only supported on Unix; ignoring");
    }

    let context = context_message("move", &source, &dest);
    let handle = match builder.start() {
        Ok(h) => h,
        Err(e) => {
            fatal::print_with_context("fsapp", &context, &e);
            return 4;
        }
    };

    // §7: same-filesystem `mv` may resolve with zero `Progress` events —
    // `drive()` treats "stream ended immediately, handle resolved Ok" as
    // the normal case already, nothing special needed here.
    let progress::DriveResult { outcome, cancelled } = progress::drive(handle, quiet).await;
    if cancelled {
        return 130;
    }
    match outcome {
        Ok(o) => {
            if summary::print_operation_block("moved", &o, true) {
                0
            } else {
                1
            }
        }
        Err(e) => {
            fatal::print_with_context("fsapp", &context, &e);
            4
        }
    }
}

async fn run_sync(
    config: &Config,
    quiet: bool,
    source: PathBuf,
    dest: PathBuf,
    batch: BatchArgs,
    safety: FsSafetyArgs,
    no_overwrite: bool,
    checksum: bool,
) -> i32 {
    let batch_r = resolve_batch("sync", &batch, config);
    let safety_r = resolve_safety("sync", &safety, config);
    let no_overwrite =
        resolve::resolve_bool(no_overwrite, "sync", "no-overwrite", config.sync.as_ref().and_then(|c| c.no_overwrite));
    let checksum = resolve::resolve_bool(checksum, "sync", "checksum", config.sync.as_ref().and_then(|c| c.checksum));

    let mut builder = FileEngine::new().sync(&source, &dest);
    if no_overwrite {
        builder = builder.overwrite(false);
    }
    if checksum {
        builder = builder.diff_strategy(file_engine::DiffStrategy::Checksum);
    }
    if let Some(v) = batch_r.small_file_threshold {
        builder = builder.small_file_threshold(v);
    }
    if let Some(v) = batch_r.batch_concurrency {
        builder = builder.batch_concurrency(v);
    }
    if let Some(v) = batch_r.on_error {
        builder = builder.on_error(v);
    }
    if safety_r.allow_fs_integrity_risk {
        builder = builder.allow_filesystem_integrity_risk(true);
    }
    #[cfg(unix)]
    let builder = if safety_r.preserve_permissions { builder.preserve_permissions(true) } else { builder };
    #[cfg(not(unix))]
    if safety_r.preserve_permissions {
        tracing::warn!("--preserve-permissions is only supported on Unix; ignoring");
    }

    let context = context_message("sync", &source, &dest);
    let handle = match builder.start() {
        Ok(h) => h,
        Err(e) => {
            fatal::print_with_context("fsapp", &context, &e);
            return 4;
        }
    };

    let progress::DriveResult { outcome, cancelled } = progress::drive(handle, quiet).await;
    if cancelled {
        return 130;
    }
    match outcome {
        Ok(o) => {
            if summary::print_sync_summary(&o) {
                0
            } else {
                1
            }
        }
        Err(e) => {
            fatal::print_with_context("fsapp", &context, &e);
            4
        }
    }
}

async fn run_compress(
    config: &Config,
    quiet: bool,
    source: PathBuf,
    dest: PathBuf,
    batch: BatchArgs,
    format: Option<fs_config::CompressFormat>,
) -> i32 {
    let section_config = config.compress.as_ref();
    let on_error = resolve::resolve(batch.on_error, "compress", "on-error", section_config.and_then(|c| c.on_error))
        .map(convert::to_error_strategy);
    let small_file_threshold = resolve::resolve(
        batch.small_file_threshold,
        "compress",
        "small-file-threshold",
        section_config.and_then(|c| c.small_file_threshold),
    );
    let batch_concurrency = resolve::resolve(
        batch.batch_concurrency,
        "compress",
        "batch-concurrency",
        section_config.and_then(|c| c.batch_concurrency),
    )
    .map(|v| v as usize);
    let format = resolve::resolve(format, "compress", "format", section_config.and_then(|c| c.format))
        .map(convert::to_compress_format);

    let mut builder = FileEngine::new().compress(&source, &dest);
    if let Some(v) = format {
        builder = builder.format(v);
    }
    if let Some(v) = small_file_threshold {
        builder = builder.small_file_threshold(v);
    }
    if let Some(v) = batch_concurrency {
        builder = builder.batch_concurrency(v);
    }
    if let Some(v) = on_error {
        builder = builder.on_error(v);
    }

    let context = context_message("compress", &source, &dest);
    let handle = match builder.start() {
        Ok(h) => h,
        Err(e) => {
            fatal::print_with_context("fsapp", &context, &e);
            return 4;
        }
    };

    let progress::DriveResult { outcome, cancelled } = progress::drive(handle, quiet).await;
    if cancelled {
        return 130;
    }
    match outcome {
        Ok(o) => {
            if summary::print_operation_block("archived", &o, false) {
                0
            } else {
                1
            }
        }
        Err(e) => {
            fatal::print_with_context("fsapp", &context, &e);
            4
        }
    }
}

async fn run_watch(_config: &Config, path: PathBuf, no_recursive: bool) -> i32 {
    let recursive = !no_recursive;
    let builder = FileEngine::new().watch(&path).recursive(recursive);

    let handle = match builder.start() {
        Ok(h) => h,
        Err(e) => {
            fatal::print_with_context("fsapp", &format!("could not watch \"{}\"", path.display()), &e);
            return 4;
        }
    };

    let progress::WatchDriveResult { result, cancelled } = progress::drive_watch(handle).await;
    if cancelled {
        return 130;
    }
    match result {
        Ok(()) => 0,
        Err(e) => {
            fatal::print_with_context("fsapp", &format!("could not watch \"{}\"", path.display()), &e);
            4
        }
    }
}

#[cfg(test)]
mod completion_tests {
    use clap::CommandFactory;

    /// The guard that makes committing generated scripts safe: a CLI change
    /// that forgets `completions/` fails here rather than shipping a script
    /// that silently omits the new flag (docs/completions-design.md §3.1).
    #[test]
    fn committed_scripts_match_the_current_cli() {
        crate::completions::assert_committed_scripts_are_current(
            &mut crate::cli::Cli::command(),
            "fsapp",
        );
    }

    /// Catches a restructure that leaves the grammar technically valid but
    /// empty — the scripts would still generate, just describe nothing.
    #[test]
    fn generated_scripts_describe_the_actual_subcommands() {
        for shell in crate::completions::PACKAGED_SHELLS {
            let script = crate::completions::render(&mut crate::cli::Cli::command(), "fsapp", shell);
            let text = String::from_utf8_lossy(&script);
            for expected in ["copy", "sync", "update-check", "completions"] {
                assert!(text.contains(expected), "{shell} script is missing `{expected}`");
            }
        }
    }
}
