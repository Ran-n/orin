[//]: # ( ---------------------------------------------------------------------- )
[//]: # (+ Authors: 	Ran# <ran.hash@proton.me> )
[//]: # (+ Created: 	2026/11/11 12:19:34.514712 )
[//]: # (+ Revised: 	2026/11/11 12:30:46.524377 )
[//]: # ( ---------------------------------------------------------------------- )


# Orin — Use Case & Etymology

This is Orin's product brief. It describes the intended application; the current repository contains documentation only.

## Name & Etymology

**Orin** is an agent-first development environment developed by **Breren**.

Its name is an invented construction inspired by two Galician words:

- **Or** — From *orixe* (origin), representing the beginning of an idea, creation, or project.
- **In** — From *intelixencia* (intelligence), representing reasoning, autonomy, and the ability to transform ideas into reality.

Together, **Orin symbolizes "Intelligence at the Origin of Creation."**

The name reflects a new approach to software development, where human intention is the starting point and artificial intelligence carries out the implementation.

**Tagline: From intention to creation.**

## Use Case

Orin is an **agent-first development environment** designed for developers who primarily use AI agents to write, modify, test, and maintain software.

Unlike traditional IDEs, where the code editor is central, Orin prioritizes supervising and interacting with AI coding agents.

The developer directs the work, reviews the results, and manages the project rather than manually writing most of the code.

### Core Workspace

- **Project Explorer:** Navigate folders and files.
- **Code Viewer:** Inspect files, changes, and diffs.
- **Agent Workspace:** Manage conversations, delegate tasks, and supervise multiple AI coding agents.
- **Terminal:** Execute commands and interact with the development environment.

Orin aims to unify agents such as Claude Code, Codex, and Gemini CLI within one lightweight, efficient workspace.

## Philosophy

Traditional IDEs are built for humans writing code. **Orin is built for humans directing intelligence that writes code.**

Its priorities are speed, simplicity, minimal resource consumption, and efficient agent supervision.

**The developer directs. Orin executes.**

## Approved Technical Direction

- **Language and UI:** Rust + GPUI.
- **Application type:** Desktop only.
- **Target operating systems:** Windows, Linux, and macOS.

See [Design decisions](design-decisions.md) for the approved technical boundaries and [User guide](user-guide.md) for current availability.
