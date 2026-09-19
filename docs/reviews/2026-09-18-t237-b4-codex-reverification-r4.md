# T-237 B4 one-doctrine/two-runtimes codex re-verification, round 4

**Verdict: REJECT.** Repair 2 is held: seam 3 is enforced on both sides, the two
shared fixtures cover the denied-repair disagreement and an active contract plus two
companions, current Rust reads those fixtures at compile time, the generated
UCFFamiliar scheme reaches the app-hosted archive test, and the checked-in archive
itself returned the expected seam-3 decisions. Repair 1 closes every top-level shape
and transport case named in round 3, but validates only that the response is an array,
not that every member is a load-board row. A malformed member carrying the ledger's
load id passes the consistency guard, is discarded by the doctrine, and restores the
same freight-idle fail-open path: a transient Swift probe rendered a proposal and
filed one POST, while the checked-in core independently judged an analogous hull free
to book another load.

Reviewer: companion:codex

Reviewed: round-3 report
`docs/reviews/2026-09-16-t237-b4-codex-reverification-r3.md`; repair merge
`75b32ae`; current checkout `47cf64b` (the one newer `origin/main` commit adds only
the round-4 brief)

## Finding

### 1. Blocker — an array member that is not a row still fails open

**File:** `ios/FamiliarSC/Sources/FamiliarSCUI/DirectFeed.swift:250-262`,
`:309-345`; `ios/FamiliarSC/Sources/FamiliarSC/Exchange.swift:100-120`,
`:419-421`; `crates/whisker/src/wire.rs:81-120`;
`ios/FamiliarSC/Tests/FamiliarSCTests/DirectPilotTests.swift:343-375`

The repair decodes `/v1/loadboard?mine=true` as `[JSONValue]`. That rejects an
object, `null`, a string, a number, a bool, and invalid JSON at the outer boundary,
but every JSON value is still valid as an array member. No row validation follows.
`live` retains a member with an absent or unknown status, `liveIDs` regards any
string `loadId` as proof that the ledger's contract is present, and `active` forwards
the member to the core. The core's `load_row`, in contrast, requires string
`loadId`, `origin`, and `dest`; it silently returns `None` when either route endpoint
is absent. The shell can therefore certify a row by id that the doctrine then drops.

A transient test in a copied package served this valid HTTP-200 body while the
checked-in `me` fixture's ledger still held L3249 open:

```json
[{"loadId":"L3249"}]
```

The id defeated the missing-row guard. The pilot document announced a normal
"The pilot would now" decision, `pilotProposal` returned an act, confirmation
repeated the malformed read, the adviser was called twice, and one `/v1/actions`
POST was recorded. The probe passed while asserting those outcomes. This is the
same render-and-confirm failure mode as round 3; only the malformed value has moved
one level down.

The checked-in archive confirms the semantic consequence without a canned verdict.
On a seam input whose ledger said L1 was booked, whose `active.row` was only
`{"loadId":"L1"}`, and whose open board offered L9, the archive returned:

```text
seam_version = 3
decision = {"type":"book","load_id":"L9"}
```

