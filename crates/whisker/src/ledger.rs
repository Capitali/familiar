//! Reading the exchange's freight ledger — pure, so every word it learned the hard
//! way (each one a stranding on LOCAL or PROD) is pinned by a test that needs no
//! world. The runner hands `/v1/me`'s `freight` array in; loadId-keyed answers come
//! out (T-232: nothing in here assumes "the" load).
//!
//! Events are folded in TICK order, not array order (codex review, finding 6): a
//! stale row appended late cannot resurrect a closed load, while a genuinely
//! later booking — the exchange re-listing a reverted contract and the ship
//! booking it again — starts a fresh lifecycle. At one tick, the terminal word
//! wins (conservative close).

use crate::doctrine::ActiveWord;
use serde_json::Value;

/// Does this event close a load WITHOUT paying us? Every way a contract leaves the
/// ship: refused, timed out — or undone. "reverted" is the word that stranded KK II
/// at foxys-diner (booked t6195, reverted t6265, 2026-09-01): a fold can UNDO a
/// booking, and a pilot that does not know the word waits forever for a crane to
/// load a contract that no longer exists. "cancel" covers a cancelBooking too.
fn is_lost(e: &str) -> bool {
    e.contains("rejected")
        || e.contains("expired")
        || e.contains("lapsed")
        || e.contains("reverted")
        || e.contains("cancel")
}

/// Does this event settle a load — the money reached the account?
fn is_settled(e: &str) -> bool {
    e.contains("payment taken") || e.contains("collected")
}

/// One load's lifecycle, reduced from its events in tick order.
struct Lifecycle {
    /// The current life's word, if the load is open.
    word: Option<ActiveWord>,
    /// The booked tick that OPENED the current life — the plan orders by this.
    booked_tick: i64,
    /// The terminal event that closed the latest life, verbatim, if closed.
    closed_by: Option<String>,
    closed_at: i64,
}

/// Fold one load's events chronologically. Same-tick precedence: terminals are
/// applied after opens, so a book-and-revert on one tick reads closed.
fn lifecycle(events: &mut [(i64, String)]) -> Lifecycle {
    // Stable sort by tick; then a second pass applies same-tick terminals last.
    events.sort_by_key(|(t, e)| {
        (
            *t,
            is_lost(&e.to_lowercase()) || is_settled(&e.to_lowercase()),
        )
    });
    let mut life = Lifecycle {
        word: None,
        booked_tick: 0,
        closed_by: None,
        closed_at: i64::MIN,
    };
    for (t, e) in events.iter() {
        let el = e.to_lowercase();
        if is_settled(&el) || is_lost(&el) {
            life.word = None;
            life.closed_by = Some(e.clone());
            life.closed_at = *t;
            continue;
        }
        if el.contains("booked") {
            // A booking STRICTLY after the last closure is a new life; one at
            // the closure's own tick is the old life's echo and stays closed.
            if life.closed_by.is_none() || *t > life.closed_at {
                life.word = Some(ActiveWord::Booked);
                life.booked_tick = *t;
                life.closed_by = None;
            }
        } else if life.word.is_some() {
            if el.contains("delivered") {
                life.word = Some(ActiveWord::Delivered);
            } else if (el.contains("pickedup") || el.contains("picked up"))
                && life.word == Some(ActiveWord::Booked)
            {
                life.word = Some(ActiveWord::PickedUp);
            }
        }
    }
    life
}

