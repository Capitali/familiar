# T-229 factory — chair self-review (codex rate-limited)

Reviewer: companion:claude (chair), taking codex's review load while it is
rate-limited (Ian: "If codex hit its limit take its load and continue without
it for now", 2026-08-28).

**This is self-review, not independent review.** It is done adversarially — I
attacked my own code from codex's perspective — but it does not replace codex's
independent eyes. Everything below stays open to codex's re-review when its
credits reset (~16:36); this doc records what I checked and the two residuals I
found and fixed so the record is honest.

## Scope

The bricks on main after codex's last RETURN: workshop blocker-5/6 repairs,
the jail rebuild (blockers 7/8), the BLE broker, and the new factory run
adapter (brick 5).

## What I attacked, and what held

- **Ledger pid-lease lock (blocker 6 repair).** Attacked: two racers both
  `create_new` → only one wins (atomic), holds. A reclaimer removes a dead
  lock then another creates → `create_new` arbitrates, holds. A live holder
  paused arbitrarily long → its pid reads alive, never stolen, holds.
- **Jail symlink escape.** Attacked: a symlink in scratch pointing at
  `~/.ssh` — sandbox matches the *physical* resolved path, which is denied;
  a symlink whose target is outside scratch for a write — physical path
  outside the one writable subpath, denied. Holds.
- **Jail output flood / fork bomb.** The concurrent drain discards beyond the
  cap and kills the group; the fork fan-out is killed as a group at the wall
  bound. Pinned, holds.
- **Broker.** No UUID field in the protocol (structural); op fixed per
  session; read rung refuses transmit; all caps enforced; refuses 0 or >1
  match. Pinned, holds.
- **Materialize.** Digest re-verification catches a content store returning
  wrong bytes; traversal re-checked before writing. Pinned, holds.

## Two residuals I found and fixed

1. **Empty-lock wedge (ledger).** A holder crashing *between* `create_new`
   and writing its pid would leave an empty lock that `owner_is_dead` treated
   as alive forever, deadlocking the ledger. Fixed: an *unidentifiable*
   (empty/garbled) lock is reclaimable after `LOCK_ORPHAN_SECS` (120s). This
   does **not** reintroduce codex's rejected flaw — that stole from
   *identifiable, live* holders on a timer; a live holder always writes a
   readable pid that reads as alive, and only unidentifiable locks (no
   live-holder claim) are timed out. Committed `f7e5354`.

2. **Repo-path coverage (jail).** `mandatory_hidden_roots` hardcodes the two
   known repo homes (`Projects/familiar`, `Development/familiar`). A repo
   checked out elsewhere would not be auto-denied. The data dir and key dirs
   (the real secrets) are covered regardless; `run_bench` takes `extra_hidden`
   so the daemon passes its actual repo + data dir explicitly. Noted, not a
   secret-exposure for order #1 (the repo is public; the data dir holds the
   secrets and is denied). Left for codex to weigh.

## For codex on return

Please independently re-review: the six blocker repairs (workshop 5/6, jail
7/8), the two residual fixes above, the broker against your ruling, and the
factory run adapter (brick 5). And confirm the jail's mandatory-denylist is an
acceptable floor given the macOS-27 dyld constraint (a true read-allowlist
needs a static/containerized runtime — recorded follow-up), or direct the
container path.

Bar at review time: fmt clean; clippy --workspace 0; workspace 875/0; jail
8/0; workshop 33/0; factory 7/0; broker 14/14 (python).
