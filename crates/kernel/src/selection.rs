//! Selection — promote / mutate / archive / observe-more. Faithful port of v1's
//! `selection.c`. The decision is a pure function of the trial and the current rigor
//! (which sets the adaptive promotion bar).

use crate::score;
use crate::trial::Trial;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Promote,
    Mutate,
    Archive,
    Reject,
    Hold,
    ObserveMore,
}

/// Decide a candidate's fate from its trial and the current rigor.
///
/// - clear success at or above the (rigor-adaptive) promotion bar → **promote**;
/// - partial at or above the mutation floor → **mutate**;
/// - failed with no classified cause → **observe more** (gather evidence first);
/// - failed but still above the mutation floor → **mutate**;
/// - otherwise → **archive** (preserved as negative evidence).
pub fn decide(trial: &Trial, rigor: f64) -> Decision {
    let overall = score::overall(trial);
    let failed = trial.result == "fail";
    let partial = trial.result == "partial";

    if !failed && !partial && overall >= score::promote_threshold(rigor) {
        return Decision::Promote;
    }
    if partial && overall >= score::mutate_threshold() {
        return Decision::Mutate;
    }
    if failed && trial.failure_class.is_empty() {
        return Decision::ObserveMore;
    }
    if failed && overall >= score::mutate_threshold() {
        return Decision::Mutate;
    }
    Decision::Archive
}

impl Decision {
    pub fn as_str(self) -> &'static str {
        match self {
            Decision::Promote => "promote",
            Decision::Mutate => "mutate",
            Decision::Archive => "archive",
            Decision::Reject => "reject",
            Decision::Hold => "hold",
            Decision::ObserveMore => "observe_more",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trial(result: &str, overall: f64, failure_class: &str) -> Trial {
        let mut t = Trial::new("t1", "c1");
        t.result = result.into();
        t.overall = overall;
        t.failure_class = failure_class.into();
        t
    }

    #[test]
    fn promotes_only_above_the_adaptive_bar() {
        // pass at 0.80: promoted when rigor low (bar 0.70), archived when rigor high (bar 0.95)
        assert_eq!(decide(&trial("pass", 0.80, ""), 0.0), Decision::Promote);
        assert_eq!(decide(&trial("pass", 0.80, ""), 1.0), Decision::Archive);
    }

    #[test]
    fn partial_above_floor_mutates() {
        assert_eq!(
            decide(&trial("partial", 0.40, "too_vague"), 0.0),
            Decision::Mutate
        );
        assert_eq!(
            decide(&trial("partial", 0.20, "too_vague"), 0.0),
            Decision::Archive
        );
    }

    #[test]
    fn failed_unclassified_observes_more() {
        assert_eq!(decide(&trial("fail", 0.10, ""), 0.0), Decision::ObserveMore);
    }

    #[test]
    fn failed_classified_above_floor_mutates_else_archives() {
        assert_eq!(
            decide(&trial("fail", 0.50, "too_complex"), 0.0),
            Decision::Mutate
        );
        assert_eq!(
            decide(&trial("fail", 0.10, "too_complex"), 0.0),
            Decision::Archive
        );
    }
}
