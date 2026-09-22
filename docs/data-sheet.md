# Datasheet — the observation log

A "Datasheet for Datasets"-style record (after Gebru et al., 2021) for the data
The Familiar collects and reasons over. The sensitivity view is
[../security/data-classification.md](../security/data-classification.md); the
conceptual model is [data-model.md](data-model.md).

## Motivation

- **Why does this data exist?** To let the familiar observe the human and technical
  systems it serves, detect loops, and measure whether it is serving (Laws I–II).
  Without observations there is no truth to act on.
- **Who collects it / for whom?** The familiar itself, on the operator's host, for
  the served. Not for any third party — nothing is transmitted outward.

## Composition

- **Instances:** observations — `actor · action · object · context · source · ts ·
  confidence`. Schema: [../data/schema/observation.schema.json](../data/schema/observation.schema.json).
- **Does it contain data about people?** Yes — served-facing observations may
  describe people and their needs/interactions. Classified High sensitivity.
- **Sensitive content?** Potentially (needs, health, schedules, relationships,
  depending on what is observed). Minimization is policy; capture only what's needed.
- **Sample:** [../data/sample/observations.jsonl](../data/sample/observations.jsonl)
  is **synthetic** — no real persons.

## Collection process

- **How collected?** Via the CLI (`observe`), sensors, and (planned) signal
  ingestion; all initiators are recorded for provenance, none granted authority.
- **Timeframe:** continuous/append-only; each record carries a `ts`.
- **Consent:** consequential outward action seeks consent (planned obedience guard);
  observation itself is local and operator-controlled. A forgetting/retention path is
  an open item ([../security/privacy-review.md](../security/privacy-review.md)).

## Preprocessing / cleaning

- Records are stored verbatim and append-only; **observations are the only truth and
  are never edited**. Derived views (loops, signals) do the normalization and can be
  rebuilt from the raw log.

## Uses

- **Current:** computing the service signal (Law I); listing.
- **Foreseeable:** loop detection, candidate generation/testing, presence and guard
  signals.
- **Uses to avoid:** profiling or surveilling individuals; any use that serves a
  commander against the served (Law III).

## Distribution & maintenance

- **Distributed?** No. The log stays local; runtime data is git-ignored and never
  committed. Only the synthetic sample ships.
- **Maintained by:** the operator/host. Integrity rests on the append-only,
  rebuildable design; corruption surfaces as a hard load error.
- **Retention:** open item — a tombstone-and-rebuild forgetting mechanism is planned.
