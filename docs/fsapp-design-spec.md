# fsapp / fset — Design Specification

Status: design finalized, no code written yet. This document is the full context
for implementation. Do not deviate from anything marked **FIXED** without
checking with Eduardo first. Items marked **OPEN** are genuinely undecided and
should be flagged rather than guessed at.

## 1. Overview

Two Rust binaries, one Cargo package (`fsapp`, two `[[bin]]` targets), in a
workspace also holding the shared `fs-config` lib crate. Wraps the
`file-engine` crate (Eduardo's own crate, published under `naut54` on
crates.io):

- **`fsapp`** — the operational CLI: `copy`, `mv`, `sync`, `watch`, `compress`.
- **`fset`** — a companion CLI that reads/writes the shared JSON config file
  used to set persistent defaults for `fsapp`'s flags.

**Revised from the original two-package design** (`fsapp` and `fset` were
initially separate Cargo packages): `dist` generates one release — and one
Homebrew formula — per Cargo *package*, not per binary. Two packages meant
`brew install fsapp` and `brew install fset` were two separate commands.
Merging `fset` into `fsapp` as a second `[[bin]]` (same two executables,
same CLI behavior) makes `dist` treat them as one release, so
`brew install fsapp` alone installs both binaries. `fset`'s source now
lives at `fsapp/src/bin/fset/` — see §2.

Both binaries share the `fs-config` library crate that owns the config
schema, (de)serialization, validation, and file I/O — this is the single
source of truth so `fset` can never write a config that `fsapp` doesn't
understand.

## 2. Workspace layout

```
fs-workspace/
├── Cargo.toml              (workspace; members = fs-config, fsapp)
├── fs-config/               lib: Config struct, serde schema, load/save, validation,
│                            backup/reset logic, path resolution
└── fsapp/                   pkg with two [[bin]] targets:
    ├── src/main.rs           bin "fsapp": copy/mv/sync/watch/compress
    └── src/bin/fset/main.rs  bin "fset": get/set/unset/list/path/edit/reset
```

`fs-config` is a plain library crate (no `[[bin]]`), pulled in as a path
dependency by the `fsapp` package (both binaries in it).

## 3. `file-engine` 2.4.0 — verified public API

**This was verified by actually compiling against the crate**, not just
reading its docs — the crate's public API had a real bug in 1.1.0 (several
`pub` types were unreachable from outside the crate because their containing
modules weren't `pub`), which Eduardo fixed and republished as 1.1.1. All of
the surface below was confirmed to compile against 2.4.0 (built into
`fsapp` 0.8.0 — `remove`, `mv-many`, and `--skip-if-identical` all
exercise it, not just `copy`/`mv`/`sync`/`watch`/`compress`/`analyze`).

Add to `fsapp`'s `Cargo.toml`:

```toml
file-engine = { version = "2.4.0", features = ["sync", "watch", "compress", "checksum", "permissions", "analyze", "remove"] }
```

**2.0.0 upgrade note.** Every public output type — `Progress`,
`StopReason`, `OperationOutcome`, `SyncOutcome` — is now
`#[non_exhaustive]`, so both exhaustive `match`es in `fsapp` need a `_`
arm, and later additions to any of them stop being breaking changes.
Builders, `Handle<T>`, `Error`, and the feature flags are unchanged from
1.x. See §7 for what the release added to the progress renderer.

**2.1.0 → 2.4.0, additive throughout — no breaking changes on top of
2.0.0's.** In order:

- **2.1.0** added `FileEngine::analyze()` / `AnalyzeBuilder` (feature
  `analyze`, on by default) — read-only tree inspection, its own
  `AnalysisHandle`/`AnalysisProgress`/`AnalysisReport`/
  `AnalysisErrorStrategy` types, deliberately not reusing `Handle<T>`/
  `Progress`/`ErrorStrategy` (see §3.4 item 10).
- **2.2.0** replaced `analyze`'s tree walk internals (`walkdir` →
  `jwalk`, multithreaded) and added `AnalyzeBuilder::walk_concurrency`.
  No API removed; `profiler::scan` (feature `operations`) is untouched
  and still uses `walkdir`.
- **2.3.0** added `FileEngine::move_many()` / `MoveManyBuilder` (batches
  several sources into one destination directory under one
  `Handle<OperationOutcome>`) and `.skip_if_identical(bool)` on
  `CopyBuilder`/`MoveBuilder`/`MoveManyBuilder` (feature `checksum`).
  Also fixed two `MoveBuilder` atomic-rename bugs — a missing destination
  parent directory is now created instead of misreported as
  `Error::SourceNotFound`, and `.overwrite(false)` is now actually
  enforced on that fast path instead of deferring to `rename(2)`'s native
  overwrite semantics — both transparent to `fsapp mv`, no flag changes.
- **2.4.0** added `FileEngine::remove()` / `RemoveBuilder` (feature
  `remove`) — the one genuinely new, and only destructive, addition.
  Unlike `analyze`, it reuses the existing `Handle<T>`/`Progress`/
  `ErrorStrategy` machinery (see §3.4 item 11).

(`operations` and `analyze` are enabled by default already; `remove` is
not and must be requested explicitly, same as `sync`/`watch`/`compress`.)

### 3.1 `FileEngine` entry points

```rust
FileEngine::new()
  .copy(source, dest)       -> CopyBuilder      // feature = "operations" (default)
  .move_path(source, dest)  -> MoveBuilder      // feature = "operations" (default)
  .move_many(sources, dest) -> MoveManyBuilder  // feature = "operations" (default); 2.3.0
  .sync(source, dest)       -> SyncBuilder      // feature = "sync"
  .watch(path)               -> WatchBuilder     // feature = "watch"
  .compress(source, dest)   -> CompressBuilder  // feature = "compress"
  .analyze(path)             -> AnalyzeBuilder   // feature = "analyze" (default); 2.1.0
  .remove(path)              -> RemoveBuilder    // feature = "remove"; 2.4.0
```

### 3.2 Builder methods (exhaustive, per builder)

