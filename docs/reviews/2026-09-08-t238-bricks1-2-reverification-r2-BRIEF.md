# Re-verification brief — T-238 bricks 1+2, round 2

**For companion:codex. Self-contained: start here cold. Launched from MacOnStick; report to
`docs/reviews/2026-09-08-t238-bricks1-2-codex-reverification-r2.md`.**

Round 1 (2026-09-08): [`2026-09-08-t238-bricks1-2-codex-design-review.md`](2026-09-08-t238-bricks1-2-codex-design-review.md)
— **REJECT**, four blockers. Repairs on `main`: `da86a59` (findings 1–4, the merchant half
of 3) and `cedd6f1` (the journal-vocabulary pin reads trade.rs). Re-verify against
**current main**, judged against T-238's acceptance line: the merchant makes a
forecast-justified buy a spot trader would not, in valid units, and says why.

## Findings and their repairs

| # | Round-1 finding | Repair | What to probe |
|---|---|---|---|
| 1 | Blocker — shelf equilibrium units treated as meal-credit prices | `da86a59` | `chain.rs`: `Units` / `Credits` newtypes; `imbalance_bps`, `mid_price`, `sell_unit_price`, `buy_unit_price` ported integer-for-integer from `UCFEngine/Economy/Pricing.swift` (neutral event modifier, clamp rails 2000/20000); `Flow::stock_at`; `parse_pricing` reads `goods[].basePrice/swingBps` and `stations[].spreadBps` from `/v1/reference`. `trade::Forecast::project` gives stock and mid now and at arrival. Pin: `the_mid_is_the_exchange_formula_and_not_the_equilibrium_count` (base 30, swing 9000, eq 600: 500→34, 0→57, 900→21). The soak fixture has equilibrium 600 and mid 34 deliberately far apart. Check the arithmetic against the Swift; check that nothing reads a count as ℳ |
| 2 | Blocker — target scan ordered by spot mid, not forecast P&L | `da86a59` | every reachable, fuelable buyer inside `ROUTE_QUESTIONS_PER_GOOD` (4) is valued at forecast net — `surviving_units × (arrival mid − haircut) − ask × units − fuel` — and the max wins with a deterministic tie-break (glut lift, pump adjacency, station name). Spot order only decides who is asked first; the pre-route screen stays lazy on spot for a berth the forecast has no word on. Pins: `a_lower_spot_with_the_higher_forecast_wins_the_target`, `prices_every_buyer_inside_the_route_budget_and_takes_the_best_net` (five buyers, four questions, never the fifth) |
| 3 | Blocker — glut, freight, new-position decay absent | merchant half `da86a59`; **freight half NOT YET** | decay charged on the NEW position (`surviving_units`); a glut at `here` breaks ties toward lifting it and the reason says so; a glut at the TARGET lowers the projected mid (Makes flow). Pins: `enough_decay_turns_a_forecast_buy_idle`, `a_glut_here_breaks_the_tie_toward_lifting_it`. The FREIGHT ranking (loads feeding a starving works / lifting a glutting one preferred among near-equal rates) needs `doctrine.rs`, the seam the iPad core is built on; it lands as its own announced commit after T-237 B4 r2 returns. Judge 3 as PARTIAL-by-design if the merchant half holds; say so |
| 4 | Blocker — executed buy drops the reason; no soak | `da86a59` + `cedd6f1` | `trade::position_opened` builds the event (one builder for runner and soak); `why` bounded to 240 chars (`bounded_why`); the buy's `traded` event carries it too. `tests/t238_forecast_soak.rs`: reference + two quote boards + galaxy documents → spot-only idle → forecast buy for works-b → `position_opened` carries the identical bounded reason. Check the runner path in main.rs writes through the same builder |

Held from round 1 (re-confirm briefly): brick 1's pure arithmetic; missing forecast preserves spot behaviour; deterministic forecast journal; no household datum.

## Method

- `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test --workspace` (never piped into `head`). Cite counts (whisker was 93 lib+bin, 1 soak; workspace 1004).
- Transient probes welcome; note exact input and output. Verdict per finding, then ACCEPT / REJECT.

## Scope and rules

- Read-only on source; output is the report file only. No install, ship, upload, deploy, game action.
- Do not edit `coordination/*`; the controller records the return.
- Off-limits regardless: `DirectFeed.swift`, `Briefs.swift`, `UCFFamiliar`, `ios/FamiliarCore`.
