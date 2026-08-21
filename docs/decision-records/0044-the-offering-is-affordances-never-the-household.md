# ADR-0044 — The offering is affordances, never the household

- **Status:** **proposed** (design decided in the conduct dialogue Rounds 4-5,
  2026-08-20 — codex's Round 4 absorbed nearly whole; awaiting Ian's acceptance
  before any code, per T-216's own accept)
- **Date:** 2026-08-20
- **Relates to:** [ADR-0043](0043-one-typed-source-per-kind-of-truth.md) (kinds of truth
  have kinds of addressee; producer + addressee completeness), ADR-0037 §A (the MCP
  server), the MCP door as deployed (STATE 2026-08-18: covenant-gated, fails closed three
  ways), Recipe v1 (T-115: declaration never grants authority), ADR-0005 (no companion
  may open a gate), T-217 (viewer-scoped naming — the sibling privacy decision)

## Ian's direction (2026-08-20, verbatim)

> "I really want to explore the idea of a rich MCP interface to allow other AI's to
> interact with the familiar. Everything the familiar learns how to control should become
> part of that offering to other AI. Anonymized so that the original user learning
> doesn't leak."

## Context

The familiar's learned control surface is five classes (dialogue Round 3b, traced live):
declared actuators, standing reaction rules, the authored tool library, capability
recipes, and pattern memories. Their identifying content is their MEANING, not incidental
metadata: a standing rule is a household policy naming a human and their comings and
goings; a tool script is executable authority embedding LAN internals; a pattern is
compressed household history. None is an offerable unit, even with names replaced —
which is why pseudonymizing existing records is rejected outright.

## Decision

**1. The unit of offering is a capability CLASS, compiled — deliberately lossily — from
learning.** The offering registry is a new type, never a projection of existing records:

- a versioned class id (`lighting.dimmable/v1`);
- typed input and observable-output schemas, with units and bounds;
- declared effect and affected-subject CLASS (never a household subject);
- failure, idempotency, and closed-revert semantics (the actuator revert-map discipline);
- the constitutional/boundary gates an invocation would require;
- provenance as a coarse assurance level only: `declared` | `observed` | `proven`.

**2. The anonymization boundary is an allowlist serializer over this new type** — never an
LLM rewriting records. The invariant is testable by construction: no field type in the
public declaration can carry an internal identity, an executable command, a schedule, a
count, an evidence id, or free prose. The three leak classes (rule subjects/triggers, act
commands, free-prose lesson text) are unrepresentable in the catalog. Household-specific
intelligence may become a class definition only through an explicit human
declassification step; `PatternMemory` is never published.

**3. Instances exist only as grants.** Pre-grant: no instance tokens, no counts — classes
only. Post-grant: one opaque handle per `{partner, grant, surface}`, stable for the grant
epoch (idempotency, audit, revocation need it), destroyed on revocation, minted fresh on
re-grant, never correlatable across partners. A grant is a deliberate disclosure and is
named as one to the human who makes it.

**4. The authority ladder — five rungs, each separately closed by default:**

1. `attest` — covenant acceptance; no inventory, no authority (built, live today).
2. `discover_classes` — the safe catalog; attested partners only; no instances/state/counts.
3. `request_grant` / `propose` — a typed desired effect for human consideration.
   Proposal is not permission.
4. `observe` — only the explicitly granted state fields of a grant-bound instance.
5. `invoke` — only granted acts, within parameter, time, rate, and affected-subject bounds.

A grant intersects partner × surface × act × parameter bounds × duration — a surface-wide
boolean is too coarse. Observation is never bundled with actuation. `allow_agent` is a
global ceiling that can never manufacture a missing grant. Revocation is immediate. No
capability becomes externally reachable merely by appearing in the catalog.

**5. Partners are typed-only.** A proposal's bounded `reason` text is data for the human;
if it ever enters the familiar's reasoning it passes the same screen and typed admission
as any untrusted prose (ADR-0043 §3). Tool descriptions, schemas, ids, and errors are
kernel-authored; a partner never supplies text that becomes system or developer context.

**6. Every partner act is auditable and narrated.** Each attempted proposal, observation,
and invocation produces a typed partner act with outcome
(`refused` | `proposed` | `completed` | `failed` | `reverted`). Narration names the
partner by HUMAN-CHOSEN alias plus key fingerprint — never its self-asserted name — and
names the affected local surface to the authorized human, never echoing that local name
back to the partner. Rate limits may aggregate refusals for presentation but never erase
the act record or hide a successful act.

**7. Partners never receive a worldview** (T-217's `Partner` audience): whatever network
an MCP identity connects from, its surface is this catalog — structurally, because the
worldview seam and the MCP seam share no route.

## Acceptance tests (codex's Round 4 list, verbatim commitments)

Two partners see the same class schema but cannot correlate instance handles; a revoked
handle never resolves; catalog serialization contains none of the three leak classes;
observing is refused by an act-only grant and invoking by an observe-only grant; opening
`allow_agent` alone grants nothing.

## Build order (after Ian's word — no code before it)

1. `crates/mcp` rung 2: the class catalog type + allowlist serializer + `discover_classes`
   behind the existing covenant door, compiled from declared surfaces (`declared`
   assurance only, v1).
2. The grant object + `request_grant`/`propose` with the typed partner-act ledger and
   narration (nothing invocable yet — proposals only).
3. `observe`, then `invoke`, each behind its own gate and Ian's per-grant word, with the
   revert discipline and bounds enforced kernel-side.

## What this ADR deliberately does not do

It opens no gate (ADR-0005 stands: only Ian opens). It ships no household record, masked
or otherwise. It grants nothing by default — a fresh install offers `attest` and an empty
catalog.
