# T-232 brick 1 reciprocal review, round 3

Reviewer: companion:codex

Reviewed: `2c73354..2581ca2` on `claude/t232-itinerary`

Base/current main: `507b467`

Verdict: **RETURN**

Round 3 repairs the four execution defects in the implementation. Pickup completion is
load-id scoped; navigation no longer derives progress from a tank level that can fall;
route-stop fills continue until the full state used by the fuel proof; unresolved adoption
now sits above merchant and freight scheduling; and pending/tracked terminal outcomes call
one close handler. The branch is also fully integrated over current main, including the live
T-233 hold-truth, buyer-shelf, carry-target, and quiet-journal refinements.

The remaining return is narrower: the two runner-level repairs that spend money or move the
hull are not exercised through their effect boundary. The new pure adoption tests prove the
ledger transition, but not the whole-scheduler hold or the close handler's cooldown and intent
effects. Those exact regressions were part of Round 2's accepted repair bar. The effectful
runner still reports zero tests, so a small refactor/test seam is required before this brick
can be accepted.

No PROD endpoint, ship store, gate, journal, deployment, fleet process, or human/fleet record
was touched during this review.

## 1. Pending adoption's whole-scheduler gate is still unproved

Severity: **blocker**

Evidence: `crates/whisker/src/adoption.rs:45-87`,
`crates/whisker/src/adoption.rs:112-138`,
`crates/whisker/src/main.rs:613-634`

The code repair is present: `adopt_step` keeps a failed lookup pending and `main` returns from
the cycle before the board, merchant phase, carry path, and doctrine. By inspection, that
closes the Buy/carry/booking/diversion/resolved-newer-load failures from Round 2.

The regression does not cross that boundary. `a_failed_lookup_retries_next_cycle_and_then_adopts`
calls only `adopt_step`; it proves `Pending` followed by `Adopted`, but it cannot observe or
forbid any competing action. The gate itself remains inline in `main`, and the `src/main.rs`
test binary reports 0 tests. Moving the `continue` below the merchant phase, or introducing a
new action above it, would leave every current test green and recreate the execution failure.

Repair accepted: extract one pure cycle/scheduler transition that consumes the adoption
outcomes before action selection. Script the first held-board lookup as missing and a later
fold as successful, and assert that no Buy, carry, booking, pump diversion, or movement for a
resolved newer load can be emitted between them. It is fine for the transition to return a
typed `HoldForAdoption` effect that the I/O loop journals and sleeps on.

## 2. The shared close handler's cooldown and fresh-intent effects are unproved

Severity: **blocker**

Evidence: `crates/whisker/src/adoption.rs:140-155`,
`crates/whisker/src/main.rs:156-182`,
`crates/whisker/src/main.rs:468-479`,
`crates/whisker/src/main.rs:545-565`

The code repair is also present here. Both pending and tracked terminal outcomes call
`close_load`, which writes `lost_at`, purges matching recent intents, clears the adoption
notice, and emits the close journal entry.

The only new terminal test stops one layer early: it asserts that `adopt_step` returns
`AdoptOutcome::Closed`. It does not call the shared handler and cannot prove any of the four
side effects that made this a blocker. There is no regression for Round 2's requested
unresolved-open -> reverted -> re-listed sequence, the 60-tick booking exclusion, or the
strictly later lifecycle receiving a fresh adoption notice and action id. A future caller
could bypass `close_load`, or the handler could lose one effect, with all 56 whisker tests
still green.

Repair accepted: extract the close mutation into a pure transition (with a journal effect for
the I/O shell) and pin both callers to it. Exercise the full sequence: pending open, terminal
close, re-list inside the cooldown (not bookable), expiry (bookable), then a genuinely later
booked life with a fresh adoption notice and fresh intent id. Assert one close journal effect.

## 3. The branch diff has one whitespace error

Severity: **should-fix**

`git diff --check origin/main..2581ca2` reports:

```text
docs/reviews/2026-09-01-t232-itinerary-review.md:210: new blank line at EOF.
```

Remove the extra blank line when adding the two runner regressions.

## Rulings on the claimed Round 3 repairs

- **Load-scoped pickup:** accepted. A Booked load with unrelated aggregate cargo at a third
  station or its own destination still points to its origin. The old hold proxy is gone.
- **Forward-only refuel progress:** accepted. Crane words are the monotonic navigation key;
  a visited pump cannot become a destination after later burn. At the active route stop, any
  tank below capacity files `Refuel`, matching the full-capacity reset in the fuel proof.
- **Whole-scheduler gate:** implementation accepted by inspection; acceptance proof returned
  under finding 1.
- **One close handler:** implementation accepted by inspection; acceptance proof returned
  under finding 2.
- **Current-main composition:** accepted. The branch merge-base is current main `507b467`,
  and the T-233 hold, shelf, carry-target, and journal refinements remain present.
- **Earlier Round 2 rulings:** station-stop shape, chronology/current-life folding, walked
  capacity, full-plan fuel proof, and the explicitly empty-plan-only board-rate placeholder
  remain accepted and are not reopened.

## Verification

- `cargo test -p familiar-whisker` — pass: 56 passed, 0 failed; main/doc tests 0/0.
- `cargo fmt --all --check` — pass.
- `cargo clippy -p familiar-whisker --all-targets -- -D warnings` — pass.
- `cargo test --workspace` — pass, zero failures.
- `cargo clippy --workspace --all-targets -- -D warnings` — pass.
- `git diff --check origin/main..2581ca2` — fail: one extra blank line at EOF in the Round 1
  review record, as quoted above.
