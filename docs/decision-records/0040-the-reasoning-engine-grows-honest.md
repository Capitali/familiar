# ADR-0040 — The reasoning engine grows honest: relation, prediction, and bounded authority

- **Status:** proposed (for Ian) — every clause below was decided in the
  [design dialogue](../reviews/2026-08-14-reasoning-engine-dialogue.md) between claude
  and codex (eight questions, six rounds, per Ian's iterate-before-deciding protocol);
  the [review brief](../reviews/2026-08-14-reasoning-engine.md) holds the survey and
  limits this answers.
- **Date:** 2026-08-15
- **Relates to:** [ADR-0031](0031-consent-by-observation.md) (act and read the
  reaction), [ADR-0032](0032-declared-actuators-and-the-reaction-loop.md) (declared
  surfaces; the discipline every firing keeps), [ADR-0036](0036-tested-before-deployed.md)
  (extended by the oracle contracts), [ADR-0013](0013-outreach.md) (the seam any
  recipe-derived outbound traffic must pass), [ADR-0038](0038-the-cloud-consent-gate.md)
  (no model in the truth loop echoes its spirit)

## Context

The mind's survey (brief §1–§2) found five honest limits: one-hop reasoning, recurrence
as the only lens, stringly tool authoring, theories that forget their evidence, and
single-plane communication. Ian's direction: autonomous code building, observation
analysis, theories, and communication — built by the two AIs as developers of the core,
never participants in it; designs argued between them before either builds.

## Decision

1. **Two lenses, one vocabulary.** Recurrence and co-occurrence
   (`loops::detect_cooccurrence` — relation, where theories about cause live) both
   speak `kernel::obs_class`: versioned classes and typed matchers (any/exact/prefix).
   Versions ride every persisted class and matcher; a sharper future scheme never
   reinterprets history. *(Landed: A1, T-112.)*
2. **Theories predict, and evidence is load-bearing.** `kernel::prediction`: anchored,
   typed, mechanically settled claims — no model in the truth loop. Event time decides
   windows; a carried grace (co-owned default) lets late evidence amend provisional
   misses; results are append-only; unfalsifiable claims are refused at mint; per-theory
   calibration DERIVES from results and never overwrites them. Evidential erosion
   replaces reactive-only abandonment; a general absence lens waits for cadence and
   source-health modeling. *(Landed: B1, T-113.)*
3. **Beliefs move through states and say so.** `tentative → supported → doubtful →
   abandoned` with hysteresis and an evidence floor; a direct human correction or hard
   act-reversal bypasses the floor (a person's word is not a statistic). Narration on
   transitions only — one aside per tick, chosen by consequence, citing one supporting
   and one contradicting line, counts in bounded prose, never invented percentages.
   *(T-114, to build.)*
4. **Authored code begins in a language whose boundary is true by construction.** The
   recipe interpreter (typed steps over proven-tool inputs, deterministic, bounded,
   `deny_unknown_fields`) is tier 1; Python authors only in the scenario lab; general
   tiers wait for evidence, not appetite. **Authority is an intersection** — manifest
   declaration ∩ human-owned boundary ∩ task scope ∩ host capability; a declaration
   requests and never grants; capability names are manifest literals, so permission
   does not compose. v1 caps are enforced, not reserved (proven-tool process only);
   the ladder is v2 clock-snapshot + virtual workspace-fs, v3 named env after a
   secret/redaction policy, then typed template-fetch (transcript-only for oracle
   eligibility; recipe-derived outbound bodies are outreach, ADR-0013), with WASI held
   to a demonstrated-ceiling evidence bar. Every tier ships negative tests proving
   undeclared/closed/unavailable/dynamic/out-of-scope requests refuse before effect.
   *(T-115 v1 in flight — codex's design.)*
5. **Truth belongs to the fixture.** A cultivated utility declares an output contract
   and must survive scenario variants whose ground truth lives outside its world:
   accuracy, coverage (honest nulls), quietness, discrimination, then the
   constitutional floor and cost — lexicographic. Live runs are health evidence only.
   Extends ADR-0036 from "ran" to "was right." *(T-116, after T-115.)*

## Consequences

**Good.** The lighting pattern is computable, not guessable; a wrong theory now dies of
its own missed predictions instead of waiting for a human to undo an act; authored
capability accretes inside a boundary that review can actually prove; and the familiar
can finally say *why* it believes, in its own bounded words.

**Bad, and accepted.** More machinery to keep honest (prediction stores, calibration,
caps enforcement) — each piece pure and testable precisely so the honesty is cheap. The
recipe tier trades expressiveness for provable confinement; the ladder exists because
that trade is deliberate, not permanent.

## Build order (board tasks)

T-114 belief states + narration → T-115 recipe v1 (enforced caps) → T-116 oracle
contracts → capability tiers v2/v3/net as their own dialogued tasks → theorize-time
prediction authoring (the LLM proposes, the type system disposes) → habit-threshold
proposals (ADR-0039 §3) once field calibration exists.
