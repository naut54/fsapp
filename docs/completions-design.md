# Shell completions — design

**Status: implemented in 0.5.0.** The decisions taken are recorded in §8;
the rest of this document is the design as built.

Goal: `fsapp cop<TAB>` completes to `copy`, on every channel we ship
through, for zsh, bash, and fish.

---

## 1. Why there was nothing before 0.5.0

Three independent gaps, all verified against the current release:

1. No `clap_complete` dependency and no completion code in the tree. clap
   parses arguments at runtime; tab-completion happens *before* the
   program runs, so it needs a separate script the shell loads.
2. The dist-generated Homebrew formula installs binaries and auto-docs
   only. Its `def install` is `bin.install "fsapp", "fset"`.
3. Nothing lands in any shell's completion path — `/opt/homebrew/share/zsh/site-functions/`
   holds `_brew`, `_gh`, `_git`, and no `_fsapp`.

Result: zsh falls back to filename completion, so `fsapp cop<TAB>` looks
for a *file* named `cop`.

## 2. What each channel can actually do

| Channel | Can install completions? | Mechanism |
|---|---|---|
| `.deb` | **Yes, fully automatic** | `cargo-deb` `assets` are entirely ours |
| Homebrew | **No** — dist limitation | see §2.1 |
| Shell installer | **No** | installs binaries to `CARGO_HOME/bin`, nothing else |
| `cargo install` | **No** | same |

Only one of four channels can be made automatic today. That asymmetry is
what shapes the rest of this design.

### 2.1 The Homebrew blocker, precisely

dist's formula template installs the binary and auto-docs, then does
`pkgshare.install(*leftover_contents)` — every other file in the archive
is dumped into `$(brew --prefix)/share/fsapp/`, which is not a completions
directory and not on the manpath.

