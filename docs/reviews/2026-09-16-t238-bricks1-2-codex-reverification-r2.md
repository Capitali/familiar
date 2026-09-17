# T-238 bricks 1+2 codex re-verification, round 2

**Verdict: REJECT.** Findings 1, 2 and 4 are repaired, and finding 3's
merchant half is repaired. The acceptance-line merchant case now uses inventory
and money in distinct types, evaluates the bounded buyer set by forecast net,
charges decay to the newly bought position, makes the forecast-only buy, and
journals the same bounded reason through the runner's shared event builder.

The newly claimed freight half is only partial in the live runner. The pure
doctrine and JSON seam prefer a pressured load within the five-percent tie band,
but the runner constructs `fold_forecast` only inside the separately gated Trade
phase. A co-pilot key is intentionally granted Freight and not Trade, so this
ordinary freight-only configuration reaches the freight doctrine with every row's
`chain_pressure` overwritten to zero. It therefore still chooses by rate alone
when a near-equal load would feed a starving works or lift a glutting one.

Reviewer: `companion:codex`

Reviewed: round-1 report
`docs/reviews/2026-09-08-t238-bricks1-2-codex-design-review.md`; repairs
`da86a59`, `cedd6f1`, `b4c51ee` and `788d94b`; later T-238 bricks 3 and 4 as
present in this checkout; requested checkout at `51c7ce6`.

## Remaining finding

### 3. PARTIAL — Blocker — a Freight-only runner never supplies chain pressure

**File:** `crates/whisker/src/main.rs:559`, `:1214-1216`, `:1525-1531`,
`:1673-1733`, `:2258-2267`; `crates/cli/src/fleet.rs:1244-1257`;
`crates/whisker/src/doctrine.rs:725-751`, `:971-988`

**Failure scenario.** `trades` is true only when `Automation::Trade` is granted.
The fold starts with `fold_forecast = None`, and the only assignment of a real
forecast occurs inside `if trades`. After that block, every freight row is given
`forecast.pressure_on(...)` or, when the option is still absent, zero.

That absence is a supported production configuration, not a theoretical malformed
store. Pairing deliberately caps a co-pilot key carrying `auto:freight` to
`["freight"]`; Trade is available only to a key with the broader `act` scope.
Take two otherwise bookable, equally routed loads where L1 pays 900 and L2 pays
873 (three percent less), and let L2 deliver a good whose destination shelf will
starve inside the horizon. The pure doctrine's pinned input with
`L2.chain_pressure = 1` books L2. Through the live co-pilot runner, the Trade phase
does not execute, `fold_forecast` stays `None`, both rows become pressure zero, and
the doctrine books L1. The production freight automation has not consulted the
forecast.

The `doctrine` and `wire` tests prove ranking and seam parsing only after a caller
has already supplied `chain_pressure`; neither test covers the runner's grant gate
or construction of that field.

**Repair accepted.** Build the read-only fold forecast independently of the Trade
action grant whenever Freight needs to rank a board (sharing one construction with
the merchant when both grants exist), then annotate rows before the freight
doctrine. Add a runner-boundary test with grants `{Freight}`, no Trade grant, a
starving/glutting fixture and near-equal loads; it must produce nonzero pressure and
book the chain-preferred row. The grant should continue to gate market actions, not
the market facts used by freight judgment.

## Round-1 findings re-verified

### 1. REPAIRED — shelf counts are no longer used as meal-credit prices

**File:** `crates/whisker/src/chain.rs:1143-1278`;
`crates/whisker/src/trade.rs:585-702`; `crates/whisker/src/main.rs:652-704`,
`:1673-1692`

`Units` and `Credits` make the dimensional boundary explicit. `parse_pricing`
reads each good's `basePrice`/`swingBps` and each station's `spreadBps` from the
reference. The exchange arithmetic is preserved in order: imbalance is bounded by
the larger of stock/equilibrium, swing is applied in basis points, the neutral
event modifier is applied, the result is clamped to 2000–20000 bps, and the price
is derived from base. Bid rounds down and ask rounds up.

`Forecast::project` first advances the shelf in inventory units and only then
derives the current and arrival mids in credits. The pin gives the requested base
30, swing 9000, equilibrium 600 results: stock 500 → mid 34, stock 0 → 57,
stock 900 → 21. The offline fixture independently keeps equilibrium 600 and mid 34
far apart. No reviewed path copies equilibrium into a price field.

