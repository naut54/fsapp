use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use fs_config::{CompressFormat, OnError, SortOrder};

#[derive(Parser)]
#[command(name = "fsapp", about = "copy / mv / sync / watch / compress, backed by file-engine")]
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

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Copy {
        source: PathBuf,
        dest: PathBuf,
        #[command(flatten)]
        batch: BatchArgs,
        #[command(flatten)]
        safety: FsSafetyArgs,
        #[arg(long)]
        overwrite: bool,
        #[arg(long)]
        max_bytes_per_batch: Option<u64>,
        #[arg(long)]
        max_files_per_batch: Option<u64>,
        #[arg(long)]
        sort_order: Option<SortOrder>,
    },
    Mv {
        source: PathBuf,
        dest: PathBuf,
        #[command(flatten)]
        batch: BatchArgs,
        #[command(flatten)]
        safety: FsSafetyArgs,
        #[arg(long)]
        overwrite: bool,
    },
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
    Watch {
        path: PathBuf,
        /// Inverts the builder's default of `true`.
        #[arg(long)]
        no_recursive: bool,
    },
    Compress {
        source: PathBuf,
        dest: PathBuf,
        #[command(flatten)]
        batch: BatchArgs,
        /// Inferred from DEST's extension if omitted.
        #[arg(long)]
        format: Option<CompressFormat>,
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
