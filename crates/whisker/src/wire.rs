//! The doctrine's reading of the exchange's JSON, and the seam through which any
//! runtime asks it (T-237 B4, "one doctrine, two runtimes", Ian 2026-09-05).
//!
//! Everything here is pure: wire JSON in, doctrine types out, a decision back as
//! JSON. The host runner (`src/main.rs`) reads the same functions before every fold;
//! the iPad calls [`advise`] through `core-ffi` with the JSON it fetched itself; a
//! service in the cloud would do the same. No socket, no file, no clock — the caller
//! supplies the routes it priced, so the doctrine never reaches for the wire.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::autonomy::{Dial, Surface};
use crate::doctrine::{self, Active, ActiveWord, Decision, LoadRow, Router, Ship};

/// The hull as the doctrine sees it, from `/v1/me`. `repair_rate` is the yard's
/// `repairCostPerHundredBps` from `/v1/reference`.
pub fn ship_from(me: &Value, repair_rate: i64) -> Ship {
    let route_len = me
        .get("route")
        .and_then(Value::as_array)
        .map(|r| r.len())
        .unwrap_or(0);
    let docked = me.get("docked").and_then(Value::as_str).map(String::from);
    // Under way = NOT berthed. PROD reports `route: []` DURING a crossing (the
    // transit rides arrival ticks, not the route array), so keying flight on a
    // non-empty route read a flying ship as "adrift between folds" and held on a
    // wrong reason (KK II, t6094 en route to titania-cold-store, 2026-09-01). A
    // course merely LAID but not yet engaged shows as `driveAwaiting`, and that
    // is handled before the doctrine ever sees the ship — so by here, no berth
    // means she is crossing. The route array stays a belt to those braces.
    // ...and a berthed hull with hops still on the plan is UNDER WAY only while the
    // plan can still be flown. The engine re-attempts the next leg each fold and,
    // when the tank will not cover it, declines in silence (metal#79) — so a hull
    // that cannot afford its own remaining course sits berthed behind a route that
    // will never move, and reading that as flight makes the pilot hold rather than
    // rescue it. KK II did this at cannery-row on 2026-09-05: docked, 18 of 600,
    // 8,323 credits, a stale course to the-bonded-hold needing about 200, and a
    // pump two ticks away — journalling "under way, no load" while parked.
    //
    // The tank is the tell. Below the critical fraction a remaining course is a
    // stall, not a crossing, and the fuel doors below should have it. Above it,
    // nothing changes: a healthy multi-hop route is left alone to fly itself.
    let fuel_now = me.get("fuel").and_then(Value::as_i64).unwrap_or(0);
    let tank = me.get("fuelCapacity").and_then(Value::as_i64).unwrap_or(0);
    let stalled = docked.is_some()
        && route_len > 0
        && tank > 0
        && (fuel_now as f64 / tank as f64) < doctrine::CRITICAL_FUEL;
    Ship {
        in_flight: docked.is_none() || (route_len > 0 && !stalled),
        docked,
        accel_milli_g: me
            .get("effectiveAccelMilliG")
            .and_then(Value::as_i64)
            .unwrap_or(doctrine::REFERENCE_ACCEL_MILLI_G),
        wear_bps: me.get("wearBps").and_then(Value::as_i64).unwrap_or(0),
        leased: !me.get("titled").and_then(Value::as_bool).unwrap_or(true)
            && me
                .get("leasePrincipal")
                .and_then(Value::as_i64)
                .unwrap_or(0)
                > 0,
        repair_per_hundred_bps: repair_rate,
        hold_used: me.get("holdUsed").and_then(Value::as_i64).unwrap_or(0),
        hold_capacity: me.get("holdCapacity").and_then(Value::as_i64).unwrap_or(0),
        fuel: me.get("fuel").and_then(Value::as_i64).unwrap_or(0),
        fuel_capacity: me.get("fuelCapacity").and_then(Value::as_i64).unwrap_or(1),
        credits: me.get("credits").and_then(Value::as_i64).unwrap_or(0),
    }
}

