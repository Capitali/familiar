//! The merchant doctrine, pure. Buy a good where it is cheap, carry it, sell it
//! where it is dear — arbitrage across the moving map, the trader's complement to
//! the freight hauler (Ian, 2026-09-01: "I want KK II to trade as well as haul").
//!
//! No socket, no clock, no store — market facts + what we hold, in; one
//! [`TradeDecision`], out. Every money rule is pinned by a test, because a trader
//! that is wrong about profit loses ℳ silently where a hauler that is wrong merely
//! sits still.
//!
//! What the first LOCAL soak taught (2026-09-01, litter-clay L-run, tranquility →
//! io-slagworks), each now a rule:
//! - **The exchange enforces a MINIMUM HOLD on bought goods** (`minHoldTicks`, a
//!   day of ticks: 48 minutes on LOCAL, 14 hours on PROD). A sell before the clock
//!   is a deterministic rejection — "minimum hold (sellable at tick N)" — not an
//!   error. So a position is not flipped; it RIDES under freight for a day and is
//!   sold at whatever dear berth the hauls pass afterwards. The clock re-arms on
//!   every further buy of the same good, so stacking is never done.
//! - **One position at a time.** The soak stacked four ore buys in four folds,
//!   halving the spare hold each time, because nothing said not to. A second
//!   position also re-arms the first's clock.
//! - **The hold is the truth, not our book.** The acked sell that the fold refused
//!   left 60 units in the hold and zero in the book. The book is reconciled against
//!   `/v1/me.cargo` every fold: refused sells restore, partial fills reduce, goods
//!   we do not remember are adopted at a conservative basis.
//! - **Sell where the lot is worth most from HERE, not where it stops hurting.**
//!   What a lot cost is spent and casts no vote: the question is only whether the
//!   counter in front of us beats the dearest berth on the map net of the fuel to
//!   reach it, the spoilage on the way, and what the hull costs to keep while it
//!   travels. A loss taken to free a hold for better work is a good trade, and a
//!   position held for the dignity of its purchase price is neither dignified nor
//!   a position. Basis returns as the fallback in exactly one case: a map that
//!   says nothing about the good, where what we paid is the only reference left.
//!
//! The disposition this doctrine is meant to have, per Ian (2026-09-04), is
//! acquisitive and unsentimental — expansion as the objective, opportunity
//! weighed on instinct and arithmetic together, information treated as the thing
//! that pays, and no attachment whatsoever to a cargo that has stopped earning.
//! It is a posture, not a rulebook, and nothing here should be read as a number
//! to be obeyed rather than a judgement to be made.
//! - **Buy conservatively.** Real ask in; target MID minus a haircut (spread, tax,
//!   drift) out; margin floors per unit AND in total, and the total must clear the
//!   fuel a dedicated carry would burn — the litter-clay run cleared its unit margin
//!   and still lost ℳ to fuel.
//! - **Small positions; never below the cash floor; only reachable, fuelable
//!   buyers**, asked of the router lazily best-payer-first (each question is a
//!   `/v1/route` call on the ship's one rate-limited key).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::chain::{self, Credits, FlowKind, Units};

use crate::doctrine::Router;

/// Fuel reserve on a carry leg: the same 20% margin the freight doctrine keeps on a
/// booking. A merchant that arrives dry has sold nothing.
const CARRY_RESERVE_BPS: i64 = 12_000;

/// One good's mid price and stock at one station — a row of `/v1/galaxy/prices`.
#[derive(Debug, Clone)]
pub struct MarketRow {
    pub good: String,
    pub station: String,
    pub mid: i64,
    pub stock: i64,
}

/// One good on the CURRENT berth's board (`/v1/stations/{id}/quotes`): what we can
/// actually pay (ask) and receive (bid) here, right now, and the caps.
#[derive(Debug, Clone)]
pub struct GoodQuote {
    pub good: String,
    pub ask: i64,
    pub bid: i64,
    pub stock: i64,
    pub max_buy: i64,
    pub max_sell: i64,
}

/// A speculative position we are carrying: what we hold, what it cost, where we
/// meant to sell it, and when the exchange will let us. Persisted in the ship store
/// so a restart does not forget cost basis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Holding {
    pub good: String,
    pub units: i64,
    /// Average ℳ per unit paid, tax included — the basis every sell decision clears.
    pub avg_cost: i64,
    /// Where we intended to sell it (advisory; we sell wherever it is profitable).
    pub sell_target: String,
    /// The tick we opened the position.
    pub opened_tick: i64,
    /// The exchange's clock: no sell folds before this tick (minimum hold). Learned
    /// from the world's `minHoldTicks` at buy time and corrected from the refusal
    /// text if the world says otherwise.
    #[serde(default)]
    pub sellable_at: i64,
}

/// The whole map's mids, `/v1/galaxy/prices`. TWO SHAPES, by the exchange's own
/// design (APIRoutes `galaxyPrices`): a bare array of `{good, station, mid, stock}`
/// while the survey dial is zero, and `{rows: [...], unsurveyed: [...]}` once an
/// operator files it. Both decode; the object flips on for everyone at once and a
/// parser that knew only the array would read the whole market as empty that day.
pub fn parse_galaxy(v: &Value) -> Vec<MarketRow> {
    let rows = v
        .as_array()
        .or_else(|| v.get("rows").and_then(Value::as_array));
    rows.map(|rows| {
        rows.iter()
            .filter_map(|r| {
                Some(MarketRow {
                    good: r.get("good")?.as_str()?.to_string(),
                    station: r.get("station")?.as_str()?.to_string(),
                    mid: r.get("mid").and_then(Value::as_i64).unwrap_or(0),
                    stock: r.get("stock").and_then(Value::as_i64).unwrap_or(0),
                })
            })
            .collect()
    })
    .unwrap_or_default()
}

