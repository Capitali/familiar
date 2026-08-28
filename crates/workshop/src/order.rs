//! Work orders and the generation contract.
//!
//! An order is the immutable statement of what the factory was asked to
//! manufacture, by whom, under which gates, proven how. It is written once
//! when opened; everything that happens afterwards is ledger events. The
//! generation contract is the typed boundary with providers: whatever the
//! adapter (local reasoner, envoy, device consult, cloud) actually did, the
//! only things it can hand back are a [`GenerationOutcome::Candidate`] the
//! workshop then validates, or a [`GenerationOutcome::Refused`] — refusal is
//! a terminally valid factory result, not an error.

use serde::{Deserialize, Serialize};

use crate::manifest::Manifest;

/// A sourced research entry. A claim without a source is a TODO, so research
/// carries where it came from and the digest of the exact text handed over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchEntry {
    pub title: String,
    /// Where the text came from — a path, URL, or record id. Provenance, not
    /// authority: the oracle decides, research only informs.
    pub source: String,
    /// sha256 of the exact bytes handed to generation.
    pub digest: String,
}

/// The oracle rungs an order climbs, in the fixed factory order. Which rungs
/// an order requires is part of the order itself (a sensor with nothing to
/// actuate has no act rung); the ledger enforces the declared plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleRung {
    /// Generated tests against generated code, offline, no radio.
    Bench,
    /// Against the live device, observation only. Per ADR-0032 a connected
    /// state query still transmits, so this rung rides `allow_actuate` —
    /// "read" names the rung's semantic result, not its physical innocence.
    Read,
    /// Closed-loop: command, then read back what the device echoes.
    Act,
    /// A human's eyes, for properties the device never echoes (SP548E
    /// colour). Evidence, not consent; answered only through the signed
    /// console seam.
    Witness,
}

/// The toolchain a candidate is allowed to assume. Provisioned by the
/// factory manager under the install/network gates — generated code never
/// runs its own installer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Toolchain {
    /// e.g. "python3.13" — the interpreter the runner will invoke.
    pub interpreter: String,
    /// sha256 of the dependency lock (empty string = stdlib only).
    pub lock_digest: String,
}

/// The immutable production order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkOrder {
    /// Stable id, e.g. "order-0001-motorlights".
    pub id: String,
    /// Who asked — a human name or the familiar's own loop, named honestly.
    pub requester: String,
    /// The goal in one sentence.
    pub goal: String,
    /// The requester's original wording, verbatim.
    pub wording: String,
    /// The declared device identity this order may touch — the match rule the
    /// trusted broker enforces (never a per-host CoreBluetooth UUID).
    pub subject: String,
    /// The capability surface being manufactured, as act labels.
    pub capability_surface: Vec<String>,
    pub research: Vec<ResearchEntry>,
    /// Boundary gates the order's live rungs require, by field name.
    pub required_gates: Vec<String>,
    /// Which rungs this order must climb, ascending, no duplicates.
    pub oracle_plan: Vec<OracleRung>,
    pub toolchain: Toolchain,
    /// Name of the containment profile the runner must enforce.
    pub containment: String,
}

/// Why generation refused. Mirrors the dialogue's contract: a refusal names
/// what was unmet and points at evidence, so nothing gets rebuilt from
/// scratch just because nothing remembers the last attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    pub code: String,
    pub rationale: String,
    pub unmet_requirements: Vec<String>,
    /// Digest of the evidence blob backing the refusal, if any.
    pub evidence: Option<String>,
}

/// What a generation adapter may return. Nothing else crosses the boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GenerationOutcome {
    Candidate {
        manifest: Manifest,
        /// Paths (into the manifest) the runner may execute.
        entrypoints: Vec<String>,
        /// Paths (into the manifest) that are the bench oracle.
        self_tests: Vec<String>,
        /// The effects the candidate claims to have — checked against the
        /// order's capability surface, never trusted past it.
        declared_effects: Vec<String>,
        toolchain_lock: String,
        capability_surface: Vec<String>,
    },
    Refused(Refusal),
}

/// Order-level validation errors (the ledger has its own).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderError {
    EmptyId,
    EmptyGoal,
    EmptySubject,
    NoCapability,
    EmptyOraclePlan,
    /// The plan must be strictly ascending — each rung at most once, in
    /// factory order.
    UnorderedOraclePlan,
    /// A candidate's entrypoint or self-test names a path outside its own
    /// manifest.
    DanglingPath(String),
    /// A candidate claims effects beyond the ordered capability surface.
    EffectBeyondOrder(String),
    ManifestInvalid(String),
}

impl std::fmt::Display for OrderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderError::EmptyId => write!(f, "order id is empty"),
            OrderError::EmptyGoal => write!(f, "order goal is empty"),
            OrderError::EmptySubject => write!(f, "order subject is empty"),
            OrderError::NoCapability => write!(f, "order names no capability surface"),
            OrderError::EmptyOraclePlan => write!(f, "order has no oracle plan"),
            OrderError::UnorderedOraclePlan => {
                write!(
                    f,
                    "oracle plan must be strictly ascending bench→read→act→witness"
                )
            }
            OrderError::DanglingPath(p) => write!(f, "path not in manifest: {p}"),
            OrderError::EffectBeyondOrder(e) => {
                write!(f, "declared effect beyond ordered surface: {e}")
            }
            OrderError::ManifestInvalid(e) => write!(f, "manifest invalid: {e}"),
        }
    }
}

