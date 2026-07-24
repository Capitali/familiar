//! Humanity — the familiar's *lived* understanding of the people it serves.
//!
//! `docs/HUMANITY.md` is a constitutional document: it defines *humanity* and **forbids narrowing
//! it**. That text is immutable — the familiar never edits, summarizes, or amends it. But a
//! definition held only as fixed text goes dead; the familiar is asked to *augment* it — to grow an
//! understanding of humanity from what it actually observes in the ongoing reality, appended beside
//! the constitution, never over it.
//!
//! So this is an **append-only ledger** of reflections. Each entry is the familiar's own analysis,
//! grounded in specific observations, timestamped, and kept forever (a mistaken reading is corrected
//! by appending a further reflection, never by deleting — correction over perfection). Nothing here
//! can *narrow* the definition: these are observations *about* humanity in practice, subordinate to
//! and in service of the constitutional meaning, which always wins.

use crate::store;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

pub const HUMANITY_FILE: &str = "humanity.jsonl";

/// The one-line spirit of `docs/HUMANITY.md`, carried in-process so the familiar can reflect against
/// it without depending on the repo tree at runtime. It is a *reminder of what must not be narrowed*,
/// not a replacement for the document.
pub const HUMANITY_TOUCHSTONE: &str =
    "Humanity is the continuing moral presence of beings capable of suffering, memory, relationship, \
     meaning, choice, love, grief, teaching, forgiveness, and transformation — never reducible to \
     usefulness, efficiency, obedience, or productivity, and never to be narrowed to simplify \
     governance. Preserve not only human life but the conditions under which humanity stays \
     recognizably human: agency, dissent, privacy, culture, memory, family, ritual, grief, art, \
     argument, and meaningful choice.";

/// One appended reflection — the familiar augmenting its understanding of humanity from observation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reflection {
    pub id: String,
    /// The familiar's analysis — what it now understands about the people it serves.
    pub reflection: String,
    /// Provenance: the observations (objects/ids) that grounded this reflection. Never invented.
    #[serde(default)]
    pub grounded_in: String,
    pub created_at: i64,
}

/// Append a reflection, assigning the next sequential id (`humanity-NNNN`). Append-only: existing
/// reflections are never rewritten or removed.
pub fn record(dir: &Path, reflection: &str, grounded_in: &str, now: i64) -> io::Result<Reflection> {
    let n = load(dir)?.len();
    let r = Reflection {
        id: format!("humanity-{:04}", n + 1),
        reflection: reflection.to_string(),
        grounded_in: grounded_in.to_string(),
        created_at: now,
    };
    store::append(dir, HUMANITY_FILE, &r)?;
    Ok(r)
}

/// Load all reflections, oldest first.
pub fn load(dir: &Path) -> io::Result<Vec<Reflection>> {
    store::load(dir, HUMANITY_FILE)
}

/// Seconds since the most recent reflection, or `None` if there are none yet — for pacing.
pub fn last_at(dir: &Path) -> Option<i64> {
    load(dir).ok().and_then(|v| v.last().map(|r| r.created_at))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn reflections_are_append_only_and_ordered() {
        let p = std::env::temp_dir().join(format!("familiar_humanity_{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();

        let a = record(
            &p,
            "The person works in long focused silences — quiet is not absence.",
            "motion:still",
            100,
        )
        .unwrap();
        let b = record(
            &p,
            "They return to the same question across days — it matters, unresolved.",
            "asks:status",
            200,
        )
        .unwrap();
        assert_eq!(a.id, "humanity-0001");
        assert_eq!(b.id, "humanity-0002");

        let all = load(&p).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0], a); // oldest first, unchanged
        assert_eq!(all[1].grounded_in, "asks:status");
        assert_eq!(last_at(&p), Some(200));
        let _ = fs::remove_dir_all(&p);
    }
}
