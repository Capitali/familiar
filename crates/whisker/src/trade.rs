//! The merchant doctrine, pure. Buy a good where it is cheap, carry it, sell it
//! where it is dear — arbitrage across the moving map, the trader's complement to
//! the freight hauler (Ian, 2026-09-01: "I want KK II to trade as well as haul").
//!
//! No socket, no clock, no store — market facts + what we hold, in; one
//! [`TradeDecision`], out. Every money rule is pinned by a test, because a trader
//! that is wrong about profit loses ℳ silently where a hauler that is wrong merely
//! sits still.
//!
//! The rules, and the caution behind each:
//! - **Sell only at a real profit — or to cut a stuck position.** A holding sells
//!   when the current berth's BID clears our cost basis by a margin. The one
//!   exception is liquidation: a holding carried too long, or one whose hold space
//!   the ship needs back, sells at whatever the bid is (even a loss) rather than
//!   becoming dead capital riding forever. Bounded risk beats stuck risk.
//! - **Buy conservatively.** Buy at the berth's real ASK; value the eventual sale at
//!   the target's MID minus a haircut that stands in for the bid-spread, tax, and
//!   the mid moving against us before we arrive. Require the estimated margin to
//!   clear a floor, so a wrong haircut still lands in the black.
//! - **Small positions.** Never spend more than a fraction of cash or fill more than
//!   a fraction of the hold on one speculative bet — the map moves, and a big
//!   position is a big way to be wrong.
//! - **Only reachable, fuelable sell targets.** A good you cannot carry to its buyer
//!   is not an arbitrage, it is ballast. Reachability is asked of the router LAZILY —
//!   the best-paying berth first, then the next — because on the wire every question
//!   is a `/v1/route` call on the ship's one key, and a fold that priced every
//!   good against every station would rate-limit the pilot off its own exchange.
//! - **Fuel for the carry.** A buy is sized against the fuel we can actually leave
//!   with (the tank here if this berth sells fuel, else what is in it), reserve
//!   included; the carry leg itself re-checks before filing.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::doctrine::Router;

/// Fuel reserve on a carry leg: the same 20% margin the freight doctrine keeps on a
/// booking. A merchant that arrives dry has sold nothing.
const CARRY_RESERVE_BPS: i64 = 12_000;

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

/// Can a leg costing `cost` fuel be flown on `fuel` in the tank, reserve included?
pub fn carry_affordable(cost: i64, fuel: i64) -> bool {
    fuel >= bps(cost, CARRY_RESERVE_BPS)
}

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

/// A speculative position we are carrying: what we hold, what it cost, and where we
/// meant to sell it. Persisted in the ship store so a restart does not forget cost basis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Holding {
    pub good: String,
    pub units: i64,
    /// Average ℳ per unit paid, including tax — the basis every sell decision clears.
    pub avg_cost: i64,
    /// Where we intended to sell it (advisory; we sell wherever it is profitable).
    pub sell_target: String,
    /// The tick we opened the position, for the stuck-inventory liquidation rule.
    pub opened_tick: i64,
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
/// A buy must show at least this estimated margin over cost (20%) to be worth the risk.
const BUY_MARGIN_BPS: i64 = 2000;
/// Never spend more than this fraction of cash (25%) on one speculative position.
const MAX_CASH_BPS: i64 = 2500;
/// Never fill more than this fraction of the (spare) hold (50%) with one bet.
const MAX_HOLD_BPS: i64 = 5000;
/// Carry a losing/idle position no longer than this many ticks before liquidating it.
const STUCK_TICKS: i64 = 240;
/// Below this cash we do not open new speculative positions — trading never starves
/// the ship of the credits it needs to refuel and service its lease.
const MIN_CASH_FLOOR: i64 = 2000;

fn bps(value: i64, b: i64) -> i64 {
    value * b / 10_000
}

