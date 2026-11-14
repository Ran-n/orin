[//]: # ( ---------------------------------------------------------------------- )
[//]: # (+ Authors: 	Ran# <ran.hash@proton.me> )
[//]: # (+ Created: 	2026/11/11 12:19:33.826510 )
[//]: # (+ Revised: 	2026/11/14 22:00:10.655974 )
[//]: # ( ---------------------------------------------------------------------- )


# Changelog

All notable changes to Orin will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Added a Close Folder button, and made the Orin logo and title bar box close the current folder and return to Welcome
- Added saved agent tool folders and per-agent setup links in Settings › Agent accounts
- Added Claude, OpenAI, and Gemini logos to agent tabs, launchers, and setup cards
- Added an editor "…" menu holding Save, Undo, Redo, and Reload from disk
- Added formatted agent replies: Markdown with headings, lists, links, and coloured code blocks
- Added the agent's plan as a checklist pinned above the message box
- Added the command and files an approval request names to its panel, with paths shown relative to the project
- Added launch buttons for each agent in the empty Agents column, with Set up agents when one can't start
- Added an Install guide button on the account card of an agent whose CLI is missing
- Added signed releases: each asset has a `.sig` made with `orin-sign`, and the updater installs only what a release key signed
- Added a backup release key, so a lost or stolen signing key can be replaced without stranding installed copies
- Added build provenance attestations to release builds, checked before a release is signed

### Changed

- Changed file and agent tabs to a flat, VS Code-style look and removed the redundant Code and Agents headings
- Changed agent discovery to also search standard user installations and Windows' current user and machine paths
- Changed conversations to show tool calls where they happened, each with a status mark, instead of the last four below the reply
- Changed the message box to wrap long lines, grow with its text, use the interface font, and show Stop in place of Send while the agent works
- Changed Open Folder on Welcome to a prominent button
- Changed model, effort, mode, and font pickers to show their value followed by a chevron
- Changed empty columns to a centred icon and hint, and an empty diff to say which changes are absent
- Changed Settings › Accounts to list tool folders in their own section
- Changed Home and End in the message box to move within the wrapped row the caret is on
- Changed the Orin logo to fill its space: larger in the title bar, on Welcome, in the empty Agents column, and as the Windows icon

### Fixed

- Fixed Recent folders not opening while the automatically started shell was running
- Fixed the installed app not finding agent CLIs when launched from Explorer or the Start menu
- Fixed terminal input dropping characters when its input queue was full
- Fixed one malformed agent update ending the whole session
- Fixed an agent's plan being appended to its reasoning again on every update
- Fixed a file in another encoding hiding its Git diff and every Git decoration
- Fixed a non-repository folder reporting a Git error when Git answers in another language
- Fixed a file deleted while its folder was listed failing the whole folder
- Fixed quitting waiting without limit on a background job blocked by an unreachable folder
- Fixed a program that floods terminal queries without reading the answers growing memory without limit
- Fixed long conversations being copied and re-measured on every poll and streamed chunk
- Fixed the message box and the editor not scrolling to keep the caret in view
- Fixed a second Orin window deleting the update another window was still downloading
- Fixed stopping a session on Linux and macOS being able to signal an unrelated process that reused its process id

### Security

- Updates are refused unless signed with one of Orin's release keys for that exact version and file, so a forge account or download host alone can no longer deliver a build

## [0.2.0] - 2026-11-13

### Added

- Added a Windows installer with per-user installation, Start menu and optional desktop shortcuts, and uninstall support
- Added installer install, reinstall, shortcut, version, and uninstall checks to Windows release builds

### Fixed

- Fixed an extra console window opening when launching Orin on Windows
- Fixed release checks comparing the ACP handshake against an outdated version

## [0.1.0] - 2026-11-13

### Added

- Added Orin's product vision, Galician name etymology, and agent-first workspace requirements
- Documented the approved Rust + GPUI desktop stack for Windows, Linux, and macOS
- Showed the Orin logo as the window icon in the Windows title bar and taskbar
- Added repository licensing, contributor records, contribution guidelines, and development documentation
- Added file-type icons and colours in the Files column and Code header
- Added icons for open and well-known folders, with build output dimmed
- Greyed Git-ignored files and folders in Files
- Added release builds for Windows, Linux and macOS with BLAKE3 checksums
- Added a daily update check with a Settings › Updates section and an opt-out
- Added one-click updates that download, verify and restart into the new version
- Added Cancel for an update download and Stop skipping in Settings › Updates

### Changed

- Build output folders (`target`, `node_modules`, `dist`, `build`, `.venv`) are listed in Files, greyed, instead of hidden; only `.git` stays hidden

### Fixed

- Reading Git status no longer rewrites the index and triggers an extra refresh
- Opening a folder no longer runs a program named by its Git `core.fsmonitor` setting
- A broken symlink no longer stops its folder from listing in Files
- Changes inside a nested `build` or `dist` folder (such as `src/build/`) now refresh Files
- An agent that takes a few seconds to cancel keeps its session instead of being failed after 5 s
- An agent refusing a message or a picker change shows the reason and keeps the session
- Programs in the Terminal that ask for the cursor position get the right answer
- Stopping a session or quitting no longer waits forever on a process that kept its output open
- Sign-in is refused, with a message, for a CLI whose path contains `%`, which cmd.exe would expand
- Retry saving re-reads a settings file that couldn't be read at start, so a repaired file isn't overwritten
- Loading a folder no longer discards unsaved edits made while it loaded
- Open buttons and Recent folders are disabled, with the reason, while opening would be refused
- A finished update waits for busy agents before restarting, and quitting mid-install finishes the install first
- A failed update check keeps the version already offered, and × keeps the banner hidden for that version
- Fixed agents failing to start on Windows when `npx` is a `.cmd` launcher (nvm4w, nvm-windows, the Node.js installer): adapter package specs no longer use `^`, and the error banner now shows the underlying cause
