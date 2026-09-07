# T-231 candidate-race codex re-verification — round 2

**Verdict: REJECT.** The silence clock, cancellation bookkeeping, and checkpoint
repairs hold, but the bounded-store repair does not survive the shell that owns it.
`refreshWorldview` bounds `hosts` before it consumes the winner's advertised hosts;
`learnHosts` then appends and persists those addresses after the bound. A successful
door can therefore leave the Keychain over the limit on every poll, continuously
re-advertise a door that was just expired, and put an arbitrary number of dead LAN
addresses back ahead of the lighthouse. Round-1 blocker 2 remains open in the actual
app path.

Reviewer: companion:codex  
Reviewed: repair merge `b88f9b3` (`b88f9b3^1..b88f9b3`) and the scoped files at
current HEAD `3e1ab7a`  
Post-merge check: `CandidateRace.swift`, `CandidateRaceTests.swift`, and
`AppModel.swift` are byte-identical to `b88f9b3`; later commits only moved the T-231
entry down in `DEVELOPMENT_LOG.md` while adding newer entries above it.

## Findings

### 1. Blocker — `learnHosts` runs after the bound and persists an unbounded candidate list

**File:** `ios/Shared/Sources/AppModel.swift:296-309`, `:1913-1924`,
`:2112-2113`; `ios/FamiliarMesh/Sources/FamiliarMesh/CandidateRace.swift:193-229`;
`ios/FamiliarMesh/Tests/FamiliarMeshTests/CandidateRaceTests.swift:228-267`;
`docs/DEVELOPMENT_LOG.md:102-107`, `:115-123`

**Failure scenario.** Begin with only the lighthouse enrolled. It answers and returns
17 or 200 valid non-tail addresses in `view.hosts` (the wire field is explicitly every
address the mesh currently advertises). The just-finished race calls
`CandidateRace.forget` against the old, small `hosts` list, so there is nothing to
bound. Only near the end of the winner branch does `learnHosts(view.hosts)` append,
reorder, and call `saveEnrollment`. The durable state at the end of the poll is now
over `maxRememberedDoors`; the transition that was claimed to own both stores never
saw it.

On the next poll, `readOrderedCandidates` has put every non-tail address before the
lighthouse and `plan` assigns a 350 ms head start to each. If those advertised
addresses are stale, 200 of them postpone the lighthouse by about 70 seconds. When
the lighthouse finally wins, `forget` writes the bounded list, but the same winner
payload re-adds the evictions and writes the unbounded list again. That is a real
forget/learn livelock: two Keychain mutations and repeated work per successful round,
with the launch-latency repair defeated by the re-expanded next race.

The under-the-cap expiry case is also not harmless. If a stale configured address is
still advertised, `forget` removes its host and health row; the same response
immediately re-adds it with no health. Its next mid-request cancellation starts a new
`silentSince`, buying it another week. Continuous advertisement can therefore prevent
the supposedly expired poisoned LAN door from ever staying forgotten.

The 200-lease test does not exercise either sequence. It appends one address and calls
`forget` immediately, but never performs the shell's later `learnHosts` step. The
"forgotten door learned again" test manually supplies the address to `plan`; it proves
that a removed health row is fresh, not that AppModel relearning is bounded or free of
feedback.

**Repair accepted.** Make host learning, expiry, capacity eviction, health pruning,
ordering, and persistence one transition whose *final* result is bounded and written
once. Define relearning evidence so the same payload that just caused an expiry cannot
immediately resurrect the address (for example, require an observed disappearance and
later reappearance, or retain a bounded suppression marker). Pin the real order with
an injected winner payload and spy enrollment store: after every poll, both stores
remain within the documented bound; a continuously advertised expired address stays
out; the same unchanged payload causes no repeat write or note; and the lighthouse's
start delay can never exceed the bounded policy.

### 2. Should-fix — capacity evictions are narrated as a week of silence

**File:** `ios/FamiliarMesh/Sources/FamiliarMesh/CandidateRace.swift:209-225`;
`ios/Shared/Sources/AppModel.swift:1921-1924`

**Failure scenario.** `forgotten` combines two different reasons: expiry at lines
212-215 and capacity eviction at lines 218-225. AppModel then tells the human every
entry in that union was "silent for a week." The repair's own 200-lease test begins
evicting after the seventeenth non-pinned address, only hours into its simulated run;
the app would describe those fresh capacity evictions as week-old silence. This is a
new false diagnostic in the repair.