/// The merchant judgment. `here` is the berth; `board` its live quotes; `galaxy` the
/// whole map's mids; `holdings` what we carry; `spare_hold` the units of hold not
/// already committed to freight; `need_hold` is true when freight wants the space back;
/// `fuel_available` is what the carry leg can leave with (a full tank at a pump).
#[allow(clippy::too_many_arguments)]
pub fn decide_trade(
    here: &str,
    tick: i64,
    board: &[GoodQuote],
    galaxy: &[MarketRow],
    holdings: &[Holding],
    credits: i64,
    spare_hold: i64,
    need_hold: bool,
    fuel_available: i64,
    pumps: &BTreeSet<String>,
    router: &dyn Router,
) -> TradeDecision {
    // 1. SELL — realize a holding whose bid here clears its basis, or cut one that is
    //    stuck (too old, or the ship needs the space). Sells free both cash and hold,
    //    so they come before any buy.
    for h in holdings {
        if h.units <= 0 {
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
        let stuck = tick - h.opened_tick > STUCK_TICKS || need_hold;
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
                    if need_hold {
                        "hold needed"
                    } else {
                        "carried too long"
                    }
                ),
            };
        }
    }

    // 2. BUY — an arbitrage we can carry. Never when cash is at the floor, the hold is
    //    tight, or freight wants the space.
    if need_hold || spare_hold <= 0 || credits < MIN_CASH_FLOOR {
        return TradeDecision::Idle {
            why: "no room/cash to open a position".into(),
        };
    }

    // The best good sold here, by estimated margin to its best reachable buyer.
    let mut best: Option<(i64, TradeDecision)> = None; // (est total margin, decision)
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
        let mut best_target: Option<(i64, &str)> = None; // (mid, station)
        for row in candidates {
            let est_proceeds = row.mid - bps(row.mid, SELL_HAIRCUT_BPS);
            if est_proceeds - q.ask < bps(q.ask, BUY_MARGIN_BPS).max(1) {
                break; // sorted: nothing below this mid clears either
            }
            let Some(cost) = router.fuel_between(here, &row.station) else {
                continue; // unreachable / unpriceable — not an arbitrage
            };
            if !carry_affordable(cost, fuel_available) {
                continue; // a buyer we cannot fly to is ballast
            }
            best_target = Some((row.mid, &row.station));
            break;
        }
        let Some((target_mid, target)) = best_target else {
            continue;
        };
        // Conservative per-unit economics: pay the real ask; expect the target mid less
        // the haircut. Prefer a pump-adjacent buyer, all else equal, so the run refuels
        // itself — but never require it.
        let est_proceeds = target_mid - bps(target_mid, SELL_HAIRCUT_BPS);
        let per_unit_margin = est_proceeds - q.ask;
        if per_unit_margin <= 0 || per_unit_margin < bps(q.ask, BUY_MARGIN_BPS) {
            continue;
        }
        // Size the position: bounded by cash, spare hold, and the shelf.
        let by_cash = bps(credits, MAX_CASH_BPS) / q.ask.max(1);
        let by_hold = bps(spare_hold, MAX_HOLD_BPS).max(0);
        let units = by_cash.min(by_hold).min(q.max_buy);
        if units <= 0 {
            continue;
        }
        let total_margin = per_unit_margin * units + if pumps.contains(target) { 1 } else { 0 };
        if best
            .as_ref()
            .map(|(m, _)| total_margin > *m)
            .unwrap_or(true)
        {
            best = Some((
                total_margin,
                TradeDecision::Buy {
                    good: q.good.clone(),
                    units,
                    sell_target: target.to_string(),
                    est_margin: per_unit_margin * units,
                },
            ));
        }
    }

    best.map(|(_, d)| d).unwrap_or(TradeDecision::Idle {
        why: "no profitable, carryable arbitrage on the board".into(),
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

    #[test]
    fn sells_a_holding_when_the_bid_clears_basis_plus_margin() {
        let hold = vec![Holding {
            good: "catnip".into(),
            units: 40,
            avg_cost: 30,
            sell_target: "foxys-diner".into(),
            opened_tick: 100,
        }];
        // bid 40 vs basis 30 (+12% = 33.6): clears.
        let board = vec![q("catnip", 42, 40, 500)];
        let d = decide_trade(
            "foxys-diner",
            150,
            &board,
            &[],
            &hold,
            9000,
            120,
            false,
            500,
            &pumps(),
            &Reach(true),
        );
        assert!(
            matches!(d, TradeDecision::Sell { good, units, .. } if good == "catnip" && units == 40)
        );
    }

    #[test]
    fn does_not_sell_at_a_loss_unless_stuck() {
        let hold = vec![Holding {
            good: "catnip".into(),
            units: 40,
            avg_cost: 50,
            sell_target: "foxys-diner".into(),
            opened_tick: 100,
        }];
        let board = vec![q("catnip", 42, 40, 500)]; // bid 40 < basis 50 — a loss
                                                    // Fresh position, hold not needed: hold it, do not dump.
        let d = decide_trade(
            "foxys-diner",
            150,
            &board,
            &[],
            &hold,
            9000,
            120,
            false,
            500,
            &pumps(),
            &Reach(true),
        );
        assert!(!matches!(d, TradeDecision::Sell { .. }));
    }

    #[test]
    fn liquidates_a_stuck_position_even_at_a_loss() {
        let hold = vec![Holding {
            good: "catnip".into(),
            units: 40,
            avg_cost: 50,
            sell_target: "foxys-diner".into(),
            opened_tick: 100,
        }];
        let board = vec![q("catnip", 42, 40, 500)];
        // Carried > STUCK_TICKS (tick 400 - opened 100 = 300 > 240): cut it.
        let d = decide_trade(
            "foxys-diner",
            400,
            &board,
            &[],
            &hold,
            9000,
            120,
            false,
            500,
            &pumps(),
            &Reach(true),
        );
        assert!(matches!(d, TradeDecision::Sell { .. }));
    }

    #[test]
    fn liquidates_when_freight_needs_the_hold() {
        let hold = vec![Holding {
            good: "catnip".into(),
            units: 40,
            avg_cost: 50,
            sell_target: "foxys-diner".into(),
            opened_tick: 100,
        }];
        let board = vec![q("catnip", 42, 40, 500)];
        let d = decide_trade(
            "foxys-diner",
            150,
            &board,
            &[],
            &hold,
            0,
            120,
            true,
            500,
            &pumps(),
            &Reach(true),
        );
        assert!(matches!(d, TradeDecision::Sell { .. }));
    }

    #[test]
    fn buys_a_profitable_reachable_arbitrage() {
        // catnip asks 30 here; sells (mid 60, -18% haircut = 49) at whisker-hollow.
        // margin 19/unit > 20% of ask (6): buy.
        let board = vec![q("catnip", 30, 28, 500)];
        let galaxy = vec![
            row("catnip", "here", 29),
            row("catnip", "whisker-hollow", 60),
        ];
        let d = decide_trade(
            "here",
            100,
            &board,
            &galaxy,
            &[],
            10_000,
            120,
            false,
            500,
            &pumps(),
            &Reach(true),
        );
        match d {
            TradeDecision::Buy {
                good,
                sell_target,
                units,
                ..
            } => {
                assert_eq!(good, "catnip");
                assert_eq!(sell_target, "whisker-hollow");
                assert!(units > 0);
            }
            other => panic!("expected Buy, got {other:?}"),
        }
    }

    #[test]
    fn refuses_arbitrage_to_an_unreachable_buyer() {
        let board = vec![q("catnip", 30, 28, 500)];
        let galaxy = vec![row("catnip", "whisker-hollow", 60)];
        let d = decide_trade(
            "here",
            100,
            &board,
            &galaxy,
            &[],
            10_000,
            120,
            false,
            500,
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
            "here",
            100,
            &board,
            &galaxy,
            &[],
            10_000,
            120,
            false,
            500,
            &pumps(),
            &Reach(true),
        );
        assert!(matches!(d, TradeDecision::Idle { .. }));
    }

    #[test]
    fn will_not_open_a_position_below_the_cash_floor() {
        let board = vec![q("catnip", 30, 28, 500)];
        let galaxy = vec![row("catnip", "whisker-hollow", 60)];
        let d = decide_trade(
            "here",
            100,
            &board,
            &galaxy,
            &[],
            1500,
            120,
            false,
            500,
            &pumps(),
            &Reach(true),
        );
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
            "here",
            100,
            &board,
            &galaxy,
            &[],
            10_000,
            120,
            false,
            500,
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
        let d = decide_trade(
            "here",
            100,
            &board,
            &galaxy,
            &[],
            10_000,
            120,
            false,
            500,
            &pumps(),
            &chart,
        );
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
        let d = decide_trade(
            "here",
            100,
            &board,
            &galaxy,
            &[],
            10_000,
            120,
            false,
            500,
            &pumps(),
            &chart,
        );
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
        let d = decide_trade(
            "here",
            100,
            &board,
            &galaxy,
            &[],
            10_000,
            120,
            false,
            55,
            &pumps(),
            &Reach(true),
        );
        assert!(matches!(d, TradeDecision::Idle { .. }));
        // With the reserve met, the same run is a trade.
        let d = decide_trade(
            "here",
            100,
            &board,
            &galaxy,
            &[],
            10_000,
            120,
            false,
            60,
            &pumps(),
            &Reach(true),
        );
        assert!(matches!(d, TradeDecision::Buy { .. }));
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
    }
}
