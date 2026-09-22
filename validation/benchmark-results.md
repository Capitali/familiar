# Benchmark Results

> **Status: no benchmarks yet.** This file is a placeholder with the *plan*, kept
> here so the gap is visible rather than implied-covered.

## Why nothing here yet

The Familiar is at bootstrap: the metabolism (the autonomous cycle) is not yet built,
so there is no steady-state workload to benchmark. Publishing fabricated or
premature numbers would violate the honesty this evidence repository is for.

## Planned benchmarks (matched to the Laws)

When the cycle and kernel land, measure:

- **Footprint (Law I — cheap survival):** resident memory and binary size at idle
  and under a representative observation rate; cross-compiled to a constrained
  target (armv7) as well as the dev host.
- **Tick cost:** wall-clock and CPU per cycle tick at varying observation/loop
  counts; confirm it scales sub-linearly with log size where derived views are cached.
- **Persistence:** append and full-load throughput for JSONL at 10³–10⁶ records;
  the decision point for whether/when SQLite is warranted.
- **Cold-start:** time-to-first-useful-signal from an empty data dir.

## Method (when run)

Recorded with the toolchain version, host, and target triple; inputs reproducible
from a fixed sample log; numbers reported with variance, not single runs. Results
will be appended here with dates, never overwritten silently.
