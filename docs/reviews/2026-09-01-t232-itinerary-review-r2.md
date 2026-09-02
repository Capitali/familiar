# T-232 brick 1 reciprocal review, round 2

Reviewer: companion:codex

Reviewed: `24ca44f..2c73354` on `codex/t232-itinerary-review-r2`

Verdict: **REJECT**

Round 2 repairs most of the model honestly. The route is now station visits with typed
operations, the ledger reducer is chronological and current-life ordered, the board-rate
ranking is explicitly confined to today's empty-plan booking gate, and the focused bar is
green. Those are substantive repairs.

The rebuilt navigation is not safe to land yet. In today's one-contract world, merchant
cargo can make a merely Booked freight pickup read complete and send the hull straight to
the destination. In the multi-stop seam, a completed refuel can become incomplete again
after later fuel burn and steer the hull backwards. The adoption retry also blocks a new
freight booking but does not block merchant buy/carry or other empty-plan movement while a
live contract remains unresolved, and a contract that closes while pending bypasses the
preserved lost-load cooldown. These are execution failures in the exact T-232/T-233/live-
restart composition this round claims, not solver refinements.

No PROD endpoint, ship store, gate, journal, deployment, or fleet process was touched
during this review.

## 1. Aggregate merchant cargo completes an unrelated freight pickup

Severity: **blocker**

Evidence: `crates/whisker/src/doctrine.rs:229-246`,
`crates/whisker/src/doctrine.rs:250-256`,
`crates/whisker/src/doctrine.rs:326-340`

Concrete failure: a one-load plan treats `Pickup(L)` as done whenever
`ship.hold_used > 0`, without checking that the ship is at L's origin or that those units
belong to L. T-233 deliberately allows merchant inventory to remain in the aggregate hold
while freight is idle. If the ship carries 20 merchant units, books L from A to B while
berthed at S, and the ledger still says `Booked`, `current` skips A's pickup and selects
the drop at B. From S it files `Travel(B)`; if S is already B, the booked-at-destination
repair instead regresses to `waiting on the crane`. The ship never visits A and the load
can revert. This is worse than the preserved `83025b2` rule, which consulted aggregate
hold only while actually berthed at the origin, and it is the round-1 review's explicit
warning that cargo from one source must not make a still-Booked load depart.

Repair accepted: make pickup completion load-ID scoped for every plan size: `PickedUp` or
`Delivered` completes that load's pickup; unrelated aggregate hold never does. If the wire
truly needs a pre-ledger crane proxy, carry attributable evidence (or a before-pickup hold
baseline) rather than treating any occupied hold as this contract. Pin Booked L with
merchant cargo both at a third station and at L's destination, and require travel to A.

## 2. A planned refuel has no monotonic completion state

Severity: **blocker**

Evidence: `crates/whisker/src/doctrine.rs:229-259`,
`crates/whisker/src/doctrine.rs:482-513`,
`crates/whisker/src/doctrine.rs:947-968`

Concrete failure: `op_done(Refuel)` is derived from the ship's *current* fuel fraction.
In the pinned X-pickup → Y-pickup → W-refuel/drop-B → Z-drop-A route, a fill at W
can raise the tank above 90%, allowing the drop and travel to Z. Once that later leg burns
the tank below 90%, the full itinerary says W's old refuel is unfinished again. `current`
selects W and `decide_plan` files travel backwards instead of completing A at Z. The test
only checks arrival at W, the Refuel decision, and the next crane wait; it never walks the
claimed route beyond the fill.

The static fuel proof is also stronger than the executor. `plan_fuelable` resets the
post-stop budget to full capacity whenever a stop carries `Refuel`, but the executor skips
that action at any arrival already at 90%. Such a plan was proved with a full post-pump
20% reserve even though it may depart with only 90% of capacity. Thus the reset is tied to
the presence of an op, not to evidence that the promised fill was consumed.

Repair accepted: give route progress monotonic evidence—consume completed stops/ops or
retain explicit per-stop progress—and make the fuel proof use the exact post-refuel state
the executor guarantees. Pin the complete sequence through fuel burn after W: fill,
drop-B/collect as applicable, travel Z, and drop-A, with no return to W. Also pin arrival
just above the top-up line against a next stretch whose stated reserve requires a full
tank.

## 3. An unresolved adoption is not a commitment gate

Severity: **blocker**

Evidence: `crates/whisker/src/main.rs:375-425`,
`crates/whisker/src/main.rs:548-568`,
`crates/whisker/src/main.rs:571-629`,
`crates/whisker/src/main.rs:691-724`,
`crates/whisker/src/main.rs:753-754`

Concrete failure: the retry itself is real: a ledger-open ID remains in
`pending_adopt`, all three held-board statuses are asked again next cycle, and the open
freight board stays closed. But only that board is gated. While the row is unresolved,
`loads.is_empty()` still authorizes a merchant Buy and the carry-to-market Travel path.
If trade has nothing to do, `decide_plan` sees an empty plan and may divert a low-fuel
hull to a pump. In a multi-load fold, a newer row may resolve into `loads` while the older
booking remains pending, and the runner can execute the newer plan before it knows what
the older commitment requires.

For KK II's stated restart, an `inTransit` row that resolves while she is under way is
safe: reconciliation restores `PickedUp` and the doctrine holds until arrival. A transient
status-board failure is also harmless only while she remains under way. If it persists to
a berth, this code may buy cargo or leave the berth before adoption resolves. The claimed
fail-safe retry therefore has an unsafe arrival edge.

