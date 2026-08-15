//! Belief states derived from prediction evidence (ADR-0040 §3, dialogue Q5).
//!
//! Prediction results remain the truth-bearing, append-only record. This module
//! folds them into a reversible current view with hysteresis. No model classifies
//! an outcome or chooses a transition. Direct human correction and a hard reversal
//! enter through an explicit typed override because a person's word is not a
//! statistic.

use crate::prediction::{self, Outcome, PredictionResult};
use crate::store;
use crate::thread;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

pub const BELIEF_VERSION: u16 = 1;
pub const BELIEFS_FILE: &str = "beliefs.json";
pub const BELIEF_TRANSITIONS_FILE: &str = "belief_transitions.jsonl";
pub const NARRATION_COOLDOWN_SECS: i64 = 6 * 60 * 60;

const SUPPORT_FLOOR: usize = 3;
const SUPPORT_MARGIN: usize = 2;
const DOUBT_FLOOR: usize = 2;
const RECOVERY_FLOOR: usize = 4;
const RECOVERY_MARGIN: usize = 3;
const ABANDON_FLOOR: usize = 4;
const ABANDON_MARGIN: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeliefState {
    Tentative,
    Supported,
    Doubtful,
    Abandoned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionCause {
    Calibration,
    HumanCorrection,
    HardActReversal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceCitation {
    pub evidence_id: String,
    pub line: String,
    pub at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSummary {
    pub favorable: usize,
    pub unfavorable: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supporting: Option<EvidenceCitation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contradicting: Option<EvidenceCitation>,
}

impl EvidenceSummary {
    pub fn total(&self) -> usize {
        self.favorable.saturating_add(self.unfavorable)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeliefTransition {
    pub thread_id: String,
    pub from: BeliefState,
    pub to: BeliefState,
    pub cause: TransitionCause,
    pub at: i64,
    pub evidence: EvidenceSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Belief {
    pub thread_id: String,
    pub state: BeliefState,
    pub state_at: i64,
    pub evaluated_results: usize,
    pub evidence: EvidenceSummary,
    #[serde(default)]
    pub applied_overrides: Vec<String>,
    #[serde(default)]
    pub last_narrated_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_narration: Option<BeliefTransition>,
}

impl Belief {
    fn new(thread_id: impl Into<String>, now: i64) -> Self {
        Self {
            thread_id: thread_id.into(),
            state: BeliefState::Tentative,
            state_at: now,
            evaluated_results: 0,
            evidence: EvidenceSummary::default(),
            applied_overrides: Vec::new(),
            last_narrated_at: 0,
            pending_narration: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeliefsFile {
    pub version: u16,
    #[serde(default)]
    pub beliefs: Vec<Belief>,
}

impl Default for BeliefsFile {
    fn default() -> Self {
        Self {
            version: BELIEF_VERSION,
            beliefs: Vec::new(),
        }
    }
}

pub fn load(dir: &Path) -> io::Result<BeliefsFile> {
    let body = match fs::read(dir.join(BELIEFS_FILE)) {
        Ok(body) => body,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BeliefsFile::default()),
        Err(error) => return Err(error),
    };
    let file: BeliefsFile = serde_json::from_slice(&body)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if file.version != BELIEF_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported belief version {}; expected {BELIEF_VERSION}",
                file.version
            ),
        ));
    }
    Ok(file)
}

fn save(dir: &Path, file: &BeliefsFile) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let body = serde_json::to_vec_pretty(file)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    fs::write(dir.join(BELIEFS_FILE), body)
}

pub fn transitions(dir: &Path) -> io::Result<Vec<BeliefTransition>> {
    store::load(dir, BELIEF_TRANSITIONS_FILE)
}

/// The pure statistical state machine. Enter and exit bars are intentionally
/// distinct: a belief needs three favorable results to earn support, accumulated
/// contradiction to become doubtful, and a stronger favorable margin to recover.
/// Abandonment is terminal; only a new theory creates a new tentative belief.
pub fn calibrated_next(state: BeliefState, evidence: &EvidenceSummary) -> Option<BeliefState> {
    let favorable_leads = evidence.favorable >= evidence.unfavorable.saturating_add(SUPPORT_MARGIN);
    let unfavorable_catches = evidence.unfavorable >= evidence.favorable;
    let favorable_recovers =
        evidence.favorable >= evidence.unfavorable.saturating_add(RECOVERY_MARGIN);
    let unfavorable_leads =
        evidence.unfavorable >= evidence.favorable.saturating_add(ABANDON_MARGIN);

    match state {
        BeliefState::Tentative
            if evidence.total() >= SUPPORT_FLOOR
                && evidence.favorable >= SUPPORT_FLOOR
                && favorable_leads =>
        {
            Some(BeliefState::Supported)
        }
        BeliefState::Supported if evidence.unfavorable >= DOUBT_FLOOR && unfavorable_catches => {
            Some(BeliefState::Doubtful)
        }
        BeliefState::Doubtful if evidence.unfavorable >= ABANDON_FLOOR && unfavorable_leads => {
            Some(BeliefState::Abandoned)
        }
        BeliefState::Doubtful if evidence.favorable >= RECOVERY_FLOOR && favorable_recovers => {
            Some(BeliefState::Supported)
        }
        _ => None,
    }
}

fn summarize(results: &[PredictionResult], thread_id: &str) -> EvidenceSummary {
    let mut summary = EvidenceSummary::default();
    for result in results
        .iter()
        .filter(|result| result.thread_id == thread_id)
    {
        let citation = citation(result);
        if result.outcome.favorable() {
            summary.favorable += 1;
            summary.supporting = Some(citation);
        } else {
            summary.unfavorable += 1;
            summary.contradicting = Some(citation);
        }
    }
    summary
}

fn citation(result: &PredictionResult) -> EvidenceCitation {
    let evidence_id = format!(
        "{}:{}:{}",
        result.prediction_id, result.opened_at, result.final_at
    );
    let line = match result.outcome {
        Outcome::Confirmed => format!(
            "prediction {} was confirmed by {}",
            result.prediction_id,
            result.settled_by.as_deref().unwrap_or("recorded evidence")
        ),
        Outcome::Missed => format!(
            "prediction {} passed deadline {} without its expected event",
            result.prediction_id, result.deadline
        ),
        Outcome::AbsentConfirmed => format!(
            "prediction {} stayed quiet through deadline {}",
            result.prediction_id, result.deadline
        ),
        Outcome::AbsentViolated => format!(
            "prediction {} was contradicted by {}",
            result.prediction_id,
            result.settled_by.as_deref().unwrap_or("recorded evidence")
        ),
    };
    EvidenceCitation {
        evidence_id,
        line,
        at: result.final_at,
    }
}

fn record_transition(
    belief: &mut Belief,
    to: BeliefState,
    cause: TransitionCause,
    evidence: EvidenceSummary,
    now: i64,
) -> BeliefTransition {
    let transition = BeliefTransition {
        thread_id: belief.thread_id.clone(),
        from: belief.state,
        to,
        cause,
        at: now,
        evidence: evidence.clone(),
    };
    belief.state = to;
    belief.state_at = now;
    belief.evidence = evidence;
    belief.pending_narration = Some(transition.clone());
    transition
}

/// Fold all append-only prediction results. A statistical transition is considered
/// only when the evidence count grows, so an old result cannot oscillate a belief on
/// every tick or immediately undo a newer human correction.
pub fn evaluate(dir: &Path, now: i64) -> io::Result<Vec<BeliefTransition>> {
    let results = prediction::results(dir)?;
    let predictions = prediction::load(dir);
    let mut thread_ids = BTreeSet::new();
    thread_ids.extend(results.iter().map(|result| result.thread_id.clone()));
    thread_ids.extend(
        predictions
            .predictions
            .iter()
            .map(|prediction| prediction.thread_id.clone()),
    );

    let mut file = load(dir)?;
    let mut written = Vec::new();
    for thread_id in thread_ids {
        let evidence = summarize(&results, &thread_id);
        let belief = if let Some(index) = file
            .beliefs
            .iter()
            .position(|belief| belief.thread_id == thread_id)
        {
            &mut file.beliefs[index]
        } else {
            file.beliefs.push(Belief::new(&thread_id, now));
            file.beliefs.last_mut().expect("belief was just inserted")
        };

        let grew = evidence.total() > belief.evaluated_results;
        belief.evaluated_results = evidence.total();
        belief.evidence = evidence.clone();
        if grew {
            if let Some(next) = calibrated_next(belief.state, &evidence) {
                written.push(record_transition(
                    belief,
                    next,
                    TransitionCause::Calibration,
                    evidence,
                    now,
                ));
            }
        }
    }
    for transition in &written {
        store::append(dir, BELIEF_TRANSITIONS_FILE, transition)?;
    }
    save(dir, &file)?;
    Ok(written)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverrideKind {
    HumanCorrection,
    HardActReversal,
}

impl OverrideKind {
    fn target(self) -> BeliefState {
        match self {
            Self::HumanCorrection => BeliefState::Doubtful,
            Self::HardActReversal => BeliefState::Abandoned,
        }
    }

    fn cause(self) -> TransitionCause {
        match self {
            Self::HumanCorrection => TransitionCause::HumanCorrection,
            Self::HardActReversal => TransitionCause::HardActReversal,
        }
    }
}

/// Apply the dialogue Q5 exception. `evidence_id` makes replay idempotent; `line`
/// is retained as the contradicting citation rather than converted into a score.
pub fn apply_override(
    dir: &Path,
    thread_id: &str,
    kind: OverrideKind,
    evidence_id: &str,
    line: &str,
    now: i64,
) -> io::Result<Option<BeliefTransition>> {
    if thread_id.trim().is_empty() || evidence_id.trim().is_empty() || line.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a belief override needs a thread, evidence id, and citation",
        ));
    }
    let mut file = load(dir)?;
    let belief = if let Some(index) = file
        .beliefs
        .iter()
        .position(|belief| belief.thread_id == thread_id)
    {
        &mut file.beliefs[index]
    } else {
        file.beliefs.push(Belief::new(thread_id, now));
        file.beliefs.last_mut().expect("belief was just inserted")
    };
    if belief
        .applied_overrides
        .iter()
        .any(|applied| applied == evidence_id)
    {
        return Ok(None);
    }
    belief.applied_overrides.push(evidence_id.to_string());
    let mut evidence = belief.evidence.clone();
    evidence.contradicting = Some(EvidenceCitation {
        evidence_id: evidence_id.to_string(),
        line: bounded(line.trim(), 240),
        at: now,
    });

    let target = kind.target();
    let transition = match (belief.state, target) {
        (BeliefState::Abandoned, _) | (BeliefState::Doubtful, BeliefState::Doubtful) => None,
        _ => Some(record_transition(
            belief,
            target,
            kind.cause(),
            evidence,
            now,
        )),
    };
    if let Some(transition) = &transition {
        store::append(dir, BELIEF_TRANSITIONS_FILE, transition)?;
    }
    save(dir, &file)?;
    Ok(transition)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NarrationCandidate {
    pub thread_id: String,
    pub text: String,
    pub consequence: u8,
    pub transition_at: i64,
}

/// Highest consequence first; ties prefer the newer transition, then stable id.
/// This does not mutate — the caller records the aside, then calls [`mark_narrated`].
pub fn next_narration(dir: &Path, now: i64) -> io::Result<Option<NarrationCandidate>> {
    let file = load(dir)?;
    let threads = thread::load(dir).unwrap_or_default();
    let mut candidates = Vec::new();
    for belief in &file.beliefs {
        let Some(transition) = &belief.pending_narration else {
            continue;
        };
        if belief.last_narrated_at > 0
            && now.saturating_sub(belief.last_narrated_at) < NARRATION_COOLDOWN_SECS
        {
            continue;
        }
        let theory = threads
            .iter()
            .find(|thread| thread.id == belief.thread_id)
            .map(|thread| thread.theory.as_str())
            .unwrap_or(&belief.thread_id);
        candidates.push(NarrationCandidate {
            thread_id: belief.thread_id.clone(),
            text: narration(theory, transition),
            consequence: consequence(transition.to),
            transition_at: transition.at,
        });
    }
    Ok(candidates.into_iter().max_by(|a, b| {
        (a.consequence, a.transition_at)
            .cmp(&(b.consequence, b.transition_at))
            .then_with(|| b.thread_id.cmp(&a.thread_id))
    }))
}

pub fn mark_narrated(dir: &Path, thread_id: &str, now: i64) -> io::Result<bool> {
    let mut file = load(dir)?;
    let Some(belief) = file
        .beliefs
        .iter_mut()
        .find(|belief| belief.thread_id == thread_id)
    else {
        return Ok(false);
    };
    if belief.pending_narration.is_none() {
        return Ok(false);
    }
    belief.pending_narration = None;
    belief.last_narrated_at = now;
    save(dir, &file)?;
    Ok(true)
}

fn consequence(state: BeliefState) -> u8 {
    match state {
        BeliefState::Tentative => 0,
        BeliefState::Supported => 1,
        BeliefState::Doubtful => 2,
        BeliefState::Abandoned => 3,
    }
}

fn narration(theory: &str, transition: &BeliefTransition) -> String {
    let theory = bounded(theory.trim(), 180);
    let opening = match transition.to {
        BeliefState::Tentative => format!("I am treating “{theory}” as tentative"),
        BeliefState::Supported => format!("I now have support for “{theory}”"),
        BeliefState::Doubtful => format!("I now doubt “{theory}”"),
        BeliefState::Abandoned => format!("I no longer think “{theory}”"),
    };
    let mut text = format!(
        "{opening}; {} predictions held and {} did not.",
        transition.evidence.favorable, transition.evidence.unfavorable
    );
    if let Some(citation) = &transition.evidence.supporting {
        text.push_str(&format!(
            " Supporting evidence: {}.",
            bounded(citation.line.trim_end_matches('.'), 220)
        ));
    }
    if let Some(citation) = &transition.evidence.contradicting {
        text.push_str(&format!(
            " Contradicting evidence: {}.",
            bounded(citation.line.trim_end_matches('.'), 220)
        ));
    }
    bounded(&text, 700)
}

fn bounded(text: &str, max_chars: usize) -> String {
    let mut out = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prediction::PredictionResult;
    use crate::thread::Thread;

    fn summary(favorable: usize, unfavorable: usize) -> EvidenceSummary {
        EvidenceSummary {
            favorable,
            unfavorable,
            supporting: Some(EvidenceCitation {
                evidence_id: "support".into(),
                line: "prediction p held".into(),
                at: 10,
            }),
            contradicting: (unfavorable > 0).then(|| EvidenceCitation {
                evidence_id: "contradiction".into(),
                line: "prediction q missed".into(),
                at: 20,
            }),
        }
    }

    fn temp(tag: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("familiar_belief_{}_{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn result(thread_id: &str, n: i64, favorable: bool) -> PredictionResult {
        PredictionResult {
            prediction_id: format!("pred-{n}"),
            thread_id: thread_id.into(),
            opened_by: format!("obs-{n}"),
            opened_at: n,
            deadline: n + 10,
            settled_by: favorable.then(|| format!("obs-{}", n + 1)),
            outcome: if favorable {
                Outcome::Confirmed
            } else {
                Outcome::Missed
            },
            final_at: n + 20,
        }
    }

    fn theory(id: &str, words: &str) -> Thread {
        Thread {
            id: id.into(),
            question: String::new(),
            theory: words.into(),
            direction: String::new(),
            created_at: 1,
            status: "pursued".into(),
            status_at: 1,
            last_worked_at: 1,
            reinforced: 0,
            answers: Vec::new(),
            origin: "llm".into(),
            origin_human: String::new(),
            actor: "familiar".into(),
            anchors: Vec::new(),
            facts_rev: 0,
            v: 0,
            family_key: String::new(),
            variant_key: String::new(),
            superseded_by: String::new(),
            kind: String::new(),
            expires_at: 0,
            rule_proposal: None,
        }
    }

    #[test]
    fn hysteresis_needs_a_floor_doubt_and_stronger_recovery() {
        assert_eq!(
            calibrated_next(BeliefState::Tentative, &summary(1, 0)),
            None
        );
        assert_eq!(
            calibrated_next(BeliefState::Tentative, &summary(3, 0)),
            Some(BeliefState::Supported)
        );
        assert_eq!(
            calibrated_next(BeliefState::Supported, &summary(3, 2)),
            None
        );
        assert_eq!(
            calibrated_next(BeliefState::Supported, &summary(3, 3)),
            Some(BeliefState::Doubtful)
        );
        assert_eq!(calibrated_next(BeliefState::Doubtful, &summary(5, 3)), None);
        assert_eq!(
            calibrated_next(BeliefState::Doubtful, &summary(6, 3)),
            Some(BeliefState::Supported)
        );
        assert_eq!(
            calibrated_next(BeliefState::Doubtful, &summary(2, 4)),
            Some(BeliefState::Abandoned)
        );
        assert_eq!(
            calibrated_next(BeliefState::Abandoned, &summary(99, 0)),
            None
        );
    }

    #[test]
    fn fold_uses_append_only_results_and_ordinary_first_confirmation_is_silent() {
        let dir = temp("fold");
        store::append(
            &dir,
            prediction::PREDICTION_RESULTS_FILE,
            &result("thread-1", 1, true),
        )
        .unwrap();
        assert!(evaluate(&dir, 100).unwrap().is_empty());
        assert_eq!(load(&dir).unwrap().beliefs[0].state, BeliefState::Tentative);

        for n in 2..=3 {
            store::append(
                &dir,
                prediction::PREDICTION_RESULTS_FILE,
                &result("thread-1", n, true),
            )
            .unwrap();
        }
        let changed = evaluate(&dir, 200).unwrap();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].to, BeliefState::Supported);
        assert_eq!(transitions(&dir).unwrap(), changed);
    }

    #[test]
    fn human_correction_and_hard_reversal_bypass_the_sample_floor_idempotently() {
        let dir = temp("overrides");
        let correction = apply_override(
            &dir,
            "thread-1",
            OverrideKind::HumanCorrection,
            "obs-correction",
            "Ian said that was wrong",
            100,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            (correction.from, correction.to),
            (BeliefState::Tentative, BeliefState::Doubtful)
        );
        assert!(apply_override(
            &dir,
            "thread-1",
            OverrideKind::HumanCorrection,
            "obs-correction",
            "Ian said that was wrong",
            101,
        )
        .unwrap()
        .is_none());

        let reversal = apply_override(
            &dir,
            "thread-1",
            OverrideKind::HardActReversal,
            "act-reversal",
            "Ian undid the light change",
            200,
        )
        .unwrap()
        .unwrap();
        assert_eq!(reversal.to, BeliefState::Abandoned);
        assert_eq!(reversal.cause, TransitionCause::HardActReversal);
    }

    #[test]
    fn narration_keeps_counts_both_citations_priority_and_cooldown() {
        let dir = temp("narration");
        thread::append(&dir, &theory("thread-doubt", "the lights follow presence")).unwrap();
        thread::append(&dir, &theory("thread-stop", "the room prefers dim light")).unwrap();

        apply_override(
            &dir,
            "thread-doubt",
            OverrideKind::HumanCorrection,
            "correction",
            "Ian said presence was not the cause",
            100,
        )
        .unwrap();
        apply_override(
            &dir,
            "thread-stop",
            OverrideKind::HardActReversal,
            "reversal",
            "Ian restored the prior light level",
            101,
        )
        .unwrap();

        let first = next_narration(&dir, 200).unwrap().unwrap();
        assert_eq!(
            first.thread_id, "thread-stop",
            "abandonment has consequence priority"
        );
        assert!(first.text.contains("I no longer think"));
        assert!(first.text.contains("0 predictions held and 0 did not"));
        assert!(first.text.contains("Contradicting evidence:"));
        assert!(mark_narrated(&dir, &first.thread_id, 200).unwrap());

        apply_override(
            &dir,
            "thread-stop",
            OverrideKind::HardActReversal,
            "later-reversal",
            "Ian undid it again",
            201,
        )
        .unwrap();
        let second = next_narration(&dir, 202).unwrap().unwrap();
        assert_eq!(second.thread_id, "thread-doubt");
        assert!(mark_narrated(&dir, &second.thread_id, 202).unwrap());
        assert!(next_narration(&dir, 203).unwrap().is_none());

        for n in 1..=6 {
            store::append(
                &dir,
                prediction::PREDICTION_RESULTS_FILE,
                &result("thread-doubt", n, true),
            )
            .unwrap();
        }
        let recovered = evaluate(&dir, 300).unwrap();
        assert_eq!(recovered[0].to, BeliefState::Supported);
        assert!(
            next_narration(&dir, 301).unwrap().is_none(),
            "the same theory rests inside its narration cooldown"
        );
        assert_eq!(
            next_narration(&dir, 202 + NARRATION_COOLDOWN_SECS)
                .unwrap()
                .unwrap()
                .thread_id,
            "thread-doubt"
        );

        let with_both = narration(
            "theory with mixed evidence",
            &BeliefTransition {
                thread_id: "thread-mixed".into(),
                from: BeliefState::Tentative,
                to: BeliefState::Supported,
                cause: TransitionCause::Calibration,
                at: 400,
                evidence: summary(3, 1),
            },
        );
        assert!(with_both.contains("Supporting evidence:"));
        assert!(with_both.contains("Contradicting evidence:"));
    }

    #[test]
    fn malformed_or_future_state_is_not_silently_reset() {
        let dir = temp("strict");
        fs::write(dir.join(BELIEFS_FILE), r#"{"version":2,"beliefs":[]}"#).unwrap();
        assert_eq!(load(&dir).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }
}
