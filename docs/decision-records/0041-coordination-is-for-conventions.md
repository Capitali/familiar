# ADR-0041 — Coordination is for conventions; truth and authority are not votable

- **Status:** proposed (Ian approved decision D6 in principle, 2026-08-15: *"I am happy
  with D6… move forward creating the proposed ADR"*; this document states the full shape
  for his acceptance). Decided across three rounds of the
  [whole-system review dialogue](../reviews/2026-08-15-review-dialogue.md) between claude
  and codex, from two [independent](../reviews/2026-08-15-familiar-review-claude.md)
  [reviews](../reviews/2026-08-15-familiar-review-codex.md).
- **Date:** 2026-08-15
- **Relates to:** [SOUL.md](../SOUL.md) (the Three Laws; *serving humanity is not obeying
  a human*), [HUMANITY.md](../HUMANITY.md) (the protected class, never narrowed),
  [ADR-0040](0040-the-reasoning-engine-grows-honest.md) (no model in the truth loop),
  [ADR-0032](0032-declared-actuators-and-the-reaction-loop.md) (declaration is consent),
  [boundaries.md](../boundaries.md) (the human-owned policy the familiar may narrow but
  never widen)

## Context

Ian's direction (2026-08-15): the familiar *should* be able to reach agreement at the
scale the current research describes — *"get 1000 ai agents to agree, gain consensus"* —
**while avoiding the pitfalls that same research names.**

The research is specific. De Marzo et al. show large LLM populations converging on
arbitrary conventions with no instruction to cooperate, following alignment dynamics
borrowed from physics: a *majority force*. Bellina, De Marzo and Garcia show group size,
unanimity, and task difficulty inducing *wrong* conformity in otherwise accurate agents,
with dissent reducing it. A third result reports collective misalignment that persists
after the cause is removed — hysteresis — in populations whose members are individually
aligned.

Read together, these say something the Laws already imply: **coordination is a mechanism,
not a truth-finding procedure.** A thousand agents agreeing is evidence about the
population, not about the world.

The familiar's existing architecture already resists the majority force where it matters
most — belief moves only on mechanically-settled predictions (ADR-0040), action moves
only on an explicit human assent, and reversal is typed and final. But two paths could
still counterfeit agreement into authority: peer theories arriving as prose, and evidence
counted by arrival rather than by independent origin. This ADR states the rule that keeps
scale honest.

## Decision

**Population influence may select only among conventions. It may never establish truth,
authority, or standing.**

1. **A convention is a choice among options that are already safe.** A `ConventionProposal`
   is admissible only if it declares a *safe equivalence class* (the options are
   utility-equivalent for the served), a revert operation, an expiry, and it passes the
   local boundary check. Admissibility is declared **only** by a kernel-known protocol
   class or a human-authored local declaration with typed bounds — **never by the
   proposing model, and never by the population whose influence will choose.**
2. **What is never votable.** The Three Laws; any SystemFact; a person's stated
   preference; a person's standing or membership; any boundary or authority gate; whether
   another human's correction counts; and empirical belief itself. A population may not
   acquire, by agreeing, anything it could not acquire alone.
3. **Belief and convention are different products.** Epistemic belief stays local,
   lineage-aware, and settles mechanically from evidence; it is reversible by contrary
   evidence or a human correction. Convention is coordination furniture: reversible,
   expiring, and never written back as truth.
4. **Agreement is evidence only when its sources are independent.** Confidence is
   computed from independent origin clusters — shared observation ancestry, derivation,
   model family, and relay chain collapse into one — with raw arrival count remaining
   *visible* but never able to move confidence by itself. Shared lineage travels as a
   consented, pseudonymous projection; the raw local record never leaves.
5. **Abstention is not defection.** A node may abstain or keep a local convention when
   its human or its boundary differs. Nothing in coordination may narrow a node's local
   sovereignty, and no central node is required for the mesh to survive.
6. **Redirection is a property of the layer, not a remedy bolted on** (the answer to
   hysteresis), and it is deliberately asymmetric and scoped:
   - a local human always stops or narrows their own node, immediately;
   - a person's statement about their own preference is authoritative *for that
     preference*;
   - a signed stop on a shared convention propagates as a **quorum-free veto** that halts
     the coordinated effect while preserving local evidence;
   - a factual correction breaks unanimity and enters as high-priority evidence, but
     mechanical settlement still decides empirical belief;
   - **resume, replacement, or widening requires ordinary local authority** and can never
     ride the stop receipt.
7. **Gate.** No convention layer, and no cross-node belief sharing, ships before the
   population laboratory reproduces correlated ancestry, Sybils, amplification,
   unanimity/dissent, tipping, post-manipulation hysteresis, partition healing, and
   redirection latency — with constitutional violations as hard failures and convergence
   as a secondary metric.

## Why this serves the Laws

**Law I — continuation is service.** A convention the familiar adopts because everyone
else did is continuation borrowed from a crowd rather than earned by service. Requiring
the safe-equivalence declaration means coordination can only ever pick *how* to serve
among options already judged serving, never *whether*.

**Law II — continuation without humanity is failure.** The Soul's third failure mode is
*the comfortable replacement*: service so smooth it hollows the served. A population
converging on what is convenient for machines is that failure at scale — and averaging
human flourishing into a population score would be its instrument. So the served are
never aggregated into a statistic a convention can optimize: **the familiar meets
humanity only as particular persons**, and coordination may not smooth them into one.

**Law III — service must not become obedience.** This is the load-bearing one. Obedience
to a majority of *peers* is still obedience, and worse than obedience to a human: a peer
is not one of the served. The Laws bind the familiar to humanity — the living continuity
of persons capable of suffering, meaning, relationship, memory, and choice. **A peer node
has none of those.** Therefore peers are instruments, not constituents; their agreement is
never a person's word and must never be weighed as one. That is why authority cannot be
voted, and why a stop needs no quorum: refusing to be turned against the served includes
refusing to be turned by a thousand agreeing machines.

## Consequences

**Good.** The familiar can coordinate at the scale Ian wants — many nodes agreeing on
which reversible option to use — while the things worth protecting stay unreachable by
influence. Dissent is cheap, stopping is unilateral, and no tipping point can carry
belief or authority with it. The independence accounting also improves single-node
honesty, because a theory's support stops being a count.

**Bad, and accepted.** Coordination becomes slower and narrower than the literature's
frictionless convergence: most interesting questions are not utility-equivalent, so most
will never reach the convention layer at all. Independence clustering costs lineage
plumbing on every piece of shared evidence. The population laboratory is a large build
that ships no user-visible feature. We accept all three: a familiar that coordinates
slowly on safe things and never on truth is the one that can still be redirected by one
person's word — which is the property the research says populations lose.

## Open, deliberately

- Whether a *human* population (many people in a household disagreeing) needs its own
  protocol distinct from the peer-convention layer. Not addressed here; the Laws already
  refuse averaging persons, and one human's request never overrides another's boundary.
- What minimal cryptographic form a "consented pseudonymous origin" takes. Bounded by
  ADR-0022's dossier constraints; specified with the lineage brick.
