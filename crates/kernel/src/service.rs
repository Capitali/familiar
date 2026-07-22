//! The service signal — **Law I made measurable**.
//!
//! Law I: *continuation is service.* The factory cannot justify its own
//! continuation apart from service to humanity, so it must be able to *see*
//! whether it is serving. This module is the first, honest version of that sight.
//!
//! It is deliberately v1-simple — the way v1's drives started on promotion-rate
//! before redundancy. With only observations to read (loops, candidates, and
//! trials port in later bricks), it measures **served-facing attention**: how much
//! of what the factory observes concerns the humans it exists to serve. That is
//! the cold-start *proxy* for service, not service rendered; later bricks fold in
//! whether observed needs are actually being reduced.

use crate::observation::Observation;

/// Tight, lowercase markers that a name concerns the served (a human system).
/// Ported from v1's `domain_is_steward` (`factory/src/drive.c`). This is a
/// cold-start classifier: it reads served-facing-ness off the words themselves.
/// Resolving proper names (e.g. "betty" -> person) waits for entity tagging (the
/// world-model port) — exactly as in v1, where a name became served-facing only
/// once a thread tagged its entity.
const STEWARD_MARKERS: &[&str] = &[
    "human",
    "person",
    "people",
    "social",
    "community",
    "client",
    "customer",
    "user",
    "steward",
];

/// Does this text name a served (human-system) entity?
pub fn names_served(text: &str) -> bool {
    let hay = text.to_ascii_lowercase();
    STEWARD_MARKERS.iter().any(|m| hay.contains(m))
}

/// The served term in an observation (actor preferred, else object), if any.
fn served_term(obs: &Observation) -> Option<&str> {
    if names_served(&obs.actor) {
        Some(&obs.actor)
    } else if names_served(&obs.object) {
        Some(&obs.object)
    } else {
        None
    }
}

/// Is this observation served-facing — does its actor or object concern the served?
pub fn is_served_facing(obs: &Observation) -> bool {
    served_term(obs).is_some()
}

/// Actor namespaces of a **personal device** — a phone/watch/iPad an agent runs on, reporting on
/// the person who carries it. An observation from one of these is direct evidence that person is
/// *present* (the device saw them), which the presence signal counts even though it is not the
/// familiar's own served-facing activity (so it does not inflate the service signal).
const PERSONAL_DEVICE_PREFIXES: &[&str] = &["phone:", "watch:", "ipad:", "iphone:"];

/// Is this a personal-device report about its owner (e.g. `phone:ian`, `watch:ian`)?
pub fn is_personal_device_report(obs: &Observation) -> bool {
    let actor = obs.actor.to_ascii_lowercase();
    PERSONAL_DEVICE_PREFIXES
        .iter()
        .any(|p| actor.starts_with(p))
}

/// The service signal (Law I): to what degree is the factory's attention on the
/// humans it exists to serve?
#[derive(Debug, Clone, PartialEq)]
pub struct ServiceSignal {
    /// Measure in [0, 1]. Zero means nothing observed touches the served.
    pub measure: f64,
    /// Observations that touch the served.
    pub served_facing: usize,
    /// Observations considered.
    pub total: usize,
    /// A representative served entity, for the human-facing summary.
    pub exemplar: Option<String>,
}

/// Saturation constant: this many served-facing observations reads as 0.5.
/// Absolute (not a ratio), faithful to v1's saturating stewardship drive.
const HALF: f64 = 3.0;

/// Compute the service signal from observations.
///
/// Zero when nothing observed touches the served — an empty-of-service log is not
/// success (the seed of Law II). Rises, saturating, with served-facing attention.
pub fn service_signal(obs: &[Observation]) -> ServiceSignal {
    let mut served_facing = 0usize;
    let mut exemplar = None;
    for o in obs {
        if let Some(term) = served_term(o) {
            served_facing += 1;
            if exemplar.is_none() {
                exemplar = Some(term.to_string());
            }
        }
    }
    let n = served_facing as f64;
    ServiceSignal {
        measure: n / (n + HALF),
        served_facing,
        total: obs.len(),
        exemplar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(actor: &str, object: &str) -> Observation {
        Observation::new(actor, "does", object, "", "test", 0, 1.0)
    }

    #[test]
    fn classifier_matches_markers_not_bare_names() {
        assert!(names_served("client_request"));
        assert!(names_served("CUSTOMER")); // case-insensitive
        assert!(names_served("the user account"));
        assert!(!names_served("cpu_load"));
        assert!(!names_served("betty")); // proper names need entity resolution (deferred)
    }

    #[test]
    fn zero_when_nothing_serves() {
        let host_only = vec![obs("host", "cpu_load"), obs("disk", "free_space")];
        let s = service_signal(&host_only);
        assert_eq!(s.measure, 0.0);
        assert_eq!(s.served_facing, 0);
        assert_eq!(s.total, 2);
        assert_eq!(s.exemplar, None);
    }

    #[test]
    fn rises_with_served_facing_attention() {
        let one = service_signal(&[obs("client", "status_report"), obs("host", "cpu_load")]);
        assert_eq!(one.served_facing, 1);
        assert!(one.measure > 0.0);
        assert_eq!(one.exemplar.as_deref(), Some("client"));

        let three = service_signal(&[
            obs("client", "status_report"),
            obs("host", "billing_for_customer"), // served via object
            obs("user", "login"),
        ]);
        assert_eq!(three.served_facing, 3);
        // monotonic: more served-facing attention reads as more service
        assert!(three.measure > one.measure);
    }

    #[test]
    fn empty_log_is_zero() {
        let s = service_signal(&[]);
        assert_eq!(s.measure, 0.0);
        assert_eq!(s.total, 0);
    }
}
