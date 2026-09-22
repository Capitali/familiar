//! Spec — the heritable genotype, separable from the developed instance.
//!
//! The **Weismann barrier**: somatic state (status, artifact_path) never writes back
//! to the genotype. A lifetime change reaches heredity only by being projected into a
//! fresh genotype ([`from_candidate`]) and developed into a new instance
//! ([`develop`]). Faithful port of v1's `spec.{h,c}`.
//!
//! Digital caveat: copies are perfect, so the default is stasis, not drift —
//! `region_fidelity` starts at 1.0 and variation must be deliberately injected.

use crate::candidate::Candidate;

/// Index into `region_fidelity`.
pub const REGION_INTERFACE: usize = 0;
pub const REGION_PRODUCTION: usize = 1;
pub const REGION_COUNT: usize = 2;

/// The heritable core.
#[derive(Debug, Clone, PartialEq)]
pub struct Spec {
    /// Interface region (reserved; thin to start).
    pub interface_contract: String,
    /// Production region — the heritable definition.
    pub hypothesis: String,
    pub artifact_type: String,
    /// CSV: the heritable trait set (inherited ∪ changed, from the source instance).
    pub traits: String,
    /// Per-region copy fidelity in [0,1]; 1.0 = faithful (low mutation).
    pub region_fidelity: [f64; REGION_COUNT],
}

impl Default for Spec {
    fn default() -> Self {
        Spec {
            interface_contract: String::new(),
            hypothesis: String::new(),
            artifact_type: String::new(),
            traits: String::new(),
            region_fidelity: [1.0; REGION_COUNT],
        }
    }
}

/// Union two CSV trait sets, de-duplicated, order-stable (left then new-from-right).
fn csv_union(a: &str, b: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for tok in a.split(',').chain(b.split(',')) {
        let t = tok.trim();
        if !t.is_empty() && !out.contains(&t) {
            out.push(t);
        }
    }
    out.join(",")
}

/// Project the heritable genotype out of a developed instance. Somatic state
/// (status, artifact_path) is deliberately excluded. The instance's full heritable
/// trait set — inherited plus the traits it changed — flows into production.
pub fn from_candidate(c: &Candidate) -> Spec {
    Spec {
        interface_contract: String::new(),
        hypothesis: c.hypothesis.clone(),
        artifact_type: c.artifact_type.clone(),
        traits: csv_union(&c.inherited_traits, &c.changed_traits),
        region_fidelity: [1.0; REGION_COUNT],
    }
}

/// Develop a fresh instance from a spec: the genotype's heritable fields arrive as
/// `inherited_traits`; somatic state starts clean (no artifact, status "generated").
/// Pure: assigns no id beyond what is passed, touches no file.
pub fn develop(
    s: &Spec,
    new_id: impl Into<String>,
    loop_id: impl Into<String>,
    parent_id: impl Into<String>,
    generation: i32,
) -> Candidate {
    Candidate {
        id: new_id.into(),
        parent_id: parent_id.into(),
        loop_id: loop_id.into(),
        generation,
        hypothesis: s.hypothesis.clone(),
        artifact_type: s.artifact_type.clone(),
        artifact_path: String::new(), // clean somatic state
        inherited_traits: s.traits.clone(),
        changed_traits: String::new(),
        mutation_reason: String::new(),
        status: "generated".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance() -> Candidate {
        Candidate {
            id: "candidate-0007".into(),
            parent_id: "candidate-0001".into(),
            loop_id: "loop-abc".into(),
            generation: 2,
            hypothesis: "do the thing".into(),
            artifact_type: "script".into(),
            artifact_path: "/produced/artifact.sh".into(), // somatic
            inherited_traits: "a,b".into(),
            changed_traits: "c,b".into(), // overlaps b
            mutation_reason: "because".into(),
            status: "passed".into(), // somatic
        }
    }

    #[test]
    fn genotype_excludes_somatic_and_unions_traits() {
        let s = from_candidate(&instance());
        assert_eq!(s.traits, "a,b,c"); // inherited ∪ changed, de-duped
        assert_eq!(s.hypothesis, "do the thing");
        // somatic fields are not even representable in a Spec — barrier upheld
    }

    #[test]
    fn develop_starts_clean_and_inherits() {
        let s = from_candidate(&instance());
        let child = develop(&s, "candidate-0008", "loop-abc", "candidate-0007", 3);
        assert_eq!(child.inherited_traits, "a,b,c");
        assert!(child.changed_traits.is_empty());
        assert!(child.artifact_path.is_empty()); // clean somatic
        assert_eq!(child.status, "generated");
        assert_eq!(child.generation, 3);
        assert_eq!(child.parent_id, "candidate-0007");
    }

    #[test]
    fn weismann_round_trip_does_not_leak_somatic() {
        // develop -> (pretend it lived: set somatic) -> from_candidate must drop it
        let s0 = from_candidate(&instance());
        let mut lived = develop(&s0, "c9", "loop-abc", "c8", 4);
        lived.status = "promoted".into();
        lived.artifact_path = "/x".into();
        let s1 = from_candidate(&lived);
        assert_eq!(s1.traits, s0.traits); // lifetime somatic change did not alter heredity
    }
}
