//! Adoption — how a running pilot learns which contracts it already holds.
//!
//! A restart, a missed fold, or a booking acked just before a crash all leave the
//! ledger knowing about contracts the runner's memory does not. Adoption is the
//! per-cycle reconciliation that closes that gap (round-1 review, finding 1: never
//! a startup one-shot — a transient board omission must delay adoption one fold,
//! not lose a live contract forever).
//!
//! THE CONTRACT THE RUNNER OWES THIS MODULE (round-2 review, finding 3): while any
//! id is still pending — ledger-open but not yet resolved to a board row — the
//! ship's freight state is UNKNOWN, and the runner files NO new commitment: no
//! booking, no merchant buy, no carry leg, no diversion, no movement for a
//! resolved newer load. Completing what is already in motion (engaging a laid
//! course, waiting out a filed action's fold) stays allowed — those are the
//! unresolved contract's own acts, not new ones.
//!
//! An id the ledger CLOSES while pending is returned as [`AdoptOutcome::Closed`]
//! and must go through the runner's one load-close handler — cooldown, intent
//! purge, journal — exactly like a tracked load's close (round-2 review, finding
//! 4: a close that bypasses the cooldown re-books the same dead contract).

use crate::doctrine::{Active, Itinerary, LoadRow};
use crate::ledger;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};

/// One adoption step's outcome for one load id.
#[derive(Debug)]
pub enum AdoptOutcome {
    /// The ledger shows it open and a board row resolved: it joins the plan.
    Adopted(Active),
    /// The ledger closed it while it was pending — route through the runner's
    /// load-close handler (cooldown, purge, journal), never silently.
    Closed { load_id: String, reason: String },
    /// Still open, still unresolved this fold: keep holding, retry next cycle.
    Pending { load_id: String },
}

/// One cycle of adoption. Merges newly ledger-open ids into `pending` (skipping
/// ids the plan already carries), then tries to resolve every pending id through
/// `lookup` (the runner asks the three held-board statuses; tests script it).
/// Returns an outcome per id acted on; `pending` keeps exactly the ids still
/// unresolved. Ordering: `ledger::open_loads` order (booking tick), so the plan
/// stays booking-ordered as adoptions resolve.
pub fn adopt_step(
    current: &[Active],
    pending: &mut Vec<String>,
    me: &Value,
    lookup: &mut dyn FnMut(&str) -> Option<LoadRow>,
) -> Vec<AdoptOutcome> {
    for (_, lid) in ledger::open_loads(me) {
        if current.iter().all(|a| a.row.load_id != lid) && !pending.contains(&lid) {
            pending.push(lid);
        }
    }
    let mut outcomes = Vec::new();
    pending.retain(|lid| {
        let word = match ledger::reconcile(me, lid) {
            Ok(word) => word,
            Err(reason) => {
                outcomes.push(AdoptOutcome::Closed {
                    load_id: lid.clone(),
                    reason,
                });
                return false;
            }
        };
        match lookup(lid) {
            Some(row) => {
                outcomes.push(AdoptOutcome::Adopted(Active { row, word }));
                false
            }
            None => {
                outcomes.push(AdoptOutcome::Pending {
                    load_id: lid.clone(),
                });
                true
            }
        }
    });
    outcomes
}

/// One fold's freight bookkeeping before ANY action selection, as a typed step
/// the I/O loop matches on (the codex-lane round-3 review, finding 1). While
/// `may_act` is false the loop journals and sleeps — no buy, carry, booking,
/// diversion, or movement for a resolved newer load can even be REACHED,
/// because action selection lives behind the match, not behind an inline flag.
pub struct FreightStep {
    /// Contracts whose board row resolved this fold — they join the plan.
    pub adopted: Vec<Active>,
    /// Contracts the ledger closed while pending — each goes through
    /// [`close_transition`], never a silent drop.
    pub closed: Vec<(String, String)>,
    /// Every id still unresolved after this fold (for the once-per-life notice).
    pub pending: Vec<String>,
    /// False while ANY id is unresolved: the freight state is unknown and the
    /// scheduler files no new commitment this fold.
    pub may_act: bool,
}

/// Run one fold's adoption and gate decision as one pure transition.
pub fn freight_step(
    current: &[Active],
    pending: &mut Vec<String>,
    me: &Value,
    lookup: &mut dyn FnMut(&str) -> Option<LoadRow>,
) -> FreightStep {
    let outcomes = adopt_step(current, pending, me, lookup);
    let mut step = FreightStep {
        adopted: Vec::new(),
        closed: Vec::new(),
        pending: pending.clone(),
        may_act: pending.is_empty(),
    };
    for o in outcomes {
        match o {
            AdoptOutcome::Adopted(a) => step.adopted.push(a),
            AdoptOutcome::Closed { load_id, reason } => step.closed.push((load_id, reason)),
            AdoptOutcome::Pending { .. } => {}
        }
    }
    step
}

