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

## Humans and devices: two records, related, never conflated

Added by [ADR-0039](decision-records/0039-humans-and-devices-are-separate-records.md), which
ended a long-running conflation: one name slot was doing two jobs, so phones established as
their *human* ("ian") while Macs were named as *machines* ("wildhorse"), and no roster could
show both facts because the model stored one.

| Record | Is | Key fields |
|---|---|---|
| **HumanRecord** | one per human the mesh serves | `handle`, `name`, `devices[]` (associations, current and past), `relationships[]`, `preferences`, `habits`, `routines[]` |
| **DeviceRecord** | one per device | `device_id`, `name`, `kind`, `posture`, `capabilities`, `observation_interfaces`, `networks[]`, `humans[]` (associations) |

Two properties of this pair carry most of its weight:

- **The relation is plural and time-bounded.** `DeviceRecord.humans` is a list of associations
  with `since`/`until`, so a device may be used by several people, or by none, without anyone
  owning it — and the history of who used it survives the association ending.
- **`kind` and `posture` are different axes.** `kind` is what the device *is* ("phone");
  `posture` is how it is *held* — `carried` (it follows a person) or `fixed` (a **station**:
  bound to a place, serving whoever is there,
  [ADR-0042](decision-records/0042-the-station.md)). Presence inference turns on the second,
  not the first: a carried device's heartbeat is evidence about its human, a station's is
  evidence only that it is powered. Posture is declared by a human and never entered by the
  familiar on its own, because the two possible mistakes fail in opposite directions —
  reading a personal phone as a station *suppresses* real presence, reading a station as
  personal *manufactures* it.

The roster is a **view** over these two records, never a third store.

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