/// Validate an order at opening. Fail closed; an invalid order never enters
/// the ledger.
pub fn validate_order(o: &WorkOrder) -> Result<(), OrderError> {
    if o.id.trim().is_empty() {
        return Err(OrderError::EmptyId);
    }
    if o.goal.trim().is_empty() {
        return Err(OrderError::EmptyGoal);
    }
    if o.subject.trim().is_empty() {
        return Err(OrderError::EmptySubject);
    }
    if o.capability_surface.is_empty() {
        return Err(OrderError::NoCapability);
    }
    if o.oracle_plan.is_empty() {
        return Err(OrderError::EmptyOraclePlan);
    }
    if o.oracle_plan.windows(2).any(|w| w[0] >= w[1]) {
        return Err(OrderError::UnorderedOraclePlan);
    }
    Ok(())
}

/// Validate a candidate against its order: manifest well-formed, every
/// entrypoint/self-test inside the manifest, no effect beyond the ordered
/// surface. A refusal is always valid — it is the contract's honest exit.
pub fn validate_outcome(order: &WorkOrder, outcome: &GenerationOutcome) -> Result<(), OrderError> {
    let (manifest, entrypoints, self_tests, effects) = match outcome {
        GenerationOutcome::Refused(_) => return Ok(()),
        GenerationOutcome::Candidate {
            manifest,
            entrypoints,
            self_tests,
            declared_effects,
            ..
        } => (manifest, entrypoints, self_tests, declared_effects),
    };
    crate::manifest::validate(manifest).map_err(|e| OrderError::ManifestInvalid(e.to_string()))?;
    let known: std::collections::BTreeSet<&str> =
        manifest.files.iter().map(|f| f.path.as_str()).collect();
    for p in entrypoints.iter().chain(self_tests.iter()) {
        if !known.contains(p.as_str()) {
            return Err(OrderError::DanglingPath(p.clone()));
        }
    }
    for e in effects {
        if !order.capability_surface.contains(e) {
            return Err(OrderError::EffectBeyondOrder(e.clone()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{digest_bytes, FileEntry, FileRole};

    pub(crate) fn order_one() -> WorkOrder {
        WorkOrder {
            id: "order-0001-motorlights".into(),
            requester: "ian".into(),
            goal: "manufacture a driver for the declared SP548E LED controller".into(),
            wording: "autonomously discover, write, execute, and automate".into(),
            subject: "ble:mfr=0x5053,wifi_mac=ba:16:b5:fe:19:82".into(),
            capability_surface: vec![
                "state".into(),
                "on".into(),
                "off".into(),
                "brightness".into(),
                "color".into(),
            ],
            research: vec![ResearchEntry {
                title: "household SP548E protocol notes".into(),
                source: "CLAUDE.md river.io network section, verified 2026-07-28".into(),
                digest: digest_bytes(b"notes"),
            }],
            required_gates: vec!["allow_execute".into(), "allow_actuate".into()],
            oracle_plan: vec![
                OracleRung::Bench,
                OracleRung::Read,
                OracleRung::Act,
                OracleRung::Witness,
            ],
            toolchain: Toolchain {
                interpreter: "python3.13".into(),
                lock_digest: String::new(),
            },
            containment: "jail-v1".into(),
        }
    }

    fn candidate() -> GenerationOutcome {
        GenerationOutcome::Candidate {
            manifest: Manifest {
                files: vec![
                    FileEntry {
                        path: "sp548e.py".into(),
                        digest: digest_bytes(b"driver"),
                        role: FileRole::Source,
                    },
                    FileEntry {
                        path: "test_sp548e.py".into(),
                        digest: digest_bytes(b"tests"),
                        role: FileRole::SelfTest,
                    },
                ],
            },
            entrypoints: vec!["sp548e.py".into()],
            self_tests: vec!["test_sp548e.py".into()],
            declared_effects: vec!["state".into(), "on".into(), "off".into()],
            toolchain_lock: String::new(),
            capability_surface: vec!["state".into(), "on".into(), "off".into()],
        }
    }

    #[test]
    fn a_whole_order_validates() {
        assert!(validate_order(&order_one()).is_ok());
    }

    #[test]
    fn an_unordered_or_duplicated_plan_is_refused() {
        let mut o = order_one();
        o.oracle_plan = vec![OracleRung::Read, OracleRung::Bench];
        assert_eq!(validate_order(&o), Err(OrderError::UnorderedOraclePlan));
        o.oracle_plan = vec![OracleRung::Bench, OracleRung::Bench];
        assert_eq!(validate_order(&o), Err(OrderError::UnorderedOraclePlan));
    }

    #[test]
    fn a_candidate_within_its_order_validates() {
        assert!(validate_outcome(&order_one(), &candidate()).is_ok());
    }

    #[test]
    fn a_refusal_is_always_a_valid_outcome() {
        let r = GenerationOutcome::Refused(Refusal {
            code: "research-insufficient".into(),
            rationale: "no framing spec for command 0x57".into(),
            unmet_requirements: vec!["dynamic-mode RGB framing".into()],
            evidence: None,
        });
        assert!(validate_outcome(&order_one(), &r).is_ok());
    }

    #[test]
    fn dangling_entrypoints_are_refused() {
        let mut c = candidate();
        if let GenerationOutcome::Candidate { entrypoints, .. } = &mut c {
            entrypoints.push("missing.py".into());
        }
        assert_eq!(
            validate_outcome(&order_one(), &c),
            Err(OrderError::DanglingPath("missing.py".into()))
        );
    }

    #[test]
    fn effects_beyond_the_ordered_surface_are_refused() {
        let mut c = candidate();
        if let GenerationOutcome::Candidate {
            declared_effects, ..
        } = &mut c
        {
            declared_effects.push("unlock-door".into());
        }
        assert_eq!(
            validate_outcome(&order_one(), &c),
            Err(OrderError::EffectBeyondOrder("unlock-door".into()))
        );
    }
}
