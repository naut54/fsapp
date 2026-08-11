# Updating fsapp

Three separate procedures, for three different people:

- [Updating the installed app](#1-updating-the-installed-app) — you use
  `fsapp` and want the new version.
- [Updating `file-engine` underneath it](#2-updating-file-engine-underneath-it)
  — you maintain `fsapp` and the engine crate published a new version.
- [Cutting a release](#3-cutting-a-release) — you maintain `fsapp` and
  want users to be able to do #1.

---

## 1. Updating the installed app

Check what you have first:

```bash
fsapp --version
fset --version
fsapp update-check     # asks GitHub whether anything newer exists
```

`fsapp` also tells you on its own. After a normal command it prints a
notice when a newer release exists, naming the one upgrade command that
matches how this copy was installed:

```
✓ 402 entries copied (1.2 GiB) in 1.1s

↑ fsapp 0.4.0 is available (you have 0.3.0)
  brew update && brew upgrade naut54/tap/fsapp
```

That check is cached for a day and runs in the background, so it never
delays anything. To turn it off:

```bash
fsapp copy src dst --no-update-check   # once
export FSAPP_UPDATE_NO_CHECK=true      # for a shell
fset set update.no-check true          # permanently
```

It also stays quiet on its own when there's nobody to read it — under
`--quiet`, when stderr isn't a terminal, and when `CI` is set.

Both binaries ship together and always carry the same version — `fset` is
a second `[[bin]]` in the `fsapp` package, not a separate crate. If they
disagree, you have two installs on `PATH` from different channels; run
`which -a fsapp fset` and remove the stale one.

Update by whichever channel you installed with:

| Channel | Update command |
|---|---|
| Homebrew | `brew update && brew upgrade naut54/tap/fsapp` |
| Shell installer | re-run the installer (below) |
| `.deb` | download the new `.deb` from the release, then `sudo dpkg -i fsapp_*.deb` |
| From source | `git pull && cargo install --path fsapp --locked` |

The shell installer is idempotent — re-running it overwrites the existing
binaries in place:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/naut54/fsapp/releases/latest/download/fsapp-installer.sh | sh
```

It installs to `$CARGO_HOME/bin` (usually `~/.cargo/bin`). Override the
location with `FSAPP_INSTALL_DIR` if you need it elsewhere; do it
consistently across updates or you'll end up with two copies.

Homebrew users: the formula lands in `naut54/homebrew-tap` as part of the
release workflow, so `brew upgrade` finds nothing until that run finishes
(about 4 minutes after the tag is pushed). `brew update` first is not
optional — without it Homebrew won't see the new formula commit.

Minimum supported Rust version is 1.88, which only matters if you install
from source. Every other channel ships prebuilt binaries.

---

## 2. Updating `file-engine` underneath it

`fsapp` is a thin CLI over the `file-engine` crate; nearly every
user-visible behavior change comes from the engine. The procedure, in the
order that catches problems earliest:

**Read the engine's changelog before touching anything.**
`file-engine`'s `CHANGELOG.md` carries an explicit "Upgrading from N.x"
section on major releases, listing exactly what breaks. Reading it is
faster than reading compiler errors, and it names things the compiler
can't — behavioral changes like dispatch order, which compile fine and
change what users see.

**Bump the requirement** in `fsapp/Cargo.toml`:

```toml
file-engine = { version = "2.0.0", features = ["sync", "watch", "compress", "checksum", "permissions"] }
```

A patch or minor bump needs only `cargo update -p file-engine`, since the
existing requirement already covers it. A major bump needs the version
string edited.

**Compile everything, including tests:**

```bash
cargo check --workspace --all-targets
```

`--all-targets` matters. Test code constructs types that library code only
reads, and `file-engine`'s output types are `#[non_exhaustive]` — a
fixture built with a struct literal breaks in a way that a plain
`cargo check` won't show you.

**Then the rest of the gate:**

```bash
cargo clippy --workspace --all-targets
cargo test --workspace
```

Note that this repo has never been `rustfmt`-clean, so `cargo fmt --check`
reports the entire tree and is not a useful signal. Match the surrounding
style instead. There is also no CI that runs any of this — the release tag
is the only gate, so run it locally.

`fs-config`'s `defaults_match_file_engine_builder_defaults` test is the
one that catches a silently changed engine default. If it fails, the
engine changed a builder default and `fs-config`'s schema needs to follow;
don't "fix" the test by editing the expected value without checking which
side is right.

**Exercise the progress path by hand.** Nothing in the test suite covers
the progress renderer, and progress bugs are invisible to `cargo test`.
The bar only renders when stderr is a tty, so a pipe won't show it:

```bash
python3 -c "
import pty; pty.spawn(['./target/release/fsapp','copy','/path/to/src','/path/to/dst'])
"
```

Two cases behave differently and you want both:

- **Same volume on APFS** — the filesystem clones the data, the copy
  finishes in milliseconds, and no in-flight progress is emitted at all.
  Correct, but it exercises none of the sampling.
- **Across volumes** — a real byte copy, which is the only way to see
  `EntryProgress`, the ETA, and the transfer rate. Make a scratch volume
  rather than hunting for a spare disk:

  ```bash
  hdiutil create -size 3g -fs APFS -volname fstest -quiet /tmp/fstest.dmg
  hdiutil attach /tmp/fstest.dmg -quiet     # mounts at /Volumes/fstest
  # ... run the copy into /Volumes/fstest/dst ...
  hdiutil detach /Volumes/fstest
  ```

  Size it well above the source tree. A copy that runs the volume out of
  space exercises the `NoSpace` fatal path instead of the one you meant to
  test.

Use a source tree with both many small files and a couple of large ones —
a few hundred small files plus a GB or so — since the small/large split is
exactly what the estimator models separately.

Also check Ctrl+C still exits `130` with the message frozen at
`cancelling...`, and that `--quiet` still suppresses the bar without
panicking. The quiet path takes `None` for the bar everywhere and is easy
to break with an `unwrap`.

**Write down what changed.** Update `docs/fsapp-design-spec.md` §3 (the
verified API surface and the hard behavioral constraints) and §7.1 (what
the bar shows). The spec claims its API surface was *verified by
compiling*, so leaving a stale version number there is worse than leaving
it undocumented.

---

## 3. Cutting a release

The pipeline is tag-driven — pushing a tag is the whole release. Nothing
is published to crates.io.

**Before tagging:**

1. Bump `version` in the root `Cargo.toml` (`[workspace.package]` — both
   crates inherit it) and rebuild so `Cargo.lock` follows.
2. Add a `## [X.Y.Z]` section to `CHANGELOG.md`. `dist` prepends it to
   the GitHub Release body; without it the release notes contain nothing
   but install instructions.
3. Confirm `cargo test --workspace` and `cargo clippy` are clean. There is
   no CI to catch it afterwards.

**Then:**

```bash
git commit                          # changes + version bump + changelog
git push origin main
git tag vX.Y.Z && git push origin vX.Y.Z
```

The tag must match the workspace version exactly, `v` prefix included, or
`dist` fails the run.

**What fires, in order:**

1. **Release** (`.github/workflows/release.yml`, generated by `dist` —
   don't hand-edit it, run `dist generate` after changing
   `dist-workspace.toml`). Builds `aarch64-apple-darwin`,
   `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`, and
   `x86_64-unknown-linux-musl`; uploads tarballs and sha256 checksums;
   generates `fsapp-installer.sh`; creates the GitHub Release; commits the
   updated formula to `naut54/homebrew-tap`. About 4 minutes.
2. **Build and attach .deb packages** (`.github/workflows/deb.yml`), on
   the Release workflow *completing*. It triggers on `workflow_run`, not
   `release: published`, because GitHub does not fire repo events for a
   release created with the default `GITHUB_TOKEN` — which is what `dist`
   uses. About 2 minutes.

Watch both with `gh run list` / `gh run watch`.

**If the .deb job is the only thing that failed**, don't re-tag. Re-run it
alone:

```bash
gh workflow run deb.yml -f tag=vX.Y.Z
```

**If the Release job failed**, delete the tag locally and remotely, fix
the problem, and re-tag. A tag that already produced a GitHub Release
can't be reused — delete the release first, or the run fails on an
existing artifact.

The Homebrew formula push needs a `HOMEBREW_TAP_TOKEN` repo secret with
push access to `naut54/homebrew-tap`. If the release succeeds but
`brew upgrade` never sees the version, that token is the first thing to
check — an expired one fails the publish job without touching the release
itself.
