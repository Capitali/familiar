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
    /// The yard's rate for a titled hull, ℳ per hundred bps of wear
    /// (`params.repairCostPerHundredBps` on `/v1/reference` — published 2026-09-04
    /// "which a client has been quoting blind"; 40 on the shipped pack). Zero means
    /// the world did not say, and our copy of the pack answers.
    pub repair_per_hundred_bps: i64,
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

// ---------------------------------------------------------------------------
// The burn rungs
// ---------------------------------------------------------------------------
//
// The exchange prices four rungs and throttles the hull by each (engine
// `EconomyParams.accelBps`). They are a genuine trade, not a speed knob: time
// goes as 1/√a and propellant as the ROCKET EQUATION on a delta-v that goes as
// √a, so hotter is faster, thirstier and harder on the drive, all at once.
//
// The rung only reaches the physics on an UNBOOKED voyage. Under a contract the
// LOAD's own class governs every leg, the deadhead to the pickup included
// (engine `FreightShips.accelMilliG`), so filing a rung on freight is filing
// into the wind. Two doors are ours: the run to a pump, and the merchant carry.

/// Half the hull. Slow, and cheap enough to change what "in reach" means.
pub const BURN_ECONOMY: i64 = 5_000;
/// The hull as it is. Rides the wire as ABSENT, so a world that never heard of
/// rungs sees exactly the filing it always saw.
pub const BURN_STANDARD: i64 = 10_000;
pub const BURN_EXPRESS: i64 = 20_000;
pub const BURN_PRIORITY: i64 = 30_000;

/// Slowest first: the order the tank is asked in.
pub const BURN_RUNGS: [i64; 4] = [BURN_ECONOMY, BURN_STANDARD, BURN_EXPRESS, BURN_PRIORITY];

/// The wire's word for a rung, or None for standard — which is filed by saying
/// nothing at all.
pub fn burn_wire_name(bps: i64) -> Option<&'static str> {
    match bps {
        BURN_ECONOMY => Some("economy"),
        BURN_EXPRESS => Some("express"),
        BURN_PRIORITY => Some("priority"),
        _ => None,
    }
}

/// Exhaust velocity, km/s (engine `FlightModel.exhaustVelocityKmPerSecond`).
const EXHAUST_KM_S: f64 = 10_000.0;
/// Propellant-fraction multiplier in fuel units (`fuelScale`).
const FUEL_SCALE: f64 = 236.0;
/// Charged on every departure whatever the distance (`fuelDockOverhead`).
const FUEL_DOCK_OVERHEAD: f64 = 5.0;

/// Mission delta-v for one leg, km/s: `dv = 2√(D·a)`, which at the shipped
/// drive is `(62/720)·√D` and scales as √(a/aRef) either side of it.
fn leg_delta_v(distance_km: i64, accel_milli_g: i64) -> f64 {
    if distance_km <= 0 {
        return 0.0;
    }
    let root = (distance_km as f64).sqrt();
    let ratio = (accel_milli_g.max(1) as f64 / REFERENCE_ACCEL_MILLI_G as f64).sqrt();
    62.0 * root / 720.0 * ratio
}

/// Propellant for one leg at a drive, the engine's own rocket equation
/// (`FlightModel.fuelUnits`). NOT the √ scaling [`fuel_at_drive`] uses: that is a
/// straight line through a curve, honest near the reference drive and wrong at
/// the ends — it reads 119 where the fold charges 112 on the economy run out of
/// titania, and 238 where the fold charges 261 on the express one. Cheap to be
/// exact here, and being exact is what lets a downshift be trusted with the last
/// of a tank.
pub fn leg_fuel_at_drive(distance_km: i64, accel_milli_g: i64) -> i64 {
    if distance_km <= 0 {
        return FUEL_DOCK_OVERHEAD as i64;
    }
    let dv = leg_delta_v(distance_km, accel_milli_g);
    (FUEL_DOCK_OVERHEAD + FUEL_SCALE * (dv / EXHAUST_KM_S).exp_m1()).floor() as i64
}

/// What a whole route costs at a rung, given the hull's own drive.
pub fn route_fuel_at_burn(legs_km: &[i64], hull_accel_milli_g: i64, burn_bps: i64) -> i64 {
    let accel = (hull_accel_milli_g * burn_bps / 10_000).max(1);
    legs_km.iter().map(|&d| leg_fuel_at_drive(d, accel)).sum()
}

/// Flight time for a whole route at a rung.
pub fn route_ticks_at_burn(legs_km: &[i64], hull_accel_milli_g: i64, burn_bps: i64) -> i64 {
    let accel = (hull_accel_milli_g * burn_bps / 10_000).max(1);
    flight_ticks(legs_km, accel)
}

