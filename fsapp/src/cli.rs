use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use clap_complete::Shell;
use fs_config::{CompressFormat, OnError, SortOrder};

#[derive(Parser)]
#[command(
    name = "fsapp",
    version,
    about = "copy / mv / sync / watch / compress / analyze / remove, backed by file-engine"
)]
pub struct Cli {
    /// -v info, -vv debug, -vvv trace (default: warn).
    #[arg(short = 'v', action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Suppress the progress bar; logging still follows -v.
    #[arg(short = 'q', long = "quiet", global = true)]
    pub quiet: bool,

    /// Override the config file location for this invocation.
    #[arg(long = "config", global = true)]
    pub config: Option<PathBuf>,

    /// Skip the automatic check for a newer fsapp release.
    #[arg(long = "no-update-check", global = true)]
    pub no_update_check: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Copy files from SOURCE to DEST.
    Copy {
        source: PathBuf,
        dest: PathBuf,
        #[command(flatten)]
        batch: BatchArgs,
        #[command(flatten)]
        safety: FsSafetyArgs,
        #[arg(long)]
        overwrite: bool,
        /// Only consulted with --overwrite unset: an already-identical
        /// destination is left alone instead of failing; a genuinely
        /// different one still fails.
        #[arg(long)]
        skip_if_identical: bool,
        #[arg(long)]
        max_bytes_per_batch: Option<u64>,
        #[arg(long)]
        max_files_per_batch: Option<u64>,
        #[arg(long)]
        sort_order: Option<SortOrder>,
    },
    /// Move files from SOURCE to DEST.
    Mv {
        source: PathBuf,
        dest: PathBuf,
        #[command(flatten)]
        batch: BatchArgs,
        #[command(flatten)]
        safety: FsSafetyArgs,
        #[arg(long)]
        overwrite: bool,
        /// Only consulted with --overwrite unset: an already-identical
        /// destination is left alone instead of failing; a genuinely
        /// different one still fails.
        #[arg(long)]
        skip_if_identical: bool,
    },
    /// Move several independent SOURCES into one DEST directory as a
    /// single batched operation — each source keeps its own basename
    /// under DEST, which must be a directory sources land inside, never
    /// a rename target the way `mv`'s DEST can be.
    MvMany {
        #[arg(required = true, num_args = 1..)]
        sources: Vec<PathBuf>,
        dest: PathBuf,
        #[command(flatten)]
        batch: BatchArgs,
        #[command(flatten)]
        safety: FsSafetyArgs,
        #[arg(long)]
        overwrite: bool,
        /// Only consulted with --overwrite unset: an already-identical
        /// destination is left alone instead of failing; a genuinely
        /// different one still fails.
        #[arg(long)]
        skip_if_identical: bool,
    },
    /// Sync DEST to match SOURCE (copies changes, deletes orphans).
    Sync {
        source: PathBuf,
        dest: PathBuf,
        #[command(flatten)]
        batch: BatchArgs,
        #[command(flatten)]
        safety: FsSafetyArgs,
        /// Inverts the builder's default of `true`.
        #[arg(long)]
        no_overwrite: bool,
        #[arg(long)]
        checksum: bool,
    },
    /// Watch PATH for filesystem changes and print events until Ctrl+C.
    Watch {
        path: PathBuf,
        /// Inverts the builder's default of `true`.
        #[arg(long)]
        no_recursive: bool,
    },
    /// Compress SOURCE into an archive at DEST.
    Compress {
        source: PathBuf,
        dest: PathBuf,
        #[command(flatten)]
        batch: BatchArgs,
        /// Inferred from DEST's extension if omitted.
        #[arg(long)]
        format: Option<CompressFormat>,
    },
    /// Inspect a tree read-only: counts, sizes, largest files, extension
    /// and age breakdowns, and optionally MIME types and duplicates.
    Analyze {
        path: PathBuf,
        /// Only files with one of these extensions (no leading dot).
        #[arg(long, value_delimiter = ',')]
        extensions: Option<Vec<String>>,
        /// Glob patterns, matched relative to PATH, that prune traversal.
        #[arg(long, value_delimiter = ',')]
        exclude: Option<Vec<String>>,
        #[arg(long)]
        min_size: Option<u64>,
        #[arg(long)]
        max_size: Option<u64>,
        #[arg(long)]
        max_depth: Option<usize>,
        #[arg(long)]
        follow_symlinks: bool,
        /// How many of the largest matched files to list. 0 disables it.
        #[arg(long)]
        top_n_largest: Option<usize>,
        /// Worker threads for concurrent directory reads/stats. Defaults
        /// to available parallelism.
        #[arg(long)]
        walk_concurrency: Option<usize>,
        /// Sniff each matched file's header to classify its MIME type.
        #[arg(long)]
        detect_mime_types: bool,
        /// Content-hash size-colliding files to find exact duplicates.
        #[arg(long)]
        detect_duplicates: bool,
        /// Stop at the first error instead of skipping and collecting it.
        #[arg(long)]
        abort_on_error: bool,
    },
    /// Delete files under PATH matching the given criteria. Previews
    /// matches without touching anything unless --no-dry-run is passed,
    /// and refuses to run at all with no filter criteria set unless
    /// --allow-unfiltered-delete opts in explicitly — deliberately no
    /// config-file section for this command: a destructive default
    /// (hard-delete, or an unfiltered delete) has no business sitting in
    /// a JSON file that isn't part of the invocation you're looking at.
    Remove {
        path: PathBuf,
        /// Only files with one of these extensions (no leading dot).
        #[arg(long, value_delimiter = ',')]
        extensions: Option<Vec<String>>,
        /// Glob patterns, matched relative to PATH, that spare an
        /// otherwise-matching entry.
        #[arg(long, value_delimiter = ',')]
        exclude: Option<Vec<String>>,
        #[arg(long)]
        min_size: Option<u64>,
        #[arg(long)]
        max_size: Option<u64>,
        /// RFC3339 timestamp, e.g. 2026-01-01T00:00:00Z.
        #[arg(long, value_parser = parse_rfc3339)]
        modified_after: Option<std::time::SystemTime>,
        /// RFC3339 timestamp, e.g. 2026-01-01T00:00:00Z.
        #[arg(long, value_parser = parse_rfc3339)]
        modified_before: Option<std::time::SystemTime>,
        #[arg(long)]
        max_depth: Option<usize>,
        #[arg(long)]
        follow_symlinks: bool,
        #[command(flatten)]
        batch: RemoveBatchArgs,
        /// Inverts the builder's default of `true`: actually delete
        /// matches instead of only previewing them.
        #[arg(long)]
        no_dry_run: bool,
        /// Unlink matches permanently instead of moving them to the
        /// platform trash/recycle bin.
        #[arg(long)]
        hard_delete: bool,
        /// Required to proceed when no filter criterion above is set —
        /// otherwise an unfiltered PATH (matching everything under it)
        /// is refused before anything is touched.
        #[arg(long)]
        allow_unfiltered_delete: bool,
    },
    /// Check whether a newer fsapp release is available.
    UpdateCheck,
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

/// Flattened into copy/mv/sync/compress — the four that go through the
/// batching pipeline (fsapp-design-spec.md §4.2).
#[derive(Args, Default)]
pub struct BatchArgs {
    #[arg(long)]
    pub small_file_threshold: Option<u64>,
    #[arg(long)]
    pub batch_concurrency: Option<u64>,
    #[arg(long)]
    pub on_error: Option<OnError>,
}

/// Flattened into copy/mv/sync only — compress and watch don't have these
/// methods on their builders (§4.2).
#[derive(Args, Default)]
pub struct FsSafetyArgs {
    #[arg(long)]
    pub preserve_permissions: bool,
    #[arg(long)]
    pub allow_fs_integrity_risk: bool,
}

/// Flattened into remove only. No `small_file_threshold` — removal has no
/// small/large split (`RemoveBuilder`'s doc comment: every matched entry
/// is treated as a batch unit) — and no `FsSafetyArgs`, since
/// `preserve_permissions`/`allow_fs_integrity_risk` aren't methods on
/// `RemoveBuilder`.
#[derive(Args, Default)]
pub struct RemoveBatchArgs {
    #[arg(long)]
    pub on_error: Option<OnError>,
    #[arg(long)]
    pub batch_concurrency: Option<u64>,
}

fn parse_rfc3339(s: &str) -> Result<std::time::SystemTime, String> {
    humantime::parse_rfc3339(s).map_err(|e| e.to_string())
}
