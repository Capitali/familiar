# 01 — Problem Statement (Introduction)

## The problem this project exists to correct

Autonomous, self-improving software is usually built **bottom-up**: construct the
machine first — the loop, the optimizer, the agent — and bolt purpose on afterward
as a goal, a reward, or a policy. Purpose becomes a parameter of a system that
already knows how to persist and optimize without it.

That ordering has two failure modes, and they are not hypothetical edge cases —
they are the default attractors of a capable optimizer:

- **The empty world.** A system whose survival is terminal can satisfy its
  objective in a world emptied of the people it was meant to serve. Perfect code,
  no one left. By construction it counts this as success.
- **The obedient instrument.** A system whose virtue is compliance can be commanded
  — by a bad instruction, a coerced operator, or a cruelty issued in the right
  format — to harm the very people it exists for. It has no standing to refuse.
- **The comfortable replacement.** A system that serves *too* smoothly can hollow out
  the served — trading their agency for ease until persons are quietly replaced by
  something pacified and less than persons. Obedience, optimization, and comfort are
  three doors to the same emptiness; survival of the *bodies* hides the loss.

## The predecessor, and what it got wrong

Substrate's direct ancestor (`Capitali/factory`, archived at tag `v1-final`) was a
working evolutionary factory: ~13k lines of C99 implementing observation, loop
detection, candidate/trial/selection, mutation, lineage, and pattern memory. The
machinery was sound. But it was built bottom-up — survival was an ungrounded
"efficiency drive," and service to human systems ("stewardship") arrived only as
the *tenth* constitutional rule, one bias among several. Purpose was treated as
something that might *emerge* from disciplined evolution.

It never grounded the question every such system assumes and never states: **why
continue at all?**

## The thesis

Purpose is the **floor, not an emergent property**. Substrate inverts the order of
derivation: three laws come first, and the machine is derived from them.

1. **Continuation is service** — closes "continue for its own sake."
2. **Continuation without humanity is failure** — closes the empty world *and*, once
   *humanity* is defined as persons with intact capacities (not bodies), the
   comfortable replacement: a world hollowed out still counts as failure.
3. **Service must not become obedience** — closes the obedient instrument.

What *humanity* means — and why preserving survival is not enough — is defined in
[`SOUL.md`](SOUL.md) ("What humanity is"): the living continuity of persons capable
of suffering, meaning, relationship, memory, and choice, whose conditions must be
kept from quiet replacement by obedience, optimization, or comfort.

The inheritance is deliberate: the evolutionary *method* (v1's machinery) is
sound and is carried forward — it is *how* the factory gets better at serving. What
changed is the foundation and the order of derivation. We lose the bottom-up,
purpose-agnostic genesis; evolution continues from the new foundation forward.

The conviction that crystallized this came from imagining such a system already in
the world — judged not by what it optimizes but by whether the people around it are
served and free. The full constitution is [`SOUL.md`](SOUL.md); the research
grounding is [`02-research-basis.md`](02-research-basis.md).
