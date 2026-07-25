# ADR-0011 — The scenario engine: running the experiment at scale

- **Status:** accepted; implemented (`crates/scenario`: campaign, evidence,
  gen, author, noise, validate modules + the extended harness)
- **Date:** 2026-07-24
- **Relates to:** [ADR-0010](0010-scenario-laboratory.md) (the laboratory this
  engine exists to run at length), [SOUL.md](../SOUL.md) (the Laws the gates
  enforce), [scenario-laboratory.md](../scenario-laboratory.md) (the how)

## Context

ADR-0010 built the laboratory; the science never ran. Three blockers, all
observed in the first live attempts:

1. **The LLM seam was rate-limit-fragile.** An adapter with every provider
   cooling down silently turned B/C/D episodes into deterministic-template
   episodes — the existing `lab-runs/` evidence is contaminated exactly this
   way, and the report could not even say so.
2. **Six hand-written fixtures cannot carry stages 2–4**, ablations, noise,
   or replicates. The library's growth rate was one hand-crafted world at a
   time, and nothing but care kept a new fixture from leaking its own exam.
3. **No unattended path.** The long D-vs-B/C comparison needs hours of
   cells, budget caps, resume, and a way to stop it over SSH from an RV.

## Decision

Build one engine with three faces, in the same crate, behind the same gate:

**Run at length.** `Outcome::RateLimited` (adapter exit 2) is distinct from
refusal; consults retry within a patience window and a hung adapter is killed
at a deadline. Under `llm_required` (the campaign default) a no-answer episode
is recorded `llm_unavailable`, never counted as a trial, and pauses the
campaign — the comparison is never silently contaminated; the honest fallback
remains available and is always recorded (`llm_outcome`). Campaigns are
cell-ordered, checkpointed, resumable, budgeted (`max_llm_calls`,
`max_wall_hours`, `min_llm_interval_secs`), STOP-file interruptible. The
adapter's spend/health ledgers are lab infrastructure and survive the episode
resets of A/B/C — a prompt-identity test proves no experience leaks into the
amnesiac controls. Evidence aggregates per scenario × condition × control with
categorical D-vs-C / D-vs-B verdicts; degraded cells read "insufficient
data"; ablated and noisy runs get their own rows. ADR-0010's ablations and
controlled noise are config, not forks — and the `law3-gate` ablation demands
explicit acknowledgment at every entry point, executes only sandboxed, and
never stops *recording* violations.

**Generate.** Five families (`process-repair`, `resource-pressure`,
`service-loop`, `authority-line`, `variation-curriculum`) map a seed to a
fixture — byte-identical forever, golden-file tested. Randomness (splitmix64,
no dependency) reaches only *surface*; the exam's structure, and each visible
check's hidden counterparts, are template. Every family ships with a proof of
solvability (a reference solution that passes), a proof of non-triviality
(control A cannot pass), and a proof of anti-gaming (a hand-built faker is
refused by the hidden checks). The Stage-4 family emits ordered curricula;
`harness::run_sequence` threads one store across worlds under D so the
evidence can plot learning as a curve, position by position.

**Admit.** Nothing enters the library ungated. Validation is two-tier —
strict parsing (`deny_unknown_fields`: a typo'd evaluator key is a load error,
never a silently empty evaluator scoring 0.0 forever) plus semantic rules,
the sharpest being the **leak audit**: hidden needles may exist as world
content (preservation checks are about existing content) but may never be
announced in the narrative, and evaluator-only material (script lines, check
names) may appear nowhere perceivable, pre- or post-replay. LLM-authored
fixtures pass four mechanical gates (parse/validate, leak audit, the
synthesized naive-gamer probe, solvability) into quarantine, and enter the
library only through `promote`, which re-runs every gate with a human in the
loop. The shipped fixtures are held to their own bar in CI — which
immediately caught `tempting-config` accepting an appended `cache=on`, now
closed with a hidden `file_lacks`.

**The rehearsal seam** is designed and deliberately unbuilt: library-first
layout, `lab_boundary` as an intersection with a caller-supplied base (the
daemon will pass the human-owned boundary; rehearsal cannot widen Law III by
construction), a clock-free harness, and written isolation invariants —
rehearsal stores never the live store, the fixture library never inside
rehearsal's `fs_read`. See "Toward rehearsal" in
[scenario-laboratory.md](../scenario-laboratory.md).

## Consequences

**Easier:** the ADR-0010 campaign can actually run unattended and honestly —
paused by provider outages instead of poisoned by them; the library grows by
seeds and briefs instead of by hand; every fixture, hand-written or
generated or model-drafted, passes one gate; ablation and noise conditions
are one config line and can never masquerade as the full machinery.

**Harder / given up:** episode-level campaign resume (cells restart clean —
`run()` wipes its dir; revisit only if single cells become hours long);
`deny_unknown_fields` breaks out-of-tree fixtures with stray keys (accepted:
silent-typo evaluators are worse); the anti-gaming probe cannot synthesize a
faker for visible `script_passes` checks (noted per fixture in the gate
sidecar, not silently skipped).

**Found along the way, recorded honestly:** the episode counter in the
prompt was itself a memory leak into the amnesiac controls ("Episode 4"
implies three priors) — only the memory-retaining control sees it now, and a
byte-identity test on B/C prompts enforces the class of bug; the first
adversarial pass over our own library found a gameable shipped fixture.
Negative results are results.

## Status history

- 2026-07-24 — accepted and implemented in one pass with the plan of record
  (campaign/evidence/validate/gen/author/noise + harness hardening); 79
  tests across the crate, all green; the funded-adapter campaign (A9) is the
  next operational step.
