# T-231 brick 1 — independent re-verification (MacOnStick lane, codex paused)

Reviewer: companion:claude on MacOnStick — a different session from the one that built and
chair-reviewed the brick (wildhorse, `a8b1c1c`), reading the landed code cold against the
task's accept bar. Not codex; recorded so codex's own pass after Sept 6 starts from two
sets of eyes rather than one.

Reviewed on main: `ios/FamiliarMesh/Sources/FamiliarMesh/CandidateRace.swift`, its eight
pins in `CandidateRaceTests.swift`, and the race site in `ios/Shared/Sources/AppModel.swift`
(`raceWorldview`, the plan/settle/prune block).

Verdict: **ACCEPT.** The chair review's two repairs are real and the bar holds. Two notes
below, neither a blocker.

## What the accept bar asked, and what the code does

- *A cold launch with one dead remembered door reaches the mesh in about the fastest live
  candidate's time, not fastest-plus-a-timeout.* Every candidate starts after `rank × 350 ms`;
  the first `.win` cancels the group. A dead first door costs the second runner one stagger.
  A live preferred door still wins because its head start beats any rival's round trip.
- *The Device badge still names every candidate and its last cause.* Losses carry the same
  compact cause vocabulary (`h<status>:<body>`, `t:<message>`, `enc`, `dec`) into `attempts`.
- *No change to enrolment or pin semantics.* The plan only reorders and expires within the
  caller's `candidates`; the five-miss re-home hysteresis and the preferred-first restart are
  untouched (read, not assumed).

## Verified by inspection

- **A cancelled lap settles nothing.** Three paths return `.cancelled`: cancelled while
  sleeping out the stagger, cancelled mid-request (`NSURLErrorCancelled`), cancelled before
  the request. None reaches `settled`, so a winning door never demotes its slower rivals — the
  property the whole design rests on.
- **A loss is a real refusal**, not a slow door: only an error thrown by the read settles a
  failure. A door that is merely slower than the winner is cancelled, not penalised.
- **The planner is pure and pinned**: demotion after three straight misses (stable within
  tier), expiry after a week of silence with the lighthouse exempt and a never-answered door
  measured from its first attempt, one success revives — eight tests, all passing on main.
- **The chair's repairs hold**: `saveDoorHealth()` runs only when a race changed the map;
  the map is pruned to current candidates ∪ the lighthouse each round.

## Two notes (for codex's pass, not blockers)

1. **A second success is discarded as evidence.** After the winner, a later `.win` from
   another door is ignored and not settled — the chair review calls this deliberate. It is
   safe, but it throws away a fact: that door answered. Settling it as a success (without
   changing the winner) would revive a demoted-but-alive door one round sooner. Optional.
2. **One clock for every settle.** All outcomes settle at `raceNow`, the plan's timestamp,
   though a loss can land a second or two later. Harmless at the week-scale expiry; noted
   only so nobody later reads `lastAttempt` as a precise time.

## Runtime proof still owed

The chair review named it and it is still open: a cold launch on Ian's iPad with a poisoned
remembered door, timed. Nothing in this review substitutes for it.

## Verification

- `swift test --package-path ios/FamiliarMesh` on main (2026-09-04): all suites green,
  CandidateRace 8/8.
