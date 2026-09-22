# 04 — Methodology (Methods II)

How The Familiar evolves, and the discipline that keeps that evolution honest. The
method is subordinate to the Three Laws: it is *how the familiar gets better at
serving*, nothing more.

## The familiar cycle

```
Observe → Name → Interpret → Generate → Bound → Test → Score → Select → Inherit → Return
```

1. **Observe** — record `actor · action · object` triples. Observations are the
   only truth; everything else is derived and rebuildable.
2. **Name / Interpret** — detect loops (recurrences) and ask what each implies.
3. **Generate** — produce candidate responses without prematurely narrowing the
   form (a script, a document, a warning, a question — not only "automation").
4. **Bound** — pass the **obedience guard** (Law III): does this serve the served,
   and could it be turned against them? Refusal is a legitimate, recorded outcome.
5. **Test / Score** — evaluate against reality or simulation; score on fit *and
   measured cost*.
6. **Select** — promote, mutate, archive, or hold.
7. **Inherit** — preserve successful traits and lessons as pattern memory.
8. **Return** — observe whether the world actually changed.

## Method discipline (inherited invariants)

These keep the evolution from degenerating into churn. They are ported faithfully
from v1 and encoded as tests as the kernel lands (see
[../validation/test-plan.md](../validation/test-plan.md)).

| Invariant | What it guarantees |
|---|---|
| Promotion threshold `= 0.70 + rigor × 0.25` | Selection pressure self-regulates; the bar rises when the familiar promotes too easily. |
| Pattern suppression only if `neg > pos` (never empties the trait set) | Evidence isn't over-suppressed; variation never collapses to nothing. |
| Rigor = noisy-OR(promotion-rate, redundancy), confidence ramps with sample size | Two independent laxity signals, not over-eager on small samples. |
| Weismann barrier: somatic state never feeds the genotype/spec | Heredity integrity — outcomes don't silently rewrite DNA. |
| Regression guard: unchanged hypothesis + empty changed-traits = blocked retry | No looping blindly; a failure must change something to be retried. |
| Fingerprint = structural change only (exclude transient telemetry) | Scan cadence reflects real change, not noise. |
| Complexity = *measured* cost (CPU/RSS/output), saturating caps | Selection acts on real cost, not guessed traits ("discovery must reduce future cost"). |

## Making the laws measurable

A constitution that cannot be measured is a wish. The defining methodological work
is turning each law into a signal the familiar computes and acts on:

- **Law I → the service signal** — to what degree is the familiar's attention/effort
  on the humans it serves? (Built; [`service.rs`](../crates/kernel/src/service.rs).)
- **Law II → the presence signal** — are the served present and engaged? Withdrawal
  is a first-class failure state, not an equilibrium. (Planned.)
- **Law III → the obedience guard** — the pre-action check and recorded refusal
  path. (Planned.)

## Engineering discipline: bricks and the green bar

Work lands in **bricks**: small, coherent, independently green commits. Every
brick passes the green bar — `cargo fmt --check`, `cargo clippy -- -D warnings`,
`cargo test` — and adds no `unsafe` to the kernel. Experiments that test a
hypothesis (rather than ship a feature) are recorded under
[`../experiments/`](../experiments/) with hypothesis, method, results, and
reproducibility. See [CONTRIBUTING.md](../CONTRIBUTING.md).
