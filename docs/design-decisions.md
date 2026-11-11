[//]: # ( ---------------------------------------------------------------------- )
[//]: # (+ Authors: 	Ran# <ran.hash@proton.me> )
[//]: # (+ Created: 	2026/11/11 12:19:34.382590 )
[//]: # (+ Revised: 	2026/11/11 12:44:16.173561 )
[//]: # ( ---------------------------------------------------------------------- )


# Design Decisions

## ADR-001: Rust + GPUI Desktop Application

**Status:** Accepted by the project owner

**Date:** 2026-11-11

### Context

Orin is an agent-first development environment by Breren.
Its [product vision](vision.md) prioritizes directing coding agents, reviewing their work, speed, simplicity, and low resource use.
The project owner requires a fully Rust desktop application targeting Windows, Linux, and macOS.

### Decision

Use **Rust + GPUI** for Orin's application and desktop interface.
Web and mobile applications are outside the approved scope.

The framework decision followed a comparison with Iced, egui/eframe, Floem, and Slint.
GPUI was selected for its fit with a custom development workspace and its use as the UI foundation of Zed.
Component libraries and implementation details will be selected with application work.

### Consequences

- Keep Orin's application implementation in Rust
- Validate Windows, Linux, and macOS behavior as application work proceeds
- Pin the GPUI revision and Rust toolchain when creating the application bootstrap
- Treat responsiveness and resource use as goals to measure, rather than performance already demonstrated by this repository
- Select agent adapters, code/diff presentation, and terminal components with their respective implementation work

### References

- [GPUI](https://gpui.rs/)
- [Upstream GPUI documentation](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md)

## ADR-002: Repository and Documentation First

**Status:** Accepted by the project owner

**Date:** 2026-11-11

### Decision

The first scaffold creates the required repository records and documentation only.
It records the product brief and approved stack, maintains the project-specific specification, and initializes local Git on `main`.

Application source, a Cargo manifest, a GPUI window, workspace layouts, agent integrations, and terminal execution are outside this step.
The repository must describe its pre-implementation state accurately.

### Consequences

There are no runnable-app commands, application builds, screenshots, or release downloads to publish yet.
The changelog starts with an empty Unreleased section; changes are recorded when preparing a requested commit.
Commits, remotes, and publication are separate requested workflows.
