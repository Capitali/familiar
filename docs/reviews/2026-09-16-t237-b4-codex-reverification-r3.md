# T-237 B4 one-doctrine/two-runtimes codex re-verification, round 3

**Verdict: REJECT.** The ledger-open/mine-missing guard and the Swift mirror of
`ledger_word` are correct, and the 500 plus syntactically invalid JSON cases now fail
closed. Two blockers remain. First, the required mine-board read validates JSON syntax
but not the endpoint's array shape: an HTTP-200 object or `null` becomes an empty board,
and a transient probe showed a normal proposal and one confirmed action POST. Second,
the checked-in FamiliarCore archive was last rebuilt before the now-in-scope T-243
`contracts[]` and `denied` seam behavior. Both old and current implementations still
stamp seam 2, so the guard accepts the stale core; on one exact input the archive said
`repair` while current Rust said `hold` because repair was denied.

Reviewer: companion:codex

Reviewed: round-2 report
`docs/reviews/2026-09-08-t237-b4-codex-reverification-r2.md`; repair `7bdbbcd`;
current checkout `ea3f9f6`

## Findings

### 1. Blocker — valid JSON of the wrong mine-board shape still fails open

**File:** `ios/FamiliarSC/Sources/FamiliarSCUI/DirectFeed.swift:240-253`,
`:300-339`, `:462-483`;
`ios/FamiliarSC/Sources/FamiliarSC/JSON.swift:5-29`;
`ios/FamiliarSC/Sources/FamiliarSC/Exchange.swift:398-418`;
`ios/FamiliarSC/Tests/FamiliarSCTests/DirectPilotTests.swift:336-380`

The repair makes the GET and JSON parse throwing, but decodes the result as the generic
`JSONValue`. That decoder accepts every JSON kind. The later expression
`mine.array ?? []` consequently turns a successfully decoded object, scalar, or `null`
into the same empty board as a real `[]`. If the ledger is freight-idle, the inconsistency
guard has no missing id to find, so the adviser is asked and confirmation repeats the
same bad gather before reaching `client.file`.

A transient test was added only to a package copy under `/tmp`. Its exact material
inputs were the checked-in `me` fixture with `freight=[]`, `contract=null`,
`contracts=[]`, and `bookedLoad=null`, plus:

```json
HTTP 200 /v1/loadboard?mine=true
{"error":"temporarily unavailable"}
```

The canned adviser returned the suite's existing economy-divert verdict. The output
was:

```text
SHAPE_PROBE pilot=The pilot would now: fly empty to the pump at paws-neptune on the economy burn. ...
adviser_calls=2 posts=1
```

The selected test passed while asserting that rendered proposal and one POST, proving
the fail-open path through both render and confirm. A smaller decoder probe also showed:

```text
input={"error":"temporarily unavailable"} ... array=nil live-count=0
input=null ... array=nil live-count=0
input=[] ... array=present live-count=0
```

There is a second, non-actionable miss in the same claim: transport errors fail closed,
but are not named by endpoint. `ExchangeClient.get` converts them to
`ExchangeError.transport(error.localizedDescription)`, whose description contains no
path (`Exchange.swift:401-403`, `:364`), and the committed pin does not simulate a
transport failure.

**Repair accepted.** Decode the mine response as an array (for example
`[JSONValue]`) and wrap it only after that succeeds, so every non-array JSON value throws
an endpoint-named decode error. Preserve the endpoint when wrapping transport failures.
Pin HTTP 200 object/`null` and a `URLProtocol` transport failure through render and
confirm, including endpoint text, no proposal, no adviser call, and zero POSTs.

### 2. Blocker — the checked-in iOS core predates the seam facts Swift now relies on

**File:** `ios/FamiliarSC/Sources/FamiliarSCUI/DirectFeed.swift:78-79`,
`:313-325`, `:347-350`;
`crates/whisker/src/wire.rs:224`, `:409-426`, `:461-488`;
`ios/UCFFamiliar/Sources/UCFFamiliarApp.swift:77-81`;
`ios/project.yml:94-122`;
`ios/FamiliarCore/FamiliarCore.xcframework`

T-243 is present in this checkout. Direct mode now sends the rest of the bay as
`contracts[]` and the key's prohibited verbs as `denied`; current `wire::advise`
consumes both and calls `decide_with`. UCFFamiliar, however, executes the checked-in
xcframework through `whiskerAdvise`. Git history shows the archive's last rebuild is
`532099c` (2026-09-09), before the T-243 doctrine/seam commits, while neither the Rust
nor Swift seam number moved from 2. The runtime guard therefore cannot detect the skew.

A transient Swift executable linked the checked-in simulator archive without modifying
it. Only the executable's platform tag was changed in `/tmp` so it could run without a
simulator runtime. Current Rust source was run over the byte-equivalent JSON. The exact
deciding facts were a titled hull docked at `a`, wear 6000 bps, credits 100000, fuel
590/600, an empty board, a pump at `a`, and `denied:["repair"]`. Results:

```text
checked-in archive: seam_version=2, decision={"type":"repair"}
current Rust source: seam_version=2, decision={"type":"hold","why":"no fuelable work on the board"}
```