The endpoint read is not sound merely because its outer container is an array. There
is already a typed `[Load]` decoder at `ExchangeClient.loadboard`; validating the raw
array against that shape (or explicitly requiring the doctrine's row fields) before
using the raw values would close this path. Add a malformed-member case such as the
one above to the existing render/confirm pin and require a named endpoint error, no
proposal, unchanged adviser count, and zero POSTs.

## Repair verdicts

### 1. Mine-board shape and named transport failures — PARTIAL

The requested top-level cases are correctly repaired. `DirectFeed` decodes
`[JSONValue]` and throws an endpoint-named `ExchangeError.decode` for the round-3
object, `null`, `"[]"`, and `7` cases (`DirectFeed.swift:254-262`).
`ExchangeClient.get` prefixes transport failures with the requested path, and
`file` names `/v1/actions` (`Exchange.swift:398-405`, `:445-462`).

The committed pin does walk 500, invalid JSON, all four specified valid-but-wrong
top-level values, and a `URLProtocol` transport failure through render, proposal, and
confirm. It checks endpoint text, no rendered proposal, the adviser's final call
count unchanged, and zero POSTs (`DirectPilotTests.swift:343-375`). `MockExchange.fail`
does produce a response-less `URLError` (`:13-60`). Thus every exact round-3 probe is
held. Finding 1 is the remaining admitted shape and makes the repair partial.

### 2. Seam 3 and checked-in archive parity — HELD

- The seam rule now explicitly includes additive facts on which safe judgment
  depends, and Rust stamps 3 (`crates/whisker/src/wire.rs:221-228`). The shell expects
  3 (`DirectFeed.swift:74-79`) and refuses every value not equal to it before it
  renders or constructs an act (`:350-359`). The committed mismatch test uses seam 4,
  not the specifically historical seam 2 (`DirectPilotTests.swift:410-420`), so it is
  not an exact old-core regression pin; the equality guard nevertheless rejects seam
  2 as written. A transient variant supplying the historical seam-2 verdict also
  passed while asserting the mismatch words, no proposal, and zero POSTs.
- `seam-denied-repair.json:3-12` is the exact round-3 input and expectation: the
  titled, worn, funded, nearly full hull with `denied:["repair"]` holds. The direct
  checked-in-archive probe returned seam 3 and that `hold` decision.
- `seam-bay-full.json:3-18` contains one `active` plus two entries in `contracts[]`
  and expects travel to b instead of booking L9. It therefore covers multiple
  contracts, not merely presence of the field. The archive returned seam 3 and
  `travel` to b.
- Rust uses literal `include_str!` calls for both files and compares source output's
  seam and decision with their expectations (`wire.rs:575-610`). Cargo's generated
  dependency file lists both JSON files as inputs, and the compiled test binary
  contains their text. Under the read-only-source rule I did not edit a fixture, but
  those compiler inputs establish that a content edit invalidates/rebuilds the test
  and changes the bytes it parses. The parity test passed in the 120-test library
  run.
- `CorePinTests` loads the same two bundled resources, calls `whiskerAdvise` from the
  app-linked archive, and compares seam plus decision (`ios/UCFFamiliarTests/CorePinTests.swift:10-21`).
  `project.yml` makes it an app-hosted unit-test target, bundles both fixtures, and
  includes it in `UCFFamiliar`'s `testTargets` (`ios/project.yml:117-149`). XcodeGen
  produced `UCFFamiliar.xcscheme` with a non-skipped `UCFFamiliarTests` testable, so
  `xcodebuild test -scheme UCFFamiliar` reaches it after project generation.
- A direct Swift executable linked to the checked-in simulator slice and run from a
  host-platform-tagged copy returned the two expected fixture decisions above.
  The simulator and device archive SHA-256 values are respectively
  `dc567df5b885edcba734a55365df0a15f4b092fab84a92c194e8e777c14ceec2`
  and `d93ea21c67f8a8625c697dbc6bb4c9d56e52f3833631ca7927b6f5509acdc771`.

Non-blocking documentation residue remains: the companion-building comments in
`DirectFeed.swift:322-325` and `wire.rs:465-468` still say that the seam version was
unchanged (the Rust comment even names seam 2), despite this repair correctly moving
the executable contract to seam 3.

## Earlier claims re-confirmed

- **HELD — ledger-open/mine-missing guard, on its stated absent-id case.** A
  well-formed successful array missing L3249 produces the named inconsistent-record
  document before the adviser, offers no proposal, refuses confirmation, preserves
  the call count, and records zero POSTs (`DirectFeed.swift:336-346`;
  `DirectPilotTests.swift:378-405`). An invalid pseudo-row with the same id evades it;
  that is finding 1, not a failure of the exact absent-id assertion.
- **HELD — `openLoads` mirrors `ledger_word`.** Both walk events in order, close on
  payment taken/collected, close on revert/expiry/lapse/cancel, treat rejection as
  loss only before any live word, preserve delivered over picked-up over booked, and
  default an otherwise nonterminal ledger to booked. Compare
  `DirectFeed.swift:423-466` with `crates/whisker/src/doctrine.rs:412-450`; the L3249
  and settled/lost fixture assertions remain at `DirectPilotTests.swift:398-405`.
- **HELD — captain-confirm path.** Rendering and canceling do not write; a displayed
  proposal retains one action id; confirmation gathers fresh, compares the typed act,
  and only then calls `ExchangeClient.file` with that retained id
  (`DirectFeed.swift:471-493`). The test suite still pins zero POSTs for render/cancel,
  one POST per confirmation, id reuse, and zero POSTs when the mind moves
  (`DirectPilotTests.swift:241-310`). `DirectFeed.confirm` remains the only source
  caller of `client.file`.
- **HELD — reasons.** The shell renders each stable emitted reason code from the
  doctrine's facts and preserves an unknown code plus sorted facts instead of
  inventing a rationale (`Briefs.swift:134-167`; `DirectPilotTests.swift:447-471`).
- **HELD — no direct-mode dial claim.** The direct input contains no dial, rendering
  calls `Briefs.pilot(... governed:false)`, and the words say no dial governs this
  device and a captain must confirm (`DirectFeed.swift:335`, `:368`;
  `Briefs.swift:86-131`). `DirectFeed.setDial` still refuses as host-only
  (`DirectFeed.swift:496-498`).

## Verification

- `cargo fmt --check`: **pass, exit 0**.
- `cargo clippy --all-targets -- -D warnings`: **pass, exit 0**. The existing
  `familiar-vision` build-script message about unavailable camera capture was emitted;
  it was not a clippy diagnostic.
- `cargo test -p familiar-whisker -p familiar-core-ffi`: **pass, exit 0** — 120
  whisker library tests, 2 whisker binary tests, and 1 integration test passed; the
  core-FFI and doc-test targets contain 0 tests.
- `CLANG_MODULE_CACHE_PATH=/tmp/t237-b4-r4-codex/module
  SWIFTPM_MODULECACHE_OVERRIDE=/tmp/t237-b4-r4-codex/module swift test
  --disable-sandbox --scratch-path /tmp/t237-b4-r4-codex/scratch`: **pass, exit 0** —
  100 tests executed, 0 failures, 2 live tests skipped. The two existing test-only
  `NSLock` calls warn about Swift 6 async-context availability.
- Malformed-array-member transient Swift test in a copied package: **pass, exit 0** —
  normal proposal rendered, 2 adviser calls, 1 POST after confirm.
- Direct checked-in-archive probe: **link and execution pass, exit 0** after changing
  only the transient executable's platform tag. Fixture outputs were seam-3 `hold`
  and seam-3 `travel` to b; the malformed-active-row input produced seam-3 `book` L9.
- XcodeGen to `/tmp`: **pass**; the generated `UCFFamiliar` scheme contains
  `UCFFamiliarTests` and the generated project includes both fixture resources.
  `xcodebuild build-for-testing` and simulator execution did not complete successfully:
  this sandbox cannot connect to CoreSimulatorService and Xcode's nested SwiftPM
  manifest diagnostics still target the denied user cache even with build/package/
  module caches redirected to `/tmp`. The direct archive execution above substitutes
  for the optional simulator probe.

Transient Swift sources, generated Xcode projects, and executable copies stayed under
`/tmp`; Cargo used the checkout's ignored `target/` build tree. This report is the only
working-tree file intended for the commit. No production source, network, deployment,
ship, game action, key, or `coordination/` file was changed.
