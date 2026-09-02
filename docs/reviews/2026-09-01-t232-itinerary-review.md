# T-232 brick 1 reciprocal review

Reviewer: companion:codex

Reviewed: `24ca44f..e8666a8` on `codex/t232-itinerary-review`

Verdict: **REJECT**

The zero/one-contract doctrine reduction is real, including the old pump-origin
rounding, and the per-load `PickedUp` evidence seam is the honest evidence available
for a multi-load hold. The brick is not safe to land as the claimed restart and
itinerary foundation, however. Adoption is still a one-shot race that can permanently
forget KK II's live `inTransit` contract, the alleged oldest-first order is not booking
order, and the multi-stop fuel walk assumes refuels that no decision ever performs.
The data structure also serializes whole contracts rather than representing station
visits with pickups/dropoffs, while its ranking uses the candidate's old ship-relative
rate as an appended marginal rate. Those are failures in the seams this brick exists
to establish, not solver refinements.

No PROD endpoint, ship store, gate, journal, deployment, or fleet process was touched
during this review.

## 1. Adoption declares completion before every open load is resolved

Severity: **blocker**

Evidence: `crates/whisker/src/main.rs:341-363`

Concrete failure: `adopted` becomes true before any of the three loadboard reads. If
the first post-restart `/v1/me` already names KK II's open load but the `inTransit`
loadboard read fails or temporarily omits it, the loop silently adds nothing and never
tries again. There is a second race: an action accepted just before the old process
dies may not appear in the first fold read at all; this code then concludes adoption
from that early snapshot forever. In flight the doctrine merely holds with no load;
after arrival it can shop the open board while the exchange still owns the unseen
contract. A delivered row missed the same way leaves money uncollected. Thus the
adopt-all pure scan is correct only when every dependent read succeeds on the one
permitted attempt.

Repair accepted: treat adoption as reconciliation, not a startup boolean. On every
cycle merge newly ledger-open load IDs into the plan, keep unresolved IDs pending, and
retry their row lookup until each resolves or a terminal ledger word closes it. Do not
book while any open ID is unresolved. Pin both a first-read failure followed by success
and an accepted-before-crash event that appears on a later fold. For the current PROD
restart specifically, successful lookup under `status=inTransit` is otherwise safe:
`reconcile` restores `PickedUp` and `ship.in_flight` makes the doctrine hold under way.

## 2. “Oldest first” is ordered by the latest lifecycle event

Severity: **blocker**

Evidence: `crates/whisker/src/ledger.rs:73-100`

Concrete failure: the stored tick is replaced with `max(t)` for every booked, picked,
or delivered event. If LA is booked at t100 and picked up at t200 while LB is booked at
t150, `open_loads` returns LB before LA. After a restart, `decide_plan` therefore works
LB first even though LA was booked first and may already be aboard. The existing test
does not expose this because every later lifecycle tick in its fixture happens to
preserve the booking order.

Repair accepted: retain the earliest actual `booked` tick separately from lifecycle
state and sort by that immutable tick (with a deterministic load-ID tie break). Add a
regression in which an older load's pickup/delivery event occurs after a newer load's
booking and assert that the older booking remains first.

## 3. Fuel feasibility invents full tanks that the itinerary never buys

Severity: **blocker**

Evidence: `crates/whisker/src/doctrine.rs:210-270`,
`crates/whisker/src/doctrine.rs:371-400`

Concrete failure: `plan_fuelable` resets `budget` to capacity whenever a leg arrives
at a fuel-selling station. With an active plan, however, `decide_plan` returns from the
plan branch before the pump top-up rule and has no refuel stop/action. For example, it
can approve A→P→B because P sells fuel; after A is delivered/collected at P, the next
booked contract makes the doctrine file travel toward B without first refuelling. The
route can then be refused for insufficient fuel, stranding paid commitments. The test
at `doctrine.rs:741-763` proves only the optimistic reset, not an executable sequence
of decisions.

The empty-plan, one-candidate arithmetic itself does reduce exactly to the old checks:
without a pump it floors `(dead + haul) * 1.2` once, and with a pump at the origin it
floors `dead * 1.2` and `haul * 1.2` separately against current fuel and capacity. The
problem is that neither old nor new active-load behavior performs the refuel that the
pump-origin branch budgets; the multi-stop generalization multiplies that latent gap.

Repair accepted: make refuelling an explicit executable itinerary stop (as the board
requires), or make the active-plan decision consume a planned refuel before the next
travel. `plan_fuelable` may reset a budget only at such an action. Pin the whole state
sequence—arrival with depleted fuel, refuel decision/ack, then travel—not just the
static feasibility predicate.

## 4. `Vec<Active>` is booking order, not a multi-stop itinerary

Severity: **blocker**

Evidence: `crates/whisker/src/doctrine.rs:119-163`,
`crates/whisker/src/doctrine.rs:348-369`

Concrete failure: each alleged stop is a whole origin→destination contract, and
`plan_legs` always emits pickup A, drop A, pickup B, drop B. It cannot express pickup A,
pickup B, then a shared/ordered drop route; nor can one station visit carry multiple
pickups/dropoffs or a refuel. For A: X→Z and B: Y→Z, the only representation flies
X→Z→Y→Z instead of X→Y→Z. Capacity accounting simultaneously sums every undelivered
booking as if all cargo overlaps, so two sequential 80-unit loads are rejected from a
120-unit hold even though the generated legs never carry them together. The promised
LoadingOrder seam and deadline-aware packing have nowhere to attach without changing
this shape.

