# Test Plan

How The Familiar is verified. The test suite is the executable specification; the
green bar is the gate.

## The green bar (enforced on every change, by CI)

```
cargo fmt --check          # formatting
cargo clippy -- -D warnings # lint; warnings are errors
cargo test                  # all tests pass
```

plus **no `unsafe` in `crates/kernel`** (compile-enforced by `#![forbid(unsafe_code)]`).

## Layers of testing

1. **Unit tests** (`#[cfg(test)]` in each kernel module) — the primary layer today.
   Pure functions tested directly; I/O tested against throwaway temp dirs.
2. **End-to-end CLI checks** — run the `substrate` binary over an isolated
   `--data-dir` and assert observable behavior (see [reproducibility.md](reproducibility.md)).
3. **Invariant tests** (incoming with the kernel port) — each inherited invariant
   becomes an explicit test; see the table below.

## Current coverage (v0.1.0)

| Area | Tests |
|---|---|
| `store` (JSONL) | missing-file-empty, append/load round-trip in order, blank-skip / malformed-error |
| `observation` | sequential id (`obs-NNNN`), round-trip field fidelity, explicit id preserved |
| `service` (Law I) | classifier markers (case-insensitive) vs bare names, zero-when-none, monotonic rise, empty-log-zero |

Total: **9 unit tests**, all passing.

## Invariants to be covered during the kernel port (Brick 5)

Each becomes a named test (source: v1, see [../docs/04-methodology.md](../docs/04-methodology.md)):

- promotion threshold `= 0.70 + rigor × 0.25`
- pattern suppression only if `neg > pos`, never empties the trait set
- rigor = noisy-OR(promotion-rate, redundancy), confidence ramps with sample size
- Weismann barrier: somatic state never feeds the genotype/spec
- regression guard: unchanged hypothesis + empty changed-traits ⇒ blocked retry
- fingerprint = structural change only (excludes transient telemetry)
- complexity = measured cost (CPU/RSS/output), saturating caps

## What is intentionally not yet tested

Performance/footprint benchmarks ([benchmark-results.md](benchmark-results.md)) and
service-proxy fidelity ([accuracy-metrics.md](accuracy-metrics.md)) — both await the
relevant machinery. Gaps are tracked honestly in [known-failures.md](known-failures.md).
