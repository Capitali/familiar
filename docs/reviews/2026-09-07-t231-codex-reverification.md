# T-231 candidate-race codex re-verification

**Verdict: REJECT.** The first-success race fixes Ian's launch complaint, and its
cancellation/adoption boundary holds, but the landed health lifecycle does not meet
the recorded expiry bar. Both persistence repairs claimed by the chair also fail under
the ordinary five-second poll loop: `door.health.v1` is still rewritten every round,
and its population is not bounded as valid learned addresses accumulate.

Reviewer: companion:codex  
Reviewed: merge `ad6ece2` and the same three files at current HEAD `3f223397`  
Post-merge check: `git log ad6ece2..HEAD -- <three paths>` is empty; none of the
reviewed source or test files changed after the merge.

## Findings

### 1. Blocker — a never-answered dead door can never reach expiry

**File:** `ios/FamiliarMesh/Sources/FamiliarMesh/CandidateRace.swift:83-93`,
`:108-125`; `ios/Shared/Sources/AppModel.swift:1636-1705`, `:1895-1902`

**Failure scenario.** The never-answered branch says it measures a week of actual
attempts, but it compares `now` with `lastAttempt`. Every settled failure replaces
`lastAttempt` with the current round's clock. A door that fails on every five-second
poll therefore remains about five seconds old forever and never expires. The exact
poisoned-LAN shape is worse when its connect lasts past the 350 ms head start: the live
lighthouse wins, cancellation turns the LAN lap into `.cancelled`, and the LAN door is
not settled at all. With no health row, `plan` treats it as new forever. It is never
demoted, never starts an expiry clock, and receives a doomed connection attempt on
every poll and every later launch.

This does not undo the headline latency win: with `[dead LAN, live lighthouse]`, the
lighthouse starts at 350 ms and the launch completes in roughly one stagger plus its
RTT rather than after the LAN's ten-second request timeout. It does refute the promised
cross-launch health/expiry lifecycle and the test's claim that a never-answered door is
measured from its first attempt. `testANeverAnsweredDoorGetsItsChancesUntilTheClockRunsOut`
constructs a last attempt already more than a week old; it never exercises repeated
failures or winner-driven cancellation.

**Repair accepted.** Give the health record an age that is not refreshed by each miss
(for example `firstUnansweredAttempt`, cleared on success), and record that an attempt
started even when winner cancellation makes its reachability outcome unknown. Expire
against the uninterrupted no-success age, while keeping cancellation distinct from a
failure so a merely slower live rival is not demoted. Pin both sequences across many
rounds: quick failure on every poll, and a dead first request cancelled by a live second
winner. In each, the non-lighthouse door must leave the plan after seven days; the
lighthouse must remain.

### 2. Blocker — the health-map pruning repair has no finite bound and retains expired rows

**File:** `ios/Shared/Sources/AppModel.swift:296-309`, `:1843`, `:1903-1906`;
`ios/FamiliarMesh/Sources/FamiliarMesh/CandidateRace.swift:83-103`

**Failure scenario.** The prune retains every key present in `candidates`, and
`candidates` is the persisted `hosts` array. `learnHosts` only appends valid addresses;
neither the planner's expiry filter nor this prune removes an expired address from
`hosts`. A roaming mesh whose valid LAN addresses change over time therefore grows
both `hosts` and the health map without limit. Even an entry that does expire from the
race plan remains in both stores, so it is permanently filtered from races rather than
forgotten and allowed to re-earn a place if learned again. The chair's
"current candidates union lighthouse" filter is internally consistent, but the current
candidate set is itself append-only, so it does not prove or provide bounded growth.

**Repair accepted.** Make expiry one owned transition over all three pieces of state:
the plan, persisted candidates, and persisted health. Remove expired non-lighthouse
doors from both `hosts` and `doorHealth`, or impose a documented finite/LRU bound that
still always retains the lighthouse and current enrollment door. Pin more than the
bound's worth of changing valid addresses and prove the stored host and health counts
stay bounded; also pin that an expired address learned later returns with fresh health.

### 3. Should-fix — the defaults-churn repair still writes on every normal poll

**File:** `ios/FamiliarMesh/Sources/FamiliarMesh/CandidateRace.swift:108-125`;
`ios/Shared/Sources/AppModel.swift:1814-1821`, `:1895-1909`

