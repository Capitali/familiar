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
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
        let unit = r.mid - bps(r.mid, SELL_HAIRCUT_BPS);
        let ticks = router
            .leg_distances_km(here, &r.station)
            .map(|legs| {
                crate::doctrine::flight_ticks(&legs, crate::doctrine::REFERENCE_ACCEL_MILLI_G)
            })
            .unwrap_or(l.min_hold.max(1))
            .max(1);
        // Arrive with less than we left with. The hold is not a vault.
        let net = unit * surviving(h, ticks, l) - fuel * l.fuel_price.max(0);
        if best.as_ref().map(|b| net > b.net).unwrap_or(true) {
            best = Some(Forward {
                station: r.station.clone(),
                net,
                ticks,
            });
        }
    }
    best
}

/// The units of a lot still there after `ticks` of carrying it, at the pack's own
/// daily decay. Rounded DOWN, because a merchant who rounds spoilage up is lying
/// to himself about his own hold.
pub fn surviving(h: &Holding, ticks: i64, l: &Ledger) -> i64 {
    let decay = l
        .decay_bps
        .and_then(|d| d.get(&h.good).copied())
        .unwrap_or(0);
    if decay <= 0 || ticks <= 0 {
        return h.units;
    }
    let days = ticks as f64 / l.ticks_per_day.max(1) as f64;
    let kept = (1.0 - decay as f64 / 10_000.0).max(0.0).powf(days);
    ((h.units as f64) * kept).floor() as i64
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

/// Load the speculative book from the ship store (`holdings.json`). Absent or
/// unreadable is an empty book — a merchant that cannot read its own ledger owns
/// nothing, which is the safe reading.
pub fn load_holdings(ship_dir: &Path) -> Vec<Holding> {
    std::fs::read_to_string(ship_dir.join("holdings.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Persist the book, dropping any zeroed-out position on the way.
pub fn save_holdings(ship_dir: &Path, holdings: &[Holding]) {
    let live: Vec<&Holding> = holdings.iter().filter(|h| h.units > 0).collect();
    if let Ok(bytes) = serde_json::to_vec_pretty(&live) {
        let _ = std::fs::write(ship_dir.join("holdings.json"), bytes);
    }
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
    if l.need_hold || l.spare_hold <= 0 || l.credits < MIN_CASH_FLOOR {
        return TradeDecision::Idle {
            why: "no room/cash to open a position".into(),
        };
    }

    // The best good sold here, by estimated total margin to its best reachable buyer.
    let mut best: Option<(i64, TradeDecision)> = None;
    let mut priced = 0; // candidate buyers the router could price
    let mut unfuelable = 0; // ...of which the tank could not reach
    let mut too_small = 0; // ...runs whose total margin did not clear fuel + floor
    for q in board {
        // Buyable here: in stock, has an ask, and a positive shelf.
        if q.stock <= 0 || q.ask <= 0 || q.max_buy <= 0 {
            continue;
        }
        // Where does this good fetch the most? Candidates best-paying first; the
        // router is asked only until one is reachable AND fuelable, and only for
        // candidates whose mid could clear the margin at all — every question costs a
        // route call on the wire.
        let mut candidates: Vec<&MarketRow> = galaxy
            .iter()
            .filter(|r| r.good == q.good && r.station != here && r.mid > 0)
            .collect();
        candidates.sort_by_key(|r| std::cmp::Reverse(r.mid));
        let mut best_target: Option<(i64, &str, i64)> = None; // (mid, station, fuel)
        for row in candidates {
            let est_proceeds = row.mid - bps(row.mid, SELL_HAIRCUT_BPS);
            if est_proceeds - q.ask < bps(q.ask, BUY_MARGIN_BPS).max(1) {
                break; // sorted: nothing below this mid clears either
            }
            let Some(cost) = router.fuel_between(here, &row.station) else {
                continue; // unreachable / unpriceable — not an arbitrage
            };
            // ...plus the leg from that market to a pump, or the run ends there.
            let cost = cost + crate::doctrine::onward_to_pump(&row.station, pumps, router);
            priced += 1;
            if !carry_affordable(cost, l.fuel_available) {
                unfuelable += 1;
                continue; // a buyer we cannot fly to is ballast
            }
            best_target = Some((row.mid, &row.station, cost));
            break;
        }
        let Some((target_mid, target, carry_fuel)) = best_target else {
            continue;
        };
        // Conservative per-unit economics: pay the real ask; expect the target mid less
        // the haircut.
        let est_proceeds = target_mid - bps(target_mid, SELL_HAIRCUT_BPS);
        let per_unit_margin = est_proceeds - q.ask;
        if per_unit_margin <= 0 || per_unit_margin < bps(q.ask, BUY_MARGIN_BPS) {
            continue;
        }
        // Size the position: bounded by cash, spare hold, and the shelf.
        let by_cash = bps(l.credits, MAX_CASH_BPS) / q.ask.max(1);
        let by_hold = bps(l.spare_hold, MAX_HOLD_BPS).max(0);
        let units = by_cash.min(by_hold).min(q.max_buy);
        if units <= 0 {
            continue;
        }
        // The run, whole: the carry's fuel is the trade's cost even when a haul ends
        // up paying for the miles — the litter-clay lesson.
        let total_margin = per_unit_margin * units - carry_fuel * l.fuel_price.max(0);
        if total_margin < MIN_TOTAL_MARGIN {
            too_small += 1;
            continue;
        }
        // Prefer a pump-adjacent buyer, all else equal, so the run refuels itself.
        let score = total_margin + if pumps.contains(target) { 1 } else { 0 };
        if best.as_ref().map(|(m, _)| score > *m).unwrap_or(true) {
            best = Some((
                score,
                TradeDecision::Buy {
                    good: q.good.clone(),
                    units,
                    sell_target: target.to_string(),
                    est_margin: total_margin,
                },
            ));
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
        }
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

    #[test]
    fn takes_the_best_reachable_buyer_and_asks_the_router_lazily() {
        let board = vec![q("catnip", 30, 28, 500)];
        // Three buyers by mid: 90 (off the chart), 70 (priced), 60 (priced).
        let galaxy = vec![
            row("catnip", "far-side", 90),
            row("catnip", "whisker-hollow", 70),
            row("catnip", "foxys-diner", 60),
        ];
        let chart = Chart {
            priced: vec![("whisker-hollow", 40), ("foxys-diner", 20)],
            asked: Default::default(),
        };
        let d = decide_trade(&at("here", 100), &board, &galaxy, &[], &pumps(), &chart);
        match d {
            TradeDecision::Buy { sell_target, .. } => assert_eq!(sell_target, "whisker-hollow"),
            other => panic!("expected Buy, got {other:?}"),
        }
        // Asked in pay order and stopped at the first that answered: never the third.
        assert_eq!(
            *chart.asked.borrow(),
            vec!["far-side".to_string(), "whisker-hollow".to_string()]
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
