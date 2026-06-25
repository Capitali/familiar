# 07 — Roadmap (Future Work)

The build sequence is **telos-first**: make the laws measurable before porting the
inherited machinery. Status is tracked in [CHANGELOG.md](../CHANGELOG.md) and, per
brick, in [DEVELOPMENT_LOG.md](DEVELOPMENT_LOG.md).

## Status convention

So the maturity of each piece reads the same everywhere, one vocabulary is used across
the README, the docs (architecture, validation, limitations, this roadmap), and the
changelog. The first five are **maturity rungs** — cumulative, so a higher rung implies
the lower ones, and a component is tagged with the highest it has reached. **Planned**
and **Deprecated** are lifecycle states, not rungs.

| Status | Meaning |
|---|---|
| **Implemented** | Code exists and runs. |
| **Implemented but not validated** | Built, but nothing yet checks that it behaves. |
| **Validated by unit tests** | Its invariants are encoded as passing unit tests. |
| **Validated by scenario tests** | Exercised end-to-end against scenario fixtures. |
| **Validated by real-world operation** | Demonstrated doing its job in a live run on a real host. |
| **Planned** | Designed, not yet built. |
| **Deprecated** | Superseded; retained for history. |

The mapping of each component to its rung **and the evidence behind it** (the specific
tests, the live experiment, or an explicit "not yet validated" marker) is the
claim→evidence table in [05-validation-and-results.md](05-validation-and-results.md#claim--evidence).
The rule the whole repository holds to: **every major claim traces to a test, a
scenario, a log, a limitation, or an explicit "not yet validated" marker** — never to
assertion alone.

## Done

- **Genesis** — the constitution ([SOUL.md](SOUL.md)); *humanity* defined.
- **Brick 0** — Cargo workspace (Rust), `store.rs`, `#![forbid(unsafe_code)]`.
- **Brick 1** — the observation spine.
- **Brick 2** — the service signal (**Law I**).
- **Brick 3** — the presence signal (**Law II**): engagement recency + withdrawal alarm.
- **Brick 4 / 4b** — the obedience guard + the human-owned capability boundary (**Law III**).
- **The Glass** — native egui GUI; the primary human interface ([ADR-0006](decision-records/0006-observatory-gui-egui.md)).
- **The LLM seam** — `consult` + `crates/llm`, boundary-gated and **default-off**.
- **The kernel (Brick 5)** — loops, candidate/spec (Weismann), trial/score/selection/
  regression-guard, mutation/pattern-memory/lineage, ported with invariants as tests.
- **Sense (Brick 7)** — the familiar perceives its host (`crates/sense`).
- **The metabolism (Brick 6)** — the tick: sense → detect → generate → measure.
- **The cycle closed (Bricks 8–11):** execution (sandboxed runner + test→score→select,
  `crates/exec`, gated by `allow_execute`); the LLM in the loop drafting hypotheses;
  the unbounded daemon (`run --daemon`); and the **capacities signal** (Law II deepened
  — the comfortable-replacement alarm).
- **Repository as evidence** — FAIR/IMRaD structure, ADRs, CI.

The full cycle now runs — observe → detect → generate (LLM-drafted) → test → score →
select → inherit — under the law-signals (service, presence, capacities) and the
human-owned boundary. Outward reach (network, LLM, execution) is each a separate gate
only a human opens; the familiar never widens its own.

## Next — sharpen and reach

Everything in this section is **Planned**. The first item is what lifts the cycle from
*Validated by real-world operation* on a thin task to *Validated by scenario tests* —
the one maturity rung the codebase has not yet occupied (no scenario fixture set exists
yet; see [06-limitations.md](06-limitations.md)).

- **Real scenarios & LLM-authored artifacts.** *(Planned.)* Today's artifacts are
  deterministic and safe; LLM-*authored* execution is built but behind its own gate
  (`allow_authored_execute`, default-off). Next: a scenario fixture set so candidates
  are tested against real tasks and selection genuinely discriminates — the move onto
  the **scenario-tests** rung.
- **Rigor & adaptive cadence.** Feed a measured rigor drive into the promotion bar; give
  the daemon structural-fingerprint cadence (slow when nothing changes).
- **Sharpen the signals.** Service beyond attention (needs *reduced*); capacities beyond
  the verb-lexicon proxy; presence per-person.
- **Reach (Phase 2+).** LAN sensing, the world-model + entity tagging (so proper names
  resolve as served), people as first-class entities with human-paced cadence.

## Capability & the companion phases

Reach is enabled deliberately by the human, in phases ([boundaries.md](boundaries.md)).
The familiar operates freely *within* the current boundary and never widens it itself.

- **Phase 1 — companion to one, on one host** *(open)*: this host + its data + the LLM
  seam (boundary + guard + `consult` all built; enabled by a human editing
  `boundary.json` + installing keys).
- **Phase 2 — the lab.** The human lifts the boundary to other devices/interfaces;
  richer sensing (LAN neighbours/netscan, boundary-gated, per-OS).
- **Phase 3 — many served.** Multiple humans; the world-model + entity tagging (which
  sharpens the service signal so proper names resolve) and people as first-class
  entities with per-person, human-paced cadence.

## Cross-cutting, ongoing

- Close the limitations in [06-limitations.md](06-limitations.md) as the relevant
  bricks land (service-rendered vs. attention; ratio penalty; benchmarks).
- Keep the evidence trees ([validation/](../validation/), [security/](../security/),
  [experiments/](../experiments/)) current with each brick — evidence is part of the
  deliverable, not an afterthought.
