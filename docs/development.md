[//]: # ( ---------------------------------------------------------------------- )
[//]: # (+ Authors: 	Ran# <ran.hash@proton.me> )
[//]: # (+ Created: 	2026/11/11 12:19:34.092613 )
[//]: # (+ Revised: 	2026/11/11 12:43:49.548805 )
[//]: # ( ---------------------------------------------------------------------- )


# Development Guide

## Current State

This repository is a documentation scaffold for Orin by Breren.
The approved application stack is **Rust + GPUI**, targeting **Windows, Linux, and macOS desktop**.
There is no application source, Cargo manifest, build pipeline, installer, or application test suite yet.

## Repository Layout

```text
orin/
├── .claude/memory/          Project-specific specification and memory index
├── docs/                   Product vision, user guide, development, and decisions
├── .gitattributes          Text and binary file handling
├── .gitignore              Rust build output, local diagnostics, and secrets
├── CHANGELOG.md             Keep a Changelog skeleton
├── CONTRIBUTING.md          Contribution process and commit style
├── CONTRIBUTORS             Ownership and revenue-share record
├── LICENSE                  Canonical project license
└── README.md                Public-facing project introduction
```

Keep global user configuration outside this repository.
The project memory contains only Orin-specific requirements and approved decisions.

## Approved Architecture

Use Rust for application code and GPUI for the desktop interface.
The product consists of the agent workspace, project explorer, code viewer, and terminal described in the [vision](vision.md).
See [Design decisions](design-decisions.md) for the accepted framework choice and current implementation boundary.

Dependency revisions, the Rust toolchain version, source-package boundaries, storage formats, agent protocols, and terminal integration
will be established with application implementation. This scaffold does not prescribe unapproved implementations.

## Working on Documentation

1. Read the [product vision](vision.md) and the project-specific specification before changing intended behavior.
2. Keep the README public-facing; put detailed user and developer material in the relevant guides.
3. Distinguish implemented behavior from planned capabilities.
4. Preserve working relative links and anchors, and avoid invented screenshots, release URLs, or commands.
5. Use the maintainer's metadata CLI to inspect/create Markdown headers before editing and refresh them afterward.

The metadata tool supports Markdown and Rust source headers. Extensionless records, Git configuration, and the canonical license
keep their required formats without generated metadata headers. Never rewrite license contents through an editor.

## Verification at This Stage

For documentation changes, verify:

- All five required repository documents exist and contain substantive content
- Local document links and anchors resolve
- The README follows the prescribed public-first order, adapted honestly to pre-implementation status
- The contribution guide contains every required section, separator, and commit-prefix row
- The canonical license is copied unchanged and every license reference names its version
- Contributor ownership and revenue total 100%, with the prescribed founder floors and verified GPG fingerprint
- Metadata headers exist on all Markdown documents and are updated through the CLI
- Contents stay within the documentation scaffold and accurately describe planned capabilities

Inspect the repository state with:

```sh
git status --short --branch
```

Application builds and tests cannot run until application source and a Cargo manifest exist.
No application tests or builds are claimed for this documentation-only scaffold.

## Requirements for Future Application Work

These are development standards, not functionality supplied by this scaffold:

- Pin the Rust toolchain and GPUI dependencies; keep the application's Cargo lockfile in version control
- Add rustfmt, Clippy, and applicable Rust tests alongside new code
- Exercise unit, integration, property, contract, lifecycle, and rendered UI cases wherever applicable
- Establish coverage reporting and a coverage gate; examine uncovered code and branches
- Give every shipped UI target the same test depth and reviewed screenshot baselines
- Keep UI automation from taking over the user's real mouse or keyboard
- Configure named, leveled logging before application startup; surface errors without logging secrets
- Keep disk and process work off the UI thread; bound concurrent independent work and clean up on every quit path
- Persist user-changeable settings and keep all supported translations complete

Platform prerequisites and verified build, run, test, and packaging commands will be added alongside the application bootstrap.
Until then, consult the [Rust documentation](https://www.rust-lang.org/learn) and [GPUI documentation](https://gpui.rs/) for background.

## Changelog

The changelog records pending changes under **Unreleased**.
Update it only when preparing a requested commit, and include it in that same commit.

Use Keep a Changelog categories: **Added**, **Changed**, **Deprecated**, **Removed**, **Fixed**, and **Security**.
Write concise entries in past tense, one bullet per notable change, without a trailing period.
Release headings use `## [x.y.z] — YYYY-MM-DD`.

## Git and Publication

Use `main` as the default branch and mirror the repository to GitHub, GitLab, and Codeberg over SSH.
Documentation-only changes do not require a version bump, tag, or release.
Commit titles and bodies follow [Commit Style](../CONTRIBUTING.md#commit-style).
Before a requested commit or push, inspect and enforce the applicable Git guard, changelog, and leak-scan checks.

## License

Orin uses the [PayBack License (PBL) v2.0](../LICENSE).
Keep its version consistent across documents, and preserve the [contributor record](../CONTRIBUTORS) format.
