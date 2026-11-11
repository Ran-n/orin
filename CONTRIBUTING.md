[//]: # ( ---------------------------------------------------------------------- )
[//]: # (+ Authors: 	Ran# <ran.hash@proton.me> )
[//]: # (+ Created: 	2026/11/11 12:19:33.961358 )
[//]: # (+ Revised: 	2026/11/11 12:43:49.489656 )
[//]: # ( ---------------------------------------------------------------------- )


Thank you for your interest in contributing.

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Reporting Bugs](#reporting-bugs)
- [Suggesting Features](#suggesting-features)
- [Submitting Changes](#submitting-changes)
- [Commit Style](#commit-style)
- [License](#license)

---

## Code of Conduct

Treat everyone with respect. Harassment, discrimination, personal attacks, and bad-faith participation are not acceptable.
Discuss ideas and technical decisions constructively, and respect people's privacy.

---

## Reporting Bugs

Open an issue on the repository host containing:

- A clear, specific title
- Steps to reproduce the problem
- Expected behavior and actual behavior
- Relevant environment details, including operating system and application version when applicable
- Relevant logs or screenshots, with credentials and private project information removed

The current repository contains documentation only; documentation errors can also be reported.

---

## Suggesting Features

Describe the problem, your proposed solution, and alternatives you considered.
Explain how the proposal serves Orin's [product vision](docs/vision.md).
Use the repository host's issue tracker to discuss the proposal.

Rust + GPUI and desktop targets for Windows, Linux, and macOS are approved.
Application implementation is outside the current repository-and-documentation scaffold.

---

## Submitting Changes

1. Fork the repository and clone your fork.
2. Create a focused branch from `main`.
3. Make the change, update the relevant documentation, and run applicable checks from the [development guide](docs/development.md).
4. Prepare the changelog when preparing the commit, then commit using the style below.
5. Open a pull request against `main`.
6. Describe why the change is needed, its behavior or documentation impact, and how you verified it.

Keep each pull request focused on one concern. Run applicable tests when a change introduces or modifies code.
Do not include credentials, private configuration, or unrelated generated files.

---

## Commit Style

Use a bracketed prefix in the title and a Conventional Commits body.

| Title prefix | Conventional Commits type | Purpose |
| --- | --- | --- |
| `[A]` | `feat` | Add a feature |
| `[R]` | `refactor` | Restructure existing code |
| `[F]` | `fix` | Fix a defect |
| `[D]` | `docs` | Change documentation |
| `[T]` | `test` | Add or change tests |
| `[C]` | `chore` | Maintain repository tooling or configuration |
| `[P]` | `perf` | Improve performance |
| `[B]` | `build` | Change builds or dependencies |
| `[S]` | `style` | Change formatting or code style |

Rules:

- Use the bracketed prefix in the title; do not replace it with a Conventional Commits prefix.
- Use a lowercase imperative verb in the Conventional Commits body; scope is optional.
- Identify breaking changes with `feat!` / `fix!` in the body or a `BREAKING CHANGE:` footer.
- Never add a `Co-Authored-By` line.
- Update the changelog in the same commit when preparing a commit; use the format in the [development guide](docs/development.md#changelog).

Example:

```text
[D] Document the approved desktop stack

docs: record rust and gpui as the approved implementation stack
```

---

## License

Contributions are governed by the [PayBack License (PBL) v2.0](LICENSE).
Personal and non-commercial use is free; commercial use is subject to its revenue-sharing terms.

Contributing does not transfer your copyright. Non-trivial contributions may entitle you to revenue sharing,
subject to listing in [CONTRIBUTORS](CONTRIBUTORS) and the license's quorum-vote process.
