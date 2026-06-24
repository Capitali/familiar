# Development Log

The linear handoff trail for Substrate v2. Newest entries on top. Before making
architectural changes, read `SOUL.md` (the Three Laws) and `ARCHITECTURE.md`, then
the latest entries here.

Each entry: what changed, why, checks run, what the next developer should know.

## 2026-06-24 — The human-owned capability boundary (companion phases)

### What changed

- `docs/boundaries.md` + `decision-records/0005`: the factory's reach is bounded by a
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
holds, growing the factory from companion-to-one into companion-to-many. Makes Law III
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
  relationship, memory, and choice; the factory preserves not only their survival but
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

Law I says continuation *is* service, so the factory must be able to see whether it is serving.
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

- Stood up the Rust workspace: `crates/kernel` (`substrate-kernel`, lib) and
  `crates/cli` (`substrate-cli`, bin `substrate`). Edition 2021; deps held to
  `serde` + `serde_json` only.
- `crates/kernel/src/lib.rs` carries `#![forbid(unsafe_code)]` — the Law III
  commitment made literal.
- `store.rs`: generic JSONL append/load over any `serde` record, with `--data-dir`
  resolution (default `substrate_data/`). Replaces v1's hand-rolled `json_util.c`.
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
