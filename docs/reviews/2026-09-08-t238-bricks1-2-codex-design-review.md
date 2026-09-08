# T-238 bricks 1+2 production-aware P&L design review

**Verdict: REJECT.** Brick 1's pure recipe/stock arithmetic is a useful base and
the current Rust bar is green, but brick 2 does not yet make a valid
production-aware P&L decision. The live adapter copies the quote's `equilibrium`
inventory target into `Forecast` as though it were a price, then performs money
arithmetic on that unit count. The target scan also stops at the first acceptable
spot-mid-sorted berth instead of choosing the best forecast-adjusted result. The
recorded brick-2 scope's glut, freight, and new-position decay paths are not wired,
and an executed buy drops its forecast reason from both execution journal events.

Reviewer: companion:codex
Reviewed: brick 1 `d60433ca`; brick 2 `751d1eb4`; forecast-journal follow-up
`e71de016`; claim `3179a599`; current main `3179a599`
Exchange contract checked: `SpaceTrucker2196/ucf-exchange` at `5f28c45e`

## Findings

### 1. Blocker — shelf equilibrium units are treated as meal-credit prices

**File:** `crates/whisker/src/main.rs:481-519`, `:1279-1303`;
`crates/whisker/src/trade.rs:521-541`, `:721-742`, `:894-930`

**Failure scenario.** The station quote carries `stock`, `capacity`, and
`equilibrium` in the same inventory unit. The runner correctly puts all three into
a `chain::Shelf`, but then copies `Shelf.equilibrium` into `Forecast.hungry`, whose
documentation renames it an "equilibrium price." `decide_trade` subtracts the 18%
sell haircut from that value, compares it with the target mid, and uses it as
expected meal credits per unit. The journal even says the bid "heads to equilibrium
{eq}."

The exchange contract makes the dimensional error explicit. In
`Sources/UCFMarketd/Handlers/APIRoutes.swift:647-658`, the quote handler computes
`midPrice(basePrice, stock, equilibrium, swingBps)` and then publishes that same
inventory equilibrium beside stock and capacity. `docs/market/engine-design.md`
defines price as a function of `base`, `stock`, and `equilibrium`; equilibrium is
not itself a price. The brick-2 acceptance test is therefore self-confirming the
wrong contract when it inserts `(20, 60)` and calls 60 the future bid. In a live
world a shelf target of 600 units can become an expected 492 meal credits merely
because the station holds quantities in the hundreds, manufacturing a trade from
incompatible units.

**Repair accepted.** Forecast the future stock, then derive a future mid from the
exchange's integer price contract using the good's `basePrice`/`swingBps` and the
station's spread, or consume an authoritative future-quote seam if the exchange
adds one. Keep inventory quantities and meal-credit prices in distinct named types.
Pin the runner adapter against a real quote/reference fixture where equilibrium and
mid are deliberately far apart; the expected price must match the exchange formula.

### 2. Blocker — the target scan is still ordered by spot mid, not forecast P&L

**File:** `crates/whisker/src/trade.rs:714-764`

**Failure scenario.** Candidates are sorted by descending current mid. Once one
candidate clears the threshold, route, and fuel checks, line 763 breaks. A later
berth whose lower spot mid is lifted far higher by a valid forecast is never
compared. Changing the old failed-margin `break` to `continue` lets a forecast
rescue a later row only when every earlier row fails; it does not make the forecast
part of target ranking.

A transient external probe held all route costs equal: ask 25, a `higher-spot`
target at mid 40, and a `starving-works` target at mid 30 with a forecast-adjusted
unit value of 50. The current function returned `higher-spot`, estimated margin
ℳ460. Evaluating the same position at `starving-works` yields ℳ1,480 under the
function's own sizing and fuel arithmetic. Thus the code can read the chain and
still choose the worse run, contrary to the P&L objective.

**Repair accepted.** Evaluate the bounded reachable/fuelable candidate set, compute
each candidate's forecast-adjusted net after fuel and decay, and select the maximum
with a deterministic tie-break. Preserve the rate-limit bound, but do not stop at
the first spot-sorted survivor. Pin a two-target case in which a lower current mid
has the higher valid forecast value.

