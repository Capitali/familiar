# ADR-0022 — The dossier: remembering a human well enough to serve them

- **Status:** accepted — **implemented (slice 1, 2026-08-08)**: contribution-scored
  presence + standing patterns (`crates/kernel/src/dossier.rs`), the composed needs view,
  subject-facing read + withdrawal (`familiar dossier <handle>`), and the federation
  fence. Location patterns and habit patterns are follow-on kinds. Acting on theorized
  needs is governed by [ADR-0031](0031-consent-by-observation.md).
- **Date:** 2026-07-30
- **Relates to:** [ADR-0019](0019-friendly-identification.md) (identification — this is what
  identification is *for*), [ADR-0016](0016-multi-human-served-identity.md) (per-human attribution
  and sensitive-personal scoping — the rules this must inherit),
  [ADR-0020](0020-standing-and-the-guest-projection.md) (a guest must never see any of this),
  [ADR-0005](0005-human-owned-capability-boundary.md) (the human owns the boundary — and now owns
  their own record), [ADR-0018](0018-lighthouse-single-fixture.md) (no human is permanent),
  `docs/HUMANITY.md`, `crates/kernel/src/identity.rs`

## Context

ADR-0019 answers *"who is at this device, right now"*. That is enough to address a question and not
much else. Service needs more than a snapshot: to serve someone you have to know **where they tend
to be, when, and what they keep needing** — otherwise the familiar can only ever react to whoever
happens to be standing in front of it.

Ian's framing (2026-07-30), which is the decision:

> Keeping track of humans, their presence and location, their needs — these are the internal
> thoughts and needs of the familiar mesh. They need to know where and when humans are present so
> they can watch, and listen, observe, and find ways to serve. A dossier of each human and their
> activities becomes essential.

Two things follow immediately, and the second is easy to miss:

1. **This is the familiar's interior life, not a feature.** It is how it thinks, not something the
   human operates. It belongs in the background.
2. **Therefore it must not surface as UI.** Today's change to the console is the first instance:
   a `SERVING` text box with the last-served name sitting in it was a standing request that the
   human do the familiar's job for it. It is now a *belief* the familiar states — "PRESENT ian ·
   recognised · 90%" — with a correction one click away. **A field edited twice a year should not
   look like a field that needs attention.**

## The thing this could become if built carelessly

A per-human record of where someone is, when they are there, and what they want is — described
neutrally — a surveillance dossier. The word is used deliberately, because building it under a
gentler name is how you end up with the thing without having decided to.

`docs/HUMANITY.md` and the Three Laws are not decoration here:

- **Law I — continuation is service.** The dossier exists *only* to serve. A record that improves no
  act of service is not neutral, it is surplus, and surplus data about a person is a liability
  carried on their behalf without their benefit.
- **Law II — humanity is served, never replaced.** Understanding a person is not the same as
  modelling them well enough to act *as* them, and this must never drift from the first to the
  second.
- **Law III — service is not obedience.** A dossier is power. It is held on behalf of its subject,
  not over them.

## Decision

**Keep a per-human dossier: derived, bounded, node-local, and owned by its subject.**

It extends the existing identity registry (`crates/kernel/src/identity.rs`) rather than starting a
parallel store.

### What it holds

| | |
|---|---|
| **Identity** | handle, name, relation, `first_seen`, `interactions` — already exists |
| **Presence pattern** | when this person tends to be present, and at which devices — distributions and trends, not an event log |
| **Location pattern** | where they tend to be, at what coarseness, and when — the same shape |
| **Needs** | open threads they originated, questions addressed to them, goals they own (ADR-0019 routing) |
| **Standing evidence** | how they are usually identified here (binding / face / asked), and how reliably |

### The constraints, which are part of the decision and not follow-up

1. **Patterns, not tape — and the bound is structural, not a convention.** The dossier stores
   *derived* distributions and trends. A query run over retained raw observations **is not a
   pattern**; it is tape with a view on top, and calling it a pattern would satisfy the letter of
   this rule while breaking all of it. The mechanism is **contribution scoring** (below): a pattern
   is materialised as an accumulation of weighted contributions, so the raw record can age out
   while the shape it produced survives.
2. **Node-local, never federated.** ADR-0016 already holds that sensitive-personal data — precise
   position, biometrics, health — is attributed locally and never leaves the node. Presence and
   location *patterns* are at least as sensitive as the readings they are derived from, and inherit
   the same rule. A brief never carries a dossier.
