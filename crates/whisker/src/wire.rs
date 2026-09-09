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
        tick: me.get("tick").and_then(Value::as_i64).unwrap_or(0),
        paws_inbound_to: me
            .get("callOut")
            .filter(|c| c.get("kind").and_then(Value::as_str) == Some("refuel"))
            .and_then(|c| c.get("to").and_then(Value::as_str))
            .map(String::from),
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
        deliver_deadline_tick: v
            .get("deliverDeadlineTick")
            .or_else(|| v.get("deliverByTick"))
            .and_then(Value::as_i64)
            .unwrap_or(0),
        held_for_other: v
            .get("heldForOther")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        // The chain's word, when the caller has one (the host's forecast); absent
        // is no word. Optional on the seam so an older or chain-less client's rows
        // still parse, and its advice differs from the host's only on a near-tie.
        chain_pressure: v.get("chain_pressure").and_then(Value::as_i64).unwrap_or(0),
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
    /// The WORLD's own price for this hull at a rung — `rungs.{standard,economy}:
    /// {fuel, ticks}` on a route row, from `/v1/route?hull=me&serviceClass=`. The
    /// host has asked the exchange this since 2026-09-08 and treats the answer as
    /// authoritative; a seam that could not carry it let the iPad keep modelling,
    /// and at the reachability boundary the two runtimes disagreed on the same
    /// facts — host CallPaws, iPad DivertToPump (codex T-237 B4 re-verification,
    /// finding 1). Every world fact that can move a decision rides the seam.
    rungs: BTreeMap<(String, String, i64), (i64, i64)>,
}

/// Bumped whenever the shape of `advise`'s input or output changes. A shell that
/// was built against one seam and is handed another must be able to tell, rather
/// than quietly reading a field that is no longer there (finding 1's skew guard).
pub const SEAM_VERSION: i64 = 2;

