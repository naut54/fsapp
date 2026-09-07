# fsapp

`copy` / `mv` / `sync` / `watch` / `compress` / `analyze`, backed by the
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

### Copy / move

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

`mv` is the same shape, minus the ETA line on a same-filesystem move
(nothing to stream). Both support `--overwrite`, `--on-error
continue|abort|undo`, `--preserve-permissions`, and batch tuning
(`--small-file-threshold`, `--batch-concurrency`).

### Sync

Makes `DEST` match `SOURCE` — copies changes, deletes orphans:

```
$ fsapp sync src/ dst/
Copy phase:
✓ 12 entries copied (4.0 MiB) in 0.3s
Delete phase:
✓ 3 entries deleted (0 B) in 0.1s
```

`--checksum` diffs by content hash instead of size/mtime; `--no-overwrite`
skips files that already exist at the destination.

### Watch

```
$ fsapp watch src/
modified: src/main.rs
created: src/new_file.rs
```

Runs until Ctrl+C. `--no-recursive` limits it to the given directory.

### Compress

```
$ fsapp compress src/ archive.zip
✓ 402 entries archived (1.2 GiB) in 2.4s
```

Format is inferred from `DEST`'s extension (`.zip`, `.gz`), or set it
explicitly with `--format`.

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

`--extensions`, `--exclude` (glob), `--min-size`/`--max-size`,
`--max-depth`, and `--follow-symlinks` filter the walk; `--detect-duplicates`
content-hashes size-colliding files (blake3) and reports duplicate groups
plus bytes wasted; `--abort-on-error` stops at the first error instead of
collecting and continuing. `--walk-concurrency` sets how many worker
threads read directories and `stat` entries concurrently (defaults to
available parallelism) — useful to turn down on a network filesystem
where too many parallel `stat()` calls fight each other, or up on a very
wide local tree.

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
