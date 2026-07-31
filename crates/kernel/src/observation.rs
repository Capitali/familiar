//! The observation record — `actor · action · object`, the only truth.
//!
//! Everything else the factory holds is derived from observations and can be
//! rebuilt from them. A faithful port of v1's `observation_t`
//! (`factory/include/observation.h`), now a `serde` struct over `store`.

use crate::store;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

/// The append-only observation log.
pub const OBSERVATIONS_FILE: &str = "observations.jsonl";

/// A single observed event: a subject–predicate–object triple plus provenance.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Observation {
    /// Sequential id (`obs-NNNN`), assigned on [`record`].
    pub id: String,
    /// Where it came from (a signal ref, a sensor, or `cli` for manual input).
    pub source: String,
    pub actor: String,
    pub action: String,
    pub object: String,
    /// Free-form detail; optional.
    #[serde(default)]
    pub context: String,
    /// Unix seconds.
    pub ts: i64,
    /// Confidence in [0, 1].
    pub confidence: f64,
}

impl Observation {
    /// Build an observation with an empty id; [`record`] assigns the id on append.
    pub fn new(
        actor: impl Into<String>,
        action: impl Into<String>,
        object: impl Into<String>,
        context: impl Into<String>,
        source: impl Into<String>,
        ts: i64,
        confidence: f64,
    ) -> Self {
        Observation {
            id: String::new(),
            source: source.into(),
            actor: actor.into(),
            action: action.into(),
            object: object.into(),
            context: context.into(),
            ts,
            confidence,
        }
    }
}

/// Append an observation, assigning the next sequential id (`obs-NNNN`) when the
/// id is empty. Returns the stored record (with its assigned id).
pub fn record(dir: &Path, mut obs: Observation) -> io::Result<Observation> {
    if obs.id.is_empty() {
        // The table's own high-water mark, not a row count. A count is O(n) — a full table
        // load on every single append — and, worse, it repeats an id the moment any row is
        // ever removed, which silently breaks every `load_by_id` lookup in the system. For an
        // un-pruned table `next_seq == count + 1`, so existing data keeps identical ids.
        let n = store::next_seq(dir, OBSERVATIONS_FILE)?;
        obs.id = format!("obs-{n:04}");
    }
    store::append(dir, OBSERVATIONS_FILE, &obs)?;
    Ok(obs)
}

/// Load all observations, oldest first.
pub fn load(dir: &Path) -> io::Result<Vec<Observation>> {
    store::load(dir, OBSERVATIONS_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!("substrate_obs_test_{tag}"));
            let _ = fs::remove_dir_all(&p);
            TempDir(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn record_assigns_sequential_ids_and_roundtrips() {
        let d = TempDir::new("seq");
        let a = record(
            d.path(),
            Observation::new("betty", "asks_for", "weekly_digest", "", "cli", 100, 0.9),
        )
        .unwrap();
        let b = record(
            d.path(),
            Observation::new("host", "reports", "cpu_load", "", "sensor", 200, 0.95),
        )
        .unwrap();
        assert_eq!(a.id, "obs-0001");
        assert_eq!(b.id, "obs-0002");

        let all = load(d.path()).unwrap();
        assert_eq!(all.len(), 2);
        // field fidelity through the JSONL round-trip
        assert_eq!(all[0], a);
        assert_eq!(all[1].actor, "host");
        assert_eq!(all[1].confidence, 0.95);
    }

    #[test]
    fn explicit_id_is_preserved() {
        let d = TempDir::new("explicit");
        let mut o = Observation::new("a", "b", "c", "ctx", "cli", 1, 1.0);
        o.id = "obs-fixed".into();
        let stored = record(d.path(), o).unwrap();
        assert_eq!(stored.id, "obs-fixed");
    }
}
