//! §8.3 operation summary, printed to stdout on exit 0 or 1.

use colored::Colorize;
use file_engine::{OperationOutcome, StopReason, SyncOutcome};

use crate::progress::human_duration;

/// Prints one summary block for `outcome`. `verb` is what appears after
/// the entry count ("copied", "moved", "archived", ...). Returns whether
/// this block represents full success (no failures, not stopped early) —
/// callers use this to decide the process exit code.
pub fn print_operation_block(verb: &str, outcome: &OperationOutcome, show_cleanup: bool) -> bool {
    let bytes: u64 = outcome.succeeded.iter().map(|e| e.size).sum();
    println!(
        "{} {} entries {verb} ({}) in {}",
        "\u{2713}".green(),
        outcome.succeeded.len(),
        human_bytes(bytes),
        human_duration(outcome.duration)
    );

    if !outcome.failed.is_empty() {
        println!("{} {} entries failed:", "\u{2717}".red(), outcome.failed.len());
        for (entry, err) in &outcome.failed {
            println!("  - {}: {err}", entry.relative_path.display());
        }
    }

    if let Some(reason) = outcome.stopped_early {
        println!(
            "{} stopped early: {}",
            "\u{26A0}".yellow(),
            stop_reason_message(reason, outcome.failed.len())
        );
    }

    if show_cleanup && !outcome.cleanup_failed.is_empty() {
        println!(
            "\u{21BA} {} entries copied but source cleanup failed (data duplicated, not lost):",
            outcome.cleanup_failed.len()
        );
        for (entry, err) in &outcome.cleanup_failed {
            println!("  - {}: {err}", entry.relative_path.display());
        }
    }

    if !outcome.directories_failed.is_empty() {
        println!(
            "{} {} directories: permission bits not applied:",
            "\u{26A0}".yellow(),
            outcome.directories_failed.len()
        );
        for (path, err) in &outcome.directories_failed {
            println!("  - {}: {err}", path.display());
        }
    }

    outcome.failed.is_empty() && outcome.stopped_early.is_none()
}

/// §8.3: sync gets two full summary blocks, headed `Copy phase:` /
/// `Delete phase:`.
pub fn print_sync_summary(outcome: &SyncOutcome) -> bool {
    println!("Copy phase:");
    let copy_ok = print_operation_block("copied", &outcome.copy, false);
    println!("Delete phase:");
    let delete_ok = print_operation_block("deleted", &outcome.delete, false);
    copy_ok && delete_ok
}

fn stop_reason_message(reason: StopReason, failed_count: usize) -> String {
    let ordinal = ordinal(failed_count);
    match reason {
        StopReason::AbortOnError => format!("reached --on-error abort after the {ordinal} failure"),
        StopReason::Undo => format!("rolled back after the {ordinal} failure (--on-error undo)"),
        StopReason::Cancelled => "cancelled".to_string(),
        StopReason::Fatal => "a fatal error stopped the operation".to_string(),
        // `StopReason` is `#[non_exhaustive]` as of file-engine 2.0.0.
        _ => "stopped early".to_string(),
    }
}

fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, 11) | (2, 12) | (3, 13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{size:.1} {}", UNITS[unit])
}
