//! Goal — a unit of *intended work* the mesh can own and burn down autonomously.
//!
//! A [`Thread`](crate::thread) is the familiar reasoning about what it observes; a **goal** is a
//! stated intention to *accomplish* something — seeded by the human (the roadmap made mesh-native)
//! or proposed by the familiar. Goals replicate across the mesh like observations, so every node
//! sees the same roadmap; a node whose **capabilities** satisfy a goal's `needs` (and whose boundary
//! permits the work) claims it and drives it through the agentic loop, then federates what it built
//! and learned. The human governs: high-consequence steps (deploy) never run without an approval.
//!
//! This is the injection channel that turns a to-do list into work the mesh can act on — the pursue
//! → agent → cultivate machinery already exists; a goal is what points it at a chosen end.

use crate::store;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

pub const GOALS_FILE: &str = "goals.jsonl";

/// A capability whose name begins with this prefix is **high-consequence** — building/testing may
/// run autonomously, but actually *doing* it (installing to a device, publishing) is a human act.
/// A goal that needs one is claimed by the capable node but parked at [`Status::AwaitingHuman`]
/// rather than executed, and surfaced for approval. Deploy is the canonical case.
pub const GATED_CAPABILITY_PREFIX: &str = "deploy";

/// Where a goal is in its life. Deliberately small and legible — the worldview renders these
/// verbatim so the roadmap's state reads the same on every node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Seeded, not yet claimed by any node.
    Proposed,
    /// A node with the needed capabilities has taken ownership.
    Claimed,
    /// The owner is actively driving it through the agentic loop.
    InProgress,
    /// The autonomous work is done; a human-gated step (deploy) waits for approval.
    AwaitingHuman,
    /// Accomplished — what it produced is federated to the mesh.
    Done,
    /// Abandoned after the work could not be completed (the reason is in `notes`).
    Failed,
    /// No node on the mesh has the capabilities it needs — waiting for one that does.
    Blocked,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Proposed => "proposed",
            Status::Claimed => "claimed",
            Status::InProgress => "in_progress",
            Status::AwaitingHuman => "awaiting_human",
            Status::Done => "done",
            Status::Failed => "failed",
            Status::Blocked => "blocked",
        }
    }
    /// True once no node should still be trying to claim or run it (a terminal or human-owned state).
    pub fn settled(self) -> bool {
        matches!(self, Status::Done | Status::Failed | Status::AwaitingHuman)
    }
}

/// One unit of intended work, shared across the mesh.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    /// Human-readable intention — what "done" means. Drives the agentic loop's task prompt.
    pub description: String,
    /// Capabilities a node must advertise to claim this (e.g. `build-rust`, `build-apple`,
    /// `deploy-apple`). Empty = any node with `execute`+`agent` may take it. See
    /// [`crate::capabilities`].
    #[serde(default)]
    pub needs: Vec<String>,
    pub status: Status,
    /// node_id of the node that claimed it; empty while unclaimed. The mesh dedups ownership on this.
    #[serde(default)]
    pub owner_node: String,
    /// Who seeded it: a human handle ("ian"), "familiar" (self-proposed), or "mesh:<node>".
    #[serde(default)]
    pub origin: String,
    /// Artifacts/tools it produced (tool ids or names), for the audit trail and federation.
    #[serde(default)]
    pub produced: String,
    /// Progress + learnings — appended as the goal advances, so the knowledge travels with it.
    #[serde(default)]
    pub notes: String,
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

