# T-237 B2 FamiliarSC codex re-verification, round 2

**Verdict: REJECT.** Repairs 2 and 4 hold, and repair 3 now preserves
`captain_id` through both store and wire summaries. Two truth/identity boundaries still do
not hold in their production paths. Grounding broadens a tick-bound claim with every fact
that shares its station, so a second trade at that station can make a buy-to-sell inversion
pass. Separately, `captainIdentity` is only computed and tested: the pairing UI still joins
captains and computers by display label and can collapse two durable captain IDs.

Reviewer: companion:codex  
Reviewed: local `origin/main` `3d328a019869afa1212caf2ec80fc67e774887fe`
(including repairs `b6e27a6` and `bf0bb9c`), exported under `/tmp` because the review
branch base `ea3f9f6` was 14 commits behind the locally available main ref. The later
commits do touch FamiliarSC, so all commands and probes below used `3d328a0`.

## Findings

### 1. Blocker remains — a shared station can defeat strong-ID grounding

**File:** `ios/FamiliarSC/Sources/FamiliarSC/BridgeVoice.swift:118-151`, `:154-177`

The named one-fact cases are repaired: buy→sell, approved→denied, refusal omission and an
invented conversational station all trip the checked-in tests. Negation is also recognized.
The binder is not actually anchored to the strongest source identifier, however.
`identifiers` puts every hyphenated token—including stations and goods—into the same
`strong` set as ticks, loads and proposal IDs. `bind` then selects every fact sharing
*any* member of that set and unions their SIDE/OUTCOME claims. An unrelated opposite-side
trade at the same station can therefore supply the sign that the tick's own fact contradicts.

A disposable test against the production code used these facts:

```text
t123: bought 40 ore at ask 15 at foxys-diner — ℳ4400
t999: sold 5 fish at foxys-diner — ℳ4450
```

and this reply:

```text
At t123, we sold 40 ore at foxys-diner.
```

`Grounding.checkReply` returned `nil`; the inverted statement was accepted. This is the
brief's required transient inversion outcome in the wrong direction, and violates the B2
line that style may bend cadence but never truth.

**Repair required.** Treat load/tick/proposal IDs as the binding keys and never broaden a
match on one of them with station or other hyphenated vocabulary. Keep station tokens in
the invention check. Pin opposing trades at one station (and hyphenated goods) so the
statement must agree with the fact selected by its strong ID.

### 2. Accepted — Swift and Rust share all 18 autonomy surfaces and defaults

**Files:** `ios/FamiliarSC/Sources/FamiliarSC/Autonomy.swift:23-64`,
`crates/whisker/src/autonomy.rs:46-68`, `:114-187`,
`ios/FamiliarSC/Tests/FamiliarSCTests/Fixtures/contract/autonomy-surfaces.json`

The enumerations match key for key: 18 surfaces in five families. Both runtimes default
`navigation.rescue` and `market.margin` to `advise` and every other surface to `auto`.
Swift strictly decodes `{"market.margin":"confirm"}`, exposes it as a trade surface, and
honors surface/family/`*` precedence. Both suites read the same fixture and compare their
runtime enumeration length, every key and every unconfigured default against it.

### 3. Blocker remains — the durable ID is carried, but production pairing still label-joins

**Files:** `ios/FamiliarSC/Sources/FamiliarSC/ShipStore.swift:209-245`,
`ios/FamiliarSC/Sources/FamiliarSCUI/Feed.swift:23-48`,
`ios/FamiliarSC/Sources/FamiliarSCUI/WireFeed.swift:94-120`,
`ios/FamiliarSC/Sources/FamiliarSCUI/PairingView.swift:35-52`,
`ios/FamiliarSC/Tests/FamiliarSCTests/ContractDriftTests.swift:41-79`

The record half is repaired. `Captain` decodes `captain_id`, using empty only for legacy;
`StoreFeed` and `WireFeed` carry it into `ShipSummary`; modern wire captain documents use
the host-provided ID route. The modern, legacy, colliding-label and rename value tests pass.

