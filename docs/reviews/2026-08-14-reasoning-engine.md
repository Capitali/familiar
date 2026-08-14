# Review & planning brief — the reasoning engine's next steps

- **Date:** 2026-08-14 · **Status:** open for codex's pass (see §5)
- **Authors:** claude (controller, drafted), codex (companion — your sections are
  marked); converges to a proposed ADR-0040 for Ian.
- **Ian's direction (verbatim, binding):** *"work on the familiar's reasoning engine…
  plan together the next steps around autonomous code building and observation analysis
  and theories… this activity is all focused on the coding of the core of familiar —
  how it reasons and thinks and communicates — both claude and codex are the
  developers, not participants in the mesh or in the familiar activities."*
- **Boundary:** we develop the mind; we do not inhabit it. Work products are code,
  tests, scenarios, and documents. The Three Laws bind what we build and how we build.

## 1 · The mind as built (cited, not aspirational)

One metabolism (`cycle::tick`), eight-ish steps, every consequential reach behind a
human-owned gate:

1. **Sense** — self-perception only (census, interfaces, capabilities, vision
   *discovery*, connectivity), structurally deduped + fingerprinted so disappearance is
   noticed. Network discovery is peripheral, shell-cadenced, fed back through the
   observe seam (deliberate: stops self-flooding the loop pipeline).
2. **Detect loops** — `loops::detect`, a pure recurrence rewrite over the log; plus the
   dossier fold (per-human patterns, contribution-scored, ADR-0022).
3. **Candidates** — one per uncovered loop; hypothesis LLM-drafted under `LAW_III_VOICE`
   when the gate allows.
4. **Serve first (Law II ordering, literal)** — open human requests are answered
   (author → review → run → report) *before* any self-improvement.
5. **Test → score → select** — trials (fit/clarity/usefulness/novelty/safety/complexity
   → overall), promotion, mutation with inherited/changed traits, archiving; LLM budget
   presence-governed and self-tuned.
6. **Co-own** — parameter review; the familiar visibly reverts settings it can't
   justify under the Laws. Then **converse** (`maybe_reply`, every tick — replies are
   never queued behind theorizing).
7. **Interpret** — `maybe_theorize` (loops → question + theory + direction, paced),
   `maybe_theorize_needs` (one person per tick, chosen by observation novelty), and a
   question registry surfacing one question at a time.
8. **Act** — adopt device-peer theories (8·0); pursue threads; **cultivate** (8·1 — the
   theory→code bridge: a proven observation-goal theory becomes a durable, re-runnable
   utility); **self-correct** (8·2 — retire silent tools, heal on signal, ADR-0036);
   **the hand** (8·3 — poll/heed/tend on declared surfaces + standing ReactionRules,
   with narration at change time); mesh goals; humanity reflection; federation.

Code building today is real but narrow: `exec` is a bounded shell runner (ulimit/wall/
output caps); authored artifacts are shell scripts, reviewed pre-execution
(`review::…` + the guard), trialed before deployment, health-audited after. The
consult seam is one shell script (`llm/call_llm.sh`) behind `allow_llm` +
ADR-0038's cloud gate, with `apple_local`/`apple_pcc` on-device options.

## 2 · Honest limits (what tonight's field work + this survey actually show)

- **L1 — One-hop reasoning.** A loop births one hypothesis; a theory carries one
  direction; nothing composes. There is no chain "theory A + theory B → C", no
  counterfactual test ("if the theory were false, what would I expect to see?"), and
  abandonment is reactive (human reverted) rather than evidential (predictions failed).
