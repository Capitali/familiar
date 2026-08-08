# ADR-0031 — Consent by observation: the query is the final gate, not the first

- **Status:** accepted (Ian, 2026-08-08) — first slice implemented the same day (the
  dossier, needs theorizing, Law I question routing)
- **Relates to:** [ADR-0022](0022-the-human-dossier.md) (the dossier — the substrate this
  reads from), [ADR-0016](0016-multi-human-served-identity.md) (per-human attribution),
  [ADR-0019](0019-friendly-identification.md) (routing), `docs/SOUL.md` (Law III),
  `docs/law-iii-responses.md`

## Context

The familiar theorizes, and until now its theories ended in a question: it would ask, and
wait. That shape makes the human the familiar's scheduler — every act of service billed
in advance against their attention, which is exactly the coin Law I says service is
priced in. A companion that must ask before every kindness is a permission-asking
appliance.

Ian's framing (2026-08-08), which is the decision:

> The eventual goal is for many of the decisions, if not most of the decisions, the
> familiar will make will be "consent by observation" — that enough observational data
> would be available to determine the user's need autonomously. Interacting with the user
> to confirm needs should eventually become the final gate, not an upfront one. The
> familiar should observe, theorize, and then act — environmental controls like HVAC or
> lighting, or automated cleaning devices. It should identify that they exist, understand
> how to control them, understand how the user interacts with them through observation
> (always turns heat down in the morning and up in the evening, or likes lights at 30%
> after 8pm), and then begin making those changes for the user, and observing their
> reaction. If it's positive the change remains; if negative the change is undone, and
> more observation — including a query to the user — might be appropriate before trying
> those control surfaces again.

## Decision

**For reversible, low-stakes service, the familiar acts on its theory and reads the
human's reaction as the consent signal. The direct question is the escalation path —
the gentlest rung after a negative or ambiguous reaction — never an upfront gate.**

The loop: observe → theorize → act → read the reaction.

- **Positive reaction** (or none): the change remains, and the pattern that produced it
  gains weight (ADR-0022 contribution scoring).
- **Negative reaction**: the change is undone, its contribution is *depreciated* — not
  deleted; a wrong guess still teaches — and the familiar returns to observation. Only
  then, and only if it still believes the need is real, does it ask.

## Why this is Law III-compatible, not a violation of it

Law III (service is not obedience) cuts both ways: the familiar must not be commanded
into harm, and the human must not be governed by their own tool. Consent-by-observation
respects both *because of its preconditions*, which are the load-bearing part:

1. **Reversibility is the license to act.** An action qualifies only if undoing it
   restores the world. Anything irreversible, outward-facing, or person-affecting keeps
   its explicit gate (`guard.rs`: `affects_person`, `irreversible`; the boundary's
   capability gates are untouched by this ADR).
2. **The reaction is honored immediately and mechanically.** A dismissal, a revert, a
   "no" — each is depreciation applied without argument. A system that acts and then
   negotiates about undoing has become coercive; this one undoes first.
3. **The human's words always outrank the theory.** A stated need (thread origin
   `"observer"`) supersedes a theorized one; a person's answer flips the record to their
   own words (`thread::add_answer_from`), and a withdrawal (`familiar dossier withdraw`)
   ends theorizing about them entirely.

## What is implemented now (slice 1)

The act→react loop already exists structurally — thread → candidate → trial →
`selection::decide` — so this slice laid the rails that make its subject a *person*:

- A theorized need is a thread that **names its human** (`Thread.origin_human`) and is
  **pursued immediately** — no confirm gate in `pursue_threads`.
- The confirm-question (`Question.subject`, origin `"need"`) files alongside the pursuit
  as one evidence channel among several. It waits for its person up to a week (held,
  never buried) and its dismissal rests it, exactly like every other question.
- Personal-need threads never federate (`merge.rs`): only the node that knows the human
  can read their reaction, so the theory isn't delegatable work anyway.

## What is deliberately not yet built

- **Real actuators.** No control surface is wired; "act" today means the internal
  trial machinery. The first candidate is deliberately modest and reversible (the
  BLE-controlled light strip — local, low-stakes, observable reaction).
- **Reaction observations scored into trials.** "The human turned it back" as trial
  evidence, and automatic revert on a negative reaction, land with the first actuator —
  building the revert path *with* the actuator, never after it.
- **Habit patterns** (`ctb|<handle>|habit|<surface>@h<hour>`) — the dossier kind that
  learns "lights at 30% after 8pm". The slot grammar anticipates it; nothing writes it
  yet.

## Consequences

**Good.** The familiar can be quietly useful — the difference between a companion and a
notification engine. Attention is spent only where observation genuinely cannot answer.

**Bad, and accepted.** A wrong theory now produces a wrong *act*, not just a wrong
question. The mitigations are the preconditions above, and the honest bound is stated
plainly: this model is only as safe as the reversibility judgment and the
reaction-reading are good. Both must stay conservative — when in doubt, an action is
irreversible, and a reaction is negative.