The archive ignores `denied` and offers an act this key cannot file. It also predates
the `contracts[]`/tour behavior, so the newly gathered bay can likewise be silently
ignored. The Swift package tests use `ScriptedMind` canned verdicts and do not link this
archive; the Rust tests exercise current source. Both suites can therefore remain green
while the shipped runtime disagrees.

**Repair accepted.** Rebuild both xcframework slices from current doctrine/seam source
and pin the checked-in artifact against current `wire::advise` on inputs containing
`denied` and multiple contracts. A semantic input/decision change that an older core
may ignore needs a seam-version bump; additive JSON compatibility is not enough when
the shell relies on the new fact for safe judgment.

## Claim verdicts

1. **NOT HELD — required, named mine-board read.** HTTP failures and invalid JSON throw,
   but valid non-array JSON is accepted and can reach an action POST. Transport failure
   text also omits the endpoint. See finding 1.
2. **HELD — ledger-open load absent from a successful array board.**
   `DirectFeed.openLoads` is compared with the live mine ids before the adviser call
   (`DirectFeed.swift:327-339`). The response names each missing load and word, returns
   a null verdict with no proposal, and fresh confirm refuses it before filing
   (`:471-482`). The committed L3249 test exercises render, proposal, confirm, and zero
   POSTs.
3. **PARTIAL — regression pins.** Both named tests prove zero POSTs on their covered
   paths. The mine-read test covers HTTP 500 and syntactically invalid JSON, but not a
   transport failure or valid JSON of the wrong shape; the inconsistency test does not
   explicitly assert that the adviser's call count stayed unchanged
   (`DirectPilotTests.swift:336-380`).
4. **HELD — Swift mirrors `ledger_word`.** Both implementations process events in
   order; close on `payment taken`/`collected`; lose on reverted/expired/lapsed/cancel;
   lose a rejection only with no prior word and otherwise skip it; and rank delivered
   over picked-up over booked, defaulting to booked. Compare
   `DirectFeed.swift:434-457` with `doctrine.rs:420-450`.

## Round-1 findings re-confirmed

- **Finding 1, rung facts and generation guard: PARTIAL.** Pump legs still carry the
  standard/economy hull quotes and missing quotes are counted. The original seam-2 rung
  boundary remains pinned. The generation guard no longer establishes source/artifact
  parity because newer decision semantics retained seam 2; see finding 2.
- **Finding 3, captain-confirm path: HELD.** Rendering/canceling do not write, one shown
  proposal retains one action id, confirm re-asks fresh and compares the typed act, and
  `DirectFeed.confirm` remains the only caller of `ExchangeClient.file`.
- **Finding 4, reasons: HELD.** Actionable source verdicts still carry stable reason
  codes and facts, and Swift renders them from those facts with an unknown-code fallback
  (`Briefs.swift:134-167`).
- **Finding 5, no direct-mode dial claim: HELD.** Direct input has no dial and
  `Briefs.pilot(governed:false)` states that no dial governs the device and confirmation
  is required (`Briefs.swift:90-130`).
- **Finding 6, checked-in FamiliarCore artifact: NOT HELD.** The artifact is executable
  and exports the seam, but it is not the current doctrine it claims to be; the direct
  parity probe in finding 2 produced different decisions under the same seam stamp.

## Verification

- `cargo fmt --check`: **pass, exit 0**.
- `cargo clippy --all-targets -- -D warnings`: **pass, exit 0**. The existing
  `familiar-vision` build-script note about unavailable camera capture was emitted but
  was not a clippy diagnostic.
- `cargo test -p familiar-whisker`: **pass, exit 0** — 119 library tests, 2 binary tests,
  and 1 integration test passed; 0 failed. The doc-test target contains 0 tests.
- `CLANG_MODULE_CACHE_PATH=/tmp/t237-b4-r3-codex/module
  SWIFTPM_MODULECACHE_OVERRIDE=/tmp/t237-b4-r3-codex/module swift test
  --disable-sandbox --scratch-path /tmp/t237-b4-r3-codex/scratch`: **pass, exit 0** —
  95 passed, 0 failed, 2 live tests skipped. The two existing test-only `NSLock` calls
  warn that they are unavailable from asynchronous contexts in Swift 6 mode.
- Wrong-shape transient Swift test in a copied package under `/tmp`: **pass, exit 0** —
  the exact HTTP-200 object input rendered an act, called the adviser twice, and recorded
  1 POST.
- Direct checked-in-artifact probe: **link pass**; expected direct launch failure because
  the executable was tagged for the unavailable iOS Simulator; `/tmp` executable-only
  platform-tag copy: **pass, exit 0**, returning seam-2 `repair`. Current-source Rust
  probe over the same input: **pass, exit 0**, returning seam-2 `hold`.
- The simulator and device archive SHA-256 values were respectively
  `7caa9bd08c49270f10fe4ab7017be00e337baf5a81b10b5bd6340db82e01a060` and
  `e4d611882539ff50b50510dfd11dae9d3f1353e08ba574f713ccbf9a4857ed5f`.

All transient sources, builds, and modified package copies stayed under `/tmp`. No
production source, network, deployment, ship, game action, key, or `coordination/` file
was changed.