impl Goal {
    /// A freshly-seeded goal: proposed, unclaimed, no owner.
    pub fn seed(id: &str, description: &str, needs: Vec<String>, origin: &str, now: i64) -> Self {
        Goal {
            id: id.to_string(),
            description: description.to_string(),
            needs,
            status: Status::Proposed,
            owner_node: String::new(),
            origin: origin.to_string(),
            produced: String::new(),
            notes: String::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Does this goal require a high-consequence, human-gated capability (deploy)? Such a goal's
    /// autonomous work may run, but the gated act itself waits for a human.
    pub fn is_human_gated(&self) -> bool {
        self.needs.iter().any(|n| n.starts_with(GATED_CAPABILITY_PREFIX))
    }

    /// Can a node advertising `caps` take this goal on — does it satisfy every `need`?
    pub fn satisfied_by(&self, caps: &[String]) -> bool {
        self.needs.iter().all(|n| caps.iter().any(|c| c == n))
    }
}

pub fn append(dir: &Path, g: &Goal) -> io::Result<()> {
    store::append(dir, GOALS_FILE, g)
}

pub fn load(dir: &Path) -> io::Result<Vec<Goal>> {
    store::load(dir, GOALS_FILE)
}

pub fn load_by_id(dir: &Path, id: &str) -> io::Result<Option<Goal>> {
    store::load_by_id(dir, GOALS_FILE, id)
}

/// Replace a goal by id (a single indexed update, not a whole-file rewrite). Returns whether it matched.
pub fn update(dir: &Path, g: &Goal) -> io::Result<bool> {
    store::update_by_id(dir, GOALS_FILE, &g.id, g)
}

/// Claim a proposed goal for `node_id`: set the owner and move it to `Claimed`, but only if it is
/// still unclaimed (idempotent against a racing peer — the first claim we see wins). Returns whether
/// this call took ownership.
pub fn claim(dir: &Path, id: &str, node_id: &str, now: i64) -> io::Result<bool> {
    let Some(mut g) = load_by_id(dir, id)? else {
        return Ok(false);
    };
    if g.status != Status::Proposed || !g.owner_node.is_empty() {
        return Ok(false); // already claimed (by us or a peer) — don't steal it
    }
    g.owner_node = node_id.to_string();
    g.status = Status::Claimed;
    g.updated_at = now;
    update(dir, &g)?;
    Ok(true)
}

/// Advance a goal's status (and stamp it), optionally appending a progress note. Returns whether the
/// id matched.
pub fn advance(dir: &Path, id: &str, status: Status, note: &str, now: i64) -> io::Result<bool> {
    let Some(mut g) = load_by_id(dir, id)? else {
        return Ok(false);
    };
    g.status = status;
    g.updated_at = now;
    if !note.is_empty() {
        if !g.notes.is_empty() {
            g.notes.push_str("; ");
        }
        g.notes.push_str(note);
    }
    update(dir, &g)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("kernel_goal_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn seed_claim_and_advance_round_trip() {
        let dir = tmp("lifecycle");
        let g = Goal::seed(
            "goal-0001",
            "iPhone reports camera availability as observations",
            vec!["build-apple".into(), "deploy-apple".into()],
            "ian",
            100,
        );
        append(&dir, &g).unwrap();
        assert_eq!(load(&dir).unwrap(), vec![g.clone()]);
        assert!(g.is_human_gated(), "a deploy-needing goal is human-gated");

        // Only a node with BOTH capabilities can take it.
        assert!(!g.satisfied_by(&["build-apple".into()]));
        assert!(g.satisfied_by(&["build-apple".into(), "deploy-apple".into(), "llm".into()]));

        // First claim wins; a second is refused.
        assert!(claim(&dir, "goal-0001", "node-mac", 200).unwrap());
        assert!(!claim(&dir, "goal-0001", "node-vm", 201).unwrap(), "already claimed");
        let after = load_by_id(&dir, "goal-0001").unwrap().unwrap();
        assert_eq!(after.owner_node, "node-mac");
        assert_eq!(after.status, Status::Claimed);

        advance(&dir, "goal-0001", Status::AwaitingHuman, "built + tested; deploy awaits approval", 300).unwrap();
        let done = load_by_id(&dir, "goal-0001").unwrap().unwrap();
        assert_eq!(done.status, Status::AwaitingHuman);
        assert!(done.notes.contains("deploy awaits approval"));
        assert!(done.status.settled());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_goal_with_no_needs_is_satisfied_by_anyone() {
        let g = Goal::seed("g", "tidy the workspace", vec![], "familiar", 1);
        assert!(g.satisfied_by(&[]));
        assert!(!g.is_human_gated());
    }
}
