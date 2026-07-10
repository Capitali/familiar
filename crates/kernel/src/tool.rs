//! The tool library — the familiar remembers the code it writes, so it reuses tools
//! instead of re-authoring them.
//!
//! When the familiar writes a script to answer a request, it keeps it: a named, described
//! **Tool**, persisted with its purpose and the keywords it serves. The next time a similar
//! request arrives it *recognizes* the tool and re-runs it — no LLM authoring, just fresh
//! execution. This is Law I made concrete (Soul: "motion becomes service only when it makes
//! the future cheaper than the past"): a growing library of skills, each authored once and
//! reused many times. The scripts live in the familiar's workspace; this is the index over
//! them. Append-only JSONL (a rewrite updates usage stats), derived/rebuildable.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

use crate::store;

pub const TOOLS_FILE: &str = "tools.jsonl";

/// A reusable capability the familiar authored once and can run again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tool {
    pub id: String,
    /// A short slug, e.g. `cpu_load`.
    pub name: String,
    /// What it does — human-readable, and used (with `keywords`) to recognize a match.
    pub purpose: String,
    /// Space-joined content words this tool serves (from the request it was born for).
    pub keywords: String,
    /// Absolute path to the persisted script in the workspace.
    pub script_path: String,
    pub created_at: i64,
    /// How many times it has been run — the efficiency dividend, visible.
    pub uses: u32,
    pub last_used: i64,
    /// Did its most recent run exit cleanly? A tool that keeps failing should not be reused.
    pub last_exit_ok: bool,
    /// A short, human-readable verdict on the most recent run — e.g. "timed out after 10120ms",
    /// "output looked wrong (permission denied)", "exit 0 in 180ms". Shown in the Glass so a
    /// failure is diagnosable, not just an orange badge. Empty until the tool has run.
    #[serde(default)]
    pub last_status: String,
    /// Provenance. Empty when this node authored the tool itself; otherwise the `node_id` of
    /// the mesh peer it was federated from. A federated tool is trusted into the *library*
    /// but — like any tool — still passes `review_script` + the sandbox on every run.
    #[serde(default)]
    pub origin: String,
    /// When a federated tool's body was verified (sha-matched) on merge; 0 for local tools.
    #[serde(default)]
    pub origin_verified_at: i64,
}

impl Tool {
    /// How strongly this tool matches a request's content words — the count of request
    /// keywords that appear in the tool's keywords, name, or purpose (all lowercased).
    pub fn overlap(&self, request_keywords: &[String]) -> usize {
        let hay = format!("{} {} {}", self.keywords, self.name, self.purpose).to_lowercase();
        let tokens: std::collections::HashSet<&str> = hay
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| !t.is_empty())
            .collect();
        request_keywords
            .iter()
            .filter(|w| tokens.contains(w.as_str()))
            .count()
    }
}

pub fn append(dir: &Path, t: &Tool) -> io::Result<()> {
    store::append(dir, TOOLS_FILE, t)
}

pub fn load(dir: &Path) -> io::Result<Vec<Tool>> {
    store::load(dir, TOOLS_FILE)
}

/// The healthy tool that best matches the request, if the match is strong enough to trust.
/// Conservative: requires at least two shared keywords (so a single common word never
/// triggers the wrong tool), and skips tools whose last run failed. Correctness over reuse.
pub fn best_match<'a>(tools: &'a [Tool], request_keywords: &[String]) -> Option<&'a Tool> {
    tools
        .iter()
        .filter(|t| t.last_exit_ok)
        .map(|t| (t, t.overlap(request_keywords)))
        .filter(|(_, n)| *n >= 2)
        .max_by_key(|(_, n)| *n)
        .map(|(t, _)| t)
}

/// Record a run of a tool: bump `uses`, stamp `last_used`, note whether it exited cleanly and
/// a short human-readable `status` (the verdict shown in the Glass). Returns the tool's new use
/// count (or None if the id was not found).
pub fn record_use(
    dir: &Path,
    id: &str,
    now: i64,
    exit_ok: bool,
    status: &str,
) -> io::Result<Option<u32>> {
    let Some(mut t) = store::load_by_id::<Tool>(dir, TOOLS_FILE, id)? else {
        return Ok(None);
    };
    t.uses += 1;
    t.last_used = now;
    t.last_exit_ok = exit_ok;
    t.last_status = status.to_string();
    let uses = t.uses;
    store::update_by_id(dir, TOOLS_FILE, id, &t)?;
    Ok(Some(uses))
}

