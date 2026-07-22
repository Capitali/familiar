//! Thread — a question the factory poses and a theory it holds.
//!
//! The **Interpret** step of the cycle made durable: as the factory observes, it
//! forms questions (to ask the human) and theories (about what the patterns mean).
//! These are *not* observations — observations are the only truth, of the world;
//! a thread is the factory reasoning *about* that truth. A minimal port of v1's
//! richer `thread_t` (fitness/decay/lineage come later).

use crate::store;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

pub const THREADS_FILE: &str = "threads.jsonl";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Thread {
    pub id: String,
    /// A question for the human, grounded in what was observed.
    pub question: String,
    /// The factory's interpretation of what the patterns mean.
    pub theory: String,
    /// What the factory could *do* to act on this theory in service — becomes a
    /// candidate's hypothesis when the thread is pursued. (Optional.)
    #[serde(default)]
    pub direction: String,
    pub created_at: i64,
    /// open | pursued | answered | abandoned | marginalized
    pub status: String,
    /// When the thread entered its *current* status (unix secs) — whatever state a theory
    /// is in, it carries the date it got there. Backfilled to `created_at` for old rows.
    #[serde(default)]
    pub status_at: i64,
    /// Last time this thread was actively worked (pursued, evidence added, answered).
    #[serde(default)]
    pub last_worked_at: i64,
    /// The human's answers to this thread's question — evidence the pursuit carries.
    /// Empty until someone answers; each answer stamps `last_worked_at`.
    #[serde(default)]
    pub answers: Vec<String>,
    /// llm | observer
    pub origin: String,
    /// Who authored the directive — the actor whose reputation governs whether it is
    /// pursued (corruption awareness, Brick 20). `"familiar"` for its own theories;
    /// `"ian"` (or another human) for observer answers. Empty = unattributed (always
    /// pursued). `#[serde(default)]` so older threads still load.
    #[serde(default)]
    pub actor: String,
}

pub fn append(dir: &Path, t: &Thread) -> io::Result<()> {
    store::append(dir, THREADS_FILE, t)
}

pub fn load(dir: &Path) -> io::Result<Vec<Thread>> {
    store::load(dir, THREADS_FILE)
}

/// Set a thread's status at `now` — a single indexed update, not a whole-file rewrite,
/// stamping `status_at` (and `last_worked_at` when the transition is active work: pursued
/// or answered). Returns true if the id was found.
pub fn update_status(dir: &Path, id: &str, status: &str, now: i64) -> io::Result<bool> {
    let Some(mut t) = store::load_by_id::<Thread>(dir, THREADS_FILE, id)? else {
        return Ok(false);
    };
    if t.status != status {
        t.status_at = now;
    }
    t.status = status.to_string();
    if matches!(status, "pursued" | "answered") {
        t.last_worked_at = now;
    }
    store::update_by_id(dir, THREADS_FILE, id, &t)
}

/// The human answered this thread's question. The answer is appended as evidence, the
/// thread is stamped as actively worked, and a discarded thread is REVIVED to open —
/// a human choosing to answer outranks the factory's earlier triage.
pub fn add_answer(dir: &Path, id: &str, text: &str, now: i64) -> io::Result<bool> {
    let Some(mut t) = store::load_by_id::<Thread>(dir, THREADS_FILE, id)? else {
        return Ok(false);
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(false);
    }
    t.answers.push(trimmed.to_string());
    t.last_worked_at = now;
    if matches!(t.status.as_str(), "abandoned" | "marginalized" | "answered") {
        t.status = "open".into();
        t.status_at = now;
    }
    store::update_by_id(dir, THREADS_FILE, id, &t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn round_trips() {
        let p = std::env::temp_dir().join("substrate_thread_test");
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        let t = Thread {
            id: "thread-0001".into(),
            question: "What would make mornings calmer?".into(),
            theory: "Repeated status requests suggest a standing digest would help.".into(),
            direction: "offer a standing morning digest".into(),
            created_at: 100,
            status: "open".into(),
            status_at: 100,
            last_worked_at: 0,
            answers: Vec::new(),
            origin: "llm".into(),
            actor: "familiar".into(),
        };
        append(&p, &t).unwrap();
        assert_eq!(load(&p).unwrap(), vec![t.clone()]);
        update_status(&p, "thread-0001", "pursued", 200).unwrap();
        let updated = &load(&p).unwrap()[0];
        assert_eq!(updated.status, "pursued");
        assert_eq!(updated.status_at, 200, "a status change is dated");
        assert_eq!(updated.last_worked_at, 200, "pursuing is active work");
        add_answer(&p, "thread-0001", "mornings mean before 10am", 300).unwrap();
        let t2 = &load(&p).unwrap()[0];
        assert_eq!(t2.answers, vec!["mornings mean before 10am"]);
        assert_eq!(t2.last_worked_at, 300, "answering is active work");
        let _ = fs::remove_dir_all(&p);
    }
}
