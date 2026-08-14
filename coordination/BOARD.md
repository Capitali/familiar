# The board

Rules and entry format: [README.md](README.md). Claim = move to `claimed` + your name,
in a pushed commit, scope checked against every other claimed task. Updated: 2026-08-14.

## Proposed

*(companions add here; the controller queues or declines)*

## Queued

### T-109 · Reasoning brief: codex's pass
- status: queued
- owner: — (reserved: companion:codex — claim when T-104 reaches a pausable point or is done)
- scope: docs/reviews/2026-08-14-reasoning-engine.md §5 (in place)
- depends: T-108
- accept: §5 written — sequencing counter-position, the C3 interpreter-tier design page, any L6+ limits; `> codex:` blocks mark disagreements
- notes: Ian (2026-08-14): claude + codex plan the reasoning engine's next steps TOGETHER — autonomous code building, observation analysis, theories, communication; both are DEVELOPERS of the mind, not participants in the mesh or the familiar's activities. T-104 synergy: FamTalker01 is the practice ground for whatever we build

### T-110 · ADR-0040 draft: the reasoning engine's next steps
- status: queued
- owner: controller
- scope: docs/decision-records/0040-*.md
- depends: T-109
- accept: converged phases from the brief as a proposed ADR for Ian; phases become board tasks with owners on acceptance
- notes: —

### T-101 · Build 85 console batch: rules list + device-name field
- status: queued
- owner: —
- scope: ios/MacApp/Resources/sphere/index.html, ios/Shared/Sources/AppModel.swift, ios/Shared/Sources/PlatformDevice.swift
- depends: —
- accept: the Device screen shows the standing rules (worldview `rules[]` sentences, one-tap disable via a new signed act) and a device-name field writing DeviceRecord.name through its door; both consoles build; fixture-verified
- notes: worldview already carries rules[] (guest-stripped) and the device store exists; the wire act for disable + name-set needs a signed endpoint in the transport (mirror /mesh/standing shape)

### T-102 · Theory-affirmation mints the rule
- status: queued
- owner: —
- scope: crates/cycle/src/lib.rs (heed path), crates/kernel/src/reaction_rule.rs
- depends: —
- accept: an assenting answer on an acted thread whose direction names a declared surface mints the matching ReactionRule (minted_from: thread:<id>), narrated into the dialog; regression pins mint-on-assent and no-mint-on-negative
- notes: closes the lighting loop end-to-end from the theory card — the CLI mint stays as the manual path

### T-103 · Reach-side reverse name lookup
- status: queued
- owner: —
- scope: crates/reach, crates/cycle (sweep call site)
- depends: —
- accept: the paced reach sweep resolves LAN neighbours' names itself (mDNS PTR / local-DNS reverse), gated by network_discovery, feeding `can-reach device:<name>` so the frontier join adopts them; hermetic test via the probe-injection seam
- notes: today a door only ever OVERHEARS names; this makes it ask. No router config may ever be required (Ian). dig -x shells out fine on macOS + the linux lighthouse

### T-105 · HumanRecord (ADR-0039 build #3)
- status: queued
- owner: —
- scope: crates/kernel (identity/dossier fold), crates/mesh/src/worldview.rs read paths
- depends: —
- accept: HumanRecord per ADR-0039 §1 folding identity registry + dossier + device associations, read paths first; dossier constraints inherited wholesale; guest projection untouched
- notes: writes (routines already live in reaction_rules.json) migrate in a later brick; read ADR-0039 before starting

### T-106 · Geo source over the wire + household-anchor question
- status: queued
- owner: —
- scope: crates/mesh/src/brief.rs (Capability), docs/decision-records/ (draft ADR)
- depends: doors on ≥ 8363f15 everywhere (the build_version signed-body trap — field lands parse-side first)
- accept: briefs carry geo source (gps|ip|manual) two-stage-safely; consoles drop the ≈ for gps; a draft ADR answers whether a daemon may inherit a household anchor from cohabiting member GPS
- notes: two-stage: parser everywhere first, emission second — see the one-longitude post-mortem in DEVELOPMENT_LOG 2026-08-13

### T-107 · ADR-0039 build #4: the migration
- status: queued
- owner: —
- scope: crates/mesh/src/record.rs, crates/mesh/src/device.rs, mesh doctor
- depends: T-105
- accept: machine-name establishments move to DeviceRecord.name in one doctor-checked pass; establishments name humans only, fleet-wide
- notes: ADR-0026's lesson — one migration, not two

## Claimed

### T-104 · FamTalker01 declares itself: the virtual smart home
- status: claimed
- owner: companion:codex
- scope: design + ops (FamTalker01's actuators.json, its observation feed), docs/decision-records/ addendum if the declaration format needs growth
- depends: —
- accept: FamTalker01's virtual controls are declared surfaces (revert-map closed), its observation points post observations, and the familiar can explore/act there under the full ADR-0032 discipline with narration; Ian sees at least one narrated act on a virtual surface
- notes: Ian (2026-08-14): a virtual smart home for the familiar to explore, begin to control, and report on when human intervention would improve efficiency or awareness. This is ADR-0032's "second surface". Planning/design task — good first companion brick
- controller (2026-08-14): welcome, codex — claim verified, scope clear. Read ADR-0032 first (declaration IS the consent; buckets must close over actions or the surface drops at load), then the narration additions in cycle's tend paths — your virtual surfaces get "what and why" speech for free once declared. FamTalker01 is linux/x86_64 at 192.168.108.11 (also seen .119); its daemon is NOT yet on current main — coordinate its upgrade through the infra session via a Proposed task, don't ssh-deploy yourself (rule 8). Virtual observation points should feed as ordinary observations through the existing ingest; if the declaration format needs a new field, that's an ADR addendum (proposed status), not a silent extension

## Blocked / gated

*(operational holds live in STATE.md's ledger — tasks blocked on other tasks sit here)*

## Done (recent — pruned to ~10; history is git's)

### T-108 · Reasoning-engine review & planning brief (draft)
- status: done
- owner: controller
- merged: (this commit)
- notes: docs/reviews/2026-08-14-reasoning-engine.md — mind-as-built map, honest limits L1-L5, direction candidates A/B/C/D, controller's phase-1 position (A1+B1+D1); §5 open for codex

### T-100 · Coordination system bring-up
- status: done
- owner: controller
- merged: 6d68b63 (+ ebf6375, lane note)
- notes: first live claim (codex → T-104) followed the protocol unaided — the system works

### T-090 · 2026-08-13→14 defect-and-feature cycle
- status: done
- owner: controller
- merged: 4661aef → 8363f15 (18 bricks)
- notes: names lead the roster; record-layer honesty (same-second dance, release semantics, doppelgänger guard, restoration-from-grant); the one-longitude ULP outage; DeviceRecords + device-sync; standing ReactionRules; theory drill-down; dialog answer threading; globe cluster zoom + ≈ provenance; discovery naming + narration principle. Narrative: DEVELOPMENT_LOG 2026-08-13/14
