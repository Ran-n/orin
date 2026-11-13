[//]: # ( ---------------------------------------------------------------------- )
[//]: # (+ Authors: 	Ran# <ran.hash@proton.me> )
[//]: # (+ Created: 	2026/11/11 12:19:34.092613 )
[//]: # (+ Revised: 	2026/11/13 15:58:36.457667 )
[//]: # ( ---------------------------------------------------------------------- )


# Development Guide

## Current State

Orin is in early development: a working **Rust + GPUI** desktop application
([ADR-001](design-decisions.md#adr-001-rust--gpui-desktop-application)) that talks to agents over ACP
([ADR-004](design-decisions.md#adr-004-agent-integration-model)).
It is verified on Windows. CI builds and tests Windows, Linux, and macOS, but Linux and macOS have not yet been checked on real machines,
and the rendered UI tests and their screenshot baselines exist for Windows only.

## Repository Layout

```text
orin/
├── src/
│   ├── main.rs             Entry point: logging first, then options, then the window
│   ├── startup.rs          Command-line options
│   ├── agents/             ACP sessions (protocol, transport, permissions, engine), session options, adapter discovery (PATH, then npx), and agent sign-in status
│   ├── terminal/           The developer's shell in a PTY, with a VT screen model
│   ├── project/            Project root confinement, file reading and saving, Git diffs and status, file watching
│   ├── editor/             Text buffers, undo history, and save checkpoints
│   ├── settings/           Versioned workspace settings (schema 2: column layout, page, recent folders, agent choices, update choices), migrations, atomic saves
│   ├── update/             Self-updating: release target and asset names, GitHub/GitLab release check, verified download, install and restart
│   ├── bin/                Small helper binaries (`orin-asset-name`, used by the release workflow)
│   ├── process/            Child-process launch and process-tree cleanup per platform
│   ├── background.rs       Bounded worker pool for disk and process work
│   ├── sharing.rs          Retries for transient Windows file-sharing conflicts
│   ├── keyboard.rs         Terminal key encoding
│   ├── logging.rs          Structured JSON logging
│   └── desktop/            GPUI interface (columns, Welcome and Settings pages, menus, controls, themes, syntax highlighting, Markdown preview); the only module that depends on GPUI
├── tests/                  Integration, property, contract, golden-file, and rendered UI tests
│   ├── agents/             Offline ACP fixture agent and agent session tests
│   ├── golden/             Saved-format golden files (settings, release API answers)
│   ├── update/             Local HTTP release server shared by the update tests
│   └── rendered/           Native UI driver, screen capture, and reviewed baselines
├── assets/
│   ├── branding/orin-mascot.png  Logo and mascot, embedded and resampled for each display scale
│   └── icons/              Lucide interface icons (ISC license, LICENSE alongside)
├── docs/                   Vision, user guide, this guide, and design decisions
├── .github/workflows/      CI for Windows, Linux, and macOS, and the tag-triggered release build
├── build.rs                Embeds the mascot as the Windows executable icon (title bar, taskbar, Alt+Tab)
├── Cargo.toml, Cargo.lock  Manifest and pinned dependencies
├── rust-toolchain.toml     Pinned Rust toolchain
├── CHANGELOG.md            Pending and released changes
├── CONTRIBUTING.md         How to report, propose, and submit changes
├── CONTRIBUTORS            Ownership and revenue-share record
├── LICENSE                 PayBack License (PBL) v2.0
└── README.md               Project introduction
```

Domain modules have no GPUI dependency and are tested without a window: `cargo test --no-default-features` builds them alone.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) through `rustup`. The first build installs the toolchain pinned in `rust-toolchain.toml` automatically.
- [Git](https://git-scm.com/), which is also used at runtime for diffs.
- **Windows:** the MSVC build tools (Visual Studio Build Tools with the C++ workload).
  If GPUI cannot find `fxc.exe`, set `GPUI_FXC_PATH` to the shader compiler in your installed
  Windows SDK's `bin/<sdk-version>/x64/` directory before building.
- **macOS:** the Xcode command-line tools.
- **Linux:** the native libraries CI installs, for example on Ubuntu:

  ```sh
  sudo apt-get install pkg-config clang cmake libasound2-dev libfontconfig-dev libfreetype-dev libwayland-dev     libx11-xcb-dev libxkbcommon-x11-dev libxcb-shape0-dev libxcb-xfixes0-dev libssl-dev libvulkan1
  ```

## Build and Run

```sh
cargo run                         # debug build
cargo run --release               # optimized build
cargo run -- --project PATH       # open a project folder
cargo run -- --settings FILE      # use a separate settings file, e.g. to keep your own untouched
```

Set `ORIN_LOG` to change logging, for example `ORIN_LOG=orin=debug`. Logs are JSON on standard error.
On Windows, Orin uses the GUI subsystem so launching the executable does not open a console window.
Redirect standard error to a file when collecting diagnostics (for example, `orin.exe 2> orin.log`).
`.env.example` lists the optional environment variables; Orin does not load it automatically.
`PATH` locates Git, agent CLIs and adapters. The terminal uses `ComSpec` on Windows and `SHELL` on Unix,
with a system shell fallback. Those variables are managed by your operating system and shell.
`GEMINI_API_KEY` and `GOOGLE_API_KEY` are checked for presence to report Gemini sign-in; their values are not saved.

## Tests

| Command | What it runs |
| --- | --- |
| `cargo test --no-default-features` | Everything that doesn't need a window |
| `cargo test --all-features` | The above plus desktop code and the offline ACP fixture agent (what CI runs) |
| `cargo coverage` | Domain suite with offline ACP fixture, missing-line report, and a 92% line coverage gate (Windows CI) |
| `cargo coverage-all` | Full default suite including desktop code and CLI entrypoint, with a 48% whole-app line coverage gate (Windows CI) |
| `cargo test --all-features --test desktop -- --ignored --test-threads=1` | Rendered UI tests on Windows (see below) |
| `ORIN_LIVE_AGENT=claude cargo test --all-features --test agents -- --ignored` | Handshake with a real installed adapter (`claude`, `codex`, or `gemini`); no prompt is sent |

CI also runs `cargo fmt --all -- --check` and `cargo clippy --locked --all-targets --all-features -- -D warnings`. Both must pass.

Install the coverage tool with `cargo install cargo-llvm-cov --version 0.9.1 --locked`.
The domain gate covers services; the whole-app gate also counts GPUI code, much of which needs the rendered suite below.
Stable Rust line coverage does not measure branch coverage. Keep the gate at or above the measured whole-number total.
Use `cargo coverage --output-path target/coverage.txt` to retain its summary; missing lines are printed to the console.
After `cargo coverage-all`, run `cargo llvm-cov --locked --all-features --no-clean --fail-under-lines 90 --test desktop -- --ignored --test-threads=1`
to include native interactions in the same coverage data. This needs an interactive Windows desktop and is separate from hosted CI.
Run coverage commands sequentially: they share an instrumented build directory and profiling files.

CI runs `cargo audit` against the lockfile. The pinned GPUI 0.2.2 tree currently brings maintenance notices
and `git2` soundness advisories through `gpui_util`; no advisory is suppressed. These upstream constraints
need rechecking when GPUI changes. `cargo upgrade --dry-run --locked` checks direct dependency versions without editing them.

Git reads have a 15-second deadline and an 8 MiB limit per output stream. Account checks have a 20-second deadline
and a 1 MiB limit per stream. Both supervise and stop their child processes and descendants on completion or failure.
Undo history retains at most 10,000 records and 8 MiB of edit text (keeping the most recent edit).
Settings saves enforce the same 1 MiB serialized-file limit as loads, preserving the last readable file on refusal.
Only regular text files can be opened; Unix named pipes are rejected without waiting for a writer.

**Rendered UI tests** start the real application, drive it with posted window messages (never your mouse or keyboard),
capture it with `PrintWindow`, and compare each screen at 1280 and 1000 pixels wide with the baselines in
`tests/rendered/baselines/windows/`. They need an interactive Windows desktop session. On a mismatch, the captured and diff images
are written to `tests/rendered/artifacts/windows/`. After an intended visual change, re-record with `ORIN_UPDATE_BASELINES=1`,
then **review every changed image** before committing it: a recorded baseline is only as good as its review.

## Backlog

Planned work lives in [TODO.md](../TODO.md), one self-contained item per change, in priority order.
Its header explains how to add and implement items; screenshots for items are in `docs/todo/`.

## Get the Repository

All mirrors use `main` as the default branch:

| Host | Repository | SSH clone URL |
| --- | --- | --- |
| GitHub | [Ran-n/orin](https://github.com/Ran-n/orin) | `git@github.com:Ran-n/orin.git` |
| GitLab | [Ran-n/orin](https://gitlab.com/Ran-n/orin) | `git@gitlab.com:Ran-n/orin.git` |
| Codeberg | [ran-n/orin](https://codeberg.org/ran-n/orin) | `git@codeberg.org:ran-n/orin.git` |

Fork on your chosen host before preparing a contribution.

## Working on Documentation

1. Read the [vision](vision.md) and [design decisions](design-decisions.md) before proposing a change to intended behavior.
2. Mark planned capabilities as planned. Don't add screenshots, download links, or commands for things that don't exist.
3. Keep the README introductory and put detail in the guides.
4. Keep relative links and anchors working, and preview tables, collapsible sections, and images before submitting.

Markdown and Rust files start with a generated metadata header (Authors, Created, Revised).
Leave it in place and don't edit the timestamps by hand; the maintainer refreshes them when integrating your change.
Copy the [LICENSE](../LICENSE) unchanged and keep the exact [CONTRIBUTORS](../CONTRIBUTORS) format.

Before submitting, check for whitespace errors:

```sh
git diff --check
```

## Engineering Standards

Every change follows these standards.

**Code**

- Pin the Rust toolchain and GPUI version, and commit `Cargo.lock`.
- Code must pass `rustfmt` and Clippy with no warnings.
- Never block the UI thread on disk, process, or network work. Run independent work concurrently, with limits.
- Stop every thread, task, child process, and handle on every quit path.
- Use named, leveled logging set up before startup. Show errors to the user; never swallow them or log secrets.

**Interface**

- Remember every setting and UI state between runs.
- Edit in place where content has a natural in-place form. Unavailable actions are visibly disabled.
- Keep every supported translation complete.

**Tests**

Every change includes tests of every type that applies. `cargo coverage` and `cargo coverage-all` enforce the measured domain and whole-app line coverage floors on Windows.

| Type | Covers |
| --- | --- |
| Unit | Logic, boundaries, success and error paths |
| Integration | Real temporary files and real child processes |
| Property-based | Invariants of parsers, formatters, geometry, and serialization |
| Regression | A pinned case for each fixed bug, shown to fail before the fix |
| Contract and golden files | Agent and CLI contracts; saved formats stay readable by later versions |
| Rendered UI | Real application interactions with reviewed screenshot baselines, on every OS, at minimum width, in every theme and locale |
| Live integration | Real agent CLIs, in a separate opt-in group |
| Shutdown | Cleanup on every quit path |

UI automation must never take over the real mouse or keyboard.
Before a release, run every test group, including ignored, feature-gated, live, and per-platform ones, and report any host that wasn't verified.

## Changelog

Record changes under **Unreleased** in [CHANGELOG.md](../CHANGELOG.md), in the same commit as the change, using the
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) categories: Added, Changed, Deprecated, Removed, Fixed, Security.

## Releases

A release includes a Windows installer and one portable binary per target, each with a BLAKE3 checksum. Orin's updater downloads the portable binary
([ADR-009](design-decisions.md#adr-009-self-updating-portable-binaries)):

| Runner | Target | Asset |
| --- | --- | --- |
| `windows-latest` | `windows-x86_64` installer | `orin-<version>-windows-x86_64-setup.exe` |
| `windows-latest` | `windows-x86_64` | `orin-<version>-windows-x86_64.exe` |
| `ubuntu-24.04` | `linux-x86_64` | `orin-<version>-linux-x86_64` |
| `macos-latest` | `macos-aarch64` | `orin-<version>-macos-aarch64` |

Next to each asset is `<asset>.b3`, one `b3sum` line. The names come from `update::target::asset_name`;
the workflow asks `cargo run --bin orin-asset-name -- <target>` instead of spelling them out.

To build the Windows installer locally, install [Inno Setup 6](https://jrsoftware.org/isinfo.php),
run `cargo build --locked --release --bin orin`, then `./packaging/windows/build-installer.ps1` in PowerShell.
The script reads the version from Cargo and writes the setup executable to `dist/`.
Setup installs to `%LOCALAPPDATA%\Programs\Orin`, with Start menu and optional desktop shortcuts and an uninstaller.
This writable per-user location also supports the existing in-app updater. Settings are preserved on uninstall.
Run `./packaging/windows/test-installer.ps1 -Installer dist/orin-<version>-windows-x86_64-setup.exe`
to check installation, both shortcuts, the installed version, reinstall, and uninstall. The test refuses
to run over an existing Orin installation or shortcuts and cleans up its temporary installation.
The Windows release job runs the same test before uploading the installer.

Release builds are compiled with `ORIN_RELEASE_TARGET=<target>`. Without it (`cargo run`, `cargo build`) the binary is a
build from source: it never checks for or installs updates. Debug builds also read `ORIN_RELEASE_TARGET`,
`ORIN_UPDATE_SOURCES=<github-url>|<gitlab-url>` and `ORIN_UPDATE_DIR` at run time for tests; release builds ignore them.

1. Promote **Unreleased** in the changelog to `## [X.Y.Z]` and set `version` in `Cargo.toml` to the same number.
2. Push the tag `vX.Y.Z`. [`release.yml`](../.github/workflows/release.yml) fails at once if the tag and `Cargo.toml`
   disagree, then builds the three targets and Windows installer and creates a **draft** GitHub release with the eight files and that version's
   changelog section.
3. Download the draft's files and check every one with `b3sum --check <asset>.b3`.
4. Attach the same eight files to the GitLab release (`glab release create vX.Y.Z <files>`) and the Codeberg release.
   GitLab must have every asset and `.b3`: the updater falls back to it when GitHub can't answer.
5. Publish the GitHub draft. Orin's update check only sees published, non-pre-release releases.
