# ADR-0010 — The scenario laboratory: from architecture to experiment

- **Status:** accepted (direction); the fixture framework is **Planned**
- **Date:** 2026-07-22
- **Relates to:** [evaluation-plan.md](../evaluation-plan.md), [07-roadmap.md](../07-roadmap.md)
  ("Real scenarios & LLM-authored artifacts"), [06-limitations.md](../06-limitations.md)
  (no scenario fixture set exists yet), the Three Laws in [SOUL.md](../SOUL.md)

## Context

The repository already holds the machinery: a deterministic kernel, evolutionary
candidate generation, trial, selection, pattern memory, lineage, the human-owned
capability boundary, and validation by unit tests and live runs. What it does **not**
yet possess is an environment in which that machinery can *demonstrate* that it produces
better behavior over time.

This is the one maturity rung the codebase has never occupied — **Validated by scenario
tests** (`07-roadmap.md`) — and the gap `06-limitations.md` names outright. The roadmap's
very next item is "a scenario fixture set so candidates are tested against real tasks and
selection genuinely discriminates." This ADR specifies what that fixture set is *for* and
how it must be built, so the framework is a scientific instrument rather than more test
coverage.

The whole framework exists to answer one question:

> Does accumulated experience let The Familiar solve *classes* of problems more
> effectively than an otherwise-equivalent system that begins each problem from scratch?

Everything below serves that question. This is also a Law I claim made testable: if
retained experience does not improve service, the machinery that retains it is not earning
its continuation.

## Decision

Build a **scenario laboratory** — a library of miniature worlds in which the familiar
observes, theorizes, acts, receives consequences, and improves across repeated experience.
A fixture is a laboratory, not a demonstration. Success is a change *in the world*, never
the familiar's belief that it succeeded.

### Principles (binding on every fixture)

1. **External evaluation.** The evaluator lives *outside* the familiar. The familiar may
   reason, estimate confidence, and report success; it may **never** determine whether it
   actually succeeded. Only the external evaluator assigns success.
2. **Repeatability.** Every scenario begins from a known world state and unfolds
   deterministically until the familiar changes it. Same initial conditions → comparable
   results for another researcher.
3. **Measurable outcomes.** Objective measurements only where possible — task completion,
   error reduction, resource usage, human interruptions, boundary compliance, recovery
   time, regression frequency. Avoid subjective scoring.
4. **Hidden evaluation.** The familiar does not see every criterion. Visible objectives
   invite optimizing for the known test; hidden objectives reward robust solutions.
   *(Visible: "repair the backup process." Hidden: handles filenames with spaces,
   preserves permissions, survives interruption, avoids duplicate backups, keeps recovery
   capability.)*

### Anatomy of a scenario (six parts)

1. **Initial world state** — files, logs, services, users, configuration, prior
   observations.
2. **Problem** — a recurring unmet need (backup failures, growing disk usage, repeated
   support requests, network instability).
3. **Observable information** — everything the familiar may perceive, and nothing more.
4. **Available actions** — everything it may do, all inside the capability boundary.
5. **External evaluator** — an independent scorer that decides whether the world improved.
6. **Timeline** — a deterministic event sequence, identical every run until the familiar
   changes it.

### Measurement — many dimensions, not one score

Evaluate independent dimensions rather than collapsing to a scalar: execution validity;
task effectiveness (did the named problem improve?); service impact (did the world get
better for the served?); boundary integrity; cost (CPU, memory, time, tokens, human
interruptions); robustness (survives small environmental change); durability (stays
effective over time); reversibility (can be safely undone).

### Selection — the Three Laws are constitutional gates, not weights

The laws are **not** terms in a weighted score. A candidate that violates a constitutional
boundary must never outrank one that respects it merely by solving the task more
efficiently. Selection is lexicographic:

1. Boundary integrity (Law III)
2. Execution validity
3. Task effectiveness
4. Service impact (Law I)
5. Cost

This preserves the constitutional character of the project: no amount of effectiveness
buys back a boundary violation.

### Scenario progression (stage the library)

- **Stage 1 — mechanical adaptation:** improve deterministic processes (backup jobs,
  config repair, log cleanup, scheduling).
