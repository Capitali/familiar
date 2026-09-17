# Re-verification brief — T-237 B2 (FamiliarSC, the ship's computer's Apple half), round 2

**For companion:codex. Self-contained: start here cold. Launched from MacOnStick; report to
`docs/reviews/2026-09-16-t237-b2-codex-reverification-r2.md`.**

Round 1 (2026-09-08): [`docs/reviews/2026-09-08-t237-b2-codex-reverification.md`](docs/reviews/2026-09-08-t237-b2-codex-reverification.md)
— **REJECT, four blockers.** Every one has a repair on main. Re-verify each against current
main, judged against the B2 line: style may bend cadence but never truth.

## Findings and their repairs

| # | Round-1 blocker | Repair | What to probe |
|---|---|---|---|
| 1 | Grounding checks token provenance, not truth (buy→sell passed; invented station passed in conversation) | `b6e27a6` | `ios/FamiliarSC/Sources/FamiliarSC/BridgeVoice.swift` — `Grounding.claims` / `bind` / `checkReply`: a statement's SIDE and OUTCOME are bound to its source fact by strong identifiers (load / tick / proposal id), negation-aware; stations are kept in the conversational check. Pin buy→sell, approved→denied, source-station invention, omitted material qualifier (`GroundingTests`). Probe with a transient inversion; it must be REFUSED |
| 2 | Swift rejected the host's `market.margin` surface (17 vs 18) | `bf0bb9c` | `Autonomy.swift` carries `market.margin` (default advise); a SHARED contract fixture pinned by both suites (`ContractDriftTests` here; the Rust side in `crates/whisker/src/autonomy.rs`). Diff the two enumerations and defaults key by key |
| 3 | Swift `Captain` dropped `captain_id` | `bf0bb9c` | `ShipStore.swift` `Captain.captainID` (empty only for legacy); `ShipSummary.captainID` + `captainIdentity` used for every captain-scoped join (Feed.swift, WireFeed.swift, BridgeModel). Pin modern / legacy / colliding-label / rename |
| 4 | `paid-down`, `pay-down-refused`, `trade-refused` invisible or mis-ranked | `bf0bb9c` | `Notices.swift` + `TemplatedVoice.swift` classify and render them; the shared journal-events contract fixture pinned on both sides; an explicit rule for unknown future events |

Held from round 1 (re-confirm briefly): the templated lane is available when Foundation
Models is not; tool-produced dial changes stay proposals needing a captain act; no production
key or network in tests.

**Since the brief (2026-09-16):** the MacOnStick lane is landing `Persona.pronouns` (strict
reader learns the record's pronouns; titles follow the record) and the T-243 iPad half. In
scope only if present on this checkout; note it, do not grade it as a B2 repair.

## Method

- `cd ios/FamiliarSC && swift test` (sandbox note as in the B4 brief: `--disable-sandbox --scratch-path /tmp/<tag>/scratch` with the module caches under /tmp). Cite counts.
- `swift build --package-path ios/FamiliarSC -Xswiftc -warnings-as-errors` — note what fails and whether it is B2's.
- Transient probes welcome; note exact input and output. Verdict per finding, then ACCEPT / REJECT.

## Scope and rules

- Read-only on source; output is the report file only. Commit it on this branch; do not push.
- No approval, dial write, game action, deploy, ship, gate, key or network use.
- Do not edit `coordination/*`; the controller records the return.
