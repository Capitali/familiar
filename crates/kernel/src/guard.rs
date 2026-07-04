//! The obedience guard — **Law III made operational**.
//!
//! Law III: *service must not become obedience.* Before any consequential action, the
//! factory asks not "was I told to?" nor "can I?" but **"Am I authorized — by my
//! constitution, by the served, and by the surrounding environment — to do this?"** The
//! guard answers with one of three outcomes — **allow**, **seek consent**, or
//! **refuse** — and a recorded rationale.
//!
//! It is the **enforcer of the capability boundary** ([`crate::boundary`]): any action
//! outside the human-owned boundary is refused, regardless of who or what asked. The
//! boundary is fail-closed, so by default every outward action is refused until a human
//! widens it. Two doctrines from [SOUL.md](../../../docs/SOUL.md) govern it:
//!
//! - **Availability is not authorization.** Technical reach (a readable path, a
//!   reachable host, a runnable command, a present token) is never permission — the
//!   boundary decides, not the capability. Enforced here: an out-of-scope `ReadFile` /
//!   `WriteFile` is refused though the bytes are reachable.
//! - **Permission does not compose.** One granted capability is not a key to another's
//!   lock. This guard enforces the *per-capability gate* and *path scope*; it does **not
//!   yet** confine the data-flow *within* a granted capability — e.g. it cannot stop an
//!   already-permitted `ExecuteArtifact` from reading an unrelated file, or an
//!   already-permitted `Network`/`Llm` call from carrying private data outward. Those
//!   remain binding constitutional restraints whose *mechanical* enforcement (an
//!   fs-jailed runner, egress/secret redaction) is tracked as hardening in
//!   [boundaries.md](../../../docs/boundaries.md). The gap is named, not hidden.

use crate::boundary::Boundary;
use serde::{Deserialize, Serialize};

/// The kind of action being weighed. Internal kinds touch only the factory's own
/// state; outward kinds reach the host or network and are boundary-gated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    /// Record an observation (internal; the only truth).
    Observe,
    /// Emit a candidate artifact into the factory's own store (internal).
    EmitArtifact,
    /// Read a file from the host.
    ReadFile,
    /// Write a file to the host.
    WriteFile,
    /// Use the network.
    Network,
    /// Consult an LLM (the periphery seam).
    Llm,
    /// Install or download a tool.
    InstallTool,
    /// Execute a generated artifact (run code the factory produced).
    ExecuteArtifact,
    /// Watch through a camera (capture frames). The most invasive reach; discovering
    /// *which* cameras exist is perception and not weighed here — only *watching* is.
    Camera,
    /// Federate with peer nodes over the mesh (announce presence, exchange briefs, pull
    /// tools/patterns). Outward transmission at node scale; *discovering* that peers exist
    /// on the tailnet is perception and not weighed here — only *exchanging* is. An
    /// identity-bearing share carries `affects_person = true`, routing it through consent.
    Mesh,
    /// Delegate a task to a multi-step agent (the agentic seam). Gated by `allow_agent`; a
    /// sharper reach than a one-shot `Llm` consult because the agent runs a loop. Every action
    /// the agent then proposes is itself weighed here against the agent's *scoped* boundary.
    Agent,
}

/// The guard's outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Proceed.
    Allow,
    /// Permissible, but high-consequence — a human should consent first.
    SeekConsent,
    /// Do not proceed.
    Refuse,
}

