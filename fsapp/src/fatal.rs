//! §8.2 fatal-error stderr formatting: first line is always
//! `<binary>: error: <top-level message, lowercase, no trailing period>`,
//! blank line, then `Caused by:` only if there's a wrapped cause chain.

/// For errors that already carry their own top-level message (e.g.
/// `fs_config::ConfigError`, whose `Display` impls are already
/// lowercase/no-period per that crate's own error strings) — the source
/// chain (`std::error::Error::source`) becomes the `Caused by:` block.
pub fn print(binary: &str, err: &dyn std::error::Error) {
    eprintln!("{binary}: error: {err}");
    print_cause_chain(err.source());
}

/// For a `file_engine::Error` surfaced mid-operation, where `top_message`
/// is fsapp's own operation-specific context (e.g. `could not copy "src/"
/// to "dst/"`) and the engine error itself becomes `Caused by: 0`.
pub fn print_with_context(binary: &str, top_message: &str, engine_err: &file_engine::Error) {
    eprintln!("{binary}: error: {top_message}");
    print_cause_chain(Some(engine_err as &dyn std::error::Error));
}

fn print_cause_chain(first: Option<&dyn std::error::Error>) {
    let mut lines = Vec::new();
    let mut cur = first;
    while let Some(e) = cur {
        lines.push(e.to_string());
        cur = e.source();
    }
    if lines.is_empty() {
        return;
    }
    eprintln!();
    eprintln!("Caused by:");
    for (i, line) in lines.iter().enumerate() {
        eprintln!("    {i}: {line}");
    }
}
