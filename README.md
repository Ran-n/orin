[//]: # ( ---------------------------------------------------------------------- )
[//]: # (+ Authors: 	Ran# <ran.hash@proton.me> )
[//]: # (+ Created: 	2026/11/11 12:19:33.694237 )
[//]: # (+ Revised: 	2026/11/11 12:43:49.367273 )
[//]: # ( ---------------------------------------------------------------------- )


<div align="center">

# Orin

**From intention to creation.**

An agent-first development environment by **Breren**, for developers who direct AI coding agents.

[![Read the vision](https://img.shields.io/badge/Read_the_vision-245A73?style=for-the-badge)](docs/vision.md)

[![Status: pre-implementation](https://img.shields.io/badge/status-pre--implementation-666666)](#project-status)
[![Desktop targets](https://img.shields.io/badge/desktop-Windows%20%7C%20Linux%20%7C%20macOS-245A73)](docs/design-decisions.md)
[![Approved language: Rust](https://img.shields.io/badge/approved_language-Rust-666666)](docs/design-decisions.md)
[![PayBack License (PBL) v2.0](https://img.shields.io/badge/license-PBL_v2.0-245A73)](LICENSE)

[Why Orin?](#why-orin) · [A closer look](#a-closer-look) · [Getting started](#getting-started) · [Questions](#questions) · [Technical details](#technical-details)

</div>

## Project status

Orin is at the **repository and documentation stage**. Its product vision and Rust + GPUI stack are approved.
Application code, working agent integrations, screenshots, and installers do not exist yet.
The capabilities below describe the intended product.

## Why Orin?

- **Direct the work:** start with your intention and let coding agents carry out the implementation.
- **Keep agents central:** supervise conversations and tasks in the main workspace.
- **Review with context:** keep project files, code, and changes alongside the agent's work.
- **Bring your tools together:** aim to unify Claude Code, Codex, and Gemini CLI in one workspace.
- **Stay focused:** prioritize speed, simplicity, low resource use, and efficient supervision.

## A closer look

| Planned workspace area | Intended purpose |
| --- | --- |
| Agent Workspace | Manage conversations, delegate tasks, and supervise multiple coding agents |
| Project Explorer | Navigate project folders and files |
| Code Viewer | Inspect files, changes, and diffs |
| Terminal | Execute commands and interact with the development environment |

The agent workspace is the central experience. The other areas support directing and reviewing the work.
See the [product vision](docs/vision.md) for the complete use case and name etymology.

## Download and install

There are no application downloads or installers at this stage. Windows, Linux, and macOS are the approved desktop targets.
Screenshots and release links will be added when real application builds are available.

## Getting started

1. Read the [vision and philosophy](docs/vision.md).
2. Review the [approved technical decisions](docs/design-decisions.md).
3. Check the [user guide's implementation status](docs/user-guide.md).
4. Read [how to contribute](CONTRIBUTING.md) before proposing a change.

## Privacy and your data

This repository contains documentation only. There is no Orin application that reads your projects or launches agents yet.
Project storage, agent permissions, and any data sharing will be documented with their implementation.

## Questions

<details>
<summary>Can I use Orin today?</summary>

The application has not been implemented. This scaffold establishes its documentation, contribution rules, and approved direction.

</details>

<details>
<summary>Which platforms will it target?</summary>

Windows, Linux, and macOS desktop applications. Web and mobile applications are outside the approved scope.

</details>

<details>
<summary>Which coding agents will it support?</summary>

Orin aims to unify tools such as Claude Code, Codex, and Gemini CLI. None of these integrations is implemented yet.

</details>

<details>
<summary>Is it free?</summary>

The [PayBack License (PBL) v2.0](LICENSE) permits free personal and non-commercial use.
Commercial use is subject to its revenue-sharing terms. See the license for the complete conditions.

</details>

<details>
<summary>How do I report a problem or suggest a feature?</summary>

Follow [Reporting Bugs](CONTRIBUTING.md#reporting-bugs) or [Suggesting Features](CONTRIBUTING.md#suggesting-features).
Use the issue tracker on the repository host where you found Orin.

</details>

## Run from source

Application implementation is outside this scaffold. There is no Cargo manifest, application source, or run/install command yet.
The [development guide](docs/development.md) records the current repository structure and future build requirements.

## Technical details

| Document | Contents |
| --- | --- |
| [Product vision](docs/vision.md) | Name, etymology, use case, and philosophy |
| [User guide](docs/user-guide.md) | Current availability and intended workflow |
| [Development guide](docs/development.md) | Repository layout, development expectations, and verification |
| [Design decisions](docs/design-decisions.md) | Approved Rust + GPUI desktop direction and implementation boundaries |
| [Contributing](CONTRIBUTING.md) | Bug reports, feature proposals, changes, and commit style |
| [Changelog](CHANGELOG.md) | Change history, populated when preparing commits |
| [Contributors](CONTRIBUTORS) | Authoritative ownership and revenue-share record |

**Approved implementation stack:** Rust + [GPUI](https://gpui.rs/).

## License

[PayBack License (PBL) v2.0](LICENSE).
Personal and non-commercial use is free; commercial use is subject to the license's revenue-sharing terms.
