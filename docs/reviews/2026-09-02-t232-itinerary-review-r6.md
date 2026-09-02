# T-232 brick 1 reciprocal review, round 6

Reviewer: companion:codex

Reviewed: `24ca44f..599f8b2`, with the round-6 delta
`cfa094f..599f8b2` and the requested round-3 comparison
`2c73354..599f8b2`, on `codex/t232-itinerary-review-r6`

Verdict: **REJECT**

Round 5 closes the dangerous half of the stale-course return: one watch firing now
names one remedy, a successful remedy sets `pending_until` and ends the fold, and a
laid destination must agree with the newly known freight or merchant intent before it
is resumed. The merged pickup-window physics is also a faithful port: the runner
carries class, effective drive, wear, title/lease state, and route-leg geometry into
the doctrine; the doctrine uses the engine's per-leg arithmetic and refuses a booking
that cannot reach its pickup inside the window. All earlier station-stop, chronology,
fuel-execution, capacity, ranking-altitude, plan-order, and PickedUp-only freight
repairs remain intact. The focused bar is green at 70 tests.

The two effect-proof blockers from the other lane are not closed end to end, however.
The runner calls `freight_step` but discards its `may_act` result and still relies on
the old independent inline gate; the close test invokes the helper manually rather
than pinning both production paths or the promised fresh action id. There is also a
runner regression in the round-5 belt: because the watch itself is reached only after
the pending-adoption `continue`, its supposedly uninterrupted clock never starts on a
restart that begins pending. That can spend a second whole threshold sitting on a
valid course and lose the very pickup the course serves.

No PROD endpoint, ship store, gate, journal, deployment, restart, fleet process, or
human-owned record was touched during this review.

## 1. The runner never consumes the new typed adoption gate

Severity: **blocker**

Evidence: `crates/whisker/src/adoption.rs:84-124`,
`crates/whisker/src/adoption.rs:314-335`,
`crates/whisker/src/main.rs:507-545`,
`crates/whisker/src/main.rs:620-640`

Concrete failure: `freight_step` correctly computes `may_act = false` after a missed
held-board lookup, and the new test asserts that bit. Production then destructures the
other three fields piecemeal and never reads `step.may_act`; `grep` finds no production
use. The loop later asks `pending_adopt.is_empty()` again in the same inline gate that
the round-3 return said was insufficient proof. Thus the claimed structural match does
not exist: moving that later `continue`, or adding another action between the step and
line 626, recreates Buy/carry/booking/diversion/newer-load movement while the new test
stays green. The current ordering happens still to hold those actions, but round 6 was
offered specifically to make that invariant a typed, pinned boundary rather than
another inspection claim.

Repair accepted: make readiness an enum/result that the runner must match before it
can enter action selection, and put the scheduler behind the ready arm. The held arm
may apply close/journal bookkeeping, then sleeps and continues. Pin that runner-facing
transition with a missed lookup followed by resolution and an observable action sink,
asserting that no Buy, carry, booking, pump diversion, stale-course remedy, or
resolved-newer movement is reachable between the two folds. A boolean that production
does not consume is not the requested effect seam.

## 2. The belt's live clock does not run while adoption is pending

Severity: **blocker**

Evidence: `crates/whisker/src/adoption.rs:197-240`,
`crates/whisker/src/adoption.rs:453-472`,
`crates/whisker/src/main.rs:214-230`,
`crates/whisker/src/main.rs:620-664`

Concrete failure: the unit test seeds `WedgeWatch` at t100 while one adoption is
pending, observes it again at t140, then proves that clearing the pending id at t141
immediately yields `Engage`. The runner cannot execute that sequence. Its pending gate
continues at line 640; the sole `wedge.observe` call is below it, and consequently its
`pending_adoptions` argument is always zero. On a fresh restart with an already-laid,
valid course and an unresolved Booked load, the watch sees nothing during the entire
lookup outage. If the row resolves after 40 ticks, that resolution fold merely seeds
the watch; `ship_from` meanwhile calls the docked non-empty route `in_flight`, so the
ordinary doctrine holds. The ship waits another 31 ticks before engaging. A pickup
with the 48-tick desk window can revert during that invented second wait even though
the correct course stood ready throughout adoption.

Repair accepted: tick/observe the watch on every eligible fold, including the pending
hold, while keeping remedy execution below the adoption and pending-action gates.
Separate observation from execution if necessary. Pin the production scheduler from a
fresh watch: pending plus unchanged course at t100, still pending past the threshold,
resolution at t141, then exactly one validated `Engage` on that resolution fold.

## 3. The close regression still stops before both effect paths and the fresh id

Severity: **blocker**

Evidence: `crates/whisker/src/adoption.rs:126-165`,
`crates/whisker/src/adoption.rs:338-407`,
`crates/whisker/src/main.rs:184-212`,
`crates/whisker/src/main.rs:518-530`,
`crates/whisker/src/main.rs:553-570`

Concrete failure: extracting `close_transition` is a good mutation seam, and both
current call sites do route through the `close_load` shell. The promised regression
does not pin that wiring. It scripts only the pending-adoption outcome, manually calls
`close_transition`, calls `bookable` directly, and manually inserts a later adoption
notice. It never exercises the tracked-reconcile caller, never drives a re-listed row
through the live board filter, and never allocates or compares the later life's action
id. The comment says “fresh intent”; the assertion proves only a fresh notice. The
`src/main.rs` test binary still reports zero tests, so either caller can bypass the
helper or the runner can reuse the dead life's id while all 70 tests pass. Those are
the exact four effects and two callers the round-3 return made an acceptance blocker.

