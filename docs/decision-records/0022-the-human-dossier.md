# ADR-0022 — The dossier: remembering a human well enough to serve them

- **Status:** accepted (design) — not yet implemented
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

1. **Patterns, not tape.** The dossier stores *derived* distributions and trends that decay. It is
   not an archive of everything a person has ever done. This follows the transience principle
   ([ADR-0018](0018-lighthouse-single-fixture.md)): the familiar should remember the shape of a
   person's life, not keep a recording of it.
2. **Node-local, never federated.** ADR-0016 already holds that sensitive-personal data — precise
   position, biometrics, health — is attributed locally and never leaves the node. Presence and
   location *patterns* are at least as sensitive as the readings they are derived from, and inherit
   the same rule. A brief never carries a dossier.
3. **The subject can read it.** A record kept about someone that they cannot see is surveillance,
   whatever its purpose. Legibility is a requirement, not a feature.
4. **The subject can delete it.** "Forget this" must actually forget. Not tombstoned, not retained
   for consistency — gone, and gone from the derived patterns too.
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
- **Deletion is genuinely hard.** Derived patterns mix people; removing one person's contribution
  from a trend is not a row delete. If that turns out to be intractable, the honest answer is to
  narrow what is derived, not to weaken the promise.
- **It cannot federate, which limits it.** A mesh-wide picture of a person would be more useful and
  is exactly what must not exist. Each node knows its own view.

## Follow-on work

- Design the store: what a pattern *is* concretely, its resolution, and how it decays.
- The subject-facing view: how a person reads their own dossier, and where that lives given it must
  not become front-and-centre UI.
- Deletion semantics, including the derived-pattern problem above.
- Only then, implementation.
