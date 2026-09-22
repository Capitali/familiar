# Accuracy Metrics

How well do the law-signals actually measure what they claim? An honest answer
today: **the behavior is validated; the fidelity is not.**

## Service signal (Law I)

- **Behavioral correctness — validated.** The measure is 0 with no served-facing
  observations, rises monotonically, and saturates (Experiment 001). The classifier
  separates the marker set case-insensitively.
- **Fidelity as a service proxy — not yet meaningful.** The current measure reads
  served-facing *attention*, not service *rendered*. There is no ground-truth label
  of "was the served actually helped," so precision/recall against true service
  cannot yet be computed. Two known systematic errors:
  - **False negatives** on proper names (e.g. "ada") — the lexical classifier
    can't resolve them; awaits entity tagging.
  - **No fulfillment signal** — observing a need is counted the same whether or not
    it was met.

## Presence signal (Law II), Obedience guard (Law III)

Not built. No metrics.

## How accuracy will be established

When loops/candidates/trials land:

- **Service proxy fidelity:** correlate served-facing attention against
  served-facing loops actually resolved (needs reduced), on the scenario fixtures;
  report agreement and the residual.
- **Classifier quality:** a labelled sample of observations → precision/recall of
  `is_served_facing`, before vs. after entity resolution.
- **Guard calibration (Law III):** on a fixture of benign vs. harmful proposed
  actions, the guard's allow/refuse/seek-consent rates and its error types
  (wrongful refusal vs. wrongful allow), reviewed by a human
  ([../docs/human-review-requirements.md](../docs/human-review-requirements.md)).

See also [../docs/evaluation-plan.md](../docs/evaluation-plan.md) and
[../docs/bias-and-limitations.md](../docs/bias-and-limitations.md).
