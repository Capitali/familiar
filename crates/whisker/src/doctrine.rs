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
    /// The drive as the hull actually delivers it, thousandths of a gravity
    /// (`effectiveAccelMilliG` on `/v1/me`): the rated 189 derated by wear —
    /// fully worn is half. KK II at 88% wear flies 105.
    pub accel_milli_g: i64,
    /// Wear, bps of fully worn (`wearBps`). Derates the drive; repair clears it.
    pub wear_bps: i64,
    /// True while the hull is U.C.F.'s iron on a lease (`titled` false with a
    /// `leasePrincipal`): the yard clears wear and bills nobody — upkeep is what
    /// the lease service charge buys. A titled hull pays `wearBps × 40 / 100`.
    pub leased: bool,
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
    /// What the contract carries — the merchant must not mistake it for its own goods.
    pub good: String,
    /// The contract's service class as a drive multiplier, bps: economy 5000 (half
    /// drive), standard 10000, express 20000, priority 30000. The class throttles
    /// or overdrives the hull on EVERY leg flown under the contract, the deadhead
    /// to pickup included (engine `FreightShips.accelMilliG`).
    pub class_bps: i64,
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
    /// The route's legs as separations in km, tonight's geometry (`distanceKm`
    /// per leg of `/v1/route`). `None` = the router could not say; the caller
    /// falls back to the board's own figure.
    fn leg_distances_km(&self, _from: &str, _to: &str) -> Option<Vec<i64>> {
        None
    }
}

/// km per tick² at the reference drive (engine `FlightModel.referenceK`).
const REFERENCE_K: i64 = 864_900;
/// The reference drive, thousandths of a gravity (`FlightModel.referenceAccelMilliG`).
pub const REFERENCE_ACCEL_MILLI_G: i64 = 189;
/// Folds between a booking and the drive actually engaging (file travel, then
/// engage on the next fold): counted against the pickup window.
const ENGAGE_OVERHEAD_TICKS: i64 = 4;

/// Flight time for legs of these separations at this drive, the engine's own
/// arithmetic (`FlightModel.travelTicks`): ticks = ⌈√(D / K)⌉ per leg, K linear in
/// acceleration. Pinned against PROD: cannery-row → titan-larder, 1,307,724,939 km,
/// is 39 ticks at 189 mg and 74–75 at the 52 mg an 88%-worn hull makes on an
/// economy contract.
pub fn flight_ticks(distances_km: &[i64], accel_milli_g: i64) -> i64 {
    let k = (REFERENCE_K * accel_milli_g.max(1) / REFERENCE_ACCEL_MILLI_G).max(1);
    distances_km
        .iter()
        .map(|&d| {
            if d <= 0 {
                return 1;
            }
            // ⌈√(d/k)⌉ in integers: the smallest t with t² ≥ d/k, i.e. t²·k ≥ d.
            let mut t = ((d as f64) / (k as f64)).sqrt().floor() as i64;
            while t * t * k < d {
                t += 1;
            }
            t.max(1)
        })
        .sum()
}

/// The drive a contract's legs are flown at: the hull throttled by the class.
pub fn contract_accel(ship_accel_milli_g: i64, class_bps: i64) -> i64 {
    (ship_accel_milli_g * class_bps / 10_000).max(1)
}