This is [axodotdev/cargo-dist#2429][issue], open since 2026-06-15 with no
comments, asking for exactly this (man pages and
`generate_completions_from_executable`). Confirmed against the v0.32.0
template we pin.

Two consequences:

- We **cannot** get Homebrew to activate completions without either an
  upstream fix or patching the formula after dist writes it.
- We **can** get the files onto a Homebrew user's disk at a predictable
  path, because `pkgshare.install` will place anything we ship in the
  archive. That's worth exploiting — see §4.

[issue]: https://github.com/axodotdev/cargo-dist/issues/2429

## 3. Generation

`clap_complete` renders a script from the `clap::Command` we already
build. Both binaries need one; `fset` has its own `Parser`, so each emits
its own.

```
fsapp completions <zsh|bash|fish>   # prints to stdout
fset  completions <zsh|bash|fish>
```

A subcommand rather than a build-time-only artifact, because it's also
what `generate_completions_from_executable` calls if dist ever fixes
#2429, and it's what makes the manual one-liner possible today.

Generating from clap rather than hand-writing scripts also means the
completions can't drift from the CLI: `update-check` and
`--no-update-check` would have appeared in 0.4.0's completions for free.

### 3.1 Where the packaged files come from

`.deb` assets and dist's `include` both need the files to **exist on disk
at package time**, not just be producible by a binary. Three ways:

**(a) Commit them, guard with a test.** `completions/` is checked in;
a test regenerates and asserts the committed files match, so they can
never silently drift. dist `include` and `cargo-deb` assets both just
point at real paths. No build-time trickery, deterministic, and the diff
is reviewable.
*Cost:* generated files in the repo, and a regeneration step in the
release checklist (which the test enforces, so it can't be forgotten).

**(b) `build.rs` writes them at compile time.** No committed artifacts.
*Cost:* build scripts writing outside `OUT_DIR` is discouraged, breaks on
a read-only source tree, and the CLI definition has to be reachable from
`build.rs` (today `cli.rs` is a private module of a binary, so this needs
an `include!` or a restructure).

**(c) Generate in CI before packaging.** No committed artifacts, no
build.rs.
*Cost:* only works for `.deb` (whose workflow we own). dist builds and
archives in one step, so there's no hook to generate files before it
archives — which kills the `include` path, and with it Homebrew's
`pkgshare` copy.

**Chose (a).** It's the only one that serves all three consumers
(`include`, `cargo-deb`, and a human reading the repo), and the staleness
test removes the usual objection to committed generated code.

## 4. Per-channel plan

### 4.1 `.deb` — fully automatic

Add to `[package.metadata.deb] assets`:

| File | Destination |
|---|---|
| `completions/fsapp.bash` | `/usr/share/bash-completion/completions/fsapp` |
| `completions/_fsapp` | `/usr/share/zsh/vendor-completions/_fsapp` |
| `completions/fsapp.fish` | `/usr/share/fish/vendor_completions.d/fsapp.fish` |

…and the same three for `fset`. Debian's paths, not Homebrew's. Nothing
else changes; `deb.yml` already runs `cargo deb -p fsapp`.

### 4.2 Archives + Homebrew — get the files there, then activate them

`dist-workspace.toml` gains:

```toml
include = ["completions"]
```

Files/directories copied into the root of every archive and installer
(dist ≥ 0.0.3; globs not supported, so a directory it is). For Homebrew
this means the scripts land in `$(brew --prefix)/share/fsapp/completions/`
— present but inert.

Activating them needs one of:

- **`fsapp completions --install`** (§4.3) — works today, one command,
  no release-process risk. **This is what 0.5.0 ships.**
- **A post-release CI job** that checks out the tap after
  `publish-homebrew-formula`, patches `Formula/fsapp.rb` to add
  `generate_completions_from_executable`, and commits. Works, but it
  races dist's own commit and silently un-patches itself if dist's
  template changes. I'd rather not couple the release to it.
- **Wait for #2429.** Free, no timeline.

### 4.3 `completions --install` — the fallback that covers everything else

```
fsapp completions --install [--shell <s>] [--dir <path>]
```

Detects the shell (`$SHELL`, overridable), picks the first writable
directory from an ordered list, writes the script, prints where it went
and what to run to activate it now. Covers the shell installer,
`cargo install`, from-source, *and* Homebrew's gap — the one mechanism
that works on every channel.

Destination order (first writable wins):

| Shell | Homebrew prefix | System | User |
|---|---|---|---|
| zsh | `$(brew --prefix)/share/zsh/site-functions/_fsapp` | `/usr/share/zsh/vendor-completions/_fsapp` | `~/.zsh/completions/_fsapp` |
| bash | `$(brew --prefix)/etc/bash_completion.d/fsapp` | `/usr/share/bash-completion/completions/fsapp` | `~/.local/share/bash-completion/completions/fsapp` |
| fish | `$(brew --prefix)/share/fish/vendor_completions.d/fsapp.fish` | `/usr/share/fish/vendor_completions.d/fsapp.fish` | `~/.config/fish/completions/fsapp.fish` |

The user-level zsh path needs an `fpath` entry, so when that's the one
chosen, print the exact line to add to `.zshrc` rather than editing the
user's shell config silently.

`fset` has the same subcommand rather than being folded into `fsapp`'s:
each binary owns its own `clap::Command`, and a single command installing
another binary's completions would have to hard-code that binary's
grammar. Two lines instead of one, documented together.

## 5. What this does *not* do

- **No PowerShell.** We ship four targets, all macOS and Linux.
  `clap_complete` can emit it; nothing would install it.
- **No auto-install on first run.** Writing to a user's shell config or
  Homebrew prefix as a side effect of `fsapp copy` is exactly the kind of
  surprise the update check was carefully designed to avoid.
- **No man pages**, though #2429 covers both and the `include` mechanism
  would carry them the same way. Worth a follow-up.

## 6. Testing

There's no CI in this repo, so all of this is a local gate:

- **Staleness test** — regenerate all six scripts, assert byte-equality
  with the committed ones. This is the test that makes option (a) safe.
- **Generation smoke test** — each shell emits non-empty output
  containing a known subcommand (`update-check`), so a clap restructure
  that silently empties the grammar fails loudly.
- **Manual, once per shell**: `zsh -c 'compinit; ...'` loads `_fsapp`
  without error; bash sources its script; fish parses its own. Generated
  scripts are syntax-checkable but not behaviour-checkable in CI.
- **`.deb` verification**: `dpkg -c` the built package and assert the
  three destination paths are present, which catches an `assets` typo
  without installing anything.

## 7. Rollout

One release, 0.5.0:

1. `clap_complete` dependency; `completions` subcommand on both binaries.
2. `completions/` committed + staleness test.
3. `cargo-deb` assets.
4. `dist-workspace.toml` `include = ["completions"]`.
5. `completions --install`.
6. `updating.md` gains a "turn on tab completion" section; the release
   notes lead with the one-liner, since existing users must run it once
   by hand regardless of channel.

Homebrew stays non-automatic until #2429 lands or we take the formula
patching risk. Everyone else gets it automatically on `.deb`, or with one
command everywhere else.

## 8. Decisions taken

1. **Option (a)** — scripts are committed under `completions/`, guarded by
   a staleness test. Verified to fail correctly: tampering with a
   committed script fails `committed_scripts_match_the_current_cli` with
   the exact regeneration command in the message.
2. **No formula patching.** Homebrew users run
   `fsapp completions --install` once. #2429 tracks the upstream fix.
3. **`--install` is per-binary**, not one command for both: `fsapp` and
   `fset` each have their own `completions` subcommand, since each owns
   its own `clap::Command`. `updating.md` lists both lines together.
4. **Man pages deferred.** Same `include` mechanism would carry them; not
   worth widening a change already touching four packaging paths.

## 9. What was verified

- `fsapp cop` → `copy`, `fsapp up` → `update-check`, `fsapp completions z`
  → `zsh`, `fset un` → `unset`, driven through the generated bash function
  with `COMP_WORDS`/`COMP_CWORD` set the way bash sets them.
- zsh's `compinit` registers `_fsapp` for `fsapp` from a directory on
  `fpath`. A fully interactive zsh capture via `zpty` hung in the test
  environment and was abandoned — the bash candidate test covers the same
  generated grammar.
- The staleness test fails on a tampered script and passes when restored.
- `--install --dir` writes both binaries' scripts to an explicit
  directory; an unwritable directory is an error, not a panic.

Note that macOS ships bash 3.2, whose `complete` builtin rejects the
`-o nosort` in clap's generated registration line. The completion
*function* still works, which is what the test drives directly. Anyone
wanting bash completion on macOS needs a newer bash from Homebrew — this
is a pre-existing macOS quirk, not something this change introduces.
