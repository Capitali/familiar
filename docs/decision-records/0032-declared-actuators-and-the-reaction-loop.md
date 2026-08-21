# ADR-0032 — Declared actuators and the reaction loop

- **Status:** accepted — implemented 2026-08-08 (the gate, the declaration format, the
  poll → heed → tend loop, reaction evidence, habit folding); reading contract amended
  2026-08-15 by T-157 so the kernel no longer contains the first device's grammar.
  **Implemented and inert in the field** (T-214, 2026-08-21, per the 2026-08-17 audit of
  the primary live node): `allow_actuate` shut and no declared surface with a runner, so
  the loop has not fired there — the machinery is tested, gated shut, and waiting on the
  human's gate, which is the designed order (ADR-0005), not a defect
- **Relates to:** [ADR-0031](0031-consent-by-observation.md) (the philosophy this makes
  concrete), [ADR-0022](0022-the-human-dossier.md) (habit patterns are a dossier kind),
  [ADR-0005](0005-human-owned-capability-boundary.md) (the gate),
  `crates/kernel/src/actuator.rs`, `crates/cycle/src/lib.rs` (the 8·3 step)

## Context

ADR-0031 committed the familiar to observe → theorize → **act** → read the reaction, and
deliberately left "act" meaning internal trial work until a real control surface existed.
This record is that surface arriving: the BLE LED strip already scripted at
`~/Development/motorlights` becomes the first thing in the physical world the familiar
may touch — and the shape of *how* it may touch anything is decided here, before the
second actuator makes the shape precedent by accident.

## Decision

### Declaration is the consent

The human writes `actuators.json` in the data dir; the familiar reads it and never
writes it. An undeclared device has no path to actuation whatever any gate says — there
is no discovery-to-actuation pipeline, deliberately. Each declared surface carries:

- `state_cmd` — how to read it, plus a `state.fields` contract. Every field is either a
  quantity with a semantic name, unit, and honest range, or an opaque enumeration with
  its complete accepted values. A source extracts each field from a top-level JSON key
  (preferred for new adapters) or from declaration-owned line prefixes and delimiters
  (the compatibility path for existing device output). The kernel validates and
  evaluates this generic contract; no device vocabulary or output grammar is compiled
  into it;
- `actions` — act label → shell command;
- `buckets` — ordered, typed predicates (`eq`, `at_most`, `at_least`) over declared
  fields, with an unconditional final fallback. **Every bucket name must be an
  `actions` key: the bucket set IS the revert map.** Duplicate buckets, ill-typed or
  out-of-range predicates, incomplete reading contracts, and maps without a final
  fallback are dropped loudly. Reversibility is the license to act at all.
- Buckets remain honest about what devices report. An action that cannot be observed
  may still exist, but it is not a bucket and therefore cannot be claimed as an exact
  restoring state. Adding a fridge temperature or vent position is declaration work,
  not a kernel change.

### One gate: `allow_actuate`

Default closed, human-opened, covering **acting and polling both** — a BLE state query
is already a connection into a device, not free perception. Like self-upgrade and
outreach, it is dropped from every delegated agent scope: driving a device is the
core's reaction-honoring loop, never something a sub-plan improvises. Enforcement is
layered: the wrapper scripts carry a `familiar:actuate` marker and
`review::reaches_device_control` requires the gate at the same two execution sites as
`reaches_network`; the guard weighs each act (`ActionKind::Actuate`); and a declared
tool never federates — outbound manifest, outbound push, and inbound push all refuse it,
because a wrapper commands *this* node's device and is meaningless or intrusive anywhere
else.

### Why a declared act is `affects_person: false`

The guard sends `affects_person || !reversible` to SeekConsent — which would put a
question in front of every act: the permission-asking appliance ADR-0031 retires. For a
*declared* surface the consent question was answered at declaration time, the act is
reversible by construction (the revert map), and the reaction channel keeps consent
continuously answerable: a hand or a word undoes the change, mechanically. An
irreversible act on the same surface still routes to SeekConsent. **The honest bound:**
this reasoning holds only while the revert path and the reaction-reading stay
conservative — when in doubt, an action is irreversible and a reaction is negative.