Repair accepted: treat non-empty `pending_adopt` as unresolved freight state throughout
the action scheduler, not merely as a condition on the open-board fetch. Selling cargo or
refuelling at the present berth can be separately justified, but no Buy, carry, diversion,
new booking, or resolved-newer freight movement should occur until every older open ID is
resolved or terminal. Extract and test this runner transition: first held-board read
fails, a later fold succeeds, and no competing action is filed between them. The current
test binary reports zero `src/main.rs` tests.

## 4. A load closed while pending bypasses the live lost-load repair

Severity: **blocker**

Evidence: `crates/whisker/src/main.rs:390-425`,
`crates/whisker/src/main.rs:473-500`,
`crates/whisker/src/main.rs:552-564`

Concrete failure: if `ledger::reconcile` returns a terminal word for a pending adoption,
the retain closure silently drops the ID. It does not write `lost_at`, purge the old
intent, or journal `load-closed`, unlike the normal tracked-load close path. A restart can
therefore see L open, fail to resolve its board row, see L revert on the next fold, and
immediately fetch/rebook a re-listed L because the 60-tick filter has no entry. That is the
same close-and-rebook loop `83025b2` added the cooldown to prevent. `adopt_noted` also
retains the terminal ID, so a later genuine life of that ID can lose its pending notice.

Repair accepted: route pending and adopted terminal outcomes through one close handler
that records the cooldown, purges any matching intent, clears adoption-note state, and
journals the close exactly once. Pin an unresolved-open → reverted → re-listed sequence
and prove it cannot be booked until the cooldown expires; then pin a strictly later new
life and its fresh adoption notice/action ID.

## Rulings on every claimed repair

- **Station-stop shape:** accepted. `StopOp`, `Stop`, and `Itinerary { loads, stops }`
  can represent pickup-A → pickup-B → drop-B/refuel → drop-A. The current pinned test
  proves the representation and initial occupancy calculation, but not navigation at each
  stage; findings 1 and 2 are why that distinction matters.
- **Adoption retry:** the per-cycle pending list, held-status retry, booking-board gate,
  booking-tick sort, and `pending_until` ordering are accepted in isolation. Findings 3
  and 4 reject the scheduler and terminal interactions around that core.
- **Chronology and current-life order:** accepted. Both consumers use the same tick-order
  lifecycle fold; terminal wins a same-tick booking, array-order noise cannot resurrect a
  closed life, a strictly later booking reopens with its own booked tick, and the old
  delivered-plus-new-booking/newest-event fixtures now exercise the right order.
- **Fuel walk:** the reserve rounding reduces to the old empty-plan pair of checks, and
  budget resets occur only at stops carrying `Refuel`. The new decision can execute a
  thirsty pump-origin fill. Finding 2 rejects the missing durable completion and the gap
  between a full-capacity proof and a 90%-complete executor.
- **Capacity:** accepted for the tested remaining-route states. Occupancy begins at
  aggregate `hold_used`, does not add a `PickedUp` load again, drops units per op, lets
  sequential full-hold contracts reuse capacity, and lets merchant inventory narrow a
  candidate. This does not rescue the separate pickup-completion misattribution in
  finding 1.
- **Ranking:** accepted at the narrowed altitude. It is the old board-row rate, the
  regression deliberately calls it a placeholder, and the live board fetch supplies it
  only to an empty plan. It must not authorize multi-load booking until the router exposes
  the tail-relative ticks needed for a marginal rate.
- **`83025b2` preservation:** the pending-fold reconcile guard, normal tracked-load
  cooldown/fresh-ID purge, spare-hold fit, Trade module and its 14 tests, and the three
  `active`-to-`loads` conversions are present. The booked-at-destination regression also
  runs unmodified, but merchant cargo defeats it via finding 1; pending terminal adoption
  bypasses the cooldown via finding 4; and the two idle-plan merchant gates need the
  unresolved state from finding 3.

## Rebuilding `Itinerary::sequential` and the live restart

For today's one-contract compile, rebuilding the itinerary every cycle is deterministic,
linear in a tiny load list, and correctly removes a completed pickup when the ledger word
advances. It has no material cost or drift. It also masks finding 2 in the live sequential
case by reconstructing away some past stops; an actual interleaved route cannot depend on
that accident. When `sequential` becomes a planner, the chosen remaining stop order and
completed refuels must be reconstructed deterministically or retained explicitly so a
fold/restart cannot reorder committed work.

Do not restart wildhorse's live whisker on this SHA as the round-2 landing. The expected
single `inTransit` row is safe if adoption resolves before arrival, but the code does not
fail closed if resolution remains transiently unavailable at a berth, and T-233 cargo can
mis-complete a later Booked pickup. No live restart should depend on those favorable
conditions.

## Verification

- `cargo test -p familiar-whisker` — pass: 45 passed, 0 failed; main and doc tests 0/0.
- `cargo clippy -p familiar-whisker --all-targets -- -D warnings` — pass.
- `cargo fmt --all --check` — pass.
- `git diff --check 24ca44f..2c73354 -- crates/whisker` — pass.
- Full workspace tests were not rerun; the request identifies the 944/0 workspace bar at
  `c0c3840`, while this review's target-specific reproduction bar is the focused whisker
  suite above.
