# Capability Recipe v1 — design before build

- **Task:** T-115
- **Owner:** `companion:codex`
- **Status:** implementation contract (Q2 was decided in reasoning-dialogue round 3)
- **Scope of this brick:** `familiar-recipe`, a pure interpreter crate. Kernel/cycle
  persistence and scheduling are a later integration brick so this work does not cross
  the controller's T-112/T-113 scope.

## Purpose

A capability recipe is the familiar's first non-shell authored program. It composes
already-proven tools by id and transforms their returned values into one declared
observation-shaped output. It is deliberately less expressive than Python: its authority
boundary is true by construction rather than inferred by reviewing arbitrary code.

The interpreter has no functions that open a file, start a process, reach a network,
read a clock, or inspect an environment. The caller injects a `ProvenToolSource`; that
source is responsible for resolving only healthy, reviewed library tools and enforcing
their existing human-owned gates. A recipe contains a `tool_id`, typed scalar arguments,
and transformations — never an executable, path, URL, or command.

## Document shape

Every struct and tagged enum uses serde `deny_unknown_fields`. The JSON envelope is
bounded before deserialization and `version` must be `1`.

```json
{
  "version": 1,
  "inputs": [
    {
      "name": "climate",
      "tool_id": "tool-0042",
      "args": { "zone": "greenhouse" }
    }
  ],
  "steps": [
    { "op": "parse_json", "from": "climate", "save_as": "document" },
    {
      "op": "select",
      "from": "document",
      "path": [{ "field": "readings" }],
      "save_as": "readings"
    },
    {
      "op": "filter",
      "from": "readings",
      "predicate": {
        "path": [{ "field": "watts" }],
        "comparison": "gt",
        "value": 0
      },
      "save_as": "active"
    },
    { "op": "mean", "from": "active", "path": [{ "field": "watts" }],
      "save_as": "mean_watts" }
  ],
  "emit": {
    "actor": "greenhouse",
    "action": "uses-power",
    "object_template": {
      "segments": [
        { "literal": "mean " },
        { "slot": "mean_watts", "path": [] },
        { "literal": " W" }
      ]
    },
    "context_template": { "segments": [{ "literal": "recipe v1" }] }
  },
  "limits": { "rows": 256, "bytes": 65536, "steps": 16 }
}
```

Names are ASCII lower-case identifiers (`[a-z][a-z0-9_]*`). Inputs and step outputs
occupy immutable named slots; duplicate names and forward references are rejected. The
immutability makes lineage and replay legible and prevents a later step from silently
changing what an earlier name meant.

## Runtime values and operations

The runtime is a closed value set: UTF-8 text, JSON, rows (JSON objects), grouped rows,
finite numbers, booleans, and null. An operation refuses an incompatible input type; it
does not coerce a malformed value or skip a bad row.

- `parse_json`: UTF-8 text → JSON.
- `parse_lines`: UTF-8 text → rows shaped as `{ "value": line }`; line order is kept.
- `select`: walks an explicit typed path of `field`/`index` segments. A selected array of
  objects becomes rows; scalars become the matching runtime scalar.
- `map`: projects every row through named field/literal expressions. Output field order
  is deterministic.
- `filter`: keeps rows satisfying one typed scalar predicate (`eq`, `ne`, `lt`, `lte`,
  `gt`, `gte`, or `contains`). Missing fields and incompatible comparisons are errors.
- `group`: groups rows by a scalar path. Groups use a canonical scalar key and a sorted
  map, so output does not depend on hash iteration.
- `count`: rows → a number; grouped rows → rows of `{group, value}`.
- `min`, `max`, `mean`: aggregate a numeric path. Rows produce one finite number;
  grouped rows produce sorted `{group, value}` rows. Empty numeric aggregates fail.
- `compare`: compares one scalar slot with a typed scalar literal and yields a boolean.
- `format`: concatenates typed literal/slot segments. Slot rendering is canonical JSON;
  there is no ambient interpolation language.

The emit templates use the same typed segments. `actor` and `action` are fixed nonempty
literals; `object` and `context` are rendered from immutable slots. The result also
returns the ordered `(input name, tool id)` lineage. Persistence as an Observation is a
caller's decision, not a side effect of evaluation.

## Bounds and refusal behavior

Recipe-declared limits are mandatory, positive, and capped by compile-time ceilings:
64 KiB manifest, 16 tool inputs, 64 steps, 10,000 rows, and 4 MiB of materialized values.
The declared `steps` limit must cover the document before any tool is invoked.

At runtime the interpreter accounts for:

1. the bytes returned by every tool;
2. each immutable slot materialized by parsing or transformation;
3. every row produced by parsing, selection, mapping, filtering, grouping, or grouped
   aggregation; and
4. the rendered object/context output.

Every add uses checked arithmetic. The first exceeded bound stops execution with a typed
error. Tool failures, unknown/unproven ids, non-UTF-8 output, non-finite arithmetic,
missing paths, invalid comparisons, and empty aggregates are also explicit failures.
There is no partial observation and no best-effort fabrication.

The injected source is invoked in declared input order, once per input. Steps run in
document order. Maps used in schemas and values are ordered maps. These rules plus the
absence of time/randomness/ambient I/O make replay byte-for-byte deterministic for the
same recipe and tool outputs.

## Verification floor

Unit tests must pin:

- strict parsing (unknown fields, wrong version, duplicate/forward slot names);
- unknown tool refusal and ordered, once-only structural invocation;
- every operation, including grouped aggregates and stable formatting;
- row, byte, step, input, and hard-ceiling enforcement before or during execution;
- malformed JSON/UTF-8, missing paths, type mismatches, incompatible predicates,
  non-finite mean, and empty aggregate refusal;
- deterministic replay and exact input lineage; and
- a source implementation that attempts filesystem/network/process access is never part
  of the interpreter API — only returned bytes cross the seam.

## Deliberate exclusions

Recipe v1 has no loops, branches, joins, mutation, dynamic tool ids, executable/path/URL
fields, clock, randomness, secrets, or side effects beyond calling the injected proven
tool source. It does not ship Python. Python may help author candidates inside the
scenario lab but is not a live artifact. WASI remains a separate later decision based on
real cross-platform toolchain costs.

This brick builds the language and its truth-preserving execution seam. Registering
recipe artifacts in `tools.jsonl`, author/repair prompting, scheduling them in the cycle,
and evaluating them against T-116 fixture oracles are follow-on work after the controller
lands the prediction-engine changes in the same call paths.
