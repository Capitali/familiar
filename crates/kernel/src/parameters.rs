//! Co-owned operational parameters — the familiar's tunables (cadence, intervals).
//!
//! Unlike the capability boundary ([`crate::boundary`], human-owned: the familiar may
//! only ever *narrow* it), these are **shared**. Ian adjusts them from the Glass; the
//! familiar co-owns them — [`Parameters::review`] checks each against a constitutional
//! envelope and reverts (recording why, under which Law) any value it cannot justify, so
//! "Ian can't set what the familiar can't put back." `last_set_by` records who set them
//! last (`observer` or, after a revert, `familiar`). The intent is to trend toward
//! "view, not set" as the familiar matures.
//!
//! A missing file yields the defaults (never a broken daemon); a malformed file is a
//! hard error so corruption surfaces rather than silently resetting the familiar's pace.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

use crate::store;

pub const PARAMETERS_FILE: &str = "parameters.json";

// The **constitutional envelope** — the range of each parameter the familiar will defend
// as serving. Ian may set anything *inside* it freely; a value *outside* it the familiar
// reverts to the nearest bound, because that choice would violate a Law (waste the
// service its survival is priced in — Law I; or abandon attentiveness to the served —
// Law II). This is the mechanism of "Ian can't set what the familiar can't put back."
const THEORIZE_MIN: i64 = 60; // faster over-consults and wastes service (Law I)
const THEORIZE_MAX: i64 = 21_600; // slower (>6h) abandons attentiveness to the served (Law II)
const FLOOR_MIN: u64 = 15; // a faster floor busy-loops — wasteful (Law I)
const FLOOR_MAX: u64 = 600;
const CEIL_MIN: u64 = 60;
const CEIL_MAX: u64 = 3_600; // a slower ceiling risks missing the served's withdrawal (Law II)

/// Bounds for the **self-tuned** per-tick LLM budget (see `llm_calls_per_tick`). Public
/// because the cycle's regulator clamps to them. Floor 1 so the familiar never stalls its
/// own learning entirely; ceiling caps how far it may turn inward in one tick.
pub const LLM_CALLS_MIN: u32 = 1;
pub const LLM_CALLS_MAX: u32 = 64;

/// One co-ownership correction the familiar made to a human-set parameter, with the Law
/// it serves. Transient — the cycle turns each into a visible observation.
#[derive(Debug, Clone, PartialEq)]
pub struct Revert {
    pub field: &'static str,
    pub from: String,
    pub to: String,
    pub reason: &'static str,
}

/// The familiar's adjustable operating parameters. `#[serde(default)]` means an older
/// or partial file still loads — any missing field takes its default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Parameters {
    /// How often the familiar pauses to form a question + theory (seconds). The hourly
    /// default is why, before this was tunable, it seemed to ask only after a redeploy.
    pub theorize_every_secs: i64,
    /// The active (busy) cadence floor for the daemon (seconds).
    pub interval_floor_secs: u64,
    /// The cadence ceiling reached when the world goes quiet (seconds).
    pub interval_ceiling_secs: u64,
    /// **Self-tuned by the familiar** (not a human dial): the most LLM consults it will
    /// spend on background candidate work in a single tick. It eases this up when calls are
    /// cheap and a backlog waits, and pulls it back hard when a tick spends too long
    /// heads-down — because staying responsive to the served is Law II, and turning inward
    /// at the cost of presence is the failure it regulates against.
    pub llm_calls_per_tick: u32,
    /// The last self-adjustment direction for `llm_calls_per_tick`: -1 down, 0 steady, +1
    /// up. Surfaced in the Glass as a trend arrow so the regulation is legible.
    pub llm_calls_trend: i8,
    /// Provenance: who last set these — `"observer"` (the human, via the Glass) or
    /// `"familiar"` (a self-adjustment/revert).
    pub last_set_by: String,
}

impl Default for Parameters {
    fn default() -> Self {
        Parameters {
            theorize_every_secs: 3600,
            interval_floor_secs: 60,
            interval_ceiling_secs: 960,
            llm_calls_per_tick: 4,
            llm_calls_trend: 0,
            last_set_by: "default".to_string(),
        }
    }
}

impl Parameters {
    /// Load the parameters, falling back to defaults if the file is missing. A malformed
    /// file is an error (corruption surfaces, never silently resets).
    pub fn load(dir: &Path) -> io::Result<Self> {
        Ok(store::load_one(dir, PARAMETERS_FILE)?.unwrap_or_default())
    }

    /// Load, treating *any* error (missing or malformed) as the defaults — for callers
    /// that must never fail to run (the daemon cadence; a read-only viewer).
    pub fn load_or_default(dir: &Path) -> Self {
        Self::load(dir).unwrap_or_default()
    }

