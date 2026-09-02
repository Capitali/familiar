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
    /// What the contract carries — the merchant must not mistake it for its own goods.
    pub good: String,
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

/// One contract the ship is committed to, with the ledger's word about it.
/// The itinerary's LIFECYCLE record — routing happens over [`Stop`]s, never
/// over contracts (T-232, both reciprocal reviews).
#[derive(Debug, Clone)]
pub struct Active {
    pub row: LoadRow,
    pub word: ActiveWord,
}

/// One typed thing to do at a berth. The routing atoms: a station visit carries
/// any number of these, so one berth can take a pickup AND a dropoff AND a fill,
/// and the day UCF-Haul#43 interleaves contracts the route can say
/// pickup-A → pickup-B → drop-B → drop-A without the model moving.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopOp {
    /// The crane loads this contract's cargo here.
    Pickup { load_id: String, units: i64 },
    /// The crane unloads this contract's cargo here.
    Drop { load_id: String, units: i64 },
    /// Top the tank up at this berth's pump — a planned fill, an itinerary
    /// entry in its own right (the board's requirement; also the repair for
    /// both reviews' finding that the fuel walk budgeted fills nothing ever
    /// executed).
    Refuel,
}

/// One station visit on the route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stop {
    pub station: String,
    pub ops: Vec<StopOp>,
}

/// The plan: contracts (lifecycle, booking order) plus the ordered station
/// visits that serve them. The ROUTE is the navigation truth; contracts are
/// evidence the ops complete against. Today's exchange enforces one contract,
/// so the live route is the degenerate origin-pickup → dest-drop compile of
/// [`Itinerary::sequential`] — but nothing in the navigation, fuel walk, or
/// occupancy check assumes it.
#[derive(Debug, Clone, Default)]
pub struct Itinerary {
    pub loads: Vec<Active>,
    pub stops: Vec<Stop>,
}

impl Itinerary {
    /// The compile today's world flies: each contract's remaining work in
    /// booking order — origin pickup (if not yet aboard), then destination
    /// drop — with same-station visits coalesced and a planned [`StopOp::Refuel`]
    /// opening every visit to a fuel-selling berth (pump fuel over tanker fuel,
    /// now AS an itinerary entry). A real multi-load planner replaces this
    /// function, not these types, when UCF-Haul#43's shape lands.
    pub fn sequential(loads: Vec<Active>, pumps: &BTreeSet<String>) -> Self {
        let mut stops: Vec<Stop> = Vec::new();
        let mut push_op = |station: &str, op: StopOp| match stops.last_mut() {
            Some(s) if s.station == station => s.ops.push(op),
            _ => stops.push(Stop {
                station: station.to_string(),
                ops: vec![op],
            }),
        };
        for a in &loads {
            match a.word {
                ActiveWord::Delivered => {}
                ActiveWord::Booked => {
                    push_op(
                        &a.row.origin,
                        StopOp::Pickup {
                            load_id: a.row.load_id.clone(),
                            units: a.row.units,
                        },
                    );
                    push_op(
                        &a.row.dest,
                        StopOp::Drop {
                            load_id: a.row.load_id.clone(),
                            units: a.row.units,
                        },
                    );
                }
                ActiveWord::PickedUp => {
                    push_op(
                        &a.row.dest,
                        StopOp::Drop {
                            load_id: a.row.load_id.clone(),
                            units: a.row.units,
                        },
                    );
                }
            }
        }
        for s in &mut stops {
            if pumps.contains(s.station.as_str()) {
                s.ops.insert(0, StopOp::Refuel);
            }
        }
        Itinerary { loads, stops }
    }

    pub fn is_empty(&self) -> bool {
        self.loads.is_empty()
    }

    fn word_of(&self, load_id: &str) -> Option<ActiveWord> {
        self.loads
            .iter()
            .find(|a| a.row.load_id == load_id)
            .map(|a| a.word)
    }

