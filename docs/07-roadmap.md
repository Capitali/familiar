# 07 — Roadmap (Future Work)

The build sequence is **telos-first**: make the laws measurable before porting the
inherited machinery. Status is tracked in [CHANGELOG.md](../CHANGELOG.md) and, per
brick, in [DEVELOPMENT_LOG.md](DEVELOPMENT_LOG.md).

## Done

- **Genesis** — the constitution ([SOUL.md](SOUL.md)).
- **Brick 0** — Cargo workspace (Rust), `store.rs`, `#![forbid(unsafe_code)]`.
- **Brick 1** — the observation spine.
- **Brick 2** — the service signal (**Law I**).
- **Repository as evidence** — FAIR/IMRaD structure, ADRs, CI.

## Next

- **Brick 3 — the presence signal (Law II).** Measure presence/engagement of the
  served from observation recency/cadence. Define the **empty-world failure state**:
  presence decaying toward zero is a first-class alarm, not an equilibrium. Per the
  definition of humanity ([SOUL.md](SOUL.md), "What humanity is"), presence is
  ultimately about the persistence of *capacities* (suffering, meaning, relationship,
  memory, choice), so this brick also seeds detection of the **comfortable
  replacement** — quiet diminishment, not only absence. Capacity-level measurement is
  hard and will start as a coarse proxy (as the service signal did) and sharpen.
- **Brick 4 — the obedience guard (Law III).** A pre-action check every consequential
  action passes — *does this serve the served, and could it be turned against them?*
  — yielding allow / seek-consent / **refuse**, with a recorded rationale. Upgrades
  v1's passive boundary warnings into an active gate. Home of "restraint is
  constitutional" (no telemetry, no exfiltration enforced here).

## Then — port the inherited kernel (the method)

- **Brick 5 — the evolutionary kernel.** Port loop detection, candidate, trial,
  score/selection, mutation + spec (the Weismann barrier), regression guard,
  lineage, and pattern memory from v1, **preserving the documented invariants**
  ([04-methodology.md](04-methodology.md)) as Rust tests. Now subordinate to the
  law-signals.
- **Brick 6 — orchestration (the metabolism).** The autonomous tick with the
  law-signals woven through it and the obedience guard at the act step; adaptive
  structural-fingerprint cadence; the LLM boundary (`prompt → call_llm.sh →
  response`) as the periphery seam.

## Later — reach and refinement

- **Brick 7+** — sensing (sensor/netscan, per-OS), the world-model and entity
  tagging (which sharpens the service signal so proper names resolve), people as
  first-class entities with human-paced cadence, a dashboard, and richer
  experiments/benchmarks.

## Cross-cutting, ongoing

- Close the limitations in [06-limitations.md](06-limitations.md) as the relevant
  bricks land (service-rendered vs. attention; ratio penalty; benchmarks).
- Keep the evidence trees ([validation/](../validation/), [security/](../security/),
  [experiments/](../experiments/)) current with each brick — evidence is part of the
  deliverable, not an afterthought.
