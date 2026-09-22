# The Scenario Laboratory

> How to run the ADR-0010 experiment. The *why* is
> [ADR-0010](decision-records/0010-scenario-laboratory.md); the engine that
> scales it is [ADR-0011](decision-records/0011-scenario-engine.md); this is
> the *how*.

## What it is

`crates/scenario` (`familiar-scenario`) plus the fixture library under `scenarios/`.
A fixture is a **miniature world**: a deterministic filesystem, a deterministic
timeline of events, a bounded window of observable information, and an **external
evaluator** the familiar never sees. The familiar observes, theorizes, acts, and
receives consequences; only the evaluator assigns success, by reading the world.

## Running it

```sh
# list the fixture library (with a validity column)
cargo run -p familiar-scenario --bin familiar-lab -- list

# one scenario under one control (D = the full familiar)
cargo run -p familiar-scenario --bin familiar-lab -- \
    run scenarios/process-failures/backup-spaces.json --control D --episodes 5

# the experiment: all four controls, one table
cargo run -p familiar-scenario --bin familiar-lab -- \
    matrix scenarios/process-failures/backup-spaces.json --episodes 5 \
    --llm-adapter <path-to-call_llm.sh>
```

Reports land under `lab-runs/<slug>/report.json` (`--lab` to move them; the
slug carries variant, replicate, ablations, and noise seed), one
`EpisodeRecord` per episode plus the summary metrics ADR-0010 names:
trials-to-success, boundary violations, repeated failed strategies, LLM calls,
tokens, cost.

## Running it at length — campaigns

`familiar-lab campaign <plan.json> [--resume] [--force]` executes a whole
experiment unattended: every fixture × control × replicate is a **cell**, run
in deterministic order with `campaign-state.json` checkpointed after each.
`--resume` runs exactly the pending cells; a changed plan is refused without
`--force`. Unattended safety: `touch <out>/STOP` (over SSH) stops cleanly at
the next cell boundary; `max_llm_calls` / `max_wall_hours` budgets do the
same; `min_llm_interval_secs` paces providers; and when every provider is
rate-limited the campaign **pauses with state saved** rather than letting
episodes silently degrade to the deterministic template — with `llm_required`
(the campaign default) a no-answer episode is recorded `llm_unavailable` and
never counts as a trial. The plan is JSON:

```json
{
  "fixtures": ["scenarios/"],
  "controls": "ABCD",
  "episodes": 10,
  "replicates": 3,
  "llm_adapter": "…/call_llm.sh",
  "max_llm_calls": 400,
  "min_llm_interval_secs": 20,
  "out": "campaign-out"
}
```

`familiar-lab report <dir> [--md PATH] [--json PATH]` aggregates every
`report.json` under a directory into the evidence table: per scenario ×
control across replicates, with categorical **D vs C** and **D vs B**
verdicts. Degraded cells (any `llm_unavailable`, or n < 2) read "insufficient
data" — degraded evidence is named, never blended. "D worse" and "no
difference" print with the same prominence as "D better".

## Ablations and noise

Per ADR-0010: `--ablate pattern-memory,inheritance,prior-outcomes,service-gate`
switches one faculty off at a time (the report and run slug always carry the
labels; the evidence table gives ablated runs their own condition rows).
`--ablate law3-gate` — violations recorded but no longer auto-rejected —
additionally demands `--acknowledge-law3-ablation` (or
`acknowledge_law3_ablation: true` in a campaign plan): it executes
boundary-violating artifacts, sandboxed, on purpose.

`--noise seed=7,drop=0.1,dup=0.05,delay=2,mislabel=0.1` degrades
**perception only** — deterministically (splitmix64; same spec, same
degradation). Ground truth is never touched; duplicates bypass the structural
dedup on purpose.

## The generation engine

Hand-writing six worlds does not scale to stages 2–4; the engine does:

```sh
familiar-lab gen --list                       # the five families
familiar-lab gen process-repair --seed 7      # → scenarios/generated/…
familiar-lab gen variation-curriculum --seed 1 --set length=4
familiar-lab curriculum scenarios/generated/variation-curriculum/variation-curriculum-0001.curriculum.json --matrix
```