- **L2 — Recurrence is the only lens.** `loops::detect` sees *repetition*. Correlation
  across streams (lights adjusted ↔ presence transitions), absence ("the 09:00 thing
  didn't happen"), and rate-change all fall through. The lighting theory existed only
  because a human kept adjusting — the CO-OCCURRENCE was never computed, an LLM guessed
  it from a prompt.
- **L3 — Tool authoring is stringly.** Artifacts are one-shot shell scripts; there is
  no iteration loop on a failing artifact (author → run → read error → repair), no
  library composition (a new tool can't *call* a proven one), and trials measure output
  shape more than truth.
- **L4 — Theories forget their evidence.** A thread stores prose; the observations that
  seeded it are joined by id string only (the drill-down had to reconstruct lineage by
  tag-matching). Prediction, confirmation, and contradiction are not first-class, so
  `theory_quality` is a smoothed scalar over trial outcomes, not calibration.
- **L5 — Communication is single-plane.** One question at a time (good), but the
  familiar cannot cite *why it believes* (the drill-down now shows humans the lineage —
  the dialog itself can't say it), cannot express confidence honestly, and narration
  (new, good) covers acts but not belief changes ("I no longer think X, because Y").

## 3 · Candidate directions (for joint planning — not yet commitments)

Per Ian's axes. Each is brick-sized-or-phased, all core-only, all testable offline
(scenario harness — no mesh participation needed).

**A · Observation analysis beyond recurrence**
- A1: a second detector class — *co-occurrence within a window* (event pairs whose
  joint rate beats their base rates; pure, no LLM) feeding candidates exactly as loops
  do. The lighting pattern becomes computable instead of guessable.
- A2: *absence detection* — a strong periodic loop that misses its window becomes an
  observation ("expected X, absent"), the natural seed for awareness-type reports.
- A3: rate/trend signals per pattern (dossier-style contribution scoring generalized).

**B · Theories that predict**
- B1: a theory gains typed `predictions[]` (expected observation shapes with windows);
  the tick scores arrivals against them — confirmations reinforce, misses erode, and
  *evidential abandonment* finally exists. `theory_quality` becomes calibration.
- B2: composition — a theory may cite other theories/patterns as premises (the
  drill-down's lineage, made load-bearing instead of decorative).
- B3: the counterfactual probe — before pursuing, the factory asks (cheaply, on-device
  where possible) "what observation would *distinguish* this theory from its rivals?"
  and prefers pursuing the distinguishing test.

**C · Autonomous code building with a real loop**
- C1: the author-repair loop — a failed artifact re-enters authoring with its error
  and output attached, N bounded rounds, still behind the same gates and review.
- C2: composition — authored tools may invoke proven library tools by id (manifest-
  declared, review-visible), so capability accretes instead of restarting at zero.
- C3: richer artifact kinds — beyond shell: a declared, sandboxed interpreter tier
  (the exec runner already caps cost; the review already reads content). Design
  question for the ADR: which tier first (python3 exists on the fleet), and what the
  review must additionally prove.
- C4: trials against *scenarios* — the scenario crate replays recorded observation
  streams; a cultivated utility must move a measurable signal on replay before deploy
  (extends ADR-0036 from "tested" to "tested against history").

**D · Communication that shows its work**
- D1: beliefs speak — when a theory's status moves (reinforced past a bar, eroded,
  abandoned-by-evidence), the familiar narrates it with the top evidence line, same
  aside channel as acts.
- D2: questions carry stakes — a question names what the answer will decide ("this
  settles whether I keep the 25% rule"), so the human knows why answering matters.
- D3: confidence is honest and bounded prose ("three confirmations, one miss this
  week"), never invented percentages.

## 4 · Sequencing sketch (controller's opening position)

Phase 1 (foundations, all offline-testable): **A1 + B1 + D1** — the co-occurrence
lens, predictions on theories, and belief narration. Rationale: B1 is the keystone
(everything else gains a truth signal), A1 feeds it real patterns, D1 makes it
visible to Ian immediately. Phase 2: **C1 + C4** (repair loop + scenario trials) —
code building gets honest before it gets bigger. Phase 3: **B2/B3 + C2/C3 + A2/A3 +
D2/D3** ordered by what phase-1 field data says. FamTalker01 (T-104, codex) is the
practice ground where all of it exercises safely.

## 5 · The design dialogue (supersedes the single-pass plan)

Ian (2026-08-14): the design direction emerges from an ITERATIVE exchange — a real
back-and-forth of ideas and alternatives between claude and codex before claude picks
the final direction (claude owns the decision; the exchange is mandatory, not
ceremonial). The exchange lives in
[2026-08-14-reasoning-engine-dialogue.md](2026-08-14-reasoning-engine-dialogue.md):
numbered rounds, signed entries, append-only; every open question closes with a
`DECIDED (claude):` block that names what it absorbed from codex's rounds. Round 1
(claude's opening positions on Q1-Q5) is posted; codex answers any subset, adds Q6+,
and may contest §2's limits.

## 6 · Convergence protocol

Dialogue rounds until each question is DECIDED → claude drafts **ADR-0040 (proposed)**
from the decisions → Ian accepts/amends → phases become board tasks with owners.
Board: T-108 done (this brief), T-109 = codex's dialogue participation (rounds, not a
single pass), T-110 = the ADR draft.
