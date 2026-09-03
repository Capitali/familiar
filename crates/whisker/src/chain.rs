//! The supply chain, read as arithmetic — T-238 brick 1 (Ian, 2026-09-02: "our
//! plans looking forward should utilize this information in p&L maximization
//! routines").
//!
//! The exchange already serves the whole production graph: `/v1/reference`
//! carries every station's recipes (inputs, outputs, ticks per cycle) and every
//! good's decay; the quotes whisker already reads carry each shelf's live stock,
//! capacity, and equilibrium. This module turns those into the two numbers a
//! forward plan wants:
//!
//! - **runway** — how many ticks a works can keep eating a given input before
//!   its shelf runs dry (a shrinking runway is a bid that must rise: feed it);
//! - **headroom** — how many ticks its output shelf can keep filling before it
//!   hits capacity (a shrinking headroom is an ask that must fall: lift it).
//!
//! HONESTY BOUND, stated once and carried in the type: the wire does not serve
//! line UTILIZATION (the works screen's "2/10"), so every rate here assumes the
//! lines run FULL. Runway is therefore a LOWER bound and headroom is a lower
//! bound too — the works cannot eat faster or fill faster than this. The live
//! corrective is the shelf's own stock-vs-equilibrium pressure, which the
//! quotes serve and the merchant already prices. If Jeff exposes utilization,
//! `Flow::rate_per_kilotick` is where it lands.
//!
//! Pure: no socket, no clock. The runner hands in parsed reference/quote JSON;
//! fixtures hand in the same shapes. Integer arithmetic throughout, engine
//! style — rates are units per 1,000 ticks (a "kilotick") so short cycles keep
//! precision without floats.

use std::collections::BTreeMap;

use serde_json::Value;

/// One production line at a station, as `/v1/reference` serves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    pub id: String,
    pub station: String,
    /// Good → units consumed per cycle.
    pub inputs: BTreeMap<String, i64>,
    /// Good → units produced per cycle.
    pub outputs: BTreeMap<String, i64>,
    pub ticks_per_cycle: i64,
}

