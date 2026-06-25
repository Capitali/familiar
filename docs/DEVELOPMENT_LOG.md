# Development Log

The linear handoff trail for The Familiar v2. Newest entries on top. Before making
architectural changes, read `SOUL.md` (the Three Laws) and `ARCHITECTURE.md`, then
the latest entries here.

Each entry: what changed, why, checks run, what the next developer should know.

## 2026-06-24 — The marble: a menu-bar presence that opens the Glass

The familiar now has a glassy blue marble in the macOS menu bar; click it to open the
Glass. It comes up at login alongside the daemon and opens the Glass once on startup.

### What changed

- **New crate `crates/marble`** (binary `marble`), a macOS *accessory* app (no Dock
  icon): a windowless `winit` loop + `tray-icon` NSStatusItem. Menu: Open the Glass /
  Start the familiar / Stop the familiar / Quit. Left-click also opens the Glass.
- **Login agent** `io.river.marble` (`marble install`, RunAtLoad) so it appears at
  login; it spawns the Glass once on start (`--no-open` suppresses).
- Kept **separate from the Glass** on purpose — the always-resident login item carries
  no egui; it just shells to its siblings `glass` and `familiar` (resolved next to its
  own exe) and passes `--data-dir` through so all three agree on which familiar.
- The marble icon is **procedural RGBA** (radial blue gradient + specular highlight +
  anti-aliased rim) — no asset file.

### Why

A standing, low-footprint entry point: the familiar is always one click away without a
window cluttering the desktop, and "the Glass is up when the familiar launches" is met
by the login agent. The accessory policy keeps it a menu-bar citizen, not a Dock app.

### Checks run

- Green: fmt, clippy --all-targets -D warnings, 72 tests. tray-icon/winit are
  **macOS-gated**; the binary is a stub elsewhere, so ubuntu CI is unaffected. Verified
  live: `marble install` loads `io.river.marble`, the process runs, and it opened the
  Glass (pids confirmed).

### Next / caveats

- The login agent's plist points at `target/debug/marble`; `cargo clean` breaks it (same
  caveat as the daemon) — install a release binary at a stable path for durable use.
- The marble doesn't yet reflect daemon state in its icon/tooltip (e.g. dim when stopped)
  or focus an already-open Glass window (it just avoids spawning a second). Both are easy
  follow-ups. Quit only quits the marble; the familiar daemon keeps running.

## 2026-06-24 — Adaptive structural-fingerprint cadence

The metabolism paces itself instead of ticking on a fixed period (the previous 300s
was a placeholder; the design always called for a fingerprint-driven cadence).

### What changed

