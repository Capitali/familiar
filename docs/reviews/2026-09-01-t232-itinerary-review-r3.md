# T-232 brick 1 reciprocal review, round 3

Reviewer: companion:codex

Reviewed: `24ca44f..2581ca2`, with the round-3 delta
`2c73354..2581ca2`, on `codex/t232-itinerary-review-r3`

Verdict: **REJECT**

Round 3 repairs the doctrine defects it names. Pickup completion is load-ID scoped;
route navigation is keyed only by monotonic crane words; a planned route-stop fill goes
all the way to the full-tank state the fuel proof assumes; pending and tracked terminal
loads share one close handler. The station-stop shape, chronological ledger reducer,
capacity walk, empty-plan ranking boundary, and the old one-contract regressions are
also intact. The focused bar is green at 56 tests.

The scheduler gate is not whole yet, however. Its stale-course recovery remains above
the pending-adoption hold and can file both an engage and a fresh travel while the
runner says the ship's freight state is unknown. The merged merchant hold reconciliation
also counts a Delivered load as cargo still aboard, which can erase a real same-good
merchant position, and equal-tick adoption order can still depend on which lookup
resolved first. The first is the exact round-2 execution blocker this round claims to
close, so this is not safe to land as the offered repair.

No PROD endpoint, ship store, gate, journal, deployment, restart, fleet process, or
human-owned record was touched during this review.

## 1. The wedge recovery bypasses the pending-adoption scheduler gate

Severity: **blocker**

Evidence: `crates/whisker/src/main.rs:434-501`,
`crates/whisker/src/main.rs:504-538`,
`crates/whisker/src/main.rs:613-634`

Concrete failure: the adoption step can leave `pending_adopt` non-empty at line 493,
but the stale-course wedge runs before the hold at line 619. After the same docked route
has remained for 30 ticks, that path unconditionally files `engage` and then files a new
`travel` to the route's last station. Only after those two wire actions does execution
reach `holding-for-adoption` and sleep.

That route cannot be attributed safely while an older ledger-open row is unresolved: it
may be a pre-crash merchant carry, a resolved newer load's movement, or a stale course
that conflicts with the unseen contract. Round 2's accepted repair explicitly allowed
waiting out `pending_until` and engaging a course already exposed as `driveAwaiting`, but
required no carry, diversion, or resolved-newer movement until adoption completed. Even
if wedge recovery's `engage` is classified as completing a laid course, its unconditional
fresh `travel` is another commitment and remains outside the gate. This also explains
why the new pure adoption tests pass while `src/main.rs` still reports zero tests: the
runner contract itself is not pinned.

Repair accepted: make the pending-adoption hold dominate stale-course recovery. It may
wait out a filed fold and perform the one specifically justified `driveAwaiting` engage;
it must not run the wedge's fresh-travel belt while an ID is unresolved. Extract or
otherwise pin the scheduler transition: ledger-open L is pending, the ship is docked
with an unchanged non-empty route for more than 30 ticks, and no `travel`, carry, buy,
booking, diversion, or resolved-newer freight action is filed before L resolves or
closes. Keep one action per fold in that path.

## 2. Delivered freight is subtracted from merchant cargo as though it were aboard

Severity: **should-fix**

Evidence: `crates/whisker/src/doctrine.rs:110-119`,
`crates/whisker/src/main.rs:709-717`,
`crates/whisker/src/trade.rs:194-255`

Concrete failure: `freight_aboard` includes every load whose word is not `Booked`, so it
includes `Delivered`. But `Delivered` is precisely the state after the destination crane
has unloaded the freight; only its payment remains to collect. If the hull still carries
30 merchant ore when a 100-unit ore load becomes Delivered, `/v1/me.cargo` contains the
30 merchant units while the freight slice contains `("ore", 100)`. `reconcile_hold`
computes `max(30 - 100, 0)`, deletes the genuine position, and saves the empty book.
After collection removes the load, the next fold adopts those same 30 units as an unknown
lot at a new conservative basis and re-arms its hold clock. The hull does not sell
freight, but it loses the real basis and a valid sale opportunity; that is contrary to
T-233's “hold is the truth” repair.

Repair accepted: pass only `ActiveWord::PickedUp` cargo to `reconcile_hold`; Delivered
and Booked loads are not aboard. Pin same-good coexistence with merchant ore beside one
PickedUp ore load and one Delivered ore load: subtract the picked-up units exactly once,
sum multiple genuinely aboard loads, and leave the merchant lot's units, basis, target,
and clock unchanged after delivery.

## 3. Equal-tick plan order can depend on lookup timing

Severity: **should-fix**

