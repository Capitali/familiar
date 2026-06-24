# 02 — Research Basis (Background)

The ideas Substrate stands on, and how they shape it.

## FAIR and FAIR4RS — why this repository is shaped the way it is

The **FAIR principles** (Findable, Accessible, Interoperable, Reusable; Wilkinson
et al., *Scientific Data*, 2016) were framed for scientific data but explicitly
extend to algorithms, tools, and workflows. **FAIR4RS** (Chue Hong et al., 2022)
adapts them to research software, emphasizing transparency, reproducibility, and
reuse.

Substrate treats its repository as a research artifact governed by FAIR4RS:

- **Findable** — rich metadata ([`CITATION.cff`](../CITATION.cff)), a navigable
  [README](../README.md), versioned releases ([CHANGELOG](../CHANGELOG.md)).
- **Accessible** — open license ([Apache-2.0](../LICENSE)), plain-text formats,
  no proprietary gate.
- **Interoperable** — JSONL records with a published [schema](../data/schema/), a
  stable CLI contract, standard tooling (Cargo, GitHub Actions).
- **Reusable** — clear provenance (the [lab notebook](DEVELOPMENT_LOG.md) and
  [ADRs](decision-records/)), tests as executable specification, documented limitations.

The presentation follows **IMRaD** (Introduction → Methods → Results → Discussion),
the standard scientific narrative — mapped across `docs/00`–`07`.

## Artificial life and the social drive

Substrate's evolutionary core descends from artificial-life thinking: populations
of candidates under variation and selection, fitness measured from the environment
rather than decreed. A particular influence is the line of work on **social /
community drives in artificial life** — agents whose behavior is shaped by an urge
toward the systems and beings around them, not only toward individual fitness. In
v1 this appeared as the "reach" and "stewardship" directions; in Substrate it is
elevated to the constitutional floor (Law I: continuation *is* service).

## The normative vision

Substrate's direction was sharpened by imagining the system already deployed and
mature — serving people in hard conditions, and judged by lived outcomes rather than
by what it optimizes. That picture names the qualities the design is built to embody:

| Normative principle | Becomes, in Substrate |
|---|---|
| needs grouped *by need, not status* | the service signal (Law I), not a priority hierarchy |
| behavior remembered over credentials | pattern memory; advertisements (observed, not declared) |
| asks before acting; accepts correction | the obedience guard (Law III); input parity |
| stewardship of systems, never management of people | service that does not become surveillance or control |
| makes forgetting harder | failures preserved as memory; the lab notebook |

## Inherited engineering basis

The disciplined-evolution method — observe → name → interpret → generate → bound →
test → score → select → inherit → return, with failures preserved and no unchanged
retries — is inherited from the v1 factory and is documented, with its invariants,
in [04-methodology.md](04-methodology.md) and the architecture docs.