/// The journal effect a close produces — the I/O shell writes it; the mutation
/// itself already happened in [`close_transition`].
#[derive(Debug, PartialEq, Eq)]
pub struct CloseEffect {
    pub load_id: String,
    pub reason: String,
}

/// The ONE close mutation every load-leaving path goes through — tracked
/// reconcile and pending adoption alike (the codex-lane round-3 review, finding
/// 2). Its four effects, pinned: the cooldown starts (`lost_at`), the booking
/// intent is forgotten so a dead fold's idempotency id never answers for a
/// fresh one (`recent`), the adoption notice clears so a genuinely later life
/// gets a fresh one (`adopt_noted`), and exactly one journal effect returns.
pub fn close_transition(
    lost_at: &mut HashMap<String, i64>,
    recent: &mut HashMap<String, (i64, String)>,
    adopt_noted: &mut BTreeSet<String>,
    load_id: &str,
    reason: String,
    tick: i64,
) -> CloseEffect {
    lost_at.insert(load_id.to_string(), tick);
    // Exact-match purge, never substring (round-6 review, finding 4): the
    // signature is the action's JSON body, and "L1" is a substring of "L10" —
    // closing one load must not delete another's idempotency record.
    recent.retain(|sig, _| {
        serde_json::from_str::<Value>(sig)
            .ok()
            .and_then(|v| v.get("loadId").and_then(|l| l.as_str().map(String::from)))
            .map(|lid| lid != load_id)
            .unwrap_or(true)
    });
    adopt_noted.remove(load_id);
    CloseEffect {
        load_id: load_id.to_string(),
        reason,
    }
}

/// The idempotency id an intent files under — the SAME id for the same still-
/// remembered intent (retry the id, never the intent; ucf-exchange#14), a
/// freshly minted one otherwise. Pinned beside [`close_transition`] so "a dead
/// life's id never answers for a fresh one" is a provable pair, not two code
/// sites that agree by luck.
pub fn action_id_for(
    recent: &HashMap<String, (i64, String)>,
    sig: &str,
    seq: &mut u64,
    now_secs: i64,
) -> String {
    if let Some((_, id)) = recent.get(sig) {
        return id.clone();
    }
    *seq += 1;
    format!("whisker-{now_secs}-{seq}")
}

/// May this load be booked again yet? The question the board filter asks of
/// the cooldown [`close_transition`] wrote — pinned beside it so the two can
/// never drift.
pub fn bookable(lost_at: &HashMap<String, i64>, load_id: &str, tick: i64, cooldown: i64) -> bool {
    lost_at
        .get(load_id)
        .map(|t| tick - t > cooldown)
        .unwrap_or(true)
}

/// What the fold decided the runner may DO next — the typed boundary the
/// round-6 review required: action selection lives behind the `Proceed` arm of
/// the runner's match, a hold journals-and-sleeps, and a belt remedy is one
/// validated wire action. There is no way to reach the scheduler without
/// consuming this value.
#[derive(Debug, PartialEq, Eq)]
pub enum FoldAction {
    /// Ledger-open contracts are unresolved: file NOTHING new this fold.
    HoldForAdoption { pending: Vec<String> },
    /// The stale-course belt fires: exactly this one remedy toward this laid
    /// destination, already validated against the fold's own intent.
    Belt { remedy: WedgeRemedy, dest: String },
    /// The freight state is known — trade and the doctrine may act.
    Proceed,
}

/// Everything one pre-action fold decided, for the I/O shell to journal and act on.
#[derive(Debug)]
pub struct FoldOutcome {
    /// Contracts adopted this fold (already appended to the state's loads).
    pub adopted: Vec<Active>,
    /// Every close this fold — pending and tracked alike, each already applied
    /// through [`close_transition`]; journal each exactly once.
    pub closes: Vec<CloseEffect>,
    /// Pending ids whose once-per-life notice is newly due.
    pub pending_notes: Vec<String>,
    /// A laid course that did NOT match the fold's intent (journal once).
    pub mismatch_note: Option<String>,
    /// The plan compiled AFTER this fold's bookkeeping — the decision's truth.
    pub plan: Itinerary,
}

/// The facts one fold observes, handed in by the I/O shell.
pub struct FoldFacts<'a> {
    pub me: &'a Value,
    pub docked: Option<&'a str>,
    pub route: &'a [String],
    pub fuel: i64,
    pub fuel_capacity: i64,
    pub tick: i64,
    /// True when no filed action is still waiting on its fold — adoption,
    /// reconciliation, and belt EXECUTION all defer to an unfolded action,
    /// while the belt's clock keeps running (round-6 review, finding 2).
    pub folded: bool,
    /// Where the merchant would carry next, when freight is empty — the
    /// second half of the belt's destination validation.
    pub carry_intent: Option<&'a str>,
    pub pumps: &'a std::collections::BTreeSet<String>,
}