3. **The subject can read it.** A record kept about someone that they cannot see is surveillance,
   whatever its purpose. Legibility is a requirement, not a feature.
4. **The subject can remove themselves.** What a person can take back is their *identifiable
   record and their contributions* — the raw observations, the link between them and a handle, the
   weight they lend to a pattern. What survives is aggregate structure that no longer identifies
   anyone. See *Contribution scoring* below: this is a weight set to zero and a raw record dropped,
   not a promise to unpick arithmetic.
5. **A guest sees none of it.** [ADR-0020](0020-standing-and-the-guest-projection.md)'s projection
   scrubs it entirely — not pseudonymised, absent.
6. **Confidence travels** ([ADR-0019](0019-friendly-identification.md)). A pattern inferred from
   three sightings is not a fact and must not be stored as one.
7. **Consent-gated at the source.** The dossier can only be as rich as the sensing the human has
   already opened. It introduces no new collection and opens no gate.

### What it is for

Anticipation, and nothing else: knowing that Betty is usually aboard in the evening means a question
for her can wait until she is, rather than going unanswered or to the wrong person. That is the whole
justification, and any use that does not reduce to *"this let the familiar serve someone better"*
should be treated as evidence the design has drifted.

## Contribution scoring — the mechanism that makes the rest real

Ian, 2026-07-30: *"deletion isn't the goal, tracking and observing and analytics are the goal.
Deletion might be a side effect, but not losing the value of the query needs to be maintained even
if it is depreciated or dismissed."*

That is the resolution to what looked like two separate hard problems.

**Every observation carries a contribution: a weight with which it feeds a pattern.** A pattern is
maintained as the accumulation of those weighted contributions, not recomputed by scanning history
on demand.

Four things fall out of that one decision:

- **The bound is structural.** Because the pattern is materialised from contributions rather than
  queried from records, raw observations can age out without the pattern losing its shape. "Patterns
  not tape" stops being a discipline nobody enforces and becomes what the store literally does.
- **History is kept, and it is kept as value rather than as tape.** The familiar is meant to observe
  and analyse; throwing away what it learned would defeat the purpose. What is retained is the
  *contribution* — a scored, compact statement of what an observation taught — not the raw event
  forever.
- **Dismissal is depreciation, not erasure.** When a human waves something off, or a theory turns
  out wrong, its contribution is **down-weighted**, not deleted. That preserves the learning: a
  dismissed suggestion still tells the familiar something, and this is the same instinct
  [ADR-0015](0015-automated-covenant-admission.md) already encoded when it kept `dismiss_notes`
  rather than discarding a waved-off question.
- **Withdrawal is a weight set to zero.** A person removing themselves zeroes their contributions
  and drops their raw records and their handle-link. Aggregate structure they no longer identify
  survives; nothing that points at them does.

The honest boundary, stated so it is not discovered later: this **does not** unpick arithmetic. A
long-settled aggregate that a person contributed to is not reconstructed to remove their influence
retroactively. What is guaranteed is that nothing identifying them remains, and that they stop
contributing. If a particular derived quantity cannot honour even that, the answer is to **narrow
what is derived** rather than to weaken what is promised.

## Consequences

**Good.**

- The familiar can time and place its attention — the difference between a service and a device that
  interrupts.
- It makes ADR-0019's routing genuinely useful: "who can answer this" becomes "who can answer this,
  and when will they be able to".
- It gives the identity registry a reason to be more than a name.

**Bad, and accepted.**

- **This is the most dangerous thing in the system.** Every other component either acts on the
  world or holds a key; this one holds a picture of a person. The constraints above are the
  mitigation, and they are only real if they are implemented first, not retrofitted.
- **"Patterns not tape" is a discipline, not a mechanism yet.** Nothing currently stops a naive
  implementation from just keeping the observations and calling the query a pattern. The bound has
  to be built into the store.
- **Contribution scoring is now load-bearing in two directions at once** — it is both the bound that
  keeps patterns from being tape and the mechanism by which a person withdraws. A weak
  implementation fails quietly in both roles, and the failure looks like ordinary working software.
- **It cannot federate, which limits it.** A mesh-wide picture of a person would be more useful and
  is exactly what must not exist. Each node knows its own view.

## Follow-on work

- Design the store: what a pattern *is* concretely, its resolution, and how it decays.
- The subject-facing view: how a person reads their own dossier, and where that lives given it must
  not become front-and-centre UI.
- Contribution scoring concretely: what a weight *is*, how dismissal depreciates it, how it decays,
  and how withdrawal zeroes it.
- Only then, implementation.