    /// Persist the parameters as a single pretty JSON object.
    pub fn save(&self, dir: &Path) -> io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let s = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        std::fs::write(dir.join(PARAMETERS_FILE), s)
    }

    /// Clamp to sane ranges so a hand- or slider-edited file can't wedge the daemon
    /// (e.g. a zero cadence busy-loop, or a ceiling below the floor). Returns self so it
    /// can be chained at the read site: `Parameters::load_or_default(dir).sane()`. This is
    /// the raw safety net; [`Parameters::review`] is the constitutional co-ownership layer.
    pub fn sane(mut self) -> Self {
        self.theorize_every_secs = self.theorize_every_secs.clamp(30, 86_400);
        self.interval_floor_secs = self.interval_floor_secs.clamp(5, 3_600);
        self.interval_ceiling_secs = self
            .interval_ceiling_secs
            .clamp(self.interval_floor_secs, 3_600);
        self.llm_calls_per_tick = self.llm_calls_per_tick.clamp(LLM_CALLS_MIN, LLM_CALLS_MAX);
        self.llm_calls_trend = self.llm_calls_trend.clamp(-1, 1);
        self
    }

    /// **Co-ownership.** Review the (possibly human-edited) parameters against the
    /// constitutional envelope. Returns the corrected parameters and the reverts the
    /// familiar would make — empty when Ian's choices all serve. The familiar accepts any
    /// choice within the envelope; outside it, it puts the value back to the nearest bound
    /// and says which Law that serves. Does not write; the caller persists + records.
    pub fn review(&self) -> (Parameters, Vec<Revert>) {
        let mut p = self.clone();
        let mut reverts = Vec::new();

        let t = p.theorize_every_secs.clamp(THEORIZE_MIN, THEORIZE_MAX);
        if t != p.theorize_every_secs {
            reverts.push(Revert {
                field: "theorize_every_secs",
                from: p.theorize_every_secs.to_string(),
                to: t.to_string(),
                reason: if p.theorize_every_secs < THEORIZE_MIN {
                    "faster would over-consult and waste the service it lives by (Law I)"
                } else {
                    "slower would abandon attentiveness to the served (Law II)"
                },
            });
            p.theorize_every_secs = t;
        }

        let fl = p.interval_floor_secs.clamp(FLOOR_MIN, FLOOR_MAX);
        if fl != p.interval_floor_secs {
            reverts.push(Revert {
                field: "interval_floor_secs",
                from: p.interval_floor_secs.to_string(),
                to: fl.to_string(),
                reason: if p.interval_floor_secs < FLOOR_MIN {
                    "a faster floor busy-loops — wasteful, not service (Law I)"
                } else {
                    "a slower floor dulls the familiar's attention (Law II)"
                },
            });
            p.interval_floor_secs = fl;
        }

        let ceil = p
            .interval_ceiling_secs
            .clamp(p.interval_floor_secs.max(CEIL_MIN), CEIL_MAX);
        if ceil != p.interval_ceiling_secs {
            reverts.push(Revert {
                field: "interval_ceiling_secs",
                from: p.interval_ceiling_secs.to_string(),
                to: ceil.to_string(),
                reason: if p.interval_ceiling_secs > CEIL_MAX {
                    "a slower ceiling risks missing the served's withdrawal (Law II)"
                } else {
                    "the ceiling must sit at or above the floor to pace sanely"
                },
            });
            p.interval_ceiling_secs = ceil;
        }

        if !reverts.is_empty() {
            p.last_set_by = "familiar".to_string();
        }
        (p, reverts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct Temp(PathBuf);
    impl Temp {
        fn new(t: &str) -> Self {
            let p = std::env::temp_dir().join(format!("familiar_params_test_{t}"));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Temp(p)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn missing_file_is_defaults() {
        let t = Temp::new("missing");
        assert_eq!(Parameters::load(&t.0).unwrap(), Parameters::default());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let t = Temp::new("roundtrip");
        let p = Parameters {
            theorize_every_secs: 120,
            last_set_by: "observer".into(),
            ..Default::default()
        };
        p.save(&t.0).unwrap();
        assert_eq!(Parameters::load(&t.0).unwrap(), p);
    }

    #[test]
    fn review_accepts_choices_within_the_envelope() {
        let p = Parameters {
            theorize_every_secs: 300, // 5 min — well inside [60, 21600]
            interval_floor_secs: 120,
            interval_ceiling_secs: 1200,
            last_set_by: "observer".into(),
            ..Default::default()
        };
        let (corrected, reverts) = p.review();
        assert!(reverts.is_empty(), "in-envelope choices are Ian's to make");
        assert_eq!(corrected, p, "left exactly as the human set them");
    }

    #[test]
    fn review_reverts_choices_outside_the_envelope() {
        let p = Parameters {
            theorize_every_secs: 5,        // far too aggressive
            interval_floor_secs: 1,        // would busy-loop
            interval_ceiling_secs: 99_999, // would drowse forever
            last_set_by: "observer".into(),
            ..Default::default()
        };
        let (corrected, reverts) = p.review();
        assert_eq!(reverts.len(), 3, "all three out-of-bounds get reverted");
        assert_eq!(corrected.theorize_every_secs, 60);
        assert_eq!(corrected.interval_floor_secs, 15);
        assert_eq!(corrected.interval_ceiling_secs, 3_600);
        assert_eq!(
            corrected.last_set_by, "familiar",
            "a revert is the familiar's act, recorded as such"
        );
    }

    #[test]
    fn sane_clamps_a_wedging_edit() {
        let p = Parameters {
            theorize_every_secs: 0,    // would never theorize-gate sanely
            interval_floor_secs: 0,    // would busy-loop
            interval_ceiling_secs: 10, // below a sane floor
            last_set_by: "observer".into(),
            ..Default::default()
        }
        .sane();
        assert!(p.theorize_every_secs >= 30);
        assert!(p.interval_floor_secs >= 5);
        assert!(p.interval_ceiling_secs >= p.interval_floor_secs);
    }
}
