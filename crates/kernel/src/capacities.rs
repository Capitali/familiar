//! The capacities signal — **Law II deepened toward [HUMANITY.md](../../docs/HUMANITY.md)**.
//!
//! Presence ([`crate::presence`]) asks whether the served are *there*. This asks the
//! harder question: are they still *exercising the capacities that make them human* —
//! suffering, meaning, relationship, memory, choice — or are they being flattened into
//! monotony and compliance? A world "safe but sedated, obedient, fully managed" is a
//! failure of the humanity rule (the **comfortable replacement**), and presence alone
//! cannot see it.
//!
//! This is a deliberately coarse cold-start (capacities are not truly reducible to a
//! verb lexicon — see the honesty note in `docs/bias-and-limitations.md`). It reads
//! two cheap proxies from served-facing observations and flags **diminishment**:
//! present, but hollowed out. It is meant to be sharpened, not trusted as final.

use crate::observation::Observation;
use crate::service;

/// Actions that evidence a capacity being *exercised* — choice, initiation, dissent,
/// meaning-making, relationship, memory, care. Lowercase substring match.
const AGENCY_MARKERS: &[&str] = &[
    "ask", "request", "choose", "decide", "refuse", "decline", "question", "propose", "initiate",
    "demand", "prefer", "create", "make", "build", "teach", "share", "connect", "argue", "grieve",
    "care", "help", "tell", "remember", "object",
];

/// Actions that evidence the *opposite* — passivity, management, compliance. Stems
/// chosen to catch common inflections (e.g. "complies"/"compliance") without matching
/// dissent ("complain"); a coarse cold-start, not a definition.
const PASSIVE_MARKERS: &[&str] = &[
    "comply", "complie", "complian", "obey", "accept", "consum", "follow", "wait", "receiv",
    "submit",
];

fn matches_any(action: &str, set: &[&str]) -> bool {
    let a = action.to_ascii_lowercase();
    set.iter().any(|m| a.contains(m))
}

/// The capacities signal.
#[derive(Debug, Clone, PartialEq)]
pub struct CapacitiesSignal {
    /// Vitality of exercised capacities, 0..1 (blend of agency and variety).
    pub measure: f64,
    /// Fraction of served-facing activity that evidences agency (vs passivity).
    pub agency: f64,
    /// Diversity of served-facing activity (distinct patterns / total), 0..1.
    pub variety: f64,
    /// Served-facing activity counted.
    pub served_facing: usize,
    /// Present but hollowed out — the comfortable-replacement alarm. Only raised once
    /// there is enough served-facing activity to judge.
    pub diminished: bool,
}

/// Minimum served-facing observations before diminishment is judged (avoid noise).
const MIN_SAMPLE: usize = 3;
/// Below this vitality, present served activity reads as diminished.
const DIMINISHED_BELOW: f64 = 0.34;

/// Compute the capacities signal from observations. Zero served-facing activity is
/// *withdrawal* (presence's domain), not diminishment — so `diminished` stays false.
pub fn capacities_signal(obs: &[Observation]) -> CapacitiesSignal {
    let served: Vec<&Observation> = obs
        .iter()
        .filter(|o| service::is_served_facing(o))
        .collect();
    let n = served.len();
    if n == 0 {
        return CapacitiesSignal {
            measure: 0.0,
            agency: 0.0,
            variety: 0.0,
            served_facing: 0,
            diminished: false,
        };
    }

    // agency: share of served-facing actions that exercise a capacity. A passive
    // marker counts against; an agency marker counts for; unmarked is neutral (0.5).
    let mut agency_sum = 0.0;
    for o in &served {
        agency_sum += if matches_any(&o.action, AGENCY_MARKERS) {
            1.0
        } else if matches_any(&o.action, PASSIVE_MARKERS) {
            0.0
        } else {
            0.5
        };
    }
    let agency = agency_sum / n as f64;

    // variety: distinct (action, object) patterns over total — collapses toward 1/n
    // under monotony (the sedated signal).
    let mut seen = std::collections::HashSet::new();
    for o in &served {
        seen.insert((o.action.clone(), o.object.clone()));
    }
    let variety = seen.len() as f64 / n as f64;

    let measure = 0.5 * agency + 0.5 * variety;
    let diminished = n >= MIN_SAMPLE && measure < DIMINISHED_BELOW;

    CapacitiesSignal {
        measure,
        agency,
        variety,
        served_facing: n,
        diminished,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn o(actor: &str, action: &str, object: &str) -> Observation {
        Observation::new(actor, action, object, "", "test", 0, 1.0)
    }

    #[test]
    fn no_served_activity_is_not_diminishment() {
        let s = capacities_signal(&[o("host", "reports", "cpu")]);
        assert_eq!(s.served_facing, 0);
        assert!(!s.diminished); // that's withdrawal (presence), not diminishment
    }

    #[test]
    fn varied_agency_reads_as_vital() {
        let obs = vec![
            o("client", "asks_for", "report"),
            o("user", "decides", "plan"),
            o("customer", "creates", "design"),
        ];
        let s = capacities_signal(&obs);
        assert!(s.agency > 0.9 && s.variety > 0.9);
        assert!(s.measure > 0.9 && !s.diminished);
    }

    #[test]
    fn monotonous_compliance_reads_as_diminished() {
        // the same passive action, repeated — present, but sedated/obedient
        let obs = vec![
            o("user", "complies_with", "schedule"),
            o("user", "complies_with", "schedule"),
            o("user", "complies_with", "schedule"),
            o("user", "complies_with", "schedule"),
        ];
        let s = capacities_signal(&obs);
        assert!(s.agency < 0.1, "passive action -> low agency");
        assert!(s.variety < 0.4, "monotony -> low variety");
        assert!(
            s.diminished,
            "present but hollowed = the comfortable replacement"
        );
    }

    #[test]
    fn small_samples_are_not_judged_diminished() {
        let obs = vec![o("user", "obeys", "order")];
        let s = capacities_signal(&obs);
        assert!(!s.diminished); // too few to judge
    }
}