**Failure scenario.** The chair changed the caller from unconditional saving to
`doorHealth != healthBefore`, but every real outcome changes the map. A success updates
both `lastAttempt` and `lastSuccess` to `raceNow`; a failure updates `lastAttempt` and
the counter. Thus a single healthy lighthouse causes `defaults.set` once per five-second
poll forever. The persisted value is not stable within a launch, let alone across
launches, and the chair's first repair only changed the spelling of the unconditional
write.

**Repair accepted.** Separate in-memory observation time from persistence dirtiness.
Persist immediately for meaningful transitions (new door, streak/demotion change,
success clearing a streak, expiry/prune), and checkpoint a continuously healthy
timestamp at a coarse bounded cadence if cross-launch freshness requires it. Add an
integration test with a spy store that runs many healthy polls and asserts a fixed
maximum number of writes, plus a relaunch test proving the health state survives.

### 4. Note — discarding a late second success is the correct conservative rule

**File:** `ios/Shared/Sources/AppModel.swift:1689-1703`

**Failure scenario.** There is no safety failure in the current rule. A second `.win`
already queued when the first win is consumed is ignored; that can delay revival of a
demoted but live door. Settling it after `cancelAll`, however, would make it impossible
to uphold the stronger requested invariant that no result from a cancelled candidate
is adopted. The present code adopts one `RaceWin`, promotes at most once, and settles
no late win.

**Repair accepted.** No production repair is required. An injectable race integration
test should pin simultaneous wins and a transport that returns success after ignoring
cancellation: exactly one result is adopted and only that winner is settled/promoted.
If future code wants the extra health evidence, it must distinguish a result completed
before cancellation from one delivered after cancellation rather than infer from
arrival order.

### 5. Note — one `raceNow` clock per settle is harmless at the current week-scale policy

**File:** `ios/Shared/Sources/AppModel.swift:1872-1874`, `:1898-1902`

**Failure scenario.** A staggered or slow loss records the race-plan time rather than
its exact start or completion time. The skew is bounded by stagger plus the ten-second
request timeout in this implementation, which is immaterial beside a seven-day expiry
window. It is not the cause of finding 1; resetting the only age on every round is.

**Repair accepted.** None required for T-231. If `lastAttempt` later becomes an exact
diagnostic or drives a short policy, carry a per-lap timestamp and settle with that.

## What held

- The accept-line timing holds by inspection. Ranks start at `0, 350, 700, ...` ms;
  therefore a dead first door delays the second live door by one stagger, not by the
  first door's request timeout. Physical cold-launch timing on Ian's iPad remains owed;
  this review did not substitute a device measurement for it.
- Cancellation is structured. `withTaskGroup` cannot return while child laps remain;
  after the first win `cancelAll()` is issued, sleep/request cancellation maps to
  `.cancelled`, cancelled laps settle nothing, and only the first `RaceWin` leaves the
  helper. The caller has one winner branch and one hysteresis/promotion decision, so no
  door is double-promoted.
- Actual completed failures do accumulate across launches, a success clears their
  streak, three failures demote stably, previously-successful dead doors expire against
  `lastSuccess`, and the exact lighthouse string bypasses expiry. Findings 1-2 cover the
  missing never-answered and bounded-storage cases.
- The all-fail path waits for every runner and carries each attempted host plus the
  existing compact cause vocabulary into `attemptLog`. One candidate is a delay-zero
  one-task race. With no network, all real failures reach the same aggregate error path.
  Enrollment, URL/pin construction, and pin learning were not changed by this brick.

## Verification

- `cd ios/FamiliarMesh && swift test`: first attempt was stopped before compilation by
  this review sandbox denying Xcode's `~/.cache/clang/ModuleCache` write.
- Rerun with Clang/SwiftPM module caches redirected into the package and SwiftPM's nested
  sandbox disabled: **48 passed, 0 failed**, including **CandidateRaceTests 8/8**.
- No Xcode app build was run; app builds are optional in the request. The Swift package
  bar does not compile or exercise the `AppModel` TaskGroup/persistence integration,
  which is why its eight pure planner tests remain green despite findings 1-3.

No production code, enrollment, pin, deployment, or fleet state was changed.

## Round 2 — repair re-verification (2026-09-07)

