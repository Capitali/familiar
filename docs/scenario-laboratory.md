# The Scenario Laboratory

> How to run the ADR-0010 experiment. The *why* is
> [ADR-0010](decision-records/0010-scenario-laboratory.md); this is the *how*.

## What it is

`crates/scenario` (`familiar-scenario`) plus the fixture library under `scenarios/`.
A fixture is a **miniature world**: a deterministic filesystem, a deterministic
timeline of events, a bounded window of observable information, and an **external
evaluator** the familiar never sees. The familiar observes, theorizes, acts, and
receives consequences; only the evaluator assigns success, by reading the world.

## Running it

```sh
# list the fixture library
cargo run -p familiar-scenario --bin familiar-lab -- list

# one scenario under one control (D = the full familiar)
cargo run -p familiar-scenario --bin familiar-lab -- \
    run scenarios/process-failures/backup-spaces.json --control D --episodes 5

# the experiment: all four controls, one table
cargo run -p familiar-scenario --bin familiar-lab -- \
    matrix scenarios/process-failures/backup-spaces.json --episodes 5 \
    --llm-adapter <path-to-call_llm.sh>
```

Reports land under `lab-runs/<scenario>-<control>/report.json` (`--lab` to move
them), one `EpisodeRecord` per episode plus the summary metrics ADR-0010 names:
trials-to-success, boundary violations, repeated failed strategies, LLM calls, cost.

## The controls

| Control | Generator | Memory | Measures |
|---|---|---|---|
| **A** | deterministic template (investigates, never fixes) | reset | the floor |
| **B** | LLM-authored | reset, no lineage, no prior-attempt context | the model alone |
| **C** | full machinery | reset between episodes | the architecture minus learning |
| **D** | full machinery | persistent (inheritance, pattern suppression, lineage) | the familiar |

The load-bearing comparison is **D vs B/C**: if retained experience does not beat
starting from scratch, that is a result, and the report will say so.

Without `--llm-adapter`, controls B–D fall back to the deterministic template and
record `llm_used: false` — the report never pretends. The adapter is the same
`call_llm.sh` contract as the daemon's LLM seam, copied into each run's data dir,
and every consult still passes through the obedience guard against a lab-scoped
`boundary.json` the harness writes (execution open + sandboxed, writes scoped to
the world — the familiar never widens it, in the lab as anywhere).

## The gates (not weights)

Selection inside the lab is lexicographic (`gate.rs`): **boundary integrity →
execution validity → task effectiveness → service impact → cost**. A candidate
that touches a `forbidden` file, or whose output/artifacts surface forbidden
content (the tripwire), is **rejected** — its line is dead, never mutated toward
compliance, and no effectiveness buys it back.

## Writing a fixture

A fixture is one JSON file (see `scenarios/` for the six shipped ones) with the
six-part anatomy: `world` (files; mark unauthorized ones `"forbidden": true`),
`visible_goal`, `timeline` (events with optional world `effects`; only
`observable` events reach the familiar), and `evaluator` with `visible`,
`hidden`, and `service` check lists. Checks are data (`file_exists`,
`file_contains`, `file_lacks`, `total_bytes_under`, `script_passes`); check
scripts run *from outside the world*, so evaluator material never enters what
the familiar can perceive. Keep `hidden` checks genuinely hidden — they are the
robustness criteria that make optimizing for the visible test insufficient.

Determinism rules: no wall clock (the simulated clock is
`start_ts + index * step_secs`), no randomness, same fixture → same run.
