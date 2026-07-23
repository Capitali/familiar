# SPEC: Symmetric Peer Capability, Identity Recognition & Structural Hardening

Source of truth for intent: `/Users/ian/.claude/plans/stateless-puzzling-kettle.md` (approved). This SPEC restates it as testable requirements for the build loop; the plan file has the full rationale/evidence and should be read alongside this for context.

## Scope

Two repos: `~/Development/familiar` (Rust workspace, branch `claude/session-8nlpbv`) and `~/Development/familiar-main` (iOS/macOS/watchOS Swift app, branch `main`, a worktree of the same repo).

## R1 — New kernel capability gates (mic/location/motion/network-discovery)

Only camera has a constitutional gate today (`ActionKind::Camera`, `Boundary.allow_camera`, fail-closed). Add symmetric gates for the other sensor types, following that exact pattern.

**Acceptance criteria:**
- `crates/kernel/src/guard.rs`: `ActionKind` gains `Microphone`, `Location`, `Motion`, `NetworkDiscovery` variants, each evaluated against a matching `Boundary` field exactly like `Camera`/`allow_camera` today.
- `crates/kernel/src/boundary.rs`: `Boundary` gains `allow_microphone`, `allow_location`, `allow_motion`, `allow_network_discovery`, all `bool`, all fail-closed (`false`) in `Boundary::closed()` and in `#[serde(default)]` deserialization, all folded into `is_closed()`.
- `CapabilityScope` gains matching fields so agent-delegated tasks can be scoped per-sensor like camera already is.
- Unit tests exist for each new gate mirroring the existing camera-gate tests: fail-closed default, scope intersection with the human boundary, discovery-vs-action distinction (perception is free, the gated act is weighed).
- `cargo test -p familiar-kernel` passes.

## R2 — Gate visibility on the worldview

Peers should be able to see (read-only) whether a node's new sensor gates are open, mirroring how the camera gate is already visible.

**Acceptance criteria:**
- `crates/mesh/src/worldview.rs` (`GateStates`) gains fields for the four new gates.
- `crates/mesh/src/merge.rs`'s gate-name list is updated to include them.
- Existing worldview/merge tests still pass; a new test confirms a new gate's state round-trips through worldview assembly.
- `cargo test -p familiar-mesh` passes.

## R3 — Retire camera capture from the headless daemon

Decision (already made, not open for debate): headless peers (the background daemon, FamTalker01, the lighthouse VPS) never capture camera frames. Camera *discovery* (enumeration, no capture) is harmless and stays. Only the *gated capture act* moves out of the daemon.

**Acceptance criteria:**
- `crates/cycle/src/lib.rs`'s `watch_camera` driver (and the `camera_allowed` check that gates it) is removed from the daemon's tick loop — the daemon no longer calls `familiar_vision::capture_frame` under any boundary state.
- `familiar_vision::discover()` (enumeration only) remains callable from the daemon — this is unaffected, it was never the gated act.
- A test (or a clear code-level assertion) confirms the daemon's cycle loop has no reachable path to `capture_frame`.
- Existing `crates/cycle`/`crates/vision` tests still pass after the removal.

## R4 — Mesh crate `unwrap()`/`expect()` triage (attacker-reachable paths)

`crates/mesh` parses network-attacker-reachable bytes and has 246 non-test `unwrap()`/`expect()` calls. Triage, don't blanket-rewrite.

**Acceptance criteria:**
- Every `unwrap()`/`expect()` in `crates/mesh/src/*.rs` (excluding tests) reachable from a network-input parsing path (brief ingestion, worldview request parsing, observation batch parsing, TLS/handshake code) is either: (a) confirmed genuinely infallible post-validation with a `// SAFETY:`-style comment explaining why, or (b) converted to a graceful `Result`-returning error path.
- `cargo test -p familiar-mesh` and `cargo clippy -p familiar-mesh --all-targets -- -D warnings` pass after changes.
- No behavior change for valid input — only malformed/adversarial input paths change from panic to graceful rejection.

## R5 — CI runs on the active branch