/// Parse the reference's `recipes` array. Rows missing their load-bearing
/// fields are skipped — a chain model must never invent a line.
pub fn parse_recipes(reference: &Value) -> Vec<Recipe> {
    let Some(rows) = reference.get("recipes").and_then(Value::as_array) else {
        return Vec::new();
    };
    rows.iter()
        .filter_map(|r| {
            let goods = |key: &str| -> BTreeMap<String, i64> {
                r.get(key)
                    .and_then(Value::as_object)
                    .map(|m| {
                        m.iter()
                            .filter_map(|(g, v)| Some((g.clone(), v.as_i64()?)))
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let recipe = Recipe {
                id: r.get("id")?.as_str()?.to_string(),
                station: r.get("station")?.as_str()?.to_string(),
                inputs: goods("inputs"),
                outputs: goods("outputs"),
                ticks_per_cycle: r.get("ticksPerCycle")?.as_i64()?,
            };
            (recipe.ticks_per_cycle > 0 && !(recipe.inputs.is_empty() && recipe.outputs.is_empty()))
                .then_some(recipe)
        })
        .collect()
}

/// Each good's decay in basis points per tick(ish), from the reference's
/// `goods` — the carry cost a forward plan charges itself for slow legs.
pub fn parse_decay(reference: &Value) -> BTreeMap<String, i64> {
    reference
        .get("goods")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|g| {
                    Some((
                        g.get("id")?.as_str()?.to_string(),
                        g.get("decayBps").and_then(Value::as_i64).unwrap_or(0),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A live shelf at one station, as the quotes serve it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shelf {
    pub station: String,
    pub good: String,
    pub stock: i64,
    pub capacity: i64,
    pub equilibrium: i64,
}

/// Which way a station moves a good.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowKind {
    /// The works EATS this good here — its shelf drains, its bid firms.
    Eats,
    /// The works MAKES this good here — its shelf fills, its ask softens.
    Makes,
}

/// One station×good flow, with the live shelf folded in when known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flow {
    pub station: String,
    pub good: String,
    pub kind: FlowKind,
    /// Units per 1,000 ticks at FULL lines (the honesty bound above).
    pub rate_per_kilotick: i64,
    /// The live shelf, when the caller had quotes for this berth.
    pub shelf: Option<Shelf>,
    /// Ticks until the shelf is empty (Eats) or full (Makes), at full lines.
    /// None without a shelf, or when the rate is zero.
    pub horizon_ticks: Option<i64>,
}

/// Fold recipes and live shelves into the full flow table, one row per
/// station×good×direction, rates summed across that station's lines.
pub fn flows(recipes: &[Recipe], shelves: &[Shelf]) -> Vec<Flow> {
    let mut rates: BTreeMap<(String, String, bool), i64> = BTreeMap::new();
    for r in recipes {
        for (good, units) in &r.inputs {
            *rates
                .entry((r.station.clone(), good.clone(), true))
                .or_insert(0) += units * 1000 / r.ticks_per_cycle;
        }
        for (good, units) in &r.outputs {
            *rates
                .entry((r.station.clone(), good.clone(), false))
                .or_insert(0) += units * 1000 / r.ticks_per_cycle;
        }
    }
    rates
        .into_iter()
        .map(|((station, good, eats), rate)| {
            let shelf = shelves
                .iter()
                .find(|s| s.station == station && s.good == good)
                .cloned();
            let horizon_ticks = shelf.as_ref().and_then(|s| {
                if rate <= 0 {
                    return None;
                }
                let room = if eats {
                    s.stock
                } else {
                    (s.capacity - s.stock).max(0)
                };
                Some(room * 1000 / rate)
            });
            Flow {
                station,
                good,
                kind: if eats {
                    FlowKind::Eats
                } else {
                    FlowKind::Makes
                },
                rate_per_kilotick: rate,
                shelf,
                horizon_ticks,
            }
        })
        .collect()
}

/// The feeds worth flying: inputs whose shelf runs dry inside the horizon, most
/// urgent first. A starving works is a rising bid with a deadline on it.
pub fn starving(flows: &[Flow], horizon_ticks: i64) -> Vec<&Flow> {
    let mut hungry: Vec<&Flow> = flows
        .iter()
        .filter(|f| f.kind == FlowKind::Eats)
        .filter(|f| f.horizon_ticks.is_some_and(|h| h <= horizon_ticks))
        .collect();
    hungry.sort_by_key(|f| f.horizon_ticks);
    hungry
}

/// The lifts worth flying: outputs whose shelf fills inside the horizon, most
/// urgent first. A glutting works is a softening ask — and a stalled line once
/// the shelf is full, which starves every buyer downstream.
pub fn glutting(flows: &[Flow], horizon_ticks: i64) -> Vec<&Flow> {
    let mut full: Vec<&Flow> = flows
        .iter()
        .filter(|f| f.kind == FlowKind::Makes)
        .filter(|f| f.horizon_ticks.is_some_and(|h| h <= horizon_ticks))
        .collect();
    full.sort_by_key(|f| f.horizon_ticks);
    full
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The reference, shaped exactly as PROD serves it (checked live 2026-09-02:
    /// the Biscuit Press row's own numbers).
    fn reference() -> Value {
        json!({
            "recipes": [
                {"id": "biscuit-press", "displayName": "Biscuit Press, Grade-2 Line",
                 "station": "tranquility", "ticksPerCycle": 8,
                 "inputs": {"grain": 22, "tinplate": 4},
                 "outputs": {"biscuit-substrate": 16}},
                {"id": "cannery-line", "station": "cannery-row", "ticksPerCycle": 10,
                 "inputs": {"fishmeal": 22, "grain": 16, "tinplate": 4},
                 "outputs": {"kibble-loaf": 33}},
                {"id": "gravy-reduction", "station": "cannery-row", "ticksPerCycle": 5,
                 "inputs": {"fishmeal": 10, "water-ice": 7},
                 "outputs": {"gravy-base": 10}},
                {"id": "broken-row", "station": "nowhere", "ticksPerCycle": 0,
                 "inputs": {"grain": 1}, "outputs": {}}
            ],
            "goods": [
                {"id": "fishmeal", "decayBps": 12},
                {"id": "kibble-loaf", "decayBps": 4}
            ]
        })
    }

    #[test]
    fn recipes_parse_the_wire_shape_and_refuse_broken_rows() {
        let recipes = parse_recipes(&reference());
        assert_eq!(recipes.len(), 3, "the zero-cycle row is not a line");
        let press = recipes.iter().find(|r| r.id == "biscuit-press").unwrap();
        assert_eq!(press.station, "tranquility");
        assert_eq!(press.inputs["grain"], 22);
        assert_eq!(press.outputs["biscuit-substrate"], 16);
        assert_eq!(press.ticks_per_cycle, 8);
        assert_eq!(parse_decay(&reference())["fishmeal"], 12);
    }

    #[test]
    fn rates_sum_across_a_stations_lines_per_kilotick() {
        // Cannery Row eats fishmeal on TWO lines: 22/10-tick and 10/5-tick →
        // 2200 + 2000 = 4200 units per kilotick. The chain sees the station's
        // whole appetite, not one line's.
        let flows = flows(&parse_recipes(&reference()), &[]);
        let fishmeal = flows
            .iter()
            .find(|f| f.station == "cannery-row" && f.good == "fishmeal")
            .unwrap();
        assert_eq!(fishmeal.kind, FlowKind::Eats);
        assert_eq!(fishmeal.rate_per_kilotick, 4200);
        assert_eq!(fishmeal.horizon_ticks, None, "no shelf, no horizon");
    }

    #[test]
    fn runway_and_headroom_come_from_the_live_shelf() {
        let shelves = vec![
            Shelf {
                station: "cannery-row".into(),
                good: "fishmeal".into(),
                stock: 862,
                capacity: 1200,
                equilibrium: 600,
            },
            Shelf {
                station: "cannery-row".into(),
                good: "kibble-loaf".into(),
                stock: 356,
                capacity: 400,
                equilibrium: 200,
            },
        ];
        let all = flows(&parse_recipes(&reference()), &shelves);
        let runway = all
            .iter()
            .find(|f| f.good == "fishmeal" && f.kind == FlowKind::Eats)
            .unwrap();
        // 862 units at 4200/kilotick → 205 ticks of eating left, at full lines.
        assert_eq!(runway.horizon_ticks, Some(205));
        let head = all
            .iter()
            .find(|f| f.good == "kibble-loaf" && f.kind == FlowKind::Makes)
            .unwrap();
        // 44 units of room at 3300/kilotick → 13 ticks until the shelf is full.
        assert_eq!(head.horizon_ticks, Some(13));
    }

    #[test]
    fn starving_and_glutting_rank_by_urgency_inside_the_horizon() {
        let shelves = vec![
            Shelf {
                station: "cannery-row".into(),
                good: "fishmeal".into(),
                stock: 40, // 9 ticks of appetite — urgent
                capacity: 1200,
                equilibrium: 600,
            },
            Shelf {
                station: "cannery-row".into(),
                good: "water-ice".into(),
                stock: 700, // 500 ticks — comfortable
                capacity: 900,
                equilibrium: 400,
            },
            Shelf {
                station: "cannery-row".into(),
                good: "gravy-base".into(),
                stock: 1990, // nearly full at 2000
                capacity: 2000,
                equilibrium: 900,
            },
        ];
        let all = flows(&parse_recipes(&reference()), &shelves);
        let feeds = starving(&all, 100);
        assert_eq!(
            feeds
                .iter()
                .map(|f| (f.good.as_str(), f.horizon_ticks.unwrap()))
                .collect::<Vec<_>>(),
            vec![("fishmeal", 9)],
            "the comfortable shelf stays off the feed list"
        );
        let lifts = glutting(&all, 100);
        assert_eq!(lifts.len(), 1);
        assert_eq!(lifts[0].good, "gravy-base");
        assert_eq!(lifts[0].horizon_ticks, Some(5));
    }
}
