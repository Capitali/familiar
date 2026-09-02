# T-232 brick 1 reciprocal review — itinerary structures and adopt-all restart

**Reviewer:** companion:codex

**Offer reviewed:** `c49b01c` (`claude/t232-itinerary`)

**Base:** `24ca44f`

**Current main while reviewing:** `930ff61`

**Verdict:** **RETURN** — the pure extraction and adopt-all direction are good, but the
branch is not yet the stop-level itinerary T-232 specifies and is not safe to integrate
over current main.

## Evidence

- `cargo test -p familiar-whisker`: **25 passed, 0 failed**.
- `cargo fmt --all --check`: passed.
- `cargo clippy -p familiar-whisker --all-targets -- -D warnings`: passed.
- A no-commit merge of `c49b01c` into `930ff61` conflicts in
  `crates/whisker/src/doctrine.rs`, `lib.rs`, and `main.rs`.

The good parts are real: ledger interpretation is now pure and load-id keyed; restart
adoption is expressed as all open loads in deterministic order; `reverted` and `cancel`
share one closed vocabulary with reconciliation; the one-contract wrapper is small; and
the plan fuel walk refuses unpriceable legs. Those are the right pieces to keep.

## Acceptance blockers

### P1 — `Vec<Active>` is an ordered contract list, not the itinerary T-232 needs

`Itinerary` currently stores `stops: Vec<Active>`, where every element contains a whole
origin→destination contract. `plan_legs` then expands each element as an indivisible
deadhead plus haul before moving to the next element. That structure cannot express the
reason this task exists: pick up A, pick up B, drop B, refuel, then drop A; coalesce two
operations at one berth; or put a refuel visit into the plan. The board explicitly calls
for ordered stops carrying pickups and dropoffs, with refuel stops as itinerary entries.

Landing this shape would therefore preserve today's sequential one-load-at-a-time model
behind an itinerary name and still require a model migration when UCF-Haul#43 arrives.
Use a station-stop type now, for example a station plus typed pickup/dropoff/refuel
operations and the affected load ids. A single contract may compile to the degenerate
origin/pickup and destination/drop sequence, but a contract must not remain the routing
node. Pin a pure route such as A-pickup → B-pickup → B-drop/refuel → A-drop and assert
both movement order and hold occupancy after each stop.

### P1 — the branch is behind live fixes in the exact files it replaces

Main moved after the branch point and the merge conflicts are semantic, not formatting.
The returned revision would otherwise regress incidents already repaired on live ships:

- a Booked load while berthed at its destination must still deadhead to its origin;
  `c49b01c` retains the old `here != dest` exemption;
- reconciliation must not read the pre-action ledger while `tick < pending_until`;
- a lost load must enter the cooldown and its old intent/idempotency record must be
  purged before a later booking can be a fresh intent;
- freight capacity must be checked against spare physical hold, including T-233 merchant
  goods, rather than only itinerary-attributed units;
- the `Trade` module, grant, route cache, holdings, sell/buy/carry phase, and one-action-
  per-fold ordering from `83025b2` must survive intact.

Rebase before the next review. Required combined regressions are: booked-at-destination
deadheads; a just-filed booking is not closed from the previous fold; a reverted load is
cooled down and later receives a fresh action id; merchant inventory reduces capacity
available to itinerary insertion; and Trade still runs only under its own grant after
the plan conversion.

### P1 — `best_insertion` does not compute a marginal plan rate

Candidates are sorted by `LoadRow::pilot_ticks()`. That row's `deadhead_ticks` was priced
from the ship's current berth, but an appended candidate begins at the existing plan's
last stop. Appending may leave earlier earnings unchanged, but it absolutely changes the
candidate's deadhead time. The router exposed to this function returns fuel only, so the
implementation currently lacks the information needed for the claim that it ranks by
marginal ℳ/tick.

Either extend the route seam to return the travel ticks from the insertion point or call
this only an empty-plan ranking seam and defer non-empty ranking honestly. Add the missing
ranking regression: candidate HIGH looks better from the current berth but is far from
the plan endpoint; candidate NEAR has the better true marginal plan rate and must win.
The existing multi-load tests cover capacity and fuel, but none covers this ranking rule.

## Next review bar

Return a revision rebased on current main that resolves all three conflicts deliberately,
uses stop-level itinerary data (or narrows the brick and its claims with Ian's recorded
acceptance), and adds the combined live-regression and marginal-ranking tests above.
Focused fmt/test/clippy must be rerun on the rebased revision; the full workspace bar is
then the pre-merge check. This review authorizes no merge, restart, deploy, gate, game
action, fleet mutation, or human-record change.
