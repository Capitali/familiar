# ADR-0046 — The floors rise to 26: Apple Intelligence is a premise

- **Status:** **accepted** — Ian, 2026-08-24, deciding T-227's fork verbatim option (a):
  "raise ALL deployment floors to 26," made with the cost stated ("every pre-26 device
  loses the app"). Grounded in his 2026-08-24 target-audience ruling (verbatim: "lets
  just keep enabling apple intelligence on Apple Silicone Mac software — we will just
  assume the need to have all those devices boot locally and that apple inetelligence
  can be enabled — even if we cant easily test today, that's the target audience.
  Ios/macos/ipados/watchos with as much apple intelligence enabled as possible").
- **Date:** 2026-08-24 (decided) · 2026-08-25 (recorded, built)
- **Relates to:** T-227 (the adoption sweep this floor unblocks), T-228 (depends on this
  landing first "or this is written twice"), T-224 (the Envoy — the other FoundationModels
  consumer), [ADR-0038](0038-thoughts-travel-by-leave.md) (`allow_llm`/`allow_llm_cloud`
  still bind every thought-path), T-226 (MacOnStick's external-boot ineligibility —
  a bench limitation, explicitly NOT a product constraint and not a reason to hedge)

## Context

`ios/project.yml` set deployment floors of iOS 17.0 / macOS 14.0 / watchOS 10.0 (the
FamiliarWatch target overriding to 9.0), while every Apple Intelligence path in the tree
(`ConsultRunner`, `LocalReasoner`) requires 26 — so the declared audience and the build
floor disagreed. That was a real fork: **(a)** raise the floors to 26, the availability
guards mostly dissolve, and every pre-26 device loses the app; or **(b)** keep the floors
and adopt behind guards forever, dropping nobody but carrying two worlds. Ian chose (a).

Two facts sharpen it beyond preference:

1. **The tree already requires the 26/27 SDK to BUILD.** CI's first Swift run proved
   Xcode 26.6 cannot compile `ConsultRunner`'s PCC lane (`PrivateCloudComputeLanguageModel`
   is a 27-SDK API). The "support old" posture was already fiction at compile time.
2. **The floor raise is not only `project.yml`.** `FamiliarMesh/Package.swift` declares its
   own platforms; left behind, the package compiles against the old floors while the app
   targets claim 26 (found 2026-08-25, recorded on the board).

## Decision

Deployment floors are **26.0 on all four platforms** — iOS, iPadOS (same target), macOS,
watchOS — in `ios/project.yml` (including the FamiliarWatch override) and in
`FamiliarMesh/Package.swift`. The shells are built for Apple Silicon devices booting from
their internal disk with Apple Intelligence enabled — that is the target audience.

**Stated narrowly (codex round 2's correction, absorbed): what the floor guarantees is
the OS version, never an available model.** 26-capable hardware can still be ineligible,
Apple Intelligence can be off, the model can be loading, and several Foundation Models
surfaces (`PrivateCloudComputeLanguageModel`, the `LanguageModel` protocol, any watch
session) first exist at OS 27. Honest runtime unavailable-states therefore survive the
floor raise; only per-target `#available(...26...)` branches proved redundant may go.

**The embedded core moves with the floor.** `FamiliarCore.xcframework` is rebuilt with
`IPHONEOS_DEPLOYMENT_TARGET=26.0` pinned in `tools/build-core.sh`, which now also
verifies no object in either slice *requires* newer than 26.0 and fails the build
otherwise (the 2026-08-25 review found 26.5-min objects inside a 26.0-linking app —
a warning, not an error, and therefore easy to ship by accident). Objects *older* than
the floor remain: Rust ships its precompiled standard library at the toolchain's own
minimum, and an older-min object links safely into a newer-min binary — the invariant
is one-directional by design. Verified 2026-08-25: the simulator app build emits zero
"was built for newer" warnings.

What this deliberately drops (Apple's published compatibility boundary for the 26 cycle;
stated so the loss is chosen, not discovered):

- **iOS/iPadOS:** every device that cannot run iOS 26 — the iPhone XS/XR generation and
  older, and pre-A12 iPads. On-device Apple Intelligence further narrows useful hardware
  to iPhone 15 Pro and later; older 26-capable phones keep the app but without the
  on-device model.
- **watchOS:** watches that cannot run watchOS 26 (Series 5 and earlier, first-gen SE) —
  the old 9.0/10.0 floors' devices.
- **macOS:** Macs that cannot run macOS 26. The product audience is Apple Silicon
  (2026-08-24 ruling); remaining Intel Macs that technically run 26 are out of audience,
  and Apple Intelligence never reaches them anyway.

Costs accepted with eyes open: TestFlight installs on any pre-26 device stop updating,
and the fleet's consoles must be on 26 to take new builds.

What this does NOT decide:

- **tvOS** — `Package.swift` lists `.tvOS(.v17)` and nobody has decided anything about
  tvOS; it stays exactly as it was until someone does.
- **Guard dissolution** — the `#if canImport(FoundationModels)` + `@available(26)` guards
  are now mostly redundant but harmless; removing them is the adoption sweep's work
  (T-227 proper, through the codex dialogue), not this record's.
- **PCC posture** — unchanged by this ADR: reopened everywhere eligible (Ian 2026-08-24),
  behind the unchanged consent stack (boundary `cloud_ok` ∧ per-device `consent.pcc` ∧
  OS 27 ∧ Apple reporting available).
- **The boundary still governs where a thought travels** — `allow_llm`/`allow_llm_cloud`
  (ADR-0038) bind every new Apple Intelligence path exactly as before.

## Mechanics worth recording

`Package.swift` uses string platform versions (`.macOS("26.0")`) rather than `.v26`
constants: the package's `swift-tools-version:5.9` predates those constants, and bumping
the tools version would silently flip the package's default language mode to Swift 6 —
a migration that deserves its own decision, not a floor raise's side effect.

## Consequences

- T-228's shells build against one honest floor; the survey/BLE work is written once.
- The watch can gain `Shared/Sources` parity (T-227 candidate ①) without carrying a
  pre-26 code path.
- "We can't easily test today" (no 26-capable bench until MacOnStick returns or hardware
  appears) is accepted by Ian as a known cost; the discipline stands — availability
  guards where the SDK still wants them, honest unavailable-states, and no claim that a
  surface works until a Mac has built and a device has run it.
