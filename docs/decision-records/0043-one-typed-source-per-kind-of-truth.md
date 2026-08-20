# ADR-0043 — One typed source per kind of truth

- **Status:** accepted (Ian, 2026-08-20 — "Build q1-q4 and the rest. Go!" on the conduct
  dialogue's decided design; this ADR is the closure brick 6 of that plan owed)
- **Date:** 2026-08-20
- **Relates to:** [ADR-0040](0040-the-reasoning-engine-grows-honest.md) (D2 closed by
  brick 3 — questions carry stakes; D3/D4 completed here), ADR-0037 §1 (the persona seam,
  built in brick 1), [ADR-0035] (the game exclusion, held structurally),
  docs/reviews/2026-08-20-conduct-dialogue.md (the dialogue whose DECIDED blocks this
  ADR records durably)

## Context

Asked to state its own Three Laws, the familiar recited Asimov's. The Laws were never
edited — `docs/SOUL.md` was simply never read at runtime, and the reply path was checked
only for whether its output looked like prose. Tracing that single wrong answer exposed a
defect *class*, not an instance: the narration layer had drifted free of the enforcement
layer, and nothing could detect it. Two answering pipelines existed and shared almost
nothing; the grounded one had no producer; the conversational one had no grounds; the
dialogue was a closed loop that wrote only to itself; and roughly half the built
state-writing surface produced no data while its documents said "implemented."

T-136 and ADR-0040's D4 asked for the epistemic rule and never got it written down. This
is that document.

## Decision

**1. One typed source per kind of truth. Renderings and documents are views — never
sibling sources.**
The constitution's runtime source is `kernel/constitution.rs`, drift-tested against
`docs/SOUL.md`; every prompt, screen, and answer renders a view of it. The system-truth
source is `system_facts::view()`; `render_for_answering` and the theorize floor render
views of it. A second assembly of the same truth is the defect (`LAW_III_VOICE` was one;
`answer_requests`' hand-written Law III paraphrase was another; both are gone).

**2. Law text is unauthorable.** The model cites a Law by id; the kernel splices the
canonical words (brick 2). A model-authored paraphrase of a Law cannot reach a human —
contradiction is unrepresentable rather than detected, which is why no prose validator
exists and none may be added (standing ruling, 2026-08-15).

**3. One road for a human's words.** The dialogue path is THE answering pipeline:
screened (`corrupting_intent`, refusing without writing the corruption ledger — Ian's
ruling), floor-grounded (the registry leads the prompt), typed (`ReplyDraft`, nine shape
checks, no judgements), and durable (`persist_exchange` writes the `Request`/`Answer`
pair with exactly the admitted confidence and cites). `answer_requests` is retired;
`fetch_and_answer` is removed — fetched material re-enters only through the same floor,
screen, and admission path, with provenance and bounds, or not at all.

**4. Own speech is never its own witness.** A `familiar/{replied,refused,asked}` row is
excluded from theory subjects and theory evidence alike; it *dereferences* — the
observations its admitted cites name become eligible again, however old. Invariant, held
by test: no chain composed solely of the familiar's own speech can raise confidence in
any world claim.

**5. Kinds of truth have kinds of addressee.** An answer addresses the asker. A theory
about the household addresses the household's humans, as a staked question. A theory
about the familiar's own machinery addresses the maintainers — it becomes a typed
`MachineryFinding` (T-218) routed to a human-visible development inbox, never a
household question and never task authority. An offering to a partner AI (T-216)
addresses a covenanted stranger, and carries capability without carrying the household.

**6. A truth-bearing type is incomplete until it has both a producer and a declared
addressee/consumer — and every terminal status names who can cause the transition.**
A write function whose only callers are tests is not a capability; a queue with no
producer is not a reserve organ; a `pursued` that nothing can surface is a defect of the
same class as the Asimov recital. T-212's structural test enforces the first half;
review enforces the second until it can be typed.

**7. The residual law-quotation gap is labelled, not guessed shut** (Ian, 2026-08-20:
regressions, no detector). A draft may quote foreign law inside `say` uncited and pass
every shape check; the regression
`foreign_law_in_say_without_cites_is_the_labelled_residual_gap` pins exactly that, and
render lends no constitutional heading to uncited prose. If the field ever shows a model
doing it, the structural close is: any claim presented as a governing Law requires a
canonical Law cite — string identity against SOUL.md's fixed text, decidable, never a
judge of prose.

## Consequences

- `admission::check_cites` is the one admission function: the reply act and the theorize
  anchors both pass through it (T-135's composition, done); `TheoryDraft` and `AskDraft`
  are its citizens. `lexical_guard` retires when its last prose caller (the needs muse)
  speaks a typed draft — recorded here so the retirement has an owner condition rather
  than a hope.
- Questions carry stakes (`because` / `turns_on` / `stake ∈ {continues, changes, stops}`
  — deliberately no `none`): ADR-0040's D2 is closed by brick 3.
- The activity feed counts what was durably answered (the persisted nouns), not what a
  queue held.
- T-210 closes against this ADR when its device-shell half lands (one Laws source for
  daemon and shells). T-211 closes with it: the two organisms are one, by construction.

## What this ADR deliberately does not do

No new validator judges prose. No gate opens. The corruption ledger gains no new writers.
The muse's observation window is untouched — the dereference is the only door own speech
has, and it opens onto evidence, never onto itself.
