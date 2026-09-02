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

/// One active load, as the caller tracks it. Under an itinerary this is one STOP
/// of the plan — the name keeps its history.
#[derive(Debug, Clone)]
pub struct Active {
    pub row: LoadRow,
    pub word: ActiveWord,
}

/// The ordered plan: every contract the ship is committed to, in booking order.
/// Today's exchange enforces one contract, so the live plan holds zero or one stop —
/// but the STRUCTURE carries any number, so the day UCF-Haul#43 lifts the cap
/// (multi-load, multi-stop freight) nothing here breaks or migrates (T-232).
#[derive(Debug, Clone, Default)]
pub struct Itinerary {
    pub stops: Vec<Active>,
}

impl Itinerary {
    /// The degenerate plan today's world flies: one contract, or none.
    pub fn single(active: Option<Active>) -> Self {
        Itinerary {
            stops: active.into_iter().collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.stops.is_empty()
    }

    /// Units the plan still owes hold space: everything not yet delivered.
    /// (Delivered cargo has left the hold; its money is parked on the contract.)
    pub fn units_held(&self) -> i64 {
        self.stops
            .iter()
            .filter(|s| s.word != ActiveWord::Delivered)
            .map(|s| s.row.units)
            .sum()
    }

    /// The stop the ship is working toward: the first not-yet-delivered contract,
    /// in plan order. Delivered stops wait only on a Collect, not on movement.
    pub fn current(&self) -> Option<&Active> {
        self.stops.iter().find(|s| s.word != ActiveWord::Delivered)
    }
}

/// The judgment over a one-contract world, exactly as it always was. This is the
/// degenerate case of [`decide_plan`] — kept as the front door so every rule the
/// LOCAL and PROD incidents taught stays pinned by its original test, unmodified.
pub fn decide(
    ship: &Ship,
    active: Option<&Active>,
    board: &[LoadRow],
    pumps: &BTreeSet<String>,
    router: &dyn Router,
) -> Decision {
    decide_plan(
        ship,
        &Itinerary::single(active.cloned()),
        board,
        pumps,
        router,
    )
}

/// The judgment. Facts in, one decision out — now over an ordered PLAN of stops.
///
/// With zero or one stop this reduces to the judgment whisker has flown since
/// LOCAL — the wrapper above pins that. With more (once the exchange allows it):
/// delivered money is collected first, then the ship works the plan in order,
/// navigating by each stop's own ledger word. One deliberate divergence from the
/// single-stop rules, stated rather than smoothed: the single-contract case reads
/// "cargo aboard" off the aggregate `hold_used` (the crane proxy the fold shows);
/// a multi-stop plan cannot attribute aggregate units to one contract, so there
/// the stop's reconciled word (`PickedUp`) is the evidence. UCF-Haul#43's API may
/// hand us per-load hold rows — revisit this seam when its shape lands.
pub fn decide_plan(
    ship: &Ship,
    plan: &Itinerary,
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

    if !plan.is_empty() {
        // Money first: a delivered contract never pays itself, and collecting
        // costs no movement.
        if let Some(done) = plan.stops.iter().find(|s| s.word == ActiveWord::Delivered) {
            return Decision::Collect {
                load_id: done.row.load_id.clone(),
            };
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
        // Every stop here is Booked or PickedUp, so current() is Some.
        let Some(cur) = plan.current() else {
            return Decision::Hold {
                why: "waiting on the crane".into(),
            };
        };
        // Cargo evidence: the single-contract crane proxy (aggregate hold), or —
        // when the plan holds more than one contract — the stop's own word.
        let laden = if plan.stops.len() == 1 {
            ship.hold_used > 0
        } else {
            cur.word == ActiveWord::PickedUp
        };
        if here == cur.row.origin && laden {
            return Decision::Travel {
                station: cur.row.dest.clone(),
            };
        }
        if here != cur.row.origin && here != cur.row.dest && cur.word == ActiveWord::Booked {
            return Decision::Travel {
                station: cur.row.origin.clone(),
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
    //
    // Booking rides an EMPTY plan only while the exchange enforces one contract —
    // filing a second would just be refused at the door. When UCF-Haul#43 lifts
    // the cap, this gate widens to best_insertion over the live plan; the ranking
    // below already speaks that shape.
    if let Some(load_id) = best_insertion(ship, plan, board, pumps, router) {
        return Decision::Book { load_id };
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

/// Which open load best joins the plan — appended at its end, the minimal honest
/// insertion heuristic (never a solver). Candidates that fit the hold beside the
/// plan's own cargo are ranked by ℳ per tick of pilot time — with an empty plan
/// that IS today's per-load rate, and appending changes no other stop's earnings,
/// so the same number is the marginal rate — and the best one whose whole
/// remaining journey the tank (plus pumps along the way) can actually fuel wins.
pub fn best_insertion(
    ship: &Ship,
    plan: &Itinerary,
    board: &[LoadRow],
    pumps: &BTreeSet<String>,
    router: &dyn Router,
) -> Option<String> {
    let here = ship.docked.as_deref()?;
    let held = plan.units_held();
    let mut ranked: Vec<&LoadRow> = board
        .iter()
        .filter(|l| !l.held_for_other && held + l.units <= ship.hold_capacity)
        .filter(|l| l.pilot_ticks() > 0 && l.estimated_net > 0)
        .collect();
    ranked.sort_by(|a, b| {
        let ra = a.estimated_net as f64 / a.pilot_ticks() as f64;
        let rb = b.estimated_net as f64 / b.pilot_ticks() as f64;
        rb.partial_cmp(&ra).unwrap_or(std::cmp::Ordering::Equal)
    });
    for l in ranked.into_iter().take(PRICED_CANDIDATES) {
        let mut legs = plan_legs(plan, here);
        let last = legs.last().map(|(_, to)| to.clone());
        let pos = last.as_deref().unwrap_or(here);
        legs.push((pos.to_string(), l.origin.clone()));
        legs.push((l.origin.clone(), l.dest.clone()));
        if plan_fuelable(ship, &legs, pumps, router) {
            return Some(l.load_id.clone());
        }
    }
    None
}

/// The plan's remaining flying, as (from, to) legs starting wherever the ship is:
/// a Booked stop still owes the deadhead to its origin and the haul; a PickedUp
/// stop owes only the haul. Delivered stops owe no movement.
fn plan_legs(plan: &Itinerary, here: &str) -> Vec<(String, String)> {
    let mut legs = Vec::new();
    let mut pos = here.to_string();
    for stop in &plan.stops {
        match stop.word {
            ActiveWord::Delivered => {}
            ActiveWord::Booked => {
                legs.push((pos.clone(), stop.row.origin.clone()));
                legs.push((stop.row.origin.clone(), stop.row.dest.clone()));
                pos = stop.row.dest.clone();
            }
            ActiveWord::PickedUp => {
                legs.push((pos.clone(), stop.row.dest.clone()));
                pos = stop.row.dest.clone();
            }
        }
    }
    legs
}

/// Can the tank fly this whole leg sequence? The walk splits at fuel-selling
/// stations — arriving at a pump refills to capacity — so the check is: the FIRST
/// stretch (before any pump) fits the tank as it stands, and every later stretch
/// fits a full tank, each with the same reserve arithmetic the single-load rule
/// has always used. A leg the router cannot price is a route the doctrine will
/// not risk. With one candidate load and an empty plan this reduces exactly to
/// the original pair of checks: `(dead+haul)·R ≤ fuel`, or a fuel-selling origin
/// with `dead·R ≤ fuel` and `haul·R ≤ capacity`.
fn plan_fuelable(
    ship: &Ship,
    legs: &[(String, String)],
    pumps: &BTreeSet<String>,
    router: &dyn Router,
) -> bool {
    let mut budget = ship.fuel;
    let mut stretch: i64 = 0;
    for (from, to) in legs {
        let Some(cost) = router.fuel_between(from, to) else {
            return false;
        };
        stretch += cost;
        if pumps.contains(to.as_str()) {
            if budget < (stretch as f64 * RESERVE) as i64 {
                return false;
            }
            budget = ship.fuel_capacity;
            stretch = 0;
        }
    }
    stretch == 0 || budget >= (stretch as f64 * RESERVE) as i64
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

    // ——— T-232: the itinerary. Everything below exercises the plan layer over a
    // stubbed multi-load world; the eleven tests above, unmodified, pin that the
    // one-contract world still flies byte-for-byte through it.

    fn stop(id: &str, origin: &str, dest: &str, word: ActiveWord) -> Active {
        Active {
            row: load(id, origin, dest, 500, (5, 10)),
            word,
        }
    }

    #[test]
    fn the_wrapper_is_the_degenerate_plan_none_and_one_stop_agree() {
        let ship = ship_at("tuna-prime", 500);
        let board = [load("L1", "a", "b", 900, (5, 10))];
        let one = stop("L2", "a", "b", ActiveWord::Booked);
        for active in [None, Some(&one)] {
            assert_eq!(
                decide(&ship, active, &board, &pumps(&["a"]), &FlatRouter(10)),
                decide_plan(
                    &ship,
                    &Itinerary::single(active.cloned()),
                    &board,
                    &pumps(&["a"]),
                    &FlatRouter(10)
                )
            );
        }
    }

    #[test]
    fn delivered_money_is_collected_before_the_rest_of_the_plan_moves() {
        // Two contracts held: one delivered, one booked. The payout outranks the
        // deadhead — collecting costs no movement.
        let ship = ship_at("elsewhere", 500);
        let plan = Itinerary {
            stops: vec![
                stop("EARN", "a", "b", ActiveWord::Delivered),
                stop("NEXT", "c", "d", ActiveWord::Booked),
            ],
        };
        let d = decide_plan(&ship, &plan, &[], &pumps(&[]), &FlatRouter(10));
        assert_eq!(
            d,
            Decision::Collect {
                load_id: "EARN".into()
            }
        );
    }

    #[test]
    fn the_plan_is_worked_in_order_first_undelivered_stop_steers() {
        // Docked away from everything, first stop booked: deadhead to ITS origin,
        // not the second stop's.
        let ship = ship_at("tuna-prime", 500);
        let plan = Itinerary {
            stops: vec![
                stop("FIRST", "a", "b", ActiveWord::Booked),
                stop("SECOND", "c", "d", ActiveWord::Booked),
            ],
        };
        let d = decide_plan(&ship, &plan, &[], &pumps(&[]), &FlatRouter(10));
        assert_eq!(
            d,
            Decision::Travel {
                station: "a".into()
            }
        );
    }

    #[test]
    fn a_multi_stop_plan_reads_cargo_from_the_stops_own_word() {
        // At the first stop's origin with its cargo aboard (per the ledger), the
        // laden leg files — even though the aggregate hold can't say WHOSE units
        // those are. The single-stop crane proxy (hold_used) stays pinned by
        // a_full_hold_at_the_origin_files_the_laden_leg above.
        let mut ship = ship_at("a", 500);
        ship.hold_used = 0; // the aggregate lies; the word doesn't
        let plan = Itinerary {
            stops: vec![
                stop("FIRST", "a", "b", ActiveWord::PickedUp),
                stop("SECOND", "c", "d", ActiveWord::Booked),
            ],
        };
        let d = decide_plan(&ship, &plan, &[], &pumps(&[]), &FlatRouter(10));
        assert_eq!(
            d,
            Decision::Travel {
                station: "b".into()
            }
        );
    }

    #[test]
    fn insertion_packs_the_hold_a_load_that_does_not_fit_beside_the_plan_waits() {
        // 120-unit hold, 100 units committed: the fat lucrative load can't ride,
        // the thin one can.
        let ship = ship_at("here", 600);
        let mut committed = stop("HELD", "x", "y", ActiveWord::PickedUp);
        committed.row.units = 100;
        let plan = Itinerary {
            stops: vec![committed],
        };
        let mut fat = load("FAT", "a", "b", 10_000, (5, 10));
        fat.units = 40;
        let mut thin = load("THIN", "a", "b", 400, (5, 10));
        thin.units = 20;
        let picked = best_insertion(&ship, &plan, &[fat, thin], &pumps(&[]), &FlatRouter(10));
        assert_eq!(picked, Some("THIN".into()));
    }

    #[test]
    fn insertion_fuels_the_whole_remaining_plan_not_just_the_candidate() {
        // The candidate's own legs are cheap — but the plan still owes a long
        // laden leg first, and the shared tank must carry all of it.
        struct PerLeg;
        impl Router for PerLeg {
            fn fuel_between(&self, from: &str, to: &str) -> Option<i64> {
                Some(match (from, to) {
                    ("x", "far-y") => 400, // the plan's own laden leg
                    _ => 50,
                })
            }
        }
        let ship = ship_at("x", 500); // covers 400+50+50 only without reserve
        let plan = Itinerary {
            stops: vec![stop("HELD", "w", "far-y", ActiveWord::PickedUp)],
        };
        let cheap = load("CHEAP", "a", "b", 900, (5, 10));
        let picked = best_insertion(&ship, &plan, std::slice::from_ref(&cheap), &pumps(&[]), &PerLeg);
        assert_eq!(picked, None, "600 of fuel cannot fly 500·1.2 of legs");
        // A pump at the plan's own destination resets the tank — now it books.
        let picked = best_insertion(&ship, &plan, &[cheap], &pumps(&["far-y"]), &PerLeg);
        assert_eq!(picked, Some("CHEAP".into()));
    }

    #[test]
    fn plan_fuel_walks_booked_stops_as_deadhead_plus_haul() {
        // A booked (not picked-up) stop owes BOTH its legs before the candidate.
        struct PerLeg;
        impl Router for PerLeg {
            fn fuel_between(&self, _: &str, _: &str) -> Option<i64> {
                Some(100)
            }
        }
        let ship = ship_at("here", 500);
        let plan = Itinerary {
            stops: vec![stop("HELD", "a", "b", ActiveWord::Booked)],
        };
        // Plan legs: here→a, a→b (200), candidate: b→c, c→d (200): 400·1.2 = 480 ≤ 500.
        let ok = load("OK", "c", "d", 900, (5, 10));
        assert_eq!(
            best_insertion(&ship, &plan, &[ok], &pumps(&[]), &PerLeg),
            Some("OK".into())
        );
        // One more unpriced leg anywhere refuses the whole plan.
        struct OneBlind;
        impl Router for OneBlind {
            fn fuel_between(&self, from: &str, _: &str) -> Option<i64> {
                if from == "a" {
                    None
                } else {
                    Some(100)
                }
            }
        }
        let ok = load("OK", "c", "d", 900, (5, 10));
        assert_eq!(
            best_insertion(&ship, &plan, &[ok], &pumps(&[]), &OneBlind),
            None
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
