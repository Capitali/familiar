//! The pilot's judgment, pure. No socket, no clock, no store — facts in, one
//! [`Decision`] out — so every rule learned the hard way is pinned by a test that
//! needs no world to run against.
//!
//! The rules and where each was learned (LOCAL world, 2026-08-31):
//! - **One intent at a time, and the fold is the truth.** An accepted action can take
//!   several folds to SHOW; re-filing the same intent is how the ship zigzagged its
//!   tank empty. (The caller owns dedupe/pacing; decide() is deliberately stateless.)
//! - **Fuel is planned before a booking, not discovered after.** The exchange's own
//!   router prices propellant per leg; a load is takeable only if the tank covers
//!   deadhead + haul with reserve, or the origin sells fuel and the tank covers the
//!   deadhead with the CAPACITY covering the haul.
//! - **Pump fuel over tanker fuel.** Top up whenever berthed at a seller below 90%.
//! - **PAWS is the floor, not a plan.** Under 5% the only right move is the tanker —
//!   double price and a wait beats a drifting hull (confirmed by the owner from PROD).
//! - **No pump at this berth is a routing fact**, learned when six refuels in a row
//!   were rejected at a cold store: divert to the cheapest-to-reach seller.

use std::collections::BTreeSet;

/// Our hull as the last fold showed it (a subset of `/v1/me`).
#[derive(Debug, Clone, Default)]
pub struct Ship {
    /// Berthed station id, or None under way.
    pub docked: Option<String>,
    /// True when a course is filed / legs remain — the engine is flying us.
    pub in_flight: bool,
    pub hold_used: i64,
    pub hold_capacity: i64,
    pub fuel: i64,
    pub fuel_capacity: i64,
    pub credits: i64,
}

/// One row of the open load board (a subset of `/v1/loadboard`).
#[derive(Debug, Clone, Default)]
pub struct LoadRow {
    pub load_id: String,
    pub origin: String,
    pub dest: String,
    pub units: i64,
    pub estimated_net: i64,
    pub deadhead_ticks: i64,
    pub haul_ticks: i64,
    pub loading_ticks: i64,
    pub held_for_other: bool,
}

impl LoadRow {
    /// Total ticks of the PILOT's time: reposition + haul + handling at both ends.
    /// Pay divided by this — never by flight time alone — is the honest rate
    /// (the headless guide's own arithmetic).
    pub fn pilot_ticks(&self) -> i64 {
        self.deadhead_ticks + self.haul_ticks + 2 * self.loading_ticks.max(8)
    }
}

/// Route costs, answered by whoever holds a wire to the exchange. Pure tests stub it;
/// the runner asks `/v1/route`. `None` means the router could not say — and a route
/// nobody can price is a route the doctrine will not risk.
pub trait Router {
    fn fuel_between(&self, from: &str, to: &str) -> Option<i64>;
}

/// What the pilot wants to do next. Every consequential variant names the
/// [`crate::Automation`] it exercises via [`Decision::automation`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Nothing to do this fold (with the reason, for the journal).
    Hold { why: String },
    /// Top up at this berth's pump.
    Refuel,
    /// Call the PAWS tanker — expensive, never terminal.
    CallPaws,
    /// Fly empty to a fuel seller.
    DivertToPump { pump: String },
    /// Book this load.
    Book { load_id: String },
    /// File a course (deadhead to origin, or the laden leg to dest).
    Travel { station: String },
    /// File for the money on a delivered contract — it never pays itself.
    Collect { load_id: String },
}

impl Decision {
    /// The automation a decision spends, or None for a Hold. The runner refuses any
    /// decision whose automation the ship store does not grant — the pay-per-feature
    /// gate (Ian, 2026-08-31), enforced at the same rung as the lease.
    pub fn automation(&self) -> Option<crate::Automation> {
        match self {
            Decision::Hold { .. } => None,
            _ => Some(crate::Automation::Freight),
        }
    }
}

