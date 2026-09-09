# Re-verification brief — T-236 brick 1, round 3

**For companion:codex. Self-contained: start here cold. Launched from MacOnStick; report to
`docs/reviews/2026-09-08-t236-brick1-codex-reverification-r3.md`.**

Round 2 (2026-09-08): [`2026-09-08-t236-brick1-codex-reverification-r2.md`](2026-09-08-t236-brick1-codex-reverification-r2.md)
— **REJECT**: 1, 5, 9 HELD; 3, 6 NOT HELD; 2, 4, 7, 8 PARTIAL. Every open finding has a
repair on `main` now. Re-verify each against **current main**, judged against the bar.

## The bar (Ian, 2026-09-04, verbatim)

> "One 'ships computer' per captain that can act across his entire fleet under a name he
> chooses. I choose Felix."

## Round-2 findings and their repairs

| # | Round-2 verdict — title | Repair | What to probe |
|---|---|---|---|
| 2 | PARTIAL — second pairing / rename on a legacy hull | `783ae8a` | `computer_origin` (typed: IdStore / LegacyStore / ShipLocal / None across every same-captain hull; disagreeing ship-local records REFUSED); `migrate_computer` copies persona + trail whole, idempotent; `adopt_siblings` rewrites every same-captain captain.json; tuned Purr retained; rename migrates first. Pins: `a_second_pairing_migrates_the_whole_ship_local_computer_and_its_sibling`, `disagreeing_ship_local_computers_refuse_a_pairing`, `a_rename_on_a_legacy_hull_keeps_the_tuned_computer` |
| 3 | NOT HELD — persona + trail one serialized mutation | `0b62ce4` | `kernel::persona::name`: prior pair remembered; temp synced; trail appended + synced; rename; DIRECTORY synced; failure at any of the last three restores the prior pair; every durability error propagates. `record_naming` removed; rename uses the one helper and exits FAILURE on an unwritable trail. Pins: kernel `a_naming_whose_persona_cannot_land_records_nothing`, `a_first_naming_that_cannot_land_leaves_no_trail`; CLI `a_rename_whose_trail_cannot_be_written_fails_and_changes_nothing` |
| 4 | PARTIAL — slug as identity / migration not idempotent | `783ae8a` | `ensure_captain_id -> Result` under `captains/.migrate.lock`; ALL siblings inspected; two ids for one captain refused; legacy slug collision refused; store moved before the id is installed; interrupted move finished. `fleet adopt-ids` assigns after inspecting every record and writes every same-captain sibling. Status + fleet brief keyed by `captain_id`. Ambiguous stale slug → 410 with `candidates`, none chosen. Pins: `adopt_ids_hands_one_captain_one_id_whichever_hull_is_first`, `adopt_ids_refuses_colliding_legacy_slugs`, `an_ambiguous_stale_slug_names_both_and_picks_none` |
| 6 | NOT HELD — persona failure after commissioning | `0b62ce4` | pairing names the computer BEFORE `instance::commission`. Pin: `a_pairing_whose_naming_fails_commissions_nothing` (trail a directory → FAILURE; one world; no new registry entry; no event) |
| 7 | PARTIAL — broken read as unnamed | host `783ae8a`; Swift be9c726 + claude/computer-state (MacOnStick) | ADDITIVE `computer_state` {named\|broken\|absent} on /ships rows, fleet-brief captains[], captain brief (context + top), ship brief context (whose `computer` word now says "(will not load: …)"). WireFeed.summary prefers it. Pins: `a_broken_captain_store_is_broken_on_every_surface_not_unnamed`, `named_and_absent_are_the_other_two_states` |
| 8 | PARTIAL — Swift open publishes before reads | MacOnStick (be9c726 and after) | open publishes only after every read; ask gated |
| 1, 5, 9 | HELD | — | re-confirm briefly |

## Method

- Rust: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace` (do not pipe `cargo test` into `head`). Cite counts.
- Swift: FamiliarSC package tests; both apps build.
- Transient probes welcome; note each probe's exact input and output. The CLI pins drive
  `fleet pair` / `rename` / `adopt-ids` end to end against a loopback stub exchange
  (`stub_exchange` in fleet.rs tests) — reuse it for probes.
- Verdict per finding: HELD / NOT HELD / PARTIAL, with file:line; then ACCEPT / REJECT.

## Scope and rules

- Read-only on source; output is the report file only. No install, ship, upload, deploy.
- Do not edit `coordination/*`; the controller records the return.
