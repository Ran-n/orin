[//]: # ( ---------------------------------------------------------------------- )
[//]: # (+ Authors: 	Ran# <ran.hash@proton.me> )
[//]: # (+ Created: 	2026/11/11 12:19:34.514712 )
[//]: # (+ Revised: 	2026/11/12 09:16:15.002288 )
[//]: # ( ---------------------------------------------------------------------- )


# Orin — Vision

Orin is an **agent-first development environment** by **Breren**: a native desktop workspace for developers
who mostly direct AI coding agents rather than write code by hand.

**Tagline: From intention to creation.**

## The Problem

Traditional IDEs are built around the code editor. The AI agent is added as a side panel or a separate terminal.
Developers who work mainly through agents end up arranging their work around the editor instead:

- Each agent CLI — Claude Code, Codex, Gemini CLI — runs in its own terminal window, with no shared view of what each one is doing.
- Following several agents at once means switching between windows and scrolling through raw terminal output.
- Reviewing an agent's changes means switching again to a separate editor, diff tool, or Git client.
- The tools that do centre on agents are usually tied to a single vendor, or are heavyweight applications.

## What Orin Is

Orin puts the agents at the centre and arranges everything else around supervising them.
The developer directs the work, reviews the results, and manages the project.

| Workspace area | Purpose |
| --- | --- |
| **Agent Workspace** | The centre of the application: hold conversations, delegate tasks, and supervise several agents at once |
| **Project Explorer** | Navigate the project's folders and files |
| **Code Viewer** | Inspect files, changes, and diffs, and edit files directly whenever a manual change is needed |
| **Terminal** | Run commands and interact with the development environment |

## What Sets Orin Apart

- **Supervision first.** Orin is designed for watching, comparing, and reviewing several agents' work at once,
  not as an editor with a chat panel attached.
- **Vendor-neutral.** Claude Code, Codex, and Gemini CLI work side by side in one workspace, so the developer
  can choose the best agent for each task.
- **Uses the CLIs you already have.** Orin runs the agent CLIs the developer has already installed and signed in to.
  It adds no account, model, or subscription of its own.
- **Native and light.** Orin is written in Rust with GPUI rather than a browser runtime, prioritizing fast startup
  and low memory and CPU use while agents stream output.

## An Example Session

This illustrates the intended experience. It does not describe current behavior.

1. The developer opens a project and starts Codex on a bug fix and Claude Code on a new test suite.
2. Both agents appear in the Agent Workspace, each showing its status and recent activity.
3. One agent stops to ask a question. The developer answers it without losing sight of the other.
4. When an agent finishes, the developer opens its changes in the Code Viewer, reviews the diff, and fixes a line by hand.
5. The developer runs the test command in the Terminal, then sends the agent its next instruction.

## Scope

**In scope:** a desktop application for Windows, Linux, and macOS, offering the four workspace areas above.
That includes a working code editor for manual changes, which supports agent-driven work rather than replacing it.

**Out of scope:**

- Web and mobile applications
- Orin's own AI model, agent service, or user accounts. Orin coordinates existing agent tools.
- Replacing a full IDE's feature set. Language tooling beyond what reviewing and small edits require is not a goal.

## Philosophy

Traditional IDEs are built for humans writing code. **Orin is built for humans directing intelligence that writes code.**

Its priorities are speed, simplicity, minimal resource consumption, and efficient agent supervision.

**The developer directs. Orin executes.**

## Name & Etymology

**Orin** is an invented name built from two Galician words:

- **Or** — from *orixe* (origin): the beginning of an idea, creation, or project.
- **In** — from *intelixencia* (intelligence): reasoning, autonomy, and the ability to turn ideas into reality.

Together, **Orin means "Intelligence at the Origin of Creation"**: human intention starts the work,
and artificial intelligence carries out the implementation.

The [logo and mascot](../assets/branding/orin-mascot.png) is a germinating **landra** (acorn): a small beginning that holds the potential for growth.

## Status

Orin is in early development and runs from source. See the [user guide](user-guide.md) for what is currently available,
and [design decisions](design-decisions.md) for the approved technical direction and the decisions still open.