Evidence: `crates/whisker/src/adoption.rs:38-79`,
`crates/whisker/src/ledger.rs:135-158`,
`crates/whisker/src/main.rs:494-501`

Concrete failure: `ledger::open_loads` correctly orders by `(booked_tick, load_id)`, but
the runner rebuilds that order as a map from ID to tick and then sorts `loads` by tick
alone. Suppose L1 and L2 were booked at the same tick, L1's board lookup misses on the
first cycle, and L2 resolves. On the next cycle L1 resolves and is appended after L2;
the stable tick-only sort preserves `[L2, L1]`, despite `open_loads` specifying
`[L1, L2]`. The sequential compile then flies lookup-resolution order rather than the
ledger's deterministic order.

Repair accepted: sort by both current-life booked tick and load ID (or carry the exact
rank returned by `open_loads`) and pin the delayed-resolution scenario. A load absent
from the current ledger may still sort last, as documented, with a deterministic ID tie.

## Rulings on the requested repairs

- **Station-stop route and navigation:** accepted apart from finding 1's runner bypass.
  `StopOp`, `Stop`, and `Itinerary { loads, stops }` can carry pickup A → pickup B →
  drop B + refuel → drop A. With A/B Booked the first pickup steers; A PickedUp advances
  to B; both PickedUp advance to B's drop/fill; B Delivered/removed advances to A's
  drop. A later-burned tank cannot make the old fill become an unfinished navigation op.
  The refuel-only-stop limitation is stated as a future planner precondition rather than
  silently claimed as supported.
- **Adoption and close handling:** the per-cycle retry, pending gate on the open-board
  fetch, `pending_until` reconciliation guard, pure adoption outcomes, and common
  `close_load` path are accepted. A pending terminal now gets cooldown, intent purge,
  adopt-note reset, and the same journal event as a tracked terminal. Finding 1 rejects
  the runner-wide gate; finding 3 narrows the ordering claim.
- **Ledger chronology:** accepted. Both consumers use the same chronological lifecycle;
  terminals win a same-tick booking, stale earlier rows cannot resurrect a close, and a
  strictly later booking starts a new life with its own booked tick.
- **Fuel execution:** accepted. Budget resets occur only at stops carrying `Refuel`;
  `decide_plan` fills to capacity at the current planned stop, while an off-route pump
  top-up is in-place and never navigation state. The empty-plan no-pump and pump-origin
  cases retain the old truncating reserve arithmetic.
- **Capacity:** accepted for the offered append/sequential seam. Occupancy begins at
  aggregate hold, does not add PickedUp cargo twice, drops it before a sequential later
  pickup, allows two full-hold sequential contracts, and lets merchant goods narrow the
  first pickup. Finding 2 is a separate merchant-book attribution error.
- **Ranking:** accepted at its stated altitude. It remains the board's ship-relative
  rate, is explicitly called a placeholder, and the live runner supplies an open board
  only to an empty plan. It must remain behind that gate until the router can provide
  tail-relative ticks.
- **Live-fix preservation and T-233 composition:** the booked-at-destination deadhead,
  pending-fold reconcile guard, close cooldown/fresh-intent purge, spare-hold fit, Trade
  grant, and the three old `active` gates converted to `loads.is_empty()` are present.
  Finding 1 is the unresolved-state exception to those gates; finding 2 is the defect in
  the generalized freight slice from the merged hold-truth merchant.

## The live restart and per-cycle compile

The expected wildhorse/KK II restart with one `inTransit` contract is safe on the core
adoption path: a successful held-board lookup restores `PickedUp`; under way the doctrine
holds; at the destination it waits for delivery/collect. A transient lookup miss now
persists and retries instead of being forgotten, and a normal arrival with no stale laid
route remains behind the adoption hold. I would not authorize the offered SHA as the
round-3 landing or live restart, however, because a docked stale-course state can still
escape that hold through finding 1.

Rebuilding `Itinerary::sequential(loads.clone(), &pumps)` every cycle is deterministic,
linear in today's tiny plan, and correctly compiles away completed pickup/drop work from
the latest ledger words. It has no material cost or current drift. A future real planner
must reconstruct chosen remaining order deterministically or persist route/progress;
otherwise a restart could reorder already committed stops. The documented inability to
navigate a standalone refuel stop belongs in that planner's acceptance bar.

## Verification

- `cargo test -p familiar-whisker` — pass: 56 passed, 0 failed; main and doc tests 0/0.
- `cargo clippy -p familiar-whisker --all-targets -- -D warnings` — pass.
- `cargo fmt --all --check` — pass.
- `git diff --check 24ca44f..2581ca2 -- crates/whisker` — pass.
- Full workspace tests were not rerun; the requested focused reproduction bar is green.
