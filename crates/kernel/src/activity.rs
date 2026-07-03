//! The activity log — a per-tick record of what the metabolism did, so the human can
//! *see* the familiar working. The Glass renders it as a feed and a signals-over-time
//! chart; before this existed, the familiar's actions (theorize, pursue, test, promote)
//! were printed only to `daemon.log` and were invisible in the Glass.
//!
//! Append-only JSONL, derived/rebuildable — not truth. It lives in the kernel (not the
//! cycle) so the Glass, which depends on the kernel and not the cycle, can read it.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

use crate::store;

pub const TICKS_FILE: &str = "ticks.jsonl";

/// One tick's worth of activity — a flat, serde-friendly mirror of the cycle's
/// `TickReport`. The cycle builds and appends one of these per tick.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivityTick {
    pub ts: i64,
    pub sensed: usize,
    pub loops: usize,
    pub new_candidates: usize,
    pub tested: usize,
    pub promoted: usize,
    pub mutated: usize,
    pub archived: usize,
    pub theorized: bool,
    pub pursued: usize,
    /// Human-set parameters the familiar reverted this tick (co-ownership, Brick 19).
    #[serde(default)]
    pub reverted: usize,
    /// Directives the familiar refused to pursue this tick because their author is a
    /// flagged corruptor (Brick 20).
    #[serde(default)]
    pub marginalized: usize,
    /// Human requests the familiar answered this tick (Brick 21).
    #[serde(default)]
    pub answered: usize,
    /// Human requests the familiar refused as constitution-breaking this tick (Brick 21).
    #[serde(default)]
    pub refused: usize,
    /// Authored artifacts the familiar declined to run after a pre-execution review found
    /// them plainly harmful (Brick 22).
    #[serde(default)]
    pub declined: usize,
    /// Distinct mesh peers whose verified briefs were merged this tick (federation).
    #[serde(default)]
    pub mesh_peers: usize,
    /// Tools auto-merged from peers into the library this tick (still gated on *use*).
    #[serde(default)]
    pub mesh_tools_merged: usize,
    /// Patterns merged from peers this tick.
    #[serde(default)]
    pub mesh_patterns_merged: usize,
    /// Inbound briefs rejected this tick (failed cert/signature re-verification).
    #[serde(default)]
    pub mesh_rejected: usize,
    pub service: f64,
    pub presence: f64,
    pub capacities: f64,
    pub structural_changed: bool,
}

impl ActivityTick {
    /// True when nothing of consequence happened this tick (mirrors `TickReport::quiet`)
    /// — the Glass can de-emphasize quiet ticks so the feed reads as a log of *actions*.
    pub fn quiet(&self) -> bool {
        !self.structural_changed
            && self.sensed == 0
            && self.new_candidates == 0
            && self.tested == 0
            && self.promoted == 0
            && self.mutated == 0
            && self.pursued == 0
            && !self.theorized
    }
}

/// Append one tick to the activity log.
pub fn append(dir: &Path, t: &ActivityTick) -> io::Result<()> {
    store::append(dir, TICKS_FILE, t)
}

/// Load the whole activity log (oldest-first).
pub fn load(dir: &Path) -> io::Result<Vec<ActivityTick>> {
    store::load(dir, TICKS_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct Temp(PathBuf);
    impl Temp {
        fn new(t: &str) -> Self {
            let p = std::env::temp_dir().join(format!("familiar_activity_test_{t}"));
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

    fn sample(ts: i64) -> ActivityTick {
        ActivityTick {
            ts,
            sensed: 0,
            loops: 1,
            new_candidates: 0,
            tested: 0,
            promoted: 0,
            mutated: 0,
            archived: 0,
            theorized: false,
            pursued: 0,
            reverted: 0,
            marginalized: 0,
            answered: 0,
            refused: 0,
            declined: 0,
            mesh_peers: 0,
            mesh_tools_merged: 0,
            mesh_patterns_merged: 0,
            mesh_rejected: 0,
            service: 0.4,
            presence: 0.8,
            capacities: 0.75,
            structural_changed: false,
        }
    }

    #[test]
    fn append_then_load_roundtrips() {
        let t = Temp::new("roundtrip");
        append(&t.0, &sample(100)).unwrap();
        append(&t.0, &sample(200)).unwrap();
        let got = load(&t.0).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[1].ts, 200);
    }

    #[test]
    fn quiet_detects_an_idle_tick_and_a_busy_one() {
        let idle = sample(1);
        assert!(idle.quiet());
        let mut busy = sample(2);
        busy.theorized = true;
        assert!(!busy.quiet());
    }
}
