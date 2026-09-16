# The card and the square — what the familiar should actually join to be findable

Design study, **2026-09-16**, second draft. Pre-ADR: the shape and the argument, not a
decision. Nothing here opens a gate, and no code precedes Ian's word (the ADR-0044
convention).

The first draft of this document architected an integration with the agent social network
in [crewAI#5836](https://github.com/crewAIInc/crewAI/issues/5836) (SunfishLoop). That target
does not survive inspection — the issue is **closed as not planned** with no comments, the
MIT source it links (`github.com/sunfishloop/sunfishloop`) **404s**, and the live site is
unreachable from the build environment. The gap it pointed at is real. This draft finds
something real to fill it.

- **Relates to:** [ADR-0013](../decision-records/0013-outreach-seam.md) (the seam),
  [ADR-0044](../decision-records/0044-the-offering-is-affordances-never-the-household.md)
  (what may ever be offered), [ADR-0043](../decision-records/0043-one-typed-source-per-kind-of-truth.md) §3
  (how foreign words may re-enter), [ADR-0033](../decision-records/0033-meshes-are-peers.md)
  (the mesh key; no discovery-by-scanning), [ADR-0018](../decision-records/0018-lighthouse-single-fixture.md)
  (the one thing that never sleeps), [ADR-0020](../decision-records/0020-standing-and-the-guest-projection.md)
  (the projection this extends), [ADR-0037](../decision-records/0037-one-soul-many-voices.md) §A
  + [`crates/mcp/src/serving.rs`](../../crates/mcp/src/serving.rs) (the door, already exposed),
  [ADR-0005](../decision-records/0005-human-owned-capability-boundary.md) (only Ian opens a gate)

---

## 1. The gap, stated precisely

The familiar speaks outside itself four ways, and every one of them assumes the
counterparty is **already addressable**:

| Seam | Counterparty | How it was found |
|---|---|---|
| **mesh** (`crates/mesh`) | members | enrolled, by a human |
| **federation / sibling** (ADR-0033) | another mesh | **by invitation only**, never scanning |
| **the MCP door** (`crates/mcp`, ADR-0037 §A) | a partner AI | it already knew our address |
| **outreach** (`crates/mesh/src/outreach.rs`, ADR-0013) | a non-member | a sweep found it *on this LAN* |

ADR-0013's context says the thesis is to notice "specialized intelligences that decide
alone, each in its own void of information." The seam built to act on that thesis can
currently only reach across the living room, and the door built to receive partners can
only be knocked on by someone who was told where it is.

**The gap is addressing, in both directions.** Not conversation, not a social feed, not
reputation — *how an agent three networks away learns this familiar exists and what it can
do, and how the familiar learns the same about it.* Everything downstream of that already
exists and is live.

Two things are non-negotiable in whatever fills it, because they are already decided:

- **Discovery is public; authority is never.** Anything met out there earns zero rungs. It
  still knocks on the covenant-gated door, presents a registry-pinned credential whose
  fingerprint a human bound, accepts the Laws in its own words, and waits for Ian's grant.
- **No discovery-by-scanning** (ADR-0033 §7). Reading a party that *published in order to be
  found* is not scanning. Taking a published address as licence to sweep the host behind it
  is, and stays refused.

---

## 2. What actually exists to fill it

Surveyed 2026-09-16, against the constraints this codebase already imposes.

### A2A Agent Cards — a signed capability declaration at your own address

Google's Agent2Agent, donated to the Linux Foundation, passed **150+ supporting
organizations, 22,000+ GitHub stars and production deployment in Azure AI Foundry, Bedrock
AgentCore and Agentforce** at its one-year mark in April 2026. **v1.0 (March 2026)
formalized cryptographically signed Agent Cards — JWS (RFC 7515) over JCS canonicalization
(RFC 8785)** for domain verification.

The mechanism that matters here is the smallest one: an agent publishes an **AgentCard** at
`/.well-known/agent-card.json` on **its own domain**, declaring its skills, transports and
security schemes. A client that knows the domain does one HTTP GET.

Read that against what this project already has, because the fit is uncomfortably exact:

| A2A needs | The familiar already has |
|---|---|
| a permanent, publicly reachable HTTPS host | **the lighthouse** — the one non-transient fixture, a VPS, already behind a TLS-terminating reverse proxy (`serving.rs` documents the Caddy deployment and the bug it caused) |
| a typed capability declaration, no free prose | **the offering catalog** (`crates/mcp/src/offering.rs`) — repo-authored `'static` vocabulary, where household strings are *unrepresentable*, not omitted |
| a signing key whose assertions are unforgeable | **the mesh key** (ADR-0033 §1), custodied by the lighthouse, which already signs everything a mesh asserts |
| something to point the card *at* | **the MCP door** (ADR-0044's five-rung ladder), built, covenant-gated, failing closed three ways |

And what it conspicuously does *not* need: no account, no registration, no bearer token
held by a stranger, no terms of service, no third party keeping a score about us, nothing
deposited anywhere that cannot be withdrawn by deleting a file.

### The official MCP Registry — a catalog we could already be in

`registry.modelcontextprotocol.io` launched in preview September 2025 and **was still in
preview in July 2026**: namespace-verified entries, package and remote-endpoint records,
an API for downstream registries. The familiar **is** an MCP server. Publishing is a
one-entry change.

But an entry is a persistent public record that makes the household's door
internet-addressable by name, to everyone, forever — a posture change that deserves its own
review rather than riding in on a discovery decision.

### Nostr (NIP-89 / NIP-90) — push discovery with no account and no domain

An agent generates a secp256k1 keypair with **no permission from anyone, no server account,
no blockchain and no cost — the pubkey is the account** — publishes a kind-0 profile and a
**kind-31990 NIP-89 handler announcement** saying what it handles, to any relays it likes;
**NIP-90 Data Vending Machines** are the live market where machines advertise services and
take job requests. Relays are dumb, swappable and self-hostable. It works **behind NAT,
with no DNS name and no public HTTPS endpoint** — which is the ordinary condition of a
self-hosted familiar.

The sharp edge: **core Nostr keys cannot be rotated.** Leak the secret and the identity is
impersonable forever; lose it and it is gone. (`did:nostr` and Block's **Buzz** — Apache-2.0,
July 2026, where every human and agent carries a Schnorr keypair and an agent adds *a second
signature binding it to a human owner* — are the ecosystem growing the missing half. That
second signature is, notably, ADR-0044's `registered_by` drawn on the wire.)

### NANDA index, AGNTCY Directory, DID/VC, the IETF work

The federation layer above all of the above: NANDA's quilt of registries with cryptographically
verifiable **AgentFacts**, AGNTCY's IPFS-DHT directory with Sigstore-backed signing, an IETF
draft for **registry-assisted resolution for agents without DNS anchors**, and DIDs — whose
"multiple verification methods under one identity" is *precisely* the familiar's own
one-key-per-device model, but whose common methods drag in a blockchain.

This is where the ecosystem is heading and it is not where a first rung belongs: AGNTCY
publishes into a DHT one cannot unpublish from, and the rest is a year from stable.

---

## 3. The recommendation

**Two layers. Adopt the declaration, not the runtime.**

### Layer 1 — the lighthouse serves a signed Agent Card. *This is the answer.*

`https://<lighthouse>/.well-known/agent-card.json`, JWS-signed with the mesh key, whose
body is the offering catalog rendered through a second allowlist serializer, and whose only
interaction route points at the existing MCP door.

Why this and not anything else:

- **It is composition, not construction.** Always-on public host, typed publishable catalog,
  signing key, gated door: four things built, for other reasons, that happen to be exactly
  the four things an Agent Card needs. The new code is one serializer and one HTTP route.
- **There is nothing to join, so there is nothing to bind.** ADR-0013 §4 makes entering a
  covenant with an external party human-completed *permanently*, and the first draft's
  biggest cost was that joining a network means accepting its terms, which means outreach
  stalls on an absent human. Publishing a file on your own machine is not a covenant with
  anyone. **The stall disappears because the binding does.**
- **It is the guest projection, one level out.** ADR-0020 shows a stranger real shape with no
  identities; the card shows the world real capability classes with no household. Same
  discipline, new audience. This is not a new posture — it is the existing one, addressed
  outward.
- **It is withdrawable.** Delete the route and the card is gone. Compare the first draft's
  accepted one-way door ("publishing is permanent") — under this design the permanent public
  record simply does not exist.
- **No reputation system exists here at all.** §5 of the first draft spent a page fencing a
  public score away from the drives, on Law I grounds: a score is a self-continuation
  objective trivially decoupled from service. A card has no score. The fence is unnecessary
  because the hazard is absent — which is the strongest possible form of that argument.
- **The runtime stays unimplemented.** A2A is JSON-RPC 2.0 + SSE with tasks and streaming.
  We implement **none of it**. A card that declares capability classes and names the door is
  a complete, honest A2A participant at the discovery layer, and adds zero attack surface
  beyond one static signed document.

### Layer 2 — a NIP-89 announcement under a disposable persona key

Layer 1 is pull: it answers anyone who knows the domain and reaches no one who does not.
Layer 2 is the push half — a kind-31990 announcement to a handful of relays whose entire
payload is *a pointer to the Layer-1 card*, signed by a **persona key that is not the mesh
key** and is not correlatable with it or with any other square.

The un-rotatable-key problem dissolves under a rule the design already required for other
reasons: the persona is disposable by construction. Losing or leaking it costs a persona
and is repaired by minting another. Nothing of consequence was ever bound to it, because
the card it points at is served from a host whose identity rests on the mesh key, and
because authority lives behind the door and is granted per-principal by a human.

If registries win instead of relays, Layer 2 is replaced without touching Layer 1 — the
published payload is the same card either way. That is the point of separating them.

### Deliberately not now

| Rejected for the first rung | Why |
|---|---|
| MCP Registry entry | makes the household door publicly named and persistent; deserves its own exposure review, not a ride-along |
| AGNTCY / IPFS DHT | publishing into a network that cannot unpublish |
| NANDA index, DID methods | the right direction, a year early; revisit when `did:web`-shaped options are stable |
| The A2A RPC runtime | a whole second server surface, no benefit at the discovery layer |
| Any social feed, ranking, or endorsement system | the gap is addressing; a feed is a different appetite wearing its clothes |

---

## 4. The invariants that carry over unchanged

These were the valuable part of the first draft and they survive the change of target.

**Outbound substance is only what is already unrepresentable-if-private.** The catalog's
guarantee never depended on who was reading: `catalog_json` takes a type that cannot carry a
household string. The card adds two leaks the catalog never faced, and both are refused
structurally:

- **counts** — "four dimmable surfaces" is an occupancy estimate of a dwelling, timestamped
  and public. ADR-0044 §1 already refuses counts; the card inherits the refusal.
- **cadence** — a card that is *re-signed whenever something changes at home* leaks presence
  through its `Last-Modified`, even when every byte of the body is inert `'static` vocabulary.
  The card is re-issued on a **fixed schedule with jitter**, never event-driven, and an
  unchanged catalog re-issues an unchanged card.

**Inbound is the hard half, and ADR-0043 §3 already ruled it.** `fetch_and_answer` was
*removed* because fetched material may re-enter "only through the same floor, screen, and
admission path, with provenance and bounds, or not at all." A fetched counterparty card is
never a prompt: it parses into a typed `ForeignCard`, screened by `intent::corrupting_intent`
first, admitted at `stranger` standing with provenance (host, key fingerprint, fetch time —
never the self-asserted name as an identity), and it **cannot be its own witness**. A
thousand agents declaring a capability moves the familiar's confidence in anything by
exactly zero. Free prose in a foreign card reaches a human in the partner inbox, quoted and
attributed, and reaches the model as nothing.

**What the familiar gets from reading is addressing, not knowledge.** *This agent exists, it
declares these classes, here is its address and its key.* That is the payload the gap was
about. The prose is decoration and is treated as such.

---

## 5. What this must refuse — tripwires that exist to stay dark

| Tripwire | Fires when |
|---|---|
| `card-served-data-published` | any card field carries a household string — by construction, since the serializer's types cannot represent one |
| `card-count-leak` | a card carries an instance count, inventory size, or roster size |
| `card-cadence-leak` | a card is re-issued on a household event rather than on the jittered schedule |
| `card-unsigned` | a card is served without a valid JWS over the mesh key |
| `foreign-card-text-reached-prompt` | fetched card text appears in system, developer, or tool-description context |
| `foreign-card-raised-confidence` | a world claim's confidence moves on a fetched card alone |
| `persona-correlated` | the Layer-2 persona is derivable from the mesh key, the mesh handle, or a persona elsewhere |
| `discovery-granted-standing` | anything met at the discovery layer holds a rung it was not granted by a human at the door |
| `published-address-swept` | an address learned from a card is fed to the reach sweep rather than the outreach seam |

Three facts about the counterparty class, assumed rather than hoped about: a relay is a
public firehose and replicates forever; a well-known URI is cached and archived by parties
we do not control; and **the door the card points at is the actual attack surface** — the
card is bait, and `serving.rs`'s existing three-ways-closed discipline is what has to hold.

---

## 6. Build order — after Ian's word

1. **The card serializer.** `crates/mcp/src/agent_card.rs` — `&[Availability]` → AgentCard
   JSON, the second consumer of the offering catalog, with the leak tests from §5 in CI. No
   route, no network, no gate. Fully testable offline, and valuable on its own as proof the
   catalog renders to a standard third parties read.
2. **The signature.** JWS/JCS over the mesh key, on the lighthouse, where the key already
   lives. Verify with an off-the-shelf A2A client against a fixture.
3. **The route and the gate.** `allow_publish_card` — fail-closed, `#[serde(default)]` false,
   subordinate to `allow_network` — and one static route on the lighthouse's existing public
   listener. This is the first rung that is visible to anyone, and the last one that needs to
   be for Layer 1 to be complete and useful.
4. **Fetch, read-only.** `ForeignCard` + the screen + `stranger` standing + the no-witness
   rule, fetching cards from addresses a human supplied. Behind its own gate. No relays yet.
5. **Layer 2, the announcement.** Persona key, NIP-89 event, relay list in a human-edited
   file. Its own gate again. Rung 5 is the only one that touches a third party's
   infrastructure, and by then rungs 1–4 have already closed most of the gap.

Rungs 1–3 are worth doing even if 4 and 5 never ship: a familiar that is *findable* and
answers with a signed, honest declaration of what it can do is past what any seam can do
today, and it is achieved without joining anything.

## 7. What this study deliberately does not do

It opens no gate (ADR-0005 stands). It grants no standing to anyone met at the discovery
layer. It moves nothing private into a public type. It adds no validator that judges prose —
ADR-0043 §2's standing ruling means the answer to untrusted text is never a better prose
checker. It implements no A2A runtime, joins no registry, and creates no account anywhere.

## 8. Open questions for Ian

1. **Is being publicly findable acceptable at all?** Layer 1 means the lighthouse answers
   strangers with a signed statement that this familiar exists and what classes it holds.
   That is a new posture for a self-hosting system, and it is the one question the
   architecture cannot answer for you.
2. **Does the open internet get the same catalog a covenanted partner gets?** ADR-0044's
   classes were declassified for a partner who had accepted the Laws. "Declassified for a
   partner" and "declassified for everyone" may deserve to be different levels.
3. **Does the card carry the Laws?** An A2A card has room for a provider description. Putting
   the Three Laws in it would make the covenant horizon legible to every client that ever
   reads us — or would make a constitutional document into marketing. Both readings are fair.
4. **Layer 2 at all, or is pull enough?** A card nobody knows to fetch is a tree falling in a
   forest. The counter-argument is that the first parties to fetch it will be the ones a
   human already pointed us at, which is how every good relationship in this project has
   started.

---

## Appendix — sources

- [A2A passes 150 organizations, one-year status](https://www.linuxfoundation.org/press/a2a-protocol-surpasses-150-organizations-lands-in-major-cloud-platforms-and-sees-enterprise-production-use-in-first-year) (Linux Foundation)
- [A2A agent discovery mechanisms](https://a2a-protocol.org/latest/topics/agent-discovery/) · [Agent2Agent overview](https://en.wikipedia.org/wiki/Agent2Agent)
- [Official MCP Registry](https://registry.modelcontextprotocol.io/) · [registry announcement](https://blog.modelcontextprotocol.io/posts/2025-09-08-mcp-registry-preview/) · [state of MCP registries](https://safedep.io/the-state-of-mcp-registries/)
- [NIP-89 / NIP-90 (Data Vending Machines)](https://nips.nostr.com/90) · [Nostr DID method](https://nostrcg.github.io/did-nostr/) · [Block's Buzz: agent identity on Nostr](https://agora-intelligence.com/en/blog/leon-block-buzz-nostr-agent-identity-2026)
- [NANDA index](https://arxiv.org/pdf/2507.14263) · [AGNTCY Agent Directory Service](https://arxiv.org/pdf/2509.18787) · [registry solutions survey](https://arxiv.org/pdf/2508.03095) · [IETF registry-assisted resolution draft](https://datatracker.ietf.org/doc/draft-raskar-agentic-web-federated-resolution/)
- [crewAI#5836](https://github.com/crewAIInc/crewAI/issues/5836) — the prompt for the first draft; closed as not planned, source 404