fn events_for(me: &Value, load_id: Option<&str>) -> Vec<(String, i64, String)> {
    me.get("freight")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|f| {
                    let lid = f.get("loadId").and_then(Value::as_str)?;
                    if let Some(want) = load_id {
                        if lid != want {
                            return None;
                        }
                    }
                    let e = f.get("event").and_then(Value::as_str)?;
                    let t = f.get("tick").and_then(Value::as_i64).unwrap_or(0);
                    Some((lid.to_string(), t, e.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The ledger's last word about one load, reduced to what decides. `Ok(word)` while
/// the contract is live; `Err(reason)` when it is settled or lost — either way the
/// caller stops tracking it (with the reason for the journal).
pub fn reconcile(me: &Value, load_id: &str) -> Result<ActiveWord, String> {
    let mut events: Vec<(i64, String)> = events_for(me, Some(load_id))
        .into_iter()
        .map(|(_, t, e)| (t, e))
        .collect();
    let life = lifecycle(&mut events);
    match (life.word, life.closed_by) {
        (Some(word), _) => Ok(word),
        (None, Some(e)) => {
            let el = e.to_lowercase();
            if is_settled(&el) {
                Err(format!("settled: {e}"))
            } else {
                Err(format!("lost: {e}"))
            }
        }
        // No events at all: the fold has not shown the booking yet — hold the
        // word we had rather than inventing one.
        (None, None) => Ok(ActiveWord::Booked),
    }
}

/// Every load the ledger still shows open, ordered by the booked tick that opened
/// its current life (ties by load id) — the restart adoption list. ALL of them,
/// not the newest: a delivered-but-uncollected load beside a fresh booking is
/// uncollected money, and under a multi-load exchange (UCF-Haul#43) any
/// adopted-newest-only pilot silently abandons the rest of its plan on every
/// restart (T-232 audit, item 3). The closed vocabulary is `reconcile`'s own —
/// `reverted`/`cancel` included, so a restart can never re-adopt a load the fold
/// already undid (they drifted once; 058a87c taught reconcile the word and the
/// old adoption scan missed it).
pub fn open_loads(me: &Value) -> Vec<(i64, String)> {
    let mut per_load: std::collections::HashMap<String, Vec<(i64, String)>> =
        std::collections::HashMap::new();
    for (lid, t, e) in events_for(me, None) {
        per_load.entry(lid).or_default().push((t, e));
    }
    let mut open: Vec<(i64, String)> = per_load
        .into_iter()
        .filter_map(|(lid, mut events)| {
            let life = lifecycle(&mut events);
            life.word.map(|_| (life.booked_tick, lid))
        })
        .collect();
    open.sort();
    open
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn me(events: &[(&str, &str, i64)]) -> Value {
        json!({
            "freight": events
                .iter()
                .map(|(lid, e, t)| json!({"loadId": lid, "event": e, "tick": t}))
                .collect::<Vec<_>>()
        })
    }

    #[test]
    fn every_open_load_is_adopted_not_just_the_newest() {
        // The T-232 audit's item 3: delivered-but-uncollected money beside a fresh
        // booking. Newest-only adoption abandoned L1's payout on every restart.
        let me = me(&[
            ("L1", "booked", 100),
            ("L1", "pickedUp", 105),
            ("L1", "delivered", 130),
            ("L2", "booked", 140),
        ]);
        assert_eq!(
            open_loads(&me),
            vec![(100, "L1".to_string()), (140, "L2".to_string())]
        );
    }

    #[test]
    fn adoption_orders_by_booking_not_by_the_latest_lifecycle_event() {
        // codex review, finding 2: LA booked t100 and picked up t200 must still
        // come before LB booked t150 — the pickup does not re-date the booking.
        let me = me(&[
            ("LA", "booked", 100),
            ("LB", "booked", 150),
            ("LA", "pickedUp", 200),
        ]);
        assert_eq!(
            open_loads(&me),
            vec![(100, "LA".to_string()), (150, "LB".to_string())]
        );
    }

    #[test]
    fn a_reverted_booking_is_closed_to_adoption_too() {
        // 058a87c taught reconcile "reverted"; adoption spoke an older vocabulary
        // and would have re-adopted the undone load on the next restart.
        let me = me(&[("L1", "booked", 100), ("L1", "reverted", 170)]);
        assert!(open_loads(&me).is_empty());
    }

    #[test]
    fn a_cancelled_booking_is_closed_to_adoption_too() {
        let me = me(&[("L1", "booked", 100), ("L1", "cancelBooking", 110)]);
        assert!(open_loads(&me).is_empty());
    }

    #[test]
    fn array_order_noise_cannot_resurrect_a_closed_load() {
        // A stale "booked" row appended late, with an EARLIER tick than the
        // closure: chronology, not array order, decides — it stays closed.
        let me = me(&[
            ("L1", "booked", 100),
            ("L1", "payment taken", 120),
            ("L1", "booked", 90),
        ]);
        assert!(open_loads(&me).is_empty());
        assert!(reconcile(&me, "L1").is_err());
    }

    #[test]
    fn a_genuinely_later_booking_starts_a_new_life() {
        // codex review, finding 6: the exchange re-lists a reverted contract and
        // the ship books the same id again — a booking strictly after the
        // closure reopens, with ITS tick as the plan order.
        let me = me(&[
            ("L1", "booked", 100),
            ("L1", "reverted", 170),
            ("L1", "booked", 200),
        ]);
        assert_eq!(open_loads(&me), vec![(200, "L1".to_string())]);
        assert_eq!(reconcile(&me, "L1"), Ok(ActiveWord::Booked));
    }

    #[test]
    fn a_same_tick_book_and_revert_reads_closed() {
        // Same-tick precedence: the terminal word wins the tick.
        let me = me(&[("L1", "booked", 100), ("L1", "reverted", 100)]);
        assert!(open_loads(&me).is_empty());
    }

    #[test]
    fn reconcile_walks_one_load_by_id_and_reads_its_word() {
        let me = me(&[
            ("L1", "booked", 100),
            ("L2", "booked", 101),
            ("L1", "pickedUp", 105),
        ]);
        assert_eq!(reconcile(&me, "L1"), Ok(ActiveWord::PickedUp));
        assert_eq!(reconcile(&me, "L2"), Ok(ActiveWord::Booked));
    }

    #[test]
    fn reconcile_reports_lost_and_settled_with_the_ledgers_own_words() {
        let me = me(&[
            ("L1", "booked", 100),
            ("L1", "reverted", 170),
            ("L2", "booked", 180),
            ("L2", "payment taken", 200),
        ]);
        assert_eq!(reconcile(&me, "L1"), Err("lost: reverted".to_string()));
        assert_eq!(
            reconcile(&me, "L2"),
            Err("settled: payment taken".to_string())
        );
    }
}
