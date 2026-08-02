//! Two renderers, per §7: a generic one over `Stream<Item = Progress>` for
//! copy/mv/sync/compress, and a separate one over `Stream<Item =
//! WatchEvent>` for watch (indefinite stream, no bar). Both listen for
//! Ctrl+C concurrently and call `.cancel()` cooperatively, per §8.1's
//! "not reinterpreted as exit 1" rule — the caller checks `cancelled` and
//! exits 130 regardless of what the awaited outcome carries.

use std::io::IsTerminal;

use file_engine::{Handle, Progress, Result as EngineResult, WatchEvent, WatchEventKind, WatchHandle};
use tokio_stream::StreamExt;

pub struct DriveResult<T> {
    pub outcome: EngineResult<T>,
    pub cancelled: bool,
}

pub async fn drive<T>(mut handle: Handle<T>, quiet: bool) -> DriveResult<T> {
    let bar = if !quiet && std::io::stderr().is_terminal() {
        let pb = indicatif::ProgressBar::new(0);
        pb.set_style(
            indicatif::ProgressStyle::with_template(
                "{spinner:.cyan} {msg} [{bar:40.cyan/blue}] {pos}/{len}",
            )
            .unwrap()
            .progress_chars("=> "),
        );
        Some(pb)
    } else {
        None
    };

    let mut cancelled = false;
    loop {
        tokio::select! {
            biased;
            _ = tokio::signal::ctrl_c(), if !cancelled => {
                handle.cancel();
                cancelled = true;
                if let Some(b) = &bar {
                    b.set_message("cancelling...");
                }
            }
            next = handle.progress().next() => {
                match next {
                    Some(p) => on_progress(p, bar.as_ref()),
                    None => break,
                }
            }
        }
    }
    if let Some(b) = &bar {
        b.finish_and_clear();
    }

    let outcome = handle.await;
    DriveResult { outcome, cancelled }
}

fn on_progress(progress: Progress, bar: Option<&indicatif::ProgressBar>) {
    match progress {
        Progress::Started { bytes_total, entries_total } => {
            tracing::info!(entries_total, ?bytes_total, "started");
            if let Some(b) = bar {
                b.set_length(entries_total as u64);
                b.set_position(0);
                b.set_message("running");
            }
        }
        Progress::EntryStarted { entry } => {
            tracing::debug!(path = %entry.relative_path.display(), "entry started");
        }
        Progress::EntryCompleted { entry } => {
            tracing::debug!(path = %entry.relative_path.display(), "entry completed");
            if let Some(b) = bar {
                b.inc(1);
            }
        }
        Progress::EntryFailed { entry } => {
            tracing::warn!(path = %entry.relative_path.display(), "entry failed");
            if let Some(b) = bar {
                b.inc(1);
            }
        }
        Progress::DirectoriesStarted { total } => {
            tracing::info!(total, "creating directories");
        }
        Progress::DirectoryCompleted { path } => {
            tracing::debug!(path = %path.display(), "directory created");
        }
        Progress::DirectoryFailed { path } => {
            tracing::warn!(path = %path.display(), "directory failed");
        }
    }
}

pub struct WatchDriveResult {
    pub result: EngineResult<()>,
    pub cancelled: bool,
}

pub async fn drive_watch(mut handle: WatchHandle) -> WatchDriveResult {
    let mut cancelled = false;
    loop {
        tokio::select! {
            biased;
            _ = tokio::signal::ctrl_c(), if !cancelled => {
                handle.cancel();
                cancelled = true;
            }
            next = handle.events().next() => {
                match next {
                    Some(event) => print_watch_event(&event),
                    None => break,
                }
            }
        }
    }
    let result = handle.await;
    WatchDriveResult { result, cancelled }
}

fn print_watch_event(event: &WatchEvent) {
    let kind = match event.kind {
        WatchEventKind::Created => "created",
        WatchEventKind::Modified => "modified",
        WatchEventKind::Removed => "removed",
        WatchEventKind::Other => "other",
    };
    for path in &event.paths {
        println!("{kind}: {}", path.display());
    }
}