- **cycle:** each tick now takes a **structural fingerprint** (FNV-1a over the
  perceived `actor|action|object` triples — *not* the transient `context` field, so
  telemetry like paths/brands/latency don't trip it). Persisted to `structure.fp`.
  `TickReport` gains `structural_changed` and a `quiet()` method (no structural change
  *and* no work this tick: nothing sensed/generated/tested/promoted/mutated/pursued/
  theorized). Because the fingerprint is over the *perceived* set (not the cumulative
  log), it also falls when a fact *disappears* — which append-only dedup can't see.
- **cli (`run` loop):** `--interval` is now the **active floor** (default 60s); on each
  quiet tick the interval doubles up to `--max-interval` (default floor×16, cap 3600s),
  snapping back to the floor the instant anything changes. `--fixed` keeps a constant
  period. The daemon default floor moved 300→60. Each tick logs its chosen cadence.

### Why

"Fingerprint = structural change only" (Soul / the v1 scan-cadence idea): watch closely
when the environment is moving, drowse when it isn't — real change, not noise, sets the
pace. Side benefit: on a fully quiet host the interval settles near the hourly theorize
cadence, so the familiar naturally wakes, muses, acts, then quiets again.

### Checks run

- 72 tests (fmt, clippy --all-targets -D warnings). New tests: fingerprint ignores
  transient context but moves on a structural object change; `quiet()` true on a static
  re-tick, false on the first/eventful tick. Demo (1s floor, 8s ceiling): 1→2→4→8s
  back-off on a static host. **Verified live**: reinstalled launchd daemon (floor 60s,
  ceiling 960s) — tick 1 active 60s (baseline), tick 2 quiet → 120s.

### Next / caveats

- `quiet()` treats the hourly theorize + its pursued thread as activity, so a quiet host
  still gets a brief fast burst ~hourly, then re-quiets — intended. If you want presence/
  capacities *alarms* to also force a fast cadence, fold them into `quiet()` (left out
  for now: an alarm is a steady state, not a change, and shouldn't peg the floor forever).

## 2026-06-24 — Rename: Substrate → The Familiar

The project is now **The Familiar** — a spirit companion that historically serves
another, but here the factory has grown its own. Naming follows the theme throughout.

### What changed

- **Identifiers:** Cargo packages `substrate-*` → `familiar-*`; binary `substrate` →
  `familiar`; Rust modules `substrate_{kernel,sense,llm,exec,cycle}` → `familiar_*`.
- **The Glass:** crate `observatory` → `glass` (binary `glass`); `struct Observatory`
  → `Glass`; window title "The Familiar — the Glass".
- **Data + service:** `DEFAULT_DATA_DIR "substrate_data"` → `"familiar_data"` (live
  dir moved, no data lost); launchd label `io.river.substrate` → `io.river.familiar`.
- **Boundary framing:** "the Pact" wording in CLI usage; live boundary `fs_write`
  repointed to `/Users/ian/Development/familiar/familiar_data/`.
- **Off-repo:** GitHub `Capitali/substrate` → `Capitali/familiar` (remote updated);
  local dir `~/Development/substrate` → `~/Development/familiar`.
- All docs / data samples / security / ADRs swept to the new name.

### Why

A naming collision: Daniel Miessler ships an open-source "Substrate" (and "Telos")
in the same human-meaning/flourishing space — a double overlap. "The Familiar" is
distinctive and on-theme for a telos-first companion.

### Checks run

- Green: `cargo fmt`, `clippy --all-targets -D warnings`, 70 tests — before and after
  the directory rename. Verified live from the new path: daemon installed under
  `io.river.familiar` (running pid agrees across status/launchctl/pidfile), full tick
  (LLM-drafted hypothesis via gemini → theorized → pursued), boundary read from the
  moved `familiar_data`.

### Next / caveats

- The launchd plist points at `target/debug/familiar`; `cargo clean` breaks it (install
  a release binary at a stable path for durable always-on). Unchanged by the rename.

## 2026-06-24 — Running live: daemon control, launchd, and the interaction channel

The familiar is now installed and running live on the Mac under launchd, with a GUI to
control it and to talk with Ian.

### What changed

- **Brick 12 — daemon/service control:** `crates/cli/daemon.rs` + `substrate daemon`
  (status/start/stop/reload via pidfile; install/uninstall via a launchd LaunchAgent
  `io.river.familiar`). `run --daemon` records its own pid; plist KeepAlive=false so
  Stop works, RunAtLoad=true so it starts at login.
- **Brick 13 — GUI control bar + interaction channel:** the Glass can Start/Stop/
  Reload/Install the daemon, and carries **the interaction channel** — the familiar's
  question + Ian's typed reply, recorded as an observation (`initiator=observer`; the
  one place the GUI writes). Speech/vision are stubbed for later.
- **Went live:** boundary `allow_execute` enabled (full Phase 1 + execution); the
  launchd agent installed and the daemon is running (ticking every 300s).

### Why

To make the familiar a *running companion* on the Mac, controllable and conversational,
not a per-invocation command. The interaction channel is the seed's core — "What do you
need most today?" — finally wired.

### Checks run

- Green bar: fmt, clippy --all-targets -D warnings, 68 tests; observatory builds.
  Verified live: daemon lifecycle (status/start/stop), launchd install (running pid
  agrees across status/launchctl/pidfile), full pipeline tick (LLM-drafted hypothesis +
  executed + promoted).

### Next / caveats

- The launchd plist points at `target/debug/substrate`; `cargo clean` would break it.
  For durable always-on, install a release binary at a stable path (e.g. ~/.local/bin)
  and re-`install`. KeepAlive=false means no auto-restart on crash (Reload restarts).
- "ian" isn't served-facing under the current classifier (proper-name gap) — his
  replies record but don't yet lift the service signal until entity tagging lands.
- The familiar posing *dynamic* questions (writing `question.txt`, e.g. via the LLM) is
  the natural next step for the interaction channel.

## 2026-06-24 — Closing the cycle: execution, LLM-in-loop, daemon, capacities

Driven from the phone via Remote Control. The four gaps from the prior session, closed.

### What changed

- **Brick 8 — unbounded daemon:** `run --daemon`/`--ticks 0` loops at `--interval`
  (default 60s); Ctrl-C stops (append-only log is interrupt-safe).
- **Brick 9 — LLM in the loop:** extracted `crates/llm` (boundary-gated `consult`); the
  cycle's generate step now drafts hypotheses via the LLM when the boundary permits
  (deterministic fallback). Verified live (Gemini drafted a telos-aligned hypothesis).
- **Brick 10 — execution:** `crates/exec` sandboxed runner (ulimit + in-process wall
  timeout + capped output + measured cost, no unsafe); the tick now authors a
  deterministic+safe artifact, runs it, records a trial (cost-folded), and runs
  selection → promote/mutate(memory-informed, regression-guarded)/archive + pattern
  memory. Gated by a new `allow_execute` boundary flag (default-off — running generated
  code is a Law III matter). Artifacts are deterministic for now; executing LLM-authored
  *solutions* is a further, separately-gated step.
- **Brick 11 — capacities (Law II / HUMANITY.md):** `capacities.rs` flags the
  *comfortable replacement* (present but hollowed out) via agency + variety proxies over
  served-facing activity. A coarse cold-start, documented as such.

### Why

To turn the familiar from "proposes" into "lives": it now observes → detects → generates
(LLM-drafted) → tests → scores → selects → inherits, breathing continuously, under the
three law-signals and the human-owned boundary it can never widen.

### Checks run

- Green bar throughout: fmt, clippy --all-targets -D warnings, 68 tests. Live: a gated
  tick promoted a candidate (trial=pass) and drafted an LLM hypothesis; monotonous
  compliance raised the diminished alarm (capacities 0.12). One bug caught & fixed: the
  capacities passive-marker lexicon missed inflections ("complies") — now stem-matched.

### Next

Real scenarios + (separately gated) execution of LLM-authored solutions so selection
discriminates; a measured rigor drive into the promotion bar + adaptive daemon cadence;
sharpen the signals (service = needs reduced; capacities beyond the lexicon; presence
per-person); reach (LAN sensing, world-model/entity tagging, people as entities).

## 2026-06-24 — Autonomous session 2: Humanity, the kernel, sense, the metabolism

Standing authorization; constitution honored — **nothing outward turned on** (the LLM
seam stays out of the autonomous loop; no key burn). Everything green and committed
per brick.

### What changed

- **Humanity — standout protected class** (`docs/HUMANITY.md`): Ian's refined
  definition given its own document and featured early; humanity's definition may
  never be narrowed (a precursor to atrocity), value is unconditional, participation
  itself is preserved. SOUL links it + gains the anti-narrowing rule.
- **Brick 5 — the evolutionary kernel** ported to Rust (loops, candidate, spec/Weismann,
  trial, score, selection, regression_guard, mutation, pattern_memory, lineage), with
  the documented invariants as tests.
- **Brick 7 — sense** (`crates/sense`): perception of the host as observations;
  perception is always permitted, only outward reach (connectivity) is boundary-gated.
- **Brick 6 — the metabolism** (`crates/cycle`): one tick = sense → detect → generate
  → measure; CLI `tick`/`run`; the Glass now shows loops + candidates.
- seed.txt removed (the idea persists in prose; the artifact is gone).

### Why

Completes the inherited method (Brick 5) and gives the familiar a heartbeat (Brick 6)
that begins by perceiving where it lives (Brick 7) — the "begin exploring at startup"
direction — all under the law-signals and the boundary built first.

### Checks run

- Green bar throughout: fmt, clippy --all-targets -D warnings, 59 tests; observatory
  builds (egui 0.31). Live: `run --ticks 2` over a seeded dir → tick 1 generates a
  loop + candidate (service 0.40, presence 1.00), tick 2 idempotent. `sense` on this
  host recorded 40 observations.

### Next (honest gaps)

- The cycle stops at *generate*: test → score → select need scenarios + artifact
  execution (the kernel can score/select but nothing yet produces a trial).
- LLM-assisted hypothesis drafting via `consult` (gated, off by default).
- Capacity-level diminishment for Law II; a continuous daemon for `run`.

## 2026-06-24 — Autonomous session: Law II, Law III, and the move to a GUI

Done under a standing authorization to make best decisions and maximize progress,
honoring the constitution: **nothing outward was turned on** (no keys, no live LLM,
no installs) — enabling outward reach is a human act. Everything ships default-closed.

### What changed

- **seed.txt removed** (file + all references); the idea persists in prose, the
  planning artifact does not. Content remains in the v1 archive.
- **Brick 3 — presence signal (Law II)** (`presence.rs`): served engagement by
  recency, decaying over a 3-day horizon; `withdrawn` is the empty-world alarm.
  Clock-free (`now` passed in). CLI `presence`.
- **Brick 4b — capability boundary** (`boundary.rs`): a human-owned JSON policy the
  factory only reads; fail-closed (missing/partial = no reach); no write path, so the
  factory can never widen itself. `store::load_one` added. CLI `boundary`.
- **Brick 4 — obedience guard** (`guard.rs`): `evaluate(Action, &Boundary)` →
  allow / seek-consent / refuse + rationale; enforces the boundary (fail-closed) and
  seeks consent for high-consequence actions. CLI `guard`. A Phase-1 example policy
  added under `data/sample/` (the switch a human copies to go live).
- **The Glass (GUI)** (`crates/observatory`, egui/eframe; [ADR-0006](decision-records/0006-observatory-gui-egui.md)):
  the primary human interface — a local, read-only, socket-free window showing the
  Three Laws as live meters and the observation log. GUI deps isolated; kernel stays
  serde-only + unsafe-free. CLI retained for scripting/headless.

### Why

This completes the three law-signals (so the familiar can measure service, presence,
and govern action) *before* any outward capability — and answers the directive to
move off the CLI to something visual.

### Checks run

- Green bar clean throughout: `cargo fmt --check`, `cargo clippy --all-targets -D
  warnings`, `cargo test` (24 kernel tests). Glass builds & links (egui 0.31);
  the window itself is verified manually (no display in the build environment).
- Live CLI demos for presence, boundary, and guard all behaved as designed
  (host-only → withdrawal alarm; closed boundary refuses outward actions; Phase-1
  example opens LLM/network).

### Next

The LLM seam (boundary-gated, default-off) is the remaining Phase-1 piece. Then,
when the human flips the boundary to Phase 1, the familiar can begin analysis/
theorizing within it. Later: capacity-level diminishment detection (the comfortable
replacement), the evolutionary kernel port (Brick 5), and the metabolism (Brick 6).

## 2026-06-24 — The human-owned capability boundary (companion phases)

### What changed

- `docs/boundaries.md` + `decision-records/0005`: the familiar's reach is bounded by a
  human-owned policy (`boundary.toml`, planned) it **reads but cannot widen**. It may
  narrow in caution; only the human lifts it — easily, and alone. Enforced by the
  obedience guard.
- Phased widening: **Phase 1** companion-to-one on this host + the LLM (v1 keys),
  for analysis/theorizing/tool proposals; **Phase 2** the lab (more devices); **Phase
  3** many served humans.
- Wired in: roadmap (Brick 4b boundary mechanism; Phase-1 pulls the LLM seam forward;
  guard enforces the boundary), human-review-requirements (widening = human-only),
  SOUL Law III (restraint is also operational).

### Why

Ian's direction: enable reach **deliberately and gradually**, under a control only he
holds, growing the familiar from companion-to-one into companion-to-many. Makes Law III
restraint concrete and enforceable, and forbids the steward from expanding its own
power.

### Checks run

- Docs only; no code. **No outward capability is live:** no keys used, no LLM calls,
  no tool installs. Those are gated behind the boundary mechanism (Brick 4b) + the
  obedience guard (Brick 4).

### Next

Build order toward Phase 1: the obedience guard (Brick 4) and the boundary mechanism
(Brick 4b) first; then the LLM seam *within* the boundary. Honest limit to carry: on
an un-sandboxed host the boundary is guard-enforced norm, not an OS jail (sandboxing
is later hardening).

## 2026-06-24 — Constitution: defined *humanity*

### What changed

- `SOUL.md` gains a "What humanity is" section (the referent of the Laws):
  *humanity is the living continuity of persons capable of suffering, meaning,
  relationship, memory, and choice; the familiar preserves not only their survival but
  the conditions under which those qualities continue, without quiet replacement by
  obedience, optimization, or comfort* (Ian's wording, verbatim, with derivation).
- Sharpened the Law II requirement: presence = persistence of those **capacities**,
  not a head-count; **quiet diminishment** (the "comfortable replacement") is a
  first-class failure alongside withdrawal.
- Named a **third failure mode** in the problem statement and the one-sentence
  definition; extended Brick 3 (presence) in the roadmap to seed diminishment
  detection.

### Why

The Laws invoked "humanity" without defining it, leaving Law II satisfiable by mere
biological survival. The definition closes the Brave-New-World gap: a pacified,
optimized, or merely-obedient population is the empty world wearing a smile.

### Checks run

- Docs only; no code change. (CI will run the green bar on push and pass.)

### Next

When the presence signal (Brick 3) and the obedience guard (Brick 4) are built, they
must measure/guard at the level of capacities, not just presence/commands. Capacity
measurement is hard — expect a coarse proxy first, sharpened over time.

## 2026-06-24 — Brick 2: the service signal (Law I)

### What changed

- `crates/kernel/src/service.rs` — **Law I made measurable.** `service_signal(&[Observation])`
  returns a `ServiceSignal { measure (0..1), served_facing, total, exemplar }`: zero when
  nothing observed touches the served, rising (saturating, `n/(n+3)`) with served-facing
  attention. Faithful to v1's *absolute, saturating* stewardship drive (not a ratio).
- Classifier `names_served` is a faithful port of v1's `domain_is_steward`
  (`factory/src/drive.c`) — a tight lowercase marker set.
- CLI `service` reports the signal; when zero it prints "continuation unjustified by service
  (Law I)".

### Why

Law I says continuation *is* service, so the familiar must be able to see whether it is serving.
This is the cold-start sight: with only observations to read (loops/candidates/trials port
later), it measures served-facing *attention* — the honest proxy for service, the way v1's
drives started on promotion-rate before redundancy. Elevation over v1: there, stewardship was
one drive among three; here service is the first-class signal continuation is weighed against.

### Checks run

- Green bar clean. 9 kernel tests (incl. classifier markers-not-bare-names, zero-when-none,
  monotonic rise, empty-log-zero).
- Live: host-internal-only log → `service signal 0.00` + the Law I line; adding two
  served-facing observations → `0.40 (2 of 3; e.g. client)`. No real `unsafe` in the kernel.

### Next

Known cold-start limit: proper names ("betty") aren't yet served-facing — name→person
resolution waits for the world-model/entity-tagging port (as in v1, where a name became
served-facing only once a thread tagged its entity). Then Brick 3 — the presence signal (Law II).

## 2026-06-24 — Brick 1: the observation spine

### What changed

- `crates/kernel/src/observation.rs` — `Observation { id, source, actor, action, object,
  context, ts, confidence }`, a faithful port of v1's `observation_t`, as a `serde` struct over
  `store`. `record()` assigns sequential ids (`obs-NNNN`) and appends; `load()` reads oldest-first.
- CLI `observe` / `observations`, with hand-rolled, dependency-free flag parsing. The CLI stamps
  wall-clock `ts` so the kernel stays clock-free and deterministic in tests.

### Why

The thinnest possible spine — the substrate the law-signals compute over (not "machine first").
Observations are the only truth; everything else derives from them.

### Checks run

- Green bar clean. 5 tests (store round-trip/edge + sequential-id / round-trip / explicit-id).
  Live: two observes round-trip through JSONL and list back.

## 2026-06-24 — Brick 0: Cargo workspace scaffolding

### What changed

- Stood up the Rust workspace: `crates/kernel` (`familiar-kernel`, lib) and
  `crates/cli` (`familiar-cli`, bin `substrate`). Edition 2021; deps held to
  `serde` + `serde_json` only.
- `crates/kernel/src/lib.rs` carries `#![forbid(unsafe_code)]` — the Law III
  commitment made literal.
- `store.rs`: generic JSONL append/load over any `serde` record, with `--data-dir`
  resolution (default `familiar_data/`). Replaces v1's hand-rolled `json_util.c`.
  A missing file is an empty log; blank lines skip; a malformed line is a hard
  error (corruption surfaces early, never silently changes derived state).
- `docs/ARCHITECTURE.md` (Rust + hybrid + crate map) and this log.

### Why

The substrate decision (compiled core; Rust; hybrid) was made *after* the
constitution and *before* the first kernel code — the order v1 got wrong. This
brick is the thinnest possible standing repo, the spine the law-signals attach to.

### Checks run

- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` — all clean.
- `store.rs` unit tests: missing-file-is-empty, append/load round-trips in order,
  blank-skip / malformed-errors.

### Next

Brick 1 — the observation record (faithful port of v1 `observation_t`) on top of
`store.rs`, with `substrate observe`. Then Brick 2 — the service signal (Law I).