The join half is not. A source-wide use check finds `captainIdentity` only at its declaration
and in tests. `ContractDriftTests.testCaptainScopedJoinsKeyOnTheIdNeverTheLabel` proves that
the computed strings differ; it does not exercise a production join. `PairingView` builds
`fleetCaptain` from a set of `.captain` labels, then finds `existingComputer` with
`$0.captain == captain`. `PairingRequest` carries the label, not the durable ID.

A disposable production-path probe supplied two named summaries with the same display
label `A B` and identities `id:c-1` and `id:c-2`. `PairingView.fleetCaptain` returned
`Optional("A B")`, collapsing two captains into the single prefill; the adjacent
`existingComputer` lookup would choose the first named row with that label. The host may
later refuse an ambiguous pair, but the Swift client has already made and displayed the
wrong join.

**Repair required.** Use the durable identity in the pairing selection/join and carry it in
the pair request when joining an existing captain. If a label maps to multiple IDs, require
an explicit identity or refuse instead of taking the first label match. Pin the actual
production helper/path, not only equality of the computed property.

### 4. Accepted — payment and refusal events are visible and deliberately ranked

**Files:** `ios/FamiliarSC/Sources/FamiliarSC/Notices.swift:35-80`,
`ios/FamiliarSC/Sources/FamiliarSC/TemplatedVoice.swift:59-85`, `:224-323`,
`ios/FamiliarSC/Tests/FamiliarSCTests/Fixtures/contract/journal-events.json`

`paid-down` now produces a money notice and a typed fact. `pay-down-refused` and
`trade-refused` produce distress notices, typed facts, danger state and severity 8, so both
outrank payment and routine activity in a full window. The shared fixture lists all 41
currently scanned runner event literals; the Rust test pins that vocabulary and the Swift
test requires a deliberate severity and typed renderer for every told event. The explicit
future rule is conservative: an unknown `*-refused` event is distress/severity 8, while an
unknown routine word is neutral severity 2 and does not buzz the captain.

## Held from round 1

- The templated lane remains always available and was the lane exercised on this host when
  Foundation Models was unavailable.
- `ProposeAutonomyTool` only records a `DialChange` for presentation; it does not write the
  dial. A captain act remains required to save a change.
- The unmodified suite used only fixture/mock keys. Its two live tests skipped because no
  dev world or live server/key was supplied. No production key or network was used.

The later `Persona.pronouns`, captain-economy/iPad work and bay fields are present on
`3d328a0`; I observed them but did not grade them as B2 repairs.

## Verification

- `cd ios/FamiliarSC && swift test --disable-sandbox --scratch-path
  /tmp/t237-b2-r2-3d328a0/scratch`, with Clang and SwiftPM module caches under `/tmp`:
  **100 passed, 0 failed, 2 skipped**. The build emitted two async-context `NSLock`
  warnings in `DirectPilotTests.swift:316`; they are test-only and not a B2 failure.
- The literal requested `swift build --package-path ios/FamiliarSC
  -Xswiftc -warnings-as-errors` was stopped before compilation by this host's
  `sandbox-exec` (`sandbox_apply: Operation not permitted`). Re-running the same package-wide
  build with `--disable-sandbox --scratch-path /tmp/t237-b2-r2-build/scratch` and module
  caches under `/tmp` **passed** with warnings promoted to errors. The only remaining output
  concerned inaccessible user-level SwiftPM caches.
- Transient grounding probe: **semantic inversion admitted**
  (`GROUNDING_PROBE_RESULT=nil`).
- Transient captain-join probe: **two IDs collapsed by label**
  (`["id:c-1", "id:c-2"]` → `Optional("A B")`).

The two transient test files lived only in the disposable `/tmp` export. This review made
no source change, approval, dial write, game action, deployment, shipment, gate decision,
key use, network call or `coordination/*` edit.