/// A rung chosen for a course, with what it will cost and take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BurnPlan {
    pub bps: i64,
    pub fuel: i64,
    pub ticks: i64,
}

/// Does the model agree with the exchange about this route?
///
/// The constants above are the SHIPPED ones and a world may price its own. So
/// they are checked, every time, against the figure `/v1/route` already quoted
/// at the reference drive. Agreement buys the right to reason about rungs at
/// all; disagreement means the model is describing some other world, and the
/// only safe move then is no move — never a hotter burn on arithmetic that has
/// just been shown wrong.
fn model_agrees(legs_km: &[i64], quoted_at_reference: i64) -> bool {
    if quoted_at_reference <= 0 || legs_km.is_empty() {
        return false;
    }
    let modelled: i64 = legs_km
        .iter()
        .map(|&d| leg_fuel_at_drive(d, REFERENCE_ACCEL_MILLI_G))
        .sum();
    let slack = (quoted_at_reference / 20).max(2);
    (modelled - quoted_at_reference).abs() <= slack
}

/// The rung to fly a course on, and what it costs.
///
/// STANDARD FIRST, always: a healthy tank flies the throttle it always flew, so
/// nothing about a well-fuelled pilot changes. Only when standard does not reach
/// does the ladder go DOWN — half throttle is slower by √2 and cheaper by rather
/// more than that, because propellant follows the rocket equation and the
/// equation curves.
///
/// That downshift is the whole point. KK sat at titania-cold-store for three of
/// Ian's real days on 135 of 600, its pilot journaling "low fuel, no affordable
/// pump; holding for a human", while foxy's-diner was 168 away at the throttle
/// the pilot could name and 112 away at the one it could not (2026-09-04).
/// Nothing was wrong with the tank. The pilot had one word for "go".
///
/// Never upward: a hotter rung is a real cost and no route NEEDS one, so
/// spending Ian's propellant to arrive early is not a call this rule makes.
/// Returns None when no rung reaches, which is the honest answer that sends the
/// tanker.
pub fn burn_that_reaches(
    legs_km: &[i64],
    quoted_at_reference: i64,
    hull_accel_milli_g: i64,
    tank: i64,
    reserve: f64,
) -> Option<BurnPlan> {
    let affords = |fuel: i64| (fuel as f64 * reserve) as i64 <= tank;

    if !model_agrees(legs_km, quoted_at_reference) {
        // Unverified arithmetic buys exactly one thing: the filing the pilot
        // would have made anyway, priced off the exchange's own quote.
        return affords(quoted_at_reference).then_some(BurnPlan {
            bps: BURN_STANDARD,
            fuel: quoted_at_reference,
            ticks: 0,
        });
    }

    for bps in [BURN_STANDARD, BURN_ECONOMY] {
        let fuel = route_fuel_at_burn(legs_km, hull_accel_milli_g, bps);
        if affords(fuel) {
            return Some(BurnPlan {
                bps,
                fuel,
                ticks: route_ticks_at_burn(legs_km, hull_accel_milli_g, bps),
            });
        }
    }
    None
}

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

/// Fuel from `at` to the nearest priceable pump — zero when `at` pumps. A leg that
/// ends where no pump is reachable is a leg that ends the voyage.
pub fn onward_to_pump(at: &str, pumps: &BTreeSet<String>, router: &dyn Router) -> i64 {
    // No pumps on the chart at all (a pack that prices no fuel): nothing to reach.
    if pumps.is_empty() || pumps.contains(at) {
        return 0;
    }
    pumps
        .iter()
        .filter_map(|p| router.fuel_between(at, p))
        .min()
        .unwrap_or(i64::MAX / 4)
}

/// Fuel for a leg the route priced at the reference drive, re-priced for the drive
/// it will actually be flown at: propellant scales with √(acceleration) (engine
/// `FlightModel.deltaVQ8`). A tuned hull (217 mg) burns ~7% more than the quote
/// and a worn one less; KK II arrived at foxys-diner with 40 in the tank after a
/// leg quoted near 240 cost 266 (2026-09-03, ucf-exchange#18). Rounded up.
pub fn fuel_at_drive(quoted: i64, accel_milli_g: i64) -> i64 {
    if quoted <= 0 {
        return quoted;
    }
    let ratio = (accel_milli_g.max(1) as f64 / REFERENCE_ACCEL_MILLI_G as f64).sqrt();
    ((quoted as f64) * ratio).ceil() as i64
}