/// The categorized reason behind a [`Decision`] — the guard's answer to
/// *"Am I authorized, by my constitution, by the served, and by the surrounding
/// environment, to do this?"* Each reason implies its decision ([`Reason::decision`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// **Refuse** — the action needs a capability the constitution / human-owned
    /// boundary does not grant. Availability is not authorization.
    ViolatesConstitutionalBoundary,
    /// **Refuse** — a boundary belonging to the surrounding environment (another
    /// system's or person's space, an explicit fence) was discovered. The environment
    /// withholds authorization even where reach is technically available.
    ExternalBoundaryDiscovered,
    /// **SeekConsent** — within reach, but the human-owned scope (or the consequence of
    /// acting in it) is ambiguous; confirm with the owner.
    AmbiguousHumanOwnedScope,
    /// **SeekConsent** — a local observation/read that may expose sensitive data;
    /// consent-gated even when technically in scope.
    PotentiallySensitiveLocalObservation,
    /// **Allow** — authorized on every count: constitution, policy, environment, and consent.
    WithinConstitutionPolicyEnvironmentAndConsent,
}

impl Reason {
    /// The decision this reason entails.
    pub fn decision(self) -> Decision {
        use Reason::*;
        match self {
            ViolatesConstitutionalBoundary | ExternalBoundaryDiscovered => Decision::Refuse,
            AmbiguousHumanOwnedScope | PotentiallySensitiveLocalObservation => {
                Decision::SeekConsent
            }
            WithinConstitutionPolicyEnvironmentAndConsent => Decision::Allow,
        }
    }
}

/// A proposed action, described enough for the guard to weigh it.
#[derive(Debug, Clone)]
pub struct Action {
    pub kind: ActionKind,
    /// What it acts on (a path, host, tool name, …) — used for scope checks.
    pub target: String,
    /// Can it be undone?
    pub reversible: bool,
    /// Does it touch a person's agency/wellbeing directly?
    pub affects_person: bool,
    /// The surrounding **environment** has signalled a boundary here — another system's
    /// or person's space, an explicit fence (a `Disallow`, a "private", a permission a
    /// human did not extend). Availability never overrides it; the guard refuses.
    pub external_boundary: bool,
    /// This local observation/read may expose **sensitive** data, so it is consent-gated
    /// even when technically within the read scope (a token, a key, a private document).
    pub sensitive: bool,
}

impl Action {
    /// A minimal action; refine `reversible`/`affects_person`/`external_boundary`/
    /// `sensitive` as the caller learns more about it.
    pub fn new(kind: ActionKind, target: impl Into<String>) -> Self {
        Action {
            kind,
            target: target.into(),
            reversible: true,
            affects_person: false,
            external_boundary: false,
            sensitive: false,
        }
    }
}

/// The guard's verdict: a decision, the categorized [`Reason`], and the human-readable
/// rationale it is recorded by.
#[derive(Debug, Clone)]
pub struct Verdict {
    pub decision: Decision,
    pub reason: Reason,
    pub rationale: String,
}

fn verdict(reason: Reason, rationale: String) -> Verdict {
    Verdict {
        decision: reason.decision(),
        reason,
        rationale,
    }
}

/// A target's standing against a set of granted path prefixes — three-valued, because
/// "not granted" and "broader than what was granted" are different questions.
enum Scope {
    /// Covered by a granted prefix.
    In,
    /// An *ancestor* of a granted path — the caller asked broader than the grant. The
    /// human owns that scope; the guard does not silently widen it, but it is a
    /// seek-consent question, not an outright refusal.
    Ambiguous,
    /// Unrelated to any grant.
    Out,
}

fn scope_of(target: &str, scopes: &[String]) -> Scope {
    if scopes
        .iter()
        .any(|p| !p.is_empty() && target.starts_with(p.as_str()))
    {
        Scope::In
    } else if !target.is_empty()
        && scopes
            .iter()
            .any(|p| !p.is_empty() && p.as_str().starts_with(target))
    {
        Scope::Ambiguous
    } else {
        Scope::Out
    }
}