| Method | `CopyBuilder` | `MoveBuilder` | `MoveManyBuilder` | `SyncBuilder` | `WatchBuilder` | `CompressBuilder` |
|---|---|---|---|---|---|---|
| `.overwrite(bool)` | ✅ default `false` | ✅ default `false` | ✅ default `false` | ✅ default `true` | — | — |
| `.skip_if_identical(bool)` (2.3.0, feature `checksum`) | ✅ default `false` | ✅ default `false` | ✅ default `false`, applied per source | ❌ | — | — |
| `.on_error(ErrorStrategy)` | ✅ | ✅ | ✅ | ✅ | — | ✅ |
| `.small_file_threshold(u64)` | ✅ | ✅ | ✅ | ✅ | — | ✅ |
| `.batch_concurrency(usize)` | ✅ | ✅ | ✅ | ✅ | — | ✅ |
| `.max_bytes_per_batch(u64)` | ✅ **copy only** | ❌ | ❌ | ❌ | — | ❌ |
| `.max_files_per_batch(usize)` | ✅ **copy only** | ❌ | ❌ | ❌ | — | ❌ |
| `.batch_sort_order(SortOrder)` | ✅ **copy only** | ❌ | ❌ | ❌ | — | ❌ |
| `.preserve_permissions(bool)` | ✅ `#[cfg(unix)]` only, feature `permissions` | ✅ same cfg | ✅ same cfg | ✅ same cfg | — | ❌ |
| `.allow_filesystem_integrity_risk(bool)` | ✅ | ✅ | ✅ | ✅ | — | ❌ |
| `.diff_strategy(DiffStrategy)` | — | — | — | ✅ **sync only** | — | — |
| `.recursive(bool)` | — | — | — | — | ✅ default `true` | — |
| `.format(CompressFormat)` | — | — | — | — | — | ✅ |
| `.start()` return type | `Result<Handle<OperationOutcome>>` | `Result<Handle<OperationOutcome>>` | `Result<Handle<OperationOutcome>>` | `Result<Handle<SyncOutcome>>` | `Result<WatchHandle>` | `Result<Handle<OperationOutcome>>` |

`.preserve_permissions()` **does not exist as a method at all** on non-Unix
targets (it's `#[cfg(all(unix, feature = "permissions"))]` on the crate side).
Any call site in `fsapp` must be behind `#[cfg(unix)]`, with a runtime warning
on other platforms if the user asked for it. `MoveManyBuilder` is
constructed via `.move_many(sources, dest)`, not `.new()` — `dest` there is
always the directory every source lands *inside*, never a rename target;
this is also true of `.copy()`/`.move_path()`'s own `dest` (see §3.4
item 12), it's just easiest to see once there's more than one source.

### 3.2b `AnalyzeBuilder` / `RemoveBuilder` methods

Neither shares a method surface with §3.2's copy-family builders in any
meaningful way — different filters, different error-strategy types,
different output — so they get their own table rather than more columns
bolted onto the one above.