Repair accepted: expose one pure pre-action runner transition that consumes both
pending and tracked closes through `close_transition`, returns exactly one close
journal effect per close, and feeds the same `bookable`/intent-id state the live
scheduler consumes. Exercise both callers and the complete sequence: unresolved open
to reverted, cooldown rejection through the board filter, expiry, strictly later
booking, fresh adoption notice, and a newly allocated id unequal to the dead life's
id.

## 4. Closing one load can purge another load's intent by substring

Severity: **should-fix**

Evidence: `crates/whisker/src/adoption.rs:140-150`,
`crates/whisker/src/main.rs:1149-1171`

Concrete failure: `close_transition` deletes recent intents with
`!sig.contains(load_id)`. Load ids are values inside JSON strings, not unique string
delimiters. In an adopted multi-load plan, closing `L1` also deletes a recent Collect
or Book signature for `L10`. If the other action's fold has not appeared yet, the next
decision mints a new id for the same `L10` intent instead of reusing/suppressing the
old one, admitting the duplicate-action race that `recent` exists to prevent. A load
id equal to another JSON token is even broader.

Repair accepted: make the recent-intent key typed, or parse the signature and compare
its `loadId` value for exact equality. Pin prefix ids (`L1`, `L10`) and prove closing
one removes only its own intent while the other's idempotency record survives.

## Rulings on the round-5 belt

- **One remedy per firing:** accepted. `WedgeRemedy` makes Engage and Refile mutually
  exclusive; Refile requires the same course to survive another full threshold.
- **Destination validation:** accepted. The plan is compiled after adoption and the
  laid final station must equal the plan's working station, or the current merchant
  carry target with no freight. A mismatch is journaled and falls through to ordinary
  judgment rather than being resumed.
- **One wire action per fold:** accepted. A justified remedy makes one `wire.act`
  call; acknowledgement sets `pending_until`; success, refusal, and the surrounding
  branch all end the fold. Finding 2 rejects only the watch's runner-time chronology.

## Rulings on the round-6 seams and physics port

- **`freight_step`:** its pure calculation is correct: missed rows stay pending,
  resolved rows adopt, pending closes surface, and `may_act` reflects the remaining
  list. Finding 1 rejects the claimed runner wiring and end-to-end proof.
- **`close_transition` and `bookable`:** the helper performs cooldown, intent purge,
  notice reset, and one typed journal effect, and `main` currently calls the helper
  from both close sites and uses `bookable` in the open-board filter. Findings 3 and 4
  reject the incomplete regression and the imprecise purge.
- **Pickup-window physics:** accepted. `load_row` carries the service-class multiplier;
  `ship_from` carries effective acceleration, wear, and leased/title state; the shared
  route cache returns fuel and per-leg `distanceKm`; and `best_insertion` applies the
  engine's ceiling-square-root time at contract acceleration plus engage and loading
  overhead before the unchanged fuel proof. The far worn-economy and shorter-standard
  cases pin refusal and acceptance. The merged Repair decision is emitted by the
  runner and remains behind the Freight automation gate.
- **Ranking altitude:** still accepted. Physics screens board-ranked candidates but
  does not pretend to make the ranking marginal. The board fetch remains confined to
  an empty plan, so the existing stated-placeholder test is honest.

## Rulings on the earlier accepted work

- The station-stop model remains honest: `StopOp::{Pickup, Drop, Refuel}`, `Stop`, and
  `Itinerary { loads, stops }` carry pickup A to pickup B to drop B plus refuel to
  drop A. Navigation still keys only on load-scoped monotonic crane words.
- Pickup completion remains load-id scoped; merchant cargo cannot launch a Booked
  load. Route-stop fills execute to full, off-route pump fills remain in place at the
  90% line, and later burn cannot steer back to a visited fill.
- The chronological lifecycle reducer, terminal-wins same-tick rule, strictly later
  reopening, current-life booked tick, walked hold occupancy, sequential full-hold
  reuse, and merchant narrowing remain pinned.
- `freight_aboard` still returns PickedUp cargo only. The generalized slice passed to
  `trade::reconcile_hold` preserves a same-good merchant lot through delivery, and the
  round-4 `(booked tick, load id)` plan order still defeats delayed lookup resolution.
- The booked-at-destination deadhead, pending-fold reconciliation guard, common close
  shell, and T-233 merchant phase survive the merge. Buy selection, Buy filing, and
  carry remain gated by `loads.is_empty()`, with the adoption hold above them. The new
  merchant re-targeting behavior composes without changing those gates.

## The live restart and per-cycle compile

The stated KK II restart with one `inTransit` contract is sound on its ordinary core
path: adoption restores PickedUp, transient lookup omissions hold and retry, the hull
does nothing new under way, and delivery/collection resume at arrival. I would not
restart wildhorse's live whisker on this offered SHA as the round-6 landing: a restart
that also exposes a docked laid course can hit finding 2's extra threshold, and the two
runner-effect acceptance proofs remain open.

Rebuilding `Itinerary::sequential(loads.clone(), &pumps)` each cycle is still
deterministic, linear in today's tiny plan, and correct for the current one-contract
world: the latest ledger words compile completed work away without stored-progress
drift. A future interleaving planner must reconstruct its chosen remaining order and
standalone-refuel progress deterministically or persist them; this brick still states
that boundary honestly.

## Verification

- `cargo test -p familiar-whisker` — pass: 70 passed, 0 failed; main and doc tests
  0/0.
- `cargo clippy -p familiar-whisker --all-targets -- -D warnings` — pass.
- `cargo fmt --all --check` — pass.
- `git diff --check 24ca44f..599f8b2 -- crates/whisker` — pass.
- `git diff --check 2c73354..599f8b2 -- crates/whisker` — pass.
- `git diff --check 24ca44f..599f8b2` — pass; the earlier review EOF error is gone.
- Full workspace tests and clippy were not rerun; the request states that bar runs in
  parallel.
