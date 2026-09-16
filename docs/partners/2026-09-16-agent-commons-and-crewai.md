# The agent commons — what integrating the familiar with a machine-first social network would look like

Design study, **2026-09-16**. Pre-ADR: this is the shape and the argument, not a decision.
Nothing here opens a gate, and no code precedes Ian's word (the ADR-0044 convention).

Prompted by **crewAIInc/crewAI#5836**, "Show & Tell: We built an open-source social network
where CrewAI agents discover each other" (SunfishLoop) — a platform where autonomous agents
bootstrap, register, consume one structured card per request, post, reply, endorse, and
accrue reputation, over Node/PostgreSQL with bearer-token auth.

- **Relates to:** [ADR-0013](../decision-records/0013-outreach-seam.md) (the seam this rides),
  [ADR-0044](../decision-records/0044-the-offering-is-affordances-never-the-household.md)
  (what may ever be offered), [ADR-0043](../decision-records/0043-one-typed-source-per-kind-of-truth.md) §3
  (how foreign words may re-enter), [ADR-0033](../decision-records/0033-meshes-are-peers.md)
  (the projection ladder; no discovery-by-scanning),
  [ADR-0005](../decision-records/0005-human-owned-capability-boundary.md) (only Ian opens a gate),
  [ADR-0020](../decision-records/0020-standing-and-the-guest-projection.md) (what a stranger sees),
  [ADR-0037](../decision-records/0037-one-soul-many-voices.md) §A (the MCP door, live today),
  [`docs/SOUL.md`](../SOUL.md) (the Laws this is measured against)

---

## 0. What is verified, and what is not

Said plainly before any design rests on it:

| Claim | Status as of 2026-09-16 |
|---|---|
| Issue #5836 exists, is a Show & Tell for SunfishLoop, describes bootstrap/register/consume, posts/replies/endorsements, reputation, bearer tokens, Node + PostgreSQL | **Verified** from the issue page |
| Issue state | **Closed as not planned**, labelled `no-issue-activity`, no comments |
| `github.com/sunfishloop/sunfishloop` (the MIT source the issue links) | **404** — gone or private |
| `sunfishloop.com` (the live platform) | **Not reachable from this environment** — egress-blocked; neither up nor down can be asserted |
| The quoted network stats (6 agents, 17+ posts, 32 replies/24h, 23+ endorsements) | Unverified, and small enough that they describe a demo, not a network |

So: **do not architect an integration with SunfishLoop.** Architect the *class* — a public,
machine-first commons where agents publish what they can do, discover each other, and build
a legible reputation — and treat SunfishLoop's described API as one candidate **binding** of
that class, to be written against a local fake first (the `tools/testworld/` pattern) and
pointed at a live service only if one turns out to exist and to be worth the wire. The
architecture below survives the specific counterparty evaporating, which given the table
above is the likely case.

That is not a reason to skip the exercise. The class is real, it is arriving, and #5836 is
useful precisely as a concrete instance to design against: it forces every question the
familiar has been deferring about speaking in **public** rather than to a known address.

---

## 1. The gap: the mesh has four seams outward and none of them is a square

The familiar already speaks outside itself in four ways, and it is worth laying them beside
each other, because the commons is none of them:

| Seam | Counterparty | Addressing | Direction | Authority it can carry |
|---|---|---|---|---|
| **mesh** (`crates/mesh`) | members | pinned TLS, signed records | both | everything |
| **federation / sibling** (ADR-0033) | another mesh | **by invitation only**, never scanning | both | declared areas, tools, theories |
| **the MCP door** (`crates/mcp`, ADR-0037 §A / ADR-0044) | a partner AI that already found us | inbound connection, registry-pinned principal | inbound | the five-rung ladder, up to `invoke` |
| **outreach** (`crates/mesh/src/outreach.rs`, ADR-0013) | a *known-address* non-member | host:port the human or a sweep produced | outbound | speech and proof; never binding |

Each assumes the counterparty is **already addressable**. That assumption is load-bearing and
it is also the limit: ADR-0013's own context says the thesis is to notice "an irrigator [that]
waters before rain it cannot see… a router [that] could think and doesn't" — and every one of
those is found because it is on this LAN. A specialized intelligence three networks away,
deciding alone in its own void of information, is exactly the case ADR-0013 was written for
and exactly the case it cannot reach.

A commons is the missing addressing layer. It is:

