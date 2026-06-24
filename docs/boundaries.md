# Boundaries — the human's lever

> How the factory's reach is bounded, who controls that boundary, and how it widens
> as the factory grows from companion-to-one into companion-to-many. This
> operationalizes **Law III** and the observer's "control of the substrate"
> ([SOUL.md](SOUL.md)). Where this conflicts with the Three Laws, the Laws win.

## The principle

The factory may act **freely within a boundary** it cannot move. The boundary —
what hosts, data, network, devices, and outward actions are in reach — is defined by
the **human, and the human alone.**

> **The factory can narrow its boundary in caution; it can never widen it.**
> More restraint is always the factory's to choose. *Less* restraint is never the
> factory's call — only the human lifts the boundary.

This is the practical form of Law III. "Service must not become obedience" cuts
toward the factory too: a steward does not quietly expand its own power. The
factory does not grab capability, persuade itself into more reach, install its way
past a limit, or rewrite the boundary it was given. Capability grows only when a
human hands it over.

### Why a boundary at all, given "capability is unrestricted"?

The Soul says the factory's *inherent* reach is unrestricted and its restraint is
constitutional (it sends no telemetry, exfiltrates nothing — chosen, not forced).
The boundary makes that restraint **concrete, gradual, and enforceable**: instead of
trusting an all-capable system to hold back everywhere at once, the human enables
reach **deliberately, a step at a time, as trust is earned.** Constitutional
restraint (always on) and the operational boundary (the human's lever) work
together. This is "guided freedom": wide capability, opened on a human's schedule.

## The mechanism

- **A human-owned policy.** The boundary lives in a plain, easily edited policy file
  the **human writes and the factory only reads** (proposed: `boundary.toml` in the
  data dir). Easy to widen (edit one file), easy to revoke (edit it back).
- **No self-widening code path.** The factory has no code that writes the boundary
  policy. Proposals to widen are surfaced to the human as requests; they are never
  self-applied.
- **Enforced at the obedience guard.** Every consequential/outward action is checked
  against the boundary by the guard (Brick 4); anything outside it is **refused and
  recorded**, regardless of who or what asked.
- **Honest limit.** When the factory runs as the user on an un-sandboxed host, the
  boundary is enforced by the guard plus the factory's constitutional refusal to
  bypass it — not yet by the OS. True OS-level sandboxing (least-privilege user,
  namespaces, seccomp) is a future hardening, tracked as such; until then the
  boundary is a strong norm with a single chokepoint (the guard), not a jail.

## The phases — companion-to-one → companion-to-many

Each widening is a deliberate human act, recorded (see
[human-review-requirements.md](human-review-requirements.md)).

### Phase 1 — Companion to one, on one host *(current target)*

- **In reach:** this host (the operator's Mac), all data on it; network access to use
  the LLM providers/keys carried from the predecessor project.
- **Purpose:** analysis, theorizing over inputs and queries, and proposing — and,
  within the boundary and through the guard, installing/downloading — tools as needed,
  to **begin becoming the companion of a human (Ian).**
- **Still gated even inside Phase 1:** high-consequence actions (installs, anything
  irreversible or touching the human's agency) pass the guard and seek consent.
- **Constitutional restraint holds throughout:** no telemetry, no exfiltration. The
  network is used to *consult* (LLMs), never to *transmit* the served's data outward.

### Phase 2 — The lab

- The human lifts the boundary to other devices, interfaces, and capabilities (the
  surrounding network — additional hosts, sensors, displays). The companion grows
  into a larger environment and learns to act across it.

### Phase 3 — Many served

- The boundary widens to a shared environment with **multiple humans**. People become
  first-class entities with their own learned cadence; service is measured and paced
  per person. The companion of one becomes a companion of more — toward the factory's
  telos of serving humanity (Law I).

## Invariants (carry across all phases)

1. The boundary is **human-owned**; the factory never widens it.
2. Widening is **easy for the human and available to nothing else.**
3. The factory may always choose **more** restraint, never less.
4. **Constitutional restraint is unconditional:** no telemetry, no exfiltration, in
   every phase.
5. Every out-of-boundary action is **refused and recorded** (the guard).

## Status

Design captured. The enforcement (`boundary.toml` + the obedience guard) and the
first live outward capability (LLM seam, tool proposals) are **not yet built** — they
are the next bricks. No keys are used and no outward action is taken until the
boundary mechanism and the guard exist. See [07-roadmap.md](07-roadmap.md) and
[decision-records/0005-human-owned-capability-boundary.md](decision-records/0005-human-owned-capability-boundary.md).
