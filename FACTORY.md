# The familiar runs as a dark factory

> Ian, 2026-08-28: *"essentially the core of the familiar should be the
> management layer of the Dark factory — the familiar development being done
> in a factory model with the intent of being fully autonomous within the
> bounds of the constitution."*

This repo already runs the way a
[dark factory](https://github.com/SpaceTrucker2196/DF_Template) runs: an agent
can walk in cold, read what is in-tree, and continue building without a human
in the inner loop. This file is the cold-walk-in index. It **links** the
canonical law and runbooks rather than repeating them — a second copy is a
second source of truth, and this factory has one.

There are two factories here, and they share one pattern:

- **(a) The development factory** — how the familiar *gets built*. Two AI
  lanes (claude + codex) take BOARD tasks as production orders, converge each
  on the green workspace bar, review each other adversarially before landing,
  and record everything in the coordination files. That is the factory you are
  standing in right now.
- **(b) The familiar's own factory** — how the *running familiar* manufactures
  a new capability it was asked for (T-229): a work order → generated code →
  a layered oracle → a proposed declaration for the human's hand → autonomous
  management under its own law. Its management core is `crates/workshop`; its
  containment is `crates/jail`.

Both are bounded by the same constitution — the Three Laws in
[`docs/SOUL.md`](docs/SOUL.md). Nothing either factory does may cross them.

## The pattern

1. **Everything an agent needs is in-tree.** Mission and law:
   [`docs/SOUL.md`](docs/SOUL.md). How the AIs behave:
   [`coordination/README.md`](coordination/README.md) and the standing brief a
   companion is started with, [`coordination/COMPANION_PROMPT.md`](coordination/COMPANION_PROMPT.md).
   The autonomy contract — what an agent decides alone, flags, or must stop
   and ask — [`coordination/AUTONOMY.md`](coordination/AUTONOMY.md).
2. **Production orders.** For the development factory: BOARD tasks
   ([`coordination/BOARD.md`](coordination/BOARD.md)). For the familiar's own
   factory: `WorkOrder` records in `crates/workshop`.
3. **The oracle is the test suite.** The merge gate is the local green bar,
   enforced before any push; CI is a post-hoc judge, never the gate. See
   "The bar" below. The familiar's own factory extends this to a four-rung
   hardware oracle (bench → read → act → witness); see the T-229 dialogue,
   `docs/reviews/2026-08-28-dark-factory-dialogue.md`.
4. **Instrumentation is append-only, and a refusal is a decision.** The
   coordination log ([`coordination/STATE.md`](coordination/STATE.md)) is the
   human-readable operational record; the machine-readable factory ledger of
   the familiar's own orders is `crates/workshop`'s append-only event log,
   whose replay *is* the order state. "We measured it and it does not work"
   earns a recorded row precisely because no code remains to remember it.

## The bar (the development oracle)

From a clean checkout, green before any push:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Swift side, when `ios/` changed (see [`ios/README.md`](ios/README.md) for the
exact invocation and the `-sdk` gotcha):

```
cd ios/FamiliarMesh && swift test
xcodegen && xcodebuild ... (Release build of FamiliarMac; sim build of FamiliarAgent)
```

CI: `.github/workflows/ci.yml` (Rust, ubuntu) and `.github/workflows/swift.yml`
(macos runner). CI judges after the fact; it is not the merge gate.

## Layout

- `crates/` — the Rust core. `kernel` (deterministic types incl. the boundary
  gates and declared-actuator surfaces), `cycle` (the metabolism / service
  loop), `exec` (bounded script execution), `mesh` (world model, transport,
  observation), `mcp` (the partner door), `llm` (the reasoner seam), and the
  T-229 factory: `workshop` (order/contract/ledger management) + `jail`
  (candidate containment).
- `ios/` — the Metal Sphere and the shells (FamiliarMac, FamiliarAgent, watch).
- `coordination/` — the two-lane shared memory (the development factory's
  records).
- `docs/` — SOUL (law), ARCHITECTURE, ADRs (`docs/decision-records/`), and the
  design dialogues (`docs/reviews/`).

## Where to read next

| Question | Read |
|---|---|
| Why does the familiar continue at all? | [`docs/SOUL.md`](docs/SOUL.md) |
| How do the two AI lanes work? | [`coordination/README.md`](coordination/README.md) |
| What may an agent do without asking? | [`coordination/AUTONOMY.md`](coordination/AUTONOMY.md) |
| What is in flight? | [`coordination/BOARD.md`](coordination/BOARD.md), [`coordination/STATE.md`](coordination/STATE.md) |
| How does the familiar manufacture a capability? | `docs/reviews/2026-08-28-dark-factory-dialogue.md`, then `crates/workshop` |
