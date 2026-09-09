# fsapp

`copy` / `mv` / `mv-many` / `sync` / `watch` / `compress` / `analyze` /
`remove`, backed by the
[`file-engine`](https://crates.io/crates/file-engine) crate. Ships as two
binaries from one install:

- **`fsapp`** — the operational CLI.
- **`fset`** — reads/writes the shared config file that sets persistent
  defaults for `fsapp`'s flags.

## Install

| Channel | Command |
|---|---|
| Homebrew | `brew install naut54/tap/fsapp` |
| Shell installer | `curl --proto '=https' --tlsv1.2 -LsSf https://github.com/naut54/fsapp/releases/latest/download/fsapp-installer.sh \| sh` |
| `.deb` | download from the [latest release](https://github.com/naut54/fsapp/releases/latest), then `sudo dpkg -i fsapp_*.deb` |
| From source | `git clone https://github.com/naut54/fsapp && cargo install --path fsapp --locked` |

`.deb` wires up shell completions automatically; every other channel
needs one command, once:

```bash
fsapp completions --install
fset completions --install
```

Not published to crates.io — `fsapp` itself, not the `file-engine`
library it wraps, is the release artifact.

## Usage

Every table below lists a command's flags in the order they're most
useful to know; `--flag` defaults are the ones `file-engine` itself
applies when a flag is omitted (CLI > `FSAPP_*` env var > `config.json` >
this default — see [Configuration](#configuration)).

### Global flags

Apply to every subcommand below.

| Flag | Default | What it does |
|---|---|---|
| `-v`, `-vv`, `-vvv` | warnings only | Raise log verbosity one step per repeat: info, then debug, then trace. |
| `-q`, `--quiet` | off | Suppress the progress bar; logging still follows `-v`. |
| `--config <PATH>` | platform config dir | Use this config file for this invocation instead of the default location. |
| `--no-update-check` | off | Skip this run's automatic check for a newer `fsapp` release. |

### Copy

```
$ fsapp copy src/ dst/
✓ 402 entries copied (1.2 GiB) in 1.1s
```

A large file streams with live progress, ETA, and transfer rate rather
than sitting still until it lands:

```
$ fsapp copy big.bin dst/
⠋ big.bin 88% [========================>    ] 1/1 ETA 12s · 210.4 MiB/s
```

| Flag | Default | What it does |
|---|---|---|
| `SOURCE` | — | File or directory to copy. |
| `DEST` | — | Directory `SOURCE` lands in — its basename is preserved underneath; this is never a `cp a b` rename target. |
| `--overwrite` | off | Replace an existing destination entry instead of failing with `DestExists`. |
| `--skip-if-identical` | off | Only consulted with `--overwrite` unset: leaves an already-identical destination alone instead of failing; a genuinely different one still fails. |
| `--on-error <continue\|abort\|undo>` | `continue` | Keep going and collect failures, stop at the first one, or roll back everything copied so far. |
| `--small-file-threshold <BYTES>` | 262144 (256 KiB) | Files at or below this size are batched together; above it, a file streams individually with its own progress. |
| `--batch-concurrency <N>` | available parallelism | Concurrent batch workers. |
| `--preserve-permissions` | off | Unix only. Apply source permission bits to created directories too (files already inherit theirs from the copy itself). |
| `--allow-fs-integrity-risk` | off | Proceed even when `DEST`'s filesystem has a known write-integrity risk (e.g. exFAT on macOS) instead of refusing before anything is written. |
| `--max-bytes-per-batch <BYTES>` | 8388608 (8 MiB) | Byte budget for one batch of small files. |
| `--max-files-per-batch <N>` | derived from the median file size | Hard cap on files per batch, overriding the size-derived default. |
| `--sort-order <asc\|desc>` | `desc` | Order small files within a batch before dispatching. |

### Move

Same shape as `copy`, minus the ETA line on a same-filesystem move
(nothing to stream — it's a rename).

| Flag | Default | What it does |
|---|---|---|
| `SOURCE` | — | File or directory to move. |
| `DEST` | — | Directory `SOURCE` lands in — its basename is preserved underneath; this is never a `mv a b` rename target. |
| `--overwrite` | off | Replace an existing destination entry instead of failing with `DestExists`. |
| `--skip-if-identical` | off | Only consulted with `--overwrite` unset: leaves an already-identical destination alone instead of failing (the now-redundant source is still removed); a genuinely different destination still fails. |
| `--on-error <continue\|abort\|undo>` | `continue` | Keep going and collect failures, stop at the first one, or roll back everything moved so far. |
| `--small-file-threshold <BYTES>` | 262144 (256 KiB) | Files at or below this size are batched together; above it, a file streams individually. |
| `--batch-concurrency <N>` | available parallelism | Concurrent batch workers. |
| `--preserve-permissions` | off | Unix only. Apply source permission bits to created directories too. |
| `--allow-fs-integrity-risk` | off | Proceed even when `DEST`'s filesystem has a known write-integrity risk instead of refusing before anything is written. |

### Move many

Moves several independent sources into one destination directory as a
single batched operation — one shared `--on-error` scope, concurrency
pool, and progress stream across all of them, rather than one `mv` call
per source:

```
$ fsapp mv-many a.txt b.txt notes/ dst/
✓ 3 entries moved (12.4 KiB) in 0.1s
```

| Flag | Default | What it does |
|---|---|---|
| `SOURCES` | — | One or more files/directories to move (at least one required). |
| `DEST` | — | Directory every source lands inside, each keeping its own basename — always a container, never a rename target. |
| `--overwrite` | off | Replace an existing destination entry instead of failing with `DestExists`. |
| `--skip-if-identical` | off | Same semantics as `mv`'s, applied per source. |
| `--on-error <continue\|abort\|undo>` | `continue` | Governs the batch as a whole: the first failure under `abort`/`undo` stops every source not yet finished, not just the one that failed. |
| `--small-file-threshold <BYTES>` | 262144 (256 KiB) | Files at or below this size are batched together; above it, a file streams individually. |
| `--batch-concurrency <N>` | available parallelism | Concurrent batch workers. |
| `--preserve-permissions` | off | Unix only. Apply source permission bits to created directories too. |
| `--allow-fs-integrity-risk` | off | Proceed even when `DEST`'s filesystem has a known write-integrity risk instead of refusing before anything is written. |

### Sync

Makes `DEST` match `SOURCE` — copies changes, deletes orphans:

```
$ fsapp sync src/ dst/
Copy phase:
✓ 12 entries copied (4.0 MiB) in 0.3s
Delete phase:
✓ 3 entries deleted (0 B) in 0.1s
```

| Flag | Default | What it does |
|---|---|---|
| `SOURCE` | — | Directory sync copies changes from. |
| `DEST` | — | Directory brought in line with `SOURCE`; entries not present in `SOURCE` are deleted from it. |
| `--no-overwrite` | off (i.e. overwrite is on by default) | Inverts the builder's default: skip files that already exist at the destination instead of replacing them. |
| `--checksum` | off (size/mtime diff) | Diff by content hash (blake3) instead of size and modified time. |
| `--on-error <continue\|abort\|undo>` | `continue` | Keep going and collect failures, stop at the first one, or roll back the phase in progress. |
| `--small-file-threshold <BYTES>` | 262144 (256 KiB) | Files at or below this size are batched together; above it, a file streams individually. |
| `--batch-concurrency <N>` | available parallelism | Concurrent batch workers. |
| `--preserve-permissions` | off | Unix only. Apply source permission bits to created directories too. |
| `--allow-fs-integrity-risk` | off | Proceed even when `DEST`'s filesystem has a known write-integrity risk instead of refusing before anything is written. |

### Watch

```
$ fsapp watch src/
modified: src/main.rs
created: src/new_file.rs
```

Runs until Ctrl+C.

| Flag | Default | What it does |
|---|---|---|
| `PATH` | — | Directory to watch for filesystem events. |
| `--no-recursive` | off (i.e. recursive by default) | Inverts the builder's default: only watch `PATH` itself, not its subdirectories. |

### Compress

```
$ fsapp compress src/ archive.zip
✓ 402 entries archived (1.2 GiB) in 2.4s
```

| Flag | Default | What it does |
|---|---|---|
| `SOURCE` | — | File or directory to archive. |
| `DEST` | — | Archive path to write. |
| `--format <zip\|gzip>` | inferred from `DEST`'s extension (`.zip`, `.gz`) | Archive format, when the extension alone doesn't say (or to override it). |
| `--on-error <continue\|abort\|undo>` | `continue` | Keep going and collect failures, stop at the first one, or roll back the archive in progress. |
| `--small-file-threshold <BYTES>` | 262144 (256 KiB) | Files at or below this size are batched together; above it, a file streams individually. |
| `--batch-concurrency <N>` | available parallelism | Concurrent batch workers. |

### Analyze

Read-only tree inspection — no writes, ever:

```
$ fsapp analyze src/ --detect-duplicates --detect-mime-types
✓ 11 files, 3 dirs, 98.6 KiB in 0.0s

Largest files:
    22.1 KiB  main.rs
    15.2 KiB  update.rs
    ...

By extension:
  rs               11 files    98.6 KiB

By MIME type:
  text/plain       11 files    98.6 KiB

Age:
  <1 day    4
  <1 week   0
  <1 month  4
  <1 year   3
  older     0
```

| Flag | Default | What it does |
|---|---|---|
| `PATH` | — | Directory to inspect. |
| `--extensions <a,b,...>` | any extension, including none | Only match files with one of these extensions (no leading dot). |
| `--exclude <glob,glob,...>` | none | Glob patterns, matched relative to `PATH`, that prune traversal. |
| `--min-size <BYTES>` / `--max-size <BYTES>` | unbounded | Size range a file must fall within to match. |
| `--max-depth <N>` | unbounded | How far the walk descends below `PATH`. |
| `--follow-symlinks` | off | Walk into symlinked directories instead of skipping them. |
| `--top-n-largest <N>` | 10 | How many of the largest matched files to list; `0` disables the section entirely. |
| `--walk-concurrency <N>` | available parallelism | Worker threads reading directories and `stat`-ing entries concurrently — turn down on a network filesystem where parallel `stat()` calls fight each other, up on a very wide local tree. |
| `--detect-mime-types` | off | Sniff each matched file's header to classify its MIME type. |
| `--detect-duplicates` | off | Content-hash (blake3) size-colliding files to find exact duplicates, grouped with bytes wasted. |
| `--abort-on-error` | off (continue and collect) | Stop at the first error instead of skipping it and continuing. |

### Remove

Deletes files under `PATH` matching a set of criteria — the destructive
counterpart to `analyze`, built on the same filter shape. Previews by
default:

```
$ fsapp remove build/ --extensions o,tmp
⚠ 214 entries would be removed (4.1 MiB)
  - build/main.o
  - build/cache.tmp
  ...
$ fsapp remove build/ --extensions o,tmp --no-dry-run
✓ 214 entries trashed (4.1 MiB) in 0.2s
```

No config-file section — every flag here is CLI-only, on purpose: a
destructive default (hard-delete, or an unfiltered delete) has no
business sitting in a JSON file the invocation you're looking at didn't
mention.

| Flag | Default | What it does |
|---|---|---|
| `PATH` | — | Root to remove matches under. |
| `--extensions <a,b,...>` | any extension, including none | Only match files with one of these extensions (no leading dot). |
| `--exclude <glob,glob,...>` | none | Glob patterns, matched relative to `PATH`, that spare an otherwise-matching entry. |
| `--min-size <BYTES>` / `--max-size <BYTES>` | unbounded | Size range a file must fall within to match. |
| `--modified-after <RFC3339>` / `--modified-before <RFC3339>` | unbounded | Modified-time range a file must fall within to match, e.g. `2026-01-01T00:00:00Z`. |
| `--max-depth <N>` | unbounded | How far the walk descends below `PATH`. |
| `--follow-symlinks` | off | Walk into symlinked directories instead of skipping them. |
| `--no-dry-run` | off (i.e. dry-run by default) | Inverts the builder's default: actually delete matches instead of only previewing them. |
| `--hard-delete` | off (trash) | Unlink matches permanently instead of moving them to the platform trash/recycle bin. |
| `--allow-unfiltered-delete` | off | Required when no filter above is set — otherwise an unfiltered `PATH` (matching everything under it) is refused before anything is touched. |
| `--on-error <continue\|abort\|undo>` | `continue` | Keep going and collect failures or stop at the first one; `undo` cannot roll back removals already applied. |
| `--batch-concurrency <N>` | available parallelism | Concurrent batch workers. |

## Configuration

`fset` sets persistent defaults so you don't have to repeat flags:

```
$ fset set copy.overwrite true
$ fset set sync.checksum true
$ fset get copy.overwrite
true
$ fset path
~/.config/fsapp/config.json
```

CLI flags always win over the config file. `fset list`, `fset unset`,
`fset edit` (opens `$EDITOR`, re-validates on save), and `fset reset`
(backs up before wiping) round out the workflow.

## Updating

```
fsapp update-check   # asks GitHub whether anything newer exists
```

`fsapp` also checks automatically after a normal command and prints a
notice only when there's something newer. See
[`docs/updating.md`](docs/updating.md) for update commands per channel
and for maintainers cutting a release.

## License

MIT
