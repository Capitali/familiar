# ADR-0034 — The Changeling, the keeper, and the promise it can prove

- **Status:** accepted — implemented 2026-08-09
- **Relates to:** [ADR-0028](0028-the-mesh-games.md) (the law of the fire — all of it still
  holds), [ADR-0029](0029-the-door-under-load.md) (§1 never park a worker, §2 temp+rename,
  §4 no seesaw), [ADR-0030](0030-the-ember-reaches-a-locked-phone.md) (the push),
  `crates/mesh/src/game.rs`, `crates/mesh/src/changeling.rs`

## The game

Each round one human is the **witness**: they write one true, campfire-sized line about
their day. The familiar forges **two changelings** — plausible false lines in the same
voice — shuffles the three, and everyone else votes for the human one. Find the truth:
+1. Slip a changeling past a voter: +1 to the witness. The familiar holds no seat and
scores nothing — it plays to lose gracefully. As many rounds as players; highest score
takes it; a tie keeps the mesh's counsel.

**Solo — "two never happened":** the familiar witnesses about the mesh's own record.
Three lines in the fixed register *"the mesh saw …"* — one drawn from a real, shareable
observation, two forged — and the lone human votes. Catch it lying about its own record
in at least two of three rounds and the changelings failed. This is audit-through-play:
the game is only winnable by a mesh whose record the human actually knows.

## What this game tests (the ADR-0028 doctrine: testers playing IS the test)

1. **The LLM seam, with a deterministic floor.** The forge prefers the model and never
   depends on it: refused, rate-limited, absent, or nonsense all fall back to seeded
   banks. A dead LLM cannot kill a live fire — and CI plays entirely on the floor.
2. **Secret-keeping beyond the riddle.** The riddle's answers ride in replicated state
   and are merely stripped from the view — acceptable there, cheating-only. Here the
   secret decides scoring, and replicated state is readable by anyone at the port
   (`GET /mesh/records` is unauthenticated). So **the truth's index never enters state
   at all.**
3. **The keeper and its commitment.** The door that receives the witness's line becomes
   the round's keeper. It holds `{game_id, round, truth_idx, salt}` in a door-local file
   (written temp+rename *before* anything publishes) and puts only
   `sha256("{id}|{round}|{salt}|{truth_idx}")` into state. A salted commitment is
   essential, and the salt must stay door-local until the reveal: over a 3-value domain,
   a commitment with its salt beside it is three hashes from naked. At the reveal the
   salt and index publish, and **any door — or any console — can check the promise.**
4. **Ballots as signed member acts** converging across doors; votes are public in state
   (they are not the secret; consoles hide choices until the reveal cosmetically).
5. **The lazy clock, now with phases.** witness → forging → voting → reveal-wait, each
   with its own expiry, all pure over (state, clock) so every door decides identically.

## The clocks, and who they blame

| Phase | Expiry | Consequence |
|---|---|---|
| witness (15 min) | the witness was silent | a strike; the round passes to the next witness — a silent witness loses the round, not the game |
| forging (3 min) | the door was slow | **no strike, ever — the door failed, not a human.** Multiplayer: back to the witness ("the forge went cold — speak your line again"); solo: the claim re-opens |
| voting (15 min per holder) | a voter was silent | the clock casts ABSTAIN for them, plus a strike; abstention scores no one |
| reveal-wait (15 min) | the keeper was silent | **any door voids the round** — no scores, the fire moves on. A crashed keeper cannot wedge the game |

The holder rotates with the phase (witness, then each un-voted voter), so the ember
badge, the countdown, the APNs push, and the strike machinery all work unchanged.

## The upgrade law (read before lighting)

`GameKind` gained `Unknown #[serde(other)]`, so **future** kinds can never again break an
old door's whole RecordSync parse. But doors older than THIS build lack that protection:
a changeling in a sync makes their entire record-sync body unparseable — membership
records stop syncing while the game burns. **Every door — wildhorse, the lighthouse,
every TestFlight device door — must run this build before the first changeling is lit.**
Shipping the code is safe; lighting early is not.

## Consequences

**Good.** The familiar steps from referee to *participant* for the first time, safely:
its moves are the forged lines, anonymous, scoreless, and bounded by a deterministic
floor. The mesh gains a game two people and a familiar can actually play — and a solo
game that doubles as an honesty audit of the record.

**Bad, and accepted.**
- The keeper is a single point of round-failure by design (the alternative is the secret
  existing in more than one place). The void clock bounds the damage to one round.
- Forged lines from the fallback banks repeat across games eventually; a household that
  plays nightly will learn them. The banks are the floor, not the ceiling.
- The commitment proves the keeper didn't move the truth after the votes; it cannot
  prove the *forgeries* were machine-made. The witness's honesty ("write one TRUE line")
  is, as at every fire, the human's own coin.
