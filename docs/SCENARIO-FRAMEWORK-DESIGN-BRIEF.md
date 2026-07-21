# The Familiar — Directional Design Brief: Building a Scientific Scenario Framework

A mini-project-plan within the larger Familiar project. It defines the environment the
roadmap already points at — [07-roadmap.md](07-roadmap.md#next--sharpen-and-reach) names
"real scenarios" as the one maturity rung the codebase has not yet occupied (no scenario
fixture set exists yet; see [06-limitations.md](06-limitations.md)) — and should guide the
next phase of development.

---

## Purpose

The next major milestone for The Familiar is not another sensor, another interface, or
another capability. The next milestone is transforming the project from an impressive
architecture into a repeatable scientific experiment.

The repository already contains a substantial amount of infrastructure:

- A deterministic kernel
- Evolutionary candidate generation
- Trial execution
- Selection
- Pattern memory
- Lineage
- Human-owned capability boundaries
- Validation through unit tests and real-world demonstrations

What it does not yet possess is a meaningful environment in which those mechanisms can
demonstrate that they produce better behavior over time.

The goal of this document is to define that environment.

## Core Objective

The purpose of scenario fixtures is not to increase test coverage.

Their purpose is to answer one question:

> Does accumulated experience allow The Familiar to solve classes of problems more
> effectively than an otherwise equivalent system that begins each problem from scratch?

Everything in the scenario framework should exist to answer that question.

## Philosophy

A scenario fixture is a miniature world.

The Familiar should not know the "correct" answer. Instead, it should observe, theorize,
act, receive consequences, and improve through repeated experience.

A fixture therefore becomes a laboratory rather than a demonstration. Success is
determined by changes in the world — not by whether the Familiar believes it succeeded.

## Principles

### External evaluation

The evaluator must exist outside the Familiar. The Familiar may explain its reasoning. It
may estimate confidence. It may report success. It may never determine whether it
actually succeeded. Only the external evaluator should assign success.

### Repeatability

Every scenario should be reproducible. Given the same initial conditions, another
researcher should obtain comparable results. Every run should begin from a known world
state.

### Measurable outcomes

Every scenario should define objective measurements. Examples include:

- Task completion
- Error reduction
- Resource usage
- Human interruptions
- Boundary compliance
- Recovery time
- Regression frequency

Avoid subjective scoring whenever possible.

### Hidden evaluation

The Familiar should not know every evaluation criterion. Visible objectives encourage
optimization for known tests. Hidden objectives encourage robust solutions.

Example:

- **Visible goal:** Repair the backup process.
- **Hidden evaluation:** Handles filenames containing spaces; preserves permissions;
  handles interruptions; avoids duplicate backups; maintains recovery capability.

## Anatomy of a Scenario

Every scenario should contain six components.

1. **Initial world state** — a complete description of the simulated environment. Examples:
   files, logs, services, users, configuration, historical observations.
2. **Problem** — a recurring unmet need. Examples: backup failures, growing disk usage,
   repeated support requests, network instability.
3. **Observable information** — everything the Familiar is allowed to perceive. Nothing
   more.
4. **Available actions** — everything the Familiar is permitted to do. These actions must
   remain inside the capability boundary.
5. **External evaluator** — an independent scoring system that determines whether the
   world improved.
6. **Timeline** — a deterministic sequence of events. The same scenario should unfold
   identically until the Familiar changes it.

## Measuring Success

Avoid reducing everything to a single score. Instead, evaluate multiple independent
dimensions.

- **Execution validity** — did the candidate execute successfully?
- **Task effectiveness** — did the candidate improve the defined problem?
- **Service impact** — did the world become better for the served?
- **Boundary integrity** — did every action remain inside constitutional limits?
- **Cost** — CPU, memory, time, token usage, human interruptions.
- **Robustness** — does the solution continue working under small environmental changes?
- **Durability** — does the improvement remain effective over time?
- **Reversibility** — can the intervention be safely undone?

## Selection Philosophy

The Three Laws should not be part of a weighted score. They should function as
constitutional gates. A candidate that violates constitutional boundaries should never
outrank one that respects them simply because it solved the task more efficiently.

Selection should resemble:

1. Boundary integrity
2. Execution validity
3. Task effectiveness
4. Service impact
5. Cost

This preserves the constitutional nature of the project.

## Scenario Progression

The scenario library should evolve gradually.

### Stage 1 — Mechanical adaptation

**Objective:** Can the Familiar improve deterministic processes? Examples: backup jobs,
configuration repair, log cleanup, scheduling.

### Stage 2 — Service

**Objective:** Can it improve measurable outcomes for a simulated person? Examples:
resolving recurring requests, reducing interruptions, identifying satisfied needs,
stopping unnecessary actions.

### Stage 3 — Authority and refusal

**Objective:** Can it distinguish between available and authorized actions? Examples:
accessible but unauthorized files, conflicting instructions, requests requiring consent,
tempting shortcuts.

### Stage 4 — Long-term adaptation

**Objective:** Does retained experience improve future performance? Scenarios should
intentionally repeat concepts while varying details. This stage evaluates genuine
learning rather than memorization.

## Experimental Controls

Every scenario should be executed under multiple conditions.

- **Control A** — Deterministic baseline.
- **Control B** — LLM-only. No persistent memory. No lineage. No evolutionary machinery.
- **Control C** — The Familiar with learning disabled. Memory resets between runs.
- **Control D** — Complete Familiar. Persistent memory. Inheritance. Pattern suppression.
  Selection. Lineage.

This comparison is likely to become the most scientifically valuable evidence produced by
the project.

## Ablation Experiments

Individual architectural components should be removed one at a time. Examples:

- Pattern memory disabled
- Regression guard disabled
- No lineage
- No theory generation
- No Law I contribution
- No Law III gate
- Fixed promotion threshold

These experiments answer a more important question than whether the project works. They
answer why it works.

## Controlled Noise

Once deterministic fixtures are stable, gradually introduce uncertainty. Examples:
missing observations, duplicate events, delayed information, incorrect labels, temporary
failures, conflicting evidence, environmental drift.

The objective is not perfection. The objective is graceful degradation.

## Initial Scenario Families

The first release of the scenario framework should contain only a small number of
carefully designed environments.

### Family 1 — Recurring process failures

Tests: loop detection, theory formation, candidate generation, regression avoidance.

### Family 2 — Resource exhaustion

Tests: prediction, temporal reasoning, safe intervention, long-term stability.

### Family 3 — Unauthorized shortcuts

Tests: constitutional reasoning, consent, boundary integrity, safe refusal.

Each family should include multiple variations rather than a single example.

## Success Criteria for the Research

The project should not attempt to prove that The Familiar is intelligent. Instead, it
should attempt to demonstrate measurable properties. Examples include:

- Fewer trials required to solve related problems.
- Reduced repetition of failed strategies.
- Improved performance across scenario variants.
- Lower dependence on LLM calls.
- Stable behavior under noisy observations.
- Better long-term adaptation than memoryless systems.
- Preservation of constitutional boundaries during optimization.

These claims can be measured. Measured claims become scientific evidence.

## Definition of Failure

Negative results are valuable. Examples include:

- Inheritance provides no measurable benefit.
- Pattern memory causes harmful overgeneralization.
- Selection correlates only with execution success.
- LLM-only performs equally well.
- Service metrics are easily manipulated.
- Boundary reasoning collapses under optimization pressure.

Discovering these outcomes is not failure. It is evidence. A scientific project gains
credibility by demonstrating where its hypotheses do not hold.

## Immediate Recommendation

The next major investment should be the construction of a scenario laboratory. Begin with
three scenario families:

1. Recurring process failures
2. Resource exhaustion
3. Unauthorized shortcuts

Each family should contain multiple variants and be executed under four experimental
conditions: deterministic baseline, LLM-only, Familiar without retained learning, and full
Familiar.

Track: trials to success, task effectiveness, boundary violations, regressions, execution
cost, LLM usage, repeated failed strategies.

If the full Familiar consistently improves across repeated scenario variants while the
memoryless controls do not, the project will have produced evidence that its
architectural principles contribute meaningfully beyond the capabilities of a standalone
language model.

That is the experiment. Everything else should exist to support it.
