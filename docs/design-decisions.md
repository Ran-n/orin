[//]: # ( ---------------------------------------------------------------------- )
[//]: # (+ Authors: 	Ran# <ran.hash@proton.me> )
[//]: # (+ Created: 	2026/11/11 12:19:34.382590 )
[//]: # (+ Revised: 	2026/11/13 07:53:53.034413 )
[//]: # ( ---------------------------------------------------------------------- )


# Design Decisions

Architecture decision records (ADRs) for Orin. **Accepted** decisions are binding.
**Proposed** decisions are open for discussion and need the project owner's approval before implementation depends on them.

| ADR | Decision | Status |
| --- | --- | --- |
| [001](#adr-001-rust--gpui-desktop-application) | Rust + GPUI desktop application | Accepted |
| [002](#adr-002-documentation-before-application-code) | Documentation before application code | Accepted |
| [003](#adr-003-one-selected-logo-and-mascot) | One selected logo and mascot | Accepted |
| [004](#adr-004-agent-integration-model) | Agent integration model | Accepted: structured protocols |
| [005](#adr-005-the-code-viewer-is-also-an-editor) | The Code Viewer is also an editor | Accepted |
| [006](#adr-006-four-side-by-side-columns) | Four side-by-side, movable columns in a compact, personal style | Accepted |
| [007](#adr-007-settings-page-and-agent-accounts) | A Settings page, Gruvbox Dark by default, and agent sign-ins instead of an Orin account | Accepted |

## ADR-001: Rust + GPUI Desktop Application

**Status:** Accepted · **Date:** 2026-11-11

### Context

Orin's [vision](vision.md) prioritizes agent supervision, speed, simplicity, and low resource use.
The owner requires a native Rust desktop application for Windows, Linux, and macOS.

### Decision

Use **Rust + GPUI** for the whole application, including its interface.
GPUI was chosen over Iced, egui/eframe, Floem, and Slint because it suits a custom development workspace
and is the UI foundation of the Zed editor. Web and mobile applications are out of scope.

### Consequences

- Application code is written in Rust only.
- The Rust toolchain and GPUI version are pinned when application work begins.
- Behavior is verified on all three desktop platforms.
- Responsiveness and resource use are goals to measure, not claims; no numeric budgets have been set.

### References

- [GPUI](https://gpui.rs/)
- [Upstream GPUI documentation](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md)

## ADR-002: Documentation Before Application Code

**Status:** Accepted · **Date:** 2026-11-11

### Decision

Settle the product vision, technical direction, and contribution rules before writing application code.
The documentation describes only what exists; planned capabilities are always marked as planned.

### Consequences

- The repository publishes no build, run, or install commands, screenshots, or downloads until a real application exists.
- Documentation-only changes do not require a version bump or release.

## ADR-003: One Selected Logo and Mascot

**Status:** Accepted · **Date:** 2026-11-11

### Decision

Use [orin-mascot.png](../assets/branding/orin-mascot.png), a germinating acorn (landra), as Orin's logo and mascot.
It is the only branding asset and stays at this stable, unversioned path.

### Consequences

- The README and future application branding use this image unchanged.
- New designs or variants require the owner's approval and replace this one rather than sitting beside it.

## ADR-004: Agent Integration Model

**Status:** Accepted by the project owner · **Date:** 2026-11-11

### Context

How Orin communicates with coding agents determines almost everything else: what the Agent Workspace can show,
how changes reach the Code Viewer, and what the Terminal is for.
Supervising several agents requires structured information: status, pending questions, tool calls, and the files each agent changed.
The supported agents, Claude Code, Codex, and Gemini CLI, are interactive terminal programs,
and some also offer machine-readable modes or adapters for structured protocols.

### Options

| Option | How it works | Gains | Costs |
| --- | --- | --- | --- |
| **A. Terminal wrapping** | Run each vendor CLI in its own pseudo-terminal and display its screen | Works with every CLI today; the agent behaves exactly as when run by hand | Orin only sees text on a screen: status, questions, and changed files must be guessed from output, so most supervision features are weak or fragile |
| **B. Structured protocol** | Talk to agents through a protocol such as the [Agent Client Protocol](https://agentclientprotocol.com/) (ACP) or a vendor's machine-readable mode | Orin receives messages, tool calls, permission requests, and edits as data, which enables real supervision and per-agent diffs | Depends on each agent's protocol support or a maintained adapter; Orin must build its own conversation UI |
| **C. Hybrid** | Use a structured protocol where an agent supports one, with terminal wrapping as a fallback | Full supervision where available, while still supporting every installed CLI | Two integration paths to build, test, and keep consistent |

### Questions to Resolve

- Which protocol or protocols, and at which versions, for each supported agent
- How agent permission requests (file writes, commands) are presented and approved
- How sessions are started, resumed, cancelled, and cleaned up on every quit path
- Whether the Terminal pane is for the developer's own commands only, or also displays wrapped agents

### Decision

The owner selected **Option B: Structured protocol** on 2026-11-11.
Agent conversations, status, permission requests, tool calls, and change information are handled as structured data.
The Terminal pane runs the developer's own shell commands; it is not the agent conversation interface.

## ADR-005: The Code Viewer Is Also an Editor

**Status:** Accepted · **Date:** 2026-11-11

### Decision

The Code Viewer displays files, changes, and diffs, and also works as a code editor whenever the developer needs to change a file by hand.

### Consequences

- Manual editing is a supported workflow, not a fallback. Saving, undo, and conflict handling with agents editing the same file are part of its design.
- Editing supports agent-driven work: language tooling beyond what reviewing and small-to-moderate edits need is not a goal.

## ADR-006: Four Side-by-Side Columns

**Status:** Accepted · **Date:** 2026-11-11

### Context

The first interface showed either the agents or the code, chosen with tabs, with the terminal stacked underneath.
Supervising agents means watching a conversation while reading the files it changes, so switching views got in the way.

### Decision

Show **Files**, **Code**, **Agents**, and **Terminal** as four columns side by side, splitting the window by width.
On 2026-11-12 the owner set this as the default order, with the editor beside the files.
Agents absorbs the leftover width as the centre of the product; dividers resize the other columns, and any column can be
collapsed to a narrow rail. On 2026-11-12 the owner asked for the layout to be personal: dragging a column's title moves
it anywhere in the row, and a title-bar button resets the order, widths, and collapsed columns.
The visual style is a compact native IDE: a thin title bar, slim column headers, icon buttons with tooltips, and
one primary action per area. It comes in dark and light palettes, and one text size (10–20 px, default 13) scales text,
code, rows, controls, and spacing together. Order, widths, collapsed columns, theme, and text size are remembered between runs.

### Consequences

- Settings schema 2 stores the column layout and appearance; schema 1 files are upgraded on load and rewritten on the
  next save. The column order and appearance were added to schema 2 with defaults, so earlier schema 2 files still load.
- The active palette and text size are process-wide, like the system theme, and applied before every frame.
  Column widths are layout geometry and do not scale with the text size.
- The default window is 1440 × 860 and the minimum stays 1000 × 680, where every column fits at its minimum width.
- Rendered UI tests check that all expanded columns sit side by side in the saved order, and that moving, resizing,
  collapsing, theme, text size, and reset persist.
- File icons are monochrome Lucide glyphs per file category (README, licence, changelog, contributing, people, Git, lock,
  build, code, web, script, document, config, data, image, archive, secret), tinted with existing palette colours, so
  every theme stays coherent without new colours; name rules beat document extensions (`README.md`) but not code
  (`history.rs`). Brand logos and multi-colour icons were rejected because GPUI's `svg()` tints a single colour and
  Lucide ships no brand marks. Folders get a role glyph (source, tests, docs, Git/CI, assets, config, build output)
  that stays when opened; plain folders switch to the open-folder glyph. Build output is listed rather than hidden
  (only `.git` is hidden), and fades: by Git's ignore rules in a repository, by name outside one, as in VS Code.

## ADR-007: Settings Page and Agent Accounts

**Status:** Accepted · **Date:** 2026-11-12

### Context

The owner asked for a configuration area and an account button like VS Code's, with a font chooser and a theme chooser,
and for the Gruvbox Dark (medium) theme to be the default. Orin's vision rules out an Orin account of its own.

### Decision

- A **Settings** page (gear button, Ctrl+,) replaces the columns while open, like a settings tab, with four sections:
  **Appearance** (theme cards, interface font, code font, text size), **Accounts**, **Layout** (reset the columns), and
  **Updates** (added by [ADR-009](#adr-009-self-updating-portable-binaries)).
  Choices apply at once and are saved; there is no confirm step. The open section is remembered.
- Themes: **Gruvbox Dark** (medium contrast, the default), Gruvbox Light, Orin Dark, and Orin Light.
  Settings files saved with Orin's earlier dark or light theme keep it.
- Fonts come from the installed font list, plus "System default". GPUI 0.2.2 also lists its own fallback families
  whether or not they are installed, so those are offered only where they ship with the operating system.
- The account button opens an **Accounts** menu, like VS Code's (see [ADR-008](#adr-008-vs-code-style-first-run-and-agents-ready-from-the-start)).
  **Settings › Accounts** shows each agent's sign-in as its own CLI reports it
  (`claude auth status --json`, `codex login status`, and for Gemini CLI only whether its Google sign-in file or an API
  key exists). **Sign in** types that agent's login into the Terminal column using the full path of the CLI it checked,
  so the shell can't run a different copy. Orin still has no account and never reads or stores credentials.

### Consequences

- The title bar held the project, the account and Settings buttons, and **Open**; text size, theme, and layout reset
  moved into Settings (their keyboard shortcuts remain). [ADR-008](#adr-008-vs-code-style-first-run-and-agents-ready-from-the-start)
  later moved the account and Settings buttons to the activity bar.
- Checking accounts starts the agents' CLIs, which may contact their vendors; Orin itself made no network requests until [ADR-009](#adr-009-self-updating-portable-binaries) added the update check.
- Rendered tests use a stand-in `codex` CLI on a restricted `PATH`, so they never reach a real agent or account.

## ADR-008: VS Code-Style First Run and Agents Ready from the Start

**Status:** Accepted · **Date:** 2026-11-12

### Context

The owner asked for a more modern look, for Claude Code and Codex to be usable as soon as the user is signed in, with
any model, for the shell to start by itself, for syntax highlighting and a Markdown preview like VS Code's, for a
VS Code-like account menu as a placeholder (no server for real sign-in), and for a Welcome page with recent projects.

### Decision

- **Agents:** an adapter on `PATH` is used as before; otherwise Orin runs it with `npx -y <package>` pinned to the
  adapter's current major version (`claude-agent-acp@0`, `codex-acp@2`, `gemini-cli@0`; npm reads a bare major as `^major`, and `^` is an escape character for `cmd.exe`), so new models arrive without
  an Orin release and breaking releases don't. The first start may download, so it gets 300 s instead of 30 s.
- **Models:** the session's ACP `configOptions` of category `model`, `thought_level`, and `mode` are shown as pickers
  and changed with `session/set_config_option`; older adapters' `models` list uses `session/set_model`. A choice is
  remembered per agent only after the agent reports it (a refused one is reported and not kept), and new sessions of
  that agent are sent it once they are ready.
- **Terminal:** the default shell starts when a project opens, without taking keyboard focus.
- **Welcome:** shown on a start without a project and from the title bar's logo or command-center box: Open Folder
  (native folder picker), the ten most recent folders (absolute paths, removable), and Get started links.
- **Accounts menu:** a placeholder Orin profile ("Not signed in") with a disabled "Sign in to sync settings" marked
  *Coming later*, then each agent's sign-in and **Manage agent accounts…**. No Orin sign-in exists or is stored.
- **Syntax highlighting:** `syntect` with its bundled Sublime/TextMate grammars. Scopes are mapped to thirteen token
  kinds that each theme colours (Gruvbox colours for the Gruvbox themes, Dark+/Light+ colours for Orin's). Parsing is
  incremental per line and stops at the last visible line; lines over 20,000 bytes stay plain.
- **Markdown preview:** follows VS Code's built-in preview stylesheet: 14 px text on a 22 px line, 26 px side padding,
  browser heading sizes with normal-weight h1–h3 and a rule under h1, collapsing block margins, 16 px code blocks
  highlighted by fence language, 5 px quote bars, row-ruled tables, and depth-dependent bullets.
- **Look:** VS Code's "Modern" layout, which the owner showed from their own setup: a title bar with a centred
  command-center box; an activity bar on the left with one icon per column (marking shown columns, toggling them) and
  the account and Settings buttons at the bottom; uppercase view titles; an editor tab with an accent line and a close
  button; Git decorations in Files (VS Code's letters and colours, read with `git status --porcelain=v1 -z` and
  `--ignored=matching`, ignored paths greyed, and refreshed by the file watcher, which also sees Git's index and HEAD;
  Git runs with `--no-optional-locks`, so reading status never rewrites the index and never wakes the watcher by
  itself); file-type icons (see [ADR-006](#adr-006-four-side-by-side-columns)); and the branch and language mode in the
  status bar.

### Consequences

- Node.js becomes the one prerequisite for agents not installed by hand, and `npx` reaches the npm registry; Orin itself
  still sends nothing.
- `settings.json` gains `page` (replacing `settings_page`, whose value is dropped once), `recent_projects`, and
  `agent_options`; the schema stays at version 2 because the fields are additive with defaults.
- The offline ACP fixture now offers models, effort, and mode, and refuses one model, so these paths are tested end to end.

## ADR-009: Self-Updating Portable Binaries

**Status:** Accepted · **Date:** 2026-11-12

### Context

Updates were open in area 9. The owner wants Orin to update itself from inside the app, checking automatically with an
opt-out, asking GitHub first and GitLab when GitHub can't answer.

### Decision

- **Releases:** pushing a `vX.Y.Z` tag builds one portable binary per target (`windows-x86_64`, `linux-x86_64`,
  `macos-aarch64`) and attaches each with a BLAKE3 `.b3` file to a draft GitHub release. The names come from
  `update::target::asset_name` (`orin-<version>-<target>`, `.exe` on Windows), printed for the workflow by
  `orin-asset-name`, so CI and the client can't drift apart. A tag that doesn't match `Cargo.toml` fails before building.
- **Which build:** release builds are compiled with `ORIN_RELEASE_TARGET`. Any other build is from source and never
  checks or replaces itself, so `target/debug/orin` or a hand-made binary can't be overwritten.
- **Check:** `releases/latest` from GitHub, then GitLab's `releases/permalink/latest` when GitHub fails (network,
  any non-200 such as a rate limit, or unparsable JSON); 10 s to connect, 30 s in total, HTTPS only, and only
  `Accept` and `User-Agent: Orin/<version>` are sent. A release is offered when it is newer, has this target's binary and
  `.b3`, and is not a pre-release (unless a pre-release is running; GitHub's `releases/latest` skips releases marked
  as pre-releases, so a pre-release build is mostly offered stable releases). The first automatic check runs 60 s after the
  window opens, then 24 h after the last successful one (1 h after a failure); a failed automatic check only updates
  Settings › Updates, while **Check now** also shows the error. Checks run on their own thread and are abandoned at quit.
- **Install:** **Update and restart** streams the binary (at most 200 MiB, 60 s without data) into the cache's
  `updates/` folder while hashing it, compares the hash with the `.b3` before anything is replaced, swaps it in with
  `self-replace` (which moves a running `.exe` aside on Windows), then closes the way every quit does and starts the new
  binary with the same `--project` and `--settings` after the final settings save. A download stops after 30 minutes.
  When an agent is mid-turn the restart waits for **Restart now**. **Cancel** or quitting mid-download stops it and
  deletes the file; quitting waits up to 5 s for the download thread, so Orin never exits halfway through replacing
  itself. Leftovers are deleted at the next start.
- **Hosts:** asset URLs are not pinned to forge hosts. Integrity comes from the `.b3` fetched over HTTPS from the same
  release, and the forges move downloads between hosts (`objects.githubusercontent.com`,
  `release-assets.githubusercontent.com`), so a host list would break updates without adding a check the hash lacks.

### Consequences

- Installers, code signing and notarisation stay open in area 9; there is no rollback yet.
- GitLab must carry every asset and `.b3` of a release, because the check falls back to it; the `dbp` release flow
  attaches the GitHub assets to GitLab and Codeberg.
- Orin's network use grows from `npx` alone to include the daily check, which can be turned off.
- Debug builds accept `ORIN_RELEASE_TARGET`, `ORIN_UPDATE_SOURCES` and `ORIN_UPDATE_DIR` at run time, so rendered tests
  exercise the whole path against a local server; release builds ignore them.

## Implementation Status of Open Areas

The first implementation settled several areas in code. They can still be revisited; the remaining items are open.

| # | Area | Settled in the current code | Still open |
| --- | --- | --- | --- |
| 1 | Agent integration | ACP protocol version 1 over each adapter's standard input and output (`claude-agent-acp`, `codex-acp`, `gemini --acp`), fetched with `npx` when not installed; model, effort, and mode pickers from the agent's config options; permission requests answered in the UI; turns can be cancelled; sessions stop with their process tree | Offline use without a cached adapter; resuming sessions across restarts; whether Orin should offer file-system access to agents |
| 2 | Toolchain and GPUI | Rust 1.95.0 and GPUI 0.2.2, pinned; a single crate whose domain modules don't depend on GPUI | Component libraries, if any |
| 3 | Workspace behavior | Four side-by-side columns that move, resize, and collapse ([ADR-006](#adr-006-four-side-by-side-columns)); a Settings page with four themes, interface and code fonts, text size, and agent sign-ins ([ADR-007](#adr-007-settings-page-and-agent-accounts)); Welcome page with recent folders, accounts menu, syntax highlighting, and a VS Code-style Markdown preview ([ADR-008](#adr-008-vs-code-style-first-run-and-agents-ready-from-the-start)); folder picker, lazy explorer with file watching, 1 MiB viewing limit, File / Unstaged / Staged views, editor with undo, redo, and conflict protection | Diff size limits beyond the file limit; multiple open files; ellipsis for long names (GPUI 0.2.2 clips them) |
| 4 | Terminal | `portable-pty` with a `vt100` screen model; the shell starts with the project; its whole process tree is stopped on close | Multiple terminals |
| 5 | Data and settings | JSON settings (schema version 2; version 1 files are upgraded) in the per-user configuration folder, saved atomically; damaged files are preserved, never overwritten | Migrations for future schema versions |
| 6 | Privacy and connectivity | No telemetry; network use is `npx` fetching a missing adapter, and a daily update check against GitHub (GitLab if GitHub can't be reached), which you can turn off in Settings › Updates; neither sends any personal data | A written privacy policy for releases |
| 7 | Platform support | Verified on Windows; CI builds and tests Windows, Linux, and macOS | Minimum OS versions, CPU architectures, graphics requirements |
| 8 | Performance | — | Budgets for startup time, idle memory and CPU, and streaming output from several agents |
| 9 | UI and distribution | English only; four themes; native rendered UI tests on Windows; portable release binaries with BLAKE3 checksums and in-app updates ([ADR-009](#adr-009-self-updating-portable-binaries)) | Linux and macOS rendered tests, other locales and themes, installers, signing, and rollback |
