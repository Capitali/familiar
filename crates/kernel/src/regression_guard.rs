//! Regression guard — no failed candidate may be retried unchanged (Soul method
//! discipline / v1 Rule 4). Faithful port of v1's `regression_guard.c`.
//!
//! A child is a regression when its parent failed (or partially failed) and the child
//! changed nothing that matters: same hypothesis **and** no `changed_traits`. Such a
//! retry is not evolution — it is looping blindly.

use crate::candidate::{self, Candidate};
use crate::trial::{self, Trial};

/// Is `child` an unchanged retry of a failed/partial parent?
///
/// Pure over the resolved parent and its trial. Root candidates (no parent) are never
/// regressions; a parent that passed is never a regression base.
pub fn is_regression(child: &Candidate, parent: &Candidate, parent_trial: &Trial) -> bool {
    if child.parent_id.is_empty() {
        return false;
    }
    if parent_trial.result != "fail" && parent_trial.result != "partial" {
        return false;
    }
    child.changed_traits.trim().is_empty() && child.hypothesis == parent.hypothesis
}

/// Convenience: resolve the parent and its latest trial from slices, then check.
/// Missing parent or parent-trial → not a regression (nothing to compare against).
pub fn check(child: &Candidate, cands: &[Candidate], trials: &[Trial]) -> bool {
    if child.parent_id.is_empty() {
        return false;
    }
    let Some(parent) = candidate::find(cands, &child.parent_id) else {
        return false;
    };
    let Some(pt) = trial::find_by_candidate(trials, &child.parent_id) else {
        return false;
    };
    is_regression(child, parent, pt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(id: &str, parent: &str, hypothesis: &str, changed: &str) -> Candidate {
        Candidate {
            id: id.into(),
            parent_id: parent.into(),
            loop_id: "loop-x".into(),
            generation: 1,
            hypothesis: hypothesis.into(),
            artifact_type: "script".into(),
            artifact_path: String::new(),
            inherited_traits: String::new(),
            changed_traits: changed.into(),
            mutation_reason: String::new(),
            status: "generated".into(),
        }
    }
    fn failed(cid: &str) -> Trial {
        let mut t = Trial::new("t", cid);
        t.result = "fail".into();
        t.failure_class = "too_complex".into();
        t
    }

    #[test]
    fn unchanged_retry_of_failed_parent_is_blocked() {
        let parent = cand("c1", "", "do X", "");
        let child = cand("c2", "c1", "do X", ""); // same hypothesis, no changed traits
        assert!(is_regression(&child, &parent, &failed("c1")));
    }

    #[test]
    fn changed_traits_or_new_hypothesis_passes() {
        let parent = cand("c1", "", "do X", "");
        let with_traits = cand("c2", "c1", "do X", "reduce_scope");
        let with_hyp = cand("c3", "c1", "do X differently", "");
        assert!(!is_regression(&with_traits, &parent, &failed("c1")));
        assert!(!is_regression(&with_hyp, &parent, &failed("c1")));
    }

    #[test]
    fn root_or_passed_parent_is_never_regression() {
        let parent = cand("c1", "", "do X", "");
        let root = cand("c1", "", "do X", "");
        assert!(!is_regression(&root, &parent, &failed("c1"))); // no parent_id
        let child = cand("c2", "c1", "do X", "");
        let mut passed = Trial::new("t", "c1");
        passed.result = "pass".into();
        assert!(!is_regression(&child, &parent, &passed));
    }

    #[test]
    fn check_resolves_from_slices() {
        let parent = cand("c1", "", "do X", "");
        let child = cand("c2", "c1", "do X", "");
        let cands = vec![parent.clone(), child.clone()];
        let trials = vec![failed("c1")];
        assert!(check(&child, &cands, &trials));
        // no trial for parent -> cannot judge -> not a regression
        assert!(!check(&child, &cands, &[]));
    }
}
