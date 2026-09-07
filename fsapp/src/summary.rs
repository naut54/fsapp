//! §8.3 operation summary, printed to stdout on exit 0 or 1.

use colored::Colorize;
use file_engine::{AnalysisReport, OperationOutcome, StopReason, SyncOutcome};

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

pub fn print_analysis_report(report: &AnalysisReport) {
    println!(
        "{} {} files, {} dirs, {} in {}",
        "\u{2713}".green(),
        report.file_count,
        report.dir_count,
        human_bytes(report.total_size),
        human_duration(report.duration)
    );

    if !report.largest_files.is_empty() {
        println!("\nLargest files:");
        for entry in &report.largest_files {
            println!("  {:>10}  {}", human_bytes(entry.size), entry.relative_path.display());
        }
    }

    if !report.by_extension.is_empty() {
        println!("\nBy extension:");
        let mut exts: Vec<_> = report.by_extension.iter().collect();
        exts.sort_by(|a, b| b.1.total_size.cmp(&a.1.total_size));
        for (ext, stats) in exts {
            let label = if ext.is_empty() { "(none)" } else { ext.as_str() };
            println!("  {:<12} {:>6} files  {:>10}", label, stats.count, human_bytes(stats.total_size));
        }
    }

    if !report.by_mime.is_empty() {
        println!("\nBy MIME type:");
        let mut mimes: Vec<_> = report.by_mime.iter().collect();
        mimes.sort_by(|a, b| b.1.total_size.cmp(&a.1.total_size));
        for (mime, stats) in mimes {
            println!("  {:<24} {:>6} files  {:>10}", mime, stats.count, human_bytes(stats.total_size));
        }
    }

    let a = &report.age_buckets;
    println!("\nAge:");
    println!("  <1 day    {}", a.under_1_day);
    println!("  <1 week   {}", a.under_1_week);
    println!("  <1 month  {}", a.under_1_month);
    println!("  <1 year   {}", a.under_1_year);
    println!("  older     {}", a.older);
    if a.unknown > 0 {
        println!("  unknown   {}", a.unknown);
    }

    if report.duplicate_groups_total > 0 {
        println!(
            "\n{} {} duplicate groups, {} wasted",
            "\u{26A0}".yellow(),
            report.duplicate_groups_total,
            human_bytes(report.duplicate_bytes_wasted)
        );
        for group in &report.duplicates {
            println!("  {} copies, {} each:", group.paths.len(), human_bytes(group.size));
            for path in &group.paths {
                println!("    - {}", path.display());
            }
        }
        if report.duplicate_groups_total > report.duplicates.len() {
            println!(
                "  ... {} more group(s) not shown",
                report.duplicate_groups_total - report.duplicates.len()
            );
        }
    }

    if report.errors_total > 0 {
        println!("\n{} {} errors:", "\u{2717}".red(), report.errors_total);
        for (path, err) in &report.errors {
            println!("  - {}: {err}", path.display());
        }
        if report.errors_total > report.errors.len() {
            println!("  ... {} more not shown", report.errors_total - report.errors.len());
        }
    }
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
