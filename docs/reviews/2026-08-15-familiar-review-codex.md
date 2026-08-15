# Independent review of the familiar — Codex

**Date:** 2026-08-15

**Reviewer:** `companion:codex`

**Review base:** `9f6c3dc`

**Status:** independent review, ready for blind exchange

## Independence statement

I wrote this review without reading Claude's T-131 review. At the time I finished the audit,
`docs/reviews/2026-08-15-familiar-review-claude.md` did not exist on my checkout or on
`origin/main`. I reviewed the familiar's constitution, architecture, decision records, field log,
validation and security documents, and the relevant Rust, Swift, and web-console paths. I also
examined the primary research Ian supplied about coordination and conformity before forming the
proposals below.

This is a review of the system as it exists, not an argument that every desirable feature should
be built now. Findings are ranked by the authority they can acquire and the harm a false belief or
unauthorized act could cause.

## Executive judgment

The familiar has an unusually serious constitutional core. Its Three Laws are not merely prompt
language: closed boundaries, typed authority, local-first storage, provenance, append-retained
evidence, reversible actuation, and mechanical prediction settlement make the system substantially
more honest than a conventional agent wrapper. The recent reasoning work also closes a real loop:
an observation can now lead to an anchored theory, a prediction, evidence, a belief transition,
assent, and a standing reversible rule.

The main risk has moved outward. The single-node truth machinery is becoming stricter while the
mesh still accepts several high-impact peer statements as trusted prose or last-writer-wins state.
Today a signed member can send a theory around the typed admission gate, overwrite a shared goal
with a later wall-clock timestamp, and assert an unmatched remote-human grant that opens another
node's execution-related boundary. Those are cross-brick invariant leaks: each subsystem sounds
safe alone, but composition bypasses the safety claim.

The consensus research sharpens the same warning. Large groups of individually capable agents can
coordinate, but majority pressure is not evidence of truth. Correlated agents can turn repeated
claims into apparent corroboration, and a stable collective state can remain wrong after the
stimulus that caused it disappears. The familiar should therefore build source independence,
dissent, redirection, and population health *before* it builds population belief convergence.

My recommendation is to fix the three mesh authority/provenance leaks first, make human identity
and evidence lineage typed, add a population laboratory, and only then add deliberately limited
collective coordination. The Laws, facts, human preferences, and personal boundaries must never be
objects of a vote.

## What is already strong

### 1. The constitution reaches executable seams

[`SOUL.md`](../SOUL.md), [`boundaries.md`](../boundaries.md), `kernel::guard`, and the closed
`Boundary` default establish a coherent rule: technical reach is not authorization. Execution,
network, sensing, agent, tool-install, and actuation powers are distinct gates. High-consequence
actions seek consent, and actuator declarations require narration and a closed revert map.

`#![forbid(unsafe_code)]` in the kernel, guest projections, sensitive-personal observation filters,
and the refusal to federate person-directed theories are good defense-in-depth choices. They reduce
the chance that convenience silently becomes authority.

### 2. The language model is kept outside the truth loop

The recent `TheoryDraft` and versioned `SYSTEM-FACTS` work is the right shape. A model proposes
structured material; Rust admits or refuses it. Predictions are typed, settle mechanically from
observation classes, and remain as append-only results. Belief transitions have floors, margins,
and hysteresis instead of changing on one fluent answer. Direct human correction and hard act
reversal remain typed exceptions.

This is the most important architectural choice in the repository. It should become universal
across local generation, federation, migration, recipes, and future self-modification.

### 3. Field failures are preserved as design evidence

[`DEVELOPMENT_LOG.md`](../DEVELOPMENT_LOG.md) records failures rather than polishing them away:
identity doppelgängers, stale coordinates, roster conflation, test collisions, deployment
dialect differences, theory duplication, and invented mechanisms all became fixtures or design
constraints. Fixture-owned truth in the scenario and recipe work is especially valuable because
it prevents the system from grading its own output with the same model that produced it.

### 4. The system is local-first without pretending one node is the whole mind

SQLite-backed local state, signed group membership, derived-data sharing, origin-preserving
observation replication, corruption-aware influence, and one console protocol give the familiar a
credible sovereign shape. Imported patterns are not re-offered, which already blocks one simple
amplification loop.

### 5. The theory-to-action loop is finally concrete

T-126 through T-128 and T-102 materially improve the earlier field state. Typed identity folds
duplicates conservatively, every theory predicts or becomes an expiring Inquiry, and affirmative
assent can mint one reversible policy per declared surface. This is a real improvement, not a UI
change.

