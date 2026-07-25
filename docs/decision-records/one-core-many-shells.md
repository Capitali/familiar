# DR: One Rust core, many thin shells — unifying peers on a single base code

Status: **superseded by ADR-0007** (2026-07-21) — the SwiftUI-shell-via-UniFFI plan
described below was not carried out; Apple platforms now run the same wgpu/egui
`crates/hologram` engine as every other peer instead of a native shell. The "one Rust
core" principle survives; the "many shells" half does not. Original proposal (2026-07-15)
kept below for history.

## Context

"The Familiar" is the **collective** of every peer, agent, system, and AI that has joined the mesh
under the Three Laws — not any single node. The goal is that **every peer runs the same base code,
regardless of the platform it runs on.** Some peers are headless (Linux/RV/VM/Cerbo); some have a GUI
(iOS/iPad/watch/Mac). The end state: enough observation + speech + vision that a UI is *optional* —
the peer talks and listens (voice/vision/audio/haptic), and shows data only when the device it's
presenting on is capable.

Today that isn't true. The Rust workspace (`kernel`, `cycle`, `mesh`, …) is the brain, but the Apple
apps carry a **parallel Swift reimplementation** of the wire protocol (`FamiliarMesh`: CryptoKit
ed25519, cert minting, the observe/worldview/enroll types). Every schema change is hand-synced across
two languages — the recurring `CertBody`/`observe`/`worldview` drift. That is the thing to end.

## Decision

**Keep one Rust core as the single source of truth; make every platform a thin shell over it.** Do
**not** move the Rust workspace "under Xcode" — Xcode builds Apple targets, cargo builds the core;
putting Rust under Xcode fights both toolchains.

- **`familiar-core`** — the Rust crates stay cargo-managed. Build them to a static library for Apple
  targets (`aarch64-apple-ios`, `aarch64-apple-ios-sim`, `aarch64-apple-darwin`) and generate Swift
  bindings with **UniFFI** (a `.udl`/proc-macro surface over mesh + sensing + cycle entry points).
- **Headless peers** run the Rust binary directly — same core, no UI.
- **GUI peers** consume `familiar-core` as a **binary Swift package** (`.xcframework`) and become a
  thin per-platform shell: enrollment, sensor capture, and I/O routing. All crypto/mesh/cycle logic
  lives in the core, called through the bindings.
- **I/O is a capability, not a requirement.** The shell exposes whatever the device has — screen if
  capable, else mic/camera/speaker/haptics. The core is identical everywhere; only the shell differs.

Peers are **equals** with equal rights and capabilities where that helps them serve. No node is named
"familiar"; each is named by its host. `SelfNode` means only "you are here."

## Phasing

1. **Kill the drift first (highest value).** Replace hand-written `FamiliarMesh` (crypto + wire) with
   UniFFI bindings to the Rust `mesh` crate. Prove the `.xcframework` + bindings build for
   iOS/sim/macOS and pass the existing cross-language interop checks. The Swift app's call sites
   change from local structs to core calls; behavior identical.
2. **Sensing/observation into the core.** Move the derived-observation logic (debounce, envelope
   build, batching) behind the core; the shell only feeds raw platform sensor callbacks in.
3. **Cycle/interaction into the core** where it makes sense for a peer to reason locally, keeping the
   constitutional gates in Rust.
4. **UI-optional interaction.** Voice (STT/TTS), vision (presence/scene), audio, haptics as the
   primary interface; screen output only when present.

## Consequences

- One place for the constitution, crypto, and wire format. Schema changes can't drift.
- New platforms = a new thin shell, not a new protocol implementation.
- Cost: a cargo→xcframework build step (cargo can emit it; wire into `ios/tools/`), and UniFFI as a
  build dependency of the core. Worth it to end the two-implementations tax.
- Interop tests stay meaningful: the golden-vector cross-checks become core-vs-core, not
  Swift-reimpl-vs-Rust.

## Alternatives rejected

- **Move everything under Xcode.** Wrong tool for a Rust workspace; loses cargo, CI, and the headless
  path.
- **Keep two implementations, sync by discipline.** The status quo; the drift is a standing tax and a
  correctness risk on a constitutional system.