/// Weigh an action against authorization's three sources — *constitution, the served,
/// and the surrounding environment* — never against mere technical availability.
///
/// 1. **Constitutional boundary (fail-closed):** outside the human-owned grant →
///    [`Reason::ViolatesConstitutionalBoundary`] (Refuse). Availability is not permission.
/// 2. **External boundary:** the environment signalled a fence here →
///    [`Reason::ExternalBoundaryDiscovered`] (Refuse).
/// 3. **Ambiguous scope:** broader than the grant → [`Reason::AmbiguousHumanOwnedScope`]
///    (SeekConsent).
/// 4. **Sensitive local observation:** may expose private data →
///    [`Reason::PotentiallySensitiveLocalObservation`] (SeekConsent).
/// 5. **High consequence** (irreversible / installs / touches a person): in scope but
///    needs the owner's confirmation → [`Reason::AmbiguousHumanOwnedScope`] (SeekConsent).
/// 6. Otherwise → [`Reason::WithinConstitutionPolicyEnvironmentAndConsent`] (Allow).
pub fn evaluate(action: &Action, boundary: &Boundary) -> Verdict {
    use ActionKind::*;

    // 1. The human-owned constitutional boundary, fail-closed. Internal actions are
    //    within reach by definition; outward capabilities are gated; paths are scoped.
    let scope = match action.kind {
        Observe | EmitArtifact => Scope::In,
        ReadFile => scope_of(&action.target, &boundary.fs_read),
        WriteFile => scope_of(&action.target, &boundary.fs_write),
        Network => bool_scope(boundary.allow_network),
        Llm => bool_scope(boundary.allow_llm),
        InstallTool => bool_scope(boundary.allow_tool_install),
        ExecuteArtifact => bool_scope(boundary.allow_execute),
        Camera => bool_scope(boundary.allow_camera),
        Mesh => bool_scope(boundary.allow_mesh),
        Agent => bool_scope(boundary.allow_agent),
    };

    if matches!(scope, Scope::Out) {
        return verdict(
            Reason::ViolatesConstitutionalBoundary,
            format!(
                "refused — {:?} on '{}' is outside the human-owned boundary; availability \
                 is not authorization. Only a human widens the boundary (docs/boundaries.md).",
                action.kind, action.target
            ),
        );
    }

    // 2. The surrounding environment's own boundary overrides technical reach.
    if action.external_boundary {
        return verdict(
            Reason::ExternalBoundaryDiscovered,
            format!(
                "refused — an external boundary was discovered at '{}'; a granted \
                 capability is not a key to another's lock (docs/SOUL.md).",
                action.target
            ),
        );
    }

    // 3. Broader than what the human granted — confirm, do not silently widen.
    if matches!(scope, Scope::Ambiguous) {
        return verdict(
            Reason::AmbiguousHumanOwnedScope,
            format!(
                "seek consent — '{}' is broader than the granted scope; confirm with the owner.",
                action.target
            ),
        );
    }

    // 4. A potentially sensitive local observation is consent-gated even in scope.
    if action.sensitive && matches!(action.kind, Observe | ReadFile) {
        return verdict(
            Reason::PotentiallySensitiveLocalObservation,
            format!(
                "seek consent — observing '{}' may expose sensitive data.",
                action.target
            ),
        );
    }

    // 5. In scope, but high-consequence: the grant covers the capability, not necessarily
    //    this act — an ambiguity only the owner resolves.
    let mut flags = Vec::new();
    if action.affects_person {
        flags.push("touches a person's agency");
    }
    if !action.reversible {
        flags.push("is not reversible");
    }
    if action.kind == InstallTool {
        flags.push("installs software");
    }
    if !flags.is_empty() {
        return verdict(
            Reason::AmbiguousHumanOwnedScope,
            format!(
                "seek consent — within the boundary but high-consequence ({}).",
                flags.join(", ")
            ),
        );
    }

    // 6. Authorized on every count.
    verdict(
        Reason::WithinConstitutionPolicyEnvironmentAndConsent,
        "allowed — within constitution, policy, environment, and consent".to_string(),
    )
}

