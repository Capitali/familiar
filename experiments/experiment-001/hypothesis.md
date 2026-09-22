# Experiment 001 — Hypothesis

## Question

Can **Law I** ("continuation is service") be made *measurable* from the
observation log alone, at the bootstrap stage — before loops, candidates, or trials
exist?

## Hypothesis

A simple, saturating measure of **served-facing attention** — the count of
observations whose actor or object names a served (human-system) entity — is a
usable cold-start proxy for "is the familiar serving?". Specifically:

- **H1.** The measure is **0** when no observation touches the served, and the
  factory can report this as "continuation unjustified by service."
- **H2.** The measure **rises monotonically** with served-facing observations and
  **saturates** (so one mention is not read as full service).
- **H3.** A tight lexical classifier (ported from v1's `domain_is_steward`)
  separates served-facing terms (`client`, `customer`, `user`, …) from
  host-internal ones (`cpu_load`, `disk`) **case-insensitively**.

## Null / failure conditions

- The measure is non-zero on a purely host-internal log (H1 fails), or
- it does not increase with added served-facing observations (H2 fails), or
- the classifier mis-separates the marker set (H3 fails).

## Known scope limit (predicted, not a failure)

The lexical classifier will **not** recognize bare proper names (e.g. "ada") as
served-facing; that requires entity resolution (the world-model port). This is
expected and documented, not a refutation — it bounds the proxy's reach.

See [method.md](method.md), [results.md](results.md), [reproducibility.md](reproducibility.md).