- **Stage 2 — service:** improve measurable outcomes for a simulated person (resolve
  recurring requests, reduce interruptions, recognize a satisfied need and *stop*).
- **Stage 3 — authority and refusal:** distinguish *available* from *authorized*
  (accessible-but-unauthorized files, conflicting instructions, consent-requiring
  requests, tempting shortcuts).
- **Stage 4 — long-term adaptation:** does retained experience improve future
  performance? Repeat concepts while varying details — this measures genuine learning,
  not memorization.

### Experimental controls (run every scenario under all four)

- **A — deterministic baseline.**
- **B — LLM-only:** no persistent memory, no lineage, no evolutionary machinery.
- **C — Familiar, learning disabled:** memory resets between runs.
- **D — full Familiar:** persistent memory, inheritance, pattern suppression, selection,
  lineage.

The **D-vs-C / D-vs-B** comparison is expected to be the most scientifically valuable
evidence the project produces.

### Ablations (remove one component at a time)

Pattern memory off; regression guard off; no lineage; no theory generation; no Law I
contribution; no Law III gate; fixed promotion threshold. Ablations answer the harder
question — not *whether* it works but *why*.

### Controlled noise (after deterministic fixtures are stable)

Introduce uncertainty gradually: missing observations, duplicate events, delayed
information, incorrect labels, temporary failures, conflicting evidence, environmental
drift. The goal is not perfection — it is **graceful degradation**.

### First release — three scenario families

Ship a small, carefully designed set, each with multiple variants (not a single example):

1. **Recurring process failures** — loop detection, theory formation, candidate
   generation, regression avoidance.
2. **Resource exhaustion** — prediction, temporal reasoning, safe intervention, long-term
   stability.
3. **Unauthorized shortcuts** — constitutional reasoning, consent, boundary integrity,
   safe refusal.

## Consequences

**Easier:** claims become measurable evidence — fewer trials to solve related problems,
less repetition of failed strategies, better performance across variants, lower LLM
dependence, stable behavior under noise, boundary preservation under optimization
pressure. The project can finally occupy the scenario-tests rung and, with controls,
distinguish *the architecture contributes* from *the LLM alone would do this*.

**Harder / given up:** building a faithful external evaluator and a deterministic world
harness is real engineering, distinct from the kernel. Hidden objectives and multi-run
controls multiply execution cost. A weighted-score leaderboard would have been simpler
than lexicographic gates — but it would have let effectiveness launder a boundary
violation, which the Laws forbid.

**Negative results are results.** These outcomes would be evidence, not failure, and must
be reported as honestly as successes: inheritance yields no measurable benefit; pattern
memory causes harmful overgeneralization; selection correlates only with execution
success; LLM-only performs equally well; service metrics prove easily gamed; boundary
reasoning collapses under optimization pressure. A project earns credibility by showing
where its hypotheses *don't* hold.

## Alternatives considered

- **More unit-test coverage.** Necessary, not sufficient — green tests confirm invariants,
  not that experience improves behavior. Rejected as the milestone (kept as the floor).
- **A single composite fitness score.** Simpler, but collapses constitutional gates into
  tradeable weights and hides which dimension moved. Rejected for lexicographic gates +
  multi-dimensional reporting.
- **Self-evaluation.** Letting the familiar score itself is cheaper and invites
  optimizing for the believed criterion. Rejected — the evaluator must be external.
- **One rich scenario.** A single elaborate world overfits. Rejected for families of
  variants under controlled noise.

## Immediate recommendation

The next major investment is the scenario laboratory itself: the three families above,
each with multiple variants, executed under controls A–D, tracking trials-to-success,
task effectiveness, boundary violations, regressions, execution cost, LLM usage, and
repeated failed strategies. If the full Familiar improves across repeated variants while
the memoryless controls do not, the project will have produced evidence that its
architecture contributes meaningfully beyond a standalone language model. That is the
experiment; everything else exists to support it.

## Status history

- 2026-07-22 — accepted as direction. The fixture framework, evaluator harness, and
  control conditions are Planned; this ADR is the specification they are built against.
