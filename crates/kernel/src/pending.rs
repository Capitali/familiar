//! **The pending human decision** (T-220; progress-areas dialogue, codex's Round 2
//! design adopted in Round 3) — the durable object between a proposal and a person.
//!
//! The defect it ends, measured live: the one thread ever armed to mint a standing rule
//! eroded to `retired` on missed predictions WHILE waiting for its human's assent. The
//! constitutional design routes all action through assent — correctly — and then let the
//! assent target die of a clock. The wrong fix was freezing the theory (waiting must not
//! become immunity from counter-evidence). The right one separates two true statements:
//!
//! - evidence may make a theory stop being worth pursuing; and
//! - elapsed human response time must never erase a question or appropriate the choice.
//!
//! So the DECISION is durable: proposal, subject, surface, question, and a basis
//! snapshot, minted when the armed question is asked, answerable however the theory
//! fares. An affirmative answer re-validates against the THEN-CURRENT surface
//! declaration and boundary — it inherits no authority from a stale theory — and the
//! narration says plainly when the supporting theory weakened while the person decided.
//! Assent with the gate shut STAGES (`awaiting_gate`): the yes is kept, narrated once,
//! and one human gate-open completes the loop. A dismissal is "not now", never "no" —
//! only an explicit negative declines.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

use crate::reaction_rule::RuleProposal;
use crate::store;

pub const PENDING_FILE: &str = "pending_decisions.jsonl";

/// pending → (awaiting_gate) → assented | declined. Terminal states are decided by the
/// subject's words or the world's shape at validation — never by a timer.
pub const STATES: [&str; 4] = ["pending", "awaiting_gate", "assented", "declined"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingDecision {
    pub id: String,
    pub thread_id: String,
    /// The human whose assent decides (the proposal's subject).
    pub subject: String,
    pub surface: String,
    pub proposal: RuleProposal,
    /// The registry question that asked, and its text as asked (a snapshot — the
    /// decision must remain legible even if the question or thread is later gone).
    pub question_id: String,
    pub question: String,
    /// Basis snapshot at ask time: what the proposal stood on, for the record and for
    /// the honesty note if the theory later weakens.
    pub basis_theory: String,
    #[serde(default)]
    pub basis_anchors: Vec<String>,
    pub facts_rev: u32,
    pub asked_at: i64,
    /// Thread answers already present at mint — only LATER answers decide.
    #[serde(default)]
    pub answers_seen: usize,
    pub status: String,
    #[serde(default)]
    pub decided_at: i64,
    /// Honesty note stamped at decision time (e.g. the supporting theory had retired).
    #[serde(default)]
    pub note: String,
}

pub fn load(dir: &Path) -> io::Result<Vec<PendingDecision>> {
    store::load(dir, PENDING_FILE)
}

/// Mint the durable decision when an armed proposal's question is asked. One open
/// decision per thread — re-derivations strengthen the thread, not the pile.
#[allow(clippy::too_many_arguments)]
pub fn mint(
    dir: &Path,
    thread_id: &str,
    subject: &str,
    proposal: &RuleProposal,
    question_id: &str,
    question: &str,
    basis_theory: &str,
    basis_anchors: &[String],
    facts_rev: u32,
    answers_seen: usize,
    now: i64,
) -> io::Result<Option<String>> {
    let all = load(dir)?;
    if all.iter().any(|d| {
        d.thread_id == thread_id && matches!(d.status.as_str(), "pending" | "awaiting_gate")
    }) {
        return Ok(None); // one open decision per thread
    }
    let id = format!("decision-{:04}", all.len() + 1);
    store::append(
        dir,
        PENDING_FILE,
        &PendingDecision {
            id: id.clone(),
            thread_id: thread_id.to_string(),
            subject: subject.trim().to_lowercase(),
            surface: proposal.surface.clone(),
            proposal: proposal.clone(),
            question_id: question_id.to_string(),
            question: question.to_string(),
            basis_theory: basis_theory.to_string(),
            basis_anchors: basis_anchors.to_vec(),
            facts_rev,
            asked_at: now,
            answers_seen,
            status: "pending".to_string(),
            decided_at: 0,
            note: String::new(),
        },
    )?;
    Ok(Some(id))
}

/// Move a decision to a new state with an honesty note. The caller (the tick's heed
/// pass) owns the semantics; this owns the write.
pub fn transition(dir: &Path, id: &str, status: &str, note: &str, now: i64) -> io::Result<bool> {
    if !STATES.contains(&status) {
        return Ok(false);
    }
    let Some(mut d) = store::load_by_id::<PendingDecision>(dir, PENDING_FILE, id)? else {
        return Ok(false);
    };
    d.status = status.to_string();
    if matches!(status, "assented" | "declined") {
        d.decided_at = now;
    }
    if !note.trim().is_empty() {
        d.note = note.trim().to_string();
    }
    store::update_by_id(dir, PENDING_FILE, id, &d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct Temp(PathBuf);
    impl Temp {
        fn new(t: &str) -> Self {
            let p = std::env::temp_dir()
                .join(format!("familiar_pending_test_{}_{t}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Temp(p)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn rp() -> RuleProposal {
        RuleProposal {
            subject: "ian".into(),
            surface: "lights".into(),
            on_away: "dim".into(),
            on_back: "bright".into(),
        }
    }

    #[test]
    fn one_open_decision_per_thread_and_transitions_hold() {
        let t = Temp::new("mint");
        let id = mint(
            &t.0,
            "thread-0001",
            "Ian",
            &rp(),
            "q-0002",
            "Dim when away?",
            "lighting follows presence",
            &["obs-0001".to_string()],
            3,
            0,
            100,
        )
        .unwrap()
        .unwrap();
        // A second mint for the same thread while one is open: refused quietly.
        assert!(mint(
            &t.0,
            "thread-0001",
            "ian",
            &rp(),
            "q-0003",
            "again?",
            "t",
            &[],
            3,
            0,
            200
        )
        .unwrap()
        .is_none());
        // Staging on a shut gate, then assent — the yes survives the wait.
        assert!(transition(
            &t.0,
            &id,
            "awaiting_gate",
            "assent heard; allow_actuate is closed",
            300
        )
        .unwrap());
        assert!(transition(
            &t.0,
            &id,
            "assented",
            "the supporting theory had retired while you decided",
            400
        )
        .unwrap());
        let d = load(&t.0).unwrap().remove(0);
        assert_eq!(d.status, "assented");
        assert_eq!(d.decided_at, 400);
        assert_eq!(d.subject, "ian", "subject normalizes");
        assert!(d.note.contains("retired"));
        // A decided thread may be asked again later: mint opens a fresh decision.
        assert!(mint(
            &t.0,
            "thread-0001",
            "ian",
            &rp(),
            "q-0004",
            "still?",
            "t",
            &[],
            3,
            1,
            500
        )
        .unwrap()
        .is_some());
        // Unknown states never write.
        assert!(!transition(&t.0, &d.id, "vetoed", "", 600).unwrap());
    }
}
