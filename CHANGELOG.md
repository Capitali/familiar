# Changelog

All notable changes to Substrate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/), and the project aims to follow
[Semantic Versioning](https://semver.org/) once it reaches 1.0. The chronological
engineering detail lives in [`docs/DEVELOPMENT_LOG.md`](docs/DEVELOPMENT_LOG.md);
this file is the human-readable summary.

## [Unreleased]

### Added
- **The Observatory (GUI)** ([ADR-0006](docs/decision-records/0006-observatory-gui-egui.md)):
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
  factory's reach is bounded by a policy only the human writes; the factory may narrow
  it but **never widen** it. Widens in phases — companion-to-one (this host + LLM) →
  the lab → many served. Enforced by the obedience guard; no outward capability runs
  until that and the boundary mechanism exist. Wired into the roadmap and human-review
  requirements; Law III in SOUL gains an "operational restraint" note.

### Changed
- **Constitution — *humanity* as a standout protected class** ([docs/HUMANITY.md](docs/HUMANITY.md)):
  a dedicated document defining humanity as a protected class whose definition **may
  never be narrowed** (narrowing who counts is named a precursor to atrocity); value
  independent of usefulness/obedience/productivity; *participation itself* a quality
  preserved (the factory guides and restrains harm but does not replace human
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

[Unreleased]: https://github.com/Capitali/substrate/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Capitali/substrate/releases/tag/v0.1.0