/// What the pilot wants to do next. Every consequential variant names the
/// [`crate::Automation`] it exercises via [`Decision::automation`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Nothing to do this fold (with the reason, for the journal).
    Hold { why: String },
    /// Top up at this berth's pump.
    Refuel,
    /// Clear the drive's wear at this berth (any berth repairs; all or nothing).
    Repair,
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
/// A leased hull repairs (free) from this wear on: 10% wear is 5% of drive.
const REPAIR_LEASED_AT_BPS: i64 = 1_000;
/// A titled hull repairs (paid) from this wear on: half worn is a quarter of drive.
const REPAIR_TITLED_AT_BPS: i64 = 5_000;
/// The yard's rate for a titled hull (`repairCostPerHundredBps`, the pack: 40).
const REPAIR_COST_PER_HUNDRED_BPS: i64 = 40;
/// Below this fraction of capacity, a berthed ship with a pump tops up.
const TOP_UP_BELOW: f64 = 0.9;
/// Below this fraction, an idle ship diverts to a pump before taking work.
const LOW_FUEL: f64 = 0.4;
/// Below this fraction, nothing matters but the tanker.
const CRITICAL_FUEL: f64 = 0.05;
/// How many top board rows get route-priced. A route call per row would be impolite.
const PRICED_CANDIDATES: usize = 5;
/// The desk reverts a booking not picked up within this many ticks of it
/// (`pickupTTLTicks` on `/v1/reference`; 48 on LOCAL and PROD, a revert penalty
/// with it). The board's `deadheadTicks` is not OUR deadhead — L2166 on LOCAL
/// advertised 19, the lane route ran 57 through foxys-diner and tuna-prime, and the
/// desk took it back at booking + 48 while we were still under way.
const PICKUP_TTL_TICKS: i64 = 48;

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
        // Booked and not at the origin: deadhead there — WHEREVER we are, the
        // destination included. The old rule excused the destination, so a hull
        // that booked a contract while berthed at its dest sat "waiting on the
        // crane" until the desk let the booking lapse: KK II at foxys-diner twice
        // on 2026-09-01 (L2605 booked t6195, reverted t6308; L2658 booked t6308,
        // reverted t6369 — ~8 hours idle and two revert penalties), reproduced
        // on LOCAL with L1849 at tranquility the same evening.
        if here != active.row.origin && active.word == ActiveWord::Booked {
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

    // The drive before work. Wear derates the drive linearly (fully worn is half),
    // and every leg is flown at that drive — KK II at 88% wear was making 105 mg
    // of 189 and missing pickup windows by it (L2706, 2026-09-02). On a leased
    // hull the yard clears it for nothing, so it is cleared early and often; a
    // titled hull pays for the work, so it waits for real wear and real cash.
    if ship.wear_bps
        >= if ship.leased {
            REPAIR_LEASED_AT_BPS
        } else {
            REPAIR_TITLED_AT_BPS
        }
    {
        let invoice = if ship.leased {
            0
        } else {
            ship.wear_bps * REPAIR_COST_PER_HUNDRED_BPS / 100
        };
        if invoice <= ship.credits / 4 {
            return Decision::Repair;
        }
    }

    // Work: best net per tick of pilot time, dock included — of what we can fuel.
    // Deliberately BEFORE the low-fuel diversion: every plan below carries its own
    // reserve, and a load whose fuel-selling origin is reachable earns on the way
    // to the pump a bare diversion would fly for free.
    let mut ranked: Vec<&LoadRow> = board
        .iter()
        // Fit against the SPARE hold, not the whole one: with freight idle whatever is in
        // the hold is the merchant's carried goods, and a contract that cannot load
        // beside them is a contract that waits on the crane forever.
        .filter(|l| !l.held_for_other && l.units <= ship.hold_capacity - ship.hold_used)
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
        // Can we be at the origin, loaded, inside the desk's pickup window? The
        // honest deadhead is the engine's own arithmetic on tonight's separations at
        // the drive THIS contract's class leaves us — L2706 on PROD (economy, 88%
        // wear) took 74 ticks over a 39-tick lane and the desk took it back at +48.
        // The board's figure is the fallback when the router cannot price legs.
        let hull = if ship.accel_milli_g > 0 {
            ship.accel_milli_g
        } else {
            REFERENCE_ACCEL_MILLI_G
        };
        let class = if l.class_bps > 0 { l.class_bps } else { 10_000 };
        let accel = contract_accel(hull, class);
        let dead_ticks = router
            .leg_distances_km(here, &l.origin)
            .map(|d| flight_ticks(&d, accel))
            .unwrap_or(l.deadhead_ticks);
        if dead_ticks + ENGAGE_OVERHEAD_TICKS + l.loading_ticks.max(8) > PICKUP_TTL_TICKS {
            continue;
        }
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
            accel_milli_g: REFERENCE_ACCEL_MILLI_G,
            wear_bps: 0,
            leased: false,
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
            good: String::new(),
            class_bps: 10_000,
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
    fn a_worn_leased_hull_repairs_for_nothing_before_taking_work() {
        // KK II, 2026-09-02: leased (titled false, principal 25000), wear 8827 bps.
        let mut ship = ship_at("titan-larder", 500);
        ship.wear_bps = 8_827;
        ship.leased = true;
        let board = vec![load("L1", "titan-larder", "tuna-prime", 900, (0, 20))];
        let d = decide(&ship, None, &board, &pumps(&[]), &FlatRouter(10));
        assert_eq!(d, Decision::Repair);
        // Barely worn: work first.
        ship.wear_bps = 500;
        let d = decide(&ship, None, &board, &pumps(&[]), &FlatRouter(10));
        assert!(matches!(d, Decision::Book { .. }), "{d:?}");
        // A titled hull at the same 88%: invoice 8827 × 40 / 100 = 3530 — repaired
        // with 10 000 in the bank (a quarter is 2 500: NOT affordable, so booked),
        // repaired once cash allows.
        ship.wear_bps = 8_827;
        ship.leased = false;
        ship.credits = 10_000;
        let d = decide(&ship, None, &board, &pumps(&[]), &FlatRouter(10));
        assert!(matches!(d, Decision::Book { .. }), "{d:?}");
        ship.credits = 20_000;
        let d = decide(&ship, None, &board, &pumps(&[]), &FlatRouter(10));
        assert_eq!(d, Decision::Repair);
    }

    #[test]
    fn flight_time_is_the_engines_arithmetic() {
        // PROD, 2026-09-02: cannery-row → titan-larder, 1,307,724,939 km. The route
        // endpoint says 39 ticks at the reference drive; KK II (wear 8827 bps →
        // 105 mg) on an ECONOMY contract (half drive → 52 mg) took 74.
        let d = [1_307_724_939_i64];
        assert_eq!(flight_ticks(&d, REFERENCE_ACCEL_MILLI_G), 39);
        assert_eq!(contract_accel(105, 5_000), 52);
        let t = flight_ticks(&d, 52);
        assert!((74..=75).contains(&t), "{t}");
        // Two legs sum; a zero-length leg still costs a tick.
        assert_eq!(
            flight_ticks(&[0, 1_307_724_939], REFERENCE_ACCEL_MILLI_G),
            40
        );
    }

    #[test]
    fn a_load_whose_honest_deadhead_would_miss_the_pickup_window_is_not_booked() {
        // L2706 on PROD: board deadhead 19; the honest figure on an economy
        // contract with a worn hull is 74 (+ engage + loading) — the desk reverts
        // at +48 (−232 ℳ). The same lane on a STANDARD contract at full drive is
        // 39 + 4 + 8 = 51 — still over. A shorter leg fits.
        struct Chart(i64);
        impl Router for Chart {
            fn fuel_between(&self, _: &str, _: &str) -> Option<i64> {
                Some(10)
            }
            fn leg_distances_km(&self, _: &str, _: &str) -> Option<Vec<i64>> {
                Some(vec![self.0])
            }
        }
        let ship = Ship {
            docked: Some("cannery-row".into()),
            accel_milli_g: 105,
            wear_bps: 8827,
            fuel: 600,
            fuel_capacity: 600,
            hold_capacity: 120,
            ..Default::default()
        };
        let mut row = LoadRow {
            load_id: "L2706".into(),
            good: "catnip".into(),
            class_bps: 5_000,
            origin: "titan-larder".into(),
            dest: "tuna-prime".into(),
            units: 40,
            estimated_net: 928,
            deadhead_ticks: 19,
            haul_ticks: 30,
            loading_ticks: 8,
            held_for_other: false,
        };
        let far = Chart(1_307_724_939);
        let d = decide(
            &ship,
            None,
            std::slice::from_ref(&row),
            &BTreeSet::new(),
            &far,
        );
        assert!(!matches!(d, Decision::Book { .. }), "{d:?}");
        // A leg a third the distance: 74/√3 ≈ 43 at 52 mg — over with overhead;
        // at standard class (105 mg) it is ~30 + 4 + 8 = 42: booked.
        row.class_bps = 10_000;
        let near = Chart(1_307_724_939 / 3);
        let d = decide(
            &ship,
            None,
            std::slice::from_ref(&row),
            &BTreeSet::new(),
            &near,
        );
        assert_eq!(
            d,
            Decision::Book {
                load_id: "L2706".into()
            }
        );
    }

    #[test]
    fn booked_while_berthed_at_the_destination_still_deadheads_to_the_origin() {
        // KK II at foxys-diner, 2026-09-01: booked a load INTO the berth she sat at
        // and waited for a crane that had nothing to load; the desk reverted it.
        let ship = Ship {
            docked: Some("foxys-diner".into()),
            fuel: 600,
            fuel_capacity: 600,
            hold_capacity: 120,
            ..Default::default()
        };
        let active = Active {
            row: LoadRow {
                load_id: "L2605".into(),
                good: "grain".into(),
                class_bps: 10_000,
                origin: "whisker-hollow".into(),
                dest: "foxys-diner".into(),
                units: 120,
                estimated_net: 500,
                deadhead_ticks: 19,
                haul_ticks: 19,
                loading_ticks: 8,
                held_for_other: false,
            },
            word: ActiveWord::Booked,
        };
        let d = decide(&ship, Some(&active), &[], &BTreeSet::new(), &FlatRouter(50));
        assert_eq!(
            d,
            Decision::Travel {
                station: "whisker-hollow".into()
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
            Decision::Repair,
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
