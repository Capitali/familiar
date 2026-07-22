# ADR-0008 — The Metal Sphere web console on macOS (Claude Design import), beside the hologram engine

- **Status:** accepted
- **Date:** 2026-07-21
- **Relates to:** ADR-0007 (the wgpu/egui holographic engine remains the cross-platform
  direction); the design source is the Claude Design project file
  `Familiar Metal Sphere.dc.html`

## Context

ADR-0007 pivoted the primary UI to a pure-Rust wgpu/egui holographic engine
(`crates/hologram`). Its v1 shipped 2026-07-21: the T1 conversation channel, T2 trust
strip, full-metadata roster, dated roadmap/theories, and the attention-cue composite
pass — functional, but visually far from the sphere-and-hologram concept the design
work had converged on.

In parallel, a full visual design was authored in Claude Design ("Familiar Metal
Sphere"): a satellite Earth with the mesh's nodes pinned and calloutted, eight orbiting
animated glyph buttons, a near-fullscreen borderless hologram panel carrying the
conversation / roster / theories / work / signals / network / vision / control screens,
and a procedural street-dive. That design is web-native (Three.js + CSS), and reproducing
its fidelity in egui would cost weeks without getting closer to the product.

## Decision

Implement the design **as designed** — as a self-contained web bundle — hosted by the
existing macOS app, while ADR-0007's engine continues as the long-term cross-platform
path:

- `ios/MacApp/Resources/sphere/index.html` is a faithful vanilla-JS port of the design
  (its CSS, keyframes, globe, arcs, street surface). The `.dc` runtime templating was
  replaced with plain DOM rendering.
- **The web layer is presentation only.** The Swift host (`SphereWebView`) does all
  daemon I/O natively: it polls loopback `GET /local/worldview` and injects the JSON via
  `window.sphereUpdate(view)`; the page's human acts return over a
  `WKScriptMessageHandler` bridge and the host POSTs `/local/answer` / `/local/gate`.
  No CORS, no ATS exceptions beyond local networking, and the daemon stays the single
  writer of the data dir.
- **All content is real.** The mock's invented peers/latencies were replaced by the
  worldview: members with status/joined/session/total-online/OS/version/interactive/
  served-human, theories and goals with their lifecycle dates, the live question and
  console dialogue (with a keyboard input — the design had none), signals + gates,
  and the addresses the familiar answers at. Where the design showed a metric the mesh
  does not measure (latency, throughput, sync %), the slot was relabeled to something
  true (SEEN / SESSION / TOTAL ONLINE) rather than fabricated (Law III).
- **Geography is honest about its limits.** Mesh nodes carry no geolocation; every node
  is anchored at the boat (`HOME` constant, labeled "aboard GIIWEO") with a small
  deterministic per-node-id spread so callouts and the street dive stay distinguishable.
  If nodes ever report real positions, `HOME + hash offset` is the one seam to replace.
- Three.js, fonts, and the Blue Marble textures load from CDN — the sphere needs
  internet on first load (cached thereafter). Vendoring them into the bundle is the
  known follow-up for a fully offline console.

## Consequences

- macOS gets the designed experience now; the SwiftUI `FamiliarConsole` (Metal orb) code
  remains in-tree as reference/fallback but is no longer the root view.
- `crates/hologram` continues as the platform-independent engine; the sphere is the
  design target it should converge on. When the engine can carry this fidelity, the web
  console retires.
- The iPad design variant (`Familiar for iPad.dc.html` in the same Claude Design
  project) can follow the same pattern in the iOS app if wanted before the engine
  matures.
