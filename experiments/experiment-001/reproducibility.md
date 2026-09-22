# Experiment 001 — Reproducibility

## Environment

- Rust (stable) via `rustup`; verified on `rustc 1.96.0`.
- No network required; no external services. Dependencies (`serde`, `serde_json`)
  are fetched by Cargo on first build.

## Reproduce the unit-level result

```sh
cargo test -p familiar-kernel service
```

Expect the `service::tests::*` cases to pass.

## Reproduce the end-to-end result

```sh
D=$(mktemp -d)
cargo run -q -p familiar-cli -- observe --actor host --action reports --object cpu_load --data-dir "$D"
cargo run -q -p familiar-cli -- service --data-dir "$D"
#   -> service signal 0.00 (...); "continuation unjustified by service (Law I)"

cargo run -q -p familiar-cli -- observe --actor client --action requests --object status_report --data-dir "$D"
cargo run -q -p familiar-cli -- observe --actor support_team --action resolves --object customer_ticket --data-dir "$D"
cargo run -q -p familiar-cli -- service --data-dir "$D"
#   -> service signal 0.40 (2 of 3 ...; e.g. client)
rm -rf "$D"
```

## Determinism notes

- The measure depends only on observation `actor`/`object` text, not on timestamps,
  ids, or ordering — so reruns give identical `measure`/`served_facing`/`total`.
- The `--data-dir` isolates state; using a fresh directory guarantees a clean run.
- Inputs are inline in the commands above (no external data file needed); a sample
  log is also available at [../../data/sample/observations.jsonl](../../data/sample/observations.jsonl).
