//! Requests and answers — the human asks, the familiar analyzes and answers.
//!
//! Until now the familiar asked and Ian answered. This is the other direction: Ian poses
//! a free-form request ("do I have network-config issues?"), the cycle analyzes it, and
//! the familiar answers. The answer carries a **confidence** that is the guard against
//! misinformation: `Known` is grounded in facts the familiar verified (its own sensing or
//! observations); `Probable` is its most-likely reasoning, *labeled* as not certain;
//! `Unknown` means it will say so rather than invent. The familiar never fabricates — a
//! known fact or the most-probable, clearly-labeled answer, never a confident guess.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

use crate::store;

pub const REQUESTS_FILE: &str = "requests.jsonl";
pub const ANSWERS_FILE: &str = "answers.jsonl";

/// A free-form request from a human to the familiar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub id: String,
    /// Who asked — governs reputation (corruption awareness).
    pub actor: String,
    pub text: String,
    pub created_at: i64,
    /// open | answered | refused
    pub status: String,
}

/// How sure the familiar is of an answer — the discipline against misinformation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Grounded in facts the familiar verified (its sensing / observations).
    Known,
    /// Its most-likely reasoning, labeled as not certain.
    Probable,
    /// It cannot answer from what it knows and will not guess.
    Unknown,
}

impl Confidence {
    pub fn label(self) -> &'static str {
        match self {
            Confidence::Known => "known",
            Confidence::Probable => "probable",
            Confidence::Unknown => "unknown",
        }
    }
}

/// The familiar's answer to a request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Answer {
    pub id: String,
    pub request_id: String,
    pub body: String,
    pub confidence: Confidence,
    /// What grounds the answer (the facts cited), or what would confirm a probable one.
    pub evidence: String,
    pub created_at: i64,
    /// "" | helpful | refine — the human's reaction, which steers refinement.
    #[serde(default)]
    pub feedback: String,
    /// The id of the tool that produced this answer, if one ran (empty otherwise). Lets a
    /// "refine" reaction retire the responsible tool so it is re-authored, not reused.
    #[serde(default)]
    pub tool_id: String,
}

pub fn append_request(dir: &Path, r: &Request) -> io::Result<()> {
    store::append(dir, REQUESTS_FILE, r)
}

pub fn load_requests(dir: &Path) -> io::Result<Vec<Request>> {
    store::load(dir, REQUESTS_FILE)
}

pub fn append_answer(dir: &Path, a: &Answer) -> io::Result<()> {
    store::append(dir, ANSWERS_FILE, a)
}

pub fn load_answers(dir: &Path) -> io::Result<Vec<Answer>> {
    store::load(dir, ANSWERS_FILE)
}

/// Set a request's status — a single indexed update. Returns true if the id was found.
pub fn update_status(dir: &Path, id: &str, status: &str) -> io::Result<bool> {
    let Some(mut r) = store::load_by_id::<Request>(dir, REQUESTS_FILE, id)? else {
        return Ok(false);
    };
    r.status = status.to_string();
    store::update_by_id(dir, REQUESTS_FILE, id, &r)
}

/// Record the human's reaction to an answer (helpful / refine) — a single indexed update.
pub fn set_feedback(dir: &Path, answer_id: &str, feedback: &str) -> io::Result<bool> {
    let Some(mut a) = store::load_by_id::<Answer>(dir, ANSWERS_FILE, answer_id)? else {
        return Ok(false);
    };
    a.feedback = feedback.to_string();
    store::update_by_id(dir, ANSWERS_FILE, answer_id, &a)
}

