//! Reading the exchange's freight ledger — pure, so every word it learned the hard
//! way (each one a stranding on LOCAL or PROD) is pinned by a test that needs no
//! world. The runner hands `/v1/me`'s `freight` array in; loadId-keyed answers come
//! out (T-232: nothing in here assumes "the" load).

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

/// The ledger's last word about one load, reduced to what decides. `Ok(word)` while
/// the contract is live; `Err(reason)` when it is settled or lost — either way the
/// caller stops tracking it (with the reason for the journal).
pub fn reconcile(me: &Value, load_id: &str) -> Result<ActiveWord, String> {
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
    let mut word = ActiveWord::Booked;
    for e in events {
        let e_lower = e.to_lowercase();
        if is_settled(&e_lower) {
            return Err(format!("settled: {e}"));
        }
        if is_lost(&e_lower) {
            return Err(format!("lost: {e}"));
        }
        if e_lower.contains("delivered") {
            word = ActiveWord::Delivered;
        } else if (e_lower.contains("pickedup") || e_lower.contains("picked up"))
            && word == ActiveWord::Booked
        {
            word = ActiveWord::PickedUp;
        }
    }
    Ok(word)
}

/// Every load the ledger still shows open, oldest booking first — the restart
/// adoption list. ALL of them, not the newest: a delivered-but-uncollected load
/// beside a fresh booking is uncollected money, and under a multi-load exchange
/// (UCF-Haul#43) any adopted-newest-only pilot silently abandons the rest of its
/// plan on every restart (T-232 audit, item 3). The closed-set here is the SAME
/// vocabulary `reconcile` speaks — `reverted`/`cancel` included, so a restart can
/// never re-adopt a load the fold already undid (they drifted once; 058a87c taught
/// reconcile the word and this scan missed it).
pub fn open_loads(me: &Value) -> Vec<(i64, String)> {
    let Some(events) = me.get("freight").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut latest: std::collections::HashMap<String, (i64, bool)> =
        std::collections::HashMap::new();
    for f in events {
        let Some(lid) = f.get("loadId").and_then(Value::as_str) else {
            continue;
        };
        let e = f
            .get("event")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase();
        let t = f.get("tick").and_then(Value::as_i64).unwrap_or(0);
        let closed = is_settled(&e) || is_lost(&e);
        let opens = e.contains("booked") || e.contains("picked") || e.contains("delivered");
        let entry = latest.entry(lid.to_string()).or_insert((t, false));
        if closed {
            entry.1 = true;
        } else if opens && !entry.1 {
            entry.0 = entry.0.max(t);
        }
    }
    let mut open: Vec<(i64, String)> = latest
        .into_iter()
        .filter(|(_, (_, closed))| !closed)
        .map(|(lid, (t, _))| (t, lid))
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
            vec![(130, "L1".to_string()), (140, "L2".to_string())]
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
    fn settled_loads_stay_settled_whatever_follows_in_the_array() {
        // Once closed, a later noise event must not resurrect the load.
        let me = me(&[
            ("L1", "booked", 100),
            ("L1", "payment taken", 120),
            ("L1", "booked", 90), // out-of-order noise
        ]);
        assert!(open_loads(&me).is_empty());
    }

    #[test]
    fn adoption_order_is_oldest_first_the_plan_is_booking_order() {
        let me = me(&[("LB", "booked", 200), ("LA", "booked", 150)]);
        assert_eq!(
            open_loads(&me),
            vec![(150, "LA".to_string()), (200, "LB".to_string())]
        );
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
