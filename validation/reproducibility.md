# Reproducibility

The Familiar aims to be reproducible end-to-end — a FAIR4RS requirement and a
precondition for trusting any result it reports.

## Environment

- **Toolchain:** Rust stable via `rustup`; verified on `rustc 1.96.0`. The exact
  version used for a result is recorded with that result.
- **Dependencies:** pinned by `Cargo.lock` (committed). Only `serde` / `serde_json`
  at present. No network needed at runtime; Cargo fetches deps on first build.
- **Platform:** developed on macOS (x86_64); the kernel is `#![forbid(unsafe_code)]`
  and platform-agnostic. Cross-compilation targets (e.g. armv7) are added via
  `rustup target add` when needed.

## Determinism guarantees

- The kernel is clock-free: timestamps are stamped by the CLI, not the kernel, so
  kernel computations (e.g. the service signal) are deterministic functions of their
  inputs.
- Derived views are pure functions of the observation log and can be rebuilt at any
  time; the log is the only source of truth.
- State is isolated per `--data-dir`; a fresh directory guarantees a clean run.

## Reproduce the full green bar

```sh
git clone https://github.com/Capitali/familiar && cd substrate
cargo fmt --check
cargo clippy -- -D warnings
cargo test           # expect all tests passing
```

CI runs exactly this on every push/PR ([../.github/workflows/ci.yml](../.github/workflows/ci.yml)),
so `main` is reproducibly green.

## Reproduce specific results

Per-experiment reproduction steps live with each experiment, e.g.
[../experiments/experiment-001/reproducibility.md](../experiments/experiment-001/reproducibility.md).
Results are reported with their date and toolchain version and appended, never
silently overwritten.
