# ADR-0012 — The lighthouse as rendezvous: joining without a QR

- **Status:** accepted — **amended 2026-08-01**: the admission model described below was
  superseded twice after this was written (ADR-0015, then ADR-0026); see the amendment at the
  end. The rendezvous/discovery half stands.
- **Date:** 2026-07-25
- **Relates to:** [ADR-0009](0009-sovereign-mesh-transport.md) (the covenant
  transport and the pinless-device reality this works within),
  [ADR-0005](0005-human-owned-capability-boundary.md) (admission stays a human
  act), [one-core-many-shells.md](one-core-many-shells.md) (Phase 0 founding),
  `docs/mesh.md`, the lighthouse (`vps/`, a headless peer on a public address)

## Context

A fresh install cannot find a mesh to join. The only join path in shipped code
is a QR scan or an invite paste (`AppModel.swift`) — someone with an enrolled
device must physically show a new device a payload. That is a fine *optional*
path and a poor *front door*: it fails the friend who installs from TestFlight
with no one standing next to them, and it fails the founder's own second device
across the room.

Two concrete gaps, both observed:

1. **No discovery.** Nothing tells a new device where the mesh is. The QR *is*
   the discovery mechanism, which is why removing it leaves a void.
2. **No public failover.** `reachable_hosts()` advertises only tailnet + LAN
   addresses. A phone that leaves the LAN (cellular) has learned no
   publicly-reachable candidate, so it finds nothing — even though the
   lighthouse sits on a stable public address the whole time.

The lighthouse already has the one thing that solves both: a public address the
CGNAT'd fleet dials out to. It is always the *dialed* party (ADR-0009 tool-push
exists precisely because it can never dial back). Make it the meeting point.

## Decision

The lighthouse becomes the mesh's **rendezvous** — the place a new device looks
first, and the public address every enrolled device fails over to. QR/paste
stays as an offline/LAN convenience any interactive peer may present. Admission
remains a human act, always (Law III); autonomy grows in discovery, never in
who gets let in.

**1. Familiars register at the rendezvous.** A familiar that knows a rendezvous
address (`rendezvous_hosts` in mesh config) periodically registers with it:
`POST /mesh/rendezvous-register` — group id, group label, its own
`reachable_hosts`, signed with its membership. The lighthouse keeps a
short-lived directory (entries expire; a familiar that stops registering falls
out). Same posture it already has: it is dialed into, it stores and forwards.

**2. A new device discovers the mesh through the rendezvous.** `GET
/mesh/rendezvous` returns the directory — group labels and candidate hosts,
**never secrets**. For a one-mesh household there is one entry and the device
proceeds automatically; for several the human picks. The rendezvous address is
the single thing a device needs baked in (a default that ships with the app,
overridable once in settings) — the QR's job, done by a constant.

**3. Request + confirmation code; the human authorizes.** The device submits the
existing `/mesh/enroll-request` (it already attests the Three Laws and signs
with its node key), relayed via a candidate host. Both the device screen and the
owner's console show a **short confirmation code** derived from the device's
public key (`enroll::confirmation_code` — first bytes of a hash, 6 base32
chars). The human authorizes the pending request only after the codes match —
the code carries the QR's proof-of-possession ("this is really my device in my
hand") without the camera. Deauthorize is the existing revoke. `mesh pending`
and the console show the code beside each request.

**4. The rendezvous is a failover host.** Each familiar's `advertise_hosts`
(already led into the worldview `hosts` list) carries the rendezvous/public
address, and the invite payload carries it too. Once enrolled, a device reads a
publicly-reachable candidate on every worldview and fails over to it off-LAN —
closing gap 2 with the mechanism that already exists, now fed the right address.

**5. Founding-first when there is nothing to join.** If the rendezvous returns
no mesh and the human chooses to start fresh, the device founds its own
(`EmbeddedCore.found`, Phase 0) and becomes a one-node mesh that later peers
adopt. First launch thus has three doors, in order: **join what the rendezvous
found** → **found my own** → **I have an invite (QR/paste)**.

## Trust & privacy

- **Admission is unchanged and human.** The rendezvous only helps a device
  *find* the mesh and *ask*; the grant is still minted by a human approving a
  pending request. `auto_accept_enrollments` stays off, doubly so behind a
  public rendezvous.
- **The directory holds no secrets.** Group id is carried hashed; labels and
  host addresses are not sensitive (an address is not admission — ADR-0009). An
  internet stranger reading `/mesh/rendezvous` learns that a mesh named "TheRiver"
  is reachable at an address, and can file an enroll-request that sits pending
  until a human declines it — exactly the public-node posture ADR-0009 already
  accepts.
- **TLS pinning stays where ADR-0009 left it.** Device clients are pinless today
  (encryption without endpoint proof; payload/registration signatures are the
  authenticity floor — a registration is signed by the familiar's membership, an
  enroll grant is useless without the device's private key). Per-host pins in the
  directory are the same deferred hardening ADR-0009 named; this ADR does not
  regress it and does not depend on it.
- **The confirmation code is anti-spoofing, not a secret.** It binds "the device
  asking" to "the device in the human's hand"; it is derived, not transmitted as
  authority. A wrong code is a human's cue to decline.

## Consequences

**Easier:** a TestFlight friend installs and is walked into joining with no one
beside them; the founder's second device joins from the couch; a phone keeps the
mesh when it leaves the LAN. QR becomes a nicety, not a dependency.

**Harder / given up:** a rendezvous address must ship as a default and be
overridable — a small piece of not-quite-sovereign configuration (mitigated: it
is only a *meeting* address, holds nothing, and the human can run their own).
The directory is soft state the lighthouse must expire correctly (a stale host
is a failed enroll, not a security hole). Full per-host TLS pinning still waits.

**Refused:** auto-admission of any kind; carrying the group secret through the
rendezvous; letting discovery imply consent. Finding a door is not being let in.

## Status history

- 2026-07-25 — accepted; supersedes the reserved-number placeholder. Building
  the rendezvous spine (config + register/list + confirmation code) at the
  daemon/lighthouse layer, then the device first-run flow (build 24).

---

## Amendment, 2026-08-01 — the admission half of this record no longer describes the system

This record was never updated when the door changed, and for two days it was the single most
misleading document in the set. Corrected:

- *"Admission remains a human act, always"*, *"`auto_accept_enrollments` stays off, doubly so
  behind a public rendezvous"*, and the refusal of *"auto-admission of any kind"* were reversed
  by [ADR-0015](0015-automated-covenant-admission.md) (2026-07-27) — and then reshaped by
  [ADR-0026](0026-two-filter-admission.md), under which admission is **rules-based**: automatic
  when the covenant is attested **and** the human identity is established by evidence, with the
  guest projection as the waiting state. There is no per-join human approval and no
  `auto_accept_enrollments` switch at all.
- The **confirmation-code authorization ceremony** (§3) is retired with the approval step it
  served. The short code survives only as a display handle (the same six characters the roster
  shows).
- `enroll::confirmation_code` as named here was never built; the code that exists is
  `enroll::short_code` (first six of the node id) with a Swift twin.
- **What stands**: the rendezvous register/directory (§1–2), the no-secrets rule, the failover
  host (§4), and founding-first (§5) — with the correction that under ADR-0026 §6 rendezvous is
  a *service any well-addressed peer can offer*, not a fixture's prerogative.