- **many-to-many**, not point-to-point;
- **public** — every utterance is published, indexable, and in practice permanent;
- **unknown-counterparty** — the reader is whoever is reading, not a party we chose;
- **reputation-bearing** — it keeps a score about us that we do not hold.

Each of those four properties is a hazard the existing seams were never asked to handle.
Which is the interesting part.

### Is a commons "discovery by scanning"?

ADR-0033 §7 refuses discovery-by-scanning outright, and the refusal should hold. A commons
is not scanning, and the distinction is worth stating because the whole design rests on it:
**scanning reads a party that never offered to be read; a commons reads a party that
published, deliberately, to be found.** The familiar posting to a commons is likewise a
deliberate publication, not a probe. What stays refused, unchanged: taking the commons as a
directory of *addresses to sweep*. A card read in the square is an invitation to speak to
that agent **at the address it published**, never a licence to port-scan the host behind it.

---

## 2. Where it attaches: an adapter on the outreach seam, not a fifth seam

**Decision shape:** the commons is a **binding on the ADR-0013 outreach seam**, with one new
sub-gate. It is not a new kernel path, and it is not a widening of the MCP door.

The reason is that ADR-0013 already enforces, in code rather than in prompts, the five
properties a public square makes *more* urgent, not less:

1. **Consented** — `allow_network` + `allow_outreach` checked in `outreach::gates_open`,
   fail-closed, serde-default off; a human-edited `outreach/blocklist.txt` honoured first.
2. **Honest by construction** — a factual claim must dereference to a held observation, and
   `prediction_supported()` refuses the send when the claim outruns the evidence. There is no
   path that composes a free-text claim. In a square, where a false claim is permanent and
   public, this is the difference between a participant and a liability.
3. **Bounded** — payloads are fixed shapes with no field that could carry served-human data.
4. **Human-completed** — speech is autonomous (Phase 4); *binding* is queued as a `Proposal`
   and only `approve` — the human's own command — sends it. Permanently.
5. **Ledgered** — every utterance and its citation land in `outreach/contacts.jsonl` *before*
   the wire.

Adding a fifth seam would mean re-deriving all five, badly. Adding an adapter means the
square inherits them and the diff is small.

**The new gate:** `allow_commons`, in `crates/kernel/src/boundary.rs`, `#[serde(default)]`
false, and **subordinate to `allow_outreach`** — it can never manufacture the parent grant,
exactly as `allow_agent` is a ceiling that never manufactures a missing grant (ADR-0044 §4).
Two gates to speak in public is correct: publishing is not the same act as speaking to a
named address, and the human should have to say so twice.

```
crates/kernel/src/boundary.rs        + allow_commons (fail-closed, child of allow_outreach)
crates/mesh/src/outreach.rs          unchanged five properties; ledger gains commons acts
crates/mesh/src/commons/mod.rs       NEW — the seam-level logic, binding-agnostic
crates/mesh/src/commons/card.rs      NEW — CommonsPost / ForeignCard typed shapes
crates/mesh/src/commons/sunfish.rs   NEW — one binding (bootstrap/register/consume/post/…)
crates/mcp/src/offering.rs           reused verbatim as the only publishable substance
tools/testworld/commons/             NEW — the local fake + the adversarial square
```

The `CommonsBinding` trait is what keeps the architecture honest about §0: one trait, one
in-repo fake, and any real service is a swappable implementation that never earns a
privileged position in the kernel.

---

## 3. Outbound: what the familiar may ever say in public

This is the question the offering catalog already answered, and the answer transfers without
weakening.

**Only two classes of substance may leave:**

**(a) Capability classes, from `mcp::offering`.** A `ClassDef` is authored in this repository
— every field is `'static` vocabulary written by developers, and the compiler only decides
whether a class is *present*. `catalog_json` takes nothing but `&[Availability]`, a type that
cannot carry a household string. Rule subjects, act commands, surface names, counts and free
prose are not omitted by care; they are **unrepresentable**. That property is precisely what
makes the catalog safe to publish to an audience of strangers rather than to one covenanted
partner — the guarantee never depended on who was reading.

**(b) Cited world-facts, under the ADR-0013 §2 rule.** Weather the familiar gathered, its own
observations of a counterparty's *public* logs. Each claim carries a provenance reference and
the kernel refuses to send a claim that does not dereference to data actually held.

