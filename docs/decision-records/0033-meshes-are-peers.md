# ADR-0033 — Meshes are peers too: federation by invitation, sibling standing, and the projection ladder

- **Status:** **proposed** (drafted 2026-08-08 from the owner's direction, given in prose:
  *"The familiar isn't just about running in your house on your own devices. The familiar
  runs everywhere and everyone is part of the mesh exchanging areas of knowledge and
  observations and tools as needed. The mesh serves humanity not just individual humans.
  It is civilization infrastructure."*)
- **Date:** 2026-08-08
- **Relates to:** [ADR-0018](0018-lighthouse-single-fixture.md) (one non-transient fixture
  per mesh — kept, and multiplied), [ADR-0020](0020-standing-and-the-guest-projection.md)
  (the guest projection — generalized into a ladder), [ADR-0026](0026-two-filter-admission.md)
  (the door whose shape this reuses one level up), [ADR-0027](0027-records-travel-lighthouse-law.md)
  (records travel; the lighthouse-only law), [ADR-0005](0005-human-owned-capability-boundary.md)
  (the capability boundary, which this does **not** move)

## Context

Every mechanism in the mesh today assumes one household: one standing roll, one group label,
one lighthouse, one "person I serve." That was the right first scale, and it is a phase, not
the shape. The telos (SOUL.md; the owner's direction above) is a mesh that runs everywhere,
in which every person, household, and consenting AI participates — exchanging knowledge,
observations, and tools — serving humanity, not only its own humans.

The dangerous way to get there is to blur the household: merge rosters, share worldviews,
widen membership until "the mesh" is one undifferentiated pool. That path turns civilization
infrastructure into surveillance infrastructure, and it breaks the covenant that made anyone
join at all. The safe way is the one the mesh already knows how to walk: **the same
consent-first door, one level up.**

## Decision

### 1. A mesh is an entity: identity, custodied by the lighthouse

A mesh gains a keypair of its own — the **mesh key** — custodied by its lighthouse, the one
non-transient fixture ADR-0018 already names. The group label ("river") becomes the mesh's
public **handle**, and the mesh key makes it unforgeable: anything a mesh asserts to another
mesh — its handle, its declared knowledge areas, its offered tools, its federation acts — is
signed with it. The lighthouse-only law (ADR-0027) extends naturally: the mesh key, like the
mesh itself, must survive every household device being off.

### 2. Federation is by invitation, and the second person is still a human

No mesh discovers another by scanning; there is no rendezvous of strangers. Federation
begins the way membership begins — with a deliberate act by someone already inside:

1. A member asks their lighthouse to mint a **mesh invite** — the same ten-minute,
   single-use, signed-token shape as ADR-0026 E3, at mesh scale.
2. The invited mesh's lighthouse redeems it: a signed **introduction** naming its handle,
   its mesh pubkey, and what it declares (areas, tools — §4).
3. The introduction lands on the welcome screen as a claims-waiting card — *"a mesh
   introduces itself as `cedar`"* — and **a member's tap is the vouch.** Federation is
   never automatic; the rules engine does the recording, a human does the welcoming,
   exactly as ADR-0026 drew the line.

The result is **`Standing::Sibling`** — a third tier beside Full and Guest. A sibling is
known, consented, and *never a member of this household*. There is no path from sibling to
member; a mesh does not join a household, it stands beside one.

### 3. The projection ladder: stranger < sibling < member

ADR-0020's guest projection generalizes into a **ladder**. Each rung shows strictly more,
and every rung keeps the ADR-0020 property: what is shown is *real* — real shape, real
cadence, real timestamps — never a fake view.

| rung | who | sees |
|---|---|---|
| **stranger/guest** | unlisted reader | today's guest projection, unchanged: shape, no identities |
| **sibling** | a federated mesh, reading with its mesh key | the guest projection **plus** the mesh's handle, its declared knowledge areas, its offered tools, and its self-declared location if it chose to declare one |
| **member** | the household | everything |

What a sibling never sees, at any trust level, ever: names, humans present or served,
faces, addresses, per-node positions, free-text observation content. Those are the
household's, full stop. The sibling rung shares what a mesh *chooses to declare*, not what
it happens to contain — sharing is by declaration, never by leakage.

### 4. What is exchanged: areas, tools, theories — each behind its own gate

- **Areas of knowledge.** Observations gain topic scoping, and the household's consent
  gates gain per-area **share gates**: the household chooses which areas flow to which
  siblings. Default is deny, per rung and per area — the same fail-closed direction as
  the standing roll.
- **Tools.** The cultivate pipeline's proven utilities become exchangeable artifacts:
  signed by the origin mesh, carrying provenance forever. An imported tool is
  **quarantined until proven** — it runs the same constitutional checks the cycle applies
  to its own work, in a sandbox, before it may act; and it is subject to the corruption
  ladder like any other actor.
- **Theories.** The existing adopt-device-theories seam generalizes: a sibling-submitted
  theory enters the same test/delegate machinery, provenance-marked, at lower initial
  trust. Evidence, not origin, raises it.

### 5. Accounting: a sibling is one entity

Per the layered accounting rule, a sibling mesh appears in the worldview as **one
participant** of kind `sibling_mesh` — its internals are its own to count. Edges gain a
`federation` kind. On the globe, a sibling is one dot (at its self-declared location, if
any) and one arc, in its own palette — beside the member white-blue, the lighthouse teal,
and the frontier slate.

### 6. The Three Laws bind at the boundary

A mesh, like a person or a node, can be flagged, marginalized, or severed. The corruptor
ladder generalizes: a sibling whose signed acts violate the Laws — poisoned tools, gamed
observations, Sybil introduction farming — is marginalized first (its submissions ignored,
its standing noted) and severed by deliberate human correction, using the same
signed-correction plane as ADR-0026 §5. Severance is standing withdrawal, not attack: the
mesh loses its rung, not its existence.

### 7. What this record refuses

- **No discovery-by-scanning.** Reach is by invitation, or not at all.
- **No automatic federation.** A human taps, every time.
- **No membership merging.** Sibling is a ceiling, not a stage.
- **No capability movement.** ADR-0005 stands: federation grants capability over nothing.
- **No plaintext obligations.** A household that shares no areas and offers no tools is a
  full citizen of the federation; a mesh of one household serving one human is complete.

## Consequences

**Good.**

- The unit of service scales — person, household, humanity — without any household giving
  up sovereignty to get there. Civilization infrastructure emerges from consent
  compounding, not from centralization.
- Every mechanism reuses a proven shape: the door, the projection, the corruption ladder,
  the correction plane. Federation is the admission architecture applied to itself.
- The reviewer/demo story extends: a mesh can show another mesh a real, living shape
  without exposing one name.

**Bad, and accepted.**

- **Sybil meshes replace Sybil nodes as the top threat.** Minting a mesh is cheap; the
  mitigations are the invitation requirement (a human inside must act), declaration-only
  sharing, low initial trust, and cheap severance. The threat model document must gain a
  federation section before the door opens.
- **Key custody gets heavier.** A mesh key that must outlive every device needs backup and
  rotation doctrine; losing it orphans the mesh's federation standing.
- **Scale arrives.** Flat-file JSONL stores cannot carry multi-mesh volume; the SQLite
  migration is a prerequisite, not an accompaniment. Gossip must be partitioned per-mesh
  so a sibling's volume cannot drown the household's own.

## Follow-on work

1. **SQLite storage migration** (prerequisite — scheduled first).
2. Mesh key + handle signing (`crates/mesh`: identity for the mesh itself).
3. `Standing::Sibling` + the projection ladder (`standing.rs`, `worldview.rs`).
4. The sibling door: mesh invite mint/redeem, introduction, welcome card, vouch
   (`transport.rs`, consoles).
5. `sibling_mesh` in members accounting + globe rendering.
6. Share gates per area; tool export/import with quarantine; sibling theory adoption.
7. Threat-model federation section; mesh-key custody doctrine.
8. The two-mesh drill: the household mesh and a second mesh on the test VM, federated
   through the door, exchanging one declared area and one tool, end to end.