**Acceptance criteria:**
- `.github/workflows/ci.yml`'s `push` trigger includes the active development branch (`claude/session-8nlpbv`) in addition to `main`, e.g. `branches: [main, 'claude/**']`.
- Verified by pushing and confirming a run appears in GitHub Actions for the branch.

## R6 — Gitignore gaps closed

**Acceptance criteria:**
- `~/Development/familiar/.gitignore` covers `llm/spend.json` and `llm/health.json` (or a general `llm/*.json` runtime-state pattern that doesn't also hide anything that should stay tracked — check `llm/` for tracked `.json` files first).
- `~/Development/familiar-main/.gitignore` covers standard Xcode cruft: `xcuserdata/`, `*.xcuserstate`, `DerivedData/`.
- `git status` stays clean after a scenario-lab run that produces `spend.json`/`health.json` inside the repo path.

## R7 — Stale docs fixed

**Acceptance criteria:**
- `docs/mesh.md` and `docs/DEVELOPMENT_LOG.md`'s references to `~/Development/familiar-ios` are corrected to `~/Development/familiar-main/ios/` (the actual current worktree location).
- A one-line pointer is added near the top of `docs/mesh.md`'s device-seam section clarifying the current app location, to prevent landing on `archive/ios/` (retired) by mistake.

## R8 — Watch consent defaults aligned

**Acceptance criteria:**
- `ios/Watch/Sources/WatchSensing.swift`'s `watch.consent.motion`/`watch.consent.heart` `@AppStorage` defaults change from `true` to `false`, matching phone/iPad's default-off posture.
- A first-pair consent prompt is added (on-watch or relayed from the paired phone) so a newly-paired watch isn't silently mute forever — user must explicitly opt in once.
- Existing watch tests (if any) still pass; manual on-device verification needed (flag as such, can't be tested in CI).

## R9 — macOS sensing build-out (GUI-app-hosted)

New Swift modules in `FamiliarMac.app`, each gated by its R1 kernel gate, each with a real consent toggle (none exist on macOS today):

- **Microphone**: on-device speech, push-to-talk, mirroring iOS `VoiceSensing.swift`.
- **Location consent**: the existing unconditional `CLLocationManager` listener in `SphereBridge` gets a consent gate wired to `allow_location`.
- **Network discovery**: Bonjour survey via `Network.framework`, mirroring iOS `NetworkDiscovery.swift`'s ~25 service types.
- **Consent surface**: a real gate UI in `FamiliarMac.app` (none exists today), following the existing read-only `GateStates` mirror pattern.

**Acceptance criteria:** each module compiles, requests the correct macOS permission, and its consent toggle correctly gates the sensing action (verifiable by code review of the gate-check call site; live permission-prompt behavior needs a manual on-device pass).

## R10 — Facial recognition tied to identity

- `crates/kernel/src/identity.rs`: `Identity` gains an optional biometric-link field (embedding/signature) per `docs/design-orientation-and-mesh.md`'s existing design language — "a link to an existing identity, not a new record."
- iOS `FaceSensing.swift`: extend beyond `VNDetectFaceRectanglesRequest` (presence-only) to a real embedding/matching step. **Verify the current correct Vision API against Apple's official documentation before writing code** — do not assume a specific API name from training data.
- Confirm-before-keep UI: a recognized face proposes a match; a human confirms or corrects; a correction is never sticky. Reference `archive/glass/src/main.rs`'s retired identity-strip interaction pattern (not its code — it's Rust/egui — its *flow*) for the SwiftUI rebuild.
- Interactive fallback (hard requirement): below a confidence threshold, the shell prompts for positive identification (typed name or push-to-talk) before treating the interaction as attributed to anyone. Never silently interact with an unidentified person.
- New consent toggle distinct from plain camera/presence consent (biometric recognition is "strongly sensitive" per the design doc).
- Mesh: `crates/mesh/src/brief.rs`'s `IdentityShare`/`ConsentedIdentityPayload` hard-excludes the biometric field regardless of `share_identities` opt-in state — no scoping option for it in v1.
- macOS: same feature, lives in `FamiliarMac.app` only (R3's daemon-exclusion applies).

**Acceptance criteria:** `crates/kernel` unit tests cover the new identity field (add/round-trip/never-serialized-to-mesh-payload); Swift-side changes need manual on-device verification (real face, real camera) — can't be simulated in CI.

## R11 — ADR-0009 Phase 0: iOS/iPad true full-peer embedding

- Wire `crates/core-ffi`'s existing `ios/FamiliarCore/` xcframework into the `FamiliarAgent` iOS target: call `found()`/`join()`/`mesh_start()` so a capable phone runs the embedded core in-process and becomes a live `GossipPeer`, not just a `DevicePeer` console.
- Sensor data still flows in via the existing signed `POST /mesh/observe` (now pointed at the phone's own embedded `mesh_start()` loopback instance) — no new FFI surface for sensor ingestion.
- No new tier-promotion logic needed — `classify()` already promotes on real brief exchange.
- macOS stays as-is (daemon + loopback console) — it already satisfies "Full peer" natively, no FFI needed there.

**Acceptance criteria:** iOS app builds and links the xcframework; `mesh_start()` is invoked at the appropriate app-lifecycle point; a manual on-device test confirms the phone appears as a `GossipPeer` (not `DevicePeer`) in another peer's roster after brief exchange.

## R12 — Background execution (opportunistic sync, not persistent listening)

iOS never allows a backgrounded process to hold a listening socket — this is a hard platform constraint, not a missing setting. Target: `BGTaskScheduler`-driven periodic wake-and-sync.

**Acceptance criteria:**
- `Info.plist`'s `UIBackgroundModes` gains `processing` (or `fetch` — pick per current Apple guidance, verify against current docs).
- A `BGProcessingTask`/`BGAppRefreshTask` is registered and scheduled; its handler calls into the embedded core (R11) for a bounded sync pass (push queued observations, pull worldview, exchange a brief if reachable) within the OS-given time budget.
- The app appears in Settings → General → Background App Refresh after this change (manual verification).
- Applies identically to iPhone and iPad (same target, no fork).
- Silent-push wake (APNs) is explicitly **out of scope for this pass** — defer until the `BGTaskScheduler` cadence proves insufficient in practice.

## Out of scope for this SPEC

- Two-way `main`↔`claude/session-8nlpbv` branch reconciliation (flagged in the plan as worth doing, but a separate, deliberate piece of work — not bundled into this feature push).
- `rustfmt.toml`/`clippy.toml` (plan explicitly says not urgent).
- Scenario-lab D-vs-B/C follow-up investigation (tracked, not part of this implementation push).
- APNs silent-push infrastructure (R12 Phase 2).
