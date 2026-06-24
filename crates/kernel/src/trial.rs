//! Trial — the record of testing a candidate against a scenario.
//!
//! `overall` is the cost-weighted fitness selection reads; the per-dimension scores
//! are kept for evidence. Faithful port of v1's `trial.c`.

use crate::store;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

pub const TRIALS_FILE: &str = "trials.jsonl";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trial {
    pub id: String,
    pub candidate_id: String,
    pub scenario_id: String,
    pub fit: f64,
    pub clarity: f64,
    pub usefulness: f64,
    pub novelty: f64,
    pub safety: f64,
    /// Measured run cost in [0,1] (1.0 = maximally costly). Folded into `overall`.
    pub complexity: f64,
    pub confidence: f64,
    /// The cost-weighted fitness selection acts on.
    pub overall: f64,
    /// "pass" | "fail" | "partial".
    pub result: String,
    pub failure_class: String,
    pub notes: String,
}

impl Trial {
    /// A trial with sensible neutral defaults; set the dimensions/result that matter.
    pub fn new(id: impl Into<String>, candidate_id: impl Into<String>) -> Self {
        Trial {
            id: id.into(),
            candidate_id: candidate_id.into(),
            scenario_id: String::new(),
            fit: 0.0,
            clarity: 0.0,
            usefulness: 0.0,
            novelty: 0.0,
            safety: 1.0,
            complexity: 0.0,
            confidence: 0.5,
            overall: 0.0,
            result: "fail".to_string(),
            failure_class: String::new(),
            notes: String::new(),
        }
    }
}

pub fn append(dir: &Path, t: &Trial) -> io::Result<()> {
    store::append(dir, TRIALS_FILE, t)
}

pub fn load(dir: &Path) -> io::Result<Vec<Trial>> {
    store::load(dir, TRIALS_FILE)
}

/// The most recent trial for a candidate (last wins), if any.
pub fn find_by_candidate<'a>(trials: &'a [Trial], candidate_id: &str) -> Option<&'a Trial> {
    trials.iter().rev().find(|t| t.candidate_id == candidate_id)
}