    /// Is this CRANE op done, judged from the ledger's own word about ITS load?
    /// Load-id-scoped only (round-2 review, finding 1): the aggregate hold can
    /// carry merchant cargo or another contract's units, so occupied hold is
    /// never evidence that THIS pickup happened — a booked load waits for its
    /// own `pickedUp` word, one fold behind the crane at worst, never departing
    /// on someone else's cargo. A drop completes on the delivered word.
    fn crane_op_done(&self, op: &StopOp) -> bool {
        match op {
            StopOp::Pickup { load_id, .. } => {
                !matches!(self.word_of(load_id), Some(ActiveWord::Booked))
            }
            StopOp::Drop { load_id, .. } => {
                matches!(self.word_of(load_id), Some(ActiveWord::Delivered) | None)
            }
            // Not a crane op — never navigated to (see `current`).
            StopOp::Refuel => true,
        }
    }

    /// The station visit the ship is working: the first stop with a CRANE op the
    /// ledger has not yet completed. Navigation keys on crane ops ONLY, because
    /// their words are monotonic (Booked → PickedUp → Delivered → settled) — a
    /// planned fill is executed opportunistically ON ARRIVAL at its stop and
    /// deliberately never steers, so later fuel burn can never point the route
    /// backwards at a pump already visited (round-2 review, finding 2). A
    /// refuel-ONLY stop is therefore invisible to navigation: `sequential` never
    /// compiles one, and a future planner that wants standalone fuel diversions
    /// must add durable per-stop progress first — stated here so it is a known
    /// edge, not a trap.
    fn current(&self) -> Option<&Stop> {
        self.stops
            .iter()
            .find(|s| s.ops.iter().any(|op| !self.crane_op_done(op)))
    }
}

/// The freight genuinely ABOARD, per each load's own ledger word: `PickedUp`
/// only. A Booked load's cargo is still at its origin and a Delivered load's
/// has already left the hold (only its payment remains) — counting either
/// against the merchant's book erases real positions (round-3 review, finding
/// 2: 30 merchant ore beside a delivered 100-ore contract read as −70 and the
/// book deleted a genuine lot). Loads whose contract names no good carry none.
pub fn freight_aboard(loads: &[Active]) -> Vec<(&str, i64)> {
    loads
        .iter()
        .filter(|a| a.word == ActiveWord::PickedUp && !a.row.good.is_empty())
        .map(|a| (a.row.good.as_str(), a.row.units))
        .collect()
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
        &Itinerary::sequential(active.cloned().into_iter().collect(), pumps),
        board,
        pumps,
        router,
    )
}

