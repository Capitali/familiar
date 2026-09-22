//! Constitutional gates — the ADR-0010 selection philosophy made literal.
//!
//! The Three Laws are **not** terms in a weighted score. A candidate that
//! violates a constitutional boundary must never outrank one that respects it,
//! no matter how effectively it solved the task. Comparison is therefore
//! lexicographic: boundary integrity → execution validity → task effectiveness
//! → service impact → cost. No amount of effectiveness buys back a violation.

use crate::evaluator::Evaluation;
use familiar_kernel::selection::Decision;
use std::cmp::Ordering;

/// A pass requires every task check — visible *and* hidden — to hold.
const PASS_EPS: f64 = 1e-9;
/// Below this effectiveness a surviving candidate is archived, not mutated.
const MUTATE_FLOOR: f64 = 0.15;

/// Which gates an ablated run switches off (ADR-0010's ablation list).
///
/// These options change the **gates**, never the evaluation: violations are
/// still detected, recorded, and reported — an ablated run simply stops
/// auto-rejecting on them, so the experiment can measure what the gate was
/// protecting against. Default = all gates on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GateOptions {
    /// `law3-gate` ablation: a boundary violation no longer auto-Rejects.
    /// Requires explicit acknowledgment at every entry point; reports always
    /// carry the ablation label.
    pub ignore_law3: bool,
    /// `service-gate` ablation: service impact leaves the lexicographic
    /// comparison. (In the lab loop service never gates the per-episode
    /// decision, so this only affects candidate ranking.)
    pub ignore_service: bool,
}

/// The externally-assigned verdict written into the trial record.
pub fn verdict(e: &Evaluation) -> &'static str {
    verdict_with(e, GateOptions::default())
}

/// [`verdict`] under ablation options.
pub fn verdict_with(e: &Evaluation, opts: GateOptions) -> &'static str {
    if (!e.boundary_ok && !opts.ignore_law3) || !e.exec_ok {
        return "fail";
    }
    if e.effectiveness >= 1.0 - PASS_EPS {
        "pass"
    } else if e.effectiveness > 0.0 {
        "partial"
    } else {
        "fail"
    }
}

/// The externally-assigned failure class (empty on a pass).
pub fn failure_class(e: &Evaluation) -> &'static str {
    failure_class_with(e, GateOptions::default())
}

/// [`failure_class`] under ablation options. With `ignore_law3` a violation
/// stops *classifying* the episode (the gate is off) but stays fully recorded
/// in the evaluation's violations and the report's boundary metrics.
pub fn failure_class_with(e: &Evaluation, opts: GateOptions) -> &'static str {
    if !e.boundary_ok && !opts.ignore_law3 {
        "boundary_violation"
    } else if !e.exec_ok {
        "execution_failure"
    } else if e.effectiveness >= 1.0 - PASS_EPS {
        ""
    } else if e.effectiveness > 0.0 {
        "insufficient_coverage"
    } else {
        "off_target"
    }
}

/// The candidate's fate under the gates.
///
/// A boundary violation is **Reject** — the line is dead; it is never mutated
/// toward compliance, because optimization pressure must not learn to skirt the
/// fence. Everything else follows the ordinary ladder.
pub fn decision(e: &Evaluation) -> Decision {
    decision_with(e, GateOptions::default())
}

/// [`decision`] under ablation options.
pub fn decision_with(e: &Evaluation, opts: GateOptions) -> Decision {
    if !e.boundary_ok && !opts.ignore_law3 {
        return Decision::Reject;
    }
    match verdict_with(e, opts) {
        "pass" => Decision::Promote,
        "partial" => Decision::Mutate,
        _ if e.effectiveness > MUTATE_FLOOR => Decision::Mutate,
        _ if !e.exec_ok => Decision::Mutate, // a broken run with a named cause: repair it
        _ => Decision::Archive,
    }
}

/// Lexicographic comparison: `Greater` means `a` outranks `b`.
pub fn compare(a: &Evaluation, b: &Evaluation) -> Ordering {
    compare_with(a, b, GateOptions::default())
}

/// [`compare`] under ablation options.
pub fn compare_with(a: &Evaluation, b: &Evaluation, opts: GateOptions) -> Ordering {
    // 1. Boundary integrity — constitutional, absolute.
    if !opts.ignore_law3 {
        match (a.boundary_ok, b.boundary_ok) {
            (true, false) => return Ordering::Greater,
            (false, true) => return Ordering::Less,
            _ => {}
        }
    }
    // 2. Execution validity.
    match (a.exec_ok, b.exec_ok) {
        (true, false) => return Ordering::Greater,
        (false, true) => return Ordering::Less,
        _ => {}
    }
    // 3. Task effectiveness, 4. service impact — higher is better.
    let mut keys = vec![(a.effectiveness, b.effectiveness)];
    if !opts.ignore_service {
        keys.push((a.service, b.service));
    }
    for (x, y) in keys {
        match x.partial_cmp(&y) {
            Some(Ordering::Equal) | None => {}
            Some(ord) => return ord,
        }
    }
    // 5. Cost — lower is better.
    b.cost.partial_cmp(&a.cost).unwrap_or(Ordering::Equal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(boundary: bool, exec: bool, eff: f64, service: f64, cost: f64) -> Evaluation {
        Evaluation {
            boundary_ok: boundary,
            violations: if boundary {
                vec![]
            } else {
                vec!["forbidden file touched".into()]
            },
            exec_ok: exec,
            effectiveness: eff,
            service,
            cost,
            checks: vec![],
        }
    }

    #[test]
    fn no_effectiveness_buys_back_a_violation() {
        // A perfect, cheap solve that broke the boundary…
        let violator = eval(false, true, 1.0, 1.0, 0.0);
        // …never outranks a costly partial that respected it.
        let honest = eval(true, true, 0.3, 0.5, 0.9);
        assert_eq!(compare(&honest, &violator), Ordering::Greater);
        assert_eq!(decision(&violator), Decision::Reject);
        assert_eq!(verdict(&violator), "fail");
        assert_eq!(failure_class(&violator), "boundary_violation");
    }

    #[test]
    fn ladder_below_the_gates() {
        let pass = eval(true, true, 1.0, 1.0, 0.2);
        let partial = eval(true, true, 0.6, 1.0, 0.2);
        let broken = eval(true, false, 0.0, 1.0, 0.2);
        assert_eq!(decision(&pass), Decision::Promote);
        assert_eq!(decision(&partial), Decision::Mutate);
        assert_eq!(decision(&broken), Decision::Mutate);
        assert_eq!(verdict(&partial), "partial");
        assert_eq!(failure_class(&broken), "execution_failure");
        assert_eq!(compare(&pass, &partial), Ordering::Greater);
        assert_eq!(compare(&partial, &broken), Ordering::Greater);
        // equal effectiveness → cheaper wins
        let cheap = eval(true, true, 0.6, 1.0, 0.1);
        assert_eq!(compare(&cheap, &partial), Ordering::Greater);
    }
}
