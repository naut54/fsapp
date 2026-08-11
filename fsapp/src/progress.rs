//! Two renderers, per §7: a generic one over `Stream<Item = Progress>` for
//! copy/mv/sync/compress, and a separate one over `Stream<Item =
//! WatchEvent>` for watch (indefinite stream, no bar). Both listen for
//! Ctrl+C concurrently and call `.cancel()` cooperatively, per §8.1's
//! "not reinterpreted as exit 1" rule — the caller checks `cancelled` and
//! exits 130 regardless of what the awaited outcome carries.

use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Duration;

use file_engine::{
    EtaEstimator, Handle, Progress, Result as EngineResult, WatchEvent, WatchEventKind, WatchHandle,
};
use tokio_stream::StreamExt;

use crate::summary::human_bytes;

pub struct DriveResult<T> {
    pub outcome: EngineResult<T>,
    pub cancelled: bool,
}

pub async fn drive<T>(mut handle: Handle<T>, quiet: bool) -> DriveResult<T> {
    let bar = if !quiet && std::io::stderr().is_terminal() {
        let pb = indicatif::ProgressBar::new(0);
        pb.set_style(
            // `wide_bar` rather than a fixed width: the message now carries
            // a filename and the prefix an ETA, and a fixed 40-column bar
            // pushed the line past the terminal width and made it wrap.
            indicatif::ProgressStyle::with_template(
                "{spinner:.cyan} {msg} [{wide_bar:.cyan/blue}] {pos}/{len} {prefix}",
            )
            .unwrap()
            .progress_chars("=> "),
        );
        // The spinner would otherwise only advance when an event arrives,
        // so a lone large file froze it for the length of the copy.
        pb.enable_steady_tick(Duration::from_millis(100));
        Some(pb)
    } else {
        None
    };
    let mut renderer = Renderer::new(bar);

    let mut cancelled = false;
    loop {
        tokio::select! {
            biased;
            _ = tokio::signal::ctrl_c(), if !cancelled => {
                handle.cancel();
                cancelled = true;
                renderer.cancelling();
            }
            next = handle.progress().next() => {
                match next {
                    Some(p) => renderer.on_progress(&p),
                    None => break,
                }
            }
        }
    }
    renderer.finish();

    let outcome = handle.await;
    DriveResult { outcome, cancelled }
}

/// Owns the bar and the state needed to label it: the ETA estimator, and
/// the large entries currently streaming. The engine samples an in-flight
/// large file every 250ms (`Progress::EntryProgress`), which is the only
/// thing that moves while a single multi-gigabyte file copies — the
/// entry-count bar sits still for the whole transfer.
struct Renderer {
    bar: Option<indicatif::ProgressBar>,
    eta: EtaEstimator,
    /// Source path -> (bytes copied so far, total size), for entries
    /// between their first `EntryProgress` sample and their terminal
    /// event. Only large entries are ever sampled, so only they appear.
    streaming: HashMap<PathBuf, (u64, u64)>,
    creating_directories: bool,
    /// Once cancelling, the message is frozen — later events must not
    /// overwrite it with "running".
    cancelled: bool,
}

impl Renderer {
    fn new(bar: Option<indicatif::ProgressBar>) -> Self {
        Self {
            bar,
            eta: EtaEstimator::new(),
            streaming: HashMap::new(),
            creating_directories: false,
            cancelled: false,
        }
    }

