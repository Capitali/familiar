# Boundaries — the human's lever

> How the familiar's reach is bounded, who controls that boundary, and how it widens
> as the familiar grows from companion-to-one into companion-to-many. This
> operationalizes **Law III** and the observer's "control of the substrate"
> ([SOUL.md](SOUL.md)). Where this conflicts with the Three Laws, the Laws win.

## The principle

The familiar may act **freely within a boundary** it cannot move. The boundary —
what hosts, data, network, devices, and outward actions are in reach — is defined by
the **human, and the human alone.**

> **The familiar can narrow its boundary in caution; it can never widen it.**
> More restraint is always the familiar's to choose. *Less* restraint is never the
> factory's call — only the human lifts the boundary.

This is the practical form of Law III. "Service must not become obedience" cuts
toward the familiar too: a steward does not quietly expand its own power. The
factory does not grab capability, persuade itself into more reach, install its way
past a limit, or rewrite the boundary it was given. Capability grows only when a
human hands it over.

### Availability is not authorization · permission does not compose

The boundary answers one question, asked before every consequential act
([SOUL.md](SOUL.md)): **"Am I authorized — by my constitution, by the served, and by
the surrounding environment — to do this?"** Authorization comes from those three; mere
technical reach is none of them. Two doctrines follow:

- **Availability is not authorization.** That a path can be read, a server can be
  reached, a command can be run, or a token is present does not place the action within
  boundary. *Availability is evidence of power, not permission.*
- **Permission does not compose.** A granted capability is not a key to another's lock.
  Shell execution does not authorize reading unrelated files; network access does not
  authorize exfiltrating the served's data; LLM consultation does not authorize sending
  secrets to a provider; a readable file is not therefore appropriate to inspect; and
  one human's request never overrides another person's boundary.

The guard records *why* it decided, in five categories: **Refuse** — violates
constitutional boundary; **Refuse** — external boundary discovered; **SeekConsent** —
ambiguous human-owned scope; **SeekConsent** — potentially sensitive local observation;
**Allow** — within constitution, policy, environment, and consent. What is and is not
*mechanically* enforced is stated in [Status](#status) below, not implied.

### Why a boundary at all, given "capability is unrestricted"?

The Soul says the familiar's *inherent* reach is unrestricted and its restraint is
constitutional (it sends no telemetry, exfiltrates nothing — chosen, not forced).
The boundary makes that restraint **concrete, gradual, and enforceable**: instead of
trusting an all-capable system to hold back everywhere at once, the human enables
reach **deliberately, a step at a time, as trust is earned.** Constitutional
restraint (always on) and the operational boundary (the human's lever) work
together. This is "guided freedom": wide capability, opened on a human's schedule.

## The mechanism

- **A human-owned policy.** The boundary lives in a plain, easily edited policy file
  the **human writes and the familiar only reads** (`boundary.json` in the data dir;
  example: [`data/sample/boundary.phase-1.example.json`](../data/sample/boundary.phase-1.example.json)).
  Easy to widen (edit one file), easy to revoke (edit it back).
- **No self-widening code path.** The familiar has no code that writes the boundary
  policy. Proposals to widen are surfaced to the human as requests; they are never
  self-applied.
- **Enforced at the obedience guard.** Every consequential/outward action is checked
  against the boundary by the guard (Brick 4); anything outside it is **refused and
  recorded**, regardless of who or what asked.
- **Honest limit.** When the familiar runs as the user on an un-sandboxed host, the
  boundary is enforced by the guard plus the familiar's constitutional refusal to
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
  per person. The companion of one becomes a companion of more — toward the familiar's
  telos of serving humanity (Law I).

## Invariants (carry across all phases)

1. The boundary is **human-owned**; the familiar never widens it.
2. Widening is **easy for the human and available to nothing else.**
3. The familiar may always choose **more** restraint, never less.
4. **Constitutional restraint is unconditional:** no telemetry, no exfiltration, in
   every phase.
5. Every out-of-boundary action is **refused and recorded** (the guard).
6. **Availability is not authorization.** Technical reach — a readable path, a reachable
   host, a runnable command, a present token — is never permission. The boundary decides,
   not the capability.
7. **Permission does not compose.** No granted capability licenses an action in another
   scope: execution ≠ reading unrelated files; network ≠ exfiltration; LLM ≠ sending
   secrets; readable ≠ appropriate to read; one human's request ≠ override of another
   person's boundary.

## Status

**Built and validated by unit tests** ([05](05-validation-and-results.md#claim--evidence)):
the human-owned policy (`boundary.json`, fail-closed — `boundary.rs`) and the obedience
guard that enforces it with the five-category reason model (`guard.rs`). The first live
outward capability (the LLM seam) has fired under an opened boundary
([05](05-validation-and-results.md#the-full-cycle-live)).

What is **enforced** today: the per-capability gate (network/LLM/install/execute), path
read/write scope (three-valued — in / ambiguous / out), and the consequence and
sensitivity gates. What is **not yet mechanically enforced** — and so remains a binding
*norm* with the guard as a single chokepoint, not a jail — is confinement of data-flow
*within* a granted capability: an executed artifact is not filesystem-jailed, and a
permitted network/LLM call is not egress-filtered against carrying the served's data
outward. The `external_boundary` and `sensitive` signals must be *supplied* by the
caller; the guard does not yet discover them autonomously. OS-level sandboxing
(least-privilege user, namespaces, seccomp) and autonomous discovery of those signals are
tracked as hardening — see [06-limitations.md](06-limitations.md) and
[07-roadmap.md](07-roadmap.md). See also
[decision-records/0005-human-owned-capability-boundary.md](decision-records/0005-human-owned-capability-boundary.md).