### The loop — poll → heed → tend (cycle step 8·3)

- **Poll** reads each surface on its own pacing (`actuator_poll_secs`, co-owned
  parameter — each poll may be a 10–30s BLE connect). The familiar's own acts pre-write
  the expected bucket into the surface-state file, so the poller *structurally cannot*
  see its own hand as a change; any observed transition is externally caused. Outside a
  reaction window it is an ordinary `adjusted` observation — attributed to the sole
  present human, or the honest `someone` (excluded from every pattern, like `observer`)
  — and the habit feed. Inside a window, a transition away from what the familiar set
  **is the human undoing the change**: the strongest possible reaction.
- **Heed** reads the words: a new answer on the acted thread, or a dismissal of its
  confirm-question. Negative (deterministic whole-word list — no model judges a
  reaction) means **undo first, argue never**; anything else from the subject closes the
  window as assent.
- **Tend** acts: a pursued need-thread whose direction names a declared surface and one
  of its acts ("dim the lights this evening" → lights/dim) is carried out — one act per
  surface per rest window, guard-weighed each time. Slice 1 initiates from need-threads
  only; habit-driven initiation waits until the habit patterns have had time to
  accumulate.

### What a reaction does

A reverted act (by hand or by word) becomes, in one move: a negative trial
(`human_reverted` / `negative_reaction`, last-wins over the promotion-time trial), the
acting candidate archived, the thread abandoned (the human's words are *kept* — the
pursuit is what is discarded), a visible `demoted` observation, the habit slot it leaned
on **depreciated** (weight halved, count kept — a wrong guess still teaches, ADR-0022),
and a six-hour rest on the surface. `score_theory` then discounts repeat directions
automatically. A quiet window or an assenting answer closes as a positive trial — and
quiet earns no rest, because quiet is consent.

### Habits

`ctb|<handle>|habit|<surface>=<bucket>@h<hour>` — the dossier kind the slot grammar
anticipated, folded from `adjusted` observations by the same one evidence ladder
(`routing::subject_and_strength`, new 0.5 "adjusted" rung). The familiar's own acts
cannot self-pattern (actor `familiar` is excluded from the fold), and `familiar actuate`
— the human's own hand through the same tools — *does* feed their pattern. Node-local,
subject-readable, withdrawable, exactly like every other dossier kind.

## Consequences

**Good.** The familiar can finally be quietly useful in the world, with every act
carrying its own undo and every reaction becoming evidence. The trial machinery gains
its first human-reaction dimension.

**Bad, and accepted.**
- A wrong theory now produces a wrong act in the physical world. Mitigations: declared
  surfaces only, reversible by construction, one act per surface per window, six-hour
  rest after any rejection.
- Bucket coarseness hides detail; the revert restores the *bucket*, not the exact prior
  state. Declaring finer typed fields and buckets is the remedy, not cleverer kernel
  inference.
- **The launchd/TCC caveat:** BLE from the daemon (bleak/CoreBluetooth under a spawned
  python) needs Bluetooth permission, and prompt attribution for a launchd agent is
  uncertain. The procedure: run `familiar actuate lights state` once interactively to
  trigger the prompt; then exercise under launchd; grant python Bluetooth in System
  Settings → Privacy if blocked. Throughout, BLE failure is tool-unhealthy and visible,
  never fatal.

## Follow-on work

- Habit-driven initiation (act from a strong habit pattern, not only a need-thread) —
  after the patterns have real depth.
- Reaction-aware trial scoring beyond pass/fail (duration-weighted standing).
- Field calibration and richer adapters should remain at the declared/cultivated-tool
  edge. If supporting a new device would require a kernel type naming that device, the
  declaration is still incomplete.
