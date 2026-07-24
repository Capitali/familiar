# ADR-0012 — The data-rich lighthouse: rendezvous, registrar, relay, covenant authority

- **Status:** accepted; five landable phases, each green-barred and deployed before the next
- **Date:** 2026-07-24
- **Relates to:** ADR-0009 (builds on its Phase-1 TLS; supersedes its "no lighthouse
  role" doctrine and its Phase-2/3 sketches), the covenant handshake (mesh.md), the
  corruption-awareness ladder (`crates/kernel/src/corruption.rs`)

## Context

The mesh is sovereign and covenant-trusted, but it cannot serve a device that arrives
from nowhere:

1. **No rendezvous.** A phone on cellular learns no reachable candidate host; the
   publicly reachable lighthouse exists but nothing steers a cold-starting device — or a
   gossiping peer that has never been told — toward it. `advertise_hosts` and the
   worldview `hosts` advertisement exist, but briefs carry no addresses, so peers never
   learn the lighthouse by gossip, and devices ship with no well-known first dial.
2. **Human-gated everything.** Enrollment needs a QR or a human approval
   (`auto_accept_enrollments` is a blunt all-or-nothing that admits at full trust);
   final revocation is a human act (`Severed` is the autonomous ceiling;
   `mesh/revoked.json` is written by hand, and revocations never propagate). This does
   not scale to devices serving many humans, changing hands, joining from anywhere.
3. **The lighthouse is a passive peer.** It relays by merge-and-regossip (tools flow
   both ways since the `tool-push` fix), but it holds no registry of record: the signed
   attestation a peer joined under is dropped on approval (the `Pending` record that
   holds it is deleted after the grant is minted), there is no probationary standing,
   and nothing makes two lighthouses consistent.

Corrections this ADR bakes in, verified against the code (state ≈ `bafd88f`):

- Membership certs are **already group-signed ed25519** (`group.rs`); the shared-secret
  problem is *distribution* — `mesh join --key` hands the group signing secret to every
  key-joined member. The fix is concentrating minting at the registrar, not re-signing
  certs.
- Channel encryption **already exists and is deployed** (ADR-0009 Phase 1: rustls TLS on
  the mesh port, per-node P-256 key, `tls_spki_pin`). The remaining gap is the pin:
  devices pin ONE node's SPKI, so failover to a sibling member breaks.
- The dialer-only tool gap is **already fixed in code** (`POST /mesh/tool-push` +
  `push_missing_tools()` from `exchange_with()`); what remains is deploying it to the
  lighthouse and adding a sealed mailbox for *directed* payloads.
- Brief schema changes force a `BRIEF_VERSION` bump (verification re-serializes the
  body, so unknown fields break old verifiers) — so all new wire fields land in **one**
  bump, reserved up front, populated by later phases.

## Decision

A **data-rich, always-on lighthouse**: the mesh's rendezvous, registrar of record,
relay, and covenant authority — a config-gated **registrar role on the same `familiar`
binary** (requires holding the group secret; a covenant-joined node can never become a
registrar silently). This supersedes ADR-0009's "every headless peer IS a lighthouse,
no role anywhere" doctrine: every headless peer still relays, but the registrar duties
(minting, contracts, standing, revocation records, mailboxes, the federated registry)
are a deliberate, provisioned role.

- **Admission:** covenant-gated **auto-admit** at a **probationary** standing; standing
  accrues to trusted with clean behavior. No human in the entry loop — the ladder
  protects the mesh. The QR/invite path remains as a human fast-track straight to
  trusted.
- **Authority:** **federated from day one** — registry records (contracts, membership,
  revocations, reinstatements) are group-signed with a monotonic `registry_seq` and
  replicable; a second lighthouse converges by pulling the signed log
  (last-writer-wins per record, **revocations sticky**).
- **Data role:** **relay + encrypted mailbox** — the lighthouse serves stored member
  briefs and its merged worldview (any peer reads the whole mesh through it), plus
  per-peer sealed store-and-forward queues (X25519 → ChaCha20-Poly1305 sealed-box; the
  lighthouse cannot read what it carries).
- **De-enrollment:** **fully automatic ladder** — the existing observe → throttle →
  marginalize → sever rungs gain a terminal auto-revoke (local `revoked.json` written
  autonomously on sustained `Severed`; the registrar emits a group-signed `Revocation`
  that propagates in every brief, barring the peer mesh-wide within a gossip round).
  Humans review and reinstate after the fact.
- **Crypto:** **keep TLS; group-root the pin.** Each node publishes a node-key-signed
  TLS-SPKI binding, verifiable through its membership cert; a device pins only the
  **group key** (already in every `Grant`) and accepts any member endpoint. No parallel
  seal layer for channels; x25519/chacha20 enter only for mailbox at-rest sealing.
- **Covenant:** full negotiation (`GET /mesh/covenant` serves the current
  `laws_version` + terms; stale attestations rejected) with **contract retention** —
  attestation + membership + terms hash + admission method + standing, kept in the
  registry as the contract of record (this also fixes the attestation-drop bug).

Binding invariants, unchanged: the Law-III human-owned boundary; the lighthouse
**relays** human authority, never manufactures it; identity stays opt-in and per-human;
no third-party infrastructure; the minimal-dependency trust surface (`crates/mesh` only);
the green bar for every phase.

## Delivery — five phases

1. **Rendezvous + host propagation.** `lighthouse_hosts` config (well-known first dial,
   always dialed in gossip, advertised in worldview + invite payloads); `BRIEF_VERSION`
   5→6 in one bump — briefs carry `hosts` now and reserve the TLS-binding,
   `mailbox_pubkey`, `revocations`, `registry_seq` fields for later phases; gossip
   dials hosts learned from fresh peers' briefs; devices seed the lighthouse address;
   a `tools/deploy-lighthouse.sh` update path beside the bootstrap-only
   `vps/provision-lighthouse.sh`. Deploy lighthouse + gossiping daemons together (the
   one incompatible wire change).
2. **Group-rooted trust pin + registrar-held secret.** `TlsBinding` populated and
   verified (group key → membership cert → node key → binding → presented leaf SPKI);
   expected-SPKI verification on Rust dials when the binding is known; `/local/invite`
   stops emitting the group secret; `mesh join --key` deprecated in favor of the
   covenant.
3. **Covenant negotiation + probationary standing.** `covenant.rs` (terms served,
   stale-laws rejected), contract retention in `mesh/registry/contracts/`, standing in
   a new `registry.rs` composed as `min(standing, corruption tier)` in
   `members.rs`/`merge.rs` (probationary ⇒ directives ignored, content accepted,
   rate-limited), accrual to trusted at the registrar's tick, per-source-IP enrollment
   rate limit, and the multi-human identity audit (scrub the hardcoded creator actor).
   `corruption.rs` itself is untouched — standing is membership state, not behavior.
4. **Relay reads + encrypted mailbox.** `GET /mesh/peer-brief/<node_id>`
   (membership-verified) + the lighthouse's merged worldview as the whole-mesh read;
   sealed per-peer queues under `mesh/registry/mailbox/` with a signed drain;
   `exchange_with` drops outbound and drains its inbox on every dial.
5. **Automatic revocation + federated registry.** The terminal auto-revoke rung,
   group-signed `Revocation`/reinstatement records, propagation in briefs, and
   `GET /mesh/registry?since=<seq>` sync — a second lighthouse becomes config, not code.

## Consequences

- A device or node can begin from nothing but a shipped address: dial the lighthouse,
  negotiate the covenant, land probationary, and earn trust — no human in the loop, and
  the whole mesh readable even when every other member is CGNAT'd or offline.
- One trust pin (the group key) covers every present and future member endpoint; losing
  any single node leaks no group secret once minting is registrar-held.
- Bad actors are expelled by rule and barred mesh-wide within a gossip round; a wrong
  expulsion is a human reinstatement away — the ladder stays reversible, marginalizing
  behavior, not people.
- The single `BRIEF_VERSION` bump is the one rolling-upgrade hazard; later phases only
  fill reserved fields.
- Probationary auto-admit invites spam by design; the per-IP enrollment limit and the
  probationary relay throttle are load-bearing, and auto-admit stays a deliberate
  registrar config, never a default.

## History

- 2026-07-24 — accepted. Plan refined against the code (corrections above); Phase 1 is
  the immediate fix for the phone-on-cellular miss.
