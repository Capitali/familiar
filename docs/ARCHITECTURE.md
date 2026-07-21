# Architecture

> How The Familiar is built. The *why* is `SOUL.md`; this is the *how*. Where they
> conflict, the Soul wins.

## The hybrid: compiled kernel + evolvable periphery

The Familiar is split in two, deliberately:

- **A compiled, deterministic kernel** (this is `crates/kernel`, in Rust) — the
  records, persistence, lineage, trial, selection, memory, and the obedience
  guard. The parts that must be reproducible, traceable, and safe.
- **An interpreted / data-driven / generated periphery** — the behavior the
  factory mutates *freely, without recompiling itself*: generated artifacts
  (shell scripts run under resource limits), data-file parameters, and the LLM
  seam (`llm/call_llm.sh`, shelled out).

This split is not a compromise; it *is* "the LLM is not the familiar" and "thin
stable kernel, everything else fluid." The slow-to-compile core changes rarely
because evolution happens in the periphery.

## Language: Rust

The kernel is **Rust**, chosen against the Three Laws and the hardware the familiar
should run on (Pis, Cerbo armv7, the router — *where the served are*):

- **Law III (cannot be turned against the served)** makes memory safety
  constitutional, not a nicety. `crates/kernel` carries `#![forbid(unsafe_code)]`
  — the commitment made literal. A long-running autonomous process with
  unrestricted local + network reach must not contain the memory-unsafety that
  becomes a remote-code-execution path.
- **Law I (cheap survival)** wants a lean, no-GC, tiny-static-binary core for
  constrained hardware. Rust gives that without sacrificing safety.
- Minimal dependencies keep the trust surface small and auditable — also Law III.
  The kernel is `serde`/`serde_json` plus one deliberate concession: `rusqlite`
  (`bundled`) for the embedded store (see [storage.md](storage.md)).

## Crate map

```
crates/
  kernel/   familiar-kernel (lib)  — the deterministic core (no unsafe)
    store.rs        the embedded SQLite store (append/load/update; JSONL export/import)
    observation.rs  the observation record (the only truth)
    service.rs      the service signal (Law I)
    presence.rs     the presence signal (Law II)
    capacities.rs   the capacities signal (Law II / HUMANITY.md — comfortable replacement)
    boundary.rs     the human-owned capability boundary (Law III) — nine fail-closed gates
    guard.rs        the obedience guard (Law III)
    loops.rs        loop detection (temporal view of the log)
    candidate.rs · spec.rs   candidates + the heritable genotype (Weismann barrier)
    trial.rs · score.rs · selection.rs · regression_guard.rs   testing & selection
                    (score.rs also scores a theory against its predecessors' outcomes)
    mutation.rs · pattern_memory.rs · lineage.rs   variation, memory, ancestry
    thread.rs       the familiar's questions + theories (the Interpret step)
    question.rs · request.rs · dialog.rs   the interaction channel + the familiar's voice
    activity.rs     the activity feed (what the familiar did, human-readable)
    tool.rs         the durable tool registry (health-tracked, judged by output)
    identity.rs     identities/entities the familiar knows (mesh opt-in sharing)
    humanity.rs     the append-only lived-understanding ledger (HUMANITY.md stays immutable)
    goal.rs · capabilities.rs   the mesh-owned roadmap: goals + what a node can DO
    corruption.rs   the graduated, reversible trust ladder (monitor→throttle→marginalize→sever)
    parameters.rs   the co-owned tuning parameters (parameters.json)
    review.rs       constitutional pre-execution review support
    version.rs      the orderable build version (VERSION + build.rs → self-upgrade)
  sense/    familiar-sense (lib) — perception of the host + LAN device discovery -> observations
  reach/    familiar-reach (lib) — reach assessment: probe discovered devices, classify how the
                                    familiar could extend into each (agent-capable / protocol-
                                    controllable / observable). The input to consent-gated expansion.
  vision/   familiar-vision (lib) — the eye: camera discovery + gated still capture (familiar-eye)
  llm/      familiar-llm (lib)   — the LLM seam: boundary-gated consult (periphery)
  exec/     familiar-exec (lib)  — sandboxed script runner (resource limits + cost)
  agent/    familiar-agent (lib) — the agentic seam: a boundary-mediated, multi-step loop (the
                                    agent proposes one action at a time; the core decides + gates)
  mesh/     familiar-mesh (lib)  — peer federation over the tailnet/LAN: ed25519 group trust, the
                                    covenant handshake, device observation ingestion, tool/pattern/
                                    goal merge, the worldview seam. Carries the crypto + async-HTTP
                                    floor (see mesh.md).
  cycle/    familiar-cycle (lib) — the metabolism: one full tick (sense → detect →
                                    interpret → generate → test → score → select → measure),
                                    plus tool cultivation (the theory→code bridge) and
                                    goal pursuit (the mesh roadmap)
  cli/      familiar-cli (bin: `familiar`) — the shell + daemon control (start/stop/
                                    reload/install via pidfile + launchd: src/daemon.rs)
```

The **SwiftUI consoles** live in [`../ios/`](../ios/) (Swift/SwiftUI, XcodeGen): the macOS
console, the iPhone/iPad apps, and the watch companion. They enrol by the covenant handshake,
push derived observations to a familiar's `/mesh/observe`, and read its `/mesh/worldview` —
thin shells over the Rust core ([ADR-0007](decision-records/0007-one-core-many-shells.md)). The earlier
egui **Glass** and menu-bar **marble** crates are archived under `archive/` (superseded
2026-07-17; see ADR-0006's status history).

## Interfaces

The **SwiftUI consoles** (macOS + iPad/iPhone/watch, [`../ios/`](../ios/)) are the primary
human interface — they show the Three Laws as live meters, the roster/mesh map, the roadmap
board, and carry the dialog with the familiar; devices enrol by covenant and read the
worldview seam. The **CLI** (`familiar`) is retained for scripting, automation, and
headless/CI use. All are thin shells over the same kernel.

## Storage

An **embedded SQLite** store (`crates/kernel/src/store.rs`, `rusqlite` with the `bundled`
feature — no system library) behind the original append/load/update API; `familiar db export`
dumps every table to JSONL for auditability and `db import` folds legacy `.jsonl` in. One logical
table per record type under a data directory (`familiar_data/` by default, `--data-dir` to
override). Local-first and auditable; the familiar sends no telemetry and exfiltrates nothing
(restraint is constitutional). See [storage.md](storage.md).

## Discipline (the green bar)

Every change must pass, with no exceptions:

- `cargo fmt --check`
- `cargo clippy -- -D warnings` (warnings are errors)
- `cargo test`
- no `unsafe` in `crates/kernel` (enforced by `#![forbid(unsafe_code)]`)