### 3. Blocker — brick 2's glut, freight, and new-position decay paths are absent

**File:** `crates/whisker/src/main.rs:1279-1305`;
`crates/whisker/src/trade.rs:275-333`, `:714-803`;
`crates/whisker/src/chain.rs:76-93`, `:191-201`

**Failure scenario.** The recorded brick-2 scope says both the merchant buy target
and freight ranking consult the forecast: feed an input before it starves, lift an
output before it gluts, and remain decay-aware. Current production code calls only
`chain::starving`. `chain::glutting` is referenced only by its unit test; no forecast
reaches freight doctrine; and `chain::parse_decay` is referenced only by its parser
test. `trade::surviving` charges decay when deciding whether to keep an already held
lot, but the new-buy path multiplies the full purchased units by the expected unit
price and never asks for route ticks or surviving units. A perishable forecast buy
can therefore be approved on proceeds from cargo that will not arrive.

The pure tests prove runway/headroom and that one decay field parses. They do not pin
decay's effect on a forecast or a newly opened position, nor any production-aware
freight or glut decision. This leaves most of the stated brick-2 behavior outside
the implementation and the acceptance bar.

**Repair accepted.** Represent both starving-input and glutting-output opportunities,
use them in the merchant and freight ranking seams named on the board, and charge
route-time decay before comparing total P&L. Add fixtures where glut changes the
preferred lift, supply pressure changes the preferred freight, and enough decay
turns an otherwise attractive forecast buy into an idle decision.

### 4. Blocker — the executed buy journal drops the forecast reason, and no soak proves the runner path

**File:** `crates/whisker/src/main.rs:1479-1497`, `:1519-1556`;
`crates/whisker/src/trade.rs:894-946`

**Failure scenario.** The decision carries a `why`, and advise/confirm gates can
journal it before an act. But the auto path returns `Gate::Act` without a gate
journal. After the buy succeeds, `position-opened` writes good, units, ask, target,
margin, and clock but not `why`. The following `traded` event extracts a reason only
for `TradeDecision::Sell`; every successful buy records `"why":""`. This directly
contradicts the board's landed note that the reason rides into `position-opened`
and the acceptance line requiring the forecast-justified buy to journal WHY.

The only offered proof is a pure `decide_trade` unit test. It neither exercises the
runner adapter nor writes a journal, and no recorded fixture soak was found. The
new `forecast` event shows the current hungry map, but it does not link an executed
position to the forecast that justified spending.

**Repair accepted.** Carry `why` into `position-opened` and the successful buy's
`traded` event. Add an offline runner/fixture soak that starts from reference,
station quotes, and galaxy prices; proves the spot-only decision is idle; proves
the valid forecast changes it to buy; and asserts the resulting execution journal
contains the same bounded reason. No live ship or real game action is required.

## What held

- Brick 1 remains pure and its fixture arithmetic is coherent: recipes parse,
  station-level rates sum, runway/headroom use live shelves, and starving/glutting
  lists rank by horizon.
- A missing forecast preserves the prior spot behavior, and the beyond-horizon test
  keeps a distant starvation reading from affecting the decision.
- The forecast journal is deterministic because `BTreeMap` order stabilizes its
  emitted list, and it writes when that list changes.
- A focused diff search found no household datum introduced by the T-238 changes.

## Verification

- `cargo fmt --check`: **pass**.
- `cargo test -p familiar-whisker`: **84 passed, 0 failed** (83 library + 1 binary).
- `cargo clippy -p familiar-whisker --all-targets -- -D warnings`: **pass**.
- Exchange wire/pricing trace at `5f28c45e`: **confirmed** `equilibrium` is the
  inventory pivot supplied with stock to `midPrice`, not a price.
- Transient external two-target probe described in finding 2: **reproduced** the
  spot-order early exit (`target=higher-spot margin=460`).
- Scope search: production code has no call to `chain::glutting` or
  `chain::parse_decay`, and no forecast input reaches freight doctrine.

No production code, deployment, ship, gate, game action, human record, or fleet
state was changed.
