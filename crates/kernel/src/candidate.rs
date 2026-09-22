//! Candidate — a response to a loop, and a developed instance of a genotype.
//!
//! A persisted candidate carries both heritable traits and *somatic* state (status,
//! artifact_path). Reproduction goes through the genotype ([`crate::spec`]), never by
//! copying somatic state — the Weismann barrier. Faithful port of v1's `candidate.c`.

use crate::loops::Loop;
use crate::store;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

pub const CANDIDATES_FILE: &str = "candidates.jsonl";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub id: String,
    pub parent_id: String,
    pub loop_id: String,
    pub generation: i32,
    pub hypothesis: String,
    pub artifact_type: String,
    /// Somatic: the produced artifact (not heritable).
    pub artifact_path: String,
    /// Heritable: traits carried in from the parent.
    pub inherited_traits: String,
    /// Somatic-until-projected: the traits this generation changed.
    pub changed_traits: String,
    pub mutation_reason: String,
    /// Somatic: lifecycle state.
    pub status: String,
}

impl Candidate {
    /// A generation-0 candidate proposed for a loop. Root (no parent), clean somatic.
    pub fn from_loop(lp: &Loop, id: impl Into<String>) -> Self {
        Candidate {
            id: id.into(),
            parent_id: String::new(),
            loop_id: lp.id.clone(),
            generation: 0,
            hypothesis: format!("Address loop {}: {}", lp.name, lp.description),
            artifact_type: "script".to_string(),
            artifact_path: String::new(),
            inherited_traits: String::new(),
            changed_traits: String::new(),
            mutation_reason: String::new(),
            status: "generated".to_string(),
        }
    }
}

/// Append a candidate.
pub fn append(dir: &Path, c: &Candidate) -> io::Result<()> {
    store::append(dir, CANDIDATES_FILE, c)
}

/// Load all candidates.
pub fn load(dir: &Path) -> io::Result<Vec<Candidate>> {
    store::load(dir, CANDIDATES_FILE)
}

/// Find a candidate by id.
pub fn find<'a>(cands: &'a [Candidate], id: &str) -> Option<&'a Candidate> {
    cands.iter().find(|c| c.id == id)
}

/// Set a candidate's status — a single indexed update, not a whole-file rewrite. Status is
/// somatic, not heritable, so updating it in place does not touch the genotype. Returns true
/// if the id was found.
pub fn update_status(dir: &Path, id: &str, status: &str) -> io::Result<bool> {
    let Some(mut c) = store::load_by_id::<Candidate>(dir, CANDIDATES_FILE, id)? else {
        return Ok(false);
    };
    c.status = status.to_string();
    store::update_by_id(dir, CANDIDATES_FILE, id, &c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loops;
    use crate::observation::Observation;

    fn a_loop() -> Loop {
        let o = |id: &str| {
            let mut x = Observation::new("client", "asks_for", "report", "", "t", 1, 1.0);
            x.id = id.into();
            x
        };
        loops::detect(&[o("o1"), o("o2")]).remove(0)
    }

    #[test]
    fn from_loop_is_clean_gen0_root() {
        let c = Candidate::from_loop(&a_loop(), "candidate-0001");
        assert_eq!(c.generation, 0);
        assert!(c.parent_id.is_empty());
        assert_eq!(c.status, "generated");
        assert!(c.artifact_path.is_empty() && c.changed_traits.is_empty());
        assert!(c.hypothesis.contains("client_asks_for"));
    }
}
