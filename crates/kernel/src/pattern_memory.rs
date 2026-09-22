//! Pattern memory — lessons from trial history (failures are fossils).
//!
//! Passing hypotheses become positive evidence; failures become negative evidence.
//! `scan_affinity` lets variation consult that memory: a trait the past rewarded is
//! amplified, one the past punished is suppressed. Faithful port of v1's
//! `pattern_memory.c`.

use crate::candidate::Candidate;
use crate::store;
use crate::trial::Trial;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

pub const PATTERNS_FILE: &str = "patterns.jsonl";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatternMemory {
    pub id: String,
    pub name: String,
    pub lesson: String,
    pub applies_when: String,
    pub positive_evidence: String,
    pub negative_evidence: String,
    pub confidence: f64,
}

/// Distill a pattern from a candidate's trial outcome (the `id` is assigned by the
/// caller from the file sequence).
pub fn from_outcome(id: impl Into<String>, cand: &Candidate, trial: &Trial) -> PatternMemory {
    let name = if !trial.failure_class.is_empty() {
        trial.failure_class.clone()
    } else if trial.result == "pass" {
        "success_pattern".to_string()
    } else {
        "observed_pattern".to_string()
    };
    let mut pm = PatternMemory {
        id: id.into(),
        name,
        lesson: format!(
            "Candidate {} result={} in scenario {} (overall={:.2})",
            cand.id, trial.result, trial.scenario_id, trial.overall
        ),
        applies_when: format!(
            "loop_id={} artifact_type={}",
            cand.loop_id, cand.artifact_type
        ),
        positive_evidence: String::new(),
        negative_evidence: String::new(),
        confidence: trial.confidence,
    };
    if trial.result == "pass" {
        pm.positive_evidence = cand.hypothesis.clone();
    } else {
        pm.negative_evidence = format!(
            "failure_class={} hypothesis={}",
            trial.failure_class, cand.hypothesis
        );
    }
    pm
}

/// Amplification and suppression for a set of comma/space-separated terms, each in
/// [0, 0.5]. For every term, sum `confidence/token_count` over patterns whose
/// positive (→amp) or negative (→sup) evidence contains it.
pub fn scan_affinity(pm: &[PatternMemory], terms: &str) -> (f64, f64) {
    let tokens: Vec<&str> = terms.split([' ', ',']).filter(|t| !t.is_empty()).collect();
    if tokens.is_empty() {
        return (0.0, 0.0);
    }
    let weight = 1.0 / tokens.len() as f64;
    let mut amp = 0.0;
    let mut sup = 0.0;
    for tok in tokens {
        for p in pm {
            if p.positive_evidence.contains(tok) {
                amp += p.confidence * weight;
            }
            if p.negative_evidence.contains(tok) {
                sup += p.confidence * weight;
            }
        }
    }
    (amp.min(0.5), sup.min(0.5))
}

pub fn append(dir: &Path, pm: &PatternMemory) -> io::Result<()> {
    store::append(dir, PATTERNS_FILE, pm)
}

pub fn load(dir: &Path) -> io::Result<Vec<PatternMemory>> {
    store::load(dir, PATTERNS_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pm(name: &str, pos: &str, neg: &str, conf: f64) -> PatternMemory {
        PatternMemory {
            id: name.into(),
            name: name.into(),
            lesson: String::new(),
            applies_when: String::new(),
            positive_evidence: pos.into(),
            negative_evidence: neg.into(),
            confidence: conf,
        }
    }

    #[test]
    fn affinity_amplifies_positive_suppresses_negative() {
        let mem = vec![
            pm("p", "reduce_scope works well", "", 0.4),
            pm("n", "", "expand_scope failed badly", 0.6),
        ];
        let (amp, sup) = scan_affinity(&mem, "reduce_scope");
        assert!(amp > 0.0 && sup == 0.0);
        let (amp2, sup2) = scan_affinity(&mem, "expand_scope");
        assert!(amp2 == 0.0 && sup2 > 0.0);
    }

    #[test]
    fn affinity_caps_at_half_and_empty_is_zero() {
        let mem: Vec<_> = (0..20)
            .map(|i| pm(&format!("p{i}"), "trait", "", 1.0))
            .collect();
        let (amp, _) = scan_affinity(&mem, "trait");
        assert!(amp <= 0.5);
        assert_eq!(scan_affinity(&mem, ""), (0.0, 0.0));
    }
}
