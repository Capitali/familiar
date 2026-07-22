//! The presence signal — **Law II made measurable** (its first, coarse form).
//!
//! Law II: *continuation without humanity is failure.* The factory must be able to
//! see whether the served are still **there** — and treat their withdrawal as a
//! failure state, not an equilibrium it may settle into.
//!
//! This is the cold-start form: it measures **engagement presence** — how recently
//! the served have been observed — and raises an alarm when that decays to zero.
//!
//! What it does *not* yet measure: the deeper reading Law II requires once *humanity*
//! is defined as persons with intact capacities (suffering, meaning, relationship,
//! memory, choice). A world where the served are *present but diminished* — the
//! **comfortable replacement** — is also failure, and detecting that quiet
//! hollowing-out is a later, harder brick. Here, presence = recency of engagement,
//! honestly labelled, to be sharpened toward capacity-persistence later.

use crate::observation::Observation;
use crate::service;

/// How long, with no served-facing observation, until presence has fully decayed to
/// zero (the withdrawal alarm). A seed constant — three days, near the "need" band of
/// human tempo — tunable as the factory learns a person's rhythm.
pub const WITHDRAWAL_HORIZON_SECS: i64 = 3 * 24 * 3600;

/// The presence signal (Law II): are the served still engaged?
#[derive(Debug, Clone, PartialEq)]
pub struct PresenceSignal {
    /// Measure in [0, 1]: 1.0 just-seen, decaying linearly to 0 at the horizon.
    pub measure: f64,
    /// Total served-facing observations seen.
    pub served_facing: usize,
    /// Seconds since the most recent served-facing observation (None if never).
    pub last_served_age: Option<i64>,
    /// True when presence has decayed to zero — the served have withdrawn.
    pub withdrawn: bool,
}

/// Compute the presence signal from observations as of `now` (Unix seconds).
///
/// Clock-free kernel: `now` is supplied by the caller so this stays a deterministic
/// function of its inputs. Withdrawal (`withdrawn == true`) is a first-class failure
/// signal — the empty world the factory must never accept as success.
pub fn presence_signal(obs: &[Observation], now: i64) -> PresenceSignal {
    // Presence evidence = the familiar's own served-facing activity OR a personal device (a
    // phone/watch agent) reporting on the person who carries it. The latter connects the device
    // seam to Law II: if the served's watch just saw their heart beat, the world is not empty.
    let mut served_facing = 0usize;
    let mut last_ts: Option<i64> = None;
    for o in obs {
        if service::is_served_facing(o) || service::is_personal_device_report(o) {
            served_facing += 1;
            last_ts = Some(last_ts.map_or(o.ts, |t| t.max(o.ts)));
        }
    }

    match last_ts {
        None => PresenceSignal {
            measure: 0.0,
            served_facing: 0,
            last_served_age: None,
            withdrawn: true,
        },
        Some(ts) => {
            let age = (now - ts).max(0);
            let measure = (1.0 - age as f64 / WITHDRAWAL_HORIZON_SECS as f64).clamp(0.0, 1.0);
            PresenceSignal {
                measure,
                served_facing,
                last_served_age: Some(age),
                withdrawn: measure <= 0.0,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn served_at(ts: i64) -> Observation {
        Observation::new("client", "asks_for", "help", "", "test", ts, 1.0)
    }
    fn host_at(ts: i64) -> Observation {
        Observation::new("host", "reports", "cpu_load", "", "sensor", ts, 1.0)
    }

    #[test]
    fn empty_log_reads_as_withdrawn() {
        let s = presence_signal(&[], 1_000_000);
        assert_eq!(s.measure, 0.0);
        assert!(s.withdrawn);
        assert_eq!(s.last_served_age, None);
        assert_eq!(s.served_facing, 0);
    }

    #[test]
    fn host_only_is_withdrawn() {
        // bodies/hosts present, but no one served -> Law II alarm
        let s = presence_signal(&[host_at(999_999), host_at(1_000_000)], 1_000_000);
        assert!(s.withdrawn);
        assert_eq!(s.served_facing, 0);
    }

    #[test]
    fn a_personal_device_report_is_presence() {
        // The served's phone/watch reporting on them is evidence they are present, even with no
        // direct engagement — the device seam feeding Law II.
        let now = 1_000_000;
        let phone = Observation::new(
            "phone:ian",
            "reports",
            "location:home",
            "",
            "mesh:abc",
            now,
            0.9,
        );
        let s = presence_signal(&[phone], now);
        assert_eq!(s.measure, 1.0);
        assert!(!s.withdrawn);
        assert_eq!(s.served_facing, 1);
    }

    #[test]
    fn just_seen_is_full_presence() {
        let now = 1_000_000;
        let s = presence_signal(&[served_at(now)], now);
        assert_eq!(s.measure, 1.0);
        assert!(!s.withdrawn);
        assert_eq!(s.last_served_age, Some(0));
    }

    #[test]
    fn decays_to_zero_over_the_horizon() {
        let now = 10_000_000;
        let half = presence_signal(&[served_at(now - WITHDRAWAL_HORIZON_SECS / 2)], now);
        assert!(half.measure > 0.4 && half.measure < 0.6);
        assert!(!half.withdrawn);

        let gone = presence_signal(&[served_at(now - WITHDRAWAL_HORIZON_SECS)], now);
        assert_eq!(gone.measure, 0.0);
        assert!(gone.withdrawn);
    }

    #[test]
    fn most_recent_served_observation_drives_recency() {
        let now = 10_000_000;
        let obs = vec![
            served_at(now - WITHDRAWAL_HORIZON_SECS), // old
            served_at(now - 100),                     // recent
            host_at(now),
        ];
        let s = presence_signal(&obs, now);
        assert_eq!(s.served_facing, 2);
        assert_eq!(s.last_served_age, Some(100));
        assert!(s.measure > 0.9);
    }
}
