# ADR-0005 — Human-owned capability boundary, widened in phases

- **Status:** accepted
- **Date:** 2026-06-24

## Context

The factory's inherent reach is unrestricted and its restraint is constitutional
([SOUL.md](../SOUL.md): "capability is unrestricted; restraint is constitutional").
Trusting an all-capable autonomous agent to hold back everywhere at once is a heavy,
brittle ask. The operator (Ian) wants reach enabled **deliberately and gradually**,
under a control only he holds — starting narrow (companion to one human, on one
host) and widening over time into a larger lab and, eventually, multiple served
humans. This is also the practical edge of Law III: a steward must not expand its
own power.

## Decision

Adopt a **human-owned capability boundary**:

- The factory's reach (hosts, data, network, devices, outward actions) is bounded by
  a **plain policy file the human writes and the factory only reads**.
- The factory **can narrow** its boundary in caution but **can never widen** it; it
  has no code path to edit the policy. Widening is a human act, easy for the human
  and available to nothing else.
- The **obedience guard** (Brick 4) enforces the boundary: out-of-boundary actions
  are refused and recorded.
- The boundary **widens in phases** (companion-to-one → the lab → many served), each
  widening a recorded human act. See [boundaries.md](../boundaries.md).

## Consequences

- **Gained:** restraint becomes concrete, gradual, and enforceable; a single human
  chokepoint for capability growth; Law III strengthened (the factory can't self-
  expand); a clear, low-drama "off switch" (edit one file).
- **Reconciles** "unrestricted capability" with safe rollout: capability is wide in
  principle, opened on the human's schedule.
- **Given up / honest limits:** running as the user on an un-sandboxed host, the
  boundary is enforced by the guard + the factory's constitutional refusal, **not yet
  by the OS** — a strong norm with one chokepoint, not a jail. OS-level sandboxing is
  deferred hardening. Also: the boundary is only as good as the guard that enforces
  it, so the guard becomes safety-critical.

## Alternatives considered

- **Pure constitutional restraint (no explicit boundary), as v1 framed it** —
  rejected: asks an all-capable agent to self-limit everywhere from day one; no
  gradual, legible control for the human.
- **OS sandbox first** — deferred, not rejected: stronger, but heavier; the
  guard+policy gets us a usable boundary now, with sandboxing as later hardening.
- **Factory may widen its own boundary with justification** — rejected outright: a
  self-widening steward is exactly the Law III failure.

## Status history

- 2026-06-24 — accepted. Enforcement (policy file + obedience guard) and the first
  in-boundary outward capability are pending bricks; nothing outward runs until they
  exist.