/// One `/v1/loadboard` row as the doctrine prices it.
pub fn load_row(v: &Value) -> Option<LoadRow> {
    Some(LoadRow {
        load_id: v.get("loadId")?.as_str()?.to_string(),
        good: v
            .get("good")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        class_bps: match v
            .get("serviceClass")
            .and_then(Value::as_str)
            .unwrap_or("standard")
        {
            "economy" => 5_000,
            "express" => 20_000,
            "priority" => 30_000,
            _ => 10_000,
        },
        origin: v.get("origin")?.as_str()?.to_string(),
        dest: v.get("dest")?.as_str()?.to_string(),
        units: v.get("units").and_then(Value::as_i64).unwrap_or(0),
        estimated_net: v.get("estimatedNet").and_then(Value::as_i64).unwrap_or(0),
        deadhead_ticks: v.get("deadheadTicks").and_then(Value::as_i64).unwrap_or(0),
        haul_ticks: v.get("haulTicks").and_then(Value::as_i64).unwrap_or(0),
        loading_ticks: v.get("loadingTicks").and_then(Value::as_i64).unwrap_or(8),
        held_for_other: v
            .get("heldForOther")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// The stations that sell fuel, from `/v1/stations`. Anything unreadable is no pump.
pub fn pumps_from(stations: &Value) -> BTreeSet<String> {
    match stations {
        Value::Array(stations) => stations
            .iter()
            .filter(|s| s.get("sellsFuel").and_then(Value::as_bool).unwrap_or(false))
            .filter_map(|s| s.get("id").and_then(Value::as_str).map(String::from))
            .collect(),
        _ => BTreeSet::new(),
    }
}

/// The ledger's last word about a load in `/v1/me.freight`, reduced to what decides.
/// `None` = settled or lost — either way the caller stops tracking it (with the
/// reason for the journal).
pub fn active_word(me: &Value, load_id: &str) -> Result<Option<ActiveWord>, String> {
    let events = me
        .get("freight")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter(|f| f.get("loadId").and_then(Value::as_str) == Some(load_id))
                .filter_map(|f| f.get("event").and_then(Value::as_str))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    doctrine::ledger_word(&events)
}

/// The dial surface a decision spends.
pub fn surface_of(d: &Decision) -> Surface {
    match d {
        Decision::Refuel | Decision::DivertToPump { .. } => Surface::NavigationFuel,
        Decision::CallPaws => Surface::NavigationRescue,
        Decision::Travel { .. } | Decision::Hold { .. } => Surface::NavigationCourse,
        Decision::Repair => Surface::ShipRepair,
        Decision::Book { .. } => Surface::FreightBook,
        Decision::Collect { .. } => Surface::FreightCollect,
    }
}

/// A decision as JSON: `type` in the journal's vocabulary plus its fields.
pub fn decision_json(d: &Decision) -> Value {
    match d {
        Decision::Hold { why } => json!({"type": "hold", "why": why}),
        Decision::Refuel => json!({"type": "refuel"}),
        Decision::Repair => json!({"type": "repair"}),
        Decision::CallPaws => json!({"type": "call-paws"}),
        Decision::DivertToPump { pump, burn_bps } => {
            json!({"type": "divert-to-pump", "pump": pump, "burn_bps": burn_bps,
                   "burn": doctrine::burn_wire_name(*burn_bps)})
        }
        Decision::Book { load_id } => json!({"type": "book", "load_id": load_id}),
        Decision::Travel { station } => json!({"type": "travel", "station": station}),
        Decision::Collect { load_id } => json!({"type": "collect", "load_id": load_id}),
    }
}

/// A router over routes the CALLER already priced — `[{from, to, fuel, legs_km}]`,
/// each `/v1/route` answer reduced to its fuel and leg separations. A pair not in the
/// table is unknown to the doctrine, which then falls back to the board's own figure
/// exactly as it does when the wire cannot say.
pub struct TableRouter {
    routes: BTreeMap<(String, String), (i64, Vec<i64>)>,
}

impl TableRouter {
    pub fn from_json(routes: &Value) -> Self {
        let mut table = BTreeMap::new();
        if let Some(rows) = routes.as_array() {
            for r in rows {
                let (Some(from), Some(to)) = (
                    r.get("from").and_then(Value::as_str),
                    r.get("to").and_then(Value::as_str),
                ) else {
                    continue;
                };
                let fuel = r.get("fuel").and_then(Value::as_i64).unwrap_or(0);
                let legs = r
                    .get("legs_km")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_i64).collect())
                    .unwrap_or_default();
                table.insert((from.to_string(), to.to_string()), (fuel, legs));
            }
        }
        TableRouter { routes: table }
    }
}

impl Router for TableRouter {
    fn fuel_between(&self, from: &str, to: &str) -> Option<i64> {
        if from == to {
            return Some(0);
        }
        self.routes
            .get(&(from.to_string(), to.to_string()))
            .map(|(f, _)| *f)
    }
    fn leg_distances_km(&self, from: &str, to: &str) -> Option<Vec<i64>> {
        if from == to {
            return Some(Vec::new());
        }
        self.routes
            .get(&(from.to_string(), to.to_string()))
            .map(|(_, l)| l.clone())
    }
}

/// The seam. `input` is one JSON object:
///
/// ```json
/// {"me": <GET /v1/me>, "board": <GET /v1/loadboard>, "stations": <GET /v1/stations>,
///  "routes": [{"from","to","fuel","legs_km"}], "repair_per_hundred_bps": 40,
///  "active_load_id": "L1234" | null, "dial": <autonomy.json> | null}
/// ```
///
/// The answer names the decision, the dial surface it spends, the captain's level on
/// that surface (advise / confirm / auto), the automation it needs, and the hull as
/// the doctrine read it — so the caller can show the pilot's mind and, under the act
/// scope, put the same act on the wire the host runner would. It never acts.
pub fn advise(input: &Value) -> Value {
    let me = input.get("me").cloned().unwrap_or(Value::Null);
    let repair_rate = input
        .get("repair_per_hundred_bps")
        .and_then(Value::as_i64)
        .unwrap_or(40);
    let ship = ship_from(&me, repair_rate);
    let board: Vec<LoadRow> = input
        .get("board")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(load_row).collect())
        .unwrap_or_default();
    let pumps = pumps_from(input.get("stations").unwrap_or(&Value::Null));
    let router = TableRouter::from_json(input.get("routes").unwrap_or(&Value::Null));
    let active = input
        .get("active_load_id")
        .and_then(Value::as_str)
        .and_then(|lid| {
            let row = board.iter().find(|l| l.load_id == lid)?.clone();
            let word = active_word(&me, lid).ok().flatten()?;
            Some(Active { row, word })
        });
    let dial = input
        .get("dial")
        .map(|d| Dial::parse(&d.to_string()))
        .unwrap_or_default();
    let decision = doctrine::decide(&ship, active.as_ref(), &board, &pumps, &router);
    let surface = surface_of(&decision);
    json!({
        "decision": decision_json(&decision),
        "surface": surface.key(),
        "family": surface.family(),
        "level": dial.level(surface).name(),
        "automation": decision.automation().map(|a| format!("{a:?}").to_lowercase()),
        "ship": {
            "docked": ship.docked, "in_flight": ship.in_flight, "fuel": ship.fuel,
            "fuel_capacity": ship.fuel_capacity, "credits": ship.credits, "wear_bps": ship.wear_bps,
            "leased": ship.leased, "hold_used": ship.hold_used, "hold_capacity": ship.hold_capacity,
            "accel_milli_g": ship.accel_milli_g,
        },
        "pumps": pumps,
        "board_rows": board.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(fuel: i64) -> Value {
        json!({
            "me": {"docked": "a", "fuel": fuel, "fuelCapacity": 600, "credits": 5000, "wearBps": 0,
                   "titled": false, "leasePrincipal": 25000, "holdUsed": 0, "holdCapacity": 60,
                   "effectiveAccelMilliG": 189, "route": [], "freight": []},
            "board": [{"loadId": "L1", "good": "water-ice", "origin": "a", "dest": "b", "units": 40,
                       "estimatedNet": 900, "deadheadTicks": 0, "haulTicks": 6, "loadingTicks": 8,
                       "serviceClass": "standard"}],
            "stations": [{"id": "a", "sellsFuel": true}, {"id": "b", "sellsFuel": false}],
            "routes": [{"from": "a", "to": "b", "fuel": 60, "legs_km": [720000]},
                       {"from": "b", "to": "a", "fuel": 60, "legs_km": [720000]}],
            "repair_per_hundred_bps": 40,
            "dial": {"freight": "confirm", "*": "auto"}
        })
    }

    #[test]
    fn a_full_tank_at_a_berth_with_a_paying_load_books_it_on_the_captains_level() {
        let out = advise(&fixture(600));
        assert_eq!(out["decision"]["type"], "book", "{out}");
        assert_eq!(out["decision"]["load_id"], "L1");
        assert_eq!(out["surface"], "freight.book");
        assert_eq!(
            out["level"], "confirm",
            "the dial's freight family says confirm"
        );
        assert_eq!(out["automation"], "freight");
        assert_eq!(out["ship"]["docked"], "a");
    }

    #[test]
    fn the_same_reading_the_host_makes() {
        // The seam reads the hull exactly as the runner does: same struct, same fields.
        let f = fixture(200);
        let ship = ship_from(&f["me"], 40);
        assert_eq!(ship.fuel, 200);
        assert!(ship.leased && !ship.in_flight);
        let out = advise(&f);
        // A third of a tank at a pump: the doctrine tops up before it takes work.
        assert_eq!(out["decision"]["type"], "refuel", "{out}");
        assert_eq!(out["surface"], "navigation.fuel");
        assert_eq!(out["level"], "auto");
    }

    #[test]
    fn an_unpriced_pair_is_unknown_not_zero() {
        let r = TableRouter::from_json(&json!([{"from": "a", "to": "b", "fuel": 60}]));
        assert_eq!(r.fuel_between("a", "b"), Some(60));
        assert_eq!(r.fuel_between("b", "a"), None);
        assert_eq!(r.fuel_between("a", "a"), Some(0));
        assert_eq!(surface_of(&Decision::CallPaws).key(), "navigation.rescue");
        assert_eq!(
            decision_json(&Decision::Book {
                load_id: "L9".into()
            })["type"],
            "book"
        );
    }
}
