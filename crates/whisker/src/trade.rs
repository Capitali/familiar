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
//! - **Sell only at a real profit — or to cut a stuck position.** Bid must clear
//!   basis by a margin; liquidation only well past the clock.
//! - **Buy conservatively.** Real ask in; target MID minus a haircut (spread, tax,
//!   drift) out; margin floors per unit AND in total, and the total must clear the
//!   fuel a dedicated carry would burn — the litter-clay run cleared its unit margin
//!   and still lost ℳ to fuel.
//! - **Small positions; never below the cash floor; only reachable, fuelable
//!   buyers**, asked of the router lazily best-payer-first (each question is a
//!   `/v1/route` call on the ship's one rate-limited key).

use std::collections::BTreeSet;
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

/// Can a leg costing `cost` fuel be flown on `fuel` in the tank, reserve included?
pub fn carry_affordable(cost: i64, fuel: i64) -> bool {
    fuel >= bps(cost, CARRY_RESERVE_BPS)
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

/// Bring the book to what the hold actually holds. `cargo` is `/v1/me.cargo`;
/// `freight` is the active contract's cargo when it is aboard (its good and units
/// are not ours to sell). `basis_hint(good)` prices a good we find aboard but do not
/// remember — the ask here if quoted, else the dearest mid on the map, so an
/// adopted lot is never sold at a phantom profit. Returns a note per change.
pub fn reconcile_hold(
    holdings: &mut Vec<Holding>,
    cargo: &[(String, i64)],
    freight: &[(&str, i64)],
    basis_hint: &dyn Fn(&str) -> i64,
    tick: i64,
    min_hold: i64,
) -> Vec<String> {
    let mut notes = Vec::new();
    let ours = |good: &str| -> i64 {
        let aboard = cargo
            .iter()
            .filter(|(g, _)| g == good)
            .map(|(_, u)| *u)
            .sum::<i64>();
        let theirs: i64 = freight
            .iter()
            .filter(|(g, _)| *g == good)
            .map(|(_, u)| *u)
            .sum();
        (aboard - theirs).max(0)
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
        let basis = basis_hint(good);
        notes.push(format!(
            "{good}: {units} units aboard the book did not know — adopted at basis {basis}"
        ));
        holdings.push(Holding {
            good: good.clone(),
            units,
            avg_cost: basis,
            sell_target: String::new(),
            opened_tick: tick,
            sellable_at: tick + min_hold,
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
        // A lot from before the clock was kept (sellable_at 0) is assumed held from
        // the day it was opened; the exchange's refusal text corrects either way.
        let sellable_at = if h.sellable_at > 0 {
            h.sellable_at
        } else {
            h.opened_tick + l.min_hold
        };
        if l.tick < sellable_at {
            waiting = Some(format!("{} sellable at t{}", h.good, sellable_at));
            continue;
        }
        let Some(q) = board.iter().find(|q| q.good == h.good) else {
            continue;
        };
        let sellable = h.units.min(q.max_sell.max(0));
        if sellable <= 0 {
            continue;
        }
        let profit_floor = h.avg_cost + bps(h.avg_cost, SELL_MARGIN_BPS);
        let stuck = l.tick > sellable_at + STUCK_HOLDS * l.min_hold.max(1) || l.need_hold;
        if q.bid >= profit_floor {
            return TradeDecision::Sell {
                good: h.good.clone(),
                units: sellable,
                why: format!(
                    "bid {} clears basis {} (+margin) at {here}",
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
        let galaxy = vec![row("litter-clay", "elsewhere", 90)];
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
    fn a_lot_without_a_clock_is_held_a_day_from_its_opening() {
        // holdings.json written before the clock existed: sellable_at 0. Not "ancient
        // and stuck" — a day from opened_tick, like any other buy.
        let hold = vec![held("ore", 56, 10, 11417, 0)];
        let board = vec![q("ore", 10, 8, 2977)];
        let d = decide_trade(
            &at("io-slagworks", 11469),
            &board,
            &[],
            &hold,
            &pumps(),
            &Reach(true),
        );
        match d {
            TradeDecision::Idle { why } => assert!(why.contains("sellable at t11705"), "{why}"),
            other => panic!("expected Idle, got {other:?}"),
        }
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

    #[test]
    fn liquidates_a_stuck_position_even_at_a_loss() {
        let hold = vec![held("catnip", 40, 50, 100, 100)];
        let board = vec![q("catnip", 42, 40, 500)];
        // Two hold-periods past the clock (100 + 2*288 = 676): cut it.
        let d = decide_trade(
            &at("foxys-diner", 700),
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
        // ack and save), plus 24 units of freight cargo that are NOT ours.
        let mut book: Vec<Holding> = vec![]; // the sold-out book
        let cargo = vec![
            ("litter-clay".to_string(), 60),
            ("ore".to_string(), 30),
            ("grain".to_string(), 24),
        ];
        let hint = |g: &str| if g == "ore" { 11 } else { 99 };
        let notes = reconcile_hold(&mut book, &cargo, &[("grain", 24)], &hint, 11416, 288);
        assert_eq!(book.len(), 2, "{book:?}");
        let clay = book.iter().find(|h| h.good == "litter-clay").unwrap();
        assert_eq!(
            (clay.units, clay.avg_cost, clay.sellable_at),
            (60, 99, 11416 + 288)
        );
        let ore = book.iter().find(|h| h.good == "ore").unwrap();
        assert_eq!((ore.units, ore.avg_cost), (30, 11));
        assert!(
            book.iter().all(|h| h.good != "grain"),
            "freight cargo adopted as ours"
        );
        assert_eq!(notes.len(), 2);

        // A partial fill reduces; an empty hold drops.
        let mut book = vec![held("ore", 30, 11, 1, 1)];
        reconcile_hold(&mut book, &[("ore".into(), 12)], &[], &hint, 5, 288);
        assert_eq!(book[0].units, 12);
        reconcile_hold(&mut book, &[], &[], &hint, 6, 288);
        assert!(book.is_empty());
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
