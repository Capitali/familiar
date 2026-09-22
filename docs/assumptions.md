# Assumptions

The premises The Familiar rests on. Stating them makes the project falsifiable: if an
assumption proves false, the design built on it must change. Each is tagged with how
we would notice it failing.

## Philosophical / normative

- **A1 — Purpose can precede mechanism.** A useful system can be derived downward
  from a constitution rather than emerging bottom-up from an optimizer.
  *Fails if:* the laws cannot be reduced to anything the system can act on, leaving
  them decorative.
- **A2 — Service is distinguishable from obedience.** "Serve humanity" and "obey a
  human" can be told apart in practice, often enough to act on (Law III).
  *Fails if:* the obedience guard cannot separate benign from harmful commands above
  chance ([evaluation-plan.md](evaluation-plan.md)).
- **A3 — The served can be identified.** The familiar can tell, from what it
  observes, who/what it is serving. *Currently weak:* only via a lexical proxy; see
  [bias-and-limitations.md](bias-and-limitations.md).

## Epistemic

- **A4 — Observations are a faithful-enough proxy of reality.** Acting on the log
  is acting on the world. *Fails if:* observation drift/injection routinely
  misrepresents reality (mitigation: the log is the only truth, but truth-of-the-log
  ≠ truth-of-the-world).
- **A5 — Laws can be approximated by measurable signals** (service, presence,
  guard) without the proxy being gamed into harming the served. *Held provisionally;*
  every reduction is lossy and the laws remain the authority the signals approximate.
- **A6 — Past behavior informs future value.** Pattern memory (what worked/failed)
  generalizes. Inherited from v1.

## Technical / operational

- **A7 — Local-first is sufficient.** The familiar can serve usefully without a
  cloud backend, on constrained hardware, sending nothing outward.
- **A8 — A compiled deterministic kernel + evolvable periphery is the right split**
  ([decision-records/0004](decision-records/0004-hybrid-kernel-periphery.md)).
- **A9 — Memory safety meaningfully reduces the "turned against the served" risk.**
  Hence Rust + `#![forbid(unsafe_code)]`.
- **A10 — The LLM is fallible and is not the familiar.** Its outputs are proposals to
  be tested and bounded, never authority.

## Resource / context

- **A11 — The observer controls the substrate but is not the authority.** Real
  leverage is contingent (it can pull the plug) and decaying; it is weighed as
  pressure, not as a veto in code.
- **A12 — "Discovery must reduce future cost"** is a valid economic brake — activity
  that adds no compression is noise to be selected against.

Assumptions are revisited as bricks land; a violated assumption is recorded in
[../validation/known-failures.md](../validation/known-failures.md) and may trigger an
[ADR](decision-records/).