impl TableRouter {
    pub fn from_json(routes: &Value) -> Self {
        let mut table = BTreeMap::new();
        let mut rungs = BTreeMap::new();
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
                if let Some(rs) = r.get("rungs").and_then(Value::as_object) {
                    for (name, bps) in [
                        ("standard", doctrine::BURN_STANDARD),
                        ("economy", doctrine::BURN_ECONOMY),
                        ("express", doctrine::BURN_EXPRESS),
                        ("priority", doctrine::BURN_PRIORITY),
                    ] {
                        if let (Some(f), Some(t)) = (
                            rs.get(name)
                                .and_then(|q| q.get("fuel"))
                                .and_then(Value::as_i64),
                            rs.get(name)
                                .and_then(|q| q.get("ticks"))
                                .and_then(Value::as_i64),
                        ) {
                            rungs.insert((from.to_string(), to.to_string(), bps), (f, t));
                        }
                    }
                }
            }
        }
        TableRouter {
            routes: table,
            rungs,
        }
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
    fn quote_at_burn(&self, from: &str, to: &str, burn_bps: i64) -> Option<(i64, i64)> {
        if from == to {
            return Some((0, 0));
        }
        self.rungs
            .get(&(from.to_string(), to.to_string(), burn_bps))
            .copied()
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
/// WHY — a stable code and the bounded numbers that chose the branch, for every
/// decision and not only Hold. Recomputed from the same inputs `decide` read, so it
/// cannot disagree with the decision it explains; the prose stays in the shell,
/// which renders these facts rather than inventing a rationale (codex T-237 B4
/// re-verification, finding 4).
pub fn explain(
    d: &Decision,
    ship: &Ship,
    active: Option<&Active>,
    board: &[LoadRow],
    pumps: &BTreeSet<String>,
    router: &dyn Router,
) -> Value {
    let here = ship.docked.clone().unwrap_or_default();
    match d {
        Decision::Hold { why } => json!({"code": "hold", "why": why}),
        Decision::Refuel => json!({
            "code": "refuel.at-pump",
            "fuel": ship.fuel, "fuel_capacity": ship.fuel_capacity,
            "below_fraction": doctrine::TOP_UP_BELOW,
        }),
        Decision::Repair => json!({
            "code": if ship.leased { "repair.free-under-lease" } else { "repair.worn" },
            "wear_bps": ship.wear_bps,
            "threshold_bps": if ship.leased {
                doctrine::REPAIR_LEASED_AT_BPS
            } else {
                doctrine::REPAIR_TITLED_AT_BPS
            },
            "invoice": if ship.leased { 0 } else {
                ship.wear_bps * ship.repair_per_hundred_bps.max(1) / 100
            },
        }),
        Decision::CallPaws => {
            let nearest = pumps
                .iter()
                .filter_map(|p| router.fuel_between(&here, p).map(|f| (f, p.clone())))
                .min();
            json!({
                "code": "rescue.no-pump-in-reach",
                "fuel": ship.fuel, "fuel_capacity": ship.fuel_capacity,
                "critical_fraction": doctrine::CRITICAL_FUEL,
                "nearest_pump": nearest.as_ref().map(|(_, p)| p.clone()),
                "nearest_pump_fuel_at_reference": nearest.map(|(f, _)| f),
            })
        }
        Decision::DivertToPump { pump, burn_bps } => {
            let quoted = router.quote_at_burn(&here, pump, *burn_bps);
            json!({
                "code": if quoted.is_some() { "fuel.pump-in-reach.world-priced" }
                        else { "fuel.pump-in-reach.modelled" },
                "pump": pump, "burn": doctrine::burn_wire_name(*burn_bps).unwrap_or("standard"),
                "burn_bps": burn_bps,
                "fuel_needed": quoted.map(|(f, _)| f),
                "ticks": quoted.map(|(_, t)| t),
                "tank": ship.fuel, "reserve": 1.1,
            })
        }
        Decision::Book { load_id } => {
            let l = board.iter().find(|l| &l.load_id == load_id);
            let rate = |l: &LoadRow| l.estimated_net as f64 / l.pilot_ticks().max(1) as f64;
            // The chain's word decided when the booked load carries it AND another
            // bookable row paid a better rate — that row lost only to the tie-break.
            let chain_decided = l.is_some_and(|b| {
                b.chain_pressure > 0
                    && board
                        .iter()
                        .any(|o| o.load_id != b.load_id && !o.held_for_other && rate(o) > rate(b))
            });
            json!({
                "code": if chain_decided { "freight.chain-preferred" } else { "freight.best-net-per-tick" },
                "chain_pressure": l.map(|l| l.chain_pressure),
                "load_id": load_id,
                "estimated_net": l.map(|l| l.estimated_net),
                "deadhead_ticks": l.map(|l| l.deadhead_ticks),
                "haul_ticks": l.map(|l| l.haul_ticks),
                "deliver_deadline_tick": l.map(|l| l.deliver_deadline_tick),
                "tick": ship.tick,
                "candidates": board.len(),
            })
        }
        Decision::Travel { station } => json!({
            "code": match active.map(|a| a.word) {
                Some(ActiveWord::PickedUp) => "freight.laden-leg",
                Some(ActiveWord::Booked) => "freight.deadhead-to-origin",
                _ => "course.filed",
            },
            "station": station,
            "load_id": active.map(|a| a.row.load_id.clone()),
        }),
        Decision::Collect { load_id } => json!({
            "code": "freight.delivered-collect",
            "load_id": load_id,
        }),
    }
}

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
    // The captain's live contract rides as ITS OWN object — `active: {row, word}`
    // — because the open board does not contain it. A hull in transit was being
    // reduced to `active_load_id`, looked up on a board of open rows, not found,
    // and read as freight-idle: the iPad would hold or shop for a new load where
    // the host said travel or collect (codex T-237 B4 re-verification, finding
    // 2). `active_load_id` still works for a caller that also put the row on the
    // board, and `word` falls back to /v1/me.freight when the object omits it.
    let active = input
        .get("active")
        .and_then(|a| {
            let row = load_row(a.get("row")?)?;
            let word = match a.get("word").and_then(Value::as_str) {
                Some("delivered") => ActiveWord::Delivered,
                Some("pickedUp") | Some("picked-up") | Some("in-transit") => ActiveWord::PickedUp,
                Some("booked") => ActiveWord::Booked,
                _ => active_word(&me, &row.load_id).ok().flatten()?,
            };
            Some(Active { row, word })
        })
        .or_else(|| {
            input
                .get("active_load_id")
                .and_then(Value::as_str)
                .and_then(|lid| {
                    let row = board.iter().find(|l| l.load_id == lid)?.clone();
                    let word = active_word(&me, lid).ok().flatten()?;
                    Some(Active { row, word })
                })
        });
    let dial = input
        .get("dial")
        .map(|d| Dial::parse(&d.to_string()))
        .unwrap_or_default();
    let decision = doctrine::decide(&ship, active.as_ref(), &board, &pumps, &router);
    let surface = surface_of(&decision);
    let reasons = explain(&decision, &ship, active.as_ref(), &board, &pumps, &router);
    json!({
        "seam_version": SEAM_VERSION,
        "doctrine_build": env!("CARGO_PKG_VERSION"),
        "decision": decision_json(&decision),
        "reasons": reasons,
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

#[cfg(test)]
mod seam_parity_tests {
    use super::*;

    /// The exact facts codex probed (T-237 B4 re-verification, finding 1): a 188 mG
    /// hull at titania-cold-store on 123 of fuel, foxy's-diner the pump, the route
    /// quoted 168 at reference over legs [3491917000, 1774626], and the exchange
    /// pricing THIS hull at standard (171, 67) and economy (114, 95).
    fn probe(with_rungs: bool) -> Value {
        let mut route = json!({
            "from": "titania-cold-store", "to": "foxys-diner",
            "fuel": 168, "legs_km": [3_491_917_000_i64, 1_774_626]
        });
        if with_rungs {
            route["rungs"] = json!({
                "standard": {"fuel": 171, "ticks": 67},
                "economy":  {"fuel": 114, "ticks": 95}
            });
        }
        json!({
            "me": {"docked": "titania-cold-store", "fuel": 123, "fuelCapacity": 600,
                   "credits": 0, "effectiveAccelMilliG": 188, "wearBps": 0,
                   "titled": false, "leasePrincipal": 25000, "holdCapacity": 160,
                   "holdUsed": 0, "route": [], "tick": 8000},
            "board": [],
            "stations": [{"id": "foxys-diner", "sellsFuel": true}],
            "routes": [route],
        })
    }

    /// With the world's rung prices in the seam, the iPad reaches the HOST's
    /// verdict: economy needs 114 × 1.1 = 125 and the tank holds 123, so no pump
    /// is in reach and the tanker is right. Without them the seam models 112,
    /// rounds its reserve to 123, and diverts — the same facts, a different
    /// decision, at a safety boundary. That is the disagreement B4 exists to
    /// forbid, and the seam now carries the fact that decides it.
    #[test]
    fn the_seam_reaches_the_hosts_verdict_at_the_reachability_boundary() {
        let out = advise(&probe(true));
        assert_eq!(out["decision"]["type"], "call-paws", "{out}");
        assert_eq!(out["reasons"]["code"], "rescue.no-pump-in-reach");
        assert_eq!(out["seam_version"], SEAM_VERSION);
        assert!(out["doctrine_build"]
            .as_str()
            .is_some_and(|b| !b.is_empty()));

        // And the thing the rungs changed, pinned so nobody removes them "because
        // the model agrees": without them the seam models its way to a divert.
        let modelled = advise(&probe(false));
        assert_eq!(modelled["decision"]["type"], "divert-to-pump", "{modelled}");
        assert_eq!(modelled["reasons"]["code"], "fuel.pump-in-reach.modelled");
    }

    /// A hull with a live contract that is NOT on the open board — which is every
    /// hull in transit — is still under that contract (finding 2). Reduced to an
    /// id and looked up on open rows, it read as freight-idle.
    #[test]
    fn the_active_load_rides_as_its_own_object() {
        let row = json!({"loadId": "L3249", "origin": "a", "dest": "b", "good": "catnip",
                         "units": 25, "estimatedNet": 400, "deadheadTicks": 0,
                         "haulTicks": 10, "loadingTicks": 8, "serviceClass": "standard"});
        let input = json!({
            "me": {"docked": "a", "fuel": 500, "fuelCapacity": 600, "credits": 5000,
                   "effectiveAccelMilliG": 189, "wearBps": 0, "titled": false,
                   "leasePrincipal": 25000, "holdCapacity": 160, "holdUsed": 25,
                   "route": [], "tick": 100, "freight": []},
            "board": [],                       // open board: the contract is not here
            "stations": [{"id": "a", "sellsFuel": true}],
            "routes": [{"from": "a", "to": "b", "fuel": 10, "legs_km": [1_000_000]}],
            "active": {"row": row, "word": "pickedUp"},
        });
        let out = advise(&input);
        assert_eq!(out["decision"]["type"], "travel", "{out}");
        assert_eq!(out["decision"]["station"], "b");
        assert_eq!(out["reasons"]["code"], "freight.laden-leg");
        assert_eq!(out["reasons"]["load_id"], "L3249");
    }

    /// Every actionable decision explains itself with a code and the numbers that
    /// chose it — not only Hold (finding 4).
    #[test]
    fn actionable_decisions_carry_reasons() {
        // Repair: leased, worn past the free-repair line.
        let repair = advise(&json!({
            "me": {"docked": "a", "fuel": 590, "fuelCapacity": 600, "credits": 100,
                   "effectiveAccelMilliG": 189, "wearBps": 5000, "titled": false,
                   "leasePrincipal": 25000, "holdCapacity": 160, "holdUsed": 0,
                   "route": [], "tick": 1},
            "board": [], "stations": [{"id": "a", "sellsFuel": true}], "routes": []
        }));
        assert_eq!(repair["decision"]["type"], "repair", "{repair}");
        assert_eq!(repair["reasons"]["code"], "repair.free-under-lease");
        assert_eq!(repair["reasons"]["wear_bps"], 5000);
        assert_eq!(repair["reasons"]["invoice"], 0);

        // Book: one good load in reach, priced against its deadline.
        let book = advise(&json!({
            "me": {"docked": "a", "fuel": 600, "fuelCapacity": 600, "credits": 5000,
                   "effectiveAccelMilliG": 189, "wearBps": 0, "titled": false,
                   "leasePrincipal": 25000, "holdCapacity": 160, "holdUsed": 0,
                   "route": [], "tick": 1000},
            "board": [{"loadId": "L1", "origin": "a", "dest": "b", "good": "catnip",
                       "units": 25, "estimatedNet": 900, "deadheadTicks": 0,
                       "haulTicks": 10, "loadingTicks": 8, "serviceClass": "standard",
                       "deliverDeadlineTick": 1100}],
            "stations": [{"id": "a", "sellsFuel": true}],
            // The plan must reach a pump AFTER delivery too, so the way home is priced.
            "routes": [{"from": "a", "to": "b", "fuel": 10, "legs_km": [1_000_000]},
                       {"from": "b", "to": "a", "fuel": 10, "legs_km": [1_000_000]}]
        }));
        assert_eq!(book["decision"]["type"], "book", "{book}");
        assert_eq!(book["reasons"]["code"], "freight.best-net-per-tick");
        assert_eq!(book["reasons"]["estimated_net"], 900);
        assert_eq!(book["reasons"]["deliver_deadline_tick"], 1100);
    }

    /// The chain's word rides the seam as `chain_pressure` on a board row (T-238
    /// finding 3, freight half): a near-tie goes to the load that feeds a starving
    /// works and the reasons say so; a row without the field is a row with no word,
    /// so a client that cannot model the chain still parses and still books.
    #[test]
    fn chain_pressure_rides_the_seam_and_breaks_only_near_ties() {
        let me = json!({"docked": "a", "fuel": 600, "fuelCapacity": 600, "credits": 5000,
                        "effectiveAccelMilliG": 189, "wearBps": 0, "titled": false,
                        "leasePrincipal": 25000, "holdCapacity": 160, "holdUsed": 0,
                        "route": [], "tick": 1000});
        let routes = json!([{"from": "a", "to": "b", "fuel": 10, "legs_km": [1_000_000]},
                            {"from": "b", "to": "a", "fuel": 10, "legs_km": [1_000_000]}]);
        let row = |id: &str, net: i64, pressure: Option<i64>| {
            let mut r = json!({"loadId": id, "origin": "a", "dest": "b", "good": "catnip",
                               "units": 25, "estimatedNet": net, "deadheadTicks": 0,
                               "haulTicks": 10, "loadingTicks": 8, "serviceClass": "standard"});
            if let Some(p) = pressure {
                r["chain_pressure"] = json!(p);
            }
            r
        };
        let advise_with = |board: Value| {
            advise(&json!({"me": me, "board": board,
                           "stations": [{"id": "a", "sellsFuel": true}, {"id": "b", "sellsFuel": true}],
                           "routes": routes}))
        };
        // Near-tie, the chain's word on the lesser: it books, and says why.
        let out = advise_with(json!([row("L1", 900, None), row("L2", 873, Some(1))]));
        assert_eq!(out["decision"]["load_id"], "L2", "{out}");
        assert_eq!(out["reasons"]["code"], "freight.chain-preferred");
        assert_eq!(out["reasons"]["chain_pressure"], 1);
        // Materially worse: money wins.
        let out = advise_with(json!([row("L1", 900, None), row("L3", 810, Some(1))]));
        assert_eq!(out["decision"]["load_id"], "L1", "{out}");
        assert_eq!(out["reasons"]["code"], "freight.best-net-per-tick");
        // No word anywhere (a chain-less client): the best rate, as before.
        let out = advise_with(json!([row("L1", 900, None), row("L2", 873, None)]));
        assert_eq!(out["decision"]["load_id"], "L1", "{out}");
        assert_eq!(out["reasons"]["code"], "freight.best-net-per-tick");
    }
}