### 2. REPAIRED — forecast net, not first acceptable spot mid, selects the target

**File:** `crates/whisker/src/trade.rs:893-1056`

Spot mid orders route questions but no longer ends the scan at the first survivor.
For every asked, reachable and fuelable buyer within the four-question budget, the
merchant derives the arrival mid, applies the haircut, charges the purchase,
surviving-unit proceeds, fuel and dock fee, and retains the maximum `net`.
Deterministic ties prefer a local glut lift, then pump adjacency, then station
name. A berth without a forecast word still gets the cheap spot-based pre-route
screen.

The lower-spot/higher-forecast pin selects `starving-works` after comparing both
buyers. The five-buyer budget pin asks exactly the four spot-leading candidates,
never the fifth, and selects the best net among the priced survivors.

### 3. PARTIAL — merchant glut and new-position decay hold; freight purity holds,
but live freight wiring does not

**File:** `crates/whisker/src/trade.rs:660-670`, `:914-924`, `:1008-1047`;
`crates/whisker/src/doctrine.rs:737-751`, `:976-988`;
`crates/whisker/src/wire.rs:116-120`, `:370-388`

The merchant charges `surviving_units` on the new position before comparing total
net, so sufficiently severe route-time decay turns the forecast run idle. A
soon-full output shelf at the current berth wins an otherwise equal trade and is
named in `why`; a `Makes` flow at a target projects increasing stock and the lower
mid that follows. Those repairs hold.

The freight model itself also holds: `Forecast::pressure_on` recognizes either a
starving input at the destination or a glutting output at the origin; both the
initial booking and companion-load ranking use pressure only inside a five-percent
near-tie band; the optional seam field defaults to no word; and the output reasons
identify a chain-preferred result. The remaining finding above is the live runner
path that must supply this otherwise-correct seam for a Freight-only ship.

### 4. REPAIRED — executed buys retain the bounded reason, with an offline soak

**File:** `crates/whisker/src/trade.rs:705-735`;
`crates/whisker/src/main.rs:1977-2018`, `:2546-2584`;
`crates/whisker/tests/t238_forecast_soak.rs`

`bounded_why` caps journal text at 240 Unicode characters. The runner calls the
single `trade::position_opened` builder after a successful buy acknowledgment, and
both `position-opened` and the buy's `traded` event carry the bounded buy reason.
The journal-vocabulary pin scans `trade.rs` as well as `main.rs`, so the shared
builder's event remains part of the bridge contract.

The fixture soak starts from reference, both quote documents and galaxy prices. It
proves 500 → 0 stock and 34 → 57 mid against equilibrium 600, proves the spot-only
case is idle, proves the forecast case buys brine for `works-b`, and asserts the
shared event builder writes the identical bounded reason. The main runner calls
that same builder.

## Held from round 1

- Brick 1's pure recipe/rate/runway/headroom arithmetic remains pinned. Bricks 3
  and 4 now add scheduled dispatch windows and measured utilization without
  reintroducing the count/price confusion.
- With no forecast, the merchant retains the spot fallback; a shelf beyond the
  carry horizon does not manufacture a forecast buy.
- Forecast journal ordering remains deterministic through ordered maps and it
  journals on a changed shelf/price reading rather than the moving countdown.
- A diff search over the four repair commits found no household datum.

## Verification

- `cargo fmt --check`: **pass, exit 0**.
- `cargo clippy --all-targets -- -D warnings`: **pass, exit 0**. The existing
  `familiar-vision` build-script note that camera capture is unavailable was
  emitted; it is not a clippy diagnostic.
- `cargo test -p familiar-whisker`: **pass, exit 0** — 115 library tests, 2
  runner tests and 1 T-238 soak passed; doc tests contain no tests.
- `cargo test --workspace`: **environment failure, exit 101**, not a product
  assertion failure in the reviewed code. The managed sandbox denies loopback
  `TcpListener::bind` and nested `sandbox-exec`; the first run stopped in
  `familiar-cli` with 21 passed and 18 tests failing at
  `127.0.0.1:0` with `Operation not permitted`. Follow-up package runs reproduced
  the same environment boundary in the cycle/MCP loopback tests and the
  factory/jail nested-sandbox tests.
- `cargo test --workspace -- --list`: **pass, exit 0** — 1,026 tests listed, 0
  benchmarks. The count has grown from the brief's 1,004 after the later landed
  work.

No source, production code, deployment, ship, gate, game action, human record,
fleet state, or coordination file was changed.