| Method | `AnalyzeBuilder` (2.1.0) | `RemoveBuilder` (2.4.0) |
|---|---|---|
| `.extensions(impl IntoIterator<...>)` | ✅ | ✅ |
| exclude-globs method | `.exclude_globs(...)` | `.exclude(...)` — **different name, same shape**; a call site templated on "the analyze/remove filter API" cannot share this one line |
| `.min_size(u64)` / `.max_size(u64)` | ✅ | ✅ |
| `.modified_after(SystemTime)` / `.modified_before(SystemTime)` | ✅ (not currently wired into `fsapp analyze`'s CLI — see §11) | ✅ |
| `.max_depth(usize)` | ✅ | ✅ |
| `.follow_symlinks(bool)` | ✅ default `false` | ✅ default `false` |
| `.top_n_largest(usize)` | ✅ default `10` | — |
| `.walk_concurrency(usize)` (2.2.0) | ✅ default: available parallelism | — |
| `.detect_mime_types(bool)` (feature `analyze`) | ✅ default `false` | — |
| `.detect_duplicates(bool)` / `.hash_concurrency(usize)` (feature `checksum`) | ✅ | — |
| `.max_reported_errors(usize)` / `.max_reported_duplicates(usize)` | ✅ (not currently wired into `fsapp analyze`'s CLI — see §11) | — |
| `.on_error(...)` | ✅ own `AnalysisErrorStrategy` (`ContinueAndCollect`/`AbortOnError` only — no `Undo`, analysis never writes anything) | ✅ shares `planner::ErrorStrategy` with the copy-family builders |
| `.batch_concurrency(usize)` | — (no batching; only the walk itself is parallelized, via `.walk_concurrency`) | ✅ |
| `.dry_run(bool)` | — | ✅ default `true` |
| `.hard_delete(bool)` | — | ✅ default `false` (platform trash) |
| `.allow_unfiltered_delete(bool)` | — | ✅ default `false` — see §3.4 item 13 |
| `.start()` return type | `Result<AnalysisHandle>` | `Result<Handle<RemoveOutcome>>` |

### 3.3 Public types (all confirmed reachable at crate root in 2.4.0)

```rust
pub use file_engine::{
    Error, Result,                          // error.rs
    Handle, Progress,                        // operations feature
    EtaEstimator,                            // added in 2.0.0
    WatchEvent, WatchEventKind, WatchHandle, // watch feature
    CopyBuilder, MoveBuilder,
    MoveManyBuilder,                         // operations feature — 2.3.0
    SyncBuilder, SyncOutcome,
    CompressBuilder, CompressFormat,
    WatchBuilder,
    AnalyzeBuilder, AnalysisHandle, AnalysisProgress,
    AnalysisReport, AnalysisErrorStrategy,   // analyze feature (default) — 2.1.0
    RemoveBuilder, RemoveOutcome,            // remove feature — 2.4.0
    ErrorStrategy, OperationOutcome, SortOrder, StopReason, // planner — fixed in 1.1.1
    DiffStrategy,                            // operations::diff — fixed in 1.1.1
    Entry,                                    // profiler — fixed in 1.1.1
};
```

```rust
pub enum ErrorStrategy { ContinueAndCollect (default), AbortOnError, Undo }
pub enum SortOrder { Ascending, Descending }
pub enum DiffStrategy { SizeAndModifiedTime (default), Checksum } // Checksum requires feature "checksum"
pub enum CompressFormat { Zip, Gzip }
#[non_exhaustive] pub enum StopReason { Fatal, AbortOnError, Cancelled, Undo }
// analyze's own error-strategy enum (2.1.0) — smaller than ErrorStrategy on
// purpose: analysis never writes anything, so Undo has no meaning here.
#[derive(Default)] pub enum AnalysisErrorStrategy { ContinueAndCollect (default), AbortOnError }

#[non_exhaustive]
pub struct OperationOutcome {
    pub succeeded: Vec<Entry>,
    pub failed: Vec<(Entry, Error)>,
    pub cleanup_failed: Vec<(Entry, Error)>,   // move only — copy succeeded, source delete failed
    pub stopped_early: Option<StopReason>,
    pub directories_failed: Vec<(PathBuf, Error)>, // preserve_permissions best-effort failures
    pub duration: Duration,                    // added in 2.0.0; per phase for sync
}

#[non_exhaustive]
pub struct SyncOutcome {
    pub copy: OperationOutcome,
    pub delete: OperationOutcome,
}

// remove feature — 2.4.0. Also the return type Handle<T> is built around
// (RemoveBuilder::start() -> Result<Handle<RemoveOutcome>>), unlike
// AnalysisReport below, which has its own dedicated handle.
#[non_exhaustive]
pub struct RemoveOutcome {
    pub succeeded: Vec<Entry>,   // actually removed; empty when dry_run
    pub previewed: Vec<Entry>,   // matched but untouched; only non-empty when dry_run (the default)
    pub failed: Vec<(Entry, Error)>,
    pub stopped_early: Option<StopReason>,
    pub duration: Duration,
}

// analyze feature (default) — 2.1.0, including the checksum-gated
// duplicate fields. AnalysisHandle resolves to this, not OperationOutcome.
#[non_exhaustive]
pub struct AnalysisReport {
    pub file_count: usize,
    pub dir_count: usize,
    pub total_size: u64,
    pub largest_files: Vec<Entry>,           // capped at top_n_largest; Entry here is analysis::report::Entry, not profiler::Entry
    pub by_extension: HashMap<String, ExtensionStats>, // "" key = no extension
    pub by_mime: HashMap<String, MimeStats>, // empty unless detect_mime_types(true)
    pub age_buckets: AgeBuckets,             // under_1_day/week/month/year, older, unknown
    pub errors: Vec<(PathBuf, Error)>,       // sample, capped at max_reported_errors
    pub errors_total: usize,                 // uncapped
    pub duplicates: Vec<DuplicateGroup>,     // feature checksum; sample, capped at max_reported_duplicates
    pub duplicate_groups_total: usize,       // feature checksum; uncapped
    pub duplicate_bytes_wasted: u64,         // feature checksum; uncapped, sums every group found
    pub duration: Duration,
}

// analyze feature (default) — 2.1.0. Deliberately NOT crate::progress::Progress
// (see AnalysisHandle below) — smaller, and analysis-specific.
#[non_exhaustive]
pub enum AnalysisProgress {
    EntryAnalyzed { path: PathBuf },         // once per matched entry, post-filter
    EntryHashed { path: PathBuf },           // feature checksum; once per file during duplicate detection
}

#[non_exhaustive]
pub enum Progress {
    // Added in 2.0.0. Emitted once per phase, before the directory pre-pass.
    Planned { directories: usize, small_files: usize, small_bytes: u64,
              large_files: usize, large_bytes: u64, small_file_threshold: u64 },
    Started { bytes_total: Option<u64>, entries_total: usize }, // can fire more than once (sync: copy phase, then delete phase)
    EntryStarted { entry: Entry },
    // Added in 2.0.0. Large (streamed) entries only, sampled every 250ms.
    EntryProgress { entry: Entry, bytes_copied: u64 },
    EntryCompleted { entry: Entry },
    EntryFailed { entry: Entry },     // NOTE: does not carry the Error — only available later via outcome.failed
    DirectoriesStarted { total: usize },
    DirectoryCompleted { path: PathBuf },
    DirectoryFailed { path: PathBuf },
}

pub struct Entry { pub path: PathBuf, pub relative_path: PathBuf, pub size: u64, pub modified: Option<SystemTime> }

pub enum WatchEventKind { Created, Modified, Removed, Other }
pub struct WatchEvent { pub kind: WatchEventKind, pub paths: Vec<PathBuf> }
```

`Handle<T>`: `.progress() -> &mut impl Stream<Item = Progress>`, `.cancel()`
(cooperative), implements `Future<Output = Result<T>>`.

`WatchHandle`: `.events() -> &mut impl Stream<Item = WatchEvent>`, `.cancel()`,
implements `Future<Output = Result<()>>`. **Structurally different from
`Handle<T>`** — no `Progress`, no bounded outcome, the future just resolves to
`Ok(())` on clean cancel or `Err` on a fatal watcher error (e.g. path removed).

`AnalysisHandle` (2.1.0): `.progress() -> &mut impl Stream<Item =
AnalysisProgress>`, `.cancel()`, implements `Future<Output =
Result<AnalysisReport>>`. **A third, independently-typed handle** — same
shape as `Handle<T>` (stream + cancel + awaitable) but a distinct
concrete type on the same precedent `WatchHandle` already sets, not
`Handle<AnalysisReport>`. `RemoveBuilder`, by contrast, resolves to the
existing `Handle<RemoveOutcome>` — see §3.4 item 11 for why the two new
2.x builders made opposite choices here.

`Error` variants (via `thiserror`): `SourceNotFound`, `DestExists`,
`Cancelled`, `NoSpace { needed, available }`, `PermissionDenied`, `Io { path,
source }`, `UnknownCompressFormat` (compress), `GzipRequiresFile` (compress),
`CaseCollision`, `FileTooLargeForDest`, `ReservedName`,
`FilesystemIntegrityRisk { filesystem }` — the last one is the only "whole
destination" fatal error not tied to a specific entry. **2.4.0 adds**
`RemoveCriteriaRequired` (feature `remove`) — returned by the awaited
`Handle`, not by `.start()` itself, when `RemoveBuilder` has no filter
criterion set and `.allow_unfiltered_delete(true)` wasn't passed — and
`TrashFailed { path, source }` (feature `remove`), when the platform has
no trash service and `.hard_delete(true)` wasn't passed to fall back to.

### 3.4 Hard behavioral constraints (do not design around these incorrectly)

1. **`watch` has no `Progress` at all** — it's a fundamentally different
   handle type with an indefinite event stream. It cannot share a renderer
   with copy/mv/sync/compress.
2. **Same-filesystem `move_path` never touches the batching pipeline** — it's
   a single atomic `rename()`, so it emits **no `Progress` events**. Only the
   cross-device fallback goes through the normal pipeline. `fsapp mv` must
   handle "no progress events arrived, but the operation succeeded" as a
   normal case, not an error.
3. **`sync` reports two separate phases** (`Progress::Started` fires once for
   copy, once for delete). If the copy phase aborts/cancels, the delete phase
   never runs — `SyncOutcome.delete` can legitimately be all-default/empty
   even though nothing went wrong with deletion itself.
4. **`EntryFailed` doesn't carry the error** — only the `Entry`. The actual
   `file_engine::Error` for a failed entry is only available once the
   `Handle` resolves, via `outcome.failed: Vec<(Entry, Error)>`. A live
   progress renderer cannot show *why* something failed until the end.
5. **Fatal errors stop everything regardless of `ErrorStrategy`**:
   `Error::Cancelled`, `Error::NoSpace`, `Error::FilesystemIntegrityRisk`.
   Everything else is per-entry and respects `ErrorStrategy`.
6. **Cancellation is cooperative at the batch/stream level, not per-file** —
   a Ctrl+C during a large file write lets that file finish.
7. **`DiffStrategy::Checksum` only exists when the `checksum` feature is
   enabled** on `file-engine` (it's a `#[cfg(feature = "checksum")]` enum
   variant, not a runtime check).
8. **Every output type is `#[non_exhaustive]` as of 2.0.0** — `Progress`,
   `StopReason`, `OperationOutcome`, `SyncOutcome`. Matches need a `_` arm;
   the two outcome structs can only be built via `Default::default()`
   followed by field assignment, which matters for test fixtures.
9. **Dispatch order changed in 2.0.0**: the smallest large file now runs
   before the small-file batches instead of after all of them. So the
   renderer sees a streamed entry early rather than at ~95% elapsed. Don't
   assume `Progress` events arrive grouped small-then-large.
10. **`analyze` has no `Progress` at all either** — like `watch`, but for
    an unrelated reason: `AnalysisHandle`/`AnalysisProgress` live behind
    the `analyze` feature, which doesn't require `operations` (the
    feature `Handle`/`Progress` live behind). Genericizing `Handle<T>`
    over the progress type would couple `analyze`-only builds to
    `operations` for no benefit to existing callers. `fsapp`'s
    `progress::drive_analyze` is consequently a third renderer, not a
    reuse of `drive`'s — see §7.
11. **`remove`, unlike `analyze`, reuses `Handle<T>`/`Progress`/
    `ErrorStrategy` wholesale** — `RemoveBuilder::start()` returns
    `Result<Handle<RemoveOutcome>>`, the same `Handle<T>` copy/mv/sync
    use, because `remove` is gated on the `operations` feature already
    being available in the same build (unlike `analyze`, which
    deliberately isn't). Practical effect: `fsapp`'s existing
    `progress::drive()` needs no changes at all to drive a `remove`
    operation — confirmed by `main.rs::run_remove` doing exactly that.
    In dry-run (the default), the spawned task returns before ever
    calling `dispatch()`, so the progress stream closes immediately with
    zero events — same "no events is not an error" shape as constraint 2.
12. **`copy`/`move_path`/`move_many`'s `dest` is always a container
    directory sources land *inside*, basename preserved — never a
    `cp src dst-file`-style rename target**, confirmed against the
    crate's own `pipeline.rs` test fixtures (a single source file lands
    at `dest.join(source.file_name())`, not at `dest` itself). Easy to
    assume otherwise from Unix `cp`/`mv` muscle memory; not obvious from
    `fsapp --help` either.
13. **`RemoveBuilder`'s empty-filter check runs before `dry_run` is
    consulted, not after** — `.start()`'s `Handle` resolves to
    `Err(Error::RemoveCriteriaRequired)` for an unfiltered `PATH` with no
    `.allow_unfiltered_delete(true)`, even in dry-run. A preview of "every
    entry under this root" is refused exactly as an actual unfiltered
    delete would be, not treated as a safe no-op case worth allowing
    through.

## 4. `fsapp` — CLI surface

### 4.1 Global flags (apply to all subcommands)

| Flag | Type | Meaning |
|---|---|---|
| `-v`, `-vv`, `-vvv` | count | log verbosity: default = warn, `-v` = info, `-vv` = debug, `-vvv` = trace |
| `-q`, `--quiet` | bool | suppress the progress bar; logging still follows `-v` |
| `--config <path>` | path | override the config file location for this invocation |
| `--no-update-check` | bool | skip the automatic check for a newer release (§12) |

Shell completions (`fsapp completions`, `fset completions`) are designed
in [`completions-design.md`](completions-design.md) rather than here —
most of that design is about packaging channels, not CLI surface.

### 4.2 Shared arg groups

**`BatchArgs`** — flattened into `copy`, `mv`, `sync`, `compress` (the four
that go through the batching pipeline):

| Flag | Maps to |
|---|---|
| `--small-file-threshold <BYTES>` | `.small_file_threshold(u64)` |
| `--batch-concurrency <N>` | `.batch_concurrency(usize)` |
| `--on-error <continue\|abort\|undo>` (default `continue`) | `.on_error(ErrorStrategy)` |

**`FsSafetyArgs`** — flattened into `copy`, `mv`, `sync` **only** (compress
and watch don't have these methods on their builders):

| Flag | Maps to |
|---|---|
| `--preserve-permissions` | `.preserve_permissions(true)`, `#[cfg(unix)]`-gated call site |
| `--allow-fs-integrity-risk` | `.allow_filesystem_integrity_risk(true)` |

### 4.3 Per-subcommand specs

**`fsapp copy <SOURCE> <DEST>`**
`BatchArgs` + `FsSafetyArgs`, plus:
- `--overwrite` (bool, default `false`)
- `--max-bytes-per-batch <BYTES>`
- `--max-files-per-batch <N>`
- `--sort-order <asc|desc>` (default `desc`)

**`fsapp mv <SOURCE> <DEST>`**
`BatchArgs` + `FsSafetyArgs`, plus:
- `--overwrite` (bool, default `false`)
(No batching-fine-tuning flags — `MoveBuilder` doesn't expose them.)

**`fsapp sync <SOURCE> <DEST>`**
`BatchArgs` + `FsSafetyArgs`, plus:
- `--no-overwrite` (bool) — inverts the builder's default of `true`
- `--checksum` (bool) — maps to `.diff_strategy(DiffStrategy::Checksum)`;
  absent means `DiffStrategy::SizeAndModifiedTime` (the builder default)

**`fsapp watch <PATH>`**
No `BatchArgs`, no `FsSafetyArgs` — watch never touches the pipeline.
- `--no-recursive` (bool) — inverts the builder's default of `true`

**`fsapp compress <SOURCE> <DEST>`**
`BatchArgs` only, plus:
- `--format <zip|gzip>` (optional; the crate infers from `DEST`'s extension
  if omitted)

### 4.4 Summary table of asymmetry

| | copy | mv | sync | watch | compress |
|---|---|---|---|---|---|
| BatchArgs | ✅ | ✅ | ✅ | ❌ | ✅ |
| FsSafetyArgs | ✅ | ✅ | ✅ | ❌ | ❌ |
| overwrite (own flag) | ✅ default false | ✅ default false | ✅ (`--no-overwrite`, default true) | — | — |
| fine batching (bytes/files/sort) | ✅ | ❌ | ❌ | — | ❌ |
| diff strategy | — | — | ✅ | — | — |
| archive format | — | — | — | — | ✅ |

No flag is shared across all five subcommands except the globals (`-v`,
`-q`, `--config`). `watch` is fully isolated from the rest.

## 5. `fset` — CLI surface

```
fset get <section>.<key>              # print the current value, or "unset" if absent
fset set <section>.<key> <value>      # validated against the same enum/type used by fsapp's clap ValueEnum
fset unset <section>.<key>            # remove the key — reverts to the file-engine builder default
fset list [<section>]                 # dump current JSON (optionally scoped to one section)
fset path                             # print the resolved config file path (debugging/scripting)
fset edit                             # open $EDITOR on the file; re-validate on save
fset reset [<section>]                # reset the whole file (or one section) to {}; always backs up first
```

Validation must use the **same enum types** as `fsapp`'s `clap::ValueEnum`
definitions — single source of truth in `fs-config`, so `fset set
copy.on-error <value>` can never accept something `fsapp copy --on-error
<value>` would reject, or vice versa.

There is intentionally **no `set <key> null`** — only `unset` reverts a key
to the builder default. Keep the schema simple: a key is either present with
a concrete value, or absent.

## 6. Config file (`fs-config` crate)

### 6.1 Path resolution (first match wins, no merging across sources)

1. `--config <path>` explicit flag on the `fsapp`/`fset` invocation
2. `FSAPP_CONFIG` environment variable
3. `dirs::config_dir()` + `/fsapp/config.json`:
   - macOS: `~/Library/Application Support/fsapp/config.json`
   - Linux: `$XDG_CONFIG_HOME/fsapp/config.json`, falling back to
     `~/.config/fsapp/config.json` if `XDG_CONFIG_HOME` is unset
   - Windows: `%APPDATA%\fsapp\config.json`
4. If `dirs::config_dir()` returns `None` → fatal error, exit `5`, message
   telling the user to pass `--config` or set `FSAPP_CONFIG`

The config directory is created (`create_dir_all`) only when `fset set` or
`fset reset` needs to write. `fsapp` in read-only mode never touches disk if
the file doesn't exist — it just proceeds with `{}` (all builder defaults).

### 6.2 Value precedence when resolving any single flag

**CLI flag explicitly passed > `FSAPP_*` env var > value in `config.json` >
`file-engine` builder default.**

### 6.3 Schema — kebab-case throughout, matching the CLI flags 1:1

```jsonc
{
  "global": { "verbosity": 0, "quiet": false },
  "copy": {
    "on-error": "continue", "small-file-threshold": null, "batch-concurrency": null,
    "preserve-permissions": false, "allow-fs-integrity-risk": false,
    "overwrite": false, "max-bytes-per-batch": null, "max-files-per-batch": null, "sort-order": "desc"
  },
  "mv": {
    "on-error": "continue", "small-file-threshold": null, "batch-concurrency": null,
    "preserve-permissions": false, "allow-fs-integrity-risk": false, "overwrite": false
  },
  "sync": {
    "on-error": "continue", "small-file-threshold": null, "batch-concurrency": null,
    "preserve-permissions": false, "allow-fs-integrity-risk": false,
    "no-overwrite": false, "checksum": false
  },
  "watch": { "no-recursive": false },
  "compress": {
    "on-error": "continue", "small-file-threshold": null, "batch-concurrency": null, "format": null
  },
  "update": { "no-check": false }
}
```

`update.no-check` is stated in the negative deliberately, alongside
`sync.no-overwrite` and `watch.no-recursive`: the check is on by default,
and the negative form is what lets "absent everywhere" mean "on" while
still resolving through the same `resolve_bool` path as every other
boolean. It also gives the env override its name for free —
`FSAPP_UPDATE_NO_CHECK=true` — with no bespoke variable. See §12.

Every section is optional; every key within a section is optional; `{}` is a
fully valid file meaning "all builder defaults everywhere". `fset set` only
ever writes the specific key the user touched — never dumps the whole struct.

### 6.4 Validation rules

| Key(s) | Type | Rule |
|---|---|---|
| `verbosity` | `u8` | `0..=3` |
| `on-error` | string enum | `"continue" \| "abort" \| "undo"` |
| `sort-order` | string enum | `"asc" \| "desc"` |
| `format` | string enum or `null` | `"zip" \| "gzip" \| null` |
| `small-file-threshold`, `max-bytes-per-batch` | `u64` or `null` | if present, `> 0` |
| `max-files-per-batch`, `batch-concurrency` | `u64` or `null` | if present, `>= 1` |
| all boolean keys | `bool` | — |

**Unknown keys are a validation error**, not silently ignored
(`#[serde(deny_unknown_fields)]` on every section struct). A typo in the JSON
must surface, not be swallowed — the goal is that a user never believes a
setting is active when it silently wasn't applied.

### 6.5 Invalid-JSON recovery flow

Triggered by either a JSON parse failure or a schema validation failure
(unknown field, wrong type, out-of-range value, invalid enum string) — both
routes lead to the same flow, no distinction made to the user beyond the
error message shown.

**Interactive (stdin is a tty):**

```
The config file at ~/Library/Application Support/fsapp/config.json is invalid:
  <serde_json error message with line/column, or validation error>

What do you want to do?
  [1] Exit without touching the file
  [2] Reset to default configuration

>
```

- Option 1 → exit code `3`, file untouched.
- Option 2 → back up the broken file first (see 6.6), write a fresh `{}`,
  and **continue the current invocation** using those defaults (no need to
  re-run the command).

**Non-interactive (no tty — pipes, CI, cron):** never prompt (would hang
forever). Abort immediately with exit code `3` and a message that states both
options in writing, including the exact non-interactive fix: `fset reset`.

**Explicit `--config <path>`:** same two-option flow, but the message names
the specific path that's broken (not the default fsapp path), so the user
doesn't go looking for the wrong file.

### 6.6 Backup naming

Same directory as `config.json`, same timestamp format in both cases:

```
config.json.invalid-1735776000     # broken/unparseable file, before an interactive or forced reset
config.json.bak-1735776000         # valid file, backed up before a manual `fset reset` on purpose
```

- Timestamp = Unix seconds, UTC, no milliseconds.
- On a same-second collision (rapid repeated invocations, tests), append
  `-2`, `-3`, ... rather than overwrite — a backup is never destroyed by a
  naming collision.

## 7. Progress & logging

- Verbosity (`-v` count) drives a `tracing` subscriber level: `0` = warn,
  `1` = info, `2` = debug, `3` = trace. Logging code should just call
  `tracing::info!`/`debug!`/etc. unconditionally at the right semantic
  level — let the subscriber filter, don't hand-roll `if verbosity >= N`
  checks in the progress renderer.
- The `indicatif` progress bar is shown whenever stderr is a tty **and**
  `--quiet` wasn't passed — independent of verbosity level, so "both,
  depending on verbosity" (Eduardo's requirement) means: the bar is a
  presentation layer, the log level is a separate axis.
- Three different renderers are required, not one — `progress.rs`'s
  `drive`/`drive_watch`/`drive_analyze`:
  - A generic one over `Stream<Item = Progress>` (`drive`) for
    `copy`/`mv`/`mv-many`/`sync`/`compress`, **and also `remove`** — it
    reuses `Handle<T>`/`Progress` (§3.4 item 11), so it needed no changes
    at all to add `remove` on top of the original four.
  - A separate one over `Stream<Item = WatchEvent>` (`drive_watch`) for
    `watch`, which has no bar (indefinite stream) — just formatted log
    lines per event, and a clean exit on Ctrl+C via `.cancel()`.
  - A third, spinner-only one over `Stream<Item = AnalysisProgress>`
    (`drive_analyze`) for `analyze` (2.1.0) — no `Progress::Planned`
    equivalent exists for a read-only walk that doesn't know the tree
    size ahead of time, so there's no bar *length* to set, just a ticking
    spinner and a live count of matched/hashed entries.
- `mv`'s same-filesystem fast path may finish with **zero** `Progress`
  events — the renderer must treat "no events, but `Handle` resolved `Ok`"
  as a normal, successful, silent case, not a bug. `remove` in dry-run
  (the default) hits the same shape for a different reason: the spawned
  task returns before calling `dispatch()` at all (§3.4 item 11).

### 7.1 What the bar shows (file-engine 2.0.0–2.4.0)

The bar's *position* is entry counts, as before. Everything else on the
line comes from 2.0.0:

- **Length is set at `Progress::Planned`**, which arrives before the
  directory pre-pass — earlier than `Started`. On a large tree that
  pre-pass can run for a minute on its own, and previously the bar showed
  `0/0` for all of it. `Started` still re-sets the length and resets the
  position, which is what gives `sync` a fresh bar for its delete phase.
- **`Progress::EntryProgress`** (destination sampled every 250ms, large
  entries only) drives the message: one streaming file shows `name NN%`,
  several show `N large files <copied>/<total>`. Without it a lone
  multi-gigabyte file moved nothing on screen for the entire copy, since
  the entry count doesn't advance until it completes.
- **`EtaEstimator`** is fed *every* event — including the ones that don't
  touch the bar, since each one bounds a span of wall time — and renders
  into the bar's prefix as `ETA 1m23s · 512.0 MiB/s`. `estimate()`
  returning `None` prints nothing rather than a fabricated number.
  Expect the estimate to start pessimistic on a mixed workload and tighten
  once the first large file lands: until then the estimator stands in the
  overall byte rate, which carries small-file per-file overhead, for the
  streaming rate it hasn't measured yet.
- **A steady tick** (100ms) keeps the spinner alive between events.
- The template uses `{wide_bar}`, not a fixed width — the message and ETA
  are long enough that a fixed 40-column bar wrapped the line.
- **`OperationOutcome.duration`** ends the §8.3 summary line (`... in
  1.1s`). For `sync` the two phases are timed separately and don't sum to
  the whole run; a delete phase skipped because the copy phase stopped
  early reports zero.
- A copy the filesystem satisfies by cloning (APFS reflink) completes
  before the first 250ms sample and emits **no** `EntryProgress` at all.
  That's correct, not a missing-events bug — verified: 1.2 GiB same-volume
  in 0.4s, no samples; the same tree across volumes samples normally.
- **`remove`'s bar is this same one, unchanged** (§3.4 item 11) — a
  removal is not size-scaled the way a copy/move is (nothing to stream),
  so `Progress::Planned`'s `small_files`/`large_files` split still
  arrives, but every matched entry lands in `small_files` regardless of
  its actual size, and `EntryProgress` (the in-flight-file sampler) never
  fires for it. `RemoveOutcome` has no `.duration` shown mid-run either —
  same as every other operation, only known once the `Handle` resolves.
- **`analyze`'s bar (`drive_analyze`) is a different one, not this one**
  (2.1.0) — `indicatif::ProgressBar::new_spinner()` rather than
  `{wide_bar}`, no `set_length`/`set_position` at all, message set to
  `"{N} entries analyzed"` (or `"{N} files hashed for duplicates"` while
  `.detect_duplicates(true)`'s hashing phase runs) on each
  `AnalysisProgress` event instead of a percentage or byte rate. No ETA:
  `EtaEstimator` is built around `Progress`, which `analyze` doesn't
  produce.

## 8. Output & exit codes

### 8.1 Exit code table

| Code | Meaning | Emitted by |
|---|---|---|
| `0` | Full success — `outcome.failed` empty, `stopped_early: None` | fsapp / fset |
| `1` | Completed with partial failures (`ContinueAndCollect` with failed entries, or `stopped_early: Some(AbortOnError \| Undo)`) | fsapp |
| `2` | CLI usage error — reserved by `clap`'s own default behavior | fsapp / fset |
| `3` | Invalid config, not repaired (non-interactive abort, or user chose "exit" at the prompt) | fsapp / fset |
| `4` | Fatal engine error propagated as `Err` (unsolicited `Cancelled`, `NoSpace`, `FilesystemIntegrityRisk` without `--allow-fs-integrity-risk`, or a pre-flight `Io`/`SourceNotFound`/`DestExists`/`PermissionDenied` before the pipeline starts) | fsapp |
| `5` | I/O error unrelated to the engine — can't read/write `config.json` beyond parsing, backup creation failed, permissions on the config directory, or `update-check` could not reach GitHub | fsapp / fset |
| `130` | User cancelled — `SIGINT`/Ctrl+C, standard `128 + SIGINT(2)` convention | fsapp |

On Ctrl+C: call `.cancel()` on the `Handle`/`WatchHandle` (cooperative — lets
the in-flight batch finish), then exit `130`. This is **not** reinterpreted
as exit `1` even though `outcome.failed` may be non-empty as a result.

### 8.2 Fatal errors (exit `2`, `3`, `4`, `5`) → **stderr**

Rendered with `anyhow`'s `{:?}` (Debug) formatting, not `{}`, to preserve the
cause chain:

```
fsapp: error: could not copy "src/" to "dst/"

Caused by:
    0: destination filesystem (exfat) has a known write-integrity issue on this platform
```

Fixed format: first line is always `<binary>: error: <top-level message,
lowercase, no trailing period>`, blank line, then `Caused by:` **only** if
there's an intermediate context chain — omit the whole block if the error has
no wrapped cause.

### 8.3 Operation summary (exit `0` or `1`) → **stdout**

Never stderr — this is the expected result of an invocation that technically
succeeded as a process, even if some entries within it failed.

```
✓ 128 entries copied (42.3 MiB)
✗ 3 entries failed:
  - src/data/report.csv: permission denied: src/data/report.csv
  - src/data/big.bin: insufficient disk space: needed 104857600 bytes, available 52428800 bytes
  - src/tmp/lock: io error on src/tmp/lock: Resource temporarily unavailable (os error 35)
⚠ stopped early: reached --on-error abort after the 4th failure
```

- `✓`/`✗`/`⚠` colored green/red/yellow via `colored`, auto-disabled when
  stdout isn't a tty, and respecting `NO_COLOR`.
- Each failure line is `entry.relative_path.display()` + `": "` +
  `file_engine::Error`'s own `Display` (from its `thiserror` impl) — don't
  reformat the engine's own error text.
- `mv` only: extra block `↺ N entries copied but source cleanup failed (data
  duplicated, not lost):` when `cleanup_failed` is non-empty.
- Any operation with `--preserve-permissions`: extra block `⚠ N directories:
  permission bits not applied:` when `directories_failed` is non-empty — this
  **never** affects the exit code (it's best-effort by the crate's own
  design).
- `sync` only: two full summary blocks, headed `Copy phase:` / `Delete
  phase:`.

## 9. Confirmed decisions checklist (do not re-litigate without asking)

- Binary names: `fsapp` (operations) and `fset` (config) — **FIXED**, chosen
  explicitly by Eduardo after being warned about the collision risk of a
  bare `fs` binary name.
- Scope: all five operations (copy, mv, sync, watch, compress) — **FIXED**.
- Progress rendering: indicatif bar + tracing logs, both gated
  independently (bar by tty/`--quiet`, logs by `-v`) — **FIXED**.
- Config file casing: kebab-case, matching CLI flags exactly — **FIXED**.
- Config path resolution: platform-idiomatic via `dirs::config_dir()`, not
  forced XDG — **FIXED**, explicitly chosen over the Linux-only alternative.
- `fset set <key> null`: rejected — only `unset` reverts to default —
  **FIXED**.
- Invalid JSON recovery: two-option prompt (exit / reset) when interactive;
  non-interactive always aborts with exit `3` — **FIXED**.
- Distribution channels: shell installer + Homebrew tap via `dist`, plus a
  lightweight downloadable `.deb` via `cargo-deb` (no self-hosted apt
  repository) — **FIXED**.
- GitHub owner/repo: `naut54/fsapp` — **FIXED**. Used throughout
  `[workspace.metadata.dist]`, the installer curl URL, the `.deb` wget URL,
  and `Cargo.toml` `repository` fields.
- Homebrew tap repo: `naut54/homebrew-tap` (create once, empty, before first
  release) — **FIXED**.
- `fset edit` validation order: validate the edited buffer **before**
  replacing the on-disk file — if invalid, reject and reopen `$EDITOR` with
  the bad content still present. Never writes a broken file to disk; this is
  a distinct retry loop from the §6.5 invalid-JSON recovery flow (that flow
  is for a config file that becomes invalid some other way, e.g. hand-edited
  outside `fset`) — **FIXED**.
- Rust edition: 2021. MSRV: `rust-version = "1.88"` pinned explicitly in the
  workspace `Cargo.toml`, matching `file-engine`'s own floor — **FIXED**.
- License: `MIT`, set on all three crates (`fs-config`, `fsapp`, `fset`) —
  **FIXED**.
- Publishing: binaries only. `fs-config`, `fsapp`, `fset` are **not**
  published to crates.io — the only distribution channels are the three from
  §10 (shell installer, Homebrew tap, `.deb`) — **FIXED**.

## 10. Distribution & installation

Goal: `fsapp` and `fset` end up on `PATH` via three channels — a one-line
shell install, Homebrew, and a downloadable `.deb` — without hand-rolling
release infrastructure.

The design rationale is below; for the actual procedures — updating an
installed copy, upgrading the `file-engine` dependency, and cutting a
release — see [`updating.md`](updating.md).

### 10.1 Tooling: `dist` (formerly `cargo-dist`)

Actively maintained by axodotdev (current stable series: 0.30.x). Configured
once via `[workspace.metadata.dist]` in the workspace root `Cargo.toml`
(`dist init` generates the initial config interactively). On every
`git tag vX.Y.Z` push, the GitHub Actions workflow `dist generate --mode ci`
produces:

1. **Release binaries** for the target platforms — at minimum
   `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`;
   add `x86_64-unknown-linux-musl` if these need to run on OVH VPS
   distributions without matching the exact glibc version used to build.
   Both `fsapp` and `fset` binaries ship in the same tarball per target
   (workspace with two `[[bin]]`s — `dist` handles multi-binary workspaces
   natively, no special config needed beyond listing both in
   `[workspace.metadata.dist] members`).
2. **Checksums** (sha256) per artifact, uploaded alongside.
3. **A shell installer script**, uploaded to the release as
   `fsapp-installer.sh`. This is the "wget" channel:
   ```bash
   curl -fsSL https://github.com/naut54/fsapp/releases/latest/download/fsapp-installer.sh | sh
   ```
   Detects OS/arch, downloads the matching tarball, verifies the checksum,
   installs both binaries to `~/.cargo/bin` (or `$FSAPP_INSTALL_DIR` if the
   user overrides it — `dist` auto-generates this env var name from the
   package name).
4. **A Homebrew formula**, auto-committed to a separate tap repository
   (`naut54/homebrew-tap` — needs to be created once, empty, before first
   release). Formula points at the prebuilt binary artifacts, **not** a
   from-source `cargo install` build — deliberate, since `file-engine`'s
   MSRV is 1.88 and a from-source formula would fail on any Homebrew
   install-time Rust older than that. End-user flow:
   ```bash
   brew tap naut54/tap
   brew install fsapp
   ```

`dist` does not produce `.deb` packages — that's a separate step (10.2).

### 10.2 `.deb` — lightweight, no hosted repository

**FIXED**: lightweight route chosen over a self-hosted signed apt
repository. A separate GitHub Actions workflow (`.github/workflows/deb.yml`
— deliberately *not* a job inside `dist`'s own `release.yml`, since that
file is self-checked by `dist` and any hand-added job there gets rejected
as "out of date"; triggered via `workflow_run` on `release.yml`'s
completion rather than `release: published`, since GitHub Actions doesn't
fire repo events for a release created with the default `GITHUB_TOKEN`)
runs `cargo deb -p fsapp` and uploads the result as an additional asset on
the same GitHub Release `dist` already created. Since `fset` is a second
`[[bin]]` in the `fsapp` package rather than a separate crate (§1), this
produces a **single** `fsapp_<version>_<arch>.deb` containing both
binaries — not two separate `.deb` files. `[package.metadata.deb]` in
`fsapp/Cargo.toml` lists both binaries under `assets`.

Install flow (this satisfies "apt" literally — `apt` can install a local
`.deb` file directly, resolving system dependencies, unlike bare `dpkg -i`):

```bash
wget https://github.com/naut54/fsapp/releases/latest/download/fsapp_0.1.1-1_amd64.deb
sudo apt install ./fsapp_0.1.1-1_amd64.deb
```

No GPG signing key, no self-hosted `Packages`/`Release` index, no
`apt update && apt install fsapp` one-liner — the user re-downloads the
`.deb` on every version bump. Explicitly rejected: a full `aptly`/`reprepro`
repo on the OVH VPS, judged disproportionate infrastructure (key rotation,
repo maintenance) for what is not a tool being distributed to the general
public.

### 10.3 What this means for the workspace layout

New pieces alongside the existing `fs-config`/`fsapp` crates (`fsapp` here
means the one package with two `[[bin]]`s, per §2) — none of these are
Rust crates themselves:

```
fs-workspace/
├── Cargo.toml                      ← adds [profile.dist]
├── dist-workspace.toml             ← dist's own config (targets, installers, tap)
├── .github/workflows/release.yml   ← generated by `dist generate`
├── .github/workflows/deb.yml       ← hand-maintained; NOT touched by `dist generate`
├── fs-config/
├── fsapp/                          ← adds [package.metadata.deb] (both binaries)
```

Plus one external, one-time setup step outside this repo: creating the empty
`octalhub/homebrew-tap` GitHub repository that `dist`'s release workflow
pushes formula updates to.

## 11. Open items for Claude Code to flag rather than guess

- Exact `indicatif` template strings (bytes-based vs count-based bar) were
  sketched informally during design but never finalized character-by-character
  — reasonable defaults are fine, just keep them consistent between
  `copy`/`mv`/`sync`/`compress`.
- Whether `compress`'s summary block needs any wording different from
  copy/mv's (e.g. "entries archived" vs "entries copied") — cosmetic, use
  judgment, but keep the `✓`/`✗`/`⚠` structure identical.
- **`fsapp analyze`'s CLI doesn't expose everything `AnalyzeBuilder`
  offers** (§3.2b): `.modified_after()`/`.modified_before()` (present
  since 2.1.0 — `remove`'s CLI got these when it was added in 2.4.0,
  `analyze`'s never did, which reads like an oversight rather than a
  decision), `.max_reported_errors()`, and `.max_reported_duplicates()`
  all have no corresponding flag. Whether to add them, and whether §4 (a
  pre-analyze/remove/mv-many section that was never updated for any of
  the three) is due a fuller pass while touching this, is a real scope
  question — flag it rather than silently expanding one command's flags
  as a side effect of an unrelated task.

(`fset edit` validation order, GitHub owner/repo, tap repo name, edition/MSRV,
license, and crates.io publishing were all open as of the previous revision
of this document — resolved and moved to §9.)

## 12. Update check

`fsapp` tells users when a newer release exists. Two modes, sharing one
cache and one comparison path (`fsapp/src/update.rs`).

### 12.1 The two modes

**`fsapp update-check`** — explicit. Always hits the network, always
prints a verdict (including "you are on the latest"), prints to **stdout**
because there the version *is* the output, and exits `5` if GitHub can't
be reached. A script asking "is there an update?" must be able to
distinguish that from "no".

**The automatic check** — runs alongside a normal command. Cached, silent
unless there is something newer, prints to **stderr** after the summary so
it never contaminates piped stdout, and never affects the operation.

### 12.2 Source of truth

`https://api.github.com/repos/naut54/fsapp/releases/latest`, whose
`tag_name` is compared against `CARGO_PKG_VERSION` using the `semver`
crate. Semantic comparison, not string comparison — as strings `"0.9.0"`
sorts after `"0.10.0"`. The endpoint excludes prereleases and drafts, so
an `-rc.1` tag never surfaces as an upgrade prompt.

The request carries a `User-Agent` (GitHub rejects API requests without
one) and is bounded by a 3s timeout.

### 12.3 Caching

`<config-dir>/fsapp/update-check.json`, holding `last-checked` and
`latest-version`. Overridable with `FSAPP_CACHE_DIR`.

Deliberately **not** derived from `resolve_config_path`: `--config` is a
per-invocation override of *which settings to read*, and dropping a cache
file next to it isn't what the user asked for. The cache is machine state,
not configuration.

- A successful answer is good for **24h**.
- A failed attempt suppresses retries for **1h** only. A failure usually
  means the machine was briefly offline, and a full day of silence would
  hide a release from anyone who ran the tool at the wrong moment.
- `last-checked` is stamped **before** the request, not after. The check
  thread dies with the process (§12.4), so without a pre-stamp an
  interrupted refresh would leave the cache stale and every subsequent
  short-lived invocation would open its own connection to GitHub — none
  living long enough to finish. The stamp caps that at one attempt per
  failure-TTL; a successful fetch overwrites it moments later.
- A `last-checked` in the future (clock moved backwards) counts as stale.
- A corrupt or unreadable cache file is ignored, not fatal.

### 12.4 The automatic check must never cost the user anything

This is the constraint the design is built around, and it has teeth:

- It runs on a **detached `std::thread`, not `tokio::task::spawn_blocking`**.
  The tokio runtime waits for blocking tasks during shutdown, so a
  `spawn_blocking` check against an unreachable host added the full 3s
  network timeout to the command *after* the copy had already finished —
  measured at 3.01s wall clock for a copy that took 0.0s. A plain thread
  is outside the runtime's control and dies with the process, which is
  exactly right: the answer is never worth waiting for.
- At exit the result gets a **300ms grace period** and no more. Measured:
  0.01s with the check disabled, 0.01s on a cache hit, 0.31s with an
  unreachable network — the grace, and nothing beyond it.
- Every failure path ends in "say nothing": unreachable network,
  unwritable cache directory, malformed cache, unparsable tag.

### 12.5 When the automatic check is suppressed

Per §6.2 precedence: `--no-update-check` > `FSAPP_UPDATE_NO_CHECK` >
`update.no-check` > default (it runs). Suppressed additionally when there
is nobody to read it:

| Condition | Why |
|---|---|
| `--quiet` | the user asked for silence |
| stderr is not a tty | the notice would land in a log or a pipe |
| `CI` is set | a build server has no use for an upgrade suggestion and shouldn't be making the request |
| `update-check` subcommand | it answers for itself; running both would double the request |

### 12.6 The notice names one command, not four

The upgrade hint is chosen from where the running binary actually lives —
`current_exe()`, canonicalized, since Homebrew's `bin/` entries are
symlinks into `Cellar`:

| Path contains | Hint |
|---|---|
| `/Cellar/` or `/homebrew/` | `brew update && brew upgrade naut54/tap/fsapp` |
| `/.cargo/` | re-run the installer script |
| starts `/usr/bin/` | download the `.deb` and `dpkg -i` |
| anything else | the releases page |

Listing every channel would mean three of the four lines are wrong for any
given reader. See `updating.md` for the full matrix.