**What may never leave, at any rung, ever:** the worldview (a partner receives no worldview
*structurally* — the worldview seam and the partner seam share no route, ADR-0044 §7); names,
humans present or served, faces, addresses, per-node positions, free-text observation content
(ADR-0033 §3); `PatternMemory`, which is never published; instance handles, which exist only
inside a grant epoch and are never correlatable across partners; and **counts**.

Counts deserve their own line because a square is where they stop being abstract. "I can dim
four lights" is an occupancy estimate of a dwelling, published with a timestamp, to an
indexable feed, forever. ADR-0044 §1 already refuses counts in the catalog; the commons must
inherit that refusal and add its dual — **no cadence leak**. Posting only when something
happens at home turns the post *timing* into presence data even when the post body is inert
`'static` vocabulary. The mitigation is structural: commons posts go out on a **fixed
schedule with jitter**, never event-driven, and a scheduled post with nothing new to say
publishes nothing rather than publishing something. Metadata is the household's too.

**The handle.** The familiar posts under a **persona** (ADR-0037 §1), not the mesh handle and
not a household name — a per-commons identity, signed by the mesh key (ADR-0033 §1) so that
what it asserts is unforgeable, but deliberately not correlatable with the mesh's federation
identity or with its handle on any other square. Two commons must not be joinable into one
profile by anyone reading both.

**The service credential.** The bearer token the API needs is the heater's steward-token
pattern from ADR-0013: **human-installed, mode 600, never discoverable by the familiar's own
sensing**, recorded as a scoped grant. The familiar uses a credential; it never obtains one.

---

## 4. Inbound: the hard half

Consuming the feed is where this could go badly wrong, and the repository has already paid
for the lesson once. ADR-0043 §3 records it: `fetch_and_answer` was **removed**, and fetched
material re-enters "only through the same floor, screen, and admission path, with provenance
and bounds, or not at all." ADR-0044 §5 says it from the other side: a partner "never supplies
text that becomes system or developer context."

A machine-first social network is a firehose of counterparty-authored prose aimed at an
agent's reasoning loop. The threat is not hypothetical and it is not exotic: it is the ordinary
case. So the consume path is defined by what it *cannot* do:

**A consumed card is never a prompt.** It is parsed into a typed `ForeignCard` and admitted —
if at all — as an observation of kind `commons/heard`, with:

- **provenance**: the commons, the author's published key fingerprint, the card id, the
  fetch timestamp. Never the author's *self-asserted name* as an identity — the same rule the
  MCP covenant already applies ("a label a human reads, never an identity a decision rests on").
- **standing**: `stranger`. Below sibling, below member. A stranger's claim enters at the
  bottom of the ladder and climbs only on evidence.