**Verdict: RETURN.** Repair `fab3aa8` closes all three Round-1 findings in its pure
model and checkpoint policy, and the repaired app compiles. Two integration edges still
break the promised forgetting lifecycle: the protected current door can become a
permanent tombstone, and `learnHosts` can expand the persisted candidate set again after
the bound has run.

Reviewer: companion:codex

Reviewed: repair `fab3aa8`, merge `b88f9b3`, and the same paths on main at `690166d`

Post-repair check: no reviewed source or test path changed after `fab3aa8`.

### R2-1. Blocker — an expired current door is retained as a permanent tombstone

**File:** `ios/FamiliarMesh/Sources/FamiliarMesh/CandidateRace.swift:178-229`;
`ios/Shared/Sources/AppModel.swift:1883-1919`

`forget` exempts every `keep` door before checking expiry and then retains the matching
health row. The caller passes the mutable current/preferred `host` as `keep`. If that door
has been silent for a week while the lighthouse is also unavailable, `plan` excludes it
as expired, but `forget` keeps both its host and its expired health. With no winning rival,
the five-success promotion path cannot move another door to the front. If the LAN address
later comes alive again, it is still present in `hosts`, so it is not re-learned, and its
old health keeps it out of every future race. The client is stranded from a door that is
answering again.

A focused regression retained `current` in `hosts`, then required the post-forget plan to
contain it. It failed: the retained health row kept the door filtered. The existing test
pins the opposite (`XCTAssertNotNil(out.health["current"])`) but does not plan from the
returned state, so it accidentally pins the tombstone.

**Repair requested.** Protect the current enrollment address from removal, not its expired
health. When a protected non-lighthouse door reaches expiry, keep it in `hosts` but clear
or renew its health so the next plan can knock again and begin a new silence window. Pin
the full returned-state sequence: expired current + unavailable lighthouse, forget,
current remains stored, current is eligible to race, and a later success revives it.

### R2-2. Blocker — successful reads can re-expand the store after the bound runs

**File:** `ios/Shared/Sources/AppModel.swift:296-309`, `:1913-1929`, `:2112-2113`

The bound is applied before the winner's worldview is consumed. Later in the same success
path, `learnHosts(view.hosts)` appends every valid missing advertised address and persists
the enlarged list without applying `CandidateRace.forget`. With more than 16 non-pinned
addresses advertised, each poll therefore follows the same cycle: `forget` drops and saves,
then `learnHosts` re-adds and saves. The next race snapshots the re-expanded list before
the next bound. Stored candidates and race width exceed the promised maximum, while the
old enrollment-write and narration churn returns at five-second cadence.

The pure 200-lease test adds one new address after each already-bounded transition. It does
not exercise the app's actual order or a worldview that repeatedly advertises every valid
door, so it cannot catch this integration failure.

**Repair requested.** Make learning plus expiry/bounding one transition before persistence,
and enforce the bound before the launch race as well as after accepting newly advertised
addresses. Pin a repeated >16-address advertisement through the integration seam: after
every successful poll, persisted hosts and health remain within the documented bound, the
next race has bounded width, and no drop/re-learn write or note loop occurs.

### What the repair did close

- `silentSince` now measures uninterrupted no-answer age; quick failures and
  winner-cancelled in-flight attempts reach expiry without treating cancellation as a miss.
- Legacy rows decode with a conservative derived silence age; success resets both the
  streak and silence clock.
- The pure `forget` transition removes ordinary expired doors from both stores and bounds
  its own returned set. R2-1 and R2-2 are caller/retention exceptions to that sound core.
- `needsCheckpoint` compares live state with the stored snapshot, persists policy
  transitions immediately, caps persisted failure streak semantics at demotion, and moves
  healthy timestamps only on hourly checkpoints.
- The first-winner/cancelled-loser adoption rule remains conservative and sound.

### Round-2 verification

- `swift test --package-path ios/FamiliarMesh`: **58 passed, 0 failed**;
  `CandidateRaceTests`: **18/18**.
- Temporary focused current-door recovery regression: **1 test, 1 expected failure** at
  the post-forget plan assertion; the probe was removed and the source tree restored.
- `xcodegen`; unsigned generic-iOS-simulator `FamiliarAgent` build: **BUILD SUCCEEDED**.
- `git diff --check`: clean before the review-record edit.

Physical cold-launch timing on Ian's iPad remains owed. No production code, enrollment,
pin, deployment, ship, gate, human/fleet record, or live device state was changed.
