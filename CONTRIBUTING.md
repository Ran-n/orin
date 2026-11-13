[//]: # ( ---------------------------------------------------------------------- )
[//]: # (+ Authors: 	Ran# <ran.hash@proton.me> )
[//]: # (+ Created: 	2026/11/11 12:19:33.961358 )
[//]: # (+ Revised: 	2026/11/12 09:16:13.147042 )
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
For conduct concerns or reports unsuitable for public discussion, contact the maintainer using the Founder's Email field in [CONTRIBUTORS](CONTRIBUTORS).

---

## Reporting Bugs

Open an issue on [GitHub](https://github.com/Ran-n/orin/issues), [GitLab](https://gitlab.com/Ran-n/orin/-/issues),
or [Codeberg](https://codeberg.org/ran-n/orin/issues) containing:

- A clear, specific title
- Steps to reproduce the problem
- Expected behavior and actual behavior
- Your operating system, how you ran Orin (commit and command), and which agents were involved
- Relevant logs or screenshots, with credentials and private project information removed

Search existing reports first and link related issues across mirrors.

---

## Suggesting Features

Describe the problem, your proposed solution, and alternatives you considered.
Explain how the proposal serves Orin's [product vision](docs/vision.md) and fits its [scope](docs/vision.md#scope).
Proposals that contradict an accepted [design decision](docs/design-decisions.md) should explain why that decision should change.

---

## Submitting Changes

1. Fork the repository on your chosen [mirror](docs/development.md#get-the-repository) and clone your fork.
2. Create a focused branch from `main`.
3. Make the change, update the relevant documentation, and run applicable checks from the [development guide](docs/development.md).
4. Prepare the changelog when preparing the commit, then commit using the style below.
5. Open a pull request against `main`.
6. Describe why the change is needed, its behavior or documentation impact, and how you verified it.

Keep each pull request focused on one concern, and include tests with any code change.
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
