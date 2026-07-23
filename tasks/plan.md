# Task breakdown — see docs/SPEC.md for full acceptance criteria

Status legend: [ ] pending, [x] complete, [~] in progress, [!] blocked/flagged

## Wave 0 — quick wins, zero dependencies

- [x] T1. R5: CI push trigger includes `claude/**`
- [x] T2. R6: gitignore gaps (`llm/spend.json`/`health.json`, familiar-main Xcode cruft)
- [x] T3. R7: fix stale iOS-path doc references

## Wave 1 — kernel foundation (blocks R2, R9, R10)

- [x] T4. R1: `ActionKind::Microphone/::Location/::Motion/::NetworkDiscovery` + matching `Boundary`/`CapabilityScope` fields, fail-closed, tested
- [x] T5. R1: extend with a distinct biometric-recognition gate (`allow_face_recognition` or similar) for R10's "new consent toggle distinct from plain camera" requirement — not in original R1 list, added here since R10 depends on it
- [x] T6. R2: `GateStates` + merge.rs gain the new gate names, tested (depends on T4, T5)

## Wave 2 — daemon simplification & hardening (independent of Wave 1)

- [x] T7. R3: remove `watch_camera`/`camera_allowed` driver from `crates/cycle`'s daemon tick loop; confirm `discover()` unaffected
- [ ] T8. R4: triage `crates/mesh` unwrap/expect on network-input paths (brief ingestion, worldview parsing, observation batch parsing, TLS/handshake)

## Wave 3 — Swift: small, independent

- [ ] T9. R8: watch consent defaults → false, add first-pair consent prompt

## Wave 4 — Swift: macOS sensing build-out (depends on T4)

- [ ] T10. R9: macOS mic (Speech, push-to-talk) in FamiliarMac.app
- [ ] T11. R9: macOS location consent gate wired to `allow_location`
- [ ] T12. R9: macOS Bonjour network discovery
- [ ] T13. R9: macOS consent-gate UI surface

## Wave 5 — identity & recognition (depends on T5)

- [ ] T14. R10: `Identity` gains biometric-link field, kernel-side tests, hard-excluded from mesh federation payloads
- [ ] T15. R10: iOS face recognition — verify current Vision API against Apple docs, then implement embedding/match
- [ ] T16. R10: confirm-before-keep UI + interactive identification fallback
- [ ] T17. R10: macOS equivalent in FamiliarMac.app

## Wave 6 — full-peer embedding & background sync

- [ ] T18. R11: wire core-ffi into iOS app (found/join/mesh_start), verify GossipPeer promotion
- [ ] T19. R12: BGTaskScheduler background sync (verify current API against Apple docs first)

## Deployment (stop-and-confirm before each — real devices/production)

- [ ] T20. Rust workspace: build, test, deploy updated daemon to this Mac
- [ ] T21. iOS: build, install to Aphelion (iPhone) + Codex (iPad) — confirm before device install
- [ ] T22. watchOS: build, install to paired watch — confirm before device install
- [ ] T23. Lighthouse VPS: redeploy with updated mesh/kernel code — confirm before remote deploy
- [ ] T24. TestFlight/App Store submission, if desired — confirm explicitly, separate from device sideload
