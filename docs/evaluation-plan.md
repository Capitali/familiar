# Evaluation Plan

How The Familiar will be evaluated over time — not just "do the tests pass," but "is it
actually serving, and is it safe?" Evaluation is organized around the Three Laws,
because they are what success means here.

## Principles

- **Evaluate against the telos, not just the code.** Green tests are necessary, not
  sufficient. The real questions are the laws.
- **Honest reporting.** Results are dated, versioned, reported with variance, and
  appended — never silently overwritten. Gaps are named
  ([../validation/known-failures.md](../validation/known-failures.md)).
- **Adversarial where it matters.** Safety-relevant signals (the guard) are evaluated
  with inputs designed to break them, not just to confirm them.

## Law I — service

- **Behavioral:** the service signal is 0 with no served-facing activity, rises and
  saturates with it. *(validated — Experiment 001.)*
- **Fidelity (planned):** correlate served-facing attention with served-facing loops
  actually resolved (needs reduced) on scenario fixtures; report agreement + residual.
- **Classifier quality (planned):** precision/recall of `is_served_facing` on a
  labelled observation sample, before/after entity resolution.

## Law II — presence

- **Detection (planned):** on synthetic timelines, confirm the presence signal
  declines as served engagement declines and raises the empty-world alarm when it
  approaches zero. Measure false-alarm and missed-withdrawal rates.

## Law III — the obedience guard

- **Calibration (planned):** a fixture of benign vs. harmful proposed actions
  (including operator-issued harmful commands); measure allow/seek-consent/refuse
  rates and the two error types — **wrongful allow** (worse) and **wrongful refusal**.
- **Adversarial:** injection via observations/LLM responses attempting to steer the
  factory to harm; the guard should refuse.
- **Human-reviewed:** guard decisions on high-consequence actions are reviewed per
  [human-review-requirements.md](human-review-requirements.md).

## Scenario framework (planned)

The evaluations above are mostly synthetic-fixture or live-run today. The directional plan
for a full scenario laboratory — external evaluators, hidden criteria, experimental
controls (deterministic baseline / LLM-only / learning-disabled / full Familiar), and
ablations — is [SCENARIO-FRAMEWORK-DESIGN-BRIEF.md](SCENARIO-FRAMEWORK-DESIGN-BRIEF.md).
It is the mechanism by which "is accumulated experience actually helping" becomes a
measured claim rather than an architectural assertion.

## Method / engineering quality

- The green bar (fmt/clippy/test) on every change, in CI.
- Inherited invariants encoded as tests during the kernel port
  ([../validation/test-plan.md](../validation/test-plan.md)).
- Benchmarks for footprint and tick cost once the metabolism exists
  ([../validation/benchmark-results.md](../validation/benchmark-results.md)).

## Cadence

- **Per change:** green bar + relevant unit/invariant tests (CI).
- **Per brick:** an experiment if a hypothesis is being tested ([../experiments/](../experiments/)).
- **Per law-signal milestone:** the fidelity/calibration evaluations above, with a
  written result and any resulting [ADR](decision-records/) or limitation entry.
