[//]: # ( ---------------------------------------------------------------------- )
[//]: # (+ Authors: 	Ran# <ran.hash@proton.me> )
[//]: # (+ Created: 	2026/11/11 12:19:33.826510 )
[//]: # (+ Revised: 	2026/11/13 15:50:00.276636 )
[//]: # ( ---------------------------------------------------------------------- )


# Changelog

All notable changes to Orin will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Added a Windows installer with per-user installation, Start menu and optional desktop shortcuts, and uninstall support
- Added installer install, reinstall, shortcut, version, and uninstall checks to Windows release builds

### Fixed

- Fixed an extra console window opening when launching Orin on Windows

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
