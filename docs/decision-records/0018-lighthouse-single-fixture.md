# ADR-0018 — The lighthouse is the single permanent fixture; everything else is a peer

- **Status:** accepted
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
- **The escrow itself is now load-bearing** and is the least-tested part of this
  decision. Writing the restore procedure — and actually rehearsing it — is
  follow-on work, not something this ADR can assert is done.

## Follow-on work

- Reduce `wildhorse` to a covenant credential; confirm `can_mint()` is `false`.
- Write and **rehearse** the escrow restore procedure.
- Correct the six dataflow documents that still name a home hub — chiefly
  `dataflows/auth-and-membership.md`, which has it holding the group key.
- Fix the stale `FamiliarMac` comment in `ios/project.yml`, which still describes
  the Mac as reading a local daemon over loopback.