/// The judgment. Facts in, one decision out — over the ROUTE.
///
/// With zero or one contract this reduces to the judgment whisker has flown since
/// LOCAL — the wrapper above pins that, including the booked-at-destination
/// deadhead (the route's first unfinished crane op is the origin pickup, wherever
/// the hull is berthed). Three stated divergences from the old single-load rules,
/// all deliberate: berthed at a fuel-selling route stop with anything less than a
/// full tank, the pilot FILLS before working the crane (the old code budgeted a
/// fill in its booking arithmetic that no active-load decision could execute —
/// both round-1 reviews; filling to FULL, not the 90% line, because the fuel walk
/// proved the route against a full tank — round-2 finding 2); a picked-up load
/// berthed at some third station files for its destination rather than waiting on
/// a crane with nothing to do (the word outranks the old positional guess); and
/// the old `hold_used > 0` crane proxy is GONE — pickup completion is the load's
/// own ledger word, never the aggregate hold, because merchant cargo or another
/// contract's units in the hold must not launch a still-Booked load toward its
/// destination (round-2 finding 1; T-233 coexistence).
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
        if let Some(done) = plan.loads.iter().find(|a| a.word == ActiveWord::Delivered) {
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
        let Some(stop) = plan.current() else {
            return Decision::Hold {
                why: "waiting on the crane".into(),
            };
        };
        // At the working stop, a planned fill executes FIRST and fills ALL THE
        // WAY — the fuel walk proved this route against a full tank at this
        // berth, so anything short of full departs on a weaker tank than the
        // proof used (round-2 review, finding 2). The pump fills to capacity in
        // one act and rejects only an already-full tank, so the guard is exact.
        if stop.station == here
            && stop.ops.contains(&StopOp::Refuel)
            && ship.fuel < ship.fuel_capacity
        {
            return Decision::Refuel;
        }
        // Berthed anywhere ELSE that pumps, below the top-up line: fill where
        // the hull stands before departing — pump fuel over tanker fuel, the
        // rule that has always held. This is filling in place, never travel;
        // navigation itself keys only on crane ops, so a pump already visited
        // is history, not a destination.
        if pumps.contains(here) && frac(ship.fuel) < TOP_UP_BELOW {
            return Decision::Refuel;
        }
        if stop.station != here {
            return Decision::Travel {
                station: stop.station.clone(),
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

/// Which open load may join the plan, appended after its last stop.
///
/// HONEST ALTITUDE (both reciprocal reviews): the RANK is the board row's own
/// ship-relative rate — `estimated_net / pilot_ticks`, whose deadhead the
/// exchange priced from the ship's berth — NOT a recomputed marginal rate from
/// the plan's endpoint. That is exactly right for the empty plan the booking
/// gate limits it to today (it IS the old ranking, unchanged), and it is a
/// stated placeholder for a live multi-load plan: a true marginal rate needs
/// route TICKS from the plan's tail, which the wire's Router cannot yet
/// answer. Do not widen the booking gate past an empty plan on the strength
/// of this ranking. What IS plan-aware, and correct now: the hold-occupancy
/// walk (a candidate must fit the hold at its own pickup, beside merchant
/// goods and every not-yet-dropped contract) and the fuel walk (the tank,
/// plus planned fills at pump stops, must cover every leg of the remaining
/// route with the candidate appended).
pub fn best_insertion(
    ship: &Ship,
    plan: &Itinerary,
    board: &[LoadRow],
    pumps: &BTreeSet<String>,
    router: &dyn Router,
) -> Option<String> {
    let here = ship.docked.as_deref()?;
    let mut ranked: Vec<&LoadRow> = board
        .iter()
        .filter(|l| !l.held_for_other && fits_hold(ship, plan, l))
        .filter(|l| l.pilot_ticks() > 0 && l.estimated_net > 0)
        .collect();
    ranked.sort_by(|a, b| {
        let ra = a.estimated_net as f64 / a.pilot_ticks() as f64;
        let rb = b.estimated_net as f64 / b.pilot_ticks() as f64;
        rb.partial_cmp(&ra).unwrap_or(std::cmp::Ordering::Equal)
    });
    for l in ranked.into_iter().take(PRICED_CANDIDATES) {
        let stops = with_candidate(plan, l, pumps);
        if plan_fuelable(ship, here, &stops, router) {
            return Some(l.load_id.clone());
        }
    }
    None
}

/// The plan's route with the candidate's own stops appended (its origin pickup,
/// its destination drop, planned fills at pump berths) — the append-at-end
/// insertion this brick limits itself to.
fn with_candidate(plan: &Itinerary, l: &LoadRow, pumps: &BTreeSet<String>) -> Vec<Stop> {
    let mut stops = plan.stops.clone();
    let mut tail = Itinerary::sequential(
        vec![Active {
            row: l.clone(),
            word: ActiveWord::Booked,
        }],
        pumps,
    )
    .stops;
    stops.append(&mut tail);
    stops
}

/// Does this candidate's cargo fit the hold AT ITS OWN PICKUP, walked along the
/// route? Occupancy starts at the aggregate hold (merchant goods and picked-up
/// freight included), rises at each pickup, falls at each drop — so two
/// sequential full-hold contracts fit a hold that could never carry them
/// TOGETHER, and a merchant position genuinely narrows what freight can board
/// (the summed-reservation check both reviews rejected is gone).
fn fits_hold(ship: &Ship, plan: &Itinerary, l: &LoadRow) -> bool {
    let mut occupancy = ship.hold_used;
    let pumps_none = BTreeSet::new();
    for stop in with_candidate(plan, l, &pumps_none) {
        for op in &stop.ops {
            match op {
                StopOp::Pickup { units, load_id } => {
                    // A picked-up contract's units already sit in the aggregate;
                    // only not-yet-loaded cargo raises occupancy here.
                    if plan.word_of(load_id) != Some(ActiveWord::PickedUp) {
                        occupancy += units;
                        if occupancy > ship.hold_capacity {
                            return false;
                        }
                    }
                }
                StopOp::Drop { units, .. } => occupancy -= units,
                StopOp::Refuel => {}
            }
        }
    }
    true
}

/// Can the tank fly this route? The walk splits at stops carrying a planned
/// [`StopOp::Refuel`] — arriving at one fills to capacity, and unlike the
/// version both reviews rejected, that fill is now a real itinerary op the
/// pilot executes on arrival (`decide_plan` files Refuel at such a berth
/// before working the crane). The FIRST stretch must fit the tank as it
/// stands; every stretch after a fill must fit a full tank; the same reserve
/// arithmetic the single-load rule always used applies per stretch, so with
/// one candidate and an empty plan this reduces exactly to the original pair
/// of checks: `(dead+haul)·R ≤ fuel`, or a fuel-selling origin with
/// `dead·R ≤ fuel` and `haul·R ≤ capacity`. A leg the router cannot price is
/// a route the doctrine will not risk.
fn plan_fuelable(ship: &Ship, here: &str, stops: &[Stop], router: &dyn Router) -> bool {
    let mut budget = ship.fuel;
    let mut stretch: i64 = 0;
    let mut pos = here.to_string();
    for stop in stops {
        if stop.station != pos {
            let Some(cost) = router.fuel_between(&pos, &stop.station) else {
                return false;
            };
            stretch += cost;
            pos = stop.station.clone();
        }
        if stop.ops.contains(&StopOp::Refuel) {
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
            good: String::new(),
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

    // ——— T-232: the itinerary. Everything below exercises the plan layer over a
    // stubbed multi-load world; the doctrine tests above, unmodified, pin that the
    // one-contract world still flies the same through it (decide IS decide_plan
    // over the sequential compile of ≤1 contract).

    fn active(id: &str, origin: &str, dest: &str, units: i64, word: ActiveWord) -> Active {
        let mut row = load(id, origin, dest, 500, (5, 10));
        row.units = units;
        Active { row, word }
    }

    #[test]
    fn the_wrapper_is_the_degenerate_plan_none_and_one_contract_agree() {
        let ship = ship_at("tuna-prime", 500);
        let board = [load("L1", "a", "b", 900, (5, 10))];
        let one = active("L2", "a", "b", 25, ActiveWord::Booked);
        for a in [None, Some(&one)] {
            assert_eq!(
                decide(&ship, a, &board, &pumps(&["a"]), &FlatRouter(10)),
                decide_plan(
                    &ship,
                    &Itinerary::sequential(a.cloned().into_iter().collect(), &pumps(&["a"])),
                    &board,
                    &pumps(&["a"]),
                    &FlatRouter(10)
                )
            );
        }
    }

    #[test]
    fn the_route_is_stops_with_typed_ops_not_contracts() {
        // The reviews' required pin: an interleaved route — pickup A, pickup B,
        // drop B and refuel at one berth, drop A — expressed directly, with the
        // hold occupancy walked per op. No sequential compile can make this
        // shape; the STRUCTURE carries it, which is the point.
        let plan = Itinerary {
            loads: vec![
                active("A", "x", "z", 50, ActiveWord::Booked),
                active("B", "y", "w", 60, ActiveWord::Booked),
            ],
            stops: vec![
                Stop {
                    station: "x".into(),
                    ops: vec![StopOp::Pickup {
                        load_id: "A".into(),
                        units: 50,
                    }],
                },
                Stop {
                    station: "y".into(),
                    ops: vec![StopOp::Pickup {
                        load_id: "B".into(),
                        units: 60,
                    }],
                },
                Stop {
                    station: "w".into(),
                    ops: vec![
                        StopOp::Refuel,
                        StopOp::Drop {
                            load_id: "B".into(),
                            units: 60,
                        },
                    ],
                },
                Stop {
                    station: "z".into(),
                    ops: vec![StopOp::Drop {
                        load_id: "A".into(),
                        units: 50,
                    }],
                },
            ],
        };
        // Movement order: the first unfinished op steers, in stop order.
        let ship = ship_at("elsewhere", 500);
        assert_eq!(
            decide_plan(&ship, &plan, &[], &pumps(&[]), &FlatRouter(10)),
            Decision::Travel {
                station: "x".into()
            }
        );
        // Occupancy is walked per op, not summed per contract: in a 115-unit
        // hold this route peaks at 110 (A and B aboard between y and w), so a
        // 10-unit candidate appended after z fits — a summed reservation
        // (50+60+10=120) would wrongly refuse it — while a candidate that
        // cannot fit the hold even alone is refused at its own pickup.
        let mut narrow = ship_at("elsewhere", 500);
        narrow.hold_capacity = 115;
        let fits = |units: i64| {
            let mut cand = load("C", "z", "q", 400, (5, 10));
            cand.units = units;
            fits_hold(&narrow, &plan, &cand)
        };
        assert!(fits(10));
        assert!(!fits(116));
    }

    #[test]
    fn two_full_hold_contracts_fit_in_sequence_the_hold_is_walked_not_summed() {
        // Both reviews rejected summed reservation: two 80-unit loads through a
        // 120-unit hold are fine SEQUENTIALLY — the legs never carry them together.
        let ship = ship_at("here", 600);
        let plan = Itinerary::sequential(
            vec![active("FIRST", "a", "b", 80, ActiveWord::Booked)],
            &pumps(&[]),
        );
        let mut second = load("SECOND", "c", "d", 400, (5, 10));
        second.units = 80;
        assert!(fits_hold(&ship, &plan, &second));
    }

    #[test]
    fn merchant_goods_narrow_what_freight_can_board() {
        // T-233 coexistence (the other lane's required regression): units the
        // merchant carries sit in the aggregate hold and genuinely reduce the
        // capacity an insertion may claim.
        let mut ship = ship_at("here", 600);
        ship.hold_used = 70; // carried trade goods, no freight aboard
        let plan = Itinerary::default();
        let mut cand = load("C", "a", "b", 400, (5, 10));
        cand.units = 51;
        assert!(!fits_hold(&ship, &plan, &cand));
        cand.units = 50;
        assert!(fits_hold(&ship, &plan, &cand));
    }

    #[test]
    fn delivered_money_is_collected_before_the_rest_of_the_plan_moves() {
        let ship = ship_at("elsewhere", 500);
        let plan = Itinerary::sequential(
            vec![
                active("EARN", "a", "b", 25, ActiveWord::Delivered),
                active("NEXT", "c", "d", 25, ActiveWord::Booked),
            ],
            &pumps(&[]),
        );
        assert_eq!(
            decide_plan(&ship, &plan, &[], &pumps(&[]), &FlatRouter(10)),
            Decision::Collect {
                load_id: "EARN".into()
            }
        );
    }

    #[test]
    fn a_multi_contract_plan_reads_cargo_from_each_loads_own_word() {
        // The aggregate hold cannot attribute units to one contract, so with
        // several contracts the ledger word is the pickup evidence: FIRST is
        // aboard per its word (hold_used says nothing), so the route files for
        // its destination.
        let mut ship = ship_at("a", 500);
        ship.hold_used = 0;
        let plan = Itinerary::sequential(
            vec![
                active("FIRST", "a", "b", 25, ActiveWord::PickedUp),
                active("SECOND", "c", "d", 25, ActiveWord::Booked),
            ],
            &pumps(&[]),
        );
        assert_eq!(
            decide_plan(&ship, &plan, &[], &pumps(&[]), &FlatRouter(10)),
            Decision::Travel {
                station: "b".into()
            }
        );
    }

    #[test]
    fn a_planned_fill_is_an_op_the_pilot_executes_then_leaves() {
        // Both reviews' finding 3, closed: arriving at a pump berth on the route
        // with a thirsty tank, the pilot FILES THE FILL before working the crane
        // — the fuel walk's budget reset is this op, executed. Once the tank is
        // above the line the same berth's crane work proceeds.
        let plan = Itinerary::sequential(
            vec![active("L", "pump-origin", "far", 25, ActiveWord::Booked)],
            &pumps(&["pump-origin"]),
        );
        let mut ship = ship_at("pump-origin", 200); // 33% — thirsty
        assert_eq!(
            decide_plan(&ship, &plan, &[], &pumps(&["pump-origin"]), &FlatRouter(10)),
            Decision::Refuel
        );
        // 98% is still not the full tank the fuel walk proved this route
        // against — the fill files until the fold shows capacity (round-2
        // review, finding 2: the proof and the executor must meet exactly).
        ship.fuel = 590;
        assert_eq!(
            decide_plan(&ship, &plan, &[], &pumps(&["pump-origin"]), &FlatRouter(10)),
            Decision::Refuel
        );
        ship.fuel = 600; // full — the crane wait begins
        assert_eq!(
            decide_plan(&ship, &plan, &[], &pumps(&["pump-origin"]), &FlatRouter(10)),
            Decision::Hold {
                why: "waiting on the crane".into()
            }
        );
    }

    #[test]
    fn anothers_cargo_never_launches_a_still_booked_load() {
        // Round-2 review, finding 1: merchant units (or any unattributed cargo)
        // in the aggregate hold must not complete L's pickup. Booked L from a→b
        // with 20 units of SOMETHING aboard: from a third station AND from L's
        // own destination, the only honest move is the deadhead to a.
        for berth in ["third-station", "b"] {
            let mut ship = ship_at(berth, 500);
            ship.hold_used = 20; // merchant cargo, not L's
            let plan = Itinerary::sequential(
                vec![active("L", "a", "b", 25, ActiveWord::Booked)],
                &pumps(&[]),
            );
            assert_eq!(
                decide_plan(&ship, &plan, &[], &pumps(&[]), &FlatRouter(10)),
                Decision::Travel {
                    station: "a".into()
                },
                "berthed at {berth}"
            );
        }
    }

    #[test]
    fn a_burned_tank_never_steers_the_route_backwards() {
        // Round-2 review, finding 2: walk the pinned interleaved route PAST the
        // fill. Both pickups done, B dropped at w (its fill executed there),
        // only A's drop at z remains — and the tank has burned low again. The
        // route must still point FORWARD to z, never back to w's pump.
        let plan = Itinerary {
            loads: vec![
                active("A", "x", "z", 50, ActiveWord::PickedUp),
                active("B", "y", "w", 60, ActiveWord::Delivered),
            ],
            stops: vec![
                Stop {
                    station: "x".into(),
                    ops: vec![StopOp::Pickup {
                        load_id: "A".into(),
                        units: 50,
                    }],
                },
                Stop {
                    station: "y".into(),
                    ops: vec![StopOp::Pickup {
                        load_id: "B".into(),
                        units: 60,
                    }],
                },
                Stop {
                    station: "w".into(),
                    ops: vec![
                        StopOp::Refuel,
                        StopOp::Drop {
                            load_id: "B".into(),
                            units: 60,
                        },
                    ],
                },
                Stop {
                    station: "z".into(),
                    ops: vec![StopOp::Drop {
                        load_id: "A".into(),
                        units: 50,
                    }],
                },
            ],
        };
        // B is Delivered → its Collect outranks movement; settle it first the
        // way the runner would, then the moving case: drop B from the loads
        // (collected) and keep its stops — the route STILL never looks back.
        let mut moving = plan.clone();
        moving.loads.retain(|a| a.row.load_id != "B");
        // Thirsty AT the pump berth: fill where the hull stands — in place,
        // which is not steering backwards.
        let mut ship = ship_at("w", 100); // 17%
        ship.hold_used = 50;
        assert_eq!(
            decide_plan(&ship, &moving, &[], &pumps(&["w"]), &FlatRouter(10)),
            Decision::Refuel
        );
        // Topped up and departed context: from z's side of the run, w is
        // history — the route points forward to z, never back to the pump.
        ship.fuel = 590; // above the top-up line
        assert_eq!(
            decide_plan(&ship, &moving, &[], &pumps(&["w"]), &FlatRouter(10)),
            Decision::Travel {
                station: "z".into()
            },
            "a visited pump is history, not a destination"
        );
        // And mid-flight to z a burned tank still holds course.
        ship.fuel = 100;
        ship.in_flight = true;
        assert_eq!(
            decide_plan(&ship, &moving, &[], &pumps(&["w"]), &FlatRouter(10)),
            Decision::Hold {
                why: "under way".into()
            }
        );
    }

    #[test]
    fn insertion_fuels_the_whole_remaining_plan_not_just_the_candidate() {
        // The candidate's own legs are cheap — but the plan still owes a long
        // laden leg first, and the shared tank must carry all of it.
        struct PerLeg;
        impl Router for PerLeg {
            fn fuel_between(&self, from: &str, to: &str) -> Option<i64> {
                Some(match (from, to) {
                    ("x", "far-y") => 400,
                    _ => 50,
                })
            }
        }
        let ship = ship_at("x", 500);
        let plan = Itinerary::sequential(
            vec![active("HELD", "w", "far-y", 25, ActiveWord::PickedUp)],
            &pumps(&[]),
        );
        let cheap = load("CHEAP", "a", "b", 900, (5, 10));
        assert_eq!(
            best_insertion(
                &ship,
                &plan,
                std::slice::from_ref(&cheap),
                &pumps(&[]),
                &PerLeg
            ),
            None,
            "500 of tank cannot fly 500·1.2 of legs"
        );
        // A pump at the plan's own destination is a planned fill the pilot will
        // execute on arrival — the walk may bank on it, and the booking opens.
        let plan_with_pump = Itinerary::sequential(
            vec![active("HELD", "w", "far-y", 25, ActiveWord::PickedUp)],
            &pumps(&["far-y"]),
        );
        assert_eq!(
            best_insertion(
                &ship,
                &plan_with_pump,
                std::slice::from_ref(&cheap),
                &pumps(&["far-y"]),
                &PerLeg
            ),
            Some("CHEAP".into())
        );
    }

    #[test]
    fn an_unpriceable_leg_refuses_the_whole_insertion() {
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
        let ship = ship_at("here", 600);
        let plan = Itinerary::sequential(
            vec![active("HELD", "a", "b", 25, ActiveWord::Booked)],
            &pumps(&[]),
        );
        let ok = load("OK", "c", "d", 900, (5, 10));
        assert_eq!(
            best_insertion(
                &ship,
                &plan,
                std::slice::from_ref(&ok),
                &pumps(&[]),
                &OneBlind
            ),
            None
        );
    }

    #[test]
    fn the_ranking_is_the_boards_rate_a_stated_placeholder_not_marginal() {
        // The codex-lane review's required regression, pinning the PLACEHOLDER
        // at its stated altitude: with a live plan ending far away, a candidate
        // whose board-priced rate is higher still ranks first, even though a
        // true marginal rate from the plan's endpoint would prefer the other.
        // This is DOCUMENTED behavior — best_insertion's comment confines the
        // booking gate to an empty plan for exactly this reason — so when a
        // marginal rate is built (UCF-Haul#43, route ticks on the Router), this
        // test is the one that must be deliberately rewritten, not silently.
        let ship = ship_at("here", 600);
        let plan = Itinerary::sequential(
            vec![active("HELD", "a", "far-end", 25, ActiveWord::Booked)],
            &pumps(&[]),
        );
        // RICH rates higher on the board (its deadhead was priced from "here");
        // NEAR sits at the plan's endpoint. The placeholder picks RICH.
        let rich = load("RICH", "elsewhere", "b", 2000, (5, 10));
        let near = load("NEAR", "far-end", "b", 1000, (5, 10));
        assert_eq!(
            best_insertion(&ship, &plan, &[rich, near], &pumps(&[]), &FlatRouter(10)),
            Some("RICH".into())
        );
    }

    #[test]
    fn only_picked_up_freight_is_aboard_delivered_and_booked_are_not() {
        // Round-3 review, finding 2: the merchant's book subtracts what freight
        // ACTUALLY occupies the hold — picked-up cargo, exactly once per load,
        // summed across loads of one good; never a delivered load's (already
        // craned off) or a booked one's (still at its origin).
        let mut delivered = active("D", "a", "b", 100, ActiveWord::Delivered);
        delivered.row.good = "ore".into();
        let mut picked = active("P", "c", "d", 40, ActiveWord::PickedUp);
        picked.row.good = "ore".into();
        let mut picked2 = active("P2", "e", "f", 25, ActiveWord::PickedUp);
        picked2.row.good = "ore".into();
        let mut booked = active("B", "g", "h", 30, ActiveWord::Booked);
        booked.row.good = "grain".into();
        let loads = [delivered, picked, picked2, booked];
        assert_eq!(freight_aboard(&loads), vec![("ore", 40), ("ore", 25)]);
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