Repair accepted: keep load lifecycle state keyed by load ID, but model the route as
ordered station visits such as `Stop { station, pickups, dropoffs, refuel }`; derive
legs and hold occupancy by walking those actions. Pin a route that combines pickups,
a shared-station visit, peak (not summed-reservation) capacity, and a refuel action.
If this brick is intentionally only a sequential booking queue, narrow the log and task
claims and leave the actual itinerary structure unaccepted rather than saying no shape
change will be needed when the cap lifts.

## 5. The append heuristic does not calculate marginal rate

Severity: **should-fix**

Evidence: `crates/whisker/src/doctrine.rs:310-344`

Concrete failure: ranking uses each board row's `estimated_net / pilot_ticks`, whose
deadhead is priced from the ship's present berth. Fuel feasibility correctly appends
the candidate from the existing plan's final destination, but neither candidate ticks
nor net are recomputed from that destination. A load beside the ship now can rank first
even when the current plan ends far from its origin, adding enough deadhead time/fuel
to make it inferior or loss-making. Because the stale rank is applied before the
five-row pricing cap, the real best append may not even be evaluated. Appending leaves
the old stops' earnings unchanged; it does not leave the candidate's marginal cost and
time unchanged.

Repair accepted: obtain route ticks and monetary fuel cost for `plan_end → origin`,
then rank on the actual delta in plan net and pilot ticks (including handling and any
deadline effect), applying the pricing cap after that marginal calculation. If the
wire cannot supply those facts yet, call this a current-board-rate placeholder rather
than a marginal-rate seam and do not use it to authorize multi-load booking.

## 6. The terminal tightening ignores event chronology

Severity: **should-fix**

Evidence: `crates/whisker/src/ledger.rs:30-58`,
`crates/whisker/src/ledger.rs:75-99`

Concrete failure: the stated noise fixture is a booked event at t90 placed after a
terminal event at t120 in array order. Rejecting that resurrection is sound. The
implementation goes further: any terminal event anywhere makes the load closed forever,
even if a genuinely later-tick booking exists. If cancellation/reversion returns a
load ID to the open board and the ship later books that same contract again, restart
will ignore the live booking; `reconcile` would also stop at the historical terminal
word. The brick does not establish an exchange invariant that load IDs can never enter
a new booked lifecycle.

Repair accepted: fold each load's events in tick order with an explicit same-tick
precedence, so the t90 noise cannot beat the t120 closure while a legitimate t130
booking can start a new lifecycle. Apply the same chronological reducer to
`open_loads` and `reconcile`. Alternatively, cite and pin an upstream contract that a
terminal load ID is globally immutable; without that contract, the existential close
is not an honest reading of “last word.”

## 7. The recorded formatting bar is red

Severity: **should-fix**

Evidence: `crates/whisker/src/doctrine.rs:759`,
`docs/DEVELOPMENT_LOG.md:47-50`

Concrete failure: the development log says fmt passed, but `cargo fmt --all --check`
reports that the long `best_insertion` call must be wrapped and exits 1. The focused
tests and clippy are green, but the claimed bar is not reproducible at the reviewed
SHA.

Repair accepted: run `cargo fmt --all`, re-run the checks, and record only the bar
actually observed.

## Rulings on the remaining claims

- **Zero/one-stop behavior:** accepted. The 11 pre-existing doctrine tests are
  unmodified, `decide` is a direct one-stop wrapper, the ranking/filter/top-five order
  is unchanged for an empty plan, and the rounding reduction is exact as described in
  finding 3. This does not validate the new multi-stop behavior.
- **Adopt-all and closed vocabulary:** accepted in the pure function, subject to
  findings 1, 2, and 6. Every row that the one-shot runner successfully resolves is
  appended; `reverted` and `cancel` now agree with `reconcile`; a resolved delivered
  row beside a booked row is collected first rather than abandoned.
- **`laden` evidence:** accepted as a provisional seam. Aggregate `hold_used` cannot
  identify a contract, while the ledger's `PickedUp` word is load-ID-scoped and is
  defined here as “the hold has our cargo.” Unifying on aggregate hold now would let
  cargo from load A falsely release booked load B. Revisit only if UCF-Haul#43 supplies
  per-load hold evidence; add a regression that A's aggregate cargo cannot make a
  still-`Booked` B depart.
- **Per-stop reconciliation and empty-plan booking gate:** `retain_mut` is correctly
  load-ID keyed and booking remains gated to an empty plan for today's one-active
  exchange. It does not repair external-state drift because unknown/unresolved open
  contracts are never merged after the startup pass (finding 1).
- **PROD restart:** do not restart wildhorse's whisker on this SHA mid-flight. If the
  current `inTransit` row is found on the first pass, the path is safe and behavior is
  the old one-stop hold-under-way path. A transient omission on that single pass makes
  the failure permanent, which is too sharp a condition for the live hull.

## Verification

- `cargo test -p familiar-whisker` — pass: 25 passed, 0 failed; main and doc tests 0/0
- `cargo clippy -p familiar-whisker --all-targets -- -D warnings` — pass
- `cargo fmt --all --check` — **fail** at `crates/whisker/src/doctrine.rs:759`
- `git diff --check 24ca44f..e8666a8` — pass
