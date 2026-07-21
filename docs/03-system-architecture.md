# 03 — System Architecture (Methods I)

> This is the narrative overview. The living, detailed reference is
> [`ARCHITECTURE.md`](ARCHITECTURE.md); the decisions behind it are in [`decision-records/`](decision-records/).

## The hybrid

The Familiar is split in two, deliberately:

- **A compiled, deterministic kernel** — `crates/kernel` (Rust). The parts that
  must be reproducible, traceable, and safe: records, persistence, lineage, trial,
  selection, memory, and the obedience guard.
- **An interpreted / data-driven / generated periphery** — the behavior the familiar
  mutates *freely, without recompiling itself*: generated artifacts (scripts run
  under resource limits), data-file parameters, and the LLM seam (shelled out).

This split is not a compromise; it *is* the principle "the model is not the familiar"
and "thin stable kernel, everything else fluid." The slow-to-compile core changes
rarely because evolution happens in the periphery.

## Why Rust (decision: [ADR-0003](decision-records/0003-rust-for-the-kernel.md))

Chosen against the Three Laws and the constrained hardware the familiar should run
on (where the served are):

- **Law III** makes memory safety *constitutional*. `crates/kernel` carries
  `#![forbid(unsafe_code)]` — a long-running autonomous process with unrestricted
  reach must not contain the memory-unsafety that becomes a remote-code-execution
  path "turned against the served."
- **Law I** wants a lean, no-GC, tiny-static-binary core for small hosts.
- Minimal dependencies (`serde`/`serde_json`, plus the one deliberate `rusqlite`
  concession for the embedded store) keep the trust surface small and auditable —
  also Law III.

## Crate map

Each entry carries a status from the [status convention](07-roadmap.md#status-convention);
the test or log behind it is in the claim→evidence table
([05](05-validation-and-results.md#claim--evidence)). "unit" = validated by unit tests;
"live" = validated by real-world operation.

```
crates/
  kernel/   familiar-kernel (lib) — the deterministic core         [unit]
    store.rs            embedded SQLite store; JSONL export/import       [unit]
    observation.rs      the observation record (the only truth)          [unit]
    service.rs          the service signal (Law I)                       [live]
    presence.rs         the presence signal (Law II)                     [unit]
    capacities.rs       the comfortable-replacement alarm (Law II)       [unit]
    guard.rs            the obedience guard (Law III)                    [unit]
    boundary.rs         the human-owned capability boundary (Law III)    [unit]
    loops.rs            loop detection over recurring triples            [unit]
    candidate.rs spec.rs       the Weismann barrier (genotype/somatic)   [unit]
    trial.rs score.rs selection.rs regression_guard.rs   the bar+ladder  [unit]
    mutation.rs pattern_memory.rs lineage.rs   suppression + ancestry    [unit]
    thread.rs           a thread = question + theory (Interpret)         [unit]
    goal.rs capabilities.rs   the mesh-owned roadmap + node capabilities [unit]
    corruption.rs       the reversible trust ladder (Law III, outward)   [unit]
    humanity.rs tool.rs identity.rs dialog.rs …   see ARCHITECTURE.md    [unit]
  sense/    familiar-sense (lib) — perceives the host                    [unit]
  vision/   familiar-vision (lib) — the eye: camera discovery (always)   [unit]
                                    + gated frame capture (familiar-eye)  [live]
  cycle/    familiar-cycle (lib) — the metabolism (one tick) + tool      [live]
                                    cultivation + goal pursuit
  exec/     familiar-exec (lib) — the sandboxed runner                   [unit]
  llm/      familiar-llm (lib) — the LLM seam (boundary-gated)           [unit/live]
  agent/    familiar-agent (lib) — the boundary-mediated agentic loop    [unit]
  mesh/     familiar-mesh (lib) — federation, covenant, worldview, trust [live]
  reach/    familiar-reach (lib) — reach assessment + consented install  [live]
  cli/      familiar-cli (bin: `familiar`) — the thin shell             [live]
```

The human interfaces are the **SwiftUI consoles** in [`../ios/`](../ios/) (macOS console +
iPhone/iPad apps + watch companion) — thin shells that enrol by covenant and read the
familiar's worldview [live]. The egui `glass/` and menu-bar `marble/` crates that preceded
them are archived under `archive/` (superseded 2026-07-17 — ADR-0006).

The eye is split like the boundary asks: **discovery** (which cameras exist) is always
permitted, but **watching** (capturing a frame) is gated by `allow_camera`, fail-closed, and
runs only in the daemon's gated tick. Capture shells out to `familiar-eye`, a tiny bundled
Swift/AVFoundation helper, so no heavy camera crate enters the trust surface and — packaged —
the macOS camera grant attaches to `Familiar.app` rather than to a terminal.

## Packaging (macOS)

`packaging/` turns the workspace binaries into a signed, **notarized** `Familiar.app` and a
`.pkg` installer that sets up the daemon launchd agent at boot; `familiar-eye` (the bundled
AVFoundation helper) is the eye. **Note:** the scripts still reference the archived `glass`
and `marble` binaries and are due for an update to the SwiftUI-console era. See
[`../packaging/README.md`](../packaging/README.md).

## The cycle (the metabolism)

The kernel has ported (`crates/cycle`); the autonomous tick runs:

```
Observe → Name → Interpret → Generate → Bound → Test → Score → Select → Inherit → Return
```

with the law-signals woven through it: **service** (Law I), **presence** and
**capacities** (Law II) are read continuously, and the **obedience guard** (Law III)
sits at the *Bound*/act steps as an active gate that can allow, seek consent, or refuse
— not a passive warning. *Interpret* is live: the familiar forms its own question +
theory and pursues open threads (its own and the human's answer) into candidate work.
The LLM boundary keeps the v1 protocol (`prompt → call_llm.sh → response`), the
canonical periphery seam. Per-step evidence is the claim→evidence table in
[05](05-validation-and-results.md#claim--evidence); the live end-to-end tick is recorded
there too.

## Storage

An **embedded SQLite** store (`rusqlite`, `bundled` — no system library) behind the original
append/load/update API, one logical table per record type under a data directory
(`familiar_data/` default, `--data-dir` override); `familiar db export`/`import` keeps the
cat-able JSONL truth available for audit. Local-first; the familiar sends no telemetry and
exfiltrates nothing. The record model and schema live in [`../data/`](../data/); the store's
design and migration story in [storage.md](storage.md).
