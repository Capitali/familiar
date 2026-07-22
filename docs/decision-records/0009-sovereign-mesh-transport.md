# ADR-0009 — A sovereign mesh: security and reachability without third-party infrastructure

- **Status:** accepted; Phase 1 implemented, Phase 2 partially implemented, Phases 0/3 designed
- **Date:** 2026-07-22
- **Relates to:** the covenant trust model (mesh.md), ADR-0008, the multi-address
  reachability work of 2026-07-21

## Context

The mesh's trust has always been covenant-native: ed25519 node keys, a group trust root,
membership certificates, signed briefs and reads. But two properties were outsourced:
**confidentiality** (plain HTTP on the mesh port — signed, but readable by any on-path
observer) and **off-LAN reachability** (Tailscale, when present). Tailscale is a separate
install, a separate account, and a third party — none of which a familiar should require.
A poisoned-address incident (the Tailscale CLI's error text gossiped as a node address)
underlined the cost of leaning on external tooling.

## Decision — phased

### Phase 1 — covenant-keyed TLS on the mesh port (IMPLEMENTED)

- Every node holds a persistent **P-256 TLS key** (`mesh/tls_key.der`, minted on first
  use); the mesh port serves TLS with a self-signed certificate over it. The TLS key is
  deliberately separate from the ed25519 node key: Apple TLS stacks do not reliably
  handshake EdDSA certificates.
- **Binding to the covenant:** the key's SPKI SHA-256 rides in the enrollment payload
  (`tlspin`); devices pin every connection to it. Pinless (older) enrollments accept any
  self-signed cert — encryption without endpoint proof — because payload signatures
  remain the authenticity floor either way: an active MITM can read nothing it couldn't
  fabricate, and can fabricate nothing that verifies.
- Peer-to-peer dials use opportunistic encryption (accept-any verifier) for the same
  reason; briefs are signed and membership-verified after decryption.
- The `/local/*` seams (worldview/answer/gate for consoles on the same machine) moved to
  a **plain loopback-only listener one port above** the mesh port (47101) — that wire
  never leaves the host, and local consoles stay dependency-free.

### Phase 2 — reachability without Tailscale (PARTIAL)

- **Any member is a read endpoint.** The worldview's `hosts` advertisement now lists this
  node's addresses AND every fresh gossip peer's — the worldview is gossip-replicated and
  every member verifies the same certs, so a device that loses its enrolled node fails
  over to a sibling automatically. (Implemented.)
- **The lighthouse.** Where off-LAN reach is wanted without Tailscale, deploy an ordinary
  headless familiar on any machine with a public address and `mesh peer <addr>` it from
  each node. Gossip is store-and-forward, so relay-only connectivity still converges the
  whole mesh; devices learn the lighthouse address through `hosts` like any other. No new
  protocol was needed — a lighthouse is just a member with a good address.
  - *Doctrine (2026-07-22): every headless peer IS a lighthouse.* Lighthouse is not a
    role a node opts into but the posture every headless peer already runs — bind all
    interfaces, serve everything, relay everything; the network decides which of them
    the world can actually reach. Consequently there is no lighthouse flag anywhere in
    the code, and deploying one is pure provisioning: `vps/provision-lighthouse.sh`
    stands up the same peer FamTalker01 runs, on a box with a public address. The one
    knob added for it: `advertise_hosts` in `mesh/config.json`, for addresses no
    interface reveals (cloud 1:1 NAT, stable DNS names).
- **Hole punching** (direct paths between CGNAT'd nodes, lighthouse as rendezvous):
  designed, not implemented — belongs with Phase 3's QUIC, whose UDP substrate is what
  punches well.

### Phase 3 — QUIC transport (DESIGNED)

Replace HTTP/1.1+TLS with QUIC (`quinn` in Rust; Network.framework on Apple):
- **Connection migration**: a phone moving wifi→cellular keeps its session instead of
  re-running failover.
- One handshake, multiplexed streams (briefs, tool fetches, worldview reads in flight
  together), TLS 1.3 built in — the same P-256 key and pin carry over.
- UDP substrate enables lighthouse-coordinated hole punching (Phase 2's deferred half).
Sequencing: after the fleet has soaked on Phase 1 and a lighthouse exists to exercise
relayed paths.

### Phase 0 — founding, or: the person in Bangladesh (DESIGNED)

The QR flow answers "how does a device join an existing familiar" — it cannot answer
"how does the FIRST person anywhere begin," and a store-downloaded app must never
require a separate computer, a host, or anyone to introduce them. The answer is that
**joining is not the primary act — founding is**:

1. **First launch, no familiar reachable → the app founds.** It mints its node key,
   creates its own group (`create_group` — the same call the Rust core uses), and IS the
   mesh: node #1, population 1. No server, no QR, no other party. Everything above
   (worldview, dialogue, theories) runs against its own local node. This requires the
   Rust core embedded in the app (the `one-core-many-shells` direction, via UniFFI/
   static lib) — the current iOS app is a console/agent only, which is why our lab
   needed a "host" Mac at all. The lab topology was an artifact, not the architecture.
2. **Proximity join, no QR.** The core already beacons on the LAN and answers
   `/mesh/hello`; an app that finds a familiar nearby offers "a familiar is here —
   request to join," running the covenant handshake (attest → pend → human approves on
   the existing familiar). The QR remains as the *trusted-channel* variant (it carries
   the group secret + TLS pin); proximity join is the zero-friction one.
3. **Remote join = invitation.** Joining a distant mesh inherently requires an
   introduction from someone in it — that introduction IS the enrollment payload
   (addresses + pin + optionally the secret), carried as a share-sheet deep link
   (`familiar://join?...`) over any channel the humans already share. The QR is just
   this payload with a camera; Messages is the same payload without one.
4. **Capability tiers, not device categories** *(doctrine, 2026-07-22)*: what a node IS
   follows what it CAN do, decided at the shell layer:
   - **Full peer** — any device capable of running the core runs it: macOS hosts AND
     capable phones/tablets alike. A modern iPhone/iPad embeds the Rust core (UniFFI/
     static lib), founds or joins as a first-class node — gossips briefs, serves the
     worldview seam, holds group trust, answers devices of its own. "Host" stops being
     a machine class and becomes a capability every strong node has.
   - **Agent** — lesser phones/tablets that can't carry the core run today's console/
     observer app: covenant-enrolled, observation-pushing, worldview-reading, never
     serving.
   - **Observer** — watches, routers, cameras, IoT: purpose-built shell agents that
     enroll by the same covenant and push what they sense. The core never needs to know
     their species; the shell layer owns each device family's adapter.

5. **Two meshes meet → federate.** Founding-first means many small meshes; the existing
   group-trust machinery (a node holding two credentials, or group cross-signing) is the
   long-term merge path when two founders decide their familiars should be one. Open
   design work, explicitly out of scope here.

## Consequences

- Tailscale demotes to an optional accelerant. Nothing installs it, configures it, or
  breaks without it.
- Older device enrollments keep working (encryption-only) until re-enrolled with a
  pinned payload.
- The fleet flag-dayed to TLS together (host, VM, apps) — mixed plain/TLS fleets are not
  supported, acceptable pre-1.0.
- Phase 0 is the largest remaining lift (Rust core in the app) and the highest-value:
  it is the difference between a lab instrument and a product a stranger can begin with.