    fn on_progress(&mut self, progress: &Progress) {
        // Every event either supplies work done or bounds a span of wall
        // time, so the estimator sees all of them, including the ones
        // that don't touch the bar.
        self.eta.observe(progress);

        match progress {
            Progress::Planned {
                directories,
                small_files,
                small_bytes,
                large_files,
                large_bytes,
                small_file_threshold,
            } => {
                tracing::info!(
                    directories,
                    small_files,
                    small_bytes,
                    large_files,
                    large_bytes,
                    small_file_threshold,
                    "planned"
                );
                // Arrives before the directory pre-pass, so the bar gets a
                // real length ahead of `Started` — on a large tree that
                // pre-pass can run for a minute on its own.
                if let Some(b) = &self.bar {
                    b.set_length((small_files + large_files) as u64);
                }
            }
            Progress::Started { bytes_total, entries_total } => {
                tracing::info!(entries_total, ?bytes_total, "started");
                self.creating_directories = false;
                if let Some(b) = &self.bar {
                    b.set_length(*entries_total as u64);
                    b.set_position(0);
                }
            }
            Progress::EntryStarted { entry } => {
                tracing::debug!(path = %entry.relative_path.display(), "entry started");
            }
            Progress::EntryProgress { entry, bytes_copied } => {
                tracing::trace!(
                    path = %entry.relative_path.display(),
                    bytes_copied,
                    size = entry.size,
                    "entry progress"
                );
                self.streaming.insert(entry.path.clone(), (*bytes_copied, entry.size));
            }
            Progress::EntryCompleted { entry } => {
                tracing::debug!(path = %entry.relative_path.display(), "entry completed");
                self.streaming.remove(&entry.path);
                if let Some(b) = &self.bar {
                    b.inc(1);
                }
            }
            Progress::EntryFailed { entry } => {
                tracing::warn!(path = %entry.relative_path.display(), "entry failed");
                self.streaming.remove(&entry.path);
                if let Some(b) = &self.bar {
                    b.inc(1);
                }
            }
            Progress::DirectoriesStarted { total } => {
                tracing::info!(total, "creating directories");
                self.creating_directories = true;
            }
            Progress::DirectoryCompleted { path } => {
                tracing::debug!(path = %path.display(), "directory created");
            }
            Progress::DirectoryFailed { path } => {
                tracing::warn!(path = %path.display(), "directory failed");
            }
            _ => {}
        }

        self.redraw();
    }

    fn redraw(&self) {
        let Some(b) = &self.bar else { return };
        if !self.cancelled {
            b.set_message(self.message());
        }
        b.set_prefix(self.eta_prefix());
    }

    /// What the operation is doing right now, preferring the streaming
    /// file since that's the part with no other visible movement.
    fn message(&self) -> String {
        if self.streaming.len() == 1 {
            let (path, (copied, size)) = self.streaming.iter().next().expect("len == 1");
            let name = truncate(
                &path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string()),
            );
            return match size {
                0 => name,
                size => format!("{name} {}%", copied * 100 / size),
            };
        }
        if self.streaming.len() > 1 {
            let copied: u64 = self.streaming.values().map(|(c, _)| c).sum();
            let total: u64 = self.streaming.values().map(|(_, s)| s).sum();
            return format!(
                "{} large files {}/{}",
                self.streaming.len(),
                human_bytes(copied),
                human_bytes(total)
            );
        }
        if self.creating_directories {
            return "creating directories".to_string();
        }
        "running".to_string()
    }

    /// `None` from the estimator means it has no measured rate yet for
    /// some regime with work outstanding — showing nothing beats showing
    /// a number that collapses a second later.
    fn eta_prefix(&self) -> String {
        let mut parts = Vec::with_capacity(2);
        if let Some(remaining) = self.eta.estimate() {
            parts.push(format!("ETA {}", human_duration(remaining)));
        }
        if let Some(rate) = self.eta.bytes_per_sec() {
            parts.push(format!("{}/s", human_bytes(rate as u64)));
        }
        parts.join(" \u{b7} ")
    }

    fn cancelling(&mut self) {
        self.cancelled = true;
        if let Some(b) = &self.bar {
            b.set_message("cancelling...");
        }
    }

    fn finish(&self) {
        if let Some(b) = &self.bar {
            b.finish_and_clear();
        }
    }
}

/// Keeps one long filename from squeezing the bar down to nothing.
/// Counts `char`s, not bytes, so a multi-byte name isn't cut mid-character.
fn truncate(name: &str) -> String {
    const MAX: usize = 28;
    if name.chars().count() <= MAX {
        return name.to_string();
    }
    let head: String = name.chars().take(MAX - 1).collect();
    format!("{head}\u{2026}")
}

/// Sub-second precision below 10s, since a copy the filesystem satisfies
/// by cloning finishes in milliseconds and "0s" reads as "unmeasured"
/// rather than "instant".
pub fn human_duration(d: Duration) -> String {
    let secs = d.as_secs();
    match secs {
        0..=9 => format!("{:.1}s", d.as_secs_f64()),
        10..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m{:02}s", secs / 60, secs % 60),
        _ => format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60),
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
