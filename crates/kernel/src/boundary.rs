//! The capability boundary — **the human's lever** (see `docs/boundaries.md`).
//!
//! The factory acts freely *within* this boundary and **can never widen it**: there
//! is deliberately no save/write function here. The boundary is a plain JSON policy
//! the human edits; the factory only ever reads it. A missing or unreadable policy is
//! treated as **fully closed** (fail-safe) — no outward capability by default.
//!
//! This makes Law III operational: a steward does not expand its own power. Reach is
//! enabled only by a human editing `boundary.json`.

use crate::store;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

/// The human-owned policy file (in the data dir; not source, not committed).
pub const BOUNDARY_FILE: &str = "boundary.json";

/// What the factory is permitted to reach. Fail-closed: anything unspecified is
/// denied (each field defaults to "off"/empty via `closed()`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Boundary {
    /// Human-readable phase label (e.g. "closed", "phase-1").
    pub phase: String,
    /// May the factory use the network at all?
    pub allow_network: bool,
    /// May the factory consult an LLM (the periphery seam)?
    pub allow_llm: bool,
    /// May the factory install/download tools?
    pub allow_tool_install: bool,
    /// May the factory **execute generated artifacts** (run code it produced)? A
    /// distinct, high-consequence gate — running generated code is its own risk.
    pub allow_execute: bool,
    /// May the factory execute **LLM-authored** artifacts (run *model-written* code)?
    /// A further, sharper gate than `allow_execute`: model-authored code with network
    /// reach is an exfiltration surface the in-process runner does not sandbox.
    pub allow_authored_execute: bool,
    /// May the familiar **watch through a camera** (capture frames)? The most invasive
    /// reach — an eye on a person, the sharpest Law III / HUMANITY test. *Discovery* of
    /// which cameras exist is perception (always allowed; the boundary governs reach, not
    /// perception); *watching* is gated here, fail-closed, and is only ever opened by an
    /// explicit human grant. Availability is not authorization — made literal for the eye.
    pub allow_camera: bool,
    /// May the familiar **federate with peer nodes over a mesh** (Tailscale)? Outward
    /// transmission — the exfiltration surface Law III guards, at node-to-node scale.
    /// *Discovering* that peers exist on the tailnet is perception; *exchanging briefs*
    /// (tools, patterns, and — only when separately opted-in — human data) is gated here,
    /// fail-closed, opened only by an explicit human grant. Enrolling a group credential
    /// and opening this flag is the human authorizing the group; the familiar never
    /// self-widens it. See `docs/mesh.md`.
    pub allow_mesh: bool,
    /// May the familiar **delegate a task to a multi-step agent** (the agentic seam)? A
    /// sharper reach than `allow_llm`: a one-shot consult returns text the core then weighs,
    /// whereas an agent runs a *loop* that proposes actions. Fail-closed, human-opened. Every
    /// action the agent proposes is still separately gated (and scoped to the agent's own
    /// capability profile), so opening this never widens what an agent may actually *do* — it
    /// only permits the delegated reasoning loop to run. See `docs/agents.md`.
    pub allow_agent: bool,
    /// Run executed artifacts under the resource sandbox (`ulimit`/wall-timeout)?
    /// Default **true** (safe). When the human sets it false, artifacts run without
    /// resource confinement — bound then by the constitution (the pre-execution review
    /// that refuses plainly harmful scripts) and a generous liveness timeout only, not by
    /// a jail. A deliberate, human-owned choice; see `docs/boundaries.md`.
    #[serde(default = "default_true")]
    pub sandbox_execution: bool,
    /// Path prefixes the factory may read.
    pub fs_read: Vec<String>,
    /// Path prefixes the factory may write.
    pub fs_write: Vec<String>,
}

fn default_true() -> bool {
    true
}

impl Default for Boundary {
    fn default() -> Self {
        Boundary::closed()
    }
}

