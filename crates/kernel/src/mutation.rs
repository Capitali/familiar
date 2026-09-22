//! Mutation — reproduce a candidate with a change, informed by memory. Faithful port
//! of v1's `mutation.c`.
//!
//! Reproduction runs through the genotype ([`crate::spec`]) so successful traits carry
//! forward and the child starts with clean somatic state. The suppression invariant: a
//! suggested trait is dropped only when memory's negative evidence **clearly outweighs**
//! the positive (`sup > amp`); the result is never empty (an empty `changed_traits` is
//! exactly what the regression guard rejects).

use crate::candidate::Candidate;
use crate::pattern_memory::{self, PatternMemory};
use crate::spec;

/// Create a child from a parent: project the parent's genotype, develop a fresh
/// instance (gen+1), then stamp this generation's changed traits and reason.
pub fn create(
    parent: &Candidate,
    reason: impl Into<String>,
    changed_traits: impl Into<String>,
    new_id: impl Into<String>,
) -> Candidate {
    let genotype = spec::from_candidate(parent);
    let mut child = spec::develop(
        &genotype,
        new_id,
        parent.loop_id.clone(),
        parent.id.clone(),
        parent.generation + 1,
    );
    child.changed_traits = changed_traits.into();
    child.mutation_reason = reason.into();
    child
}

/// Base heuristic: map a failure class to a comma-separated trait change. Unknown or
/// uncertain causes → a small adjustment.
pub fn suggest(failure_class: &str) -> String {
    let fc = failure_class;
    if fc.is_empty() {
        return "small_adjustment".to_string();
    }
    let has = |needle: &str| fc.contains(needle);
    if has("too_complex") || has("complexity") {
        "reduce_scope,shorten,plain_language,remove_governance_overhead"
    } else if has("too_simple") || has("insufficient") {
        "add_detail,expand_scope,add_examples"
    } else if has("unclear") || has("clarity") {
        "rewrite_for_clarity,add_examples,simplify_language"
    } else if has("costly") || has("resource_heavy") {
        "reduce_allocation,fewer_deps,tighter_loop,shrink_output"
    } else if has("boundary") || has("unsafe") || has("risk") {
        "reduce_data,add_consent,require_review,narrow_scope"
    } else if has("off_target") || has("low_fit") {
        "retarget_to_loop_terms,address_required_traits,regenerate_response"
    } else if has("vague") {
        "add_detail,define_audience,add_examples"
    } else {
        "small_adjustment"
    }
    .to_string()
}

/// Memory-informed suggestion: start from the base heuristic, then drop any trait the
/// memory clearly punishes (`sup > amp` and `sup` non-negligible). Never returns empty
/// — falls back to the base suggestion if memory would suppress everything.
pub fn suggest_informed(failure_class: &str, pm: &[PatternMemory]) -> String {
    let base = suggest(failure_class);
    if pm.is_empty() {
        return base;
    }
    let kept: Vec<&str> = base
        .split(',')
        .filter(|tok| {
            let (amp, sup) = pattern_memory::scan_affinity(pm, tok);
            // keep unless negative evidence clearly outweighs positive
            sup <= amp || sup < 0.0001
        })
        .collect();
    if kept.is_empty() {
        base
    } else {
        kept.join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parent() -> Candidate {
        Candidate {
            id: "candidate-0001".into(),
            parent_id: String::new(),
            loop_id: "loop-x".into(),
            generation: 0,
            hypothesis: "do X".into(),
            artifact_type: "script".into(),
            artifact_path: "/produced".into(), // somatic — must not leak to child
            inherited_traits: "a".into(),
            changed_traits: "b".into(),
            mutation_reason: String::new(),
            status: "failed".into(),
        }
    }

    fn pm(pos: &str, neg: &str, conf: f64) -> PatternMemory {
        PatternMemory {
            id: "p".into(),
            name: "p".into(),
            lesson: String::new(),
            applies_when: String::new(),
            positive_evidence: pos.into(),
            negative_evidence: neg.into(),
            confidence: conf,
        }
    }

    #[test]
    fn create_inherits_traits_and_clean_somatic() {
        let child = create(&parent(), "too_complex", "reduce_scope", "candidate-0002");
        assert_eq!(child.parent_id, "candidate-0001");
        assert_eq!(child.generation, 1);
        assert_eq!(child.inherited_traits, "a,b"); // parent's full heritable set
        assert_eq!(child.changed_traits, "reduce_scope");
        assert_eq!(child.mutation_reason, "too_complex");
        assert!(child.artifact_path.is_empty()); // somatic did not leak
        assert_eq!(child.status, "generated");
    }

    #[test]
    fn suggest_maps_known_classes() {
        assert!(suggest("too_complex").contains("reduce_scope"));
        assert_eq!(suggest(""), "small_adjustment");
        assert_eq!(suggest("mystery_failure"), "small_adjustment");
    }

    #[test]
    fn informed_drops_only_clearly_punished_traits() {
        // memory punishes reduce_scope, rewards shorten
        let mem = vec![pm("shorten is good", "reduce_scope is bad", 0.5)];
        let out = suggest_informed("too_complex", &mem);
        assert!(!out.contains("reduce_scope"));
        assert!(out.contains("shorten"));
    }

    #[test]
    fn informed_never_returns_empty() {
        // memory punishes everything in the base suggestion -> fall back to base
        let base = suggest("too_complex");
        let mem: Vec<_> = base
            .split(',')
            .map(|t| pm("", &format!("{t} is bad"), 0.5))
            .collect();
        let out = suggest_informed("too_complex", &mem);
        assert!(!out.is_empty());
        assert_eq!(out, base);
    }

    #[test]
    fn no_memory_equals_base() {
        assert_eq!(suggest_informed("too_complex", &[]), suggest("too_complex"));
    }
}
