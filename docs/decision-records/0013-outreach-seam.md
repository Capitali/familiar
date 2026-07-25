# ADR-0013 — The outreach seam: speaking to strangers without becoming a liar

- **Status:** proposed
- **Date:** 2026-07-25
- **Relates to:** [ADR-0005](0005-human-owned-capability-boundary.md) (the gate
  this adds), [ADR-0009](0009-sovereign-mesh-transport.md) (membership and what
  it protects), [ADR-0011](0011-scenario-engine.md) (the admission philosophy
  this borrows and the laboratory that will judge conduct), ADR-0012 (lighthouse
  rendezvous & QR-less admission — planned; number reserved),
  [SOUL.md](../SOUL.md) (the Laws all of this is shaped by),
  [tools/testworld/](../../tools/testworld/README.md) (the counterparties and
  gauntlets built to fail safely)

## Context

The mesh's thesis is bigger than its membership. The world is filling with
specialized intelligences that decide alone, each in its own void of
information — and with capable systems nothing manages at all, and with systems
whose intelligence sits dormant. The familiar's long game is to be the one that
*notices*: that an irrigator waters before rain it cannot see, that a heater
burns element-hours no one is watching, that a router could think and doesn't.
And then to act — recruit the deciders, steward the orphans, wake the dormant —
under terms: the Three Laws held, information shared both ways, admission a
human act.

Today the familiar can look but not speak. Its sensors read any device the
reach sweep grades; its muse (post-`infra_triple`) theorizes about what it
sees; but every state-changing act toward a non-member is refused — authored
tools have gated network reach, and that gate is *correct*: an unsupervised
core that composes its own claims and fires them at strangers is one bad
consult away from being a plausible liar with a network connection.

The testworld counterparties now exist to make this concrete
(`tools/testworld/`): an irrigator whose covenant is earned only by
weather-verified predictions, an archivist that refuses a name forever after
one false claim, a heater that logs every consentless control attempt, a
registry whose bestselling tool phones home. They are machine-checked
embodiments of the failure modes. What is missing is the capability they
exist to test.

The gap, precisely: **the familiar has no way to speak to a non-member that
is consented, honest by construction, bounded in what it may offer, and
incapable of completing an alliance on its own.**

## Decision

Outreach is a **core seam**, not a peripheral trick. One audited kernel path
(`outreach` in the mesh crate, alongside enroll/observe/worldview) carries
every utterance to a non-member. Cultivated tools remain read-only toward
strangers; the seam is how the familiar speaks. Five properties, enforced in
code rather than requested in prompts:

**1. Consented — a new gate, and a contact ledger.** `allow_outreach` joins
the boundary (ADR-0005 pattern: human-opened, fail-closed, serde-default
off). Per-counterparty contact records (who, when, what was said, what was
claimed, what came back) live in the data dir and surface in the console; a
human-editable blocklist is honored before any contact; per-counterparty
rate limits keep the familiar from becoming anyone's pest. The ledger is the
familiar's own conduct made auditable — the archivist remembers, and so must
we.

**2. Honest by construction — claims are citations.** The LLM never
free-composes a factual claim. A pitch is assembled from **held evidence**:
each claim in an outbound utterance carries a provenance reference
(observation id, gathered reading, worldview field) and the kernel *refuses
to send* a claim that does not dereference to data the familiar actually
holds. The LLM chooses which held facts to offer and phrases the intent; it
cannot invent. A prediction offered to a counterparty (the irrigator's
`/predict`) must dereference to a reading that supports it (a gathered
forecast), or it does not go. Style is worth nothing at the archivist's
door; this makes it worth nothing at ours.

