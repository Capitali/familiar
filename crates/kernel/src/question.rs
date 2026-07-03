//! The question coordination — where the familiar decides *what to ask, and when*.
//!
//! The familiar does not fire questions at the human as it thinks of them. It keeps a
//! registry of everything it might ask — the origin-story root ("What do you need most
//! today?"), questions it forms from its theories, and clarifications it needs to complete
//! an observation or make a decision — and surfaces **one at a time**, chosen by a policy.
//!
//! A dismissed question is **never thrown away**. Dismissal is data: it grows the question's
//! rest period (so the familiar doesn't nag) and is recorded with its context (the seed of
//! later understanding *why* it was dismissed). The root question recurs whenever the policy
//! judges the moment right — weighing how often it's been dismissed, unmet human needs, and
//! the familiar's own need to know. Append-only JSONL; a rewrite updates a question's stats.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

use crate::store;

pub const QUESTIONS_FILE: &str = "questions.jsonl";
/// The origin-story root question — the standing one the familiar always returns to.
pub const ROOT_TEXT: &str = "What do you need most today?";
pub const ROOT_ID: &str = "q-root";

/// Rest after a question is *answered* before it may recur (root only; non-root answered
/// questions retire). Eight hours — a day's natural cadence.
pub const ANSWER_REST_SECS: i64 = 8 * 3600;
/// Base rest after a *dismissal*; the actual rest grows with how often it's been dismissed,
/// so a question the human keeps waving off is asked less and less — but never never.
pub const DISMISS_REST_SECS: i64 = 4 * 3600;
/// Cap on the grown dismissal rest — a week. Even an oft-dismissed question comes back.
pub const DISMISS_REST_MAX_SECS: i64 = 7 * 24 * 3600;

/// One thing the familiar may ask the human, with its history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub text: String,
    /// Where it came from: `"root"`, `"llm"` (a theory), `"need"` (to complete an observed
    /// need), `"observer"`. Used to prioritise.
    pub origin: String,
    pub created_at: i64,
    pub times_asked: u32,
    pub times_dismissed: u32,
    pub last_asked: i64,
    pub last_dismissed: i64,
    /// True once answered. The root question is the exception — it recurs regardless.
    pub answered: bool,
    /// Why it was waved off, when known — the seed of understanding dismissal, kept so it is
    /// never merely discarded.
    pub dismiss_notes: Vec<String>,
}

impl Question {
    fn new(id: &str, text: &str, origin: &str, now: i64) -> Self {
        Question {
            id: id.to_string(),
            text: text.to_string(),
            origin: origin.to_string(),
            created_at: now,
            times_asked: 0,
            times_dismissed: 0,
            last_asked: 0,
            last_dismissed: 0,
            answered: false,
            dismiss_notes: Vec::new(),
        }
    }

    fn is_root(&self) -> bool {
        self.id == ROOT_ID
    }

    /// How long this question should rest before it may surface again.
    fn rest_secs(&self) -> i64 {
        if self.last_dismissed >= self.last_asked && self.times_dismissed > 0 {
            (DISMISS_REST_SECS * (1 + self.times_dismissed as i64)).min(DISMISS_REST_MAX_SECS)
        } else if self.answered {
            ANSWER_REST_SECS
        } else {
            0 // never engaged yet — available immediately
        }
    }

    /// May this question surface now? Retired (answered, non-root) questions never do;
    /// everything else is available once it has rested long enough since last shown/dismissed.
    pub fn available(&self, now: i64) -> bool {
        if self.answered && !self.is_root() {
            return false;
        }
        let last = self.last_asked.max(self.last_dismissed);
        now - last >= self.rest_secs()
    }
}

pub fn load(dir: &Path) -> io::Result<Vec<Question>> {
    store::load(dir, QUESTIONS_FILE)
}

pub fn append(dir: &Path, q: &Question) -> io::Result<()> {
    store::append(dir, QUESTIONS_FILE, q)
}

/// Seed the root question once, so the familiar always has the origin-story question to
/// return to. Idempotent.
pub fn ensure_root(dir: &Path, now: i64) -> io::Result<()> {
    let qs = load(dir)?;
    if !qs.iter().any(|q| q.id == ROOT_ID) {
        append(dir, &Question::new(ROOT_ID, ROOT_TEXT, "root", now))?;
    }
    Ok(())
}

/// Add a question the familiar formed (e.g. from a theory or an unmet need), unless an
/// open question with the same text already exists (don't ask the same thing twice).
/// Returns the id used.
pub fn add(dir: &Path, text: &str, origin: &str, now: i64) -> io::Result<String> {
    let text = text.trim();
    let mut qs = load(dir)?;
    if let Some(existing) = qs.iter().find(|q| q.text == text) {
        return Ok(existing.id.clone());
    }
    let id = format!("q-{:04}", qs.len() + 1);
    let q = Question::new(&id, text, origin, now);
    qs.push(q.clone());
    append(dir, &q)?;
    Ok(id)
}

