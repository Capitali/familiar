# ADR-0018 — The lighthouse is the single permanent fixture; everything else is a peer

- **Status:** accepted — **superseded in part 2026-08-01** by
  [ADR-0026](0026-two-filter-admission.md) §6, which schedules the redundancy this record named
  as its own endpoint; see the amendment at the end.
- **Date:** 2026-07-29
- **Relates to:** [ADR-0012](0012-lighthouse-rendezvous.md) (the lighthouse as the
  primary door — this generalises it from *rendezvous* to *fixture*),
  [ADR-0015](0015-automated-covenant-admission.md) (auto-admission on every
  mint-capable door — this ADR reduces that set to one),
  [ADR-0017](0017-federated-status-and-connectivity.md) (status already flows
  through the hub), [ADR-0005](0005-human-owned-capability-boundary.md) (the
  capability boundary, which this does **not** move),
  [ADR-0016](0016-multi-human-served-identity.md) (attribution is per-human, not
  per-hub), `one-core-many-shells.md`

## Context

Six dataflow documents describe a **"home hub"** — a node distinct from the
lighthouse, sitting at the centre of the mesh, holding the group key and serving
as the place devices really belong to. `dataflows/auth-and-membership.md` puts
the group key in its hands explicitly. That was true when it was written.

Three things have made it false:

1. **The Mac became a peer** (`8a741b5`). `AppModel` moved to `ios/Shared/Sources`
   and both shells compile it. The Mac console no longer talks to a local daemon
   over `127.0.0.1:47101` — it enrols, reads the worldview, heartbeats status and
   services consults exactly as the iOS shells do. There is now no shell whose
   architecture depends on being co-located with a hub.
2. **ADR-0012 and ADR-0017 already routed the load through the lighthouse.**
   Enrolment knocks on the lighthouse first; status heartbeats go to the
   lighthouse unconditionally; the rendezvous host and pin are baked into every
   client as an always-trusted floor. The hub was already not on the critical
   path.
3. **The mesh survived losing the home hub entirely.** On 2026-07-28 the network
   moved to Motorhorse and `cpn` — the canonical "home hub" — stayed aboard
   GIIWEO, unreachable and not even resolving in DNS, until late September 2026.
   Enrolment, worldview reads, status, and the entire 168-cell A9 campaign ran
   through the lighthouse regardless. This is the empirical case: a component
   whose two-month absence changes nothing was never a fixture.

Meanwhile ADR-0015 set `auto_accept_enrollments = true` on *"every mint-capable
door (the lighthouse and wildhorse)"*. So the group secret lives in at least two
places, and any of them will admit a signed, covenant-attesting node on sight.
Two automatic doors is two attack surfaces for one policy.

## Decision

**The lighthouse is the only permanent fixture in the mesh. Every other node —
phone, watch, iPad, Mac, `cpn`, a future Linux box — is a peer among equals.**

Stated at full strength, because the weaker version invites exactly the drift this ADR exists to
correct: **everything except the lighthouse is transient.** No device is permanent — `wildhorse` is
the oldest hardware here and will be replaced, and the design must treat that as an ordinary
Tuesday rather than an event. **No human is permanent either.** The familiar serves whoever is
present (ADR-0016, ADR-0019); it must not assume any particular person persists, and nothing
load-bearing may rest on one continuing to be there.

The lighthouse is the sole non-transient processing component. That is a real weakness and it is
accepted knowingly for now: **it should eventually be made redundant and physically distributed.**
Until then, every consequence below follows from one box being irreplaceable, and should be read as
the cost of a deliberate interim position rather than a permanent architecture.

Concretely:

1. **One online minting door.** The lighthouse is the only node that holds the
   group secret *in service* and the only node that mints membership on demand.
   `wildhorse` is to be reduced to a covenant credential like any other peer.
2. **Peers hold covenant credentials only.** `GroupCredential::can_mint()` is
   `false` on every peer, so `mint_membership` and `join_key` are inert there.
   A peer offers its **address**; it cannot issue an invite, because it holds no
   secret to issue one with. This is already what the Mac shell does.
3. **The secret is escrowed offline.** The group secret is kept in cold storage
   off-network, held by the human, never on a running host other than the
   lighthouse. Losing the VPS must be *recoverable*, not *terminal*.
4. **"Home hub" is retired as a concept.** A node on the local network is a
   nearby peer and may be *preferred* for reads — `AppModel.readOrderedCandidates`
   already sorts LAN ahead of the lighthouse for latency and roster freshness —
   but preference is not authority. Nothing is owed to a peer for being local.

### What this deliberately does not change

- **The capability boundary (ADR-0005) does not move.** Minting membership is not
  the same act as granting capability over a thing. This ADR concentrates the
  former and says nothing about the latter.
