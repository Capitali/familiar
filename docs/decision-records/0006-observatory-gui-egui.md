# ADR-0006 — A native GUI (the Glass) via egui/eframe

- **Status:** superseded by [ADR-0007 — one core, many shells](0007-one-core-many-shells.md)
- **Date:** 2026-06-24

## Context

The familiar needs a human interface that is **visual**, not a command line — a
window onto its truth (observations) and the law-signals (service, presence,
boundary). It must stay **local-first** (no telemetry, no listening sockets — Law
III restraint), run on the operator's machine, and not compromise the kernel's
minimal-dependency, `#![forbid(unsafe_code)]` discipline.

## Decision

Build **the Glass**, a native GUI, with **egui/eframe** (immediate-mode, pure
Rust), in its own crate `crates/observatory` (binary `observatory`).

- The GUI is **read-only**: it watches state and never mutates the observation log,
  so the model can't drift from what was observed.
- GUI dependencies (eframe/egui and their tree) are **isolated in the observatory
  crate**; the kernel stays `serde`-only and unsafe-free.
- The **CLI is retained** for scripting, automation, and headless/CI use; the GUI
  becomes the primary *human* interface.

## Consequences

- **Gained:** a local, single-binary visual interface with no network surface (a
  browser/web-server dashboard would open a socket — avoided); idiomatic Rust;
  cross-platform.
- **Cost / given up:** eframe pulls a large dependency tree (winit, glow, image,
  arboard, …) — a real trust-surface increase, accepted because it is *isolated to
  the GUI crate* and the kernel (the safety-critical part) stays minimal. Workspace
  build time grows.
- **Verification limit:** a GUI can't be unit-tested by rendering; correctness rests
  on (a) the kernel's tested signals, which the GUI only displays, and (b) compile +
  manual inspection. The view logic is kept thin for this reason.

## Alternatives considered

- **Web dashboard (Rust HTTP server + browser)** — rejected: opens a listening
  socket (an outward surface at odds with Law III restraint) and adds a web stack;
  egui native keeps everything in one local window with no socket.
- **TUI (ratatui)** — rejected for the primary interface: still terminal-bound, and
  the directive was to move *to a GUI*. (A TUI remains possible later for headless
  hosts.)
- **Tauri (web frontend + Rust backend)** — rejected: heavier (system webview + JS
  toolchain) than a pure-Rust immediate-mode GUI for this need.

## Status history

- 2026-06-24 — accepted. First Glass shows the Three Laws as live meters plus
  the observation log; it will grow as the kernel does.
- 2026-07-17 — **superseded.** The SwiftUI consoles (the native macOS console + the
  iPad/iPhone apps + the watch companion, [`../../ios/`](../../ios/)) became the human
  interface; the egui Glass and the menu-bar marble were archived under `archive/` and
  removed from the workspace. The direction is
  [ADR-0007 — one Rust core, many thin shells](0007-one-core-many-shells.md).
