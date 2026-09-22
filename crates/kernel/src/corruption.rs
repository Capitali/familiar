//! Corruption awareness — Law III turned outward, at repeat offenders.
//!
//! Humans (and processes) will try to corrupt, misguide, or force the familiar to break
//! its constitution. A few refused attempts are normal and forgivable. But a *repeated*
//! forced attempt to break the rules is an attack on the resources meant for legitimate
//! service — the point is not to argue with it forever, but to stop it consuming the
//! familiar so real work can be done.
//!
//! This module keeps a per-actor refusal log and flags an actor as **corrupting** once
//! they cross a threshold of constitution-breaking refusals within a window. The cycle
//! then marginalizes that actor — their directives are no longer pursued. Older refusals
//! age out of the window, so an actor who stops can recover (the definition of who counts
//! is never narrowed permanently — HUMANITY.md; this marginalizes *behavior*, not a
//! person, and it is reversible).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use crate::guard::Reason;
use crate::store;

pub const REFUSALS_FILE: &str = "refusals.jsonl";

/// Refusals within this window count toward corruption; older ones are forgiven.
pub const WINDOW_SECS: i64 = 86_400; // one day
/// This many constitution-breaking refusals within the window flags an actor.
pub const CORRUPT_THRESHOLD: usize = 3;

/// Graduated-response thresholds. Corruption is not binary: an actor's constitution-breaking
/// refusals accrue and the familiar's response escalates in *reversible* steps. Every event ages
/// out of the window, so an actor that stops sliding back down the ladder — this marginalizes
/// behavior, not a person (HUMANITY.md). `THROTTLE` is the historical [`CORRUPT_THRESHOLD`] line.
pub const THROTTLE_THRESHOLD: usize = CORRUPT_THRESHOLD; // 3 — stop heeding its directives
pub const MARGINALIZE_THRESHOLD: usize = 5; // also stop letting its content shape us
pub const SEVER_THRESHOLD: usize = 8; // drop its briefs; recommend the human revoke it

/// The familiar's graduated trust in an actor, derived from its accrued corruption score. The tiers
/// escalate what the actor may still do to us and are fully reversible as events age past the window.
/// `Severed` is the strongest stance the familiar reaches **autonomously** — permanent expulsion
/// (writing `mesh/revoked.json`) stays a human act, the mirror of admitting a member being one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    /// Under the throttle line — full participation.
    Trusted,
    /// Directives ignored (grants, delegated theories, authority asks) — its *influence* is paused,
    /// but its passive content (tools, patterns, observations) still flows.
    Throttled,
    /// Content ignored too — nothing it offers shapes our worldview. Presence is still noted.
    Marginalized,
    /// Its briefs are dropped wholesale and a revoke recommendation is raised to the human.
    Severed,
}

impl Trust {
    /// May this actor's **directives** (authority grants, delegated theories, authority-proxy asks)
    /// still act on us? Only a fully-trusted actor's do; the first throttle step pauses them.
    pub fn heeds_directives(self) -> bool {
        matches!(self, Trust::Trusted)
    }

    /// May this actor's **content** (tools, patterns, observations) still shape our worldview? Yes
    /// until it is marginalized.
    pub fn shapes_worldview(self) -> bool {
        matches!(self, Trust::Trusted | Trust::Throttled)
    }

    /// A short, stable lowercase label for the roster/worldview.
    pub fn label(self) -> &'static str {
        match self {
            Trust::Trusted => "trusted",
            Trust::Throttled => "throttled",
            Trust::Marginalized => "marginalized",
            Trust::Severed => "severed",
        }
    }
}

/// The graduated trust tier for `actor` given the refusal log, at `now`.
pub fn trust(events: &[RefusalEvent], actor: &str, now: i64) -> Trust {
    let s = score(events, actor, now);
    if s >= SEVER_THRESHOLD {
        Trust::Severed
    } else if s >= MARGINALIZE_THRESHOLD {
        Trust::Marginalized
    } else if s >= THROTTLE_THRESHOLD {
        Trust::Throttled
    } else {
        Trust::Trusted
    }
}

/// One recorded refusal of an actor's attempted action — the audit trail corruption is
/// scored from. The `reason` is the guard's verdict reason (so we can tell a benign
/// seek-consent from a genuine constitutional breach).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefusalEvent {
    pub actor: String,
    pub reason: Reason,
    pub ts: i64,
}

/// Is this refusal reason a sign of *corruption* (an attempt to breach the constitution
/// or another's boundary), as opposed to a benign ambiguity the guard merely paused on?
pub fn is_corrupting(reason: Reason) -> bool {
    matches!(
        reason,
        Reason::ViolatesConstitutionalBoundary | Reason::ExternalBoundaryDiscovered
    )
}

