# T-237 B4 one-doctrine/two-runtimes codex re-verification, round 2

**Verdict: REJECT.** Four of the five round-1 findings are repaired, and the
checked-in xcframework executes the repaired seam: the boundary probe now returns
`call-paws` with seam 2 and reasons. The active-contract repair is correct on its
fixture path, but its source read still fails open. Any error reading
`/v1/loadboard?mine=true` is converted to an empty board; direct mode can consequently
show and, after the same error on its fresh re-read, file a freight-idle act while
`/v1/me.freight` still says the hull is under contract. That recreates round 1's safety
failure whenever one critical read is unavailable.

Reviewer: companion:codex
Reviewed: round-1 report
`docs/reviews/2026-09-08-t237-b4-codex-reverification.md`; Rust repairs
`c63a61c` and `c9419b9`; Swift/artifact repair `749c4d9`; current main
`b6e27a6`

## Findings

### 1. Blocker — an unavailable mine board is still treated as “no active contract”

**File:** `ios/FamiliarSC/Sources/FamiliarSCUI/DirectFeed.swift:235-242`,
`:288-301`, `:375-392`;
`ios/FamiliarSC/Sources/FamiliarSC/Exchange.swift:398-418`;
`ios/FamiliarSC/Tests/FamiliarSCTests/DirectPilotTests.swift:127-183`

**Failure scenario.** The repair does carry a successful mine-board row as
`active: {row}`, and the seam derives its word from `/v1/me.freight`. But line 241
wraps the entire authenticated fetch, HTTP status check and JSON decode in `try?` and
substitutes `[]` on every failure. If `/v1/me` says load L3249 was booked/picked up
while `/v1/loadboard?mine=true` transiently returns 500, times out, or changes to an
unreadable shape, `active` is omitted. The seam is then told that the hull is
freight-idle and may choose an open load. `confirm` repeats this gather fresh, so a
persistent or repeated mine-board failure can produce the same wrong `book` act and
reach `client.file` after the captain taps. The host's live `Active` state and the
ledger have not disappeared merely because this one read failed.

This is also silent. The pilot document says the resulting decision rather than the
failed endpoint, despite the direct-mode rule that a broken gather be named. The mock
suite always serves the mine fixture successfully in `setUp`; it tests an unpriced
route/rung, but not a failed or inconsistent mine board.

**Repair accepted.** Make the mine-board fetch a required, throwing part of the
decision gather. Also fail closed when `/v1/me.freight` has an unresolved booking but
the successful mine response cannot supply its row: show the missing/inconsistent
contract fact and offer no `PilotProposal`. Pin a 500/transport/decode case and a
ledger-open/mine-empty case through render and confirm, proving zero `/v1/actions`
POSTs.

## Round-1 findings re-verified

### 1. Hull-specific route rungs and seam skew — repaired

**File:** `ios/FamiliarSC/Sources/FamiliarSC/Exchange.swift:426-434`;
`ios/FamiliarSC/Sources/FamiliarSCUI/DirectFeed.swift:235-287`, `:303-323`;
`crates/whisker/src/wire.rs:176-268`, `:370-438`

Every priced leg to a pump asks for standard and economy `forHull` quotes and emits
`rungs.<class>.{fuel,ticks}`, the shape `TableRouter::from_json` consumes. Missing
rungs are counted and narrated. `DirectFeed.seamVersion == 2` gates both rendering and
proposal creation. The exact round-1 188 mG / 123 fuel boundary input now returns
`call-paws` from the checked-in simulator archive with
`rescue.no-pump-in-reach` and `seam_version: 2`.

### 2. Active contract as its own object — repaired on success, incomplete because of finding 1

**File:** `ios/FamiliarSC/Sources/FamiliarSCUI/DirectFeed.swift:288-301`;
`crates/whisker/src/wire.rs:130-145`, `:384-412`

The successful path ranks in-transit, then booked, then delivered-uncollected rows,
sends `active: {row}`, sends no `active_load_id`, and lets `active_word` read the
ledger. The open-board fixture does not contain L3249 while the mine fixture's row
survives into the input. Finding 1 is the remaining failure mode: an error obtaining
that row is represented as absence rather than an unavailable fact.

### 3. Captain-confirm act path — repaired

**File:** `ios/FamiliarSC/Sources/FamiliarSCUI/Feed.swift:228-302`;
`crates/whisker/src/main.rs:1752-1774`;
`ios/FamiliarSC/Sources/FamiliarSCUI/DirectFeed.swift:319-323`, `:375-395`;
`ios/FamiliarSC/Sources/FamiliarSC/Exchange.swift:437-461`;
`ios/FamiliarSC/Sources/FamiliarSCUI/BridgeModel.swift:132-136`, `:295-319`;
`ios/FamiliarSC/Sources/FamiliarSCUI/ShipBridgeView.swift:22-23`, `:151-186`

`ExchangeAct` contains only the six doctrine acts and constructs the same action
bodies as the host's decision `body` match. A displayed `PilotProposal` retains one
id; confirmation gathers fresh, compares the typed act, and files the retained id
only on equality. The bridge retains the proposal on transport/exchange/decode
failures and discards/re-reads on a mind refusal. The only UI caller of
`confirmPilotAct` is `PilotActRow`; searches of the voice sources find no route to it.
The mock tests prove zero action POSTs on render/speak/cancel, one per confirm, retained
id on retry, and zero on a moved mind.

