//! T-238 brick 2, offline: the runner's path from documents to a journal line
//! (codex design review, finding 4). Starts from a `/v1/reference`, two station
//! quote boards, and a `/v1/galaxy/prices` — real document shapes, no live ship —
//! and proves that the spot-only decision is idle, that a valid forecast turns it
//! into a buy, and that the execution journal carries the same bounded reason
//! the merchant gave. The equilibrium (600 units) and the mid (34 ℳ) are
//! deliberately far apart so a forecast that read one as the other would show.

use std::collections::{BTreeMap, BTreeSet};

use familiar_whisker::chain;
use familiar_whisker::doctrine::Router;
use familiar_whisker::trade::{self, Forecast, Holding, Ledger, TradeDecision};
use serde_json::Value;

const REFERENCE: &str = include_str!("fixtures/t238/reference.json");
const QUOTES_HERE: &str = include_str!("fixtures/t238/quotes-salt-flat.json");
const QUOTES_WORKS: &str = include_str!("fixtures/t238/quotes-works-b.json");
const GALAXY: &str = include_str!("fixtures/t238/galaxy.json");

/// Every leg costs 50 fuel; no distances, so the hold clock is the sell tick.
struct Lane;
impl Router for Lane {
    fn fuel_between(&self, _: &str, _: &str) -> Option<i64> {
        Some(50)
    }
}

/// The runner's one-time shelf sweep: capacity and equilibrium per (station,
/// good) from each station's quotes, merged with live stock from the galaxy.
fn shelves(quotes: &[(&str, &Value)], galaxy: &[trade::MarketRow]) -> Vec<chain::Shelf> {
    let shape: BTreeMap<(String, String), (i64, i64)> = quotes
        .iter()
        .flat_map(|(station, v)| {
            v["goods"].as_array().unwrap().iter().map(move |g| {
                (
                    (station.to_string(), g["good"].as_str().unwrap().to_string()),
                    (
                        g["capacity"].as_i64().unwrap(),
                        g["equilibrium"].as_i64().unwrap(),
                    ),
                )
            })
        })
        .collect();
    galaxy
        .iter()
        .filter_map(|r| {
            let (capacity, equilibrium) = *shape.get(&(r.station.clone(), r.good.clone()))?;
            Some(chain::Shelf {
                station: r.station.clone(),
                good: r.good.clone(),
                stock: r.stock,
                capacity,
                equilibrium,
            })
        })
        .collect()
}

/// The runner's ledger at salt-flat, for a fold with or without the forecast.
fn ledger_at<'a>(
    fc: Option<&'a Forecast>,
    decay: &'a BTreeMap<String, i64>,
    min_hold: i64,
) -> Ledger<'a> {
    Ledger {
        here: "salt-flat",
        tick: 9_000,
        credits: 10_000,
        spare_hold: 120,
        need_hold: false,
        fuel_available: 500,
        fuel_price: 2,
        min_hold,
        daily_fixed_cost: 600,
        ticks_per_day: 288,
        decay_bps: Some(decay),
        forecast: fc,
        borrowable: 0,
    }
}

#[test]
fn from_documents_to_a_journal_line_the_forecast_reason_survives() {
    let reference: Value = serde_json::from_str(REFERENCE).unwrap();
    let here: Value = serde_json::from_str(QUOTES_HERE).unwrap();
    let works: Value = serde_json::from_str(QUOTES_WORKS).unwrap();
    let galaxy = trade::parse_galaxy(&serde_json::from_str::<Value>(GALAXY).unwrap());
    let board = trade::parse_board(&here);

    let recipes = chain::parse_recipes(&reference);
    let pricing = chain::parse_pricing(&reference);
    let decay = chain::parse_decay(&reference);
    assert_eq!(recipes.len(), 1, "the reference's recipe parsed");
    assert_eq!(pricing.goods["brine"], (30, 9_000));
    assert_eq!(pricing.spread["works-b"], 500);

    let min_hold = 288;
    let shelves = shelves(&[("salt-flat", &here), ("works-b", &works)], &galaxy);
    let forecast = Forecast::build(&recipes, &shelves, &pricing, min_hold + 96);
    let pumps: BTreeSet<String> = BTreeSet::new();

    // The forecast prices the shelf in the exchange's units: 500 → 0 on a 600
    // shelf lifts the mid 34 → 57. Nothing here is 600 ℳ.
    let p = forecast.project("works-b", "brine", min_hold).unwrap();
    assert_eq!((p.stock_now.0, p.stock_then.0), (500, 0));
    assert_eq!((p.mid_now.0, p.mid_then.0), (34, 57));

    // 1. Spot only: idle. 34 less the haircut is 28, under the hurdle on an ask of 24.
    let spot = trade::decide_trade(
        &ledger_at(None, &decay, min_hold),
        &board,
        &galaxy,
        &[],
        &pumps,
        &Lane,
    );
    assert!(matches!(spot, TradeDecision::Idle { .. }), "{spot:?}");

    // 2. With the forecast: a buy, for works-b, and it says why.
    let d = trade::decide_trade(
        &ledger_at(Some(&forecast), &decay, min_hold),
        &board,
        &galaxy,
        &[],
        &pumps,
        &Lane,
    );
    let TradeDecision::Buy {
        good,
        units,
        sell_target,
        est_margin,
        why,
        ..
    } = d
    else {
        panic!("the forecast should have carried it: {d:?}");
    };
    assert_eq!((good.as_str(), sell_target.as_str()), ("brine", "works-b"));
    assert!(why.starts_with("forecast: works-b eats brine"), "{why}");
    assert!(
        why.contains("mid 34→57") && why.contains("absent events"),
        "{why}"
    );
    assert!(why.chars().count() <= trade::WHY_MAX_CHARS, "{why}");
    assert!(est_margin > 0);

    // 3. The execution journal, as the runner writes it after the ack: the same
    //    bounded reason, on the record that opens the position.
    let opened = Holding {
        good: good.clone(),
        units,
        avg_cost: 24,
        sell_target: sell_target.clone(),
        opened_tick: 9_000,
        sellable_at: 9_000 + min_hold,
    };
    let event = trade::position_opened(1_757_000_000, 9_000, &opened, est_margin, &why, "hold");
    assert_eq!(event["event"], "position-opened");
    assert_eq!(event["why"].as_str().unwrap(), why);
    assert_eq!(event["why"], trade::bounded_why(&why));
    assert_eq!(event["good"], "brine");
    assert_eq!(event["sell_target"], "works-b");
    assert_eq!(event["est_margin"], est_margin);
}