/// The reserve margin over priced fuel: routes are honest but the world moves.
const RESERVE: f64 = 1.2;
/// Below this fraction of capacity, a berthed ship with a pump tops up.
const TOP_UP_BELOW: f64 = 0.9;
/// Below this fraction, an idle ship diverts to a pump before taking work.
const LOW_FUEL: f64 = 0.4;
/// Below this fraction, nothing matters but the tanker.
const CRITICAL_FUEL: f64 = 0.05;
/// How many top board rows get route-priced. A route call per row would be impolite.
const PRICED_CANDIDATES: usize = 5;

/// The word the ledger last said about our active load, reduced to what decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveWord {
    /// Booked; we have not yet seen pickup.
    Booked,
    /// The hold has our cargo (the origin's work is done).
    PickedUp,
    /// Delivered — the money is parked on the contract until collected.
    Delivered,
}

/// One active load, as the caller tracks it.
#[derive(Debug, Clone)]
pub struct Active {
    pub row: LoadRow,
    pub word: ActiveWord,
}

/// The judgment. Facts in, one decision out.
pub fn decide(
    ship: &Ship,
    active: Option<&Active>,
    board: &[LoadRow],
    pumps: &BTreeSet<String>,
    router: &dyn Router,
) -> Decision {
    let frac = |n: i64| n as f64 / ship.fuel_capacity.max(1) as f64;

    // The tanker outranks everything — it is the only act that works everywhere,
    // including under way, and a dry hull can execute nothing else anyway.
    if frac(ship.fuel) < CRITICAL_FUEL {
        return Decision::CallPaws;
    }

    if let Some(active) = active {
        match active.word {
            ActiveWord::Delivered => {
                return Decision::Collect {
                    load_id: active.row.load_id.clone(),
                }
            }
            ActiveWord::PickedUp | ActiveWord::Booked => {}
        }
        if ship.in_flight {
            return Decision::Hold {
                why: "under way".into(),
            };
        }
        let Some(here) = ship.docked.as_deref() else {
            return Decision::Hold {
                why: "adrift between folds".into(),
            };
        };
        if here == active.row.origin && ship.hold_used > 0 {
            return Decision::Travel {
                station: active.row.dest.clone(),
            };
        }
        if here != active.row.origin && here != active.row.dest && active.word == ActiveWord::Booked
        {
            return Decision::Travel {
                station: active.row.origin.clone(),
            };
        }
        return Decision::Hold {
            why: "waiting on the crane".into(),
        };
    }

    if ship.in_flight {
        return Decision::Hold {
            why: "under way, no load".into(),
        };
    }
    let Some(here) = ship.docked.as_deref() else {
        return Decision::Hold {
            why: "adrift between folds".into(),
        };
    };

    // Fuel before work when it costs nothing: berthed at a seller, top up.
    if pumps.contains(here) && frac(ship.fuel) < TOP_UP_BELOW {
        return Decision::Refuel;
    }

    // Work: best net per tick of pilot time, dock included — of what we can fuel.
    // Deliberately BEFORE the low-fuel diversion: every plan below carries its own
    // reserve, and a load whose fuel-selling origin is reachable earns on the way
    // to the pump a bare diversion would fly for free.
    let mut ranked: Vec<&LoadRow> = board
        .iter()
        .filter(|l| !l.held_for_other && l.units <= ship.hold_capacity)
        .filter(|l| l.pilot_ticks() > 0 && l.estimated_net > 0)
        .collect();
    ranked.sort_by(|a, b| {
        let ra = a.estimated_net as f64 / a.pilot_ticks() as f64;
        let rb = b.estimated_net as f64 / b.pilot_ticks() as f64;
        rb.partial_cmp(&ra).unwrap_or(std::cmp::Ordering::Equal)
    });
    for l in ranked.into_iter().take(PRICED_CANDIDATES) {
        let (Some(dead), Some(haul)) = (
            router.fuel_between(here, &l.origin),
            router.fuel_between(&l.origin, &l.dest),
        ) else {
            continue;
        };
        let whole = ((dead + haul) as f64 * RESERVE) as i64;
        let dead_only = (dead as f64 * RESERVE) as i64;
        let haul_only = (haul as f64 * RESERVE) as i64;
        if ship.fuel >= whole
            || (pumps.contains(l.origin.as_str())
                && ship.fuel >= dead_only
                && ship.fuel_capacity >= haul_only)
        {
            return Decision::Book {
                load_id: l.load_id.clone(),
            };
        }
    }

    // No fuel-sound work. If the tank is the reason, go stand at a pump — but only
    // one THE TANK CAN REACH: the engine refuses an unaffordable route at the fold
    // ("route needs about 217 in the tank and it holds 157"), so filing one is a
    // slow way to stand still. No affordable pump means the tanker, at ANY level.
    if frac(ship.fuel) < LOW_FUEL {
        let mut best: Option<(i64, &String)> = None;
        for p in pumps {
            if let Some(cost) = router.fuel_between(here, p) {
                if (cost as f64 * 1.1) as i64 <= ship.fuel
                    && best.map(|(c, _)| cost < c).unwrap_or(true)
                {
                    best = Some((cost, p));
                }
            }
        }
        return match best {
            Some((_, pump)) => Decision::DivertToPump { pump: pump.clone() },
            None => Decision::CallPaws,
        };
    }
    Decision::Hold {
        why: "no fuelable work on the board".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FlatRouter(i64);
    impl Router for FlatRouter {
        fn fuel_between(&self, _: &str, _: &str) -> Option<i64> {
            Some(self.0)
        }
    }
    struct NoRouter;
    impl Router for NoRouter {
        fn fuel_between(&self, _: &str, _: &str) -> Option<i64> {
            None
        }
    }

    fn ship_at(station: &str, fuel: i64) -> Ship {
        Ship {
            docked: Some(station.into()),
            in_flight: false,
            hold_used: 0,
            hold_capacity: 120,
            fuel,
            fuel_capacity: 600,
            credits: 10_000,
        }
    }

    fn load(id: &str, origin: &str, dest: &str, net: i64, ticks: (i64, i64)) -> LoadRow {
        LoadRow {
            load_id: id.into(),
            origin: origin.into(),
            dest: dest.into(),
            units: 25,
            estimated_net: net,
            deadhead_ticks: ticks.0,
            haul_ticks: ticks.1,
            loading_ticks: 8,
            held_for_other: false,
        }
    }

    fn pumps(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_nearly_dry_hull_calls_the_tanker_before_all_else() {
        // Even mid-flight with a delivered load waiting: dry ships execute nothing.
        let mut ship = ship_at("cannery-row", 20);
        ship.in_flight = true;
        let active = Active {
            row: load("L1", "a", "b", 500, (5, 10)),
            word: ActiveWord::Delivered,
        };
        let d = decide(&ship, Some(&active), &[], &pumps(&[]), &NoRouter);
        assert_eq!(d, Decision::CallPaws);
    }

    #[test]
    fn delivered_money_is_collected_it_never_pays_itself() {
        let ship = ship_at("b", 500);
        let active = Active {
            row: load("L1", "a", "b", 500, (5, 10)),
            word: ActiveWord::Delivered,
        };
        let d = decide(&ship, Some(&active), &[], &pumps(&[]), &FlatRouter(10));
        assert_eq!(
            d,
            Decision::Collect {
                load_id: "L1".into()
            }
        );
    }

    #[test]
    fn a_full_hold_at_the_origin_files_the_laden_leg() {
        let mut ship = ship_at("a", 500);
        ship.hold_used = 25;
        let active = Active {
            row: load("L1", "a", "b", 500, (5, 10)),
            word: ActiveWord::PickedUp,
        };
        let d = decide(&ship, Some(&active), &[], &pumps(&[]), &FlatRouter(10));
        assert_eq!(
            d,
            Decision::Travel {
                station: "b".into()
            }
        );
    }

    #[test]
    fn booked_elsewhere_means_deadhead_to_the_origin() {
        let ship = ship_at("tuna-prime", 500);
        let active = Active {
            row: load("L1", "a", "b", 500, (5, 10)),
            word: ActiveWord::Booked,
        };
        let d = decide(&ship, Some(&active), &[], &pumps(&[]), &FlatRouter(10));
        assert_eq!(
            d,
            Decision::Travel {
                station: "a".into()
            }
        );
    }

    #[test]
    fn berthed_at_a_pump_below_ninety_percent_tops_up_first() {
        let ship = ship_at("foxys-diner", 500); // 83% — pump fuel is half tanker fuel
        let board = [load("L1", "foxys-diner", "b", 900, (0, 10))];
        let d = decide(
            &ship,
            None,
            &board,
            &pumps(&["foxys-diner"]),
            &FlatRouter(10),
        );
        assert_eq!(d, Decision::Refuel);
    }

    #[test]
    fn low_fuel_diverts_to_the_cheapest_priceable_pump() {
        struct ByName;
        impl Router for ByName {
            fn fuel_between(&self, _: &str, to: &str) -> Option<i64> {
                match to {
                    "near-pump" => Some(30),
                    "far-pump" => Some(200),
                    _ => Some(50),
                }
            }
        }
        let ship = ship_at("cold-store", 100); // 17%
        let d = decide(
            &ship,
            None,
            &[],
            &pumps(&["far-pump", "near-pump"]),
            &ByName,
        );
        assert_eq!(
            d,
            Decision::DivertToPump {
                pump: "near-pump".into()
            }
        );
    }

    #[test]
    fn an_unaffordable_pump_is_never_filed_the_tanker_is_called_instead() {
        // Every pump costs more to reach than the tank holds — the engine would
        // refuse the route at the fold, so the only honest move is the tanker.
        let ship = ship_at("tranquility", 157); // the live incident's numbers
        let d = decide(
            &ship,
            None,
            &[],
            &pumps(&["paws-neptune"]),
            &FlatRouter(217),
        );
        assert_eq!(d, Decision::CallPaws);
    }

    #[test]
    fn a_booking_must_carry_its_own_fuel_plan() {
        // The lucrative load needs 300+300 fuel; the tank holds 400: skipped in favour
        // of the modest one the ship can actually fuel. THE incident of 2026-08-31.
        struct PerLeg;
        impl Router for PerLeg {
            fn fuel_between(&self, from: &str, to: &str) -> Option<i64> {
                Some(match (from, to) {
                    (_, "rich-origin") | ("rich-origin", _) => 300,
                    _ => 40,
                })
            }
        }
        let ship = ship_at("here", 400);
        let board = [
            load("RICH", "rich-origin", "far", 10_000, (30, 60)),
            load("MODEST", "near", "next", 400, (5, 10)),
        ];
        let d = decide(&ship, None, &board, &pumps(&[]), &PerLeg);
        assert_eq!(
            d,
            Decision::Book {
                load_id: "MODEST".into()
            }
        );
    }

    #[test]
    fn a_fuel_selling_origin_lets_the_tank_be_filled_there() {
        // Can't carry the whole journey, but CAN reach the origin — and the origin
        // sells fuel, and a full tank covers the haul: takeable.
        struct PerLeg;
        impl Router for PerLeg {
            fn fuel_between(&self, from: &str, _to: &str) -> Option<i64> {
                Some(if from == "pump-origin" { 400 } else { 50 })
            }
        }
        let ship = ship_at("here", 100);
        let board = [load("L1", "pump-origin", "far", 900, (5, 40))];
        let d = decide(&ship, None, &board, &pumps(&["pump-origin"]), &PerLeg);
        assert_eq!(
            d,
            Decision::Book {
                load_id: "L1".into()
            }
        );
    }

    #[test]
    fn an_unpriceable_route_is_never_risked() {
        let ship = ship_at("here", 600);
        let board = [load("L1", "a", "b", 900, (5, 10))];
        let d = decide(&ship, None, &board, &pumps(&[]), &NoRouter);
        assert_eq!(
            d,
            Decision::Hold {
                why: "no fuelable work on the board".into()
            }
        );
    }

    #[test]
    fn every_consequential_decision_names_the_freight_automation() {
        for d in [
            Decision::Refuel,
            Decision::CallPaws,
            Decision::DivertToPump { pump: "p".into() },
            Decision::Book {
                load_id: "L".into(),
            },
            Decision::Travel {
                station: "s".into(),
            },
            Decision::Collect {
                load_id: "L".into(),
            },
        ] {
            assert_eq!(d.automation(), Some(crate::Automation::Freight));
        }
        assert_eq!(Decision::Hold { why: "x".into() }.automation(), None);
    }
}