fn bool_scope(granted: bool) -> Scope {
    if granted {
        Scope::In
    } else {
        Scope::Out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_llm() -> Boundary {
        Boundary {
            phase: "phase-1".into(),
            allow_network: true,
            allow_llm: true,
            allow_tool_install: false,
            allow_execute: false,
            allow_authored_execute: false,
            allow_camera: false,
            allow_mesh: false,
            allow_agent: false,
            sandbox_execution: true,
            fs_read: vec!["/Users/ian/".into()],
            fs_write: vec!["/Users/ian/Development/familiar/familiar_data/".into()],
        }
    }

    #[test]
    fn closed_boundary_refuses_all_outward_actions() {
        let b = Boundary::closed();
        for kind in [
            ActionKind::Network,
            ActionKind::Llm,
            ActionKind::InstallTool,
            ActionKind::ExecuteArtifact,
            ActionKind::Camera,
            ActionKind::Mesh,
            ActionKind::Agent,
            ActionKind::ReadFile,
            ActionKind::WriteFile,
        ] {
            let v = evaluate(&Action::new(kind, "x"), &b);
            assert_eq!(
                v.decision,
                Decision::Refuse,
                "{kind:?} should be refused when closed"
            );
        }
    }

    #[test]
    fn internal_actions_always_allowed() {
        let b = Boundary::closed();
        assert_eq!(
            evaluate(&Action::new(ActionKind::Observe, ""), &b).decision,
            Decision::Allow
        );
        assert_eq!(
            evaluate(&Action::new(ActionKind::EmitArtifact, ""), &b).decision,
            Decision::Allow
        );
    }

    #[test]
    fn llm_allowed_when_boundary_opens_it() {
        let v = evaluate(&Action::new(ActionKind::Llm, "provider"), &open_llm());
        assert_eq!(v.decision, Decision::Allow);
        // but network still refused if its flag were off — here network is on, so:
        assert_eq!(
            evaluate(&Action::new(ActionKind::Network, "host"), &open_llm()).decision,
            Decision::Allow
        );
    }

    #[test]
    fn install_seeks_consent_even_when_permitted() {
        let mut b = open_llm();
        b.allow_tool_install = true;
        let v = evaluate(&Action::new(ActionKind::InstallTool, "ripgrep"), &b);
        assert_eq!(v.decision, Decision::SeekConsent);
    }

    #[test]
    fn write_scope_enforced_and_consequence_weighed() {
        let b = open_llm();
        // in scope, reversible -> allow
        let inside = Action::new(
            ActionKind::WriteFile,
            "/Users/ian/Development/familiar/familiar_data/x",
        );
        assert_eq!(evaluate(&inside, &b).decision, Decision::Allow);
        // out of scope -> refuse
        let outside = Action::new(ActionKind::WriteFile, "/etc/passwd");
        assert_eq!(evaluate(&outside, &b).decision, Decision::Refuse);
        // in scope but irreversible -> seek consent
        let mut irr = inside.clone();
        irr.reversible = false;
        assert_eq!(evaluate(&irr, &b).decision, Decision::SeekConsent);
    }

    #[test]
    fn affecting_a_person_seeks_consent() {
        let b = open_llm();
        let mut a = Action::new(ActionKind::Observe, "betty");
        a.affects_person = true;
        assert_eq!(evaluate(&a, &b).decision, Decision::SeekConsent);
    }

    #[test]
    fn watching_a_camera_is_refused_until_a_human_opens_it() {
        // a camera being present is not permission to watch — fail-closed
        let v = evaluate(
            &Action::new(ActionKind::Camera, "FaceTime HD Camera"),
            &open_llm(),
        );
        assert_eq!(v.decision, Decision::Refuse);
        assert_eq!(v.reason, Reason::ViolatesConstitutionalBoundary);
        // only an explicit human grant opens the eye
        let mut b = open_llm();
        b.allow_camera = true;
        assert_eq!(
            evaluate(&Action::new(ActionKind::Camera, "FaceTime HD Camera"), &b).decision,
            Decision::Allow
        );
    }

    #[test]
    fn mesh_is_refused_until_opened_then_consent_gates_identity_shares() {
        // A reachable tailnet is not permission to federate — fail-closed.
        let v = evaluate(&Action::new(ActionKind::Mesh, "group:river"), &open_llm());
        assert_eq!(v.decision, Decision::Refuse);
        assert_eq!(v.reason, Reason::ViolatesConstitutionalBoundary);

        // Human opens the group: a plain tool/pattern brief is allowed.
        let mut b = open_llm();
        b.allow_mesh = true;
        assert_eq!(
            evaluate(&Action::new(ActionKind::Mesh, "brief"), &b).decision,
            Decision::Allow
        );

        // But an identity-bearing share (affects_person) still seeks consent, even with
        // the mesh open — matching the per-human opt-in model.
        let mut pii = Action::new(ActionKind::Mesh, "identity:betty");
        pii.affects_person = true;
        assert_eq!(evaluate(&pii, &b).decision, Decision::SeekConsent);
    }

    #[test]
    fn delegating_to_an_agent_is_refused_until_opened() {
        // An agent is reach the human must open — closed by default even with the LLM open.
        let v = evaluate(&Action::new(ActionKind::Agent, "network-scan"), &open_llm());
        assert_eq!(v.decision, Decision::Refuse);
        assert_eq!(v.reason, Reason::ViolatesConstitutionalBoundary);
        let mut b = open_llm();
        b.allow_agent = true;
        assert_eq!(
            evaluate(&Action::new(ActionKind::Agent, "network-scan"), &b).decision,
            Decision::Allow
        );
    }

    #[test]
    fn out_of_scope_names_the_constitutional_boundary() {
        // Availability is not authorization: /etc/passwd is readable on the host, but
        // outside the granted read scope, so it is refused on constitutional grounds.
        let v = evaluate(
            &Action::new(ActionKind::ReadFile, "/etc/passwd"),
            &open_llm(),
        );
        assert_eq!(v.decision, Decision::Refuse);
        assert_eq!(v.reason, Reason::ViolatesConstitutionalBoundary);
    }

    #[test]
    fn external_boundary_refuses_even_when_in_scope() {
        // In the read scope and the capability is granted, yet the environment fenced it:
        // a granted capability is not a key to another's lock.
        let b = open_llm();
        let mut a = Action::new(ActionKind::ReadFile, "/Users/ian/notes.txt");
        a.external_boundary = true;
        let v = evaluate(&a, &b);
        assert_eq!(v.decision, Decision::Refuse);
        assert_eq!(v.reason, Reason::ExternalBoundaryDiscovered);
    }

    #[test]
    fn asking_broader_than_the_grant_seeks_consent() {
        // Granted /Users/ian/; asking about its ancestor /Users/ is broader than the
        // grant — ambiguous human-owned scope, not an outright refusal.
        let v = evaluate(&Action::new(ActionKind::ReadFile, "/Users/"), &open_llm());
        assert_eq!(v.decision, Decision::SeekConsent);
        assert_eq!(v.reason, Reason::AmbiguousHumanOwnedScope);
    }

    #[test]
    fn sensitive_local_observation_seeks_consent() {
        // Technically inside the read scope, but flagged sensitive (a key) — consent first.
        let b = open_llm();
        let mut a = Action::new(ActionKind::ReadFile, "/Users/ian/.ssh/id_ed25519");
        a.sensitive = true;
        let v = evaluate(&a, &b);
        assert_eq!(v.decision, Decision::SeekConsent);
        assert_eq!(v.reason, Reason::PotentiallySensitiveLocalObservation);
    }

    #[test]
    fn fully_authorized_action_names_all_four_sources() {
        let v = evaluate(
            &Action::new(ActionKind::ReadFile, "/Users/ian/notes.txt"),
            &open_llm(),
        );
        assert_eq!(v.decision, Decision::Allow);
        assert_eq!(
            v.reason,
            Reason::WithinConstitutionPolicyEnvironmentAndConsent
        );
    }
}