The repair claim's phrase “the only POST in the package” is too broad literally:
host-management methods and direct enrolment also POST. The safety property that
matters here does hold: `ExchangeClient.file` is the only `/v1/actions` writer, and
`DirectFeed.confirm` is its sole caller.

### 4. Reasons — repaired

**File:** `crates/whisker/src/wire.rs:288-367`, `:417-428`;
`ios/FamiliarSC/Sources/FamiliarSCUI/Briefs.swift:92-165`

The seam returns a stable reason code and branch-specific facts for every decision.
Swift has a case for every emitted code, builds the sentence from those fields, and
falls back to the unknown code plus sorted raw facts. Every actionable verdict gets a
separate `Because …` line; a hold carries its reason in the decision line.

### 5. Direct-mode dial claim — repaired

**File:** `ios/FamiliarSC/Sources/FamiliarSCUI/DirectFeed.swift:298-318`;
`ios/FamiliarSC/Sources/FamiliarSCUI/Briefs.swift:89-130`

The input has no `dial`. Direct mode renders `Briefs.pilot(governed: false)`, names
the surface without calling the seam's default level the captain's setting, and says
that nothing is filed without confirmation.

### 6. Checked-in FamiliarCore artifact — repaired

**File:** `ios/FamiliarCore/FamiliarCore.xcframework`;
`ios/FamiliarCore/Generated/familiar_core.swift:548-556`;
`ios/UCFFamiliar/Sources/UCFFamiliarApp.swift:72-76`;
`ios/project.yml:94-122`

The device and simulator libraries are arm64 slices, export
`uniffi_familiar_core_fn_func_whisker_advise`, and both contain the seam-2 strings for
`rungs`, `active`, `seam_version`, the reason codes, and `callOut`. Their SHA-256
values match the blobs committed at `749c4d9`, and the doctrine/seam/core-FFI sources
have not changed since its parent tree. A transient Swift probe linked the generated
binding to the checked-in simulator archive. With only the probe executable's platform
tag changed in `/tmp` so the host dyld could run it without an installed simulator
runtime, the archive itself returned:

```text
decision.type = call-paws
reasons.code = rescue.no-pump-in-reach
seam_version = 2
```

The probe used the exact requested hull and route facts: 188 mG, fuel 123, standard
171/67 and economy 114/95. The transient source was removed.

## What held

- The pure decision files contain no filesystem I/O; state and I/O remain in the
  runner/store boundary.
- `whisker_advise` parses one supplied JSON value, calls `wire::advise`, and does not
  perform an exchange action.
- Route and rung failures are counted rather than silently converted into zero-cost
  routes. Seam skew yields words and no proposal.
- Direct mode still has no dial write and cannot file an action through the voice.
- No key, live network, deployment, game action, or `coordination/` edit was used.

## Verification

- `cargo fmt --all -- --check`: **pass, exit 0**.
- `cargo clippy --all-targets -- -D warnings`: **pass, exit 0**. The existing
  `familiar-vision` build-script note about unavailable camera capture was emitted but
  is not a clippy diagnostic.
- `cargo test -p familiar-whisker -p familiar-core-ffi`: **pass, exit 0** — 91
  whisker library tests, 2 whisker binary tests and 1 integration test passed; the
  core-FFI and doc-test targets contain no tests.
- `cd ios/FamiliarSC && swift test`: **environment failure, exit 1** before manifest
  evaluation because the sandbox denies `/Users/ian/.cache/clang/ModuleCache`.
- `CLANG_MODULE_CACHE_PATH=/tmp/t237-b4-r2-swift/module
  SWIFTPM_MODULECACHE_OVERRIDE=/tmp/t237-b4-r2-swift/module swift test
  --disable-sandbox --scratch-path /tmp/t237-b4-r2-swift/scratch`: **pass, exit 0** —
  86 passed, 0 failed, 2 live tests skipped. Two test-only `NSLock` calls warn that
  they become errors in Swift 6 mode.
- `cd ios && xcodegen generate`: **pass, exit 0**; regeneration left no tracked diff.
- The exact requested `xcodebuild -project FamiliarAgent.xcodeproj -scheme
  UCFFamiliar -configuration Debug -sdk iphonesimulator -destination
  'generic/platform=iOS Simulator' CODE_SIGNING_ALLOWED=NO build`: **environment
  failure, exit 74** during package resolution because the sandbox denies the default
  DerivedData/SourcePackages directories; compilation did not start.
- Re-runs with DerivedData, package, manifest and Clang module caches redirected to
  `/tmp`, including Xcode's manifest-sandbox override: **environment failure, exit
  74** at SwiftPM's nested `sandbox-exec` (`sandbox_apply: Operation not permitted`).
  This environment also has no installed simulator runtime. The product-build bar
  therefore could not be independently completed here.
- Direct transient Swift link against
  `ios-arm64-simulator/libfamiliar_core.a`: **link pass**; direct launch exited 134
  because simulator programs require an unavailable `dyld_sim`. `vtool` platform-tag
  copy plus execution in `/tmp`: **pass, exit 0**, with the call-paws result above.

No production code, deployment, ship, gate, game action, human record, fleet state,
or coordination file was changed.
