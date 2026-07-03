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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct Temp(PathBuf);
    impl Temp {
        fn new(t: &str) -> Self {
            let p = std::env::temp_dir().join(format!("familiar_request_test_{t}"));
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
}
