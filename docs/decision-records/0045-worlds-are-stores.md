# ADR-0045 — Worlds are stores

- **Status:** **accepted** — Ian, 2026-08-23, verbatim: "Move forward adr-0045."
  (Design decided in the ships-computer dialogue,
  docs/reviews/2026-08-21-ships-computer-dialogue.md Rounds 1-3 — codex's Round 2
  absorbed nearly whole. Amends ADR-0037 §B's "partition at
  the record" by reference: the record-level `world` field is dead before it was built)
- **Date:** 2026-08-21 (proposed) · 2026-08-23 (accepted)
- **Relates to:** [ADR-0037](0037-one-soul-many-voices.md) (Purr, the persona seam, the
  MCP transport revision — §B amended here), [ADR-0043](0043-one-typed-source-per-kind-of-truth.md)
  (kinds of truth have kinds of addressee; producer + addressee completeness),
  [ADR-0044](0044-the-offering-is-affordances-never-the-household.md) (the grant ladder
  this composes with), [ADR-0035](0035-the-pact.md) (the game exclusion, held),
  [ADR-0005](0005-human-owned-capability-boundary.md) (one human-owned boundary),
  ADR-0042 (the station ceremony's human acts, reused; its topology, rejected), T-205

## Context

The game needs Purr — a ship's-computer persona of the familiar inside UCF's fictional
world — and T-205 recorded the load-bearing safety requirement: *"a fleet of happy
captains must never be able to raise the number that says the familiar is serving
humanity."* Two partition designs ended up on the books at once: T-205 specified a
`world` FIELD on observations, threads, questions, and dossier contributions; the persona
seam's own doc said *"the world partition is the data dir."* A task that can be built two
ways will eventually be built both ways. This ADR makes one of them the design.

## Decision

**1. The partition is the data dir. The `world` field dies unbuilt.** A ship instance is
its own store: own persona, own declared surfaces, own observation log, own reasoning
cadence, own keys, own process lifetime. No ordinary truth-bearing record anywhere
carries a `world` discriminator — a filter on a shared store is an exclusion that
eventually acquires one forgotten reader; a store the household engine holds no handle to
cannot be read by construction. The household's law signals need no `WHERE world='real'`
clause because there is nothing else in the store they scan.

**2. Crossings are typed envelopes carrying provenance — never records.** A stable,
opaque `WorldInstanceId` lives in the provisioning registry; every bridge envelope
carries instance id + source key + grant epoch + schema version + event id. That
metadata authenticates, deduplicates, revokes, and routes a message that has
deliberately left its isolated store; it is not a filter that makes mixed data safe.
Isolation is stronger than `&Path` discipline: the ship process receives an opaque store
capability (or an OS sandbox exposing only its root), and bridge code accepts typed
envelopes, never paths.

**3. The bridge is narrow and two-directional — but inward is control-plane only.**
Outward: `AttentionNotice` (low stores, a completed trade), with event ids,
observed/sent times, expiry, and supersession. Inward: exactly the human acts that
create and end the relationship — `CommissioningBundle`, `GrantUpdate`,
`BoundaryNarrowed`, `Rename`, `Decommission` — never household observations or
biography. A fresh Purr is unaware of the household; it is not unrevocable.

**4. A report is evidence that Purr reported, never evidence of the payload.** The
household may cite a bridge RECEIPT for "commissioned ship X reported low water at T" —
authorship, time, and delivery are real events. It may never promote the payload into
household truth, dereference ship evidence ids, or let it touch a theory, dossier,
presence, capacity, or service signal. Most notices address the captain's console, not
the household muse — delivery through a household-owned surface does not make a message
household reasoning material (ADR-0043 §5).

**5. One constitutional ceiling; leases only narrow.** Purr owns no independently
editable boundary. Three layers: the one human-owned root boundary; a signed, EXPIRING
projection of it available to the ship process; ship-local per-capability grants that
can only narrow. Stale, malformed, or narrowed-past-lease → consequential calls fail
closed. Every UCF act is checked twice for different truths: the real effect channel
against the root boundary (a "fictional" thrust command is still a real network call),
and the game capability against the captain's grant. Boundary truth is shared;
authority is instance-scoped.

**6. The familiar is a partner at UCF's door, held to its own ADR-0044 standard.**
Discovery metadata is a claim: a local typed declaration maps server identity + pinned
tool schema/version to an effect class before the metabolism may call; schema drift
closes the tool until reviewed. UCF v1's ten read-only tools classify as observation;
**its rung-5 act set is empty** — nothing becomes callable by appearing in discovery.
When mutations exist, effects (not tool names) are classified, and standing assent is a
`CaptainGrant` envelope: capability class, commodity/route sets, per-act and cumulative
bounds with atomic budget reservation, freshness requirements, expiry. UCF's quote is
evidence for whether a trade serves the captain, never authority to trade.

**7. Purr's voice rides the one reply road** (ADR-0043 §3) in the ship store: admission,
cites, unauthorable law text, the durable exchange — persona alters the mask, never the
authority. The captain-turn MCP tool is named **`purr.hear`** (a typed captain turn;
never caller-authored Purr prose, never instructions). Unprompted speech is a typed
`Announcement` through the same seam — no fabricated request/answer pair. The
commissioning bundle carries the constitution/version HASH and fails commissioning on
mismatch; the Laws keep one source.

**8. Purr is neither household member nor sibling mesh.** Its own cryptographic
principal; typed grants only (MCP client to UCF; audience-scoped resources to the
captain's console; commissioned partner to the household when the captain asks). No
group certificate, no worldview, no record sync, no gossip, no federation. The station
ceremony's HUMAN ACTS — name, commission, associate, correct, revoke — are reused; its
topology is not. The household keeps a minimal `WorldInstance` provisioning record
(instance pubkey, label, commissioner, endpoint, lifecycle, active grant ids) — a record
of a real software relationship, never a copy of the ship world or a dossier of play.

**9. Decommission revokes authority; it never silently destroys history.** Keys and
grants end immediately and the process stops; the store's fate — archive, export, or
delete — is an explicit human retention act. A grant epoch is authority, not identity.

## The decisive test (hostile, from the dialogue)

Seed household-only sentinels in every record class; run the complete ship cadence and
replies; prove no ship output or store contains them. Seed ship-only sentinels; prove no
household muse, dossier, service signal, question, or capacity reads them. The console
world-switcher queries the ship's audience-scoped resources directly and never renders a
blended worldview.

## Build order (accepted with this ADR; step 7 additionally gated)

1. T-205 rewritten against this ADR (done with this commit — the task now has one
   buildable meaning).
2. The partition + hostile sentinel tests, before any game ingestion.
3. The MCP contract v2 with the game team (`purr.hear` semantics; pinned read-only UCF
   declarations).
4. One `WorldInstance` provisioned through the commissioning ceremony — no mesh
   membership, no automatic deletion.
5. The ship world's read-only UCF cadence + the captain's direct console view; only
   typed attention notices cross outward.
6. Captain turns and Purr announcements through the shared admission/rendering seam.
7. Propose/observe/invoke grants — only under accepted ADR-0044 machinery AND a real
   mutating UCF schema. Not part of the v1 fiction.

## What this ADR deliberately does not do

No gate opens. No game datum enters a household store, masked or otherwise. The Pact's
structural exclusion (ADR-0035) stands unchanged beneath all of it.