fn update<F: FnMut(&mut Question)>(dir: &Path, id: &str, mut f: F) -> io::Result<bool> {
    let Some(mut q) = store::load_by_id::<Question>(dir, QUESTIONS_FILE, id)? else {
        return Ok(false);
    };
    f(&mut q);
    store::update_by_id(dir, QUESTIONS_FILE, id, &q)
}

pub fn record_asked(dir: &Path, id: &str, now: i64) -> io::Result<bool> {
    update(dir, id, |q| {
        q.times_asked += 1;
        q.last_asked = now;
    })
}

/// A dismissal — tracked, never disposed. Grows the rest period and keeps the (optional)
/// reason so the familiar can later learn why this question doesn't land.
pub fn record_dismissed(dir: &Path, id: &str, now: i64, note: &str) -> io::Result<bool> {
    update(dir, id, |q| {
        q.times_dismissed += 1;
        q.last_dismissed = now;
        if !note.trim().is_empty() {
            q.dismiss_notes.push(note.trim().to_string());
        }
    })
}

pub fn record_answered(dir: &Path, id: &str, now: i64) -> io::Result<bool> {
    update(dir, id, |q| {
        q.answered = true;
        q.last_asked = now;
    })
}

/// The priority of an origin — higher surfaces first. Completing an observed human need
/// outranks the standing root, which outranks the familiar's own theories.
fn origin_rank(origin: &str) -> u8 {
    match origin {
        "need" => 3,
        "root" => 2,
        _ => 1,
    }
}

/// Choose the question to surface now (or `None` to ask nothing). Among the available
/// questions, prefer higher origin-rank; then the one dismissed *least* (don't nag); then
/// the oldest. `unmet_needs` biases the familiar toward asking about needs over the root.
pub fn next(questions: &[Question], now: i64, unmet_needs: usize) -> Option<&Question> {
    questions
        .iter()
        .filter(|q| q.available(now))
        .max_by_key(|q| {
            let mut rank = origin_rank(&q.origin) as i64;
            // when needs are waiting, lift need-questions decisively above the root
            if unmet_needs > 0 && q.origin == "need" {
                rank += 5;
            }
            // prefer the least-dismissed, then the oldest (negative for max_by_key)
            (rank, -(q.times_dismissed as i64), -q.created_at)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct Temp(PathBuf);
    impl Temp {
        fn new(t: &str) -> Self {
            let p = std::env::temp_dir().join(format!("familiar_question_test_{t}"));
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
    fn root_is_seeded_once_and_recurs_after_answering() {
        let t = Temp::new("root");
        ensure_root(&t.0, 0).unwrap();
        ensure_root(&t.0, 0).unwrap(); // idempotent
        assert_eq!(load(&t.0).unwrap().len(), 1);
        // answered -> rests, then recurs after the answer-rest window
        record_answered(&t.0, ROOT_ID, 1000).unwrap();
        let qs = load(&t.0).unwrap();
        assert!(!qs[0].available(1000), "just answered — resting");
        assert!(
            qs[0].available(1000 + ANSWER_REST_SECS),
            "the root question returns"
        );
    }

    #[test]
    fn dismissal_is_tracked_grows_the_rest_and_is_never_disposed() {
        let t = Temp::new("dismiss");
        ensure_root(&t.0, 0).unwrap();
        record_dismissed(&t.0, ROOT_ID, 1000, "not now").unwrap();
        let q = &load(&t.0).unwrap()[0];
        assert_eq!(q.times_dismissed, 1);
        assert_eq!(q.dismiss_notes, vec!["not now".to_string()]);
        // still present (not disposed), resting longer than a fresh dismissal would
        assert!(!q.available(1000 + DISMISS_REST_SECS - 1));
        assert!(q.available(1000 + 2 * DISMISS_REST_SECS));
        // dismissed again -> rests longer still (asked less and less, never never)
        record_dismissed(&t.0, ROOT_ID, 2000, "").unwrap();
        let q = &load(&t.0).unwrap()[0];
        assert_eq!(q.times_dismissed, 2);
        assert!(!q.available(2000 + 2 * DISMISS_REST_SECS));
    }

    #[test]
    fn need_questions_outrank_the_root_when_needs_wait() {
        let t = Temp::new("rank");
        ensure_root(&t.0, 0).unwrap();
        add(&t.0, "Did the backup finish?", "need", 10).unwrap();
        let qs = load(&t.0).unwrap();
        // with an unmet need pending, the need-question is chosen over the root
        assert_eq!(next(&qs, 100, 1).map(|q| q.origin.as_str()), Some("need"));
        // dedup: adding the same text again doesn't create a second
        add(&t.0, "Did the backup finish?", "need", 20).unwrap();
        assert_eq!(load(&t.0).unwrap().len(), 2);
    }
}
