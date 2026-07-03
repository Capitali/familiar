# Data Model

The conceptual model of what The Familiar stores and how the records relate. This is the
*meaning*; the operational format is an embedded SQLite database (one table per record type) —
see [storage.md](storage.md). JSONL is now the import/export format (`familiar db export`),
not the on-disk store. Human-owned config (`boundary.json`, `parameters.json`) stays plain text.

## The one truth: observations

Everything begins with the **observation** — a `actor · action · object` triple plus
provenance (`source`, `ts`, `confidence`, optional `context`). The observation log
is therefore a **triple store**, and it is the *only* authoritative record. Every
other record is **derived** from observations and can be discarded and rebuilt. This
is what lets derived views churn freely without ever drifting from what was actually
observed.

Current schema: [`../data/schema/observation.schema.json`](../data/schema/observation.schema.json).

## Derived & lifecycle records (porting with the kernel)

These existed in v1 and port in subordinate to the law-signals. Listed here so the
full model is visible even before the code lands:

| Record | Is | Derived/relates to |
|---|---|---|
| **Loop** | a recurring `actor·action·object` pattern | grouped observations (temporal view) |
| **Candidate** | a response to a loop | `loop_id`, `parent_id` (lineage), hypothesis, traits |
| **Trial** | a test of a candidate | `candidate_id`, scenario, scores, failure class |
| **Pattern memory** | a lesson from trial history | positive/negative evidence across candidates |
| **Lineage** | ancestry of a candidate | the `parent_id` chain |
| **Service / Presence signal** | Law I / Law II measures | computed from observations (and later loops/trials) |
| **Guard record** | a Law III decision | allow / seek-consent / refuse + rationale, attached to an action |

## Relationships (sketch)

```
observation* ──grouped temporally──▶ loop ──prompts──▶ candidate ──tested by──▶ trial
     │                                                     │                      │
     └──condensed spatially──▶ (world-model, later)        └──parent_id──▶ lineage │
                                                                                   ▼
                            service/presence signals ◀──computed from──  pattern memory
```

## Invariants the model must hold

- Observations are append-only and authoritative; derived records never feed back as
  truth.
- A candidate child has a `parent_id`; a mutation records its `changed_traits`.
- The genotype/phenotype (Weismann) barrier: somatic state never edits heritable DNA.

(Full invariant list and their tests: [04-methodology.md](04-methodology.md) and
[../validation/test-plan.md](../validation/test-plan.md).)
