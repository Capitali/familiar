# ADR-0002 — Re-found telos-first as a new repository

- **Status:** accepted
- **Date:** 2026-06-23

## Context

The predecessor (`Capitali/factory`, ~13k LOC C99) was built bottom-up: the
evolutionary machine first, purpose added afterward (survival as an ungrounded
efficiency drive; stewardship only at the tenth rule). It never grounded *why
continue at all*, leaving open the two default failure modes of a capable
optimizer — the empty world and the obedient instrument (see
[01-problem-statement.md](../01-problem-statement.md)). A normative vision —
imagining such a system already deployed and serving people — crystallized a
different conviction: **purpose is the floor, not emergent.**

## Decision

Archive the predecessor (tag `v1-final`, mark archived) and re-found the project in
a **new repository** whose **genesis commit is the constitution** — three laws as
root, the whole design derived downward from them ([SOUL.md](../SOUL.md)). The
evolutionary *method* is inherited; the *foundation and order of derivation* change.

## Consequences

- **Gained:** purpose is structurally prior to mechanism; the laws can be made
  measurable before any machine is built; a clean lineage boundary.
- **Given up:** the bottom-up, purpose-agnostic origin; the v1 git history and its
  accumulated runtime data (preserved in the archive, not carried forward).
- **Carried forward:** the sound machinery (observation/loop/candidate/trial/
  selection/memory) and its invariants — to be *ported*, subordinate to the laws,
  not rebuilt from scratch (so as not to violate "discovery must reduce future cost").

## Alternatives considered

- **Refactor v1 in place** — rejected: the bottom-up framing is pervasive in v1's
  Soul and structure; re-founding on the constitution is clearer than retrofitting.
- **New Soul only, keep building v1** — rejected: the inversion is foundational
  enough to warrant a clean start.

## Status history

- 2026-06-23 — accepted; `Capitali/factory` archived at `v1-final`.
