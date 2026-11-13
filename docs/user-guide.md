[//]: # ( ---------------------------------------------------------------------- )
[//]: # (+ Authors: 	Ran# <ran.hash@proton.me> )
[//]: # (+ Created: 	2026/11/11 12:19:34.241299 )
[//]: # (+ Revised: 	2026/11/13 08:56:38.825473 )
[//]: # ( ---------------------------------------------------------------------- )


# User Guide

## Current Availability

Orin is in early development. Download the portable program from the [latest release](https://github.com/Ran-n/orin/releases/latest)
(see [Download and install](../README.md#download-and-install)) or run it [from source](../README.md#run-from-source). There are no installers yet.

| Area | Status |
| --- | --- |
| Files, Code (viewer and editor), and Terminal columns | Available |
| Agents column with Claude Code, Codex, and Gemini CLI over ACP | Available |
| Windows | Verified |
| Linux and macOS | Release downloads built, not yet verified |
| Release downloads and in-app updates | Available from 0.1.0 |
| Installers | Not available |

## Set Up Agents

Orin talks to agents through the [Agent Client Protocol](https://agentclientprotocol.com/) (ACP).
All you need is [Node.js](https://nodejs.org/) and to be signed in to the agents you use, such as Claude Code and Codex.
Orin fetches each agent's ACP adapter with `npx` the first time you start it (this first start can take a minute),
so every agent you are signed in to is ready from the start. An adapter you installed yourself is used instead.

| Agent | Command Orin runs | Fetched with `npx` when missing |
| --- | --- | --- |
| Claude Code | `claude-agent-acp` | `@agentclientprotocol/claude-agent-acp` (current 0.x release) |
| Codex | `codex-acp` | `@agentclientprotocol/codex-acp` (current 2.x release) |
| Gemini CLI | `gemini --acp` | `@google/gemini-cli` (current 0.x release) |

Orin has no account of its own and never asks for your credentials. The person button at the bottom of the activity bar opens the
**Accounts** menu, which shows each agent's sign-in; **Manage agent accounts…** opens **Settings › Accounts**, where
**Sign in** runs that agent's own login in the Terminal column. Signing in to Orin itself (for example to sync settings)
is shown in the menu but isn't available yet.

Without Node.js, and without an adapter of your own on your `PATH`, an agent's start button is greyed out and the Agents
column says what to install.

## The Welcome Page

When no project is open, Orin starts on **Welcome**, like VS Code's start page:

- **Start:** **Open Folder…** (Ctrl+Shift+O) picks a project folder with your system's folder browser; **Settings** opens Settings.
- **Recent:** the last ten folders you opened, newest first. Click one to open it; × removes it from the list
  (the folder itself is not touched). Hover a folder to see its full path.
- **Get started:** shortcuts to the theme and fonts, the agent accounts, and the column layout.

The Orin logo in the title bar, or the box in the middle of the title bar, brings Welcome back at any time;
**Back to** *project* returns to the columns.

## The Workspace

Orin shows four columns side by side, so everything is visible at once: **Files**, **Code**, **Agents**, and **Terminal**.
It is laid out like VS Code:

- The **activity bar** down the left edge has one icon per column, in the columns' order. A bar beside an icon marks a
  shown column; click the icon to hide or show it (from Welcome or Settings, it returns to the columns). The person and
  gear icons at the bottom open the Accounts menu and Settings.
- **Files** colours names by Git change and shows VS Code's letters: **M** modified, **A** added, **U** untracked,
  **D** deleted, **R** renamed, and **!** for a merge conflict; a folder with changes inside gets a dot.
  Files and folders Git ignores, such as build output, are shown in grey.
- The **status bar** shows the Git branch on the left and the open file's language on the right.

- **Move** a column by dragging its title (the icon and name in its header) left or right; the others make room as you go.
- **Resize** a column by dragging the divider next to it. The Agents column takes whatever width is left.
- **Hide** a column with the « button in its header; it shrinks to a narrow strip. Click the strip's icon to bring it back.
- **Reset** the column order, widths, and hidden columns under **Settings › Layout**.
- Orin remembers the order, every column's width, and whether it is hidden. Hover any icon button to see what it does.

## Settings

The gear button at the bottom of the activity bar (Ctrl+,) opens **Settings** in place of the columns; × or Escape closes it.
Every choice applies at once and is remembered.

- **Appearance**
  - **Theme:** Gruvbox Dark (the default), Gruvbox Light, Orin Dark, or Orin Light. Each card previews its colours.
  - **Interface font** and **Code font:** pick any installed font, or the system default. A sample line shows the choice.
    The code font is used by the editor, the terminal, diffs, and code blocks.
  - **Text size:** 10 to 20 pixels (default 13). Code, rows, buttons, and spacing grow with it.
    Ctrl+- and Ctrl+= change it from anywhere, and Ctrl+0 resets it.
- **Accounts** (also from **Manage agent accounts…** in the Accounts menu): each agent's sign-in, as its own CLI reports it: signed in (with the account it names), not signed in,
  or not installed. **Sign in** closes Settings and runs the agent's login in the Terminal column (`claude auth login`,
  `codex login`, or `gemini`), where you answer its prompts. **Check again** asks every agent once more.
  Signing in needs an open project, because the terminal starts in the project folder.
- **Layout:** reset the column order, widths, and hidden columns.
- **Updates:** the version you run and the result of the last check. Orin checks for a new release a minute after it
  starts and once a day after that; **Check for updates automatically** turns that off, and **Check now** checks at once.
  When a newer version has a build for your system, a banner says so: **Skip this version** stays quiet until a newer one
  appears (**Stop skipping** in Settings › Updates offers it again), and × hides the banner until Orin starts again
  or a different version is offered. **Update and restart** downloads the new version (**Cancel** stops it), checks it
  against its published BLAKE3 checksum, and reopens your project in it (unsaved edits are asked about first, as on
  every quit). While an agent is working, Orin doesn't cut it off: the banner offers **Restart now** instead. If Orin
  can't replace itself (for example in a protected folder), download it from the release page.
  A copy you built from source doesn't check: update it with `git pull` and a new build.

## Using Orin

1. **Open a project.** Choose **Open Folder…** on Welcome, a folder under **Recent**, or **Open** in the title bar
   (Ctrl+Shift+O). Orin reopens it next time. To switch folders, first save or reload an edited file and stop the
   running agents and terminal; until then the Open buttons are disabled and their tooltip says why.
2. **Browse files** in **Files**. Click a folder to expand it and a file to open it in **Code**.
   Each file shows an icon for its kind: README, licence, changelog, Git and lock files have their own, and other files
   are marked as code, script, document, config, data, image or archive. Folders such as src, tests, docs and build
   output have their own icons too; outside Git, build output is dimmed. Only Git's own `.git` folder is hidden.
   The list refreshes when files change on disk; the refresh button (Ctrl+Shift+R) forces it.
3. **Start an agent** with **+ Claude**, **+ Codex**, or **+ Gemini** in the Agents header.
   You can run several sessions at once; each gets a tab showing its status as a coloured dot.
   **Choose the model** (and, where the agent offers them, the reasoning effort and mode) from the buttons under the tabs.
   The list is the agent's own, so it follows what your plan includes. Orin remembers your choice for each agent once the
   agent accepts it, and new sessions of that agent start with it; a choice the agent refuses is reported and not kept.
4. **Give it an intention** in the message box. Enter sends; Shift+Enter adds a new line. **Cancel** stops the current turn.
5. **Answer permission requests.** When an agent asks to change files or run a command, the request is pinned above the
   message box with the agent's own choices (such as *Allow once* or *Reject*) plus **Decline**. The agent waits for your answer.
   Which actions ask first is set by each agent's own permission settings.
6. **Review changes.** Files an agent changed appear as **Review** buttons in its conversation and open in **Code**.
   Switch between **File**, **Unstaged**, and **Staged** to see the file or its Git changes.
7. **Edit by hand** in **Code**. The open file shows as a tab, and its × closes it (save or reload first if you have
   unsaved edits); undo, redo, reload, and save are in the column header.
   Code is coloured by language, as in VS Code, using the same TextMate-style grammars (Rust, Python, JavaScript,
   TypeScript, JSON, HTML, CSS, Markdown, shell, C, C++, Go, Java, and many more). The status bar shows the file's language.
   If the file changes on disk while you have unsaved edits, Orin keeps your edits and warns you instead of overwriting either version.
   For a Markdown file, the eye button (Ctrl+Shift+V) shows it rendered the way VS Code's built-in Markdown preview does
   (its type sizes, spacing, rules, quote bars, tables, and coloured code blocks), and follows your unsaved edits; the pencil button switches back to the source. Links to web pages open in your browser and
   links to project files open in **Code**. Raw HTML in the file isn't shown in the preview. Orin remembers which view you used.
8. **Run commands** in **Terminal**: your system shell starts in the project folder as soon as the project opens.
   The stop button ends it and the play button starts a new one.

## Keyboard Shortcuts

| Action | Windows and Linux | macOS |
| --- | --- | --- |
| Open project | Ctrl+Shift+O | Cmd+Shift+O |
| Refresh project | Ctrl+Shift+R | Ctrl+Shift+R |
| Show or hide the Terminal column | Ctrl+\` | Ctrl+\` |
| Larger text | Ctrl+= | Cmd+= |
| Smaller text | Ctrl+- | Cmd+- |
| Default text size | Ctrl+0 | Cmd+0 |
| Preview Markdown or edit its source | Ctrl+Shift+V | Cmd+Shift+V |
| Open Settings | Ctrl+, | Cmd+, |
| Close Settings or a menu | Escape | Escape |
| Quit | Ctrl+Shift+Q | Cmd+Q |

## Command-Line Options

| Option | Effect |
| --- | --- |
| `--project PATH` | Open this project folder |
| `--settings FILE` | Use this settings file instead of the default one |
| `--help` | Show the options |
| `--version` | Show the version |

## Your Data

- Orin reads and writes only inside the project folder you open, plus its own settings file.
- The settings file holds the window size and position, the column order, widths, and hidden columns, the theme, fonts, and text size, the open page (Welcome or Settings), the open project, the recent folders,
  each agent's chosen model, effort, and mode, the selected file, expanded folders, view choices, and the update
  choices (automatic checks on or off, the time of the last check, and a skipped version).
  It is stored in your system's per-user configuration folder (on Windows, `%APPDATA%\Breren\Orin\config\workspace.json`).
  If it is damaged, Orin starts with defaults, leaves the file untouched, and doesn't overwrite it. After you repair
  it, **Retry saving** reads it again and keeps its settings, with the window and project you have open laid over them.
- Conversations and credentials are not stored by Orin. Each agent keeps its own history and sign-in.
  The Accounts page runs `claude auth status` and `codex login status`, and for Gemini CLI only checks whether
  `~/.gemini/oauth_creds.json` or an API key variable exists; Orin never reads or keeps what they contain.
- Orin sends no telemetry. Its network use is `npx` fetching a missing adapter, and a daily update check against
  GitHub (GitLab if GitHub can't be reached), which you can turn off in **Settings › Updates**. Neither sends any personal
  data: the update check sends only the request and `User-Agent: Orin/<version>`. What an agent sends to its vendor is
  governed by that agent and its vendor.
- Update downloads wait in the cache folder (on Windows, `%LOCALAPPDATA%\Breren\Orin\cache\updates`) only until they
  are installed; a download that fails, doesn't match its checksum, or is interrupted is deleted.

## Help

Report problems through [Reporting Bugs](../CONTRIBUTING.md#reporting-bugs) and send proposals through
[Suggesting Features](../CONTRIBUTING.md#suggesting-features).