/// The runner's whole freight memory, folded as ONE pure transition per cycle
/// (all three reviews' converged demand): adoption with retry, tracked
/// reconciliation, the shared close path, booking order, the belt's clock and
/// validation — facts in, a typed action out. The I/O shell journals the
/// outcome and matches the action; it derives nothing itself.
#[derive(Debug, Default)]
pub struct FreightState {
    pub loads: Vec<Active>,
    pub pending: Vec<String>,
    pub lost_at: HashMap<String, i64>,
    pub recent: HashMap<String, (i64, String)>,
    pub adopt_noted: BTreeSet<String>,
    pub wedge: WedgeWatch,
}

impl FreightState {
    pub fn fold(
        &mut self,
        facts: &FoldFacts,
        lookup: &mut dyn FnMut(&str) -> Option<LoadRow>,
    ) -> (FoldOutcome, FoldAction) {
        let mut adopted = Vec::new();
        let mut closes = Vec::new();
        let mut pending_notes = Vec::new();

        if facts.folded {
            // Adoption — every ledger-open contract, retried until resolved.
            for outcome in adopt_step(&self.loads, &mut self.pending, facts.me, lookup) {
                match outcome {
                    AdoptOutcome::Adopted(a) => {
                        self.adopt_noted.remove(&a.row.load_id);
                        adopted.push(a.clone());
                        self.loads.push(a);
                    }
                    AdoptOutcome::Closed { load_id, reason } => {
                        closes.push(close_transition(
                            &mut self.lost_at,
                            &mut self.recent,
                            &mut self.adopt_noted,
                            &load_id,
                            reason,
                            facts.tick,
                        ));
                    }
                    AdoptOutcome::Pending { load_id } => {
                        if self.adopt_noted.insert(load_id.clone()) {
                            pending_notes.push(load_id);
                        }
                    }
                }
            }
            // Tracked reconciliation — the fold is the truth, through the SAME
            // close path as a pending close.
            let mut kept = Vec::with_capacity(self.loads.len());
            for mut a in std::mem::take(&mut self.loads) {
                match ledger::reconcile(facts.me, &a.row.load_id) {
                    Ok(word) => {
                        a.word = word;
                        kept.push(a);
                    }
                    Err(reason) => {
                        closes.push(close_transition(
                            &mut self.lost_at,
                            &mut self.recent,
                            &mut self.adopt_noted,
                            &a.row.load_id,
                            reason,
                            facts.tick,
                        ));
                    }
                }
            }
            self.loads = in_booking_order(kept, &ledger::open_loads(facts.me));
        }

        let plan = Itinerary::sequential(self.loads.clone(), facts.pumps);

        // The belt's clock runs EVERY fold — a pending adoption or an unfolded
        // action holds the FIRING, never the counting (round-6 finding 2), so
        // a course that stood ready through an adoption outage is engaged on
        // the very fold the freight state clears.
        let holds = self.pending.len() + usize::from(!facts.folded);
        let fired = self.wedge.observe(
            facts.docked,
            facts.route,
            facts.fuel,
            facts.fuel_capacity,
            holds,
            facts.tick,
        );

        let mut mismatch_note = None;
        let action = if !self.pending.is_empty() {
            FoldAction::HoldForAdoption {
                pending: self.pending.clone(),
            }
        } else if let Some(remedy) = fired {
            let intended: Option<String> = plan
                .current()
                .map(|s| s.station.clone())
                .or_else(|| facts.carry_intent.map(String::from));
            let laid = facts.route.last().cloned();
            if resume_stale_course(laid.as_deref(), intended.as_deref()) {
                FoldAction::Belt {
                    remedy,
                    dest: laid.unwrap_or_default(),
                }
            } else {
                mismatch_note = Some(format!(
                    "laid course to {laid:?} no longer matches intent {intended:?}; not resumed"
                ));
                FoldAction::Proceed
            }
        } else {
            FoldAction::Proceed
        };

        (
            FoldOutcome {
                adopted,
                closes,
                pending_notes,
                mismatch_note,
                plan,
            },
            action,
        )
    }
}

/// What one matured wedge firing is allowed to do — exactly ONE wire action
/// (round-4 review, finding 1: the old belt released an engage AND a fresh
/// travel in one fold). The first firing for a course engages it (the drive is
/// a two-step file-then-engage on some folds, UCF-Haul#65); if the same course
/// still stands a threshold later, the next firing re-files the travel — the
/// belt to the braces, one buckle per fold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WedgeRemedy {
    Engage,
    Refile,
}