/// The feedback intake, completing the chain `Answer.tool_id → set_feedback →
/// tool::mark_unhealthy` that was built for exactly this and never wired: an observation
/// shaped `<any actor> / feedback / helpful|refine / answer:<id>` (from a device via the
/// observe seam, or the local console) records the reaction, and a **refine** retires the
/// *authored* tool behind the answer so it is re-authored, not reused. A **declared**
/// actuator tool is never retired this way — it ran correctly, the *decision* was wrong,
/// and reaction-to-acts has its own machinery (ADR-0032); retiring it would silently
/// disable the surface, including its own revert path. A no-op for any other shape, so
/// callers run this unconditionally over incoming observations.
pub fn maybe_apply_feedback(
    dir: &Path,
    action: &str,
    object: &str,
    context: &str,
) -> io::Result<Option<String>> {
    if action != "feedback" {
        return Ok(None);
    }
    let Some(answer_id) = context.trim().strip_prefix("answer:") else {
        return Ok(None);
    };
    let verdict = object.trim().to_lowercase();
    if !matches!(verdict.as_str(), "helpful" | "refine") {
        return Ok(None);
    }
    if !set_feedback(dir, answer_id, &verdict)? {
        return Ok(None);
    }
    if verdict != "refine" {
        return Ok(Some(String::new()));
    }
    let tool_id = store::load_by_id::<Answer>(dir, ANSWERS_FILE, answer_id)?
        .map(|a| a.tool_id)
        .unwrap_or_default();
    if tool_id.is_empty() {
        return Ok(Some(String::new()));
    }
    let declared = crate::tool::load(dir)?
        .iter()
        .any(|t| t.id == tool_id && t.origin == "declared");
    if declared {
        return Ok(Some(String::new()));
    }
    crate::tool::mark_unhealthy(dir, &tool_id)?;
    Ok(Some(tool_id))
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
                .join(format!("familiar_request_test_{}_{t}", std::process::id()));
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

    #[test]
    fn request_and_answer_round_trip_with_status_and_feedback() {
        let t = Temp::new("roundtrip");
        append_request(
            &t.0,
            &Request {
                id: "req-0001".into(),
                actor: "ian".into(),
                text: "do I have network issues?".into(),
                created_at: 100,
                status: "open".into(),
            },
        )
        .unwrap();
        append_answer(
            &t.0,
            &Answer {
                id: "ans-0001".into(),
                request_id: "req-0001".into(),
                body: "en0 is up and 1.1.1.1 is reachable.".into(),
                confidence: Confidence::Known,
                evidence: "host has interface:en0; connectivity:online".into(),
                created_at: 101,
                feedback: String::new(),
                tool_id: String::new(),
            },
        )
        .unwrap();

        update_status(&t.0, "req-0001", "answered").unwrap();
        set_feedback(&t.0, "ans-0001", "helpful").unwrap();

        assert_eq!(load_requests(&t.0).unwrap()[0].status, "answered");
        let a = &load_answers(&t.0).unwrap()[0];
        assert_eq!(a.confidence, Confidence::Known);
        assert_eq!(a.feedback, "helpful");
    }

    fn answer_with_tool(dir: &std::path::Path, ans: &str, tool_id: &str) {
        append_answer(
            dir,
            &Answer {
                id: ans.into(),
                request_id: "req-0001".into(),
                body: "b".into(),
                confidence: Confidence::Known,
                evidence: String::new(),
                created_at: 101,
                feedback: String::new(),
                tool_id: tool_id.into(),
            },
        )
        .unwrap();
    }

    fn tool_row(id: &str, origin: &str) -> crate::tool::Tool {
        crate::tool::Tool {
            id: id.into(),
            name: "n".into(),
            purpose: "p".into(),
            keywords: "k".into(),
            script_path: "/dev/null".into(),
            created_at: 1,
            uses: 0,
            last_used: 0,
            last_exit_ok: true,
            last_status: String::new(),
            origin: origin.into(),
            origin_verified_at: 0,
            null_streak: 0,
            last_useful_at: 0,
        }
    }

    #[test]
    fn a_refine_reaction_retires_the_authored_tool_behind_the_answer() {
        // The chain Answer.tool_id → set_feedback → mark_unhealthy, finally producing.
        let t = Temp::new("feedback_chain");
        answer_with_tool(&t.0, "ans-0001", "tool-0007");
        crate::tool::append(&t.0, &tool_row("tool-0007", "")).unwrap();
        let retired = maybe_apply_feedback(&t.0, "feedback", "refine", "answer:ans-0001").unwrap();
        assert_eq!(retired.as_deref(), Some("tool-0007"));
        let tl = &crate::tool::load(&t.0).unwrap()[0];
        assert!(
            !tl.last_exit_ok,
            "refine retires the authored tool from reuse"
        );
        assert_eq!(load_answers(&t.0).unwrap()[0].feedback, "refine");
    }

    #[test]
    fn a_declared_actuator_tool_survives_a_refine() {
        // It ran correctly; the DECISION was wrong — and retiring it would kill the
        // revert path too. Reaction-to-acts has its own machinery (ADR-0032).
        let t = Temp::new("feedback_declared");
        answer_with_tool(&t.0, "ans-0001", "tool-act-lights-dim");
        crate::tool::append(&t.0, &tool_row("tool-act-lights-dim", "declared")).unwrap();
        let retired = maybe_apply_feedback(&t.0, "feedback", "refine", "answer:ans-0001").unwrap();
        assert_eq!(
            retired.as_deref(),
            Some(""),
            "feedback recorded, nothing retired"
        );
        assert!(crate::tool::load(&t.0).unwrap()[0].last_exit_ok);
        // And non-feedback shapes are a clean no-op.
        assert!(
            maybe_apply_feedback(&t.0, "told the familiar", "hi", "console")
                .unwrap()
                .is_none()
        );
    }
}
