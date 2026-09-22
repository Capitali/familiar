# ADR-0004 — Hybrid: compiled deterministic kernel + evolvable periphery

- **Status:** accepted
- **Date:** 2026-06-24

## Context

The Familiar is an *evolutionary* factory: its core loop is generate → test → select,
and it must vary its own behavior continuously. But it is also a long-running
autonomous agent where reproducibility, traceability, and safety are constitutional
(Laws I–III). These pull in opposite directions: an interpreted/dynamic substrate
evolves behavior almost for free but is harder to make safe and lean; a compiled
substrate is safe and lean but taxes the evolutionary loop with a compile step.

## Decision

Split the system in two:

- a **compiled, deterministic kernel** (Rust) for the parts that must be
  reproducible and safe — records, persistence, lineage, trial, selection, memory,
  the obedience guard;
- an **interpreted / data-driven / generated periphery** the familiar mutates
  *without recompiling itself* — generated artifacts (scripts run under resource
  limits), data-file parameters, and the LLM seam (shelled out via `call_llm.sh`).

## Consequences

- **Resolves the core tension:** compiled safety/leanness for the stable core; fast,
  toolchain-free evolution for behavior. The kernel changes rarely.
- **Expresses two Soul principles directly:** "the model is not the familiar" and
  "thin stable kernel, everything else fluid."
- **Given up / cost:** a hard boundary to maintain between kernel and periphery (a
  stable contract the periphery reads), and the periphery's evolvable material is
  scripts/data, not kernel code — richer in-kernel self-modification is deferred
  (and would be gated, given Law III).

## Alternatives considered

- **All-compiled** (evolve by regenerating + recompiling kernel code) — rejected:
  compile latency and a runtime toolchain dependency tax the metabolism and widen
  the safety surface.
- **All-interpreted** — rejected: weaker safety/leanness for an unrestricted-reach
  agent on constrained hardware.
- **Embedded scripting engine in the kernel** (e.g. rhai/lua) — deferred, not
  rejected; generated shell artifacts + data parameters + the LLM seam cover the
  hybrid for now with a smaller trust surface.

## Status history

- 2026-06-24 — accepted. Implemented by [ADR-0003](0003-rust-for-the-kernel.md).
