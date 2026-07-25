# ADR-0007 — A wgpu/egui holographic engine as the primary UI, superseding the SwiftUI shells and the egui Glass

- **Status:** superseded by ADR-0008 (Metal Sphere) — the wgpu/egui engine
  (`crates/hologram`) was retired 2026-07-22; nothing shipped on it and the sphere became
  the interface on every platform it can run. This record is kept for history.
- **Was:** accepted
- **Date:** 2026-07-21
- **Supersedes:** ADR-0006 (the egui Glass), and the SwiftUI-shell portion of
  `one-core-many-shells.md`

## Context

The familiar's human interface has gone through two prior directions:

1. ADR-0006 built **the Glass**, a read-only egui/eframe window (`crates/glass`), later
   archived in favor of native platform apps.
2. `one-core-many-shells.md` proposed keeping the Rust workspace as the single source of
   truth and making Apple platforms (`ios/`: `FamiliarMesh`, `MacApp`, `Shared`, `App`,
   `Watch`) thin SwiftUI shells over it via UniFFI bindings.

Both directions optimized for a **calm, dignified, low-visual-noise** companion surface —
warm-dark, restrained, anti-engagement (see the original `docs/UI-DESIGN-BRIEF.md`). That
choice is being deliberately reversed: the familiar's presence should read as a visibly
**alive, animated, holographic** projection — closer to a sci-fi companion display (Iron
Man / Westworld register) than a settings-app-style dashboard — across every platform it
runs on, Apple included.

## Decision

Replace the SwiftUI shells and the archived egui Glass with a single **wgpu/egui
holographic rendering engine**, written in pure Rust, as the one primary UI for every
platform:

- **Graphics abstraction:** `wgpu` only — no raw Metal/OpenGL/Vulkan/DX12 written by hand.
  `wgpu::Backends::METAL` is selected explicitly on macOS/iOS (Apple Silicon unified
  memory, low-overhead command queues); Windows/Linux fall back through wgpu's
  Vulkan/DX12/GL backends.
- **Windowing:** `winit`, which carries the platform surface (including `CAMetalLayer` on
  Apple) without a separate native shell per OS.
- **UI content:** `egui` + `egui-wgpu` render the familiar's actual interface (the
  conversation, the law-signals, the boundary controls — the data inventory in
  `docs/UI-DESIGN-BRIEF.md` is unchanged) to a texture; a custom WGSL fragment pass then
  composites that texture with the holographic effect (chromatic aberration, procedural
  glitch, moving scanlines, Fresnel rim glow, ambient flicker) before presenting.
- **New crate:** `crates/hologram`, workspace member, depending on `familiar-kernel` and
  the other core crates exactly as the archived Glass and the SwiftUI shells did — the
  Rust core (`kernel`, `cycle`, `mesh`, `sense`, …) is unchanged; only the presentation
  layer moves.
- **Apple apps:** `ios/` (`FamiliarMesh`, `MacApp`, `Shared`, `App`, `Watch`) move to
  `archive/ios`, following the same archive-not-delete precedent as `archive/glass` and
  `archive/marble` — the code is preserved and buildable from history, it is simply no
  longer the maintained path forward.

## Consequences

- **Gained:** one Rust codebase and one rendering path for every platform — no more
  hand-synced Swift reimplementation of wire/crypto logic (the exact drift
  `one-core-many-shells.md` was trying to end, now ended by removing the second language
  entirely rather than bridging to it); a visually distinctive, animated presence that
  reads as more alive.
- **Cost / given up:** the calm/anti-engagement design brief no longer describes the
  actual UI (see the accompanying rewrite of `docs/UI-DESIGN-BRIEF.md`) — the Three Laws
  still govern *content and behavior* (no dark patterns, no manipulation, honest
  known/probable/unknown confidence), but the *visual register* is now deliberately
  animated and ornamented rather than restrained. Native platform affordances (Apple
  notifications, watchOS complications, TestFlight distribution, the notarized `.pkg`
  installer) are given up until/unless rebuilt on top of a wgpu/winit shell — winit's
  platform integration is thinner than a native SwiftUI app's. `#![forbid(unsafe_code)]`
  in the kernel is unaffected (the new dependency weight lands in `crates/hologram`, same
  isolation principle as ADR-0006's).
- **Verification limit:** as with the archived Glass, a rendering pipeline can't be
  meaningfully unit-tested; correctness rests on the kernel's tested signals (which the
  UI only displays) plus compile + manual/visual inspection of the WGSL passes.

## Alternatives considered

- **Keep SwiftUI + UniFFI (`one-core-many-shells.md` as written)** — rejected: it keeps
  Apple on a separate, calmer visual register than the rest of the fleet, and the target
  aesthetic (holographic, animated) doesn't fit SwiftUI's native chrome without fighting
  the platform's own design language.
- **Add wgpu/egui as an additional shell, alongside SwiftUI, for non-Apple peers only** —
  rejected for this decision: the goal is one consistent presence across every peer, not
  a split aesthetic by platform.
- **Web dashboard (Tauri or a Rust HTTP server + browser)** — rejected for the same
  reason ADR-0006 rejected it: an outward network surface at odds with Law III restraint,
  and a heavier toolchain than wgpu/winit/egui.

## Status history

- 2026-07-21 — accepted. `crates/hologram` created; `ios/` archived to `archive/ios`.