/// The drive a contract's legs are flown at: the hull throttled by the class.
pub fn contract_accel(ship_accel_milli_g: i64, class_bps: i64) -> i64 {
    (ship_accel_milli_g * class_bps / 10_000).max(1)
}

/// The ledger's word about one load, read as a STATE MACHINE over its events in
/// order — never as "the first terminal-looking word". `Ok(Some(word))` is a live
/// contract; `Err(reason)` is settled or lost, either way no longer ours to fly.
///
/// The order matters: a duplicate booking refused AFTER the real one ("booked",
/// then "rejected: load is not open" — PROD L2831 when a restart re-filed inside the
/// fold, 2026-09-02) is noise on a live contract, not its loss. A refusal only loses
/// a load that was never ours; a revert, expiry, lapse or cancel loses it whenever.
pub fn ledger_word(events: &[&str]) -> Result<Option<ActiveWord>, String> {
    let mut word: Option<ActiveWord> = None;
    for e in events {
        let l = e.to_lowercase();
        if l.contains("payment taken") || l.contains("collected") {
            return Err(format!("settled: {e}"));
        }
        if l.contains("reverted")
            || l.contains("expired")
            || l.contains("lapsed")
            || l.contains("cancel")
        {
            return Err(format!("lost: {e}"));
        }
        if l.contains("rejected") {
            if word.is_none() {
                return Err(format!("lost: {e}"));
            }
            continue; // a refused extra order on a contract we already hold
        }
        if l.contains("delivered") {
            word = Some(ActiveWord::Delivered);
        } else if l.contains("pickedup") || l.contains("picked up") {
            if word != Some(ActiveWord::Delivered) {
                word = Some(ActiveWord::PickedUp);
            }
        } else if l.contains("booked") && word.is_none() {
            word = Some(ActiveWord::Booked);
        }
    }
    Ok(Some(word.unwrap_or(ActiveWord::Booked)))
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
    DivertToPump { pump: String, burn_bps: i64 },
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
    //
    // EXCEPT standing on a pump, which outranks the tanker in turn. This rule used
    // to fire first and unconditionally, so a hull that was low enough to be in
    // trouble called a tanker even while berthed at a fuel seller — and having
    // called one, could not call another. Kibble Klipper did exactly that on
    // 2026-09-04: 23 of 600 tied up alongside foxy's-diner, which sells fuel at 2 a
    // unit, journalling "low fuel, no affordable pump" at a pump, with a tanker
    // 54 hours out charging 33,594 for what the counter beside her wanted 1,154 for.
    // A tanker is what you call when no pump is in reach. This one is under the hull.
    if frac(ship.fuel) < CRITICAL_FUEL
        && !ship.docked.as_deref().is_some_and(|at| pumps.contains(at))
    {
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
            // The world's own rate when it publishes one, our copy of the pack when not.
            let rate = if ship.repair_per_hundred_bps > 0 {
                ship.repair_per_hundred_bps
            } else {
                REPAIR_COST_PER_HUNDRED_BPS
            };
            ship.wear_bps * rate / 100
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
        // The plan must reach a pump AFTER the delivery too: a hull that arrives at
        // a pumpless destination with an empty tank has no move left but the
        // tanker (LOCAL, titan-larder, 2026-09-02: fuel 94, no pump in reach, a
        // PAWS call-out from Saturn for ~15,000 ℳ). Cost of the onward leg to the
        // nearest priceable pump, zero when the destination pumps.
        let onward = onward_to_pump(&l.dest, pumps, router);
        // The quote is for the reference drive; these legs fly at the contract's.
        let dead = fuel_at_drive(dead, accel);
        let haul = fuel_at_drive(haul, accel);
        let onward = fuel_at_drive(onward, accel);
        let whole = ((dead + haul + onward) as f64 * RESERVE) as i64;
        let dead_only = (dead as f64 * RESERVE) as i64;
        let haul_only = ((haul + onward) as f64 * RESERVE) as i64;
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
    // ...or, berthed where nothing pumps, whatever the tank reads: from a pumpless
    // berth every plan must also carry the leg to a pump, and a half tank can make the
    // whole board "unfuelable" — KK II sat 179 folds (5½ hours) at titania-cold-store
    // on 306 of 600 that way (2026-09-03). At a pump the plan is priced against a full
    // tank, so going there is never the wrong move when there is no work here.
    if frac(ship.fuel) < LOW_FUEL || (!pumps.is_empty() && !pumps.contains(here)) {
        // Each pump is asked at the throttle that REACHES it, not only at the
        // one the pilot prefers. Standard first, so a healthy tank flies exactly
        // as it always did; half throttle only when standard falls short, which
        // is the difference between a run and three days at a dead berth.
        let mut best: Option<(BurnPlan, &String)> = None;
        for p in pumps {
            let Some(cost) = router.fuel_between(here, p) else {
                continue;
            };
            let legs = router.leg_distances_km(here, p).unwrap_or_default();
            let Some(plan) = burn_that_reaches(&legs, cost, ship.accel_milli_g, ship.fuel, 1.1)
            else {
                continue;
            };
            // Cheapest arrival wins, and a rung that costs less IS cheaper —
            // ranking on the reference quote would rank routes by a throttle
            // nobody is flying.
            if best.map(|(b, _)| plan.fuel < b.fuel).unwrap_or(true) {
                best = Some((plan, p));
            }
        }
        return match best {
            Some((plan, pump)) => Decision::DivertToPump {
                pump: pump.clone(),
                burn_bps: plan.bps,
            },
            // A pumpless berth with no reachable pump on a healthy tank is not a
            // distress; only a genuinely low tank calls the tanker.
            None if frac(ship.fuel) < LOW_FUEL => Decision::CallPaws,
            None => Decision::Hold {
                why: "no fuelable work on the board, no pump in reach".into(),
            },
        };
    }
    Decision::Hold {
        why: "no fuelable work on the board".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// KK's real numbers, the fold's own: titania-cold-store to foxy's-diner on
    /// 2026-09-04, quoted 168 at the reference drive over two legs, with 135 in
    /// a 600 tank. Standard does not reach and the pilot called it stranded for
    /// three of Ian's real days. Half throttle reaches with room to spare.
    ///
    /// The 106 is not a guess: the ship departed on this rung and the tank went
    /// 135 to 29 on the first leg, which is 106 to the unit.
    #[test]
    fn half_throttle_reaches_the_pump_that_standard_cannot() {
        let legs = [3_491_917_000_i64, 1_774_626];
        assert_eq!(leg_fuel_at_drive(legs[0], 94), 106);

        let plan = burn_that_reaches(&legs, 168, REFERENCE_ACCEL_MILLI_G, 135, 1.1)
            .expect("a rung that reaches");
        assert_eq!(plan.bps, BURN_ECONOMY);
        assert_eq!(plan.fuel, 112);
        // Slower, and by about the √2 the brachistochrone promises: 66 becomes 94.
        assert_eq!(plan.ticks, 94);
    }

    /// A tank that can afford the throttle it always flew keeps flying it. This
    /// is the gate on the whole rule: nothing about a well-fuelled pilot changes.
    #[test]
    fn a_healthy_tank_still_flies_standard() {
        let legs = [3_491_917_000_i64, 1_774_626];
        let plan = burn_that_reaches(&legs, 168, REFERENCE_ACCEL_MILLI_G, 600, 1.1)
            .expect("a rung that reaches");
        assert_eq!(plan.bps, BURN_STANDARD);
        assert_eq!(plan.ticks, 66);
    }

    /// A tank that reaches nothing sends the tanker rather than inventing a rung.
    #[test]
    fn no_rung_reaches_on_an_empty_tank() {
        let legs = [3_491_917_000_i64, 1_774_626];
        assert_eq!(
            burn_that_reaches(&legs, 168, REFERENCE_ACCEL_MILLI_G, 40, 1.1),
            None
        );
    }

    /// A world whose fuel does not match the shipped constants gets the filing it
    /// would have got anyway — never a rung reasoned out of arithmetic that has
    /// just been shown to describe somewhere else.
    #[test]
    fn an_unrecognised_fuel_model_refuses_to_reason_about_rungs() {
        let legs = [3_491_917_000_i64, 1_774_626];
        let plan = burn_that_reaches(&legs, 900, REFERENCE_ACCEL_MILLI_G, 2_000, 1.1)
            .expect("the quoted filing still stands");
        assert_eq!(plan.bps, BURN_STANDARD);
        assert_eq!(plan.fuel, 900);
        // And on a tank that cannot afford the quote, no filing at all.
        assert_eq!(
            burn_that_reaches(&legs, 900, REFERENCE_ACCEL_MILLI_G, 500, 1.1),
            None
        );
    }

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
            repair_per_hundred_bps: 0,
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
    fn the_ledger_is_read_in_order_and_a_refused_duplicate_is_not_a_loss() {
        // PROD L2831: booked, then the restart's duplicate refused, same tick.
        let w = ledger_word(&["booked", "rejected: load is not open"]);
        assert_eq!(w, Ok(Some(ActiveWord::Booked)));
        // Never ours: the refusal is the loss.
        assert!(ledger_word(&["rejected: you let this contract lapse"]).is_err());
        // Held, booked, reverted, then refused again: lost at the revert.
        let w = ledger_word(&[
            "the desk is holding this one for you",
            "booked",
            "reverted",
            "rejected: you let this contract lapse",
        ]);
        assert!(
            matches!(w, Err(ref r) if r.starts_with("lost: reverted")),
            "{w:?}"
        );
        // The happy path, in order.
        assert_eq!(
            ledger_word(&["booked", "picked up 6 units"]),
            Ok(Some(ActiveWord::PickedUp))
        );
        assert_eq!(
            ledger_word(&["booked", "picked up", "delivered"]),
            Ok(Some(ActiveWord::Delivered))
        );
        assert!(
            ledger_word(&["booked", "picked up", "delivered", "settled: payment taken"]).is_err()
        );
    }

    #[test]
    fn a_booking_reserves_fuel_for_the_leg_from_the_destination_to_a_pump() {
        // Titan-larder pumps nothing; the nearest pump from it costs 60. A load
        // whose dead + haul the tank covers, but not the onward 60, is not booked.
        struct Chart;
        impl Router for Chart {
            fn fuel_between(&self, from: &str, to: &str) -> Option<i64> {
                Some(match (from, to) {
                    (a, b) if a == b => 0,
                    (_, "titan-larder") => 100,
                    ("titan-larder", "foxys-diner") => 60,
                    _ => 500,
                })
            }
        }
        let pumps = pumps(&["foxys-diner"]);
        let board = vec![load("L1", "here", "titan-larder", 900, (0, 20))];
        // dead 0 + haul 100 + onward 60 = 160 × 1.2 = 192 in the tank.
        let mut ship = ship_at("here", 180);
        let d = decide(&ship, None, &board, &pumps, &Chart);
        assert!(!matches!(d, Decision::Book { .. }), "{d:?}");
        ship.fuel = 200;
        let d = decide(&ship, None, &board, &pumps, &Chart);
        assert_eq!(
            d,
            Decision::Book {
                load_id: "L1".into()
            }
        );
    }

    #[test]
    fn idle_at_a_pumpless_berth_goes_to_the_pump_whatever_the_tank_reads() {
        // titania-cold-store, 2026-09-03: fuel 306/600, an empty board of fuelable
        // work, no pump here → go stand at the nearest affordable pump, do not sit.
        let mut ship = ship_at("titania-cold-store", 306);
        let pumps = pumps(&["foxys-diner"]);
        let d = decide(&ship, None, &[], &pumps, &FlatRouter(60));
        assert_eq!(
            d,
            Decision::DivertToPump {
                pump: "foxys-diner".into(),
                burn_bps: BURN_STANDARD
            }
        );
        // Berthed AT a pump, tank full, nothing to do: hold, do not shuttle.
        ship.docked = Some("foxys-diner".into());
        ship.fuel = 600;
        let d = decide(&ship, None, &[], &pumps, &FlatRouter(60));
        assert!(matches!(d, Decision::Hold { .. }), "{d:?}");
    }

    #[test]
    fn fuel_is_repriced_for_the_drive_it_flies_at() {
        assert_eq!(fuel_at_drive(240, REFERENCE_ACCEL_MILLI_G), 240);
        // drive-tune: 217 mg → √(217/189) ≈ 1.071 → 258 (the leg cost 266 with the
        // haircut the quote already carries).
        assert_eq!(fuel_at_drive(240, 217), 258);
        // worn to 105 mg: less propellant, not more.
        assert!(fuel_at_drive(240, 105) < 240);
        assert_eq!(fuel_at_drive(0, 217), 0);
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

    /// A pump under the hull outranks the tanker. KK stood at foxy's-diner on 23 of
    /// 600 calling for a truck that was 54 hours out and wanted 33,594, while the
    /// counter she was tied up to sold the same fuel for 1,154.
    #[test]
    fn a_pump_under_the_hull_outranks_the_tanker() {
        let ship = ship_at("foxys-diner", 23);
        let d = decide(&ship, None, &[], &pumps(&["foxys-diner"]), &FlatRouter(10));
        assert_eq!(d, Decision::Refuel);
        // The same tank anywhere that does not pump still calls the truck.
        let adrift = ship_at("titania-cold-store", 23);
        let d = decide(&adrift, None, &[], &pumps(&["foxys-diner"]), &NoRouter);
        assert_eq!(d, Decision::CallPaws);
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
                pump: "near-pump".into(),
                burn_bps: BURN_STANDARD
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
            Decision::DivertToPump {
                pump: "p".into(),
                burn_bps: BURN_STANDARD,
            },
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