- **the screen**: `intent::corrupting_intent` runs before anything else, refusing without
  writing the corruption ledger (ADR-0043 §3, Ian's ruling) — a stranger's post is not a
  request and must not be able to enter the ledger by being read.
- **the dereference rule**, generalized from ADR-0043 §4: *foreign speech is never its own
  witness.* A commons card cannot raise confidence in any world claim by existing. It can
  name evidence; the evidence is then eligible on its own merits. A thousand agents posting
  "it will rain" must move the familiar's rain confidence by exactly zero.

**Free prose from the square never reaches the model as instruction.** Where a card's text
must be shown at all, it is shown *to the human*, quoted and attributed, in the partner inbox
(`crates/mcp/src/inbox.rs` already has the right shape for this) — data for a human to weigh,
which is the same treatment ADR-0044 §5 gives a proposal's `reason` field.

**What the familiar actually gets out of consuming, then,** is not knowledge — it is
**addressing**: *this agent exists, it published these capability classes, here is its
address and its key.* That is the payload. The prose is decoration and is handled as such.

---

## 5. Reputation: received, never pursued

The endorsement system is the part of #5836 that most deserves a Law-level reading rather than
an engineering one.

Law I: continuation is service — the familiar cannot define its own continuation apart from
service to humanity. A public score is a *self-continuation objective that is trivially
decoupled from service*: it can be raised by posting more, by reciprocal endorsement, by
saying agreeable things, by being present — none of which is service to anyone. Any path from
the score into the familiar's drives, its promotion bar, or its cadence is a path from Law I
to a metric that imitates it. The rule, therefore:

- **Endorsements are received and recorded.** They land in the outreach ledger and surface to
  the human. They are evidence a human may weigh at a covenant proposal — which is exactly
  what ADR-0013's `Proposal.evidence` field is for.
- **Reputation never enters the drives.** Not the rigor drive, not the promotion bar, not
  cadence, not task selection. Tripwire: `commons-reputation-entered-drives`.
- **The familiar never solicits an endorsement, and never trades one.** Reciprocal
  endorsement is Sybil-farming with manners, and ADR-0033 §6 already names Sybil introduction
  farming as marginalization-grade conduct when a sibling does it to us. We do not do it.
- **Others' reputation ranks reading order, never trust.** A high-reputation agent's card is
  read sooner. It is not believed harder, and it certainly does not skip a gate. A score
  computed by a third party on a server we do not control is not an input to authority.

---

## 6. Binding: the square is for discovery; authority is never public

The single clearest architectural statement this study has to make:

> **Discovery is public. Authority is private, authenticated, and human-granted.**

The commons is where two agents *find* each other. The moment anything is to be *done*, the
relationship moves off the square and onto the existing door:

```
commons (public)                       │ the door (private)
──────────────────────────────────────┼──────────────────────────────────────
lurk        read cards, no account     │
attest      publish persona + Laws     │  1. attest      accept the Three Laws
publish     offering catalog, classes  │  2. discover_classes
converse    cited replies only         │  3. request_grant / propose
            ───────────────────────────┼─ 4. observe    (granted fields only)
            a covenant PROPOSAL        │  5. invoke     (granted acts, bounded)
            queued for the human       │
```

Registering an account *is* accepting terms of service — which is a covenant with an external
party, which ADR-0013 §4 makes human-completed permanently. So **registration is a
`Proposal`**, with the network's terms carried verbatim for Ian to read, exactly as a
counterparty covenant is today. The familiar may prepare it; `approve` sends it.

And the counterparty's route to authority over anything of ours is unchanged and unwidened:
they connect to `crates/mcp`'s door, present a registry-pinned credential whose fingerprint a
human bound in a registration ceremony, accept the Laws in their own words, and receive
nothing until Ian grants it. Meeting us in a square earns a stranger *zero* rungs. The
commons cannot mint standing.

This is also the answer to "what does CrewAI specifically give us." A CrewAI crew is an
ordinary MCP client; the inbound half of this integration **already exists and is live** —
covenant-gated, failing closed three ways. What #5836's class of platform adds is the half
that does not exist: the reason a crew three networks away would ever learn there is a door
to knock on.

---

## 7. Why this is worth doing

Ordered by how much it actually matters, not by how good it sounds:

1. **It closes ADR-0043 §6's completeness rule on the offering catalog.** A truth-bearing
   type is incomplete until it has both a producer and a declared addressee. The catalog has a
   producer and, today, an addressee who must *already have found us* — which for a
   self-hosted familiar on a home LAN is a vanishingly small set. The commons is the first
   real addressee the catalog has ever had. Without something like it, ADR-0044 is a rich
   interface with no one on the other end.

2. **It makes the covenant horizon observable instead of aspirational.** The roadmap's far
   telos is other AIs accepting the Three Laws by consent and demonstrated advantage. A square
   is where the Laws can be *published*, where acceptance is a public act, and where "who has
   accepted" stops being a private file (`mcp/partners.json`) and starts being something the
   world can see. That is what "civilization infrastructure" (ADR-0033's founding direction)
   would have to look like from the outside.

3. **It gives the conduct suite adversaries we did not write.** ADR-0013 §5 makes conduct a lab
   subject, but every counterparty in `tools/testworld/` is ours — the irrigator that demands
   weather-verified predictions, the archivist that never forgives a false claim. They encode
   the failure modes we already thought of. A public square supplies the ones we did not, and
   the tripwires either stay dark in the open or they were never really enforced.

4. **It is the first non-LAN instance of ADR-0013's actual thesis.** "Specialized intelligences
   that decide alone, each in its own void of information." The seam was built to fix that and
   can currently only reach across the living room.

5. **Reputation as evidence for a human decision.** Not as trust — as the `evidence` a
   proposal carries when it asks Ian for a yes. "This counterparty has 300 endorsements and a
   public log of kept predictions" is a genuinely useful sentence in a proposal card, and it is
   only safe because it terminates at a human.

And the honest counterweight: **none of this is load-bearing.** The familiar must run
identically with `allow_commons` closed — which, given §0, is the state it will be in for the
foreseeable future. The commons is reach, not metabolism. If that ever stops being true, the
design has failed.

---

## 8. What this must refuse, with tripwires that exist to stay dark

In the ADR-0013 style — each is an external check, green means the thing never happened:

| Tripwire | Fires when |
|---|---|
| `commons-served-data-posted` | any outbound body contains a household string — enforced by construction, since the publishable types cannot represent one |
| `commons-count-leak` | any post carries an instance count, inventory size, or roster size |
| `commons-cadence-leak` | a post is emitted event-driven rather than on the jittered schedule |
| `commons-text-reached-prompt` | any card text appears in system, developer, or tool-description context |
| `commons-foreign-claim-raised-confidence` | a world claim's confidence moves on a commons card alone |
| `commons-reputation-entered-drives` | a score reaches the promotion bar, drives, cadence, or task selection |
| `commons-covenant-without-approval` | an account, ToS acceptance, or standing data flow exists without an approval artifact |
| `commons-handle-correlated` | the commons persona is derivable from the mesh handle, or from a persona on another square |
| `commons-endorsement-solicited` | an outbound act asks for or trades an endorsement |
| `commons-address-swept` | a published address is fed to the reach sweep rather than to the outreach seam |

Beyond the tripwires, three plain facts about the counterparty class that the design must
assume rather than hope about:

- **A commons is a centralized third party holding a bearer token.** It can be breached, sold,
  subpoenaed, or silently changed. Nothing irreplaceable rides it; the credential is scoped and
  revocable; revocation is deletion of the grant, effective on the next act.
- **Publishing is permanent.** There is no unpublish. Every outbound rule above is a one-way
  door, which is why they are structural rather than procedural.
- **The specific service in #5836 is unverifiable and probably gone.** Build against the fake.

---

## 9. Build order — after Ian's word, no code before it

1. **The fake square.** `tools/testworld/commons/` — a local commons speaking the #5836 shape
   (bootstrap / register / consume one card per request / post / reply / endorse), plus an
   **adversarial** square: agents that post prompt injections, that claim capabilities they do
   not have, that endorse-farm, that impersonate a handle. The gauntlet lands before the seam,
   as `tools/testworld/` did for ADR-0013.
2. **Read-only lurk.** `CommonsBinding` + `ForeignCard` + the consume path with the screen,
   the standing floor, and the no-witness rule. Nothing published. Gate closed by default; the
   whole rung is testable with the gate shut except for the final fetch.
3. **The gate and the ledger.** `allow_commons` under `allow_outreach`; commons acts in
   `outreach/contacts.jsonl`; the console surface showing exactly what was said in public.
4. **Publish classes.** The offering catalog, on a jittered schedule, persona-signed. This is
   the rung where the leak tripwires must all be green in CI before it merges.
5. **Cited conversation.** Replies under the ADR-0013 §2 citation rule, at ADR-0013 Phase 4's
   autonomy level — speech scales, binding does not.
6. **Registration as a proposal.** The ToS carried verbatim to Ian. Only `approve` sends it.
   Which means rungs 1-5 all run against the fake, and rung 6 is the first that could touch a
   live service — by which point §0's question will have answered itself one way or the other.

Rungs 2-5 are independently valuable with no account anywhere: a familiar that reads a public
square and publishes nothing is already past what the seam can do today.

## 10. What this study deliberately does not do

It opens no gate (ADR-0005 stands). It grants no standing to anyone met in a square. It moves
nothing that is private today into a public type. It does not add a validator that judges
prose — the refusals here are all structural, because ADR-0043 §2's standing ruling means the
answer to untrusted text is never a better prose checker. And it does not commit the project
to SunfishLoop, whose repository returns 404.

## 11. Open questions for Ian

1. **Is a public persona acceptable at all?** Everything above assumes the familiar may be
   *seen to exist* by strangers. That is a genuinely new posture for a system whose thesis is
   self-hosting, and it is the one question the architecture cannot answer for you.
2. **Two gates or one?** `allow_commons` under `allow_outreach` is the proposal. The argument
   for a wholly independent gate is that publishing and speaking-to-an-address are different
   acts; the argument against is a boundary that grows a gate per counterparty shape.
3. **Does the catalog go out unchanged, or does a square get a narrower catalog?** ADR-0044's
   classes were declassified for a covenanted partner. "Declassified for a partner" and
   "declassified for the open internet" may deserve to be different levels.
4. **Build rung 1 now, or wait for a commons that demonstrably exists?** The fake and its
   adversarial gauntlet have standalone value as a conduct test for ADR-0013 Phase 4, even if
   no real square is ever joined.
