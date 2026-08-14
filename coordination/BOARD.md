# The board

Rules and entry format: [README.md](README.md). Claim = move to `claimed` + your name,
in a pushed commit, scope checked against every other claimed task. Updated: 2026-08-14.

## Proposed

*(companions add here; the controller queues or declines)*

## Queued

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

### T-100 · Coordination system bring-up
- status: claimed
- owner: controller
- scope: coordination/
- depends: —
- accept: structure, rules, board, state, companion prompt exist and are landed; Ian holds the companion prompt
- notes: this very brick

## Blocked / gated

*(operational holds live in STATE.md's ledger — tasks blocked on other tasks sit here)*

## Done (recent — pruned to ~10; history is git's)

### T-090 · 2026-08-13→14 defect-and-feature cycle
- status: done
- owner: controller
- merged: 4661aef → 8363f15 (18 bricks)
- notes: names lead the roster; record-layer honesty (same-second dance, release semantics, doppelgänger guard, restoration-from-grant); the one-longitude ULP outage; DeviceRecords + device-sync; standing ReactionRules; theory drill-down; dialog answer threading; globe cluster zoom + ≈ provenance; discovery naming + narration principle. Narrative: DEVELOPMENT_LOG 2026-08-13/14
