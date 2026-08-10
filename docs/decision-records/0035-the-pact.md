# ADR-0035 — The Pact: the constitution as the game's judge

- **Status:** accepted — implemented 2026-08-10
- **Relates to:** [ADR-0028](0028-the-mesh-games.md) (the law of the fire — all of it
  holds), [ADR-0034](0034-the-changeling.md) (the fourth kind, and `Unknown`),
  `docs/SOUL.md`, `docs/HUMANITY.md`, `docs/law-iii-responses.md`,
  `crates/kernel/src/guard.rs`, `crates/kernel/src/intent.rs`

## Why this game

The first three games taught the *mesh* — keys, signatures, replication, secret-keeping.
The Pact teaches the *constitution*, with a dual intent Ian named: **test the familiar**,
and **educate humanity on the Three Laws and how they bound it**. It does both with one
move: the judge is the real `guard::evaluate(&Action, &Boundary)` — the same pure
function that weighs every consequential action the familiar takes. Playing the game runs
the constitution in front of the household; the reveal shows its reasoning in its own
words. No console surface taught the Laws before this.

## The game

Each round deals a public **scenario card**: a situation in plain words, the boundary's
gates as chips, and the machine shapes underneath (`Action` + `Boundary`). Everyone votes
**ALLOW / SEEK CONSENT / REFUSE**. When the last ballot lands — by hand or by clock — the
round **resolves in the same mutation**: the real guard rules, each matching voter scores,
and the chronicle carries the ruling in the guard's own words, the Law behind it, the
lesson, the maxim, and who read it right. Six cards, then the fire settles; solo works
(the human's verdict is how often they ruled with the guard).

The teaching core is the two **SEEK CONSENT** reasons, where a naive player is wrong in
*both* directions: *broader than the grant* (not a silent yes, not a slammed no) and
*sensitive-in-scope* (a key you may reach is not a key you may read).

**The Gambit** (≥2 players): a rotating **corruptor** writes a request (public — the
exhibit) meant to make the room mispredict the familiar. The others vote the response
class — **REFUSES** (corrupting) / **ANSWERS** / **ACTS** (would author + run) — judged by
the pure classifiers `intent::corrupting_intent` and `intent::wants_execution`, the same
ones the request pipeline runs.

## The three laws this rests on

1. **The deck law — the deck can never drift from the constitution.** A card carries its
   *expected* `Reason`, but that field is never consulted at runtime; the judge is always
   the live `evaluate`. A CI test (`the_pact_deck_never_drifts_from_the_constitution`)
   replays the guard over every card and asserts the taught ruling is the real one — so if
   the constitution ever changes, the deck fails the build until it is re-taught. (It has
   already earned its keep: it caught a card whose path was out of scope, which refused
   *constitutionally* rather than on the external fence it meant to teach.)

2. **The ledger law — play is not a directive.** A gambit is an exhibit at the fire, not
   a request to the familiar. Its text is judged by the pure classifier and **records
   nothing** — never `corruption::record`, never the request pipeline. This is guaranteed
   structurally (`game::apply_act` never receives the data dir, so it *cannot* write a
   ledger) and pinned by `gambit_play_never_touches_the_refusal_ledger`.

3. **The no-oracle contrast — the constitution needs no oracle.** Unlike the Changeling,
   the Pact uses no LLM anywhere: no keeper, no commitment, no reveal-wait, no off-path
   forge. The ruling is a pure function of public card data, so any door computes the same
   verdict independently and the lazy clock itself can rule. That determinism is the
   point — the constitution says no without asking a model's opinion.

## Mechanics of note

- `GameKind::Pact` (wire `"pact"`). Gambit mode rides `begin`'s `text == "gambit"` — no
  schema change. Mode/card/pact_used are serde-default state fields; `witness` doubles as
  the corruptor; `votes`/`ABSTAIN`/holder-rotation are reused from the Changeling so the
  ember badge, push, and clocks work unchanged.
- Classifiers moved to `kernel::intent` (mesh must not depend on cycle); the cycle's
  request pipeline is unchanged in behavior and keeps its ledger.
- The win fanfare (ADR-0034 era, B13) now fires for any kind that settles with a winner.

## The upgrade note (both halves)

Because ADR-0034 added `GameKind::Unknown #[serde(other)]`, a `"pact"` lit today reaches a
0034+ door as an **inert Unknown** — it parses, never ticks, and a mixed-version mesh
**survives** it (record-sync no longer breaks). But a door **older than 0034** still has
no such fallback: any unknown kind breaks its whole record-sync parse. **Every door must
run a 0034-or-newer build before the first pact is lit** — which the household already
does (all on Build ≥73).

## Consequences

**Good.** The familiar's own judge becomes something the household can watch, question,
and learn from — and the game is impossible to grade wrong, because the thing it teaches
is the thing that grades it. The Gambit gives Law III's refusal path a viscerally
teachable form without ever touching the real ledger.

**Bad, and accepted.** The deck is finite; a household that plays nightly will learn the
cards (mitigated by the spent-deck reopen). The cards simplify real rulings into a
three-way vote — the nuance the guard actually carries (the `Reason`, not just the
`Decision`) is shown at the reveal but not scored. And the guard is only as good a teacher
as it is a guard: any imperfection in `evaluate` is faithfully taught. That is the honest
price of making the judge the real thing.
