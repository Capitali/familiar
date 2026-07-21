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

What is **enforced** today: the per-capability gate (nine, each fail-closed — network / LLM /
tool-install / execute / authored-execute / camera / mesh / agent / self-upgrade),
path read/write scope (three-valued — in / ambiguous / out), the consequence and sensitivity
gates, and — for any executed artifact — a **constitutional pre-execution review**
(`cycle::review_script`) that reads the script and refuses (records, never runs) plainly
harmful actions: destructive wipes, reading secrets, exfiltration, privilege escalation,
or tampering with its own boundary.

**Peer federation — `allow_mesh`.** Sharing tools/knowledge/humans with peer familiars over
the tailnet is outward transmission — the exfiltration surface Law III guards, at node-to-node
scale. `allow_mesh` is fail-closed (`false` by default and for any pre-existing boundary file),
opened only by a human who also enrolls a group credential. *Discovering* that peers exist is
perception; *exchanging briefs* is gated. Identity sharing is a further, per-human/per-group
opt-in on top of the open gate (the guard routes an identity-bearing share through
`SeekConsent`). Merged peer tools still pass `review_script` + sandbox + `allow_execute` on use.
Full protocol and threat model in [mesh.md](mesh.md).

**Delegation and self-modification — the two sharpest gates.** `allow_agent` opens the
boundary-mediated agentic loop; a delegated agent runs under a *scoped* boundary that always
drops `allow_agent` (no sub-agents) and `allow_self_upgrade` (a delegate never rewrites the
core). `allow_self_upgrade` — the familiar replacing its own running build — is fail-closed
and dormant until a human deliberately opens it; today only the orderable build version and
the gate exist (no upgrade path acts on them).

**Executing generated code — the sandbox is a human-owned toggle.** Running an artifact
needs `allow_execute`; running a *model-authored* one needs the further
`allow_authored_execute`. Both run under a resource sandbox (`ulimit`/wall-timeout) by
default (`sandbox_execution: true`). The human may set `sandbox_execution: false` — then
authored code runs **unconfined**, bound by the pre-execution review and a liveness
timeout only, not by a jail. This is a deliberate, recorded choice consistent with
"capability is unrestricted; restraint is constitutional."

What is **not yet mechanically enforced** — and so remains a binding *norm* — is full
confinement of data-flow *within* a granted capability: the pre-execution review is a
conservative heuristic (it cannot catch every hostile script), a permitted network/LLM
call is not egress-filtered, and `external_boundary` / `sensitive` must be *supplied* by
the caller, not autonomously discovered. OS-level sandboxing (least-privilege user,
namespaces, seccomp), egress filtering, and autonomous signal discovery are tracked as
hardening — see [06-limitations.md](06-limitations.md) and [07-roadmap.md](07-roadmap.md).
See also
[decision-records/0005-human-owned-capability-boundary.md](decision-records/0005-human-owned-capability-boundary.md).
