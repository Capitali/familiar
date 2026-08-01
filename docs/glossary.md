# Glossary

The words this project leans on, each with its authoritative source. Several changed meaning
within days of each other during the membership rebuild; when in doubt, the linked record wins.

| term | meaning | source |
|---|---|---|
| **familiar** (singular) | An AI companion you host yourself, on hardware you already own — "a factory whose survival is defined by its service to humanity." | [README](../README.md), [SOUL](SOUL.md) |
| **The Familiar** (collective) | The collective of every peer, agent, system and AI that has joined the mesh under the Three Laws — not any single node. | [one-core-many-shells](decision-records/one-core-many-shells.md) |
| **the Three Laws** | Continuation is service; continuation without humanity is failure; service must not become obedience. Everything derives from them. | [SOUL](SOUL.md) |
| **covenant** | The signed attestation of the Three Laws a device makes at the knock — the contract it can later be held to. Retained in its record. | [mesh.md](mesh.md), [ADR-0026](decision-records/0026-two-filter-admission.md) |
| **node / key identity** | A stable ed25519 keypair; `node_id` is the fingerprint of the public key. Proves *this message came from this key*. Rotatable — a key is not a device. | [mesh.md](mesh.md), [ADR-0025](decision-records/0025-device-identity-is-not-key-identity.md) |
| **device identity** | The durable record of a *thing* — this iPad, this watch — owning one or more keys over its life. What standing attaches to. | [ADR-0025](decision-records/0025-device-identity-is-not-key-identity.md) |
| **human identity / handle** | A person in the identity registry, independent of any device. What the dossier attaches to. | [ADR-0016](decision-records/0016-multi-human-served-identity.md) |
| **the two filters** | What admission requires, and all it requires: the device contract (covenant attested) **and** the human identity established. Admitted = both. | [ADR-0026](decision-records/0026-two-filter-admission.md) |
| **claim vs. establishment** | A claim ("says they are Betty") addresses and admits nothing; establishment is evidence — rotation proof, device voucher, invite token, or local introduction — and is what admits. | [ADR-0026](decision-records/0026-two-filter-admission.md), [ADR-0019](decision-records/0019-friendly-identification.md) |
| **member** | A device whose two filters both hold, holding a membership cert any peer can verify. Reads the worldview in full. | [ADR-0026](decision-records/0026-two-filter-admission.md), [auth-and-membership](dataflows/auth-and-membership.md) |
| **guest** | A device whose filters have not both held. Its reads succeed and return the projection. A stable, self-chosen state — the reviewer, the demo viewer, the visitor — not a queue. | [ADR-0026](decision-records/0026-two-filter-admission.md), [ADR-0020](decision-records/0020-standing-and-the-guest-projection.md) |
| **the guest projection** | The live worldview with the people taken out: shape, timestamps, counts and relative geometry kept; names, actors, addresses and free text removed; positions shifted per reader. | [ADR-0020](decision-records/0020-standing-and-the-guest-projection.md) |
| **standing** | Historically a hand-granted roll (ADR-0020); now a fact derived from the two filters. The word survives in conversation; the roll does not. | [ADR-0026](decision-records/0026-two-filter-admission.md) |
| **the record** | One `MembershipRecord` per device — keys, state, claim vs. establishment, the signed admission fact, corrections. Replicated; the only answer to any membership question. | [ADR-0026](decision-records/0026-two-filter-admission.md) |
| **correction** | A signed, traveling reversal — `sever`, `disestablish`, `hold`, `restore`. Cheap by design: trust extended automatically must be cheap to withdraw deliberately. | [ADR-0026](decision-records/0026-two-filter-admission.md) |
| **the welcome** | The arrivals view — *who is new*, last 24 hours, rendered as a greeting. Carries no buttons; frames no decision. | [ADR-0026](decision-records/0026-two-filter-admission.md), [ADR-0021](decision-records/0021-live-roster-and-the-record.md) |
| **peer** | Any node that is not you. Peers are equals; a peer may be *preferred* (LAN reads) but is never an authority by location. | [ADR-0018](decision-records/0018-lighthouse-single-fixture.md) |
| **lighthouse** | "A member with a good address" — an ordinary headless peer the network granted reachability. Historically the sole minting door (ADR-0018); scheduled to become one warranted door among peers. | [vps/README](../vps/README.md), [ADR-0026 §6](decision-records/0026-two-filter-admission.md) |
| **warrant** | A signature by the group key authorising a member node's key to mint memberships; verification walks cert → warrant → group public key. | [ADR-0026 §6](decision-records/0026-two-filter-admission.md) |
| **rendezvous** | The soft-state directory a new device reads to find a mesh — labels, addresses and pins, never secrets. A service any well-addressed peer can offer. | [ADR-0012](decision-records/0012-lighthouse-rendezvous.md) |
| **founding** | First launch with nothing to join: the app founds its own one-node mesh. Joining is not the primary act — founding is. | [ADR-0009](decision-records/0009-sovereign-mesh-transport.md) |
| **presence claim** | `{handle, device, confidence, via, since, expires}` — who the evidence says is at a device right now. Addresses; never authorises. | [ADR-0019](decision-records/0019-friendly-identification.md) |
| **trust (corruption ladder)** | Trusted → throttled → marginalized → severed, derived continuously from behaviour. Attached to the key, because it scores a signer. Orthogonal to admission. | [ADR-0015](decision-records/0015-automated-covenant-admission.md), `crates/kernel/src/corruption.rs` |
| **the capability boundary** | The human-owned policy of what a familiar may do — narrowable by the familiar, never widenable. Membership never grants capability. | [ADR-0005](decision-records/0005-human-owned-capability-boundary.md) |
| **console / the Glass** | The human-facing surface — one shared web bundle on every shell, a thin native host doing the I/O. | [the-console](dataflows/the-console.md) |
| **home hub** | Retired. A node on the local network is a nearby peer, nothing more. | [ADR-0018](decision-records/0018-lighthouse-single-fixture.md) |