/// Retire a tool by marking it unhealthy, so [`best_match`] skips it and the familiar
/// re-authors a fresh one instead of reusing it. Used when the human's feedback says an
/// answer a tool produced was wrong. Returns true if the id was found.
pub fn mark_unhealthy(dir: &Path, id: &str) -> io::Result<bool> {
    let Some(mut t) = store::load_by_id::<Tool>(dir, TOOLS_FILE, id)? else {
        return Ok(false);
    };
    t.last_exit_ok = false;
    t.last_status = "retired by your feedback".to_string();
    store::update_by_id(dir, TOOLS_FILE, id, &t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct Temp(PathBuf);
    impl Temp {
        fn new(t: &str) -> Self {
            let p = std::env::temp_dir().join(format!("familiar_tool_test_{t}"));
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

    fn tool(id: &str, name: &str, purpose: &str, keywords: &str) -> Tool {
        Tool {
            id: id.into(),
            name: name.into(),
            purpose: purpose.into(),
            keywords: keywords.into(),
            script_path: format!("/ws/{id}.sh"),
            created_at: 1,
            uses: 0,
            last_used: 0,
            last_exit_ok: true,
            last_status: String::new(),
            origin: String::new(),
            origin_verified_at: 0,
        }
    }

    #[test]
    fn best_match_reuses_a_strong_match_and_skips_weak_or_broken_ones() {
        let cpu = tool(
            "tool-0001",
            "cpu_load",
            "reports cpu load average and uptime",
            "cpu load uptime",
        );
        let mut broken = tool("tool-0002", "disk", "reports disk usage", "disk usage free");
        broken.last_exit_ok = false; // a failing tool is not reused
        let tools = vec![cpu.clone(), broken];
        // strong overlap -> reuse the cpu tool
        let kw: Vec<String> = ["cpu", "load", "average"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            best_match(&tools, &kw).map(|t| t.id.as_str()),
            Some("tool-0001")
        );
        // a request that only shares one common word -> no reuse (author fresh)
        let kw2: Vec<String> = vec!["cpu".into()];
        assert!(best_match(&tools, &kw2).is_none());
        // a request matching only the broken tool -> not reused
        let kw3: Vec<String> = ["disk", "usage"].iter().map(|s| s.to_string()).collect();
        assert!(best_match(&tools, &kw3).is_none());
    }

    #[test]
    fn record_use_increments_and_persists() {
        let t = Temp::new("use");
        append(&t.0, &tool("tool-0001", "cpu_load", "p", "cpu load")).unwrap();
        assert_eq!(
            record_use(&t.0, "tool-0001", 100, true, "exit 0 in 12ms").unwrap(),
            Some(1)
        );
        assert_eq!(
            record_use(&t.0, "tool-0001", 200, true, "exit 0 in 9ms").unwrap(),
            Some(2)
        );
        let reloaded = &load(&t.0).unwrap()[0];
        assert_eq!(reloaded.uses, 2);
        assert_eq!(reloaded.last_used, 200);
        assert_eq!(reloaded.last_status, "exit 0 in 9ms");
        assert_eq!(record_use(&t.0, "nope", 1, true, "").unwrap(), None);
    }

    #[test]
    fn mark_unhealthy_retires_a_tool_from_reuse() {
        let t = Temp::new("retire");
        append(&t.0, &tool("tool-0001", "scan", "p", "network scan run results")).unwrap();
        let kw = vec!["network".to_string(), "scan".to_string()];
        // healthy → best_match will reuse it
        assert!(best_match(&load(&t.0).unwrap(), &kw).is_some());
        // the human's "refine" retires it → no longer a reuse candidate
        assert!(mark_unhealthy(&t.0, "tool-0001").unwrap());
        assert!(best_match(&load(&t.0).unwrap(), &kw).is_none());
        assert_eq!(load(&t.0).unwrap()[0].last_status, "retired by your feedback");
        assert!(!mark_unhealthy(&t.0, "nope").unwrap());
    }
}