impl Boundary {
    /// The fail-closed default: no outward capability whatsoever.
    pub fn closed() -> Self {
        Boundary {
            phase: "closed".to_string(),
            allow_network: false,
            allow_llm: false,
            allow_tool_install: false,
            allow_execute: false,
            allow_authored_execute: false,
            allow_camera: false,
            allow_mesh: false,
            allow_agent: false,
            sandbox_execution: true,
            fs_read: Vec::new(),
            fs_write: Vec::new(),
        }
    }

    /// True when no outward capability is granted at all.
    pub fn is_closed(&self) -> bool {
        !self.allow_network
            && !self.allow_llm
            && !self.allow_tool_install
            && !self.allow_execute
            && !self.allow_authored_execute
            && !self.allow_camera
            && !self.allow_mesh
            && !self.allow_agent
            && self.fs_read.is_empty()
            && self.fs_write.is_empty()
    }
}

/// A **capability scope** — the subset of reach a single agent specialist is trusted with.
/// It is a *request*, never a grant: the effective boundary an agent acts under is the
/// **intersection** of this scope with the human-owned boundary ([`scoped_boundary`]), so an
/// agent can never exceed either. A network specialist gets `{network, execute}`; a
/// control-systems specialist gets its own reach and cannot scan even under an open
/// `allow_network`. Fail-closed: [`CapabilityScope::none`] is all-off.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CapabilityScope {
    pub network: bool,
    pub execute: bool,
    pub authored_execute: bool,
    pub tool_install: bool,
    pub camera: bool,
    pub mesh: bool,
    pub fs_read: Vec<String>,
    pub fs_write: Vec<String>,
}

impl Default for CapabilityScope {
    fn default() -> Self {
        CapabilityScope::none()
    }
}

impl CapabilityScope {
    /// The fail-closed scope: no reach at all.
    pub fn none() -> Self {
        CapabilityScope {
            network: false,
            execute: false,
            authored_execute: false,
            tool_install: false,
            camera: false,
            mesh: false,
            fs_read: Vec::new(),
            fs_write: Vec::new(),
        }
    }
}

/// The effective boundary an agent runs under: the **intersection** of the human-owned
/// boundary and the agent's own [`CapabilityScope`] — least privilege. Each gate is granted
/// only if *both* allow it; each path is kept only if the agent requested it *and* the
/// boundary already covers it. `allow_llm` is preserved (an agent must be able to reason) and
/// `allow_agent` is dropped (an agent does not spawn sub-agents in this scope), so delegating
/// can never widen reach beyond what the human already opened.
pub fn scoped_boundary(b: &Boundary, s: &CapabilityScope) -> Boundary {
    Boundary {
        phase: format!("{}·scoped", b.phase),
        allow_network: b.allow_network && s.network,
        allow_llm: b.allow_llm,
        allow_tool_install: b.allow_tool_install && s.tool_install,
        allow_execute: b.allow_execute && s.execute,
        allow_authored_execute: b.allow_authored_execute && s.authored_execute,
        allow_camera: b.allow_camera && s.camera,
        allow_mesh: b.allow_mesh && s.mesh,
        allow_agent: false,
        sandbox_execution: b.sandbox_execution,
        fs_read: intersect_paths(&b.fs_read, &s.fs_read),
        fs_write: intersect_paths(&b.fs_write, &s.fs_write),
    }
}

/// Keep each *requested* path only when the *granted* set already covers it (a granted
/// prefix is an ancestor of it) — so a scope can narrow the boundary's paths but never add one.
fn intersect_paths(granted: &[String], requested: &[String]) -> Vec<String> {
    requested
        .iter()
        .filter(|r| {
            !r.is_empty()
                && granted
                    .iter()
                    .any(|g| !g.is_empty() && r.starts_with(g.as_str()))
        })
        .cloned()
        .collect()
}