/// One berth's live board (`/v1/stations/{id}/quotes` → `{goods: [...]}`). A rumour
/// berth answers 200 with `goods: []` and a survey block — an empty board, which the
/// merchant reads as nothing to trade here, never as an error.
pub fn parse_board(v: &Value) -> Vec<GoodQuote> {
    v.get("goods")
        .and_then(Value::as_array)
        .map(|goods| {
            goods
                .iter()
                .filter_map(|g| {
                    Some(GoodQuote {
                        good: g.get("good")?.as_str()?.to_string(),
                        ask: g.get("ask").and_then(Value::as_i64).unwrap_or(0),
                        bid: g.get("bid").and_then(Value::as_i64).unwrap_or(0),
                        stock: g.get("stock").and_then(Value::as_i64).unwrap_or(0),
                        max_buy: g.get("maxBuyUnits").and_then(Value::as_i64).unwrap_or(0),
                        max_sell: g.get("maxSellUnits").and_then(Value::as_i64).unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The hull's cargo as `/v1/me` lists it: `[{good, units}]`.
pub fn parse_cargo(me: &Value) -> Vec<(String, i64)> {
    me.get("cargo")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|c| {
                    Some((
                        c.get("good")?.as_str()?.to_string(),
                        c.get("units").and_then(Value::as_i64).unwrap_or(0),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The tick a refusal names, from "rejected: minimum hold (sellable at tick 11655)".
pub fn sellable_tick_from_refusal(outcome: &str) -> Option<i64> {
    let idx = outcome.find("sellable at tick ")?;
    let rest = &outcome[idx + "sellable at tick ".len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// The cost basis a fill actually set: what moved, per unit, tax included — the
/// number the sell rule must clear. Rounded UP: a basis of 12.28 is 13 for the
/// purpose of clearing it. Never below the quoted ask.
pub fn basis_from_total(total: i64, units: i64, ask: i64) -> i64 {
    if units <= 0 || total <= 0 {
        return ask;
    }
    ((total + units - 1) / units).max(ask)
}

/// A position at its sell target whose bid did not clear has a STALE target: the
/// mid that chose it has moved. Pick the dearest berth on the map (not here) whose
/// haircut mid still clears basis + margin, or no target at all — then the goods
/// ride under freight until a passing bid clears, rather than the hull ferrying
/// them to a market that no longer pays (LOCAL gravy-base, 2026-09-02: three
/// carries to velvet-array, three folds of no sale, between hauls).
pub fn retarget(h: &mut Holding, here: &str, galaxy: &[MarketRow]) -> Option<String> {
    // Aim at the dearest berth that counters the good, full stop. It used to have to
    // clear what the lot cost as well, which meant a lot bought badly had nowhere to
    // be aimed at all and rode along unaddressed. Where a lot is worth MOST is a
    // question about the map; what it cost is a question about the past.
    let best = galaxy
        .iter()
        .filter(|r| r.good == h.good && r.station != here && r.mid > 0)
        .max_by_key(|r| r.mid)
        .map(|r| r.station.clone());
    let was = std::mem::replace(&mut h.sell_target, best.clone().unwrap_or_default());
    if was == h.sell_target {
        None
    } else {
        Some(format!(
            "{}: {} did not pay; now bound for {}",
            h.good,
            if was.is_empty() {
                "no market"
            } else {
                was.as_str()
            },
            if h.sell_target.is_empty() {
                "wherever a bid clears"
            } else {
                h.sell_target.as_str()
            }
        ))
    }
}

/// Can a leg costing `cost` fuel be flown on `fuel` in the tank, reserve included?
pub fn carry_affordable(cost: i64, fuel: i64) -> bool {
    fuel >= bps(cost, CARRY_RESERVE_BPS)
}

/// Take the exchange's own word for when each lot may be sold.
///
/// `/v1/me.holds` publishes `sellableAtTick` per good, and it is the same clock
/// the fold enforces — so it outranks anything we inferred. We used to learn this
/// number only by being refused, which is free but slow, and wrong in between:
/// Kibble Klipper sat at foxy's-diner on 2026-09-04 journaling "bluefin-reserve
/// sellable at t8005" with 0 credits and 23 fuel, while the exchange had been
/// saying t6336 — an hour and a half of a stranded ship declining to sell the one
/// thing that could have refuelled her, on a clock that had passed long before.
///
/// A good absent from `holds` says nothing (the exchange lists what it is
/// tracking, not what it is not), so an absent good keeps whatever we had.
pub fn adopt_exchange_clocks(holdings: &mut [Holding], holds: &[(String, i64)]) -> Vec<String> {
    let mut notes = Vec::new();
    for h in holdings.iter_mut() {
        let Some((_, theirs)) = holds.iter().find(|(g, _)| *g == h.good) else {
            continue;
        };
        if *theirs != h.sellable_at {
            notes.push(format!(
                "{}: our book said sellable at t{}, the exchange says t{} — theirs",
                h.good, h.sellable_at, theirs
            ));
            h.sellable_at = *theirs;
        }
    }
    notes
}

/// The hold clocks off `/v1/me.holds`.
pub fn parse_holds(me: &Value) -> Vec<(String, i64)> {
    me.get("holds")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|r| {
                    let g = r.get("good").and_then(Value::as_str)?;
                    let t = r.get("sellableAtTick").and_then(Value::as_i64)?;
                    Some((g.to_string(), t))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// What a lot is worth if we carry it somewhere else: the dearest berth on the
/// map, net of the fuel to reach it, and how long that takes.
///
/// Deliberately says nothing about what the lot COST. Cost is spent; the only
/// question a held lot poses is where it is worth most from here.
pub fn best_forward(
    h: &Holding,
    here: &str,
    galaxy: &[MarketRow],
    l: &Ledger,
    router: &dyn Router,
) -> Option<Forward> {
    let mut best: Option<Forward> = None;
    // Dearest first, and stop at the first one the router can actually price and
    // the tank can reach: every question here is a /v1/route on the ship's one
    // rate-limited key.
    let mut rows: Vec<&MarketRow> = galaxy
        .iter()
        .filter(|r| r.good == h.good && r.station != here && r.mid > 0)
        .collect();
    rows.sort_by_key(|r| -r.mid);
    for r in rows.iter().take(4) {
        let Some(fuel) = router.fuel_between(here, &r.station) else {
            continue;
        };
        if !carry_affordable(fuel, l.fuel_available) {
            continue;
        }
        let ticks = router
            .leg_distances_km(here, &r.station)
            .map(|legs| {
                crate::doctrine::flight_ticks(&legs, crate::doctrine::REFERENCE_ACCEL_MILLI_G)
            })
            .unwrap_or(l.min_hold.max(1))
            .max(1);
        // THE SELL SIDE SEES THE SAME FORECAST THE BUY SIDE SEES. On the LOCAL soak
        // (2026-09-08, t73323) the merchant sold a gravy-base lot here at spot
        // because velvet-array "would net 561" at its spot mid — and two ticks later
        // bought the same lot back a credit dearer, because velvet-array's shelf was
        // emptying and the buy rule, reading the forecast, valued it at 38 not 23.
        // One target, two prices, a round-trip spread paid and a hold clock reset for
        // nothing. A lot in the hold is valued where it is going at the mid it will
        // find there, capped at double the spot as every forecast is — and at TWO
        // moments: on arrival, or once the shelf has drained (inside the carry
        // horizon). Waiting is charged at the ship's own hurdle per tick, the test
        // the sell rule applies downstream, so a lot is held for a forecast exactly
        // when the forecast pays for the wait.
        let spot = r.mid - bps(r.mid, SELL_HAIRCUT_BPS);
        let hurdle = hurdle_per_tick(l);
        let mut whens = vec![ticks];
        if let Some(dry) = l
            .forecast
            .and_then(|f| f.flow_at(&r.station, &h.good, FlowKind::Eats))
            .and_then(|f| f.horizon_ticks)
        {
            whens.push(dry.clamp(ticks, l.carry_horizon().max(ticks)));
        }
        for when in whens {
            let unit = match l
                .forecast
                .and_then(|f| f.project(&r.station, &h.good, when))
            {
                Some(p) => (p.mid_then.0 - bps(p.mid_then.0, SELL_HAIRCUT_BPS)).min(spot * 2),
                None => spot,
            };
            // Arrive with less than we left with. The hold is not a vault.
            let net = unit * surviving(h, when, l) - fuel * l.fuel_price.max(0);
            let worth = net as f64 - hurdle * when as f64;
            let beats = best
                .as_ref()
                .map(|b| worth > b.net as f64 - hurdle * b.ticks as f64)
                .unwrap_or(true);
            if beats {
                best = Some(Forward {
                    station: r.station.clone(),
                    net,
                    ticks: when,
                });
            }
        }
    }
    best
}

/// The units of a lot still there after `ticks` of carrying it, at the pack's own
/// daily decay. Rounded DOWN, because a merchant who rounds spoilage up is lying
/// to himself about his own hold.
pub fn surviving(h: &Holding, ticks: i64, l: &Ledger) -> i64 {
    surviving_units(&h.good, h.units, ticks, l)
}

/// The same arithmetic for a lot not yet bought: the units a NEW position would
/// still have on arrival. A forecast buy is judged on what lands, not on what is
/// loaded (codex finding 3).
pub fn surviving_units(good: &str, units: i64, ticks: i64, l: &Ledger) -> i64 {
    let decay = l.decay_bps.and_then(|d| d.get(good).copied()).unwrap_or(0);
    if decay <= 0 || ticks <= 0 {
        return units;
    }
    let days = ticks as f64 / l.ticks_per_day.max(1) as f64;
    let kept = (1.0 - decay as f64 / 10_000.0).max(0.0).powf(days);
    ((units as f64) * kept).floor() as i64
}

/// A lot's best realization away from here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Forward {
    pub station: String,
    /// Proceeds after the carry's fuel.
    pub net: i64,
    /// Ticks to get there.
    pub ticks: i64,
}

/// The bar a waiting lot has to clear, in ℳ per tick.
///
/// A hold full of cargo that is not improving faster than the hull costs to keep
/// is a hold losing money, however the cargo's basis reads. The lease is the
/// honest floor: on PROD it is 600 a day over 288 ticks, so a little over 2 ℳ a
/// tick of pure hurdle before any question of a better cargo arises.
pub fn hurdle_per_tick(l: &Ledger) -> f64 {
    let per_day = l.daily_fixed_cost.max(0) as f64;
    let ticks = l.ticks_per_day.max(1) as f64;
    per_day / ticks
}

/// The speculative book from its stored JSON. Absent or unreadable is an empty
/// book — a merchant that cannot read its own ledger owns nothing, which is the
/// safe reading. (The file is the runner's: `store::load_holdings` — T-237 B4.)
pub fn parse_holdings(text: &str) -> Vec<Holding> {
    serde_json::from_str(text).unwrap_or_default()
}

/// The book as the store writes it, any zeroed-out position dropped on the way.
pub fn holdings_json(holdings: &[Holding]) -> Option<Vec<u8>> {
    let live: Vec<&Holding> = holdings.iter().filter(|h| h.units > 0).collect();
    serde_json::to_vec_pretty(&live).ok()
}

/// Bring the book to what the hold actually holds. `cargo` is `/v1/me.cargo`, which
/// lists the MERCHANT'S goods only: contract freight never enters the actor's cargo
/// map (engine: only `marketBuy`/`marketSell` and the galley touch it; `holdUsed` is
/// the same sum). An earlier revision subtracted the active contract's units here and
/// "corrected" a 32-unit lot to 22 while a 10-unit load rode along (LOCAL gravy-base,
/// 2026-09-02). `basis_hint(good)` prices a good we find aboard but do not
/// remember — the ask here if quoted, else the dearest mid on the map, so an
/// adopted lot is never sold at a phantom profit — and names the berth paying that
/// mid, so the lot has somewhere to be carried once its clock passes. Returns a note
/// per change.
pub fn reconcile_hold(
    holdings: &mut Vec<Holding>,
    cargo: &[(String, i64)],
    basis_hint: &dyn Fn(&str) -> (i64, String),
    tick: i64,
) -> Vec<String> {
    let mut notes = Vec::new();
    let ours = |good: &str| -> i64 {
        cargo
            .iter()
            .filter(|(g, _)| g == good)
            .map(|(_, u)| *u)
            .sum::<i64>()
            .max(0)
    };
    for h in holdings.iter_mut() {
        let actual = ours(&h.good);
        if actual != h.units {
            notes.push(format!(
                "{}: book said {} units, hold has {} — book corrected",
                h.good, h.units, actual
            ));
            h.units = actual;
        }
    }
    holdings.retain(|h| h.units > 0);
    for (good, _) in cargo {
        if holdings.iter().any(|h| &h.good == good) {
            continue;
        }
        let units = ours(good);
        if units <= 0 {
            continue;
        }
        let (basis, target) = basis_hint(good);
        notes.push(format!(
            "{good}: {units} units aboard the book did not know — adopted at basis {basis}, for {target}"
        ));
        holdings.push(Holding {
            good: good.clone(),
            units,
            avg_cost: basis,
            sell_target: target,
            // Ours to carry from now; bought at a tick only the exchange knows.
            opened_tick: tick,
            sellable_at: 0,
        });
    }
    notes
}

/// What the merchant wants to do this fold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeDecision {
    /// Nothing worth doing (with the reason, for the journal).
    Idle { why: String },
    /// Sell units of a held good at the current berth's bid.
    Sell {
        good: String,
        units: i64,
        why: String,
    },
    /// Buy units of a good at the current berth, meaning to sell it at `sell_target`.
    Buy {
        good: String,
        units: i64,
        sell_target: String,
        est_margin: i64,
        /// The ground the buy stands on — spot, or the forecast that justified a
        /// run a spot trader would not have made. Journaled, so the book says why.
        why: String,
        /// What sized the position: `hold`, `cash` or `shelf` — the ship's own
        /// evidence for the frame ladder (T-242).
        bound: String,
    },
}

/// Sell only when the bid clears cost basis by this fraction (12%): the trade must
/// beat the round-trip friction, not merely break even.
const SELL_MARGIN_BPS: i64 = 1200;
/// The haircut applied to a target's MID when estimating sale proceeds (18%): stands
/// in for the bid-spread, tax, and the mid drifting before we arrive.
const SELL_HAIRCUT_BPS: i64 = 1800;
/// A buy must show at least this estimated margin per unit over cost (20%).
const BUY_MARGIN_BPS: i64 = 2000;
/// ...and at least this much in total, AFTER the carry's fuel: a run that nets
/// less than a docking fee or two is not worth a day of hold space.
const MIN_TOTAL_MARGIN: i64 = 150;
/// Never spend more than this fraction of cash (25%) on one speculative position.
const MAX_CASH_BPS: i64 = 2500;
/// Never fill more than this fraction of the (spare) hold (50%) with one bet.
const MAX_HOLD_BPS: i64 = 5000;
/// Route questions per good per fold: the ship's one rate-limited key.
const ROUTE_QUESTIONS_PER_GOOD: i64 = 4;
/// How runs rank: net first; then a run that lifts a glut here; then a
/// pump-adjacent buyer; then the station name, so equal runs tie deterministically.
type RunKey = (i64, bool, bool, std::cmp::Reverse<String>);
/// A position still unsold this many hold-periods after its clock is liquidated at
/// the next bid: bounded risk beats stuck risk.
const STUCK_HOLDS: i64 = 2;
/// Below this cash we do not open new speculative positions — trading never starves
/// the ship of the credits it needs to refuel and service its lease.
const MIN_CASH_FLOOR: i64 = 2000;

fn bps(value: i64, b: i64) -> i64 {
    value * b / 10_000
}

/// The market facts one judgment needs, besides the board and the map.
#[derive(Debug, Clone, Default)]
pub struct Ledger<'a> {
    pub here: &'a str,
    pub tick: i64,
    pub credits: i64,
    /// Units of hold not committed to freight.
    pub spare_hold: i64,
    /// True when a booked contract's cargo would not fit beside what we carry.
    pub need_hold: bool,
    /// What a carry leg can leave with (a full tank at a pump).
    pub fuel_available: i64,
    /// ℳ per unit of fuel, for charging a carry to the trade.
    pub fuel_price: i64,
    /// The world's minimum hold, ticks.
    pub min_hold: i64,
    /// Mortgage payment + lease service per day: what the hull costs to merely
    /// exist. It is the floor under every hurdle rate here, because a position
    /// improving slower than the lease bites is losing money while it waits.
    pub daily_fixed_cost: i64,
    /// Ticks in a world-day, for turning that daily charge into a per-tick one.
    pub ticks_per_day: i64,
    /// How fast each good rots, bps per day (the pack's `decayBps`). Cargo waiting
    /// for a better price is cargo spoiling at the same time, and on the luxuries
    /// that is not a rounding error: bluefin sheds 23% of itself a day.
    pub decay_bps: Option<&'a BTreeMap<String, i64>>,
    /// The chain's forecast for this fold, or None on a world with no recipes on
    /// the wire — in which case the merchant is exactly the spot trader it was.
    pub forecast: Option<&'a Forecast>,
    /// The credit line this hull may put to work, ALREADY GATED by the captain's
    /// `market.margin` dial — zero when they have not opened it.
    ///
    /// Ian, 2026-09-05: "working capital should be utilized to maximize success;
    /// that would be proper servitude and following the three laws." Kibble
    /// Klipper was the case that asked the question: idle at foxy's-diner with
    /// ℳ411 in hand, ℳ8,574 of untouched line, and a merchant that could not see
    /// it — every credit it earned went to the overdraft before the purse, so a
    /// solvent ship sat still for want of money it had.
    pub borrowable: i64,
}

/// What the supply chain says a counter is ABOUT to do (T-238 brick 2). Built
/// each fold from `chain::flows` — the recipes on the wire against live shelves —
/// and the exchange's price register, and consulted when the merchant scores a
/// target: a works whose input shelf is draining will be bidding UP for that good
/// by the time we arrive, so the spot mid understates what the run is worth; a
/// works whose output shelf is filling will be bidding DOWN, and the spot
/// overstates it. Ian, 2026-09-02: "when trading, are we planning routes between
/// our ships to deliver goods to processing facilities… our plans looking
/// forward."
///
/// The projection is in the exchange's own units all the way: stock (units) along
/// the flow, then the exchange's mid formula (ℳ), then the haircut. It never reads
/// an inventory number as money (codex finding 1), and it says "absent events"
/// because the event modifier is not visible from here.
#[derive(Debug, Default, Clone)]
pub struct Forecast {
    /// Every station×good flow the recipes describe, with its live shelf folded in
    /// where the caller had one. Only flows WITH a shelf can be priced.
    pub flows: Vec<chain::Flow>,
    pub pricing: chain::Pricing,
    /// The window the caller planned with: what starves or gluts inside it.
    pub horizon_ticks: i64,
    /// What the captain's OTHER hulls already have bound for a shelf: (station,
    /// good) → units aboard sister ships whose sell target is that station. A
    /// fleet that works together does not send two ships to fill one starving
    /// works; the second sees the first's cargo as stock already on its way (Ian,
    /// 2026-09-09: "All three should be working together to maximize profit").
    pub inbound: BTreeMap<(String, String), i64>,
}

/// One priced projection: the shelf now and at arrival, and the mid each implies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Projection {
    pub kind: FlowKind,
    pub stock_now: Units,
    pub stock_then: Units,
    pub mid_now: Credits,
    pub mid_then: Credits,
    /// Ticks until the shelf is empty (Eats) or full (Makes), at full lines.
    pub horizon_ticks: Option<i64>,
}

impl Forecast {
    pub fn build(
        recipes: &[chain::Recipe],
        shelves: &[chain::Shelf],
        pricing: &chain::Pricing,
        horizon_ticks: i64,
    ) -> Forecast {
        Forecast {
            flows: chain::flows(recipes, shelves),
            pricing: pricing.clone(),
            horizon_ticks,
            inbound: BTreeMap::new(),
        }
    }

    pub fn flow_at(&self, station: &str, good: &str, kind: FlowKind) -> Option<&chain::Flow> {
        self.flows
            .iter()
            .find(|f| f.station == station && f.good == good && f.kind == kind)
    }

    /// Inputs whose shelf runs dry inside the horizon, most urgent first.
    pub fn starving(&self) -> Vec<&chain::Flow> {
        chain::starving(&self.flows, self.horizon_ticks)
    }

    /// Outputs whose shelf fills inside the horizon, most urgent first.
    pub fn glutting(&self) -> Vec<&chain::Flow> {
        chain::glutting(&self.flows, self.horizon_ticks)
    }

    /// The chain's word on a freight load (T-238 finding 3, freight half): +1 when
    /// its destination EATS the good from a shelf that runs dry inside the horizon,
    /// or its origin MAKES the good onto a shelf that fills inside it. A tie-break
    /// for the freight doctrine, never money.
    pub fn pressure_on(&self, origin: &str, dest: &str, good: &str) -> i64 {
        let inside = |f: &chain::Flow| f.horizon_ticks.is_some_and(|h| h <= self.horizon_ticks);
        let feeds = self.flow_at(dest, good, FlowKind::Eats).is_some_and(inside);
        let lifts = self
            .flow_at(origin, good, FlowKind::Makes)
            .is_some_and(inside);
        i64::from(feeds || lifts)
    }

    /// The mid at `station` for `good`, now and `ticks_ahead` from now, from the
    /// shelf's flow and the exchange's formula. Where a berth both eats and makes
    /// a good the eating flow speaks (it is the one that moves the bid). None
    /// without a shelf, a flow, or a base price for the good.
    pub fn project(&self, station: &str, good: &str, ticks_ahead: i64) -> Option<Projection> {
        let flow = self
            .flow_at(station, good, FlowKind::Eats)
            .or_else(|| self.flow_at(station, good, FlowKind::Makes))?;
        let shelf = flow.shelf.as_ref()?;
        let &(base, swing) = self.pricing.goods.get(good)?;
        let eq = Units(shelf.equilibrium);
        let stock_now = Units(shelf.stock);
        // The fleet's own cargo lands on the shelf before ours does: count it in.
        let fleet = self
            .inbound
            .get(&(station.to_string(), good.to_string()))
            .copied()
            .unwrap_or(0);
        let stock_then = Units(
            (flow.stock_at(ticks_ahead)?.0 + fleet).min(shelf.capacity.max(shelf.stock + fleet)),
        );
        Some(Projection {
            kind: flow.kind,
            stock_now,
            stock_then,
            mid_now: chain::mid_price(Credits(base), stock_now, eq, swing),
            mid_then: chain::mid_price(Credits(base), stock_then, eq, swing),
            horizon_ticks: flow.horizon_ticks,
        })
    }
}

/// The longest reason the journal will carry. A reason is for a reader, not a
/// dump; what would not fit is not the reason.
pub const WHY_MAX_CHARS: usize = 240;

pub fn bounded_why(why: &str) -> String {
    if why.chars().count() <= WHY_MAX_CHARS {
        return why.to_string();
    }
    let mut out: String = why.chars().take(WHY_MAX_CHARS - 1).collect();
    out.push('…');
    out
}

/// The `position-opened` journal event, built in one place so the runner and the
/// soak write the same record — and so the reason the merchant gave for spending
/// is the reason the journal shows (codex finding 4).
pub fn position_opened(
    now: i64,
    tick: i64,
    h: &Holding,
    est_margin: i64,
    why: &str,
    bound: &str,
) -> Value {
    serde_json::json!({
        "at": now, "tick": tick, "event": "position-opened",
        "good": h.good, "units": h.units, "ask": h.avg_cost, "sell_target": h.sell_target,
        "est_margin": est_margin, "sellable_at": h.sellable_at, "why": bounded_why(why),
        "bound": bound,
    })
}

impl Ledger<'_> {
    /// How far ahead a buy made now can look: the hold clock (nothing sells before
    /// it) plus one posting window. A shelf that runs dry inside this is a shelf
    /// that will be bidding up while we are still allowed to arrive and sell.
    pub fn carry_horizon(&self) -> i64 {
        self.min_hold.max(1) + 96
    }

    /// What the merchant can actually put behind a position: cash plus whatever
    /// of the line the captain has opened. Cash alone when they have not.
    pub fn working_capital(&self) -> i64 {
        self.credits.saturating_add(self.borrowable.max(0))
    }
}

/// The merchant judgment.
pub fn decide_trade(
    l: &Ledger,
    board: &[GoodQuote],
    galaxy: &[MarketRow],
    holdings: &[Holding],
    pumps: &BTreeSet<String>,
    router: &dyn Router,
) -> TradeDecision {
    let here = l.here;
    // 1. SELL — realize a holding whose bid here clears its basis, or cut one that is
    //    stuck (long past its clock, or the ship needs the space). Never before the
    //    exchange's clock: that is a refusal, not a sale.
    let mut waiting: Option<String> = None;
    for h in holdings {
        if h.units <= 0 {
            continue;
        }
        // `sellable_at == 0` means the clock is UNKNOWN, not zero: goods we found in
        // the hold rather than bought (a captain's own cargo, adopted on a fold) were
        // bought at a tick we never saw. Assuming a fresh day would freeze the
        // captain's own cargo for a world-day of ours; the exchange is the authority
        // and says so for free — an early sell is refused with the true tick and no
        // money moves, and `sellable_at` is set from that refusal. So: try, and learn.
        if h.sellable_at > 0 && l.tick < h.sellable_at {
            waiting = Some(format!("{} sellable at t{}", h.good, h.sellable_at));
            continue;
        }
        let Some(q) = board.iter().find(|q| q.good == h.good) else {
            continue;
        };
        let sellable = h.units.min(q.max_sell.max(0));
        if sellable <= 0 {
            continue;
        }
        // Stuck is measured from when the lot came into our care, never from an
        // unknown clock: a lot adopted this fold is not "carried too long".
        let stuck = l.tick > h.opened_tick + (STUCK_HOLDS + 1) * l.min_hold.max(1) || l.need_hold;

        // THE SELL TEST IS FORWARD-LOOKING. What the lot cost is spent and gone,
        // and a rule that waits for the bid to clear basis is a rule that lets a
        // bad buy freeze a good hold for as long as the market disagrees — Ian,
        // 2026-09-04: "maximizing profits and continuous growth are the doctrine.
        // If that means taking a loss to gain a more profitable route, cargo,
        // contract, then that needs to be part of the calculation."
        //
        // So the only question is which is worth more from HERE: the bid on the
        // counter in front of us, or the dearest berth on the map net of the fuel
        // to reach it — and if the far berth is worth more, whether it is worth
        // more FAST ENOUGH to beat what the hull costs to keep while it waits.
        let here_now = q.bid * sellable;
        // Do we know anything about this good's market at all? An empty map is not
        // the news that nowhere pays better — it is no news, and the difference
        // matters. Blind, the only reference a merchant has is what he paid, so the
        // old basis floor stands as the fallback; sighted, basis has no vote.
        let sighted = galaxy.iter().any(|r| r.good == h.good && r.station != here);
        if !sighted {
            if q.bid >= h.avg_cost + bps(h.avg_cost, SELL_MARGIN_BPS) {
                return TradeDecision::Sell {
                    good: h.good.clone(),
                    units: sellable,
                    why: format!(
                        "bid {} clears basis {} (+margin) at {here}, and the map is blank                          for this good",
                        q.bid, h.avg_cost
                    ),
                };
            }
            if stuck {
                return TradeDecision::Sell {
                    good: h.good.clone(),
                    units: sellable,
                    why: format!(
                        "liquidating stuck position: bid {} vs basis {} ({})",
                        q.bid,
                        h.avg_cost,
                        if l.need_hold {
                            "hold needed"
                        } else {
                            "carried too long"
                        }
                    ),
                };
            }
            continue;
        }

        let forward = best_forward(h, here, galaxy, l, router);
        let improvement = forward
            .as_ref()
            .map(|f| (f.net - here_now) as f64 / f.ticks.max(1) as f64)
            .unwrap_or(0.0);
        let hurdle = hurdle_per_tick(l);
        if q.bid > 0 && improvement <= hurdle {
            let against = match &forward {
                Some(f) => format!(
                    "{} would net {} in {} ticks ({:.1} ℳ/tick, hurdle {:.1})",
                    f.station, f.net, f.ticks, improvement, hurdle
                ),
                None => "no berth on the map pays better and is reachable".into(),
            };
            return TradeDecision::Sell {
                good: h.good.clone(),
                units: sellable,
                why: format!(
                    "taking {} here at bid {} (basis {}): {against}",
                    here_now, q.bid, h.avg_cost
                ),
            };
        }
        if stuck {
            return TradeDecision::Sell {
                good: h.good.clone(),
                units: sellable,
                why: format!(
                    "liquidating stuck position: bid {} vs basis {} ({})",
                    q.bid,
                    h.avg_cost,
                    if l.need_hold {
                        "hold needed"
                    } else {
                        "carried too long"
                    }
                ),
            };
        }
    }

    // 2. BUY — an arbitrage we can carry. One position at a time (a second buy re-arms
    //    the clock and halves the hold again); never when cash is at the floor, the
    //    hold is tight, or freight wants the space.
    if holdings.iter().any(|h| h.units > 0) {
        return TradeDecision::Idle {
            why: waiting.unwrap_or_else(|| "holding a position; no bid here clears it".into()),
        };
    }
    if l.need_hold || l.spare_hold <= 0 || l.working_capital() < MIN_CASH_FLOOR {
        return TradeDecision::Idle {
            why: "no room/cash to open a position".into(),
        };
    }

    // The best good sold here, by estimated total margin to its best reachable buyer.
    let mut best: Option<(RunKey, TradeDecision)> = None;
    let mut priced = 0; // candidate buyers the router could price
    let mut unfuelable = 0; // ...of which the tank could not reach
    let mut too_small = 0; // ...runs whose total margin did not clear fuel + floor
    for q in board {
        // Buyable here: in stock, has an ask, and a positive shelf.
        if q.stock <= 0 || q.ask <= 0 || q.max_buy <= 0 {
            continue;
        }
        // Where does this good fetch the most, ONCE WE GET THERE? Every reachable,
        // fuelable buyer inside the route budget is valued at its forecast-adjusted
        // net — units that survive the carry, at the mid the shelf will show on
        // arrival, less the fuel — and the best net wins. Spot order only decides
        // which berths the router is asked about first: the budget is a rate limit
        // on the ship's one key, not a stopping rule (codex finding 2).
        let mut candidates: Vec<&MarketRow> = galaxy
            .iter()
            .filter(|r| r.good == q.good && r.station != here && r.mid > 0)
            .collect();
        candidates.sort_by(|a, b| b.mid.cmp(&a.mid).then_with(|| a.station.cmp(&b.station)));
        // Lifting this good off a shelf that is about to fill keeps the line here
        // running (a full shelf stalls it, and starves every buyer downstream). It
        // adds no money to the run — it breaks ties between equal runs.
        let lifts_glut = l
            .forecast
            .and_then(|f| f.flow_at(here, &q.good, FlowKind::Makes))
            .filter(|f| {
                f.horizon_ticks
                    .is_some_and(|h| h <= l.forecast.map_or(0, |f| f.horizon_ticks))
            })
            .and_then(|f| f.horizon_ticks);
        // (net, lifts a glut, pump-adjacent, station) → the decision
        let mut best_run: Option<(RunKey, TradeDecision)> = None;
        let mut asked = 0;
        for row in candidates {
            if asked >= ROUTE_QUESTIONS_PER_GOOD {
                break;
            }
            let spot = row.mid - bps(row.mid, SELL_HAIRCUT_BPS);
            // Cheap screen before a route question. A berth the forecast has no
            // word on must clear the hurdle on its spot; one it does have a word on
            // may be lifted to at most double its spot (a reading of a rate, not a
            // promise), so it is asked about even when the spot alone would not pay.
            let has_word = l
                .forecast
                .is_some_and(|f| f.project(&row.station, &q.good, 0).is_some());
            let ceiling = if has_word { spot * 2 } else { spot };
            if ceiling - q.ask < bps(q.ask, BUY_MARGIN_BPS).max(1) {
                continue;
            }
            asked += 1; // a question is a question, answered or not
            let Some(cost) = router.fuel_between(here, &row.station) else {
                continue; // unreachable / unpriceable — not an arbitrage
            };
            priced += 1;
            // ...plus the leg from that market to a pump, or the run ends there.
            let cost = cost + crate::doctrine::onward_to_pump(&row.station, pumps, router);
            if !carry_affordable(cost, l.fuel_available) {
                unfuelable += 1;
                continue; // a buyer we cannot fly to is ballast
            }
            // When we could sell there: the flight, and never before the hold clock.
            let flight = router
                .leg_distances_km(here, &row.station)
                .map(|legs| {
                    crate::doctrine::flight_ticks(&legs, crate::doctrine::REFERENCE_ACCEL_MILLI_G)
                })
                .unwrap_or(l.min_hold.max(1))
                .max(1);
            let sell_ticks = flight.max(l.min_hold.max(1));
            let (unit_value, why) =
                match l
                    .forecast
                    .and_then(|f| f.project(&row.station, &q.good, sell_ticks))
                {
                    Some(p) => {
                        let then = p.mid_then.0 - bps(p.mid_then.0, SELL_HAIRCUT_BPS);
                        let verb = match p.kind {
                            FlowKind::Eats => "eats",
                            FlowKind::Makes => "makes",
                        };
                        (
                            then.min(spot * 2),
                            format!(
                            "forecast: {} {verb} {} — shelf {}→{} by t+{sell_ticks}, mid {}→{} \
                             (spot mid {}, absent events)",
                            row.station, q.good, p.stock_now.0, p.stock_then.0, p.mid_now.0,
                            p.mid_then.0, row.mid
                        ),
                        )
                    }
                    None => (
                        spot,
                        format!("spot: mid {} at {} less the haircut", row.mid, row.station),
                    ),
                };
            let per_unit_margin = unit_value - q.ask;
            if per_unit_margin <= 0 || per_unit_margin < bps(q.ask, BUY_MARGIN_BPS) {
                continue;
            }
            // Size the position: bounded by cash, spare hold, and the shelf.
            let by_cash = bps(l.working_capital(), MAX_CASH_BPS) / q.ask.max(1);
            let by_hold = bps(l.spare_hold, MAX_HOLD_BPS).max(0);
            let units = by_cash.min(by_hold).min(q.max_buy);
            let bound = if units == by_hold && by_hold <= by_cash {
                "hold"
            } else if units == by_cash {
                "cash"
            } else {
                "shelf"
            };
            if units <= 0 {
                continue;
            }
            // The run, whole: what LANDS at the forecast value, less what we paid for
            // what we loaded, less the carry's fuel even when a haul ends up paying
            // for the miles — the litter-clay lesson. Spoilage is charged here, on
            // the new position, not only on lots already held.
            let landing = surviving_units(&q.good, units, sell_ticks, l);
            // The berth charges to dock: a run's cost beside the fuel (the reference
            // prices it; the pilot never charged it until 2026-09-09).
            let dock = l
                .forecast
                .and_then(|f| f.pricing.dock.get(row.station.as_str()).copied())
                .unwrap_or(0);
            let net = unit_value * landing - q.ask * units - cost * l.fuel_price.max(0) - dock;
            if net < MIN_TOTAL_MARGIN {
                too_small += 1;
                continue;
            }
            let why = match lifts_glut {
                Some(h) => format!(
                    "{why}; lifts {here}'s {} before its shelf fills in {h}t",
                    q.good
                ),
                None => why,
            };
            let key = (
                net,
                lifts_glut.is_some(),
                pumps.contains(&row.station),
                std::cmp::Reverse(row.station.clone()),
            );
            if best_run.as_ref().is_none_or(|(k, _)| key > *k) {
                best_run = Some((
                    key,
                    TradeDecision::Buy {
                        good: q.good.clone(),
                        units,
                        sell_target: row.station.clone(),
                        est_margin: net,
                        why: bounded_why(&why),
                        bound: bound.to_string(),
                    },
                ));
            }
        }
        let Some((key, decision)) = best_run else {
            continue;
        };
        if best.as_ref().is_none_or(|(k, _)| key > *k) {
            best = Some((key, decision));
        }
    }

    best.map(|(_, d)| d).unwrap_or_else(|| TradeDecision::Idle {
        why: if priced > 0 && priced == unfuelable {
            format!("{unfuelable} arbitrage(s) on the board, none flyable on fuel {}", l.fuel_available)
        } else if too_small > 0 {
            format!("{too_small} arbitrage(s) on the board too small to clear fuel + ℳ{MIN_TOTAL_MARGIN}")
        } else {
            "no profitable, carryable arbitrage on the board".into()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Reach(bool);
    impl Router for Reach {
        fn fuel_between(&self, _: &str, _: &str) -> Option<i64> {
            if self.0 {
                Some(50)
            } else {
                None
            }
        }
    }

    /// A router that prices only the named berths (others are off the lane graph)
    /// and counts every question it is asked.
    struct Chart {
        priced: Vec<(&'static str, i64)>,
        asked: std::cell::RefCell<Vec<String>>,
    }
    impl Router for Chart {
        fn fuel_between(&self, _: &str, to: &str) -> Option<i64> {
            self.asked.borrow_mut().push(to.to_string());
            self.priced.iter().find(|(s, _)| *s == to).map(|(_, f)| *f)
        }
    }

    fn q(good: &str, ask: i64, bid: i64, stock: i64) -> GoodQuote {
        GoodQuote {
            good: good.into(),
            ask,
            bid,
            stock,
            max_buy: 1000,
            max_sell: 1000,
        }
    }
    fn row(good: &str, station: &str, mid: i64) -> MarketRow {
        MarketRow {
            good: good.into(),
            station: station.into(),
            mid,
            stock: 100,
        }
    }
    fn pumps() -> BTreeSet<String> {
        BTreeSet::new()
    }
    fn held(good: &str, units: i64, avg_cost: i64, opened: i64, sellable_at: i64) -> Holding {
        Holding {
            good: good.into(),
            units,
            avg_cost,
            sell_target: "foxys-diner".into(),
            opened_tick: opened,
            sellable_at,
        }
    }
    /// Berthed at `here`, flush with cash, tank and room, clock long past.
    fn at(here: &'static str, tick: i64) -> Ledger<'static> {
        Ledger {
            here,
            tick,
            credits: 10_000,
            spare_hold: 120,
            need_hold: false,
            fuel_available: 500,
            fuel_price: 2,
            min_hold: 288,
            daily_fixed_cost: 600,
            ticks_per_day: 288,
            decay_bps: None,
            forecast: None,
            borrowable: 0,
        }
    }

    /// A forecast for the tests: `station` EATS `good` at `rate` units per
    /// kilotick from a shelf of `stock` against `eq`, priced at `base`/`swing`.
    fn eating(
        station: &str,
        good: &str,
        stock: i64,
        eq: i64,
        rate: i64,
        base: i64,
        swing: i64,
    ) -> Forecast {
        let shelf = chain::Shelf {
            station: station.into(),
            good: good.into(),
            stock,
            capacity: eq * 2,
            equilibrium: eq,
        };
        let mut fc = Forecast {
            flows: vec![chain::Flow {
                station: station.into(),
                good: good.into(),
                kind: FlowKind::Eats,
                rate_per_kilotick: rate,
                shelf: Some(shelf),
                horizon_ticks: (rate > 0).then(|| stock * 1000 / rate),
            }],
            pricing: chain::Pricing::default(),
            horizon_ticks: 288 + 96,
            inbound: BTreeMap::new(),
        };
        fc.pricing.goods.insert(good.into(), (base, swing));
        fc
    }

    /// T-238 brick 2's accept line: the merchant makes a forecast-justified buy a
    /// spot-arb trader would not, and says why — in the exchange's own units.
    /// Brine asks 24 here. works-b's mid is 34 (base 30, swing 9000, 500 on a
    /// 600 shelf): 28 after the haircut, under the 20% hurdle — no spot trade. But
    /// works-b EATS 13,333 brine a kilotick: by the hold clock (288 ticks) the
    /// shelf is EMPTY, the mid is 57, and the run is worth making. The
    /// equilibrium is 600 and the price never goes near it.
    #[test]
    fn a_forecast_justifies_a_buy_the_spot_would_not() {
        let board = vec![q("brine", 24, 20, 500)];
        let galaxy = vec![row("brine", "works-b", 34)];
        let mut l = at("here", 150);
        let fc = eating("works-b", "brine", 500, 600, 13_333, 30, 9_000);

        let spot = decide_trade(&l, &board, &galaxy, &[], &pumps(), &Reach(true));
        assert!(
            !matches!(spot, TradeDecision::Buy { .. }),
            "no forecast, no trade: {spot:?}"
        );

        l.forecast = Some(&fc);
        match decide_trade(&l, &board, &galaxy, &[], &pumps(), &Reach(true)) {
            TradeDecision::Buy {
                sell_target,
                why,
                est_margin,
                units,
                bound,
                ..
            } => {
                assert_eq!(sell_target, "works-b");
                assert!(
                    why.contains("forecast") && why.contains("mid 34→57"),
                    "{why}"
                );
                assert!(why.contains("absent events"), "{why}");
                // 57 less the haircut; 60 units (half the hold) at 24; 50 fuel at 2.
                assert_eq!(units, 60);
                assert_eq!(
                    bound, "hold",
                    "60 is half the hold; cash would have bought 104"
                );
                let unit = 57 - bps(57, SELL_HAIRCUT_BPS);
                assert_eq!(est_margin, unit * 60 - 24 * 60 - 50 * 2);
            }
            other => panic!("the forecast should have carried it: {other:?}"),
        }
    }

    /// A shelf that drains slowly moves the price only as far as it drains by the
    /// time we could sell: a starvation 50,000 ticks out is no forecast at all.
    #[test]
    fn a_shelf_beyond_the_horizon_is_no_forecast() {
        let board = vec![q("brine", 24, 20, 500)];
        let galaxy = vec![row("brine", "works-b", 34)];
        let mut l = at("here", 150);
        let fc = eating("works-b", "brine", 500, 600, 10, 30, 9_000);
        l.forecast = Some(&fc);
        let d = decide_trade(&l, &board, &galaxy, &[], &pumps(), &Reach(true));
        assert!(!matches!(d, TradeDecision::Buy { .. }), "{d:?}");
    }

    /// The berth's dock fee is part of the run's cost: a run that clears the floor by
    /// less than the fee does not clear it.
    #[test]
    fn a_dock_fee_is_charged_against_the_run() {
        let board = vec![q("brine", 24, 20, 500)];
        let galaxy = vec![row("brine", "works-b", 34)];
        let mut l = at("here", 150);
        let mut fc = eating("works-b", "brine", 500, 600, 13_333, 30, 9_000);
        l.forecast = Some(&fc);
        let before = match decide_trade(&l, &board, &galaxy, &[], &pumps(), &Reach(true)) {
            TradeDecision::Buy { est_margin, .. } => est_margin,
            other => panic!("{other:?}"),
        };
        fc.pricing.dock.insert("works-b".into(), 12);
        let l2 = Ledger {
            forecast: Some(&fc),
            ..at("here", 150)
        };
        match decide_trade(&l2, &board, &galaxy, &[], &pumps(), &Reach(true)) {
            TradeDecision::Buy { est_margin, .. } => assert_eq!(est_margin, before - 12),
            other => panic!("{other:?}"),
        }
    }

    /// The fleet works together: works-b is starving and the run pays — unless a
    /// sister ship already has 500 brine bound for works-b, in which case the shelf
    /// will not be empty when we arrive, the mid barely moves, and this ship stays
    /// out of its sister's trade (Ian, 2026-09-09).
    #[test]
    fn a_sister_ships_inbound_cargo_keeps_the_second_ship_out_of_the_same_run() {
        let board = vec![q("brine", 24, 20, 500)];
        let galaxy = vec![row("brine", "works-b", 34)];
        let mut l = at("here", 150);
        let alone = eating("works-b", "brine", 500, 600, 13_333, 30, 9_000);
        let mut with_sister = alone.clone();
        with_sister
            .inbound
            .insert(("works-b".into(), "brine".into()), 500);
        l.forecast = Some(&alone);
        assert!(matches!(
            decide_trade(&l, &board, &galaxy, &[], &pumps(), &Reach(true)),
            TradeDecision::Buy { .. }
        ));
        l.forecast = Some(&with_sister);
        let d = decide_trade(&l, &board, &galaxy, &[], &pumps(), &Reach(true));
        assert!(
            !matches!(d, TradeDecision::Buy { .. }),
            "the sister's cargo fills it: {d:?}"
        );
        let p = with_sister.project("works-b", "brine", 288).unwrap();
        assert!(p.stock_then.0 >= 500, "{p:?}");
    }

    /// Finding 2: the best FORECAST net wins, not the best spot mid. `higher-spot`
    /// pays 40 today and always will; `starving-works` pays 35 today and 60 when
    /// we arrive. The old scan took the first spot-sorted survivor and never
    /// compared them.
    #[test]
    fn a_lower_spot_with_the_higher_forecast_wins_the_target() {
        let board = vec![q("catnip", 25, 20, 500)];
        let galaxy = vec![
            row("catnip", "higher-spot", 40),
            row("catnip", "starving-works", 35),
        ];
        let mut l = at("here", 150);
        let fc = eating("starving-works", "catnip", 600, 600, 20_000, 35, 9_000);
        l.forecast = Some(&fc);
        let chart = Chart {
            priced: vec![("higher-spot", 50), ("starving-works", 50)],
            asked: Default::default(),
        };
        match decide_trade(&l, &board, &galaxy, &[], &pumps(), &chart) {
            TradeDecision::Buy {
                sell_target, why, ..
            } => {
                assert_eq!(sell_target, "starving-works", "{why}");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            chart.asked.borrow().as_slice(),
            ["higher-spot", "starving-works"],
            "both were priced, dearest spot first, inside the route budget"
        );
    }

    /// Finding 3: decay is charged on the NEW position. The same brine run, but
    /// brine loses 90% a day and the hold clock is a day: six units land, and the
    /// forecast that made the run attractive cannot pay for what rotted.
    #[test]
    fn enough_decay_turns_a_forecast_buy_idle() {
        let board = vec![q("brine", 24, 20, 500)];
        let galaxy = vec![row("brine", "works-b", 34)];
        let mut l = at("here", 150);
        let fc = eating("works-b", "brine", 500, 600, 13_333, 30, 9_000);
        let mut decay = BTreeMap::new();
        decay.insert("brine".to_string(), 9_000);
        l.forecast = Some(&fc);
        l.decay_bps = Some(&decay);
        let d = decide_trade(&l, &board, &galaxy, &[], &pumps(), &Reach(true));
        assert!(matches!(d, TradeDecision::Idle { .. }), "{d:?}");
    }

    /// Finding 3: a glut here changes the preferred lift. Two goods with identical
    /// economics; `here` MAKES tuna and its shelf fills inside the horizon. Tuna
    /// is lifted first, and the reason says why.
    #[test]
    fn a_glut_here_breaks_the_tie_toward_lifting_it() {
        let board = vec![q("bream", 20, 15, 500), q("tuna", 20, 15, 500)];
        let galaxy = vec![row("bream", "far", 40), row("tuna", "far", 40)];
        let mut l = at("here", 150);
        let mut fc = Forecast {
            horizon_ticks: 384,
            ..Default::default()
        };
        fc.flows.push(chain::Flow {
            station: "here".into(),
            good: "tuna".into(),
            kind: FlowKind::Makes,
            rate_per_kilotick: 10_000,
            shelf: Some(chain::Shelf {
                station: "here".into(),
                good: "tuna".into(),
                stock: 900,
                capacity: 1_000,
                equilibrium: 600,
            }),
            horizon_ticks: Some(10),
        });
        l.forecast = Some(&fc);
        match decide_trade(&l, &board, &galaxy, &[], &pumps(), &Reach(true)) {
            TradeDecision::Buy { good, why, .. } => {
                assert_eq!(good, "tuna");
                assert!(why.contains("before its shelf fills in 10t"), "{why}");
            }
            other => panic!("{other:?}"),
        }
        // Without the glut, the tie falls to the alphabet — bream — deterministically.
        l.forecast = None;
        match decide_trade(&l, &board, &galaxy, &[], &pumps(), &Reach(true)) {
            TradeDecision::Buy { good, .. } => assert_eq!(good, "bream"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_reason_is_bounded_and_the_opened_event_carries_it() {
        let long = "x".repeat(1_000);
        assert_eq!(bounded_why(&long).chars().count(), WHY_MAX_CHARS);
        let h = held("brine", 60, 23, 100, 388);
        let ev = position_opened(1, 100, &h, 1_280, "forecast: works-b eats brine", "hold");
        assert_eq!(ev["bound"], "hold");
        assert_eq!(ev["event"], "position-opened");
        assert_eq!(ev["why"], "forecast: works-b eats brine");
        assert_eq!(ev["units"], 60);
    }

    /// The LOCAL soak's lesson (t73323): a lot bound for a starving works is not sold
    /// here for a bid that beats the target's SPOT, when the target's FORECAST beats
    /// the bid. Sell side and buy side read one forecast.
    #[test]
    fn a_lot_bound_for_a_starving_works_is_not_sold_here_at_spot() {
        // Held 32 gravy-base at 15; here bids 19. velvet-array's spot mid is 23 (18
        // after the haircut — worse than 19 here), but its shelf drains to empty by
        // the hold clock and the mid heads to 38 (31 after the haircut).
        let hold = vec![held("gravy-base", 32, 15, 100, 100)];
        let board = vec![q("gravy-base", 20, 19, 500)];
        let galaxy = vec![row("gravy-base", "velvet-array", 23)];
        let mut l = at("foxys-diner", 150);
        let fc = eating("velvet-array", "gravy-base", 233, 600, 13_333, 20, 9_000);
        // A short flight, as the LOCAL router priced it (4 ticks): the lot could be
        // sold on arrival, or held there while the shelf drains (17 ticks).
        struct Near;
        impl Router for Near {
            fn fuel_between(&self, _: &str, _: &str) -> Option<i64> {
                Some(50)
            }
            fn leg_distances_km(&self, _: &str, _: &str) -> Option<Vec<i64>> {
                Some(vec![1])
            }
        }
        // Without the forecast: the spot forward (18) loses to 19 here — sell.
        let d = decide_trade(&l, &board, &galaxy, &hold, &pumps(), &Near);
        assert!(matches!(d, TradeDecision::Sell { .. }), "{d:?}");
        // With it: the lot is worth more where it is going — hold.
        l.forecast = Some(&fc);
        let d = decide_trade(&l, &board, &galaxy, &hold, &pumps(), &Near);
        assert!(
            !matches!(d, TradeDecision::Sell { .. }),
            "sold what the forecast wanted kept: {d:?}"
        );
    }

    #[test]
    fn sells_a_holding_when_the_bid_clears_basis_plus_margin() {
        let hold = vec![held("catnip", 40, 30, 100, 100)];
        // bid 40 vs basis 30 (+12% = 33.6): clears.
        let board = vec![q("catnip", 42, 40, 500)];
        let d = decide_trade(
            &at("foxys-diner", 150),
            &board,
            &[],
            &hold,
            &pumps(),
            &Reach(true),
        );
        assert!(
            matches!(d, TradeDecision::Sell { good, units, .. } if good == "catnip" && units == 40)
        );
    }

    #[test]
    fn never_sells_before_the_exchanges_clock() {
        // LOCAL t11415: "rejected: minimum hold (sellable at tick 11655)". A bid that
        // clears is still not a sale until the clock — and no buy stacks on top.
        let hold = vec![held("litter-clay", 60, 13, 11367, 11655)];
        let board = vec![q("litter-clay", 18, 40, 0)];
        // The far berth pays WORSE than this counter, so the sell rule has no reason
        // to carry and the only thing standing between the lot and a sale is the
        // clock — which is what this test is about.
        let galaxy = vec![row("litter-clay", "elsewhere", 20)];
        let d = decide_trade(
            &at("io-slagworks", 11415),
            &board,
            &galaxy,
            &hold,
            &pumps(),
            &Reach(true),
        );
        match d {
            TradeDecision::Idle { why } => assert!(why.contains("sellable at t11655"), "{why}"),
            other => panic!("expected Idle until the clock, got {other:?}"),
        }
        // The clock passed: the same bid sells.
        let d = decide_trade(
            &at("io-slagworks", 11655),
            &board,
            &galaxy,
            &hold,
            &pumps(),
            &Reach(true),
        );
        assert!(matches!(d, TradeDecision::Sell { .. }));
    }

    #[test]
    fn a_lot_with_an_unknown_clock_is_offered_not_frozen() {
        // Adopted cargo (sellable_at 0): the exchange knows when it was bought and we
        // do not, so we offer it — a bid that clears is taken, and an early sell is
        // refused for free with the true tick, which then sets the clock.
        let hold = vec![held("ore", 56, 10, 11417, 0)];
        let board = vec![q("ore", 20, 18, 2977)];
        let d = decide_trade(
            &at("io-slagworks", 11469),
            &board,
            &[],
            &hold,
            &pumps(),
            &Reach(true),
        );
        assert!(matches!(d, TradeDecision::Sell { .. }), "{d:?}");
        // And a lot taken up this fold is not "carried too long": under water, it is
        // held rather than dumped.
        let fresh = vec![held("ore", 56, 50, 11460, 0)];
        let d = decide_trade(
            &at("io-slagworks", 11469),
            &board,
            &[],
            &fresh,
            &pumps(),
            &Reach(true),
        );
        assert!(!matches!(d, TradeDecision::Sell { .. }), "{d:?}");
    }

    #[test]
    fn does_not_sell_at_a_loss_unless_stuck() {
        let hold = vec![held("catnip", 40, 50, 100, 100)];
        let board = vec![q("catnip", 42, 40, 500)]; // bid 40 < basis 50 — a loss
                                                    // Past the clock but not long past, hold not needed: hold it, do not dump.
        let d = decide_trade(
            &at("foxys-diner", 150),
            &board,
            &[],
            &hold,
            &pumps(),
            &Reach(true),
        );
        assert!(!matches!(d, TradeDecision::Sell { .. }));
    }

    /// Ian's ruling, 2026-09-04: "maximizing profits and continuous growth are the
    /// doctrine — if that means taking a loss to gain a more profitable route,
    /// cargo, contract, then that needs to be part of the calculation."
    ///
    /// A lot bought at 140 with the best counter on the map at 48. Under the old
    /// rule it sat until it was declared stuck, because the bid never cleared what
    /// it cost. Cost is spent. The hold is not.
    #[test]
    fn takes_a_loss_rather_than_hold_cargo_no_one_will_pay_more_for() {
        let hold = vec![held("bluefin-reserve", 114, 140, 100, 100)];
        let board = vec![q("bluefin-reserve", 94, 80, 186)];
        // The map is seen, and nowhere on it pays better than the counter here.
        let galaxy = vec![row("bluefin-reserve", "velvet-array", 48)];
        let d = decide_trade(
            &at("foxys-diner", 150),
            &board,
            &galaxy,
            &hold,
            &pumps(),
            &Reach(true),
        );
        match d {
            TradeDecision::Sell { good, why, .. } => {
                assert_eq!(good, "bluefin-reserve");
                assert!(
                    why.contains("basis 140"),
                    "the book still says what it cost: {why}"
                );
            }
            other => panic!("expected the loss to be taken, got {other:?}"),
        }
    }

    /// ...and the same lot is CARRIED, not dumped, when the map says somewhere pays
    /// enough more to beat what the hull costs while it travels.
    #[test]
    fn carries_a_lot_when_a_dearer_berth_beats_the_hurdle() {
        let hold = vec![held("bluefin-reserve", 114, 140, 100, 100)];
        let board = vec![q("bluefin-reserve", 94, 80, 186)];
        let galaxy = vec![row("bluefin-reserve", "tuna-prime", 300)];
        let d = decide_trade(
            &at("foxys-diner", 150),
            &board,
            &galaxy,
            &hold,
            &pumps(),
            &Reach(true),
        );
        assert!(!matches!(d, TradeDecision::Sell { .. }), "{d:?}");
    }

    /// Cargo waiting for a better price is cargo spoiling. Bluefin sheds 23% a day,
    /// so a berth that pays a little more a long way off pays less than it looks.
    #[test]
    fn spoilage_is_charged_against_the_carry() {
        let h = held("bluefin-reserve", 100, 140, 100, 100);
        let mut decay = std::collections::BTreeMap::new();
        decay.insert("bluefin-reserve".to_string(), 2_300_i64);
        let mut l = at("foxys-diner", 150);
        l.decay_bps = Some(&decay);
        assert_eq!(surviving(&h, 0, &l), 100);
        // One world-day out: 77 of the 100 arrive.
        assert_eq!(surviving(&h, 288, &l), 77);
        // Two days: 59.
        assert_eq!(surviving(&h, 576, &l), 59);
        // A good the pack does not rot arrives whole.
        let ore = held("ore", 100, 10, 100, 100);
        assert_eq!(surviving(&ore, 576, &l), 100);
    }

    /// The lease is the floor under the whole calculation: a lot improving slower
    /// than the hull costs to keep is not improving.
    #[test]
    fn the_hurdle_is_what_the_hull_costs_to_keep() {
        let l = at("foxys-diner", 150);
        // 600 a day over 288 ticks.
        assert!((hurdle_per_tick(&l) - 2.083).abs() < 0.01);
    }

    /// The exchange's clock outranks ours, in both directions.
    #[test]
    fn the_exchanges_hold_clock_beats_our_book() {
        let mut book = vec![held("bluefin-reserve", 114, 140, 6048, 8005)];
        let notes = adopt_exchange_clocks(&mut book, &[("bluefin-reserve".to_string(), 6336_i64)]);
        assert_eq!(book[0].sellable_at, 6336);
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("t8005"), "{}", notes[0]);
        assert!(notes[0].contains("t6336"), "{}", notes[0]);
        // Agreeing is silent.
        assert!(adopt_exchange_clocks(&mut book, &[("bluefin-reserve".into(), 6336)]).is_empty());
        // A good the exchange is not tracking says nothing, so we keep what we had.
        assert!(adopt_exchange_clocks(&mut book, &[("ore".into(), 1)]).is_empty());
        assert_eq!(book[0].sellable_at, 6336);
    }

    #[test]
    fn reads_the_hold_clocks_off_the_wire() {
        let me = serde_json::json!({
            "holds": [
                {"good": "bluefin-reserve", "sellableAtTick": 6336},
                {"good": "catnip"}
            ]
        });
        assert_eq!(
            parse_holds(&me),
            vec![("bluefin-reserve".to_string(), 6336)]
        );
        assert!(parse_holds(&serde_json::json!({})).is_empty());
    }

    /// Ian, 2026-09-05: "the automation should include the ability to borrow to
    /// speculate... working capital should be utilized to maximize success."
    ///
    /// Kibble Klipper's own numbers, idle at foxy's-diner: ℳ411 in hand against a
    /// ℳ2,000 floor, and ℳ8,574 of credit line it could not see. Cash alone keeps
    /// it parked; the line put to work lets it trade.
    #[test]
    fn the_credit_line_is_working_capital_when_the_captain_opens_it() {
        let mut l = at("foxys-diner", 150);
        l.credits = 411;
        let board = vec![q("catnip", 20, 40, 500)];
        let galaxy = vec![row("catnip", "velvet-array", 90)];

        // Dial closed: the line is zero here, and the ship stays put.
        l.borrowable = 0;
        assert_eq!(l.working_capital(), 411);
        let d = decide_trade(&l, &board, &galaxy, &[], &pumps(), &Reach(true));
        assert!(!matches!(d, TradeDecision::Buy { .. }), "{d:?}");

        // Dial open: the same berth, the same board, and a position it can carry.
        l.borrowable = 8_574;
        assert_eq!(l.working_capital(), 8_985);
        let d = decide_trade(&l, &board, &galaxy, &[], &pumps(), &Reach(true));
        assert!(matches!(d, TradeDecision::Buy { .. }), "{d:?}");
    }

    #[test]
    fn liquidates_a_stuck_position_even_at_a_loss() {
        let hold = vec![held("catnip", 40, 50, 100, 100)];
        let board = vec![q("catnip", 42, 40, 500)];
        // Three hold-periods after it came into our care (100 + 3*288 = 964): cut it.
        let d = decide_trade(
            &at("foxys-diner", 1000),
            &board,
            &[],
            &hold,
            &pumps(),
            &Reach(true),
        );
        assert!(matches!(d, TradeDecision::Sell { .. }));
    }

    #[test]
    fn liquidates_when_freight_needs_the_hold() {
        let hold = vec![held("catnip", 40, 50, 100, 100)];
        let board = vec![q("catnip", 42, 40, 500)];
        let mut l = at("foxys-diner", 150);
        l.spare_hold = 0;
        l.need_hold = true;
        let d = decide_trade(&l, &board, &[], &hold, &pumps(), &Reach(true));
        assert!(matches!(d, TradeDecision::Sell { .. }));
    }

    #[test]
    fn one_position_at_a_time() {
        // LOCAL t11417–11423: ore bought four folds running (30, 15, 7, 4 units),
        // each re-arming the clock and halving the spare hold. Holding anything
        // means no new buy, however good the board looks.
        let hold = vec![held("ore", 30, 11, 11417, 11705)];
        let board = vec![q("ore", 10, 8, 2977), q("catnip", 30, 28, 500)];
        let galaxy = vec![row("ore", "far", 40), row("catnip", "far", 90)];
        let d = decide_trade(
            &at("io-slagworks", 11419),
            &board,
            &galaxy,
            &hold,
            &pumps(),
            &Reach(true),
        );
        assert!(matches!(d, TradeDecision::Idle { .. }), "{d:?}");
    }

    #[test]
    fn buys_a_profitable_reachable_arbitrage() {
        // catnip asks 30 here; sells (mid 60, -18% haircut = 49) at whisker-hollow.
        // margin 20/unit > 20% of ask (6): buy. 60 units × 20 = 1200 − fuel 50×2: clears.
        let board = vec![q("catnip", 30, 28, 500)];
        let galaxy = vec![
            row("catnip", "here", 29),
            row("catnip", "whisker-hollow", 60),
        ];
        let d = decide_trade(
            &at("here", 100),
            &board,
            &galaxy,
            &[],
            &pumps(),
            &Reach(true),
        );
        match d {
            TradeDecision::Buy {
                good,
                sell_target,
                units,
                est_margin,
                ..
            } => {
                assert_eq!(good, "catnip");
                assert_eq!(sell_target, "whisker-hollow");
                assert!(units > 0);
                assert_eq!(est_margin, 20 * units - 100);
            }
            other => panic!("expected Buy, got {other:?}"),
        }
    }

    #[test]
    fn a_run_must_clear_its_carry_fuel_and_the_total_floor() {
        // The litter-clay run: 60 units at 12, target mid 16 → proceeds 13, margin 1/unit
        // (= 8% < 20%): refused on the unit rule already. Make the unit rule pass but
        // the total fail: 10 units of margin 20 = 200, minus carry 50 × 2 = 100 → 100 <
        // 150 floor. Pass.
        let board = vec![GoodQuote {
            good: "gravy".into(),
            ask: 30,
            bid: 28,
            stock: 10,
            max_buy: 10,
            max_sell: 100,
        }];
        let galaxy = vec![row("gravy", "far", 61)]; // 61 − 18% = 50; margin 20
        let d = decide_trade(
            &at("here", 100),
            &board,
            &galaxy,
            &[],
            &pumps(),
            &Reach(true),
        );
        match d {
            TradeDecision::Idle { why } => assert!(why.contains("too small"), "{why}"),
            other => panic!("expected Idle, got {other:?}"),
        }
        // Same run with fuel free (a pump-to-pump hop the freight would fly anyway
        // costs 0 here): 200 ≥ 150, buy.
        let mut l = at("here", 100);
        l.fuel_price = 0;
        let d = decide_trade(&l, &board, &galaxy, &[], &pumps(), &Reach(true));
        assert!(matches!(d, TradeDecision::Buy { .. }), "{d:?}");
    }

    #[test]
    fn refuses_arbitrage_to_an_unreachable_buyer() {
        let board = vec![q("catnip", 30, 28, 500)];
        let galaxy = vec![row("catnip", "whisker-hollow", 60)];
        let d = decide_trade(
            &at("here", 100),
            &board,
            &galaxy,
            &[],
            &pumps(),
            &Reach(false),
        );
        assert!(matches!(d, TradeDecision::Idle { .. }));
    }

    #[test]
    fn refuses_a_thin_spread_that_would_not_clear_friction() {
        // mid 33 target, -18% = 27; ask 30 → negative margin. No trade.
        let board = vec![q("catnip", 30, 28, 500)];
        let galaxy = vec![row("catnip", "whisker-hollow", 33)];
        let d = decide_trade(
            &at("here", 100),
            &board,
            &galaxy,
            &[],
            &pumps(),
            &Reach(true),
        );
        assert!(matches!(d, TradeDecision::Idle { .. }));
    }

    #[test]
    fn will_not_open_a_position_below_the_cash_floor() {
        let board = vec![q("catnip", 30, 28, 500)];
        let galaxy = vec![row("catnip", "whisker-hollow", 60)];
        let mut l = at("here", 100);
        l.credits = 1500;
        let d = decide_trade(&l, &board, &galaxy, &[], &pumps(), &Reach(true));
        assert!(matches!(d, TradeDecision::Idle { .. }));
    }

    #[test]
    fn position_size_is_bounded_by_cash_and_hold() {
        let board = vec![GoodQuote {
            good: "catnip".into(),
            ask: 30,
            bid: 28,
            stock: 100000,
            max_buy: 100000,
            max_sell: 1000,
        }];
        let galaxy = vec![row("catnip", "whisker-hollow", 60)];
        // 25% of 10_000 / 30 = 83 by cash; 50% of 120 = 60 by hold → 60 wins.
        let d = decide_trade(
            &at("here", 100),
            &board,
            &galaxy,
            &[],
            &pumps(),
            &Reach(true),
        );
        match d {
            TradeDecision::Buy { units, .. } => assert_eq!(units, 60),
            other => panic!("expected bounded Buy, got {other:?}"),
        }
    }

    /// Every buyer inside the route budget is priced and the best NET wins — not
    /// the first that answered (codex T-238 finding 2). The budget still binds:
    /// five buyers on the board, four questions, the fifth is never asked.
    #[test]
    fn prices_every_buyer_inside_the_route_budget_and_takes_the_best_net() {
        let board = vec![q("catnip", 30, 28, 500)];
        let galaxy = vec![
            row("catnip", "far-side", 90),
            row("catnip", "whisker-hollow", 70),
            row("catnip", "foxys-diner", 60),
            row("catnip", "titania", 55),
            row("catnip", "enceladus", 50),
        ];
        let chart = Chart {
            priced: vec![
                ("whisker-hollow", 40),
                ("foxys-diner", 20),
                ("enceladus", 1),
            ],
            asked: Default::default(),
        };
        let d = decide_trade(&at("here", 100), &board, &galaxy, &[], &pumps(), &chart);
        match d {
            TradeDecision::Buy { sell_target, .. } => assert_eq!(sell_target, "whisker-hollow"),
            other => panic!("expected Buy, got {other:?}"),
        }
        assert_eq!(
            *chart.asked.borrow(),
            vec![
                "far-side".to_string(),
                "whisker-hollow".to_string(),
                "foxys-diner".to_string(),
                "titania".to_string()
            ],
            "asked in pay order, all four inside the budget, never the fifth"
        );
    }

    #[test]
    fn does_not_ask_the_router_about_buyers_that_could_not_clear_the_margin() {
        let board = vec![q("catnip", 30, 28, 500)];
        // mid 33 → proceeds 27 < ask: no candidate clears, so no route is ever priced.
        let galaxy = vec![
            row("catnip", "whisker-hollow", 33),
            row("catnip", "foxys-diner", 31),
        ];
        let chart = Chart {
            priced: vec![("whisker-hollow", 40)],
            asked: Default::default(),
        };
        let d = decide_trade(&at("here", 100), &board, &galaxy, &[], &pumps(), &chart);
        assert!(matches!(d, TradeDecision::Idle { .. }));
        assert!(
            chart.asked.borrow().is_empty(),
            "priced a route that could not pay"
        );
    }

    #[test]
    fn will_not_buy_what_it_cannot_fuel_the_carry_for() {
        let board = vec![q("catnip", 30, 28, 500)];
        let galaxy = vec![row("catnip", "whisker-hollow", 60)];
        // Leg costs 50; 55 in the tank is under the 20% reserve (60). Ballast — pass.
        let mut l = at("here", 100);
        l.fuel_available = 55;
        let d = decide_trade(&l, &board, &galaxy, &[], &pumps(), &Reach(true));
        assert!(matches!(d, TradeDecision::Idle { .. }));
        // With the reserve met, the same run is a trade.
        l.fuel_available = 60;
        let d = decide_trade(&l, &board, &galaxy, &[], &pumps(), &Reach(true));
        assert!(matches!(d, TradeDecision::Buy { .. }));
    }

    #[test]
    fn the_hold_is_the_truth_refused_sells_restore_and_strangers_are_adopted() {
        // LOCAL t11416: the book had sold 60 litter-clay; the fold refused; the hold
        // still had them. And 30 ore aboard the book never heard of (a crash between
        // ack and save). Contract freight is never in this list.
        let mut book: Vec<Holding> = vec![]; // the sold-out book
        let cargo = vec![("litter-clay".to_string(), 60), ("ore".to_string(), 30)];
        let hint = |g: &str| {
            if g == "ore" {
                (11, "io-slagworks".to_string())
            } else {
                (99, "far".to_string())
            }
        };
        let notes = reconcile_hold(&mut book, &cargo, &hint, 11416);
        assert_eq!(book.len(), 2, "{book:?}");
        let clay = book.iter().find(|h| h.good == "litter-clay").unwrap();
        // Adopted: ours to carry from now, with the clock left to the exchange.
        assert_eq!((clay.units, clay.avg_cost, clay.sellable_at), (60, 99, 0));
        assert_eq!(
            clay.sell_target, "far",
            "an adopted lot needs somewhere to go"
        );
        let ore = book.iter().find(|h| h.good == "ore").unwrap();
        assert_eq!((ore.units, ore.avg_cost), (30, 11));
        assert_eq!(notes.len(), 2);

        // A partial fill reduces; an empty hold drops.
        let mut book = vec![held("ore", 30, 11, 1, 1)];
        reconcile_hold(&mut book, &[("ore".into(), 12)], &hint, 5);
        assert_eq!(book[0].units, 12);
        reconcile_hold(&mut book, &[], &hint, 6);
        assert!(book.is_empty());
    }

    #[test]
    fn a_target_that_did_not_pay_is_replaced_by_one_that_still_would() {
        let mut h = held("gravy-base", 10, 16, 1, 1);
        h.sell_target = "velvet-array".into();
        // velvet's mid fell to 19 (−18% = 15 < 18 floor); tranquility still pays 25.
        let galaxy = vec![
            row("gravy-base", "velvet-array", 19),
            row("gravy-base", "tranquility", 25),
        ];
        let note = retarget(&mut h, "velvet-array", &galaxy);
        assert_eq!(h.sell_target, "tranquility");
        assert!(note.unwrap().contains("tranquility"));
        // Berthed at the dearest counter, the lot is re-aimed at the next dearest —
        // even one paying less than the lot cost. Where it is worth MOST is the only
        // question a target answers; whether the trip is worth making at all is the
        // sell rule's business, and it asks that fresh at every berth.
        let galaxy = vec![
            row("gravy-base", "velvet-array", 19),
            row("gravy-base", "tranquility", 20),
        ];
        let note = retarget(&mut h, "tranquility", &galaxy);
        assert_eq!(h.sell_target, "velvet-array");
        assert!(note.is_some());
        // Unchanged target: no note.
        assert!(retarget(&mut h, "tranquility", &galaxy).is_none());
        // Nothing on the map counters the good at all: no target, ride under freight.
        let note = retarget(&mut h, "tranquility", &[]);
        assert_eq!(h.sell_target, "");
        assert!(note.is_some());
    }

    #[test]
    fn reads_the_clock_out_of_the_refusal_and_the_basis_out_of_the_receipt() {
        assert_eq!(
            sellable_tick_from_refusal("rejected: minimum hold (sellable at tick 11655)"),
            Some(11655)
        );
        assert_eq!(
            sellable_tick_from_refusal("rejected: insufficient credits"),
            None
        );
        // 60 units for 737 total (723 + 14 tax): 12.28 → 13.
        assert_eq!(basis_from_total(737, 60, 12), 13);
        // Never below the ask, and a nonsense receipt falls back to it.
        assert_eq!(basis_from_total(100, 60, 12), 12);
        assert_eq!(basis_from_total(0, 60, 12), 12);
    }

    #[test]
    fn galaxy_decodes_both_the_bare_array_and_the_surveyed_object() {
        let bare = serde_json::json!([{"good": "catnip", "station": "a", "mid": 10, "stock": 5}]);
        let wrapped = serde_json::json!({"rows": [{"good": "catnip", "station": "a", "mid": 10, "stock": 5}],
                                         "unsurveyed": ["b"]});
        for v in [bare, wrapped] {
            let rows = parse_galaxy(&v);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].station, "a");
            assert_eq!(rows[0].mid, 10);
        }
        assert!(parse_galaxy(&serde_json::json!({"error": "x"})).is_empty());
    }

    #[test]
    fn a_rumour_berth_is_an_empty_board_not_an_error() {
        let v = serde_json::json!({"station": "x", "known": "rumour", "goods": [],
                                   "survey": {"station": "x", "nextTierName": "sighted"}});
        assert!(parse_board(&v).is_empty());
        let v = serde_json::json!({"goods": [{"good": "catnip", "ask": 13, "bid": 11, "stock": 640,
                                              "maxBuyUnits": 600, "maxSellUnits": 600}]});
        let b = parse_board(&v);
        assert_eq!(
            (b[0].ask, b[0].bid, b[0].max_buy, b[0].max_sell),
            (13, 11, 600, 600)
        );
        let me = serde_json::json!({"cargo": [{"good": "ore", "units": 30}]});
        assert_eq!(parse_cargo(&me), vec![("ore".to_string(), 30)]);
    }
}