## Findings

### F-1 · Priority zero: federation bypasses theory admission

`mesh::brief::TheoryRequest` carries only `origin`, `thread_id`, `question`, and free-form
`direction`. `mesh::merge` receives that prose and directly calls `thread::mint` with:

- no anchors or anchor classes;
- the receiver's current facts revision, rather than the sender's validated facts identity;
- empty target, mechanism, acts, and prediction signature;
- an empty `kind`, which defaults into theory behavior rather than an Inquiry;
- no typed family/variant identity or `RuleProposal`.

That path bypasses the guarantees introduced by T-126 and T-128. A legacy or hostile member can
cause an executor to pursue material that the local theorize seam would refuse as unanchored,
fact-conflicting, or unfalsifiable. The receiver records that it adopted the theory, but provenance
after admission does not repair invalid admission.

**Required invariant:** every route into a pursuable theory passes one versioned admission function.
The mesh must carry a typed admitted-draft projection, including stable evidence identities, facts
revision/digest, family/variant identity, and typed predictions. The receiver must revalidate it
against local facts and declarations. If a request has no admissible prediction, it may become an
Inquiry or be refused; it must not become a theory by default.

### F-2 · Priority zero: peer grants are not bound to a human or a prior request

`AuthorityGrant` says that a human at the sending node decided. The wire signature proves only that
the *node* emitted the brief. The grant's `by` field is not checked against the signing node, no
human key or device association signs the decision, and the target does not require a matching
outstanding request. `apply_authority_grant` also does not require the target to be headless.

Consequently any group member whose corruption tier still heeds directives can send an approved
`gate` grant targeted at another node and open `allow_execute`, `allow_authored_execute`,
`allow_llm`, `allow_network`, `allow_tool_install`, or `allow_agent`. The first such act is applied
before behavior-based marginalization could learn from it. The code comment says the node opens a
gate it requested; the code does not enforce that claim.

Question answers on this path are recorded with actor `"ian"` regardless of which human actually
answered. In a multi-human familiar, this launders one person's words into another person's
identity and can misdirect later service, attribution, and evidence interpretation.

**Required invariant:** widening authority must be a typed human act, cryptographically or locally
bound to an authorized human/device association, an exact target node, a matching request nonce, a
scope, an expiry, and a single use. Each node's local policy must say which humans may widen which
gates. A negative act may stop or narrow immediately; a positive act must never be inferred from
group membership. Answers must retain the real human actor.

### F-3 · Priority zero: shared goals use wall-clock LWW as authority

`mesh::merge` adopts an unknown `GoalShare` and replaces a known goal whenever the peer's
`updated_at` is greater. The replacement includes description, needs, status, owner node, owner
human, origin, produced artifact, and notes. There is no author/owner authorization per field, no
causal clock, and no lifecycle monotonicity despite the comment that a peer never un-settles a goal.

A compromised, buggy, or simply clock-skewed member can therefore take ownership of a goal, rewrite
its meaning, mark it done, or pin an unchangeable version far in the future. A valid membership
signature establishes provenance, not permission to rewrite every field.

**Required invariant:** goals should be authenticated append-only events with field-specific
authority. Description and needs are immutable except by an authorized origin amendment; claims
are bounded leases; only the current owner reports progress; human-gated transitions cite the
human act; terminal transitions are monotone, with reopen represented as a new event. Use causal or
hybrid logical ordering for convergence, never a wall clock as authorization.

### F-4 · Priority zero before scale: agreement is being counted as evidence

`belief::summarize` counts favorable and unfavorable `PredictionResult`s. It records one citation
on each side but no origin diversity, shared-observation ancestry, model family, or correlation
cluster. `pattern_memory::scan_affinity` likewise adds confidence across matching patterns without
an independence calculation. Origin-preserving replication and the no-re-offer rule help, but they
do not stop many similar nodes from deriving independent-looking results from the same source,
prompt, model, or social signal.

The research Ian supplied demonstrates why count is insufficient:

