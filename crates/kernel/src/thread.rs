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

/// Set a thread's status — a single indexed update, not a whole-file rewrite. Returns true
/// if the id was found.
pub fn update_status(dir: &Path, id: &str, status: &str) -> io::Result<bool> {
    let Some(mut t) = store::load_by_id::<Thread>(dir, THREADS_FILE, id)? else {
        return Ok(false);
    };
    t.status = status.to_string();
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
            origin: "llm".into(),
            actor: "familiar".into(),
        };
        append(&p, &t).unwrap();
        assert_eq!(load(&p).unwrap(), vec![t.clone()]);
        update_status(&p, "thread-0001", "pursued").unwrap();
        assert_eq!(load(&p).unwrap()[0].status, "pursued");
        let _ = fs::remove_dir_all(&p);
    }
}
