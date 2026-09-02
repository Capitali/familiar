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

use crate::doctrine::{Active, LoadRow};
use crate::ledger;
use serde_json::Value;

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

/// The stale-course watch, made a type so its dominance rule is PINNED (round-3
/// review, finding 1): the belt — an `engage` plus a re-filed `travel` — is a
/// COMMITMENT, and no commitment is filed while any ledger-open contract is
/// unresolved. The watch keeps its own clock; a pending adoption does not reset
/// it, so the belt fires promptly once the freight state is known again.
#[derive(Debug, Default)]
pub struct WedgeWatch {
    key: Option<(String, Vec<String>)>,
    since: i64,
}

impl WedgeWatch {
    /// Ticks a docked, laid, healthy-tank course must sit unmoved before the
    /// belt fires (ucf-exchange#16 — a course filed while dry never departs on
    /// its own after refuelling).
    pub const THRESHOLD_TICKS: i64 = 30;

    /// Watch one fold. Returns true exactly when the belt should fire: docked,
    /// the same non-empty route standing for over the threshold, a tank above
    /// a tenth — and NOTHING pending adoption. Firing re-arms the clock.
    pub fn observe(
        &mut self,
        docked: Option<&str>,
        route: &[String],
        fuel: i64,
        fuel_capacity: i64,
        pending_adoptions: usize,
        tick: i64,
    ) -> bool {
        let (Some(here), false) = (docked, route.is_empty()) else {
            self.key = None;
            return false;
        };
        if fuel <= fuel_capacity / 10 {
            self.key = None;
            return false;
        }
        let key = (here.to_string(), route.to_vec());
        if self.key.as_ref() != Some(&key) {
            self.key = Some(key);
            self.since = tick;
            return false;
        }
        if tick - self.since <= Self::THRESHOLD_TICKS {
            return false;
        }
        // The dominance rule: an unresolved contract holds the belt, without
        // resetting the clock — the moment adoption settles, the belt may fire.
        if pending_adoptions > 0 {
            return false;
        }
        self.since = tick;
        true
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
        assert!(!w.observe(Some("berth"), &route, 500, 600, 1, 100));
        assert!(
            !w.observe(Some("berth"), &route, 500, 600, 1, 140),
            "past the threshold, still pending: held"
        );
        assert!(
            w.observe(Some("berth"), &route, 500, 600, 0, 141),
            "pending cleared: the belt fires without waiting a fresh threshold"
        );
        assert!(
            !w.observe(Some("berth"), &route, 500, 600, 0, 142),
            "firing re-armed the clock"
        );
    }

    #[test]
    fn the_belt_still_fires_normally_when_nothing_is_pending() {
        let mut w = WedgeWatch::default();
        let route = vec!["b".to_string()];
        assert!(!w.observe(Some("a"), &route, 500, 600, 0, 100));
        assert!(
            !w.observe(Some("a"), &route, 500, 600, 0, 130),
            "at the threshold: not yet"
        );
        assert!(w.observe(Some("a"), &route, 500, 600, 0, 131));
        // A changed route, an empty route, or a thin tank all stand the watch down.
        assert!(!w.observe(Some("a"), &[], 500, 600, 0, 132));
        assert!(!w.observe(Some("a"), &route, 50, 600, 0, 170));
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
