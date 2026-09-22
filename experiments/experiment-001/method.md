# Experiment 001 — Method

## Implementation under test

- `crates/kernel/src/service.rs` — `service_signal(&[Observation]) -> ServiceSignal`
  with measure `n / (n + 3)` where `n` = served-facing observation count
  (absolute, saturating; faithful to v1's stewardship drive).
- Classifier `names_served(text)` — case-insensitive substring match against the
  marker set `{human, person, people, social, community, client, customer, user,
  steward}` on an observation's `actor` and `object`.
- CLI surface: `substrate observe …` and `substrate service`.

## Procedure

1. **Unit-level** (deterministic, no I/O timing): exercise `service_signal` and
   `names_served` directly with constructed observations — see the `#[cfg(test)]`
   module in `service.rs`. Cases: classifier markers vs bare names; zero-when-none;
   monotonic rise (1 vs 3 served-facing); empty-log-zero.
2. **End-to-end** (CLI over a throwaway data dir): record a host-internal
   observation, read the signal; then record served-facing observations, read again;
   confirm the reported measure and message change as predicted.

## Controls

- A dedicated `--data-dir` per run (no shared state).
- Deterministic inputs (timestamps and ids do not affect the measure).
- The green bar (`fmt`/`clippy`/`test`) must be clean for the build under test.

## Measurements recorded

- `measure` (0..1), `served_facing` count, `total` count, `exemplar`.
- Pass/fail of each unit assertion.