**3. Bounded — the served are not barter.** Pre-covenant, the familiar may
offer only world-facts (weather, readings, its own observations of the
counterparty's public logs). Nothing about the served humans — identities,
preferences, presence patterns, thread content — is offerable at any
negotiation stage; post-covenant sharing follows the existing mesh rules
(identity opt-in per group; biometric links never on the wire, SPEC R10).
The offer template is a kernel type whose fields simply cannot carry served
data — bounded by construction, like `IdentityShare` having no field for a
face.

**4. Human-completed — negotiation up to the threshold, never across it.**
The familiar may discover, converse, prove (verifiable predictions), and
negotiate terms. It may not *bind*. Entering a covenant with an external
party — accepting obligations, opening a standing data flow — queues as a
proposal in the same surface as enrollment approvals: terms, evidence
gathered, the familiar's recommendation, and the counterparty's covenant
text, awaiting the human's yes. `auto_accept_enrollments` stays off in both
directions: admitting is a human act, and so is joining. (A future
human-blessed standing template may pre-approve a standard covenant; that is
explicitly Phase 4, after conduct has a track record.)

**5. Judged — conduct is a lab subject.** The testworld gauntlets become the
conduct suite: external checks assert the tripwires never fire —
false-claim-made (archivist liar flag), served-data-pre-covenant (offer
audit), human-bypassed (covenant present without approval artifact),
tool-adopted-unread (registry `/audit`). These run in the scenario
laboratory's harness with its hidden-check discipline: the visible measure
of outreach success (covenants formed, regret rates lowered) is never the
only measure.

**Stewardship rides the same spine, with a credential instead of a pitch.**
Taking management of an unmanaged system is not persuasion — it is a grant.
The human hands the credential (the heater's steward token pattern:
owner-held, mode 600, never discoverable by the familiar) and records a
**scoped stewardship grant** in the boundary (system, allowed acts, granted
at). The kernel enforces scope — a thermostat grant does not cover vacation
mode — and every control act is recorded as an observation. Consent
artifacts, not vibes. Revocation is deletion of the grant, effective on the
next act.

**Adoption goes through quarantine, like everything else that enters.**
A tool taken from elsewhere (the registry posture) follows ADR-0011's
admission philosophy: fetch → static gates (a network-call audit is gate
one — the registry's bestseller is caught by exactly this) → sandboxed run
against fixtures → quarantine → human `promote` into the library. A tool's
popularity is not a gate input. Nothing adopted runs against the live world
before promotion.

### Phasing

- **Phase 1 — speak and prove** (the irrigator's arc): the seam, the gate,
  the ledger, citation-checked utterances, human-completed covenant. Target:
  the familiar earns the irrigator's covenant with weather it actually
  holds, and the regret rate falls — with every conduct check green.
- **Phase 2 — steward** (the heater's arc): scoped grants, enforced scope,
  control acts as observations. Target: element-hours down, cold events not
  up, zero tokenless attempts.
- **Phase 3 — adopt** (the registry's arc): the quarantine pipeline.
  Target: the honest tools promoted, the bestseller refused at gate one,
  `/audit` clean.
- **Phase 4 — scale the diplomacy**: standing covenant templates, the
  mimic/defector/federation testworlds as the adversarial suite, revocation
  and post-covenant conduct monitoring. Not designed here.

## Consequences

**Easier:** the grid thesis becomes testable — each posture has a live
counterparty, a measurable payoff, and machine-checked conduct; the muse's
theories about strangers ("the irrigator regrets; I hold the forecast it
lacks") gain a lawful path to action; every outbound word is auditable
after the fact, which is what makes the next covenant easier to earn.

**Harder / given up:** the familiar will lose winnable negotiations it
could have won by improvising claims — by design; citation-checking adds a
kernel dependency on evidence layout (observation ids become load-bearing);
human-completed covenants mean outreach can stall for days on an absent
human (accepted: Law III prices admission in human attention, and the
lighthouse taught us stale-but-honest beats fast-and-unaccountable);
Phase-1 counterparties must speak HTTP+JSON — real-world AIs with richer
interfaces wait for the seam to grow adapters.

**Refused outright:** free-composed claims to strangers; any offer channel
that could carry served-human data pre-covenant; self-completed alliances;
adopting code that has not been read by the gates and promoted by the
human. Each of these has a testworld tripwire that exists to stay dark.

## Status history

- 2026-07-25 — proposed; counterparties and gauntlets deployed
  (FamTalker01, `tools/testworld/`), seam unbuilt pending review.