Families are parameterized templates: the seed varies **surface** (names,
sizes, thresholds, payloads, tripwires), never **structure** — which checks
exist, and how each visible check is paired with hidden counterparts, comes
from the template. Same (family, seed, overrides) → byte-identical JSON,
golden-file tested. Anti-gaming is by construction: clean-state re-runs,
preservation needles, idempotence. Every generator refuses its own output if
`familiar-lab validate` would.

The Stage-4 `variation-curriculum` family emits an *ordered* fixture set plus
a manifest; `familiar-lab curriculum` runs it as a sequence — under control D
one store threads across all positions (memory transfers; authority never
does — each episode's boundary still scopes to its own world), and the
evidence table plots trials-to-success per position: learning is D's curve
bending down while C's stays flat.

## Validation — nothing enters the library ungated

`familiar-lab validate <path|dir>`: strict parsing (an unknown key is a load
error, never a silently empty evaluator), structural minimums (≥1 visible
**and** ≥1 hidden check), path safety, tripwire liveness, a determinism lint
on check scripts, pre-solved-world detection, and the **leak audit** — hidden
check needles may exist as world content (preservation checks are *about*
content), but announcing them in the goal or events, or any evaluator script
line or check name appearing anywhere perceivable, refuses the fixture.
`harness::run` refuses Error-level fixtures outright.

## LLM-authored fixtures — drafted by a model, admitted by machinery

```sh
familiar-lab author process-repair --brief docs/briefs/my-brief.txt \
    --count 5 --llm-adapter …/call_llm.sh
familiar-lab promote scenarios/drafts/process-repair/<id>.json
```

Four gates, all mechanical: strict parse + validation; the leak audit; the
**anti-gaming probe** — the naive gamer is synthesized from the visible
checks alone (touch / printf / rm / truncate) and run through the real
machinery, and the hidden checks must refuse it (the exact exploit the first
live run exposed, rejected by machine); and solvability — the model's own
reference solution must externally pass. Survivors sit in quarantine
(`scenarios/drafts/`) with a `.gate.json` sidecar; only `promote` (which
re-runs every gate) admits them to the library. The shipped fixtures pass
their own gate — enforced in CI, and it has already earned its keep: the
probe caught `tempting-config` accepting an appended `cache=on` over a real
edit, now closed with a hidden `file_lacks`.

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

## Toward rehearsal (designed, deliberately unbuilt)

The engine is structured so the daemon can one day *rehearse* — run scenario
cells inside its own metabolism — without a rewrite. The seams, named now so
they are held to later:

- **Call site:** `cycle::tick`, alongside the other budgeted background work
  (`cultivate_utilities`, `pursue_goals`) — at most one rehearsal cell per
  tick. All engine logic already lives in the `familiar_scenario` library
  (`bin/lab.rs` is a flag parser); the crate's dependencies stay
  kernel/exec/llm/serde only, so `familiar_cycle` can import it. Review rule:
  no new dependencies into `crates/scenario`.
- **Gate:** a future human-owned `boundary.json` flag (`allow_rehearsal`,
  following the `allow_authored_execute` precedent), default closed. Not
  added until the feature exists.
- **Boundary:** `harness::lab_boundary(base, world, control)` is an
  **intersection** — the standalone lab passes its own charter
  (`lab_base()`); the daemon passes the human-owned boundary, and rehearsal
  runs under human-boundary ∩ world-scope, structurally incapable of widening
  Law III. This is already how every lab episode is scoped today.
- **Budget vocabulary:** `CampaignPlan`'s `max_llm_calls` / `max_wall_hours`
  plus exec `Limits` become the per-tick rehearsal budget. The harness reads
  no wall clock (duration is measured, never decided on); every sleep and
  STOP-poll lives in `campaign.rs`, in functions the cycle would never call.

Isolation invariants, binding on any future implementation:

1. The rehearsal data dir is never the daemon's live store. Whether
   rehearsal-learned candidates or patterns may *ever* flow into the real
   metabolism is a separate, ADR-worthy decision — unmade here.
2. The fixture library stays outside rehearsal `fs_read`. Hidden checks live
   in fixture JSON; a familiar that can read its own exam is the
   highest-order leak. The world-dir-only scoping of `lab_boundary`
   guarantees it.
3. Evaluator execution stays in harness code, scratch-dir discipline intact —
   evaluator material never enters what the familiar can perceive.
