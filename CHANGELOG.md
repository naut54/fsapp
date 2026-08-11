# Changelog

All notable changes to this project are documented here.

This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0]

### Added

- **Shell completions for zsh, bash, and fish**, on both binaries:

  ```bash
  fsapp completions --install     # detects $SHELL, finds a writable dir
  fsapp completions zsh           # or print it and place it yourself
  ```

  `--install` prefers your Homebrew prefix when the binary lives there,
  falls back to the system directory and then to a user-owned one, and
  prints the `fpath` line to add when it lands somewhere zsh does not read
  by default.

- **`.deb` packages install completions automatically** into Debian's
  `bash-completion`, `zsh/vendor-completions`, and
  `fish/vendor_completions.d` directories, for both `fsapp` and `fset`.

- **Archives now carry the completion scripts** (dist `include`), so they
  are present on every channel without regenerating them.

### Notes

- Scripts are generated from clap's own command tree, never hand-written,
  so they cannot drift from the CLI — `update-check` and
  `--no-update-check` are completable without anyone having written them
  down. The generated files are committed under `completions/`, and a test
  regenerates and compares them, so a CLI change that forgets to
  regenerate fails the suite instead of shipping a stale script.

- **Homebrew does not activate completions**, and this release does not
  change that. dist's formula installs the binaries and dumps everything
  else into `$(brew --prefix)/share/fsapp/`, which no shell reads
  (axodotdev/cargo-dist#2429). Homebrew users run
  `fsapp completions --install` once. Patching the formula after dist
  writes it was considered and rejected: it races dist's own commit and
  would silently stop working if their template changed.

- No PowerShell or Elvish. `completions <shell>` will still print them,
  but all four release targets are macOS or Linux, so nothing would
  install them.

## [0.4.0]

### Added

- **An update check.** `fsapp update-check` asks GitHub whether a newer
  release exists and prints a verdict either way. Alongside a normal
  command, the same check runs automatically and prints a notice only
  when there is something newer:

  ```
  ✓ 402 entries copied (1.2 GiB) in 1.1s

  ↑ fsapp 0.4.0 is available (you have 0.3.0)
    brew update && brew upgrade naut54/tap/fsapp
  ```

  The suggested command is chosen from where the running binary lives, so
  Homebrew users are told to use Homebrew and `.deb` users are not.

- **`--no-update-check`, `FSAPP_UPDATE_NO_CHECK`, and `update.no-check`**
  to turn the automatic check off, following the same precedence as every
  other setting. It also stays quiet under `--quiet`, when stderr is not a
  terminal, and when `CI` is set.

### Notes

- The automatic check never delays the operation. It runs on a detached
  thread — not a tokio blocking task, which the runtime waits for during
  shutdown — and gets a 300ms grace period at exit and no more. Measured:
  0.01s with the check disabled, 0.01s on a cache hit, 0.31s against an
  unreachable network.

- The result is cached for 24 hours in `<config-dir>/fsapp/update-check.json`
  (1 hour after a failure). Every failure path is silent: an unreachable
  network, an unwritable cache directory, and a corrupt cache file all
  mean "say nothing", never "interrupt the user".

- Adds `ureq` (rustls) and `semver`, taking the dependency tree from 200
  to 245 crates. TLS stays in-process, so the musl target and the `.deb`
  gain no system OpenSSL dependency.

## [0.3.0]

Upgrades to `file-engine` 2.0.0 and spends the new progress API on the
one thing the bar was worst at: a large file copying with nothing on
screen moving.

### Added

- **ETA and transfer rate on the progress bar**, e.g.
  `ETA 1m23s · 512.0 MiB/s`, from `file-engine`'s new `EtaEstimator`. It
  models directory, small-file, and large-file cost separately, so a
  mixed workload isn't estimated from a single meaningless
  bytes-per-second figure. Nothing is shown while the estimate has no
  measured rate to stand on, rather than a number that collapses a
  second later.

- **Live progress within a large file.** The engine now samples an
  in-flight large entry every 250ms, and the bar's message shows
  `large_a.bin 88%` for one streaming file, or
  `2 large files 424.0 MiB/1.2 GiB` for several. Previously a lone
  multi-gigabyte copy showed no movement at all between starting and
  finishing — the entry counter doesn't advance until the file lands.

- **Operation duration in the summary** — `✓ 402 entries copied
  (1.2 GiB) in 1.1s`, from the new `OperationOutcome.duration`. `sync`
  reports it per phase; the two phases don't sum to the whole run, since
  the diff that precedes them belongs to neither.

- **A steady spinner tick** (100ms). The spinner previously only advanced
  when a progress event arrived, so it froze for exactly as long as a
  large copy took.

### Changed

- **The bar is set up earlier.** Its length now comes from the new
  `Progress::Planned`, which arrives before the directory pre-pass rather
  than after it. On a large tree that pre-pass runs for a while on its
  own, and the bar used to read `0/0` for all of it.

- **The bar sizes itself to the terminal** (`{wide_bar}` rather than a
  fixed 40 columns). With a filename in the message and an ETA after the
  counter, a fixed-width bar pushed the line past the terminal width and
  wrapped it. Long filenames are truncated to 28 characters.

- **Durations under 10 seconds show one decimal** (`0.4s`, not `0s`). A
  same-volume copy that the filesystem satisfies by cloning finishes in
  milliseconds, and `in 0s` read as "unmeasured" rather than "instant".

- **`file-engine` 1.1.1 → 2.0.0.** Its output types are now
  `#[non_exhaustive]`; both exhaustive `match`es here gained a `_` arm.
  Its dispatch order also changed — the smallest large file now runs
  before the small-file batches instead of after all of them — so
  progress events no longer arrive grouped small-then-large. See
  `docs/fsapp-design-spec.md` §3 and §7.1.

### Notes

- A copy the filesystem satisfies by cloning (APFS reflink, same volume)
  completes before the first 250ms sample and emits no in-flight progress
  at all. That is correct rather than a missing-events bug: there was
  nothing to wait for.

- On a workload of many small files plus a few large ones, the ETA starts
  pessimistic and tightens sharply once the first large file completes.
  Until then the estimator stands in the overall byte rate, which carries
  small-file per-file overhead, for the streaming rate it has not yet
  measured.

## [0.2.1]

### Added

- `--version` on both `fsapp` and `fset`.

## [0.2.0]

### Changed

- `fset` is now a second `[[bin]]` in the `fsapp` package rather than a
  separate crate, so `brew install fsapp` installs both binaries in one
  command.

## [0.1.1]

### Fixed

- `fsapp --help` showed the wrong subcommand descriptions.

## [0.1.0]

Initial release: the `fsapp` operational CLI and the `fset` config CLI,
covering copy/mv/sync/watch/compress, with shell, Homebrew, and `.deb`
install channels.