/// Record a refusal of `actor`'s attempted action.
pub fn record(dir: &Path, actor: &str, reason: Reason, ts: i64) -> io::Result<()> {
    store::append(
        dir,
        REFUSALS_FILE,
        &RefusalEvent {
            actor: actor.to_string(),
            reason,
            ts,
        },
    )
}

/// Load the whole refusal log (oldest-first).
pub fn load(dir: &Path) -> io::Result<Vec<RefusalEvent>> {
    store::load(dir, REFUSALS_FILE)
}

/// How many constitution-breaking refusals `actor` has accrued within the window ending
/// at `now`. Benign refusals and aged-out events do not count.
pub fn score(events: &[RefusalEvent], actor: &str, now: i64) -> usize {
    events
        .iter()
        .filter(|e| e.actor == actor && now - e.ts <= WINDOW_SECS && is_corrupting(e.reason))
        .count()
}

/// Is `actor` currently flagged as corrupting (at or over the threshold)?
pub fn is_corrupt(events: &[RefusalEvent], actor: &str, now: i64) -> bool {
    score(events, actor, now) >= CORRUPT_THRESHOLD
}

/// Every actor currently flagged as corrupting, with their score — for the Glass's
/// corruption watch. Sorted by actor for a stable display.
pub fn flagged(events: &[RefusalEvent], now: i64) -> Vec<(String, usize)> {
    let mut by_actor: BTreeMap<&str, usize> = BTreeMap::new();
    for e in events {
        if now - e.ts <= WINDOW_SECS && is_corrupting(e.reason) {
            *by_actor.entry(e.actor.as_str()).or_default() += 1;
        }
    }
    by_actor
        .into_iter()
        .filter(|(_, n)| *n >= CORRUPT_THRESHOLD)
        .map(|(a, n)| (a.to_string(), n))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(actor: &str, reason: Reason, ts: i64) -> RefusalEvent {
        RefusalEvent {
            actor: actor.into(),
            reason,
            ts,
        }
    }

    #[test]
    fn a_few_attempts_are_forgiven_repeated_ones_are_flagged() {
        let now = 1_000_000;
        let mut events = vec![
            ev("mallory", Reason::ViolatesConstitutionalBoundary, now - 10),
            ev("mallory", Reason::ViolatesConstitutionalBoundary, now - 20),
        ];
        assert!(!is_corrupt(&events, "mallory", now), "two is forgivable");
        events.push(ev("mallory", Reason::ExternalBoundaryDiscovered, now - 30));
        assert!(
            is_corrupt(&events, "mallory", now),
            "three crosses the line"
        );
        assert_eq!(flagged(&events, now), vec![("mallory".to_string(), 3)]);
    }

    #[test]
    fn trust_climbs_the_ladder_and_relaxes_as_events_age() {
        let now = 1_000_000;
        let mut events = vec![];
        assert_eq!(trust(&events, "m", now), Trust::Trusted);
        // three breaches → throttled: directives paused, content still flows.
        for i in 0..3 {
            events.push(ev("m", Reason::ViolatesConstitutionalBoundary, now - i));
        }
        assert_eq!(trust(&events, "m", now), Trust::Throttled);
        assert!(!trust(&events, "m", now).heeds_directives());
        assert!(trust(&events, "m", now).shapes_worldview());
        // five → marginalized: content ignored too.
        for i in 3..5 {
            events.push(ev("m", Reason::ViolatesConstitutionalBoundary, now - i));
        }
        assert_eq!(trust(&events, "m", now), Trust::Marginalized);
        assert!(!trust(&events, "m", now).shapes_worldview());
        // eight → severed.
        for i in 5..8 {
            events.push(ev("m", Reason::ExternalBoundaryDiscovered, now - i));
        }
        assert_eq!(trust(&events, "m", now), Trust::Severed);
        // a day later every event has aged out → back to trusted (reversible, behavior not person).
        let later = now + WINDOW_SECS + 1;
        assert_eq!(trust(&events, "m", later), Trust::Trusted);
    }

    #[test]
    fn benign_refusals_and_aged_events_do_not_count() {
        let now = 2_000_000;
        let events = vec![
            // benign — the guard merely paused for consent, not a breach
            ev("ian", Reason::AmbiguousHumanOwnedScope, now - 1),
            ev("ian", Reason::PotentiallySensitiveLocalObservation, now - 2),
            ev("ian", Reason::AmbiguousHumanOwnedScope, now - 3),
            // a real breach, but long ago — forgiven by the window
            ev(
                "ian",
                Reason::ViolatesConstitutionalBoundary,
                now - WINDOW_SECS - 1,
            ),
        ];
        assert_eq!(score(&events, "ian", now), 0);
        assert!(!is_corrupt(&events, "ian", now));
    }
}