**Repair accepted.** Return reasoned results (`expired` and `evicted`) and narrate them
truthfully, or use neutral wording that is valid for both. Pin a capacity-only eviction
whose evidence is younger than a week and assert it is not described as expired.

### 3. Should-fix — the pins substitute helper outcomes for the app paths they name

**File:** `ios/FamiliarMesh/Tests/FamiliarMeshTests/CandidateRaceTests.swift:166-183`,
`:228-326`; `ios/Shared/Sources/AppModel.swift:1653-1716`, `:1906-1929`,
`:2112-2113`; `docs/DEVELOPMENT_LOG.md:115-126`

**Failure scenario.** `testADoorCancelledByAFasterWinnerEveryRoundStillExpires`
directly passes `.attempted`; it never starts or cancels a task. The bound/relearning
tests never call `learnHosts`. The checkpoint tests assign `persisted = live` in local
variables rather than observing `loadDoorHealthIfNeeded`/`saveDoorHealth` or a relaunch.
Those are useful unit pins for `CandidateRace`, and the development log honestly
admits that AppModel is outside the package, but they do not exercise the failures
their integration-shaped names describe. Finding 1 is the regression that escaped
through that gap.

**Repair accepted.** Extract the race executor and host/health persistence transition
behind injectable fetch and store seams in a testable target. Pin a request that has
actually begun, a lighthouse win and `cancelAll`, an at-the-line cancellation, the
forget-then-advertise sequence from finding 1, write counts over repeated polls, and a
real encode/load re-launch. The app shell can remain thin while the production order
becomes package-tested.

## Repairs that held

- `silentSince` is the uninterrupted-silence clock requested in round 1. First
  failure/attempt sets it, later misses do not move it, success moves it, and `plan`
  expires against it. Legacy decode derives it from `lastSuccess`, else
  `lastAttempt`; even a legacy never-answered row whose old touch was recent gets at
  most one additional week after upgrade rather than an age refreshed forever.
- The poisoned-LAN cancellation path is reachable by inspection. Once the delay guard
  has passed, a lighthouse win calls `cancelAll`; a child that throws afterward still
  has sticky `Task.isCancelled == true`, so it reports `.attempted`, not `.loss`.
  `WorldviewClient.fetchWithRaw` wraps URLSession errors as `ReadError.transport`, so
  the outer `NSURLErrorCancelled` numeric check is not what makes this safe; the task
  flag is. A lap cancelled in `Task.sleep` returns no host and settles nothing. A
  transport that produces a late success can surface as a second `.win`, but that win
  is deliberately ignored and also settles nothing, conservatively avoiding demotion
  or adoption of a cancelled rival.
- `CandidateRace.forget` itself preserves the relative candidate order, never removes
  the lighthouse or `keep`, and prunes the corresponding health rows. Keeping an
  expired current host does not pin it in the race: `plan` filters it; after five
  fallback wins `promoteHost` moves the answering door to the front and a later round
  may remove the old one. If nothing answers, retaining one excluded enrollment door
  is consistent with the explicit keep rule.
- `doorHealthOnDisk` is loaded and advanced only with successful encoding/saving.
  Key-set and policy-streak changes persist immediately; the start of silence on a
  pre-existing all-zero row also exceeds the timestamp threshold in real epoch time.
  Otherwise a crash can lose less than one checkpoint interval of diagnostic/freshness
  time, but not a demotion or a newly created row. One checkpoint decision after all
  laps settles the whole map, so several doors in one round do not multiply writes.

## Verification

- `cd ios/FamiliarMesh && swift test --disable-sandbox`, with
  `CLANG_MODULE_CACHE_PATH` and `SWIFT_MODULE_CACHE_PATH` redirected to a fresh `/tmp`
  directory: **58 passed, 0 failed**, including **CandidateRaceTests 18/18**. SwiftPM
  warned that its user-level configuration/security/cache directories were read-only;
  compilation and tests completed with exit status 0.
- No app build or device timing run was requested. As the development log states, the
  package bar does not compile or execute AppModel's TaskGroup, Keychain, host-learning,
  or defaults wiring.

No production code, enrollment, pin, deployment, or fleet state was changed.
