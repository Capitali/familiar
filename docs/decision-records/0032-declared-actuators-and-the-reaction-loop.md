# ADR-0032 — Declared actuators and the reaction loop

- **Status:** accepted — implemented 2026-08-08 (the gate, the declaration format, the
  poll → heed → tend loop, reaction evidence, habit folding)
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

- `state_cmd` — how to read it (the motorlights text contract: a `light mode :` line
  and a `brightness : N/255  (NN%)` line);
- `actions` — act label → shell command;
- `buckets` — ordered rules mapping a raw reading to a coarse state, **where every
  bucket name must be an `actions` key: the bucket set IS the revert map.** A surface
  whose buckets are not closed over its actions cannot honor the revert promise and is
  dropped loudly at load. Reversibility is the license to act at all.
- Buckets are honest about what devices report: the SP548E never echoes its colour and
  its state block cannot even show *off*, so such a surface declares no `off` bucket —
  off remains an act it can take, not a state it can verify.

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
- Bucket coarseness hides detail (colour, exact levels); the revert restores the
  *bucket*, not the exact prior state. Declaring finer buckets is the remedy, not
  cleverer inference.
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
- A second surface (the declaration format should prove itself against something that
  is not a light).