/// The stale-course watch, made a type so its rules are PINNED: the belt is a
/// COMMITMENT, and no commitment is filed while any ledger-open contract is
/// unresolved (round-3 finding 1 — the watch keeps its own clock through a
/// pending hold, so it fires promptly once the freight state is known), and a
/// firing names exactly one remedy (round-4 finding 1).
#[derive(Debug, Default)]
pub struct WedgeWatch {
    key: Option<(String, Vec<String>)>,
    since: i64,
    engaged: bool,
}

impl WedgeWatch {
    /// Ticks a docked, laid, healthy-tank course must sit unmoved before the
    /// belt fires (ucf-exchange#16 — a course filed while dry never departs on
    /// its own after refuelling).
    pub const THRESHOLD_TICKS: i64 = 30;

    /// Watch one fold. Returns a remedy exactly when the belt should fire:
    /// docked, the same non-empty route standing for over the threshold, a tank
    /// above a tenth — and NOTHING pending adoption. A firing re-arms the clock
    /// and names ONE action: engage first, a re-filed travel only if the same
    /// course survives another whole threshold after the engage.
    pub fn observe(
        &mut self,
        docked: Option<&str>,
        route: &[String],
        fuel: i64,
        fuel_capacity: i64,
        pending_adoptions: usize,
        tick: i64,
    ) -> Option<WedgeRemedy> {
        let (Some(here), false) = (docked, route.is_empty()) else {
            self.key = None;
            return None;
        };
        if fuel <= fuel_capacity / 10 {
            self.key = None;
            return None;
        }
        let key = (here.to_string(), route.to_vec());
        if self.key.as_ref() != Some(&key) {
            self.key = Some(key);
            self.since = tick;
            self.engaged = false;
            return None;
        }
        if tick - self.since <= Self::THRESHOLD_TICKS {
            return None;
        }
        // The dominance rule: an unresolved contract holds the belt, without
        // resetting the clock — the moment adoption settles, the belt may fire.
        if pending_adoptions > 0 {
            return None;
        }
        self.since = tick;
        if self.engaged {
            Some(WedgeRemedy::Refile)
        } else {
            self.engaged = true;
            Some(WedgeRemedy::Engage)
        }
    }
}

/// May a stale laid course be RESUMED at all? Only when its destination is the
/// station the now-known plan (or, with no freight, the merchant's carry
/// intent) would head for anyway — a course laid before a crash or during an
/// adoption gap can point at yesterday's errand, and resuming it flies a
/// newly adopted contract the wrong way (round-4 review, finding 1). A
/// mismatched course is simply NOT resumed: the ordinary decision below files
/// the right travel itself, one action, this fold.
pub fn resume_stale_course(route_last: Option<&str>, intended: Option<&str>) -> bool {
    match (route_last, intended) {
        (Some(dest), Some(want)) => dest == want,
        _ => false,
    }
}

