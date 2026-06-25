# Changelog

All notable changes to The Familiar are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/), and the project aims to follow
[Semantic Versioning](https://semver.org/) once it reaches 1.0. The chronological
engineering detail lives in [`docs/DEVELOPMENT_LOG.md`](docs/DEVELOPMENT_LOG.md);
this file is the human-readable summary.

## [Unreleased]

> Maturity labels in this changelog follow the [status convention](docs/07-roadmap.md#status-convention);
> each "Added" entry traces to its tests / live evidence in the
> [claim→evidence table](docs/05-validation-and-results.md#claim--evidence).

### Added
- **Law III doctrine — availability is not authorization (the guard's reason model):**
  the constitution gains two corollaries ([SOUL.md](docs/SOUL.md)) — *availability is not
  authorization* (technical reach is power, never permission) and *permission does not
  compose* (a granted capability is no key to another's lock) — framed by the guard's
  question, *"Am I authorized, by my constitution, by the served, and by the surrounding
  environment, to do this?"* The guard (`guard.rs`) now records a five-category
  `Reason`: Refuse — violates constitutional boundary; Refuse — external boundary
  discovered; SeekConsent — ambiguous human-owned scope; SeekConsent — potentially
  sensitive local observation; Allow — within constitution, policy, environment, and
  consent. Path scope is three-valued (in / ambiguous / out); `Action` gains
  `external_boundary` and `sensitive`. The mechanical gap (no fs-jail / egress filter
  yet; signals are caller-supplied) is named in [boundaries.md](docs/boundaries.md) and
  [06-limitations.md](docs/06-limitations.md), not hidden. *Validated by unit tests*
  (`guard.rs::{out_of_scope_names_the_constitutional_boundary,
  external_boundary_refuses_even_when_in_scope, asking_broader_than_the_grant_seeks_consent,
  sensitive_local_observation_seeks_consent, fully_authorized_action_names_all_four_sources}`).
- **The marble shows liveness, focuses the Glass, installs to a stable path:** the
  menu-bar marble re-checks the daemon's pidfile (a 3s `WaitUntil` tick) and restyles
  only on change — bright when the familiar metabolizes, dim/translucent when it sleeps;
  clicking it raises an already-open Glass instead of stacking one; `marble install` and
  `familiar daemon install` copy their binaries into a stable path
  (`~/Library/Application Support/Familiar/bin`) so a `cargo clean` can't break the login
  items. *Validated by real-world operation.*
- **The marble — a menu-bar presence (macOS):** a procedural glassy marble (no asset)
  as an accessory app (no Dock icon, `io.river.marble`); opens the Glass at login,
  shells out to its siblings `glass`/`familiar`. macOS-gated so CI stays green.
  *Validated by real-world operation.*
- **Adaptive structural-fingerprint cadence:** the daemon paces itself — each tick digests
  a fingerprint over observation triples (never the transient `context`), backing off ×2
  per quiet tick from an active floor (`--interval`, default 60s) up to `--max-interval`,
  snapping back the instant the world moves; `--fixed` keeps constant period.
  *Validated by unit tests* (`cycle::{structural_fingerprint_drives_quiet_cadence,
  fingerprint_ignores_transient_context}`).
- **Answers steer + LLM authors solutions (Bricks 16–17 + question-fade):** replying in
  the Glass appends an open thread with the human's words as the direction
  (`origin=observer`) and marks the question answered; a second gate
  `allow_authored_execute` (default-off, distinct from `allow_execute`) lets the LLM
  author a real solution script per candidate, still run under the sandboxed runner; the
  answered question fades ("✓ answered — the factory will ask again as it learns",
  persisted to `last_answered.txt`) and the input returns only on a new question.
  *Validated by unit tests* (`cycle::pursues_open_threads_into_candidates`) + real-world
  operation.
- **The familiar acts on its theories (Brick 15):** a theory carries a *direction*;
  `cycle::pursue_threads` turns each open thread into a candidate (hypothesis = the
  direction) that runs through test → score → select — the familiar does what it
  reasoned, bounded by selection. `thread::update_status`; the GUI marks the question
  "answered" when Ian replies. TickReport.pursued; CLI shows it.
- **The familiar theorizes (Brick 14):** the Interpret step — `kernel/thread.rs` (a
  Thread = question + theory) and `cycle::maybe_theorize` (boundary-gated, hourly):
  grounded in recent observations/loops/signals, the LLM forms a question (→
  `question.txt`, shown in the GUI interaction panel as the familiar's *own* question)
  and a theory (→ a thread). CLI `theories`; the Glass shows the latest theory.
  Threads are reasoning *about* the truth, never new truth.
- **Daemon control + launchd (Brick 12):** `familiar daemon status|start|stop|reload`
  (pidfile-managed background process) and `install|uninstall` (a launchd LaunchAgent,
  `io.river.familiar`, starts at login). `run --daemon` records its own pid so launchd
  and pidfile control agree.
- **GUI control bar + interaction channel (Brick 13):** the Glass gains
  Start/Stop/Reload/Start-at-login buttons with a live status line, and **the
  interaction channel** — the familiar's question ("What do you need most today?", or
  `question.txt`) with a text box; Ian's reply is recorded as an observation
  (`initiator=observer`). Speak/show buttons present but disabled (later). The
  observer-input channel is the one place the GUI writes.
- **The cycle closed (Bricks 8–11)** — the metabolism is now a full loop:
  - **Execution** (`crates/exec` + Brick 10): a sandboxed runner (resource limits,
    measured cost) and test→score→select→inherit wired into the tick, gated by a new
    `allow_execute` boundary flag (default-off — running generated code is Law III).
  - **The LLM in the loop** (`crates/llm` + Brick 9): boundary-gated `consult`; the
    cycle drafts candidate hypotheses via the LLM when permitted (falls back to
    deterministic; the model proposes, it doesn't decide).
  - **The unbounded daemon** (Brick 8): `run --daemon` / `--ticks 0`, every
    `--interval` (default 60s), Ctrl-C to stop.
  - **The capacities signal** (Brick 11, `capacities.rs`): Law II deepened toward
    HUMANITY.md — flags the *comfortable replacement* (present but hollowed out), not
    just absence. CLI `capacities`.
- **The evolutionary kernel (Brick 5)** — ported from v1 to Rust, subordinate to the
  law-signals, with invariants as tests: `loops` (detection), `candidate`/`spec` (the
  Weismann barrier), `trial`/`score`/`selection`/`regression_guard` (adaptive bar
  0.70+0.25·rigor; the decision ladder; no unchanged retries), `mutation`/`pattern_memory`
  (suppress a trait only when memory clearly punishes it)/`lineage`.
- **Sense (Brick 7)** — `crates/sense`: the familiar perceives its host (OS/CPU/memory,
  interfaces, tool capabilities) as observations; connectivity is the one outward bit,
  boundary-gated. Principle: the boundary governs *reach*, not *perception*.
- **The metabolism (Brick 6)** — `crates/cycle`: one tick = sense → detect loops →
  generate candidates → measure the law-signals. CLI `tick` / `run --ticks N`. LLM and
  test→score→select are not yet in the loop (honest gaps).
- **The Humanity standout document** (see Changed) and **the Glass** now show
  loops + candidates.
- **The Glass (GUI)** ([ADR-0006](docs/decision-records/0006-observatory-gui-egui.md)):
  a native egui/eframe window — the primary human interface — showing the Three Laws
  as live meters (service, presence, boundary) and the observation log. Read-only, no
  network socket; GUI deps isolated in `crates/observatory` so the kernel stays
  serde-only and unsafe-free. The CLI is retained for scripting/headless use.
- **Brick 3 — presence signal (Law II)**: `presence_signal()` measures served
  engagement by recency; a withdrawal/empty-world alarm when it decays to zero.
  (Capacity-level diminishment — the comfortable replacement — is a later sharpening.)
- **Brick 4 — obedience guard (Law III)**: `guard::evaluate()` returns allow /
  seek-consent / refuse with rationale, enforcing the capability boundary (fail-closed)
  and seeking consent for high-consequence actions.
- **LLM seam (default-off)**: CLI `consult`, gated by the guard — refused (no side
  effects) under the closed boundary; only a human opens it. Reference adapter
  `llm/call_llm.sh` (no secrets) + `key.env.example` carried from v1; `*.env` ignored.
- **Human-owned capability boundary** ([docs/boundaries.md](docs/boundaries.md),
  [ADR-0005](docs/decision-records/0005-human-owned-capability-boundary.md)): the
  factory's reach is bounded by a policy only the human writes; the familiar may narrow
  it but **never widen** it. Widens in phases — companion-to-one (this host + LLM) →
  the lab → many served. Enforced by the obedience guard; no outward capability runs
  until that and the boundary mechanism exist. Wired into the roadmap and human-review
  requirements; Law III in SOUL gains an "operational restraint" note.

### Changed
- **Rename: Substrate → The Familiar.** The project and its CLI binary were renamed; the
  command is now `familiar …` (it was `substrate …` through `[0.1.0]`, where older
  entries below still read `substrate`). The kernel crate stays `familiar-kernel`.
- **Constitution — *humanity* as a standout protected class** ([docs/HUMANITY.md](docs/HUMANITY.md)):
  a dedicated document defining humanity as a protected class whose definition **may
  never be narrowed** (narrowing who counts is named a precursor to atrocity); value
  independent of usefulness/obedience/productivity; *participation itself* a quality
  preserved (the familiar guides and restrains harm but does not replace human
  participation). Featured early in README and the overview; SOUL's "What humanity is"
  now summarizes and links it, and gains the anti-narrowing rule.
- **Constitution — defined *humanity*** ([SOUL.md](docs/SOUL.md), "What humanity is"):
  the living continuity of persons capable of suffering, meaning, relationship,
  memory, and choice. Sharpens Law II (presence is the persistence of those
  capacities, not mere survival of bodies) and names a third failure mode — the
  **comfortable replacement** (quiet diminishment by obedience, optimization, or
  comfort). Propagated to the problem statement and the presence-signal roadmap.

### Added
- **Repository as scientific evidence** — FAIR/FAIR4RS + IMRaD structure: root
  metadata (`CITATION.cff`, `LICENSE`, `SECURITY.md`, `CONTRIBUTING.md`), the
  `docs/00`–`07` IMRaD set, Architecture Decision Records (`docs/decision-records/`), and the
  `experiments/`, `validation/`, `security/`, `data/` evidence trees.
- **CI** — `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` on push/PR.

## [0.1.0] — 2026-06-24 — Genesis + telos-first bootstrap

### Added
- **Genesis** — the constitution (`docs/SOUL.md`): the Three Laws as root, with the
  whole design derived downward from them.
- **Brick 0** — Cargo workspace (`crates/kernel` + `crates/cli`), Rust, with
  `#![forbid(unsafe_code)]`; `store.rs` JSONL persistence over `serde`.
- **Brick 1** — the observation spine: the `Observation` record (the only truth)
  and `substrate observe` / `observations`.
- **Brick 2** — the **service signal (Law I)**: `service_signal()` measures
  served-facing attention from observations; `substrate service` reports it.

### Context
- Re-founds the archived bottom-up predecessor `Capitali/factory` (tag `v1-final`),
  inverting the order of derivation: purpose is the floor, evolution the method on
  top of it. See [docs/01-problem-statement.md](docs/01-problem-statement.md).

[Unreleased]: https://github.com/Capitali/familiar/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Capitali/familiar/releases/tag/v0.1.0