/// Load the human-owned boundary policy. A missing file is **fully closed**
/// (fail-safe). The factory only reads; there is no write path — widening is a human
/// act (editing the file), never the factory's.
pub fn load(dir: &Path) -> io::Result<Boundary> {
    Ok(store::load_one::<Boundary>(dir, BOUNDARY_FILE)?.unwrap_or_else(Boundary::closed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct Temp(PathBuf);
    impl Temp {
        fn new(t: &str) -> Self {
            let p = std::env::temp_dir().join(format!("substrate_boundary_test_{t}"));
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
    fn default_is_closed() {
        assert!(Boundary::closed().is_closed());
        assert!(Boundary::default().is_closed());
    }

    #[test]
    fn missing_file_is_closed() {
        let t = Temp::new("missing");
        let b = load(&t.0).unwrap();
        assert!(b.is_closed());
        assert_eq!(b.phase, "closed");
    }

    #[test]
    fn reads_an_open_phase_1_policy() {
        let t = Temp::new("phase1");
        fs::write(
            t.0.join(BOUNDARY_FILE),
            r#"{"phase":"phase-1","allow_network":true,"allow_llm":true}"#,
        )
        .unwrap();
        let b = load(&t.0).unwrap();
        assert_eq!(b.phase, "phase-1");
        assert!(b.allow_network && b.allow_llm);
        assert!(!b.is_closed());
        // unspecified capabilities stay closed (fail-safe partial parse)
        assert!(!b.allow_tool_install);
        assert!(b.fs_write.is_empty());
    }

    #[test]
    fn sandbox_execution_defaults_on_when_unspecified() {
        let t = Temp::new("sandbox_default");
        // a policy that opens execution but says nothing about the sandbox
        fs::write(
            t.0.join(BOUNDARY_FILE),
            r#"{"phase":"phase-1","allow_execute":true}"#,
        )
        .unwrap();
        let b = load(&t.0).unwrap();
        assert!(b.allow_execute);
        assert!(
            b.sandbox_execution,
            "the safe default is sandboxed; turning it off must be an explicit human choice"
        );
    }

    #[test]
    fn mesh_defaults_closed_and_counts_as_outward_capability() {
        // Fail-closed by default, and old policy files that predate the flag stay closed.
        assert!(!Boundary::closed().allow_mesh);
        let t = Temp::new("mesh_absent");
        fs::write(
            t.0.join(BOUNDARY_FILE),
            r#"{"phase":"phase-1","allow_llm":true}"#,
        )
        .unwrap();
        assert!(!load(&t.0).unwrap().allow_mesh, "unspecified mesh stays off");
        // Opening only the mesh flag is enough to make the boundary no longer closed.
        let mut b = Boundary::closed();
        b.allow_mesh = true;
        assert!(!b.is_closed());
    }

    #[test]
    fn agent_gate_defaults_closed() {
        assert!(!Boundary::closed().allow_agent);
        let mut b = Boundary::closed();
        b.allow_agent = true;
        assert!(!b.is_closed());
    }

    #[test]
    fn scoped_boundary_is_a_true_intersection() {
        // A generously-open boundary.
        let mut b = Boundary::closed();
        b.allow_network = true;
        b.allow_execute = true;
        b.allow_camera = true;
        b.fs_read = vec!["/Users/ian/".into()];
        // A network specialist's scope: network + execute, one narrower read path. No camera.
        let mut scope = CapabilityScope::none();
        scope.network = true;
        scope.execute = true;
        scope.fs_read = vec!["/Users/ian/Development/".into()]; // within the grant
        scope.fs_write = vec!["/etc/".into()]; // NOT granted → dropped

        let eff = scoped_boundary(&b, &scope);
        assert!(eff.allow_network && eff.allow_execute);
        assert!(!eff.allow_camera, "scope withholds camera even though boundary allows it");
        assert!(!eff.allow_agent, "a scoped agent cannot itself spawn agents");
        assert_eq!(eff.fs_read, vec!["/Users/ian/Development/".to_string()]);
        assert!(eff.fs_write.is_empty(), "a path the boundary never granted is not added");

        // And a scope can't exceed a closed boundary: everything off in → everything off out.
        let eff2 = scoped_boundary(&Boundary::closed(), &scope);
        assert!(eff2.is_closed());
    }

    #[test]
    fn malformed_policy_is_an_error_not_silently_open() {
        let t = Temp::new("malformed");
        fs::write(t.0.join(BOUNDARY_FILE), "{ not json").unwrap();
        assert!(load(&t.0).is_err());
    }
}
