# ADR-0003 — Rust for the deterministic kernel

- **Status:** accepted
- **Date:** 2026-06-24

## Context

The kernel is the compiled, deterministic core of the [hybrid architecture](0004-hybrid-kernel-periphery.md):
a long-running autonomous process with **unrestricted local and network reach**,
intended to run on constrained hardware (Raspberry Pi, armv7, routers) — *where the
served are*. The language choice is bounded by the Three Laws ([SOUL.md](../SOUL.md))
and by v1's experience (hand-rolled JSON was a recurring cost).

## Decision

Write the kernel in **Rust**.

## Consequences

- **Law III made literal.** `crates/kernel` carries `#![forbid(unsafe_code)]`.
  Memory safety is treated as constitutional: a memory-corruption defect in an
  unrestricted-reach agent is exactly a "turned against the served" path.
- **Law I served.** No GC, tiny static binaries, low footprint on small hosts;
  good cross-compilation (rustup targets) to reach that hardware.
- **Ergonomics.** `serde` removes v1's hand-rolled-JSON pain.
- **Cost / given up:** slower compiles and a steeper language than Go/C. This is
  absorbed by the thin-stable-kernel design — behavior evolves in the periphery,
  so the kernel recompiles rarely.

## Alternatives considered

- **Go** — memory-safe, fast to write, superb cross-compilation; rejected for the
  GC and heavier binaries on tiny hardware, and a larger runtime/trust surface.
- **Zig** — lean and C-adjacent; rejected as pre-1.0, too unstable for a long-lived
  foundation.
- **Stay with C (v1's language)** — cheapest port; rejected because memory-unsafety
  in an unrestricted-reach autonomous agent is the Law III liability being left behind.

## Status history

- 2026-06-24 — accepted.
