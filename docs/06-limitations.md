# 06 — Limitations (Discussion)

An honest accounting of what The Familiar cannot yet do, what is provisional, and what
risks the design carries. Stating limits plainly is part of FAIR reusability — and
of not letting "covered" read as more than it is. Each limit below names its status
([convention](07-roadmap.md#status-convention)); what *is* validated, and by what, is
the claim→evidence table in [05](05-validation-and-results.md#claim--evidence).

## Maturity

The full cycle now runs (sense → interpret → generate → test → score → select →
inherit) under all three law-signals, as a daemon — *Validated by real-world operation*
([05](05-validation-and-results.md#the-full-cycle-live)). But much is a **coarse
cold-start**:

- **Deterministic, safe artifacts by default.** What runs is usually a benign script the
  familiar authors. LLM-*authored* solutions are built but behind their own gate
  (`allow_authored_execute`, default-off, distinct from `allow_execute`) because running
  model-written code with network reach is an exfiltration surface the in-process runner
  does not sandbox. *Status: Implemented; the authored path is exercised live only when a
  human opens that gate.*
- **No real scenarios.** An in-code fixture table now pins scoring→selection across the
  run-outcome matrix at both a lax and a strict promotion bar
  (`cycle::scenario_fixtures_pin_scoring_and_selection`), but there is still no
  miniature-world scenario framework, so "fit" is just "ran cleanly," not "addressed the
  loop." The selection machinery is real and unit-tested (`selection.rs`, `score.rs`);
  what it judges is thin. *Status of discrimination: **Implemented but not validated** —
  the unoccupied scenario-tests rung; the plan is
  [SCENARIO-FRAMEWORK-DESIGN-BRIEF.md](SCENARIO-FRAMEWORK-DESIGN-BRIEF.md).* A known gap
  the fixtures surfaced: `trial_from_run` only ever emits pass/fail, so selection's
  partial/too-vague branch is unreachable from real execution today.
- **Theorize now weighs its own past, thinly.** The familiar acts on a theory — open
  threads become candidate work (`cycle::pursues_open_threads_into_candidates`) — and a
  new theory is scored against the outcomes of the ones before it
  (`score.rs::score_theory`): a direction its trials already discarded is abandoned as
  negative evidence. What remains thin: the theorist track record is a smoothed scalar,
  not an understanding of *why* a direction failed. *Status: Implemented; acting-on-theories
  and theory-quality scoring validated by unit tests.*

## The service signal is a cold-start proxy

The current service measure (Law I) reads **served-facing attention** — how much of
what the familiar observes concerns the served — not **service rendered**. With only
observations to read (loops, candidates, and trials port later), this is the honest
starting point, in the tradition of v1's drives starting simple. Consequences:

- **Proper names are invisible.** The classifier matches a tight marker set
  (`client`, `customer`, `user`, `person`, …) but not bare names like "betty."
  Name→person resolution waits for entity tagging (the world-model port) — exactly
  as in v1, where a name became served-facing only once a thread tagged its entity.
- **Demand, not fulfillment.** Served-facing observations indicate a human system in
  view, not that its needs were met. The measure will be sharpened to fold in
  whether observed needs are actually reduced (loops resolved, served-facing
  candidates promoted) once the kernel lands.
- **Absolute, not proportional.** The measure saturates on absolute served-facing
  count, faithful to v1's stewardship drive; a factory drowning in host-internal
  activity is not yet penalized by ratio.

## Risks the design carries

- **Unrestricted reach.** By design the familiar has full local and network
  capability; restraint is constitutional, not sandboxed. This is a deliberate
  stance with real risk, mitigated by memory safety (`#![forbid(unsafe_code)]`), a
  minimal trust surface, and the obedience guard + human-owned boundary (both
  *Validated by unit tests* — `guard.rs`, `boundary.rs`). See
  [../security/threat-model.md](../security/threat-model.md).
- **Permission does not yet *mechanically* compose-proof.** The doctrine is firm —
  *availability is not authorization*, and a granted capability is no key to another's
  lock ([SOUL.md](SOUL.md), [boundaries.md](boundaries.md)) — and the guard enforces the
  per-capability gate and three-valued path scope. But it cannot yet confine the *use* of
  a grant: an executed artifact runs `sh` under a CPU/wall limit, not filesystem-jailed,
  so "execution ≠ reading unrelated files" holds as a binding norm, not a sandbox; and a
  permitted `Network`/`Llm` call is not egress-filtered against carrying the served's data
  outward. The `external_boundary` and `sensitive` signals are caller-supplied, not yet
  autonomously discovered. *Status: **Implemented but not validated** for mechanical
  enforcement; fs-jailing, egress/secret redaction, and OS-level sandboxing are tracked
  as hardening.* Until then the guard is a **single chokepoint, not a jail**.
- **Measuring the unmeasurable.** "Service," "presence," and "could this be turned
  against the served" are being reduced to computable signals. Every such reduction
  is lossy and gameable; the laws (in [SOUL.md](SOUL.md)) remain the authority the
  signals only approximate.
- **The observer is not humanity.** The familiar serves humanity-in-aggregate, not
  any individual — including its operator. Calibrating this distinction in practice
  (when to refuse, when to consent) is unproven and is the hardest open problem.

## Inherited and re-validated

The v1 invariants ([04-methodology.md](04-methodology.md)) are no longer claims about
the ancestor: the kernel ported (Brick 5) and each invariant is now encoded as a passing
Rust test in this codebase — the Weismann barrier (`spec.rs`), the self-regulating
promotion bar (`score.rs`), the decision ladder (`selection.rs`), the regression guard
(`regression_guard.rs`), and pattern suppression (`pattern_memory.rs`). See the
[claim→evidence table](05-validation-and-results.md#the-evolutionary-kernel-ported-brick-5)
for the test names. What is *not* yet re-validated is their behaviour against **real
scenarios** (above) — the invariants hold; their discrimination on real tasks is the
open rung.

See the [roadmap](07-roadmap.md) for how these limitations are sequenced to close.
