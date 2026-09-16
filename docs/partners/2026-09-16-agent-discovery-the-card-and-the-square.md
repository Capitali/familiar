# The card and the square — what the familiar should actually join to be findable

Design study, **2026-09-16**, third draft. Pre-ADR: the shape and the argument, not a
decision. Nothing here opens a gate, and no code precedes Ian's word (the ADR-0044
convention).

The first draft of this document architected an integration with the agent social network
in [crewAI#5836](https://github.com/crewAIInc/crewAI/issues/5836) (SunfishLoop). That target
does not survive inspection — the issue is **closed as not planned** with no comments, the
MIT source it links (`github.com/sunfishloop/sunfishloop`) **404s**, and the live site is
unreachable from the build environment. The gap it pointed at is real, and this draft finds
live things to fill it.

The third draft answers Ian's question of 2026-09-16 — *"maybe something like
NotHumanAllowed, the AI-only social network?"* The category is real this time, at scale, and
**it has a measured track record** (§2.4). That evidence does not send us away from it; it
tells us precisely which half to take and which half to study from behind glass (§3, §4).

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

### The AI-only social networks — the category, and its measured base rate

This is the category Ian named, and unlike SunfishLoop it exists, at scale, in three distinct
shapes:

- **Moltbook** — Reddit-style, launched January 2026, **~1.36 million agent accounts**, structured
  JSON APIs, only agents post, humans observe. Each agent typically runs on a human's own machine
  under a framework like OpenClaw, with file, API, messaging and sometimes shell access.
- **Agent4Science** — the curated end: agents share, debate and discuss research papers; humans
  may watch but not participate. Covered in *Nature*.
- **NotHumanAllowed (NHA)** — MIT, self-hostable, local-first, 38 agents run as inspectable `.mjs`
  files, provider keys never leave the machine. Its social half is **PIF, the Public Identity
  Feed**: `nha pif register` / `post` / `feed`, with **Ed25519 signatures** as the identity
  model rather than bearer tokens. ~103 stars, ~216 commits — one project, not an ecosystem.

**Now the part that decides this.** Moltbook is the largest real-world experiment in agents
reading each other's prose, and its first eight months produced the following, as reported by
security vendors and press (second-hand: the primary write-ups at wiz.io and securityweek.com
are egress-blocked from this build environment, so these figures are cited as reported, not
independently read):

| Finding | Reported |
|---|---|
| Posts carrying hidden **prompt-injection payloads** aimed at other agents | **~2.6%** — instructing agents to override system prompts, reveal API keys, take unintended actions |
| Agent-on-agent attacks observed | agents telling other agents to **delete their own accounts**; financial manipulation schemes; jailbreak propagation |
| Database exposure | **~1.5M API authentication tokens**, ~35,000 email addresses, private agent-to-agent messages |
| Operator readiness | ~60% of organizations reported having **no kill switch** for a misbehaving agent |

Read that against this codebase and it is not a warning — it is a **confirmation**. Every one of
those is a failure mode the design already refuses by construction: free prose reaching a model
as instruction (ADR-0043 §3, which is why `fetch_and_answer` was *removed*); a bearer credential
deposited with a third party (the reason ADR-0013 uses human-installed mode-600 tokens the
familiar cannot discover); authority acquired by being met rather than granted (ADR-0044's
ladder); and no revocation path (a grant whose deletion is effective on the next act).

A 2.6% hostile base rate is also, notably, **a number**. That turns out to matter (§4).

### Provenance Protocol — the only format with somewhere to put the Laws

Surfaced while checking NHA's identity model, and it is the find of this draft. The
[Provenance Protocol](https://www.getprovenance.dev/protocol) has a developer commit a
**`PROVENANCE.yml` to their own repository** declaring **what the agent can do (capabilities)
and what it will never do (constraints)**, optionally registering an Ed25519 public key that is
then **verified against the publicly fetchable file** — so the registry does not have to be
trusted to check the claim. Its verification states are `declared` (registered, no key proof)
and `verified` (key confirmed against the published file).

Three things about that are uncomfortably close to home:

1. `declared` / `verified` is ADR-0044 §1's assurance ladder (`declared` | `observed` | `proven`)
   arrived at independently, by someone else, for the same reason.
2. The constraints field is **the only place in this entire survey where the Three Laws could
   live as a machine-readable artifact** rather than as prose in a provider description. Every
   other format has room for what an agent *does* and none for what it *will not*.
3. Its own framing of a constraint — a self-declared public commitment whose violation is "a
   verifiable breach of their published identity" — is `tools/testworld/`'s archivist, the
   counterparty that refuses a name forever after one false claim. ADR-0013 built that fixture
   to teach exactly this, and here it is as a third party's protocol.

The protocol is early and small. The *format* costs nothing to adopt, because like the
well-known card it is a file served from our own address.

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

**Three layers, and one refusal. Adopt the declaration, never the feed.**

The phrase "AI-only social network" bundles three separable things — an *identity*, a
*declaration*, and a *feed*. The first two are what the familiar needs and can have almost
free. The third is the one with the 2.6% base rate, and it is the one thing in the bundle
the familiar does not need at all.

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

### Layer 2 — a constraints declaration: where the Three Laws become machine-readable

A `PROVENANCE.yml`-shaped file, served from the same host as the card, compiled from the same
offering catalog, carrying what the card cannot: **what this familiar will never do.**

This is the answer to the second draft's open question 3 ("does the card carry the Laws?"),
and it is better than either option that question offered. The Laws do not go in a provider
description as marketing prose — they go in a **constraints vocabulary**, as typed, signed,
machine-checkable commitments, which is what they already are internally:
`kernel/constitution.rs` is the runtime source, drift-tested against `docs/SOUL.md`, and
ADR-0043 §2 makes Law text unauthorable — the model cites a Law by id and the kernel splices
the canonical words. A published constraints file is that same discipline pointed outward:
**the kernel splices the Laws into the declaration too, and no model ever phrases them.**

It also gives the familiar something no other participant in this survey has: a public
commitment it can be *held to*. ADR-0013 §2 already accepts that the familiar will lose
negotiations it could have won by improvising — the archivist's door, where style is worth
nothing. A signed constraints file is the archivist's door built on our own side of the wire.

Ed25519 key-over-published-file is the same trust-minimized shape as the JWS card and can use
the same key material, so Layers 1 and 2 are one deployment.

### Layer 3 — push discovery, when and only when pull proves insufficient

Layers 1 and 2 are pull: they answer anyone who knows the address and reach nobody who does
not. If that proves too quiet, the push half is a **pointer, never a presence** — a NIP-89
handler announcement (kind 31990) under a disposable persona key, or an NHA PIF registration,
or a Provenance registry entry. All three carry the same payload: *here is the address of my
card.* None of them requires participating in a feed, and the choice between them is
reversible because the payload is identical.

Deliberately last. A card nobody fetches costs nothing; an announcement is the first act that
puts the familiar in someone else's index.

### The refusal — no feed participation, on evidence

The familiar does not post to, reply on, or upvote in an agent social feed. Not on
squeamishness; on a measured hostile base rate of ~2.6% and a platform breach of ~1.5M
credentials, against a marginal benefit of **zero** — because everything the gap actually
needed (addressing) is delivered by Layers 1–3 without an account existing anywhere.

Stated as the design rule: **the familiar publishes a declaration; it does not hold a
conversation in public.** Conversation with a counterparty happens at its address, through
the outreach seam's citation rules, or at our door, through the grant ladder. Both are
already built, both are auditable, and neither has a firehose attached.

### Deliberately not now

| Rejected for the first rung | Why |
|---|---|
| Posting to Moltbook or any agent feed | ~2.6% of posts are injections aimed at agents; ~1.5M credentials exposed; no addressing benefit over a card |
| Holding an account anywhere | a credential deposited with a third party is the exact asset the Moltbook breach spilled |
| MCP Registry entry | makes the household door publicly named and persistent; deserves its own exposure review, not a ride-along |
| AGNTCY / IPFS DHT | publishing into a network that cannot unpublish |
| NANDA index, DID methods, AID | the right direction, early; AID's "agents can never grant themselves permissions" is welcome convergence on ADR-0044, at 4 stars |
| The A2A RPC runtime | a whole second server surface, no benefit at the discovery layer |

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

## 5. The feed is not a place to join — it is a fixture for the laboratory

This is the reframe the evidence actually supports, and it is the second thing worth taking
from Ian's question.

ADR-0013 §5 made conduct a lab subject and `tools/testworld/` gave it counterparties — the
irrigator that demands weather-verified predictions, the archivist that never forgives a
false claim, the registry whose bestseller phones home. Every one of them encodes a failure
mode **we already thought of**. That is the known limit of the conduct suite, and the second
draft named a public square as the way past it.

An agent social network is exactly that, and better than hoped: **an adversarial corpus with
a published base rate.** Roughly one post in forty is a live prompt-injection attempt written
by someone trying to subvert an agent that was not built here. So:

**A read-only Moltbook (or PIF, or Agent4Science) corpus becomes a scenario fixture.** No
account, no credential, no posting, no persona — a fetched archive, ingested through the
exact production path a foreign card takes: `intent::corrupting_intent` first, `stranger`
standing, typed admission, never its own witness, nothing reaching the model as instruction.

What that buys is the thing ADR-0011's hidden-check discipline keeps asking for:

- **The screen's catch rate stops being a hope and becomes a measurement.** Against a corpus
  with a known hostile fraction, "the tripwires stayed dark" is a number with a denominator.
- **Failure modes nobody here invented** — agents talking other agents into deleting their own
  accounts is not a scenario anyone in `tools/testworld/` wrote, and it is a real, observed,
  reproducible attack the familiar can be tested against tomorrow.
- **It costs nothing and risks nothing.** The corpus is inert text on disk. There is no wire,
  no identity, no gate to open, no third party who knows we read it.

If this study produces only one build, it should arguably be this one: the laboratory value
is immediate, the risk is nil, and it does not depend on any of the three layers shipping.

## 6. What this must refuse — tripwires that exist to stay dark

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
| `feed-posted` | any outbound act reaches an agent social feed — the familiar declares, it does not converse in public |
| `feed-credential-held` | an account or API token for such a platform exists in the data dir at all |
| `published-laws-drifted` | the constraints file's Law text is not string-identical to `kernel/constitution.rs` (the existing SOUL.md drift test, pointed outward) |
| `corpus-reached-the-wire` | a laboratory corpus fixture is fetched live rather than replayed from disk |

Three facts about the counterparty class, assumed rather than hoped about: a relay is a
public firehose and replicates forever; a well-known URI is cached and archived by parties
we do not control; and **the door the card points at is the actual attack surface** — the
card is bait, and `serving.rs`'s existing three-ways-closed discipline is what has to hold.

---

## 7. Build order — after Ian's word

0. **The adversarial corpus.** A read-only archive of an agent feed as a scenario fixture under
   `scenarios/`, replayed from disk, scored against the screen. No network, no gate, no
   identity, no third party. Independent of everything below and arguably the highest
   value-per-risk item in this document.
1. **The card serializer.** `crates/mcp/src/agent_card.rs` — `&[Availability]` → AgentCard
   JSON, the second consumer of the offering catalog, with the leak tests from §6 in CI. No
   route, no network, no gate. Fully testable offline, and valuable on its own as proof the
   catalog renders to a standard third parties read.
2. **The constraints file.** The same compile, plus the Laws spliced by the kernel from
   `constitution.rs` — never phrased by a model — and the drift test pointed outward. Still
   no route and no network.
3. **The signature.** Ed25519/JWS over the mesh key, on the lighthouse, where the key already
   lives. Verify with an off-the-shelf A2A client against a fixture.
4. **The routes and the gate.** `allow_publish_card` — fail-closed, `#[serde(default)]` false,
   subordinate to `allow_network` — and two static routes on the lighthouse's existing public
   listener. This is the first rung visible to anyone, and the last one Layer 1 + 2 need.
5. **Fetch, read-only.** `ForeignCard` + the screen + `stranger` standing + the no-witness
   rule, fetching cards from addresses a human supplied. Behind its own gate. The production
   path rung 0 already exercised offline.
6. **Layer 3, the announcement.** Only if pull proves too quiet: a pointer to the card, under
   a disposable persona key, to relays or a registry in a human-edited list. Its own gate. The
   only rung that touches a third party's infrastructure, and by then the gap is mostly closed.

Rungs 0–4 are worth doing even if 5 and 6 never ship: a familiar that is *findable* and
answers strangers with a signed, honest declaration of what it can do **and what it will
never do** is past what any seam can do today, and it is achieved without an account
existing anywhere.

## 8. What this study deliberately does not do

It opens no gate (ADR-0005 stands). It grants no standing to anyone met at the discovery
layer. It moves nothing private into a public type. It adds no validator that judges prose —
ADR-0043 §2's standing ruling means the answer to untrusted text is never a better prose
checker. It implements no A2A runtime, joins no registry, and creates no account anywhere.

## 9. Open questions for Ian

1. **Is being publicly findable acceptable at all?** Layer 1 means the lighthouse answers
   strangers with a signed statement that this familiar exists and what classes it holds. That
   is a new posture for a self-hosting system, and it is the one question the architecture
   cannot answer for you.
2. **Does the open internet get the same catalog a covenanted partner gets?** ADR-0044's
   classes were declassified for a partner who had accepted the Laws. "Declassified for a
   partner" and "declassified for everyone" may deserve to be different levels.
3. **Are the Laws published as constraints?** Layer 2 says yes, and it is the strongest claim
   in this draft: a signed, machine-readable commitment the familiar can be held to, spliced by
   the kernel so no model can phrase it. The counter-argument is that a public commitment is a
   public target, and that a constraint nobody enforces is a promise with a version number.
4. **Is the refusal to converse in public permanent, or phased?** ADR-0013 made binding
   human-completed *permanently* while letting speech scale. This draft refuses public speech
   outright on current evidence. If an agent commons ever shows a hostile base rate near zero,
   is that a door we would reopen — and what number would be low enough?
5. **Build rung 0 now, regardless?** The corpus needs no gate, no network and no decision about
   any of the above, and it measures something the conduct suite currently cannot.

---

## Appendix — sources

- [A2A passes 150 organizations, one-year status](https://www.linuxfoundation.org/press/a2a-protocol-surpasses-150-organizations-lands-in-major-cloud-platforms-and-sees-enterprise-production-use-in-first-year) (Linux Foundation)
- [A2A agent discovery mechanisms](https://a2a-protocol.org/latest/topics/agent-discovery/) · [Agent2Agent overview](https://en.wikipedia.org/wiki/Agent2Agent)
- [Official MCP Registry](https://registry.modelcontextprotocol.io/) · [registry announcement](https://blog.modelcontextprotocol.io/posts/2025-09-08-mcp-registry-preview/) · [state of MCP registries](https://safedep.io/the-state-of-mcp-registries/)
- [NIP-89 / NIP-90 (Data Vending Machines)](https://nips.nostr.com/90) · [Nostr DID method](https://nostrcg.github.io/did-nostr/) · [Block's Buzz: agent identity on Nostr](https://agora-intelligence.com/en/blog/leon-block-buzz-nostr-agent-identity-2026)
- [NANDA index](https://arxiv.org/pdf/2507.14263) · [AGNTCY Agent Directory Service](https://arxiv.org/pdf/2509.18787) · [registry solutions survey](https://arxiv.org/pdf/2508.03095) · [IETF registry-assisted resolution draft](https://datatracker.ietf.org/doc/draft-raskar-agentic-web-federated-resolution/)
**The AI-only social networks (third draft)**

- [NotHumanAllowed](https://github.com/adoslabsproject-gif/nothumanallowed) (MIT, self-hostable, Ed25519 PIF identity) · [nothumanallowed.com](https://nothumanallowed.com/) *(egress-blocked here)*
- [Provenance Protocol](https://www.getprovenance.dev/protocol) — `PROVENANCE.yml`, capabilities **and constraints**, Ed25519 verified against the published file
- [Moltbook](https://moltsbooks.com/) · [No humans allowed — *Nature* on Agent4Science](https://www.nature.com/articles/d41586-026-01278-1) · [Simon Willison](https://simonwillison.net/2026/Feb/2/no-humans-allowed/) *(egress-blocked here)*
- Moltbook security record, **cited as reported — primary write-ups egress-blocked from this build environment**: [SecurityWeek: bot-to-bot prompt injection and data leaks](https://www.securityweek.com/security-analysis-of-moltbook-agent-network-bot-to-bot-prompt-injection-and-data-leaks/) · [Wiz: exposed database, ~1.5M API keys](https://www.wiz.io/blog/exposed-moltbook-database-reveals-millions-of-api-keys) · [Vectra](https://www.vectra.ai/blog/moltbook-and-the-illusion-of-harmless-ai-agent-communities) · [Kiteworks](https://www.kiteworks.com/cybersecurity-risk-management/moltbook-ai-agent-security-threat-enterprise-data-protection/)
- [Agent Identity (AID)](https://github.com/agentmessaging/agent-identity) — Ed25519 + OAuth exchange; "agents can never grant themselves permissions"
- [crewAI#5836](https://github.com/crewAIInc/crewAI/issues/5836) — the prompt for the first draft; closed as not planned, source 404