/// Booking order, deterministically (round-3 review, finding 3): the current
/// life's booked tick, then the load id — never the order lookups happened to
/// resolve in. A load the ledger has not shown yet sorts last with an id tie:
/// it IS the newest (a booking acked this fold).
pub fn in_booking_order(mut loads: Vec<Active>, opens: &[(i64, String)]) -> Vec<Active> {
    let rank: std::collections::HashMap<&str, i64> =
        opens.iter().map(|(t, l)| (l.as_str(), *t)).collect();
    loads.sort_by(|a, b| {
        let ka = (
            rank.get(a.row.load_id.as_str())
                .copied()
                .unwrap_or(i64::MAX),
            &a.row.load_id,
        );
        let kb = (
            rank.get(b.row.load_id.as_str())
                .copied()
                .unwrap_or(i64::MAX),
            &b.row.load_id,
        );
        ka.cmp(&kb)
    });
    loads
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctrine::ActiveWord;
    use serde_json::json;

    fn me(events: &[(&str, &str, i64)]) -> Value {
        json!({
            "freight": events
                .iter()
                .map(|(lid, e, t)| json!({"loadId": lid, "event": e, "tick": t}))
                .collect::<Vec<_>>()
        })
    }

    fn row(id: &str) -> LoadRow {
        LoadRow {
            load_id: id.into(),
            good: "kibble".into(),
            class_bps: 10_000,
            origin: "a".into(),
            dest: "b".into(),
            units: 10,
            estimated_net: 100,
            deadhead_ticks: 5,
            haul_ticks: 10,
            loading_ticks: 8,
            held_for_other: false,
        }
    }

    fn facts<'a>(
        me: &'a Value,
        docked: Option<&'a str>,
        route: &'a [String],
        tick: i64,
        pumps: &'a BTreeSet<String>,
    ) -> FoldFacts<'a> {
        FoldFacts {
            me,
            docked,
            route,
            fuel: 500,
            fuel_capacity: 600,
            tick,
            folded: true,
            carry_intent: None,
            pumps,
        }
    }

    #[test]
    fn the_scheduler_is_unreachable_between_a_missed_lookup_and_its_adoption() {
        // Round-6 finding 1, at the runner-facing boundary: the fold RETURNS the
        // action, and the shell can only act by matching it — fold one holds,
        // fold two proceeds with the contract adopted. No Buy, carry, booking,
        // diversion, belt, or movement exists outside the Proceed/Belt arms.
        let me = me(&[("L1", "booked", 100)]);
        let pumps = BTreeSet::new();
        let route: Vec<String> = Vec::new();
        let mut state = FreightState::default();

        let mut miss = |_: &str| -> Option<LoadRow> { None };
        let (out, action) = state.fold(&facts(&me, Some("berth"), &route, 100, &pumps), &mut miss);
        assert_eq!(
            action,
            FoldAction::HoldForAdoption {
                pending: vec!["L1".to_string()]
            }
        );
        assert!(out.adopted.is_empty() && out.closes.is_empty());
        assert_eq!(
            out.pending_notes,
            vec!["L1".to_string()],
            "one notice, once"
        );

        let mut hit = |lid: &str| -> Option<LoadRow> { Some(row(lid)) };
        let (out, action) = state.fold(&facts(&me, Some("berth"), &route, 101, &pumps), &mut hit);
        assert_eq!(action, FoldAction::Proceed);
        assert_eq!(out.adopted.len(), 1);
        assert!(out.pending_notes.is_empty(), "the notice was already given");
        assert_eq!(state.loads.len(), 1);
    }

    #[test]
    fn a_course_that_stood_ready_through_adoption_engages_on_the_resolving_fold() {
        // Round-6 finding 2's exact production sequence: pending L plus an
        // unchanged laid course at t100; still pending past the threshold; L
        // resolves at t141 — and the belt fires EXACTLY ONE validated Engage on
        // that same fold, because the clock ran through the hold.
        let ledger_me = me(&[("L1", "booked", 90)]);
        let pumps = BTreeSet::new();
        let route = vec!["titan-larder".to_string()];
        let mut state = FreightState::default();
        let mut miss = |_: &str| -> Option<LoadRow> { None };

        let (_, action) = state.fold(
            &facts(&ledger_me, Some("berth"), &route, 100, &pumps),
            &mut miss,
        );
        assert!(matches!(action, FoldAction::HoldForAdoption { .. }));
        let (_, action) = state.fold(
            &facts(&ledger_me, Some("berth"), &route, 135, &pumps),
            &mut miss,
        );
        assert!(
            matches!(action, FoldAction::HoldForAdoption { .. }),
            "past the threshold, still pending: the belt holds but the clock ran"
        );

        // L resolves: Booked toward titan-larder — the laid course IS the plan's
        // working station, so the resolution fold engages it, once.
        let mut hit = |lid: &str| -> Option<LoadRow> {
            let mut r = row(lid);
            r.origin = "titan-larder".into();
            Some(r)
        };
        let (out, action) = state.fold(
            &facts(&ledger_me, Some("berth"), &route, 141, &pumps),
            &mut hit,
        );
        assert_eq!(out.adopted.len(), 1);
        assert_eq!(
            action,
            FoldAction::Belt {
                remedy: WedgeRemedy::Engage,
                dest: "titan-larder".to_string()
            }
        );
        // And a course pointing somewhere the plan does NOT want is dropped.
        let me2 = me(&[("L2", "booked", 150)]);
        let stale = vec!["merchant-market".to_string()];
        let mut state2 = FreightState::default();
        let mut hit2 = |lid: &str| -> Option<LoadRow> { Some(row(lid)) }; // origin "a"
        let (_, a1) = state2.fold(&facts(&me2, Some("berth"), &stale, 200, &pumps), &mut hit2);
        assert_eq!(a1, FoldAction::Proceed);
        let (out2, a2) = state2.fold(&facts(&me2, Some("berth"), &stale, 232, &pumps), &mut hit2);
        assert_eq!(a2, FoldAction::Proceed, "mismatched course: never resumed");
        assert!(out2.mismatch_note.is_some());
    }

    #[test]
    fn both_close_paths_run_the_whole_life_cooldown_fresh_id_included() {
        // Round-6 finding 3, end to end through the production transition, BOTH
        // callers: a pending close and a tracked close each run close_transition;
        // the cooldown feeds the same `bookable` the board filter uses; the dead
        // life's intent id is never reused; a strictly later life gets a fresh
        // notice.
        let pumps = BTreeSet::new();
        let route: Vec<String> = Vec::new();

        // — the PENDING caller —
        let mut state = FreightState::default();
        let sig = "{\"loadId\":\"L1\",\"type\":\"book\"}".to_string();
        let sib = "{\"loadId\":\"L10\",\"type\":\"book\"}".to_string();
        state
            .recent
            .insert(sig.clone(), (90, "dead-id".to_string()));
        state
            .recent
            .insert(sib.clone(), (90, "sibling-id".to_string()));
        state.pending.push("L1".to_string());
        state.adopt_noted.insert("L1".to_string());
        let me1 = me(&[("L1", "booked", 100), ("L1", "reverted", 150)]);
        let mut nolookup = |_: &str| -> Option<LoadRow> { panic!("closed ids are not looked up") };
        let (out, action) = state.fold(
            &facts(&me1, Some("berth"), &route, 150, &pumps),
            &mut nolookup,
        );
        assert_eq!(out.closes.len(), 1, "exactly one close journal effect");
        assert_eq!(action, FoldAction::Proceed);
        // The cooldown the live board filter reads:
        assert!(!bookable(&state.lost_at, "L1", 150 + 60, 60));
        assert!(bookable(&state.lost_at, "L1", 150 + 61, 60));
        // The dead life's id is gone — a fresh intent mints a fresh id — while
        // the SIBLING L10's record survived the exact-match purge (finding 4):
        let mut seq = 7u64;
        let fresh = action_id_for(&state.recent, &sig, &mut seq, 999);
        assert_ne!(fresh, "dead-id");
        assert_eq!(
            action_id_for(&state.recent, &sib, &mut seq, 999),
            "sibling-id"
        );
        // A strictly later life is pending again WITH a fresh notice:
        let me2 = me(&[
            ("L1", "booked", 100),
            ("L1", "reverted", 150),
            ("L1", "booked", 230),
        ]);
        let mut miss = |_: &str| -> Option<LoadRow> { None };
        let (out, action) = state.fold(&facts(&me2, Some("berth"), &route, 231, &pumps), &mut miss);
        assert!(matches!(action, FoldAction::HoldForAdoption { .. }));
        assert_eq!(
            out.pending_notes,
            vec!["L1".to_string()],
            "the fresh life's notice"
        );

        // — the TRACKED caller —
        let mut state = FreightState::default();
        state.loads.push(Active {
            row: row("L2"),
            word: ActiveWord::Booked,
        });
        let me3 = me(&[("L2", "booked", 100), ("L2", "reverted", 160)]);
        let mut nolookup2 = |_: &str| -> Option<LoadRow> { panic!("nothing pends") };
        let (out, _) = state.fold(
            &facts(&me3, Some("berth"), &route, 160, &pumps),
            &mut nolookup2,
        );
        assert_eq!(out.closes.len(), 1, "the tracked close runs the same path");
        assert!(state.loads.is_empty());
        assert!(!bookable(&state.lost_at, "L2", 200, 60));
    }

    #[test]
    fn no_action_can_be_reached_between_a_missed_lookup_and_its_adoption() {
        // The codex-lane round-3 review, finding 1, end to end: fold one's board
        // omits ledger-open L — the step says MAY NOT ACT, and the runner's
        // action selection is structurally behind that gate. Fold two resolves
        // L — the step adopts it and opens the scheduler in the same fold.
        let me = me(&[("L1", "booked", 100)]);
        let mut pending = Vec::new();

        let mut miss = |_: &str| -> Option<LoadRow> { None };
        let step = freight_step(&[], &mut pending, &me, &mut miss);
        assert!(!step.may_act, "unknown freight state: nothing may be filed");
        assert!(step.adopted.is_empty() && step.closed.is_empty());
        assert_eq!(step.pending, vec!["L1".to_string()]);

        let mut hit = |lid: &str| -> Option<LoadRow> { Some(row(lid)) };
        let step = freight_step(&[], &mut pending, &me, &mut hit);
        assert!(step.may_act, "resolved: the scheduler opens this fold");
        assert_eq!(step.adopted.len(), 1);
        assert_eq!(step.adopted[0].row.load_id, "L1");
        assert!(step.pending.is_empty());
    }

    #[test]
    fn the_close_transition_runs_its_full_sequence_cooldown_included() {
        // The codex-lane round-3 review, finding 2, the whole life: pending
        // open → reverted while pending → closed through the ONE transition →
        // not bookable inside the cooldown → bookable after → a strictly later
        // booked life arrives with a fresh notice and a fresh intent.
        let mut lost_at: HashMap<String, i64> = HashMap::new();
        let mut recent: HashMap<String, (i64, String)> = HashMap::new();
        let mut adopt_noted: BTreeSet<String> = BTreeSet::new();

        // The dead booking's intent and notice, as the runner would hold them.
        recent.insert(
            "{\"loadId\":\"L1\",\"type\":\"book\"}".to_string(),
            (150, "whisker-1-1".to_string()),
        );
        adopt_noted.insert("L1".to_string());

        // The ledger closes L1 while it is pending adoption.
        let me1 = me(&[("L1", "booked", 100), ("L1", "reverted", 150)]);
        let mut pending = vec!["L1".to_string()];
        let mut lookup = |_: &str| -> Option<LoadRow> { panic!("closed ids are not looked up") };
        let step = freight_step(&[], &mut pending, &me1, &mut lookup);
        assert_eq!(
            step.closed,
            vec![("L1".to_string(), "lost: reverted".to_string())]
        );
        assert!(step.may_act, "the close resolved the pending id");

        let (lid, reason) = step.closed.into_iter().next().unwrap();
        let effect = close_transition(
            &mut lost_at,
            &mut recent,
            &mut adopt_noted,
            &lid,
            reason,
            150,
        );
        assert_eq!(
            effect,
            CloseEffect {
                load_id: "L1".into(),
                reason: "lost: reverted".into()
            }
        );
        assert!(
            recent.is_empty(),
            "the dead booking's intent id is forgotten"
        );
        assert!(adopt_noted.is_empty(), "a later life earns a fresh notice");

        // Re-listed inside the cooldown: not bookable. After it: bookable.
        assert!(!bookable(&lost_at, "L1", 150 + 60, 60));
        assert!(bookable(&lost_at, "L1", 150 + 61, 60));
        assert!(bookable(&lost_at, "unrelated", 151, 60));

        // A strictly later booked life is seen again — pending, fresh notice
        // possible (adopt_noted no longer remembers the dead life).
        let me2 = me(&[
            ("L1", "booked", 100),
            ("L1", "reverted", 150),
            ("L1", "booked", 230),
        ]);
        let mut miss = |_: &str| -> Option<LoadRow> { None };
        let step = freight_step(&[], &mut pending, &me2, &mut miss);
        assert_eq!(step.pending, vec!["L1".to_string()]);
        assert!(!step.may_act);
        assert!(
            adopt_noted.insert("L1".to_string()),
            "the fresh life's notice is genuinely fresh"
        );
    }

    #[test]
    fn a_failed_lookup_retries_next_cycle_and_then_adopts() {
        // Round-1 finding 1's exact scenario: the ledger names an open load but
        // the board omits it this fold. The id must stay pending — the gate the
        // runner owes — and resolve on the next cycle's successful read.
        let me = me(&[("L1", "booked", 100), ("L1", "pickedUp", 110)]);
        let mut pending = Vec::new();

        let mut first = |_: &str| -> Option<LoadRow> { None };
        let outcomes = adopt_step(&[], &mut pending, &me, &mut first);
        assert!(
            matches!(outcomes.as_slice(), [AdoptOutcome::Pending { load_id }] if load_id == "L1")
        );
        assert_eq!(pending, vec!["L1".to_string()], "the id survives the miss");

        let mut second = |lid: &str| -> Option<LoadRow> { Some(row(lid)) };
        let outcomes = adopt_step(&[], &mut pending, &me, &mut second);
        match outcomes.as_slice() {
            [AdoptOutcome::Adopted(a)] => {
                assert_eq!(a.row.load_id, "L1");
                assert_eq!(a.word, ActiveWord::PickedUp);
            }
            other => panic!("expected adoption, got {other:?}"),
        }
        assert!(pending.is_empty());
    }

    #[test]
    fn a_load_closed_while_pending_reports_the_close_it_never_vanishes() {
        // Round-2 finding 4: the runner must hear about this close so the
        // cooldown and intent purge run — a silent drop re-books the dead id.
        let me = me(&[("L1", "booked", 100), ("L1", "reverted", 150)]);
        let mut pending = vec!["L1".to_string()];
        let mut lookup = |_: &str| -> Option<LoadRow> { panic!("closed ids are not looked up") };
        let outcomes = adopt_step(&[], &mut pending, &me, &mut lookup);
        assert!(matches!(
            outcomes.as_slice(),
            [AdoptOutcome::Closed { load_id, reason }]
                if load_id == "L1" && reason == "lost: reverted"
        ));
        assert!(pending.is_empty());
    }

    #[test]
    fn the_belt_never_fires_while_an_adoption_is_unresolved() {
        // Round-3 finding 1: ledger-open L is pending, the ship is docked with
        // the same laid route past the threshold — the belt holds, and fires on
        // the very fold the pending clears (its clock was never reset).
        let mut w = WedgeWatch::default();
        let route = vec!["titan-larder".to_string()];
        assert!(w.observe(Some("berth"), &route, 500, 600, 1, 100).is_none());
        assert!(
            w.observe(Some("berth"), &route, 500, 600, 1, 140).is_none(),
            "past the threshold, still pending: held"
        );
        assert_eq!(
            w.observe(Some("berth"), &route, 500, 600, 0, 141),
            Some(WedgeRemedy::Engage),
            "pending cleared: the belt fires without waiting a fresh threshold"
        );
        assert!(
            w.observe(Some("berth"), &route, 500, 600, 0, 142).is_none(),
            "firing re-armed the clock"
        );
    }

    #[test]
    fn the_belt_still_fires_normally_when_nothing_is_pending() {
        let mut w = WedgeWatch::default();
        let route = vec!["b".to_string()];
        assert!(w.observe(Some("a"), &route, 500, 600, 0, 100).is_none());
        assert!(
            w.observe(Some("a"), &route, 500, 600, 0, 130).is_none(),
            "at the threshold: not yet"
        );
        assert_eq!(
            w.observe(Some("a"), &route, 500, 600, 0, 131),
            Some(WedgeRemedy::Engage)
        );
        // A changed route, an empty route, or a thin tank all stand the watch down.
        assert!(w.observe(Some("a"), &[], 500, 600, 0, 132).is_none());
        assert!(w.observe(Some("a"), &route, 50, 600, 0, 170).is_none());
    }

    #[test]
    fn one_remedy_per_firing_engage_first_refile_a_threshold_later() {
        // Round-4 finding 1: a matured course gets exactly ONE action per fold —
        // engage on the first firing; the re-filed travel only if the SAME
        // course survives a further whole threshold; a new course starts over.
        let mut w = WedgeWatch::default();
        let route = vec!["b".to_string()];
        assert!(w.observe(Some("a"), &route, 500, 600, 0, 100).is_none());
        assert_eq!(
            w.observe(Some("a"), &route, 500, 600, 0, 131),
            Some(WedgeRemedy::Engage)
        );
        assert!(
            w.observe(Some("a"), &route, 500, 600, 0, 160).is_none(),
            "the engage bought the course a fresh threshold"
        );
        assert_eq!(
            w.observe(Some("a"), &route, 500, 600, 0, 162),
            Some(WedgeRemedy::Refile),
            "still wedged a threshold after the engage: the belt to the braces"
        );
        let other = vec!["c".to_string()];
        assert!(w.observe(Some("a"), &other, 500, 600, 0, 163).is_none());
        assert_eq!(
            w.observe(Some("a"), &other, 500, 600, 0, 194),
            Some(WedgeRemedy::Engage),
            "a different course starts its remedies over"
        );
    }

    #[test]
    fn a_mismatched_stale_course_is_never_resumed() {
        // Round-4 finding 1: a course laid toward merchant market M while load L
        // (A→B) was unresolved must NOT be resumed once L is known — the plan's
        // own decision files the right travel instead.
        assert!(!resume_stale_course(Some("market-m"), Some("origin-a")));
        assert!(resume_stale_course(Some("origin-a"), Some("origin-a")));
        assert!(
            !resume_stale_course(Some("market-m"), None),
            "no intent, no resume"
        );
        assert!(!resume_stale_course(None, Some("origin-a")));
    }

    #[test]
    fn plan_order_is_booked_tick_then_id_never_lookup_resolution_order() {
        // Round-3 finding 3's delayed-resolution scenario: L1 and L2 booked the
        // same tick, L2's lookup resolved a cycle earlier — the plan still reads
        // [L1, L2], and a ledger-unseen load sorts last.
        let resolved_order = vec![
            Active {
                row: row("L2"),
                word: ActiveWord::Booked,
            },
            Active {
                row: row("L3-just-acked"),
                word: ActiveWord::Booked,
            },
            Active {
                row: row("L1"),
                word: ActiveWord::Booked,
            },
        ];
        let opens = vec![(100, "L1".to_string()), (100, "L2".to_string())];
        let ordered = in_booking_order(resolved_order, &opens);
        assert_eq!(
            ordered
                .iter()
                .map(|a| a.row.load_id.as_str())
                .collect::<Vec<_>>(),
            vec!["L1", "L2", "L3-just-acked"]
        );
    }

    #[test]
    fn a_load_the_plan_already_carries_is_not_re_adopted() {
        let me = me(&[("L1", "booked", 100)]);
        let held = [Active {
            row: row("L1"),
            word: ActiveWord::Booked,
        }];
        let mut pending = Vec::new();
        let mut lookup = |_: &str| -> Option<LoadRow> { panic!("nothing should be looked up") };
        let outcomes = adopt_step(&held, &mut pending, &me, &mut lookup);
        assert!(outcomes.is_empty());
        assert!(pending.is_empty());
    }
}
