# ADR-0036 — Tested before deployed; self-correcting after

- **Status:** accepted — implemented 2026-08-10; **inert in the field** (T-214,
  2026-08-21, per the 2026-08-17 audit of the primary live node: `allow_execute` shut —
  28 tools all `uses: 0`, 222 candidates all `generated` across 536 ticks. Fail-closed as
  designed; the label keeps the document honest about live effect — ADR-0043 §6)
- **Relates to:** `docs/SOUL.md` ("capability is unrestricted; restraint is
  constitutional"), [ADR-0010](0010-scenario-laboratory.md) /
  [ADR-0011](0011-scenario-engine.md) (the lab's effectiveness gates, referenced not
  rebuilt), [ADR-0032](0032-declared-actuators-and-the-reaction-loop.md) (the other
  "a deployed thing that turned out bad gets undone" loop),
  `crates/cycle/src/lib.rs`, `crates/kernel/src/tool.rs`

## Context

The familiar cultivated a tool, `network_status_aggregator`, that pinged three fictional
IPs (`192.168.1.10/20/30` — the classic tutorial subnet, not the household's real
`192.168.108.x`), reported "No reachable devices found," and was kept in the durable
library, re-run every ~20 minutes, feeding fabricated "network is unstable" readings into
the muse — which then theorized, tirelessly, about a connectivity crisis that existed
nowhere but in its own output. Nobody's network was failing; the familiar had invented the
failure and could not stop believing itself.

Ian's directive: **"assure the generated code is tested, successful, and follows the three
laws before deploying"; "the familiar needs to be self correcting."** Four gaps let the
fabrication survive:

1. **Persist-before-prove** — a drafted tool was written into the durable library *before*
   it ran once, with health hardcoded to "ok."
2. **Exit-clean ≠ useful** — the only correctness check was a 12-entry error denylist;
   "No reachable devices found" matched none of it, so a clean-exit fabrication read as
   healthy.
3. **The lab that gates on effectiveness was never wired to the live path** — the scenario
   laboratory (ADR-0010/0011) already runs `guard::evaluate` + assertion checks +
   lexicographic gates, but nothing cultivated ever passed through it.
4. **No autonomous retirement** — a deployed tool was retired only by a human's "refine"
   feedback or a manual prune; a fabricating sensor that exited clean was never re-judged.

## Decision — three pillars

### A · Test before deploy
A drafted tool must **earn** its place. Both authoring paths reorder to
author → constitutional review (unchanged Law III safety gate) → **trial run in a
transient script through the exact same review, boundary gates, and sandbox a deployed run
faces** → **prove it genuinely succeeded** → deploy only then. The trial's one run doubles
as the answer/reading, so nothing runs twice. On failure nothing enters the library, the
rejection is recorded visibly, and the human is told honestly ("I drafted a tool for that,
but it didn't produce a usable result — so I haven't kept it").

"Genuinely succeeded" is two-layered:
- **The floor** (`looks_unsuccessful`, deterministic, works with no LLM): the old error
  denylist **plus** a phrase-anchored *null-result* denylist ("no reachable devices", "no
  devices found", "0 hosts up", "no data", …) — output that is clean but says, in
  substance, *I found nothing*. Every needle is multiword, never a bare "empty"/"none", so
  a real reading whose value is zero or none ("battery: 0%") is never mistaken for failure.
- **The ceiling** (`assess_result`, a boundary-gated consult): when the LLM is open, the
  familiar reads its own tool's output and judges honestly whether it *accomplished the
  goal* — catching plausible-but-useless output the keywords can't. The consult can only
  ever *tighten* the floor: refused, rate-limited, or absent, it defers, and the floor
  stands alone.

### B · Self-correct after (the audit)
Deployment is not permanent tenure. Each tool carries a `null_streak` and a
`last_useful_at`, updated on every run on the **same reversible, windowed discipline as
`corruption`**: a useful run heals the streak to zero and stamps the moment; a null run
accrues; a run older than a day's window no longer counts. A per-tick `audit_tool_health`
step retires any healthy tool whose streak reaches **3** (~1h at the cultivation cadence) —
autonomously, with a visible `retired-sensor` record, no human in the loop. `best_match`
already skips an unhealthy tool, so retirement stops it re-running and frees its theory to
be re-authored. A tool that starts producing signal again before it is retired simply
heals. Declared actuator wrappers (ADR-0032) are exempt — they are not sensors and their
health is the reaction loop's business.

### C · Defense in depth at the muse
Even past A and B, a `gathered` reading now reaches the muse **only** if its producing
sensor is currently healthy *and* the reading is genuine signal (`looks_unsuccessful` says
no). A reading from a since-retired sensor is stale; a null-result reading is a fabricated
non-signal; neither resurfaces as current truth. The B9 connectivity-topic blocklist stays
beneath this as belt-and-suspenders.

## Consequences

**Good.** The exact fabrication that started this can no longer happen: a tool that finds
nothing is never deployed, and if one slips through (or its subject vanishes later), it is
retired within the hour without anyone noticing by hand. The reframe is constitutional, not
cosmetic — "capability is unrestricted; restraint is constitutional" now governs the tools
the familiar writes for itself, exactly as it governs the actions it takes.

**Bad, and accepted.**
- The floor is a denylist, and denylists are never complete; a fabrication phrased to dodge
  every null-result marker *and* pass the self-assessment consult could still deploy. The
  audit is the backstop — it retires on *behavior over time*, not wording.
- A cultivate theory whose first draft fails is marked attempted, so the same dud isn't
  re-authored every cadence; a genuinely-fixable theory that had a bad authoring moment
  won't get a second automatic try until a new need spawns a new theory. The alternative —
  re-authoring garbage every 20 minutes — is worse.
- The self-assessment consult spends one LLM call per deploy trial. Cheap, and only on the
  authoring path (reuse never re-assesses).
