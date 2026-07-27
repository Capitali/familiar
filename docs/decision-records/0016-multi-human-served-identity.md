# ADR-0016 — Multi-human served identity: the node serves many, tagged and scoped

- **Status:** accepted (implemented — first pass)
- **Date:** 2026-07-27
- **Relates to:** [ADR-0009](0009-sovereign-mesh-transport.md) (identity federation +
  `IdentityShare`), `crates/kernel/src/identity.rs` (the registry this builds on),
  [ADR-0015](0015-automated-covenant-admission.md) (whose admin-auth follow-up rides on the
  facial-recognition effort that will auto-set the served human), `docs/design-orientation-and-mesh.md`

## Context

A familiar node does not belong to one creator. It serves whoever is present, and devices
are shared and change hands — Betty's iPad (`clover`), iPhone (`willow`), and watch sit on
the same mesh as Ian's. The identity model already supported this in principle: an
append-only registry of everyone the familiar has come to know (`identities.jsonl` —
handle/name/relation/interactions/face_signature) plus an `observer.txt` pointer to who is
present now, with face signatures that never federate.

But the served human was **hard-coded to `ian`** wherever a device attributed activity, so
every device's observations claimed to be Ian's — mislabelling Betty's own devices — and
there was no per-human attribution or scoping in the worldview.

## Decision

**A node serves many humans, and activity is attributed per human.** Two rules:

1. **One shared worldview, per-human attribution.** The mesh is a shared worldview by
   design; activity stays visible, tagged with *which* human did or said it (the actor's
   human suffix — `phone:betty`, `watch:ian`). We do not split into private per-human
   consoles.

2. **Sensitive-personal signals are scoped private.** A shared worldview is not a shared
   body: a person's **health, precise position, and biometrics** (`heart_rate:`,
   `location:`, `gyro:`, `face:`) are attributed to their human locally but **never
   federated to peers and never shown as another human's data**. (Face already never left
   via `IdentityShare`; this extends the same rule to what a personal device reports.)

The served human is established, in order: a facial-recognition confirmation (future — the
facial-recognition effort, also ADR-0015's admin authenticator); a manual "who's using
this" pick in the Device menu; else a neutral default (`observer`) — **never a baked
creator**.

## Implementation (first pass — this change)

- **Dynamic served-human handle.** iOS `DeviceActor` (`ios/App/Sources/NetworkDiscovery.swift`)
  composes `phone|ipad:<human>` from `AppModel.servedHuman` (`@AppStorage`, default
  `observer`), set via a **SERVING** field on the console Device screen
  (`setHuman` bridge → `AppModel.setServedHuman`, slugged to match
  `identity::slug`). The watch takes the paired phone's handle over the existing
  `PhoneWatchLink` hand-off and tags `watch:<human>`. Console answers and the CLI goal
  seed (`crates/cli/src/main.rs`) attribute to the current human, not `ian`.
- **Sensitive scoping.** `familiar_kernel::service::{SENSITIVE_PERSONAL_PREFIXES,
  is_sensitive_personal}` classify the private set; `crates/mesh/src/merge.rs` drops them
  from the outbound brief so they never reach the wire. Tested: health/position/biometric
  are absent from a peer brief while ordinary activity still federates.
- **Attribution** rides the actor suffix, which `members::derive_presence` and the roster
  already surface — correct now that the suffix stops lying.

## Consequences

- Betty's devices tag `…:betty`, not `…:ian`; the roster attributes correctly; her
  heart-rate/location never appear in another node's federated worldview.
- The `observer.txt` single-current pointer stays (whoever spoke last, for greeting); the
  multi-human model is carried by attribution, not by a per-viewer split.
- A device left at its `observer` default attributes to "observer" until told who is using
  it — deliberately honest (no false creator) rather than convenient.

## Out of scope (follow-ups)

- **Face-driven auto-switching** of the served human — lands with the facial-recognition
  effort (which also becomes ADR-0015's admin authenticator).
- **Per-viewer worldview facets** (each human sees only their own sensitive data, rather
  than it simply being unbroadcast) — the current pass keeps sensitive data off the wire
  and off other humans' rosters, which is the guarantee that matters first.
- **Enrollment-time name capture** so a fresh device starts with its human already set
  rather than `observer`.
