# T-231 brick 1 — chair self-review (codex paused until Sept 6)

Reviewer: companion:claude, reviewing its own brick adversarially — recorded per the
T-229 precedent and Ian's 2026-09-02 word ("stop using codex as coding partner till
sept 6th … lets finish that ourselves"). Explicitly NOT a substitute for codex's
independent eyes; re-verification owed after the pause.

Reviewed: `claude/t231-candidate-race` (brick at `a2e0f8f`, merged over current main),
against the task's accept bar and the pure pins in `CandidateRaceTests`.

Verdict: **ACCEPT-WITH-REPAIRS — both repairs applied in this land.**

## What held under attack

- **The race's semantics.** First success wins; losers are CANCELLED and a cancelled
  lap settles no health (a winning door must not demote its rivals) — pinned at the
  planner level, verified by inspection at the TaskGroup level (`.cancelled` returns
  before any settle). A second success arriving after the winner is ignored and
  deliberately unsettled either way.
- **Preference stays preference.** The stagger gives the doctrine's order a 350 ms
  head start per rank; the five-miss hysteresis is untouched and still the only
  thing that re-homes the console; every-round-restarts-at-preferred is untouched.
- **Diagnostics survive.** All-fail keeps the per-host cause vocabulary on the
  Device screen (`attemptLog`), and the no-grant early return keeps its exact error.
- **The planner's edges.** Lighthouse never expires; a never-answered door expires
  only after a week of actual attempts; one success revives a demoted door — all
  pinned (8 tests).

## The two findings (fixed before landing)

1. **Defaults churn.** `saveDoorHealth()` ran on every read cycle — a UserDefaults
   write every ~3 s forever. Now written only when a race changed the map.
2. **Unbounded health growth.** Hosts that left the candidate list kept their
   health rows forever; years of roaming would grow the map without bound. Now
   pruned to current candidates ∪ the lighthouse each round (history for a door
   that returns is deliberately forfeit — it re-earns its rank fresh).

## Left open, named for codex's later pass

- The race runs on every read, not just launch (deliberate — mid-session failover
  gets the same cheapness); worth codex's independent judgment on poll-cadence
  network cost against N candidates when several doors are live-but-slow.
- The acceptance evidence the task names — a cold launch on the iPad with a
  poisoned remembered door reaching the mesh in lighthouse-RTT plus one stagger —
  is runtime proof to collect on Ian's device after this merges.

## Verification

- FamiliarMesh `swift test`: 48/0 (8 CandidateRace pins included), after the repairs.
- `xcodegen`; FamiliarMac Release build; FamiliarAgent iOS-simulator build
  (`-destination` only) — counts in the merge commit.