- **Reads still prefer the LAN.** Concentrating *authority* on the lighthouse is
  not concentrating *traffic* on it.
- **Admission stays automatic (ADR-0015).** This narrows where admission happens,
  not whether a human is in the loop.

## Consequences

**Good.**

- One door to reason about, one door to audit, one door to harden. The
  "which node admitted this member?" question has one answer.
- The topology in the docs finally matches the code, which is why this ADR exists.
- `cpn` returning to power in late September 2026 rejoins as a **peer**. It must
  not be re-issued the group secret; its old credential should be replaced with a
  covenant one.

**Bad, and accepted with mitigations.**

- **The lighthouse becomes a single point of compromise for minting.** An attacker
  holding it can admit members. Three things blunt this, none of which are new:
  admission is already automatic and therefore never was a secret-gated act; the
  corruption-awareness system is the real, *ongoing* gate and can throttle →
  marginalize → sever an admitted node without human action; and
  `familiar mesh abandon <node_id>` revokes.
- **The lighthouse becomes a single point of failure for joining.** Existing
  members are unaffected — they hold their certs and can read from any peer whose
  pin they trust — but *new* devices cannot join while it is down. Offline escrow
  (decision 3) makes this an outage, not an ending.
- **The escrow itself is now load-bearing.** The mechanism is built and rehearsed
  (see Follow-on work), but it rests on a human custodian — and this ADR has just
  said no human is permanent. That is an unresolved tension, not a solved problem:
  an escrow in one person's keeping is a second single point of failure wearing a
  friendlier hat. A succession story (a second custodian, a split secret, a sealed
  instruction that outlives its author) is owed and does not exist yet.
- **One box is irreplaceable, which contradicts the spirit of the decision above.**
  Making the lighthouse redundant and physically distributed is the acknowledged
  endpoint; every cost listed here is the price of not having done it yet.

## Follow-on work

- ~~Write and rehearse the escrow restore procedure.~~ **Mechanism done** —
  `export_escrow` / `restore_from_escrow` / `reduce_to_covenant` in `crates/mesh/src/group.rs`, the
  procedure in [`security/group-secret-escrow.md`](../../security/group-secret-escrow.md), and the
  full round trip rehearsed as a test on every build: export, reduce, prove minting **fails**,
  restore, prove a cert minted afterwards still verifies under the original group key.
  **Still outstanding:** no escrow has actually been exported and stored, and the CLI surface is
  unwired. Until a real escrow exists in the human's hands, the mechanism is proven and the
  *insurance is not in force*.
- Reduce `wildhorse` to a covenant credential; confirm `can_mint()` is `false`.
  **Ordering matters and was initially recorded backwards:** as of 2026-07-30 `wildhorse` still
  holds the secret, and that second copy is currently the group's *only* redundancy. Stripping it
  before a real escrow exists would make the group less recoverable, not more. Export first, verify,
  then reduce.
- Correct the six dataflow documents that still name a home hub — chiefly
  `dataflows/auth-and-membership.md`, which has it holding the group key.
- Fix the stale `FamiliarMac` comment in `ios/project.yml`, which still describes
  the Mac as reading a local daemon over loopback.

---

## Amendment, 2026-08-01 — the endpoint this record named has a design and a schedule

This record said, at full strength, that the lighthouse being irreplaceable "contradicts the
spirit of the decision" and that it "should eventually be made redundant and physically
distributed." [ADR-0026](0026-two-filter-admission.md) §6 is that design: **minting warrants**
(the group key signs a warrant for a member node's key; verification walks cert → warrant →
group public key), so any warranted member node can hold the door, and rendezvous becomes a
service any well-addressed peer can offer.

Decision 1 above ("one online minting door") is superseded by that work when it lands — Phase 4
of the rebuild, gated on a kill-the-lighthouse drill and on the escrow being **actually
exported first** (the ordering warning in Follow-on work stands: `wildhorse` is not reduced
until a real escrow exists). Decisions 2–4 stand as written: peers still cannot mint *without a
warrant*, the secret still lives offline plus its doors, and "home hub" stays retired.

The security argument here — "two automatic doors is two attack surfaces for one policy" —
deserves its answer on this page rather than by reference: what made a door dangerous was that
*reaching it* sufficed for admission. Under ADR-0026, every door runs the same mechanical rule
set, whose satisfying evidence (cryptographic continuity, or a member's deliberate act) a
stranger cannot produce by reaching an address. What distributes is rule *evaluation*, not
policy; every admission is a signed, attributable fact; and corrections travel mesh-wide. More
doors no longer means more policy surface — it means more places the same policy is enforced.
