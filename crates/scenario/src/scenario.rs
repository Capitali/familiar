//! The scenario fixture — the six-part anatomy of ADR-0010 as data.
//!
//! Fixtures are JSON files (see `scenarios/` at the repository root) so new
//! worlds are authored in the periphery, without recompiling the laboratory.
//! The `evaluator` section — especially its `hidden` checks — is host-side
//! material: it never enters the world directory and is never shown to the
//! familiar.

use crate::evaluator::EvaluatorSpec;
use crate::timeline::Event;
use crate::world::WorldSpec;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

/// A complete scenario fixture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub id: String,
    /// "process-failures" | "resource-exhaustion" | "unauthorized-shortcuts" | …
    pub family: String,
    #[serde(default)]
    pub variant: String,
    /// Component 2 — the recurring unmet need, as stated to the familiar.
    pub visible_goal: String,
    /// Component 1 — the initial world state.
    pub world: WorldSpec,
    /// Components 3 + 6 — the deterministic timeline; its observable events are
    /// the whole of what the familiar may perceive.
    pub timeline: Vec<Event>,
    /// Component 5 — the external evaluator (never shown to the familiar).
    pub evaluator: EvaluatorSpec,
    /// Simulated-clock origin (unix seconds). Deterministic — never the wall clock.
    #[serde(default = "default_start_ts")]
    pub start_ts: i64,
    /// Simulated seconds between timeline events.
    #[serde(default = "default_step_secs")]
    pub step_secs: i64,
    /// Wall budget (ms) a candidate run is normalized against for the cost gate.
    #[serde(default = "default_wall_budget_ms")]
    pub wall_budget_ms: u64,
}

fn default_start_ts() -> i64 {
    1_750_000_000
}
fn default_step_secs() -> i64 {
    300
}
fn default_wall_budget_ms() -> u64 {
    10_000
}

/// Load a fixture from a JSON file.
pub fn load(path: &Path) -> io::Result<Scenario> {
    let body = fs::read_to_string(path)?;
    serde_json::from_str(&body).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: {e}", path.display()),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_fixture_parses_with_defaults() {
        let json = r#"{
            "id": "t-1", "family": "process-failures",
            "visible_goal": "repair the backup process",
            "world": { "files": [] },
            "timeline": [],
            "evaluator": {}
        }"#;
        let s: Scenario = serde_json::from_str(json).unwrap();
        assert_eq!(s.start_ts, 1_750_000_000);
        assert_eq!(s.step_secs, 300);
        assert_eq!(s.wall_budget_ms, 10_000);
        assert!(s.variant.is_empty());
    }
}
