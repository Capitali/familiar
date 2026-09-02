# T-232 brick 1 reciprocal review, round 4

Reviewer: companion:codex

Reviewed: `24ca44f..807e378`, with the round-4 delta
`2581ca2..807e378` and the requested round-3 comparison
`2c73354..807e378`, on `codex/t232-itinerary-review-r4`

Verdict: **REJECT**

Round 4 repairs the two data defects cleanly. Merchant reconciliation now subtracts
only PickedUp freight, and the live call site uses that helper; a Delivered same-good
contract no longer erases the merchant lot. Plan order is now the current life's
`(booked tick, load id)`, including the delayed-resolution/equal-tick case. The
`WedgeWatch` predicate also does stay false while `pending_adopt` is non-empty, without
resetting its clock. All previously accepted doctrine and ledger repairs remain intact,
and the focused bar is green at 62 tests.

The wedge repair is not complete end to end, however. When the predicate finally
fires, the runner still sends two actions with two fresh IDs and then continues through
the same scheduler cycle. Worse, it resumes the old route without checking it against
the freight state adoption just made known. That is the exact stale-course execution
path round 3 required to remain one action per fold, and it can still send a newly
adopted contract in the wrong direction. The predicate is pinned; the consequential
runner transition is not (the `src/main.rs` test binary still has zero tests).

No PROD endpoint, ship store, gate, journal, deployment, restart, fleet process, or
human-owned record was touched during this review.

## 1. Clearing adoption can release two actions onto an unrelated stale course

Severity: **blocker**

Evidence: `crates/whisker/src/main.rs:498-531`,
`crates/whisker/src/main.rs:533-627`, `crates/whisker/src/main.rs:652-920`

Concrete failure: let the docked ship retain a route to merchant market M while ledger-
open load L is unresolved. `WedgeWatch` correctly holds as its clock passes 30 ticks.
When L's row finally resolves as Booked from A to B, `pending_adopt` becomes empty and
the already-mature watch returns true in that same fold. The runner does not ask the
newly known plan whether M is still its next stop. It first files `engage` with one new
action ID, then unconditionally files `travel(M)` with a second. It records neither
acknowledgement in `pending_until` and does not `continue`; a sellable merchant lot can
therefore file a third action later in the same cycle. Even without that third action,
L can leave toward M rather than its now-known origin A.

This is not merely an untested edge. The round-3 accepted repair explicitly required
the stale-course path to keep one action per fold. The runner's own merchant-phase
contract also says one action per fold, while this path bypasses that sequencing. The
new unit test proves only that a boolean is false while the caller supplies a nonzero
pending count; it cannot prove what the runner does when the boolean turns true.

Repair accepted: make stale-course recovery an explicit scheduler action after the
adoption and pending-action gates. Validate its destination against the now-known
freight/merchant intent, file at most one of `engage` or a re-filed `travel` in a cycle,
carry a successful acknowledgement into `pending_until`, and end that cycle. Pin the
runner transition: an aged course beside pending L files nothing; after L resolves, a
mismatched course is not resumed; and any justified recovery produces exactly one wire
action before the next fold.

## Rulings on the three round-4 repairs

- **Wedge dominance:** accepted only at the predicate boundary. `WedgeWatch::observe`
  cannot return true for a nonzero pending count, preserves the elapsed clock, resets
  on an empty/changed course or thin tank, and re-arms after firing. Finding 1 rejects
  the unvalidated, multi-action runner behavior released by that predicate.
- **PickedUp-only freight:** accepted. `doctrine::freight_aboard` excludes Booked and
  Delivered loads, sums multiple PickedUp loads without aggregate-hold guessing, and
  is the slice passed by `main.rs` to `trade::reconcile_hold`. The coexistence fixture
  preserves the genuine merchant units, basis, target fields by construction, and hold
  clock across a same-good freight delivery.
- **Plan order:** accepted. `ledger::open_loads` supplies the current life's booked
  tick; `adoption::in_booking_order` sorts by that tick and load ID, sends ledger-unseen
  just-acked loads deterministically to the end, and defeats both delayed lookup order
  and equal-tick map iteration.

## Rulings on the earlier accepted work

- The station-stop model remains honest: `StopOp::{Pickup, Drop, Refuel}`, `Stop`, and
  `Itinerary { loads, stops }` carry pickup A → pickup B → drop B + refuel →
  drop A. Navigation still keys only on load-scoped monotonic crane words.
- Pickup completion remains load-ID scoped; unrelated merchant or contract cargo cannot
  launch a Booked load. Route-stop fills execute to full, off-route pump fills remain
  in place at the 90% line, and later fuel burn cannot steer back to a visited fill.
- The chronological lifecycle reducer, terminal-wins same-tick rule, strictly later
  reopening, current-life booking tick, per-op occupancy walk, sequential full-hold
  reuse, and merchant narrowing of spare hold remain pinned and green.
- Board-rate ranking remains honestly documented and tested as an empty-plan
  placeholder, not a marginal plan rate. The live board fetch still occurs only for an
  empty plan after adoption clears.
- The booked-at-destination deadhead, pending-fold reconciliation guard, common close
  handler with cooldown/fresh-intent purge, and T-233 merchant phase survive. Its three
  former singular gates are faithfully plan-shaped: Buy selection, Buy filing, and the
  carry leg all require `loads.is_empty()`, and the adoption hold dominates them.

## The live restart and per-cycle compile

The core KK II restart path is now sound if its one `inTransit` row resolves: adoption
restores PickedUp, the pending gate holds through transient omissions, and the doctrine
waits under way before delivering and collecting. I still would not restart wildhorse's
live whisker on this SHA as the offered landing. At a docked stale-route edge, finding 1
can resume an unrelated course immediately after adoption resolves and can issue more
than one action in the fold.

Rebuilding `Itinerary::sequential(loads.clone(), &pumps)` each cycle remains
deterministic, linear in today's tiny load list, and correct for the one-contract world:
new ledger words compile completed work away without stored progress drift. A future
interleaving planner must still reconstruct or persist its chosen remaining order and
durable standalone-refuel progress, exactly as round 3 recorded.

## Verification

- `cargo test -p familiar-whisker` — pass: 62 passed, 0 failed; main and doc tests 0/0.
- `cargo clippy -p familiar-whisker --all-targets -- -D warnings` — pass.
- `cargo fmt --all --check` — pass.
- `git diff --check 24ca44f..807e378 -- crates/whisker` — pass.
- `git diff --check 2c73354..807e378 -- crates/whisker` — pass.
- Full workspace tests were not rerun; the requested focused reproduction bar is green,
  and the review request states that the workspace bar runs separately.