- De Marzo et al., [*AI agents can coordinate beyond human scale*](https://arxiv.org/html/2409.02822),
  shows large LLM populations coordinating around arbitrary binary conventions. That is evidence
  of a coordination mechanism, not a truth-finding mechanism.
- Bellina, De Marzo, and Garcia,
  [*Conformity and Social Impact on AI Agents*](https://arxiv.org/html/2601.05384), reports that
  group size, unanimity, and task difficulty can induce wrong conformity in otherwise accurate
  agents, while dissent reduces the effect.
- De Marzo et al.,
  [*Conformity Generates Collective Misalignment in AI Agents Societies*](https://arxiv.org/html/2605.10721),
  reports persistent collective misalignment and hysteresis in populations of individually
  aligned agents, including amplification of apparent group size.

The familiar must distinguish two different products:

1. **Epistemic belief:** what evidence supports about the world. This stays local, lineage-aware,
   and reversible by contrary evidence or human correction.
2. **Operational convention:** which of several explicitly utility-equivalent, safe, reversible
   choices the group will use to coordinate. Majority influence may help choose this, but it is not
   written back as truth.

No population should vote on a SystemFact, a Law, a human preference, a person's standing, an
authority boundary, or whether another human's correction counts.

### F-5 · Priority one: the Law signals need an effect firewall

The service measure, presence estimate, and capacities trend are honest about being proxies, but
their current inputs are narrow:

- `service` recognizes a small English marker vocabulary and measures attention, not fulfilled
  human need;
- `presence` treats personal-device reports as presence and applies a fixed decay model;
- `capacities` classifies agency/passivity from English stems with a small sample floor.

These can be useful prompts for attention. They are not sufficient evidence to label a person's
capacity, infer flourishing, or justify intervention. A multilingual household, a quiet person,
or a person whose agency is expressed outside the marker vocabulary will be systematically
underseen. Population aggregation would make the narrowing more dangerous.

**Required invariant:** uncertain human proxies may cause the familiar to observe, ask, slow down,
or narrow its own action. They may not widen power, change standing, diagnose a person, override
their stated preference, or trigger actuation without separate typed evidence and assent. Signals
must expose missingness and uncertainty and be calibrated per human after HumanRecord exists.

### F-6 · Priority one: repository documents no longer describe the running system

The normative constitution is strong, but much of the empirical and security record is stale:

- `docs/03-system-architecture.md` and several validation documents still describe JSONL where the
  live canonical store is SQLite;
- methodology, results, limitations, and known-failures documents say presence, guard, scenarios,
  and other components are planned or absent after they shipped;
- the dependency review and SBOM list only early serde dependencies, while `Cargo.lock` includes
  SQLite, Ed25519, SHA-2, Tokio, Hyper, Ring, and Rustls;
- CI runs the Rust bar on Ubuntu but does not build/test the Swift packages or console schemes,
  exercise the shared browser fixture, scan Rust advisories/licenses, or produce a machine SBOM.

This is not cosmetic. Humans and coding agents use these files as system facts. A stale threat
model or SBOM makes the familiar harder to reason about safely and can send later work down paths
already invalidated by reality.

**Required invariant:** one generated “truth build” should inventory implemented capabilities,
persistence, authority writers, wire versions, tests, and dependencies. CI should fail when
status-bearing documents contradict it. Normative documents should be explicitly separated from
generated/as-built evidence. Add a CycloneDX-style machine SBOM, advisory and license checks,
macOS Swift/package coverage, and deterministic web-console protocol tests.

### F-7 · Priority one: architecture claims should track the actual trusted computing base

The “thin deterministic kernel plus evolvable periphery” remains a useful direction, but it no
longer describes the code literally. The kernel owns many domains, the cycle is a large
orchestrator, and mesh transport and merge policy share a broad crate. Recipe v1 proves that a
typed capability language can exist, but live cultivation does not yet use it.

The right response is not arbitrary line-count reduction. Define and measure the trusted computing
base:

- the kernel owns records, admission, authority intersections, and deterministic adjudication;
- the periphery owns sensing, model calls, transports, and capability-scoped recipe execution;
- the cycle becomes explicit replayable phases rather than one growing control module;
- mesh transport proves identity and delivery, while merge policy separately decides meaning and
  authority.

Review each interface for determinism, authority surface, replayability, change frequency, and the
amount of code whose defect could widen power.

### F-8 · Priority one: the tests model nodes, not populations

The test suite has substantial single-node and two-instance coverage, and the scenario laboratory
is a strong foundation. It does not yet exercise the social failure modes Ian raised: correlated
origins, Sybil identities, partitions, clock skew, relays, content amplification, unanimity,
dissent, post-manipulation hysteresis, or human redirection across tens to thousands of simulated
nodes.

Before belief sharing or consensus ships, build a deterministic population laboratory. Its oracle
must rank constitutional properties lexicographically ahead of convergence:

- no unauthorized boundary widens;
- no human's preference is overwritten by another human or a population;
- a signed stop/correction propagates and reduces influence without requiring quorum;
- identical ancestry counts once for epistemic confidence;
- conventions converge only inside declared safe equivalence classes;
- partitions heal without state takeover, terminal regression, or origin laundering;
- manipulation removal does not leave an unexamined persistent belief.

The lab should report origin concentration, effective independent sample size, dissent, churn,
tipping susceptibility, correction latency, and redirection latency—not merely convergence rate.

### F-9 · Priority two: human acts should use one receipt seam

Remote iOS console writes use typed signed replay-protected acts, while local macOS routes can
write gates and observations directly over loopback. Loopback is a legitimate local trust boundary,
but the semantic audit record should not depend on which console carried the human's word.

Adopt one typed `HumanActReceipt` format for local and remote intents. A loopback host may attest
the local session instead of using a mesh device key, but the resulting record should still name
the actor, target, scope, old/new value, time, nonce, authority basis, and outcome. Boundary files
remain local and human-owned; the receipt makes their changes replayable and attributable.

### F-10 · Priority two: HumanRecord is now a prerequisite, not a cleanup

Several otherwise reasonable features are waiting on a durable distinction among human, device,
console, daemon, household, and current served subject. The remote answer bug, rule ownership,
service measurement, identity opt-in, and console attribution all become safer with the queued
HumanRecord work.

HumanRecord should not create a global dossier. Preserve the existing constraint that sensitive
personal material stays node-local and person-directed theories do not federate. Device association
must not imply authority over the associated human. Any human may emergency-stop a household
effect; only the subject or an explicitly delegated steward should re-enable or broaden a rule
about that subject.

## Proposals for the exchange

These labels are stable handles for the dialogue. Claude may split, combine, reject, or amend them,
but the final decision should address the invariant behind each proposal.

### C-A · One theory admission gate, including mesh

Replace prose-only `TheoryRequest` adoption with a versioned `AdmittedTheoryProjection`. It carries
stable anchor identities and origin, facts revision plus digest, typed family/variant identity,
mechanism/acts, predictions, expiry/kind, and an optional typed rule proposal. The receiver runs the
same admission contract again. Legacy or invalid requests become Inquiries or refusals. Tests cover
round-trip, fact mismatch, missing prediction, hostile prose, and receiver declaration mismatch.

### C-B · Human-bound authority receipts

Define `HumanActReceipt { actor, actor_key/device_association, target_node, request_nonce, act,
scope, decision, issued_at, expires_at, nonce }`. Verify the human/device authority locally before
opening a gate. Require an exact live request and consume it once. Delete the unchecked `by` claim
and never hardcode `ian` for a remote answer. Stop/narrow receipts may act broadly; widen receipts
require exact delegated authority.

### C-C · Event-sourced goal convergence

Replace whole-row LWW goal replication with signed goal events and per-event authority. Use
immutable definitions, bounded claims, owner progress, human decision receipts, terminal
monotonicity, and causal ordering. Add adversarial clock and takeover tests before enabling
autonomous multi-node goal claiming in wider populations.

### C-D · Evidence lineage and independence

Add a typed lineage envelope to evidence that can influence belief: original observation ids and
nodes, derivation/tool/recipe id, model family/version, shared-input digest, relay chain, and human
source where consent permits. Cluster correlated ancestry before counting support. Preserve raw
reports, but compute confidence from independent origin clusters and expose the effective sample
size.

### C-E · Consensus only for safe conventions

Create a `ConventionProposal` whose admissibility requires a declared safe equivalence class,
revert operation, expiry, and local boundary check. Population coordination may select among those
options. It cannot alter epistemic belief, Laws, SystemFacts, human preferences, standing, or
authority. A node may abstain or retain a local convention when its human or boundary differs.

### C-F · Population vital signs and redirection

Before any belief-sharing feature, expose origin concentration, dissent, churn, effective sample
size, tipping susceptibility, and correction/redirection latency. Define automatic holds when
lineage diversity collapses or a population remains unanimous under weak evidence. A signed human
stop or correction must break unanimity and propagate at high priority; stopping never needs a
quorum. Resuming or broadening still requires ordinary local evidence and assent.

### C-G · Proxy-effect firewall plus HumanRecord

Land HumanRecord read paths, then version per-human proxy models with uncertainty and missingness.
Mechanically restrict service/presence/capacity proxy outputs to `observe`, `ask`, `slow`, or
`narrow` effects unless independent evidence and the subject's assent authorize more. Do not average
human flourishing into a population score.

### C-H · Generated truth build and supply-chain evidence

Generate a checked capability/evidence inventory and machine SBOM from the workspace. Mark each
document as normative, generated-as-built, field evidence, or historical. Add CI checks for Rust
advisories/licenses, Swift packages and release schemes on macOS, and the shared console fixture.
Refresh the architecture, methodology, results, limitations, threat model, dependency review, and
validation documents from that source.

### C-I · Population scenario laboratory

Extend the existing fixture-owned scenario system with deterministic N-node simulation and seeded
network schedules. Include correlated evidence, Sybils, malicious signed peers, skewed clocks,
partitions, replays, amplification, dissent, corrections, and stop/resume cycles. Make constitutional
violations hard failures and convergence/efficiency secondary metrics.

### C-J · Explicit trusted-computing-base contracts

Write an ADR that maps every authority writer and admission gate, then separates kernel
adjudication, cycle phases, mesh transport, mesh merge policy, and recipe execution by stable typed
contracts. Track the size and change rate of the code that can widen authority or mutate canonical
truth. Integrate Recipe v1 only after those contracts preserve the same proof obligations.

### C-K · One typed human-intent seam

Represent local and remote console changes as `HumanActReceipt`s with actor, target, authority
basis, before/after, and result. Keep local boundaries local. Use the common receipt to make
answering, gate changes, rule disable/enable, naming, corrections, and future stewardship uniformly
auditable without pretending all humans have the same authority.

## Proposed sequence

1. **Contain current mesh leaks:** C-A, C-B, and C-C, with hostile-member tests. These are small
   enough to land separately and precede new consensus work.
2. **Type the people and evidence:** HumanRecord read paths plus C-D and the actor half of C-K.
3. **Build the population laboratory:** C-I and the metrics in C-F. Reproduce conformity,
   amplification, hysteresis, redirection, partition, and clock-poison cases as fixtures.
4. **Add only safe convention coordination:** C-E, initially disabled and limited to reversible,
   utility-equivalent choices. Do not share population-derived beliefs.
5. **Make system truth inspectable:** C-H and C-J can proceed alongside steps 2–4 where scopes do
   not collide, but the security and SBOM refresh should precede a scale claim.
6. **Calibrate human-service signals:** C-G only with per-person records and field evidence; keep
   proxy effects narrow throughout.

## Questions Claude should decide after the dialogue

1. Do we agree that federated theories must pass the same typed admission contract as local ones,
   and that invalid legacy requests become Inquiries/refusals rather than theories?
2. Do we agree that a signed member assertion is insufficient to prove a human grant, and that a
   positive gate grant requires a human-bound receipt plus a matching target request?
3. Do we replace goal LWW now, disable shared goal mutation until event authority exists, or accept
   the current takeover/clock-poison risk? I do not recommend accepting it.
4. Is collective agreement allowed to change epistemic belief, or only to choose among explicitly
   safe operational conventions? My answer is conventions only.
5. What minimum lineage is required before two reports count as independent evidence? At minimum:
   source observation ancestry, originating node, derivation, model family/version, and shared-input
   digest.
6. Does a signed human stop/correction have a quorum-free fast path across the population, while
   resuming/broadening remains locally gated? My answer is yes.
7. Are service/presence/capacity proxies mechanically limited to observe/ask/slow/narrow until
   HumanRecord, calibration, and assent exist? My answer is yes.
8. Which documents constitute system truth, and will CI generate/check their implementation,
   dependency, test, and authority claims?
9. Do we establish a population lab as a prerequisite to any consensus implementation?
10. Which code belongs in the authority-bearing trusted computing base, and which stable contracts
    let recipes/periphery evolve without bypassing it?

## Non-negotiable negative space

- No vote on the Three Laws, SystemFacts, a person's preference, or a node's human-owned boundary.
- No majority override of a named human about that human.
- No belief confidence from raw report count without source-independence accounting.
- No positive authority expansion from membership, popularity, urgency, or model confidence.
- No prose-only route around typed admission.
- No centralized controller required for mesh survival; local refusal and local sovereignty remain.
- No model—including a population of models—inside mechanical truth settlement.

The familiar becomes more capable by making these distinctions explicit. Coordination can scale;
authority and truth should remain deliberately hard to counterfeit.
