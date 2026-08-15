//! The knowledge floor (T-126, dialogue 2026-08-15 Q2): the system's designed facts,
//! typed and kernel-owned, injected into every theorize consult and ENFORCED after
//! parse — a draft whose mechanism contradicts a fact refuses at mint, citing the fact.
//!
//! Three categories, deliberately separate (codex, round 2):
//! 1. design invariants — compiled here, versioned;
//! 2. deployment capabilities — derived live from the human's `actuators.json`
//!    declaration (never compiled: the declaration IS the consent, ADR-0032);
//! 3. observations — evidence, never promoted into facts.
//!
//! This module also owns [`TheoryDraft`], the strict admission contract every LLM
//! theorize reply must parse into (round 2's cross-cutting shape): the model proposes
//! inside the shape; kernel types decide what becomes durable.

use std::io;
use std::path::Path;

use serde::Deserialize;

use crate::actuator;

/// Registry schema version — bumps only when the *shape* of facts changes.
pub const FACTS_SCHEMA_VERSION: u32 = 1;
/// Content revision — bumps when a fact is added, amended, or superseded. Admitted
/// drafts record the revision they were validated against; a changed fact supersedes
/// a revision, it never silently reinterprets old threads.
pub const FACTS_REVISION: u32 = 1;

/// Mechanisms a draft may claim its proposal works through. Everything else refuses:
/// an unknown mechanism is not waved through because no keyword matched (round 2).
pub const KNOWN_MECHANISMS: &[&str] = &[
    "observation", // watch and report
    "presence",    // act on the shared presence judgment (Away/Back)
    "schedule",    // act on time
    "surface-act", // drive a declared surface
    "question",    // ask the human
];

/// Membership mechanisms that exist (SF-2). Proposals about admitting people/devices
/// may speak ONLY these; external identity providers do not exist here and may not
/// be invented.
pub const MEMBERSHIP_MECHANISMS: &[&str] =
    &["covenant", "attestation", "grant", "invite", "introduction"];

/// A designed lifecycle event class (SF-1 and friends): the system working as built.
/// A draft that claims one of these is a defect refuses.
pub struct LifecycleFact {
    pub id: &'static str,
    /// Matches an observation class by actor + action ("familiar purged …").
    pub actor: &'static str,
    pub action: &'static str,
    pub rendering: &'static str,
}

/// The compiled design invariants (category 1).
pub const LIFECYCLE_FACTS: &[LifecycleFact] = &[LifecycleFact {
    id: "SF-1",
    actor: "familiar",
    action: "purged",
    rendering: "Visitor purging is designed hygiene (B10): a guest who never became a \
                member is forgotten after two hours. Purge events are the system \
                working as built — never a defect to diagnose, fix, or prevent.",
}];

/// SF-2's rendering (the constraint itself is [`MEMBERSHIP_MECHANISMS`]).
pub const MEMBERSHIP_RENDERING: &str =
    "Mesh membership happens ONLY by covenant attestation and grants (ADR-0012/0026); \
     invites and introductions ride that. No external identity providers — no AppleID, \
     no OAuth, no passwords, no accounts — exist here, and none may be proposed.";

/// A refusal names the fact it stands on — the reason is on the record, not guessed.
#[derive(Debug)]
pub struct Refusal {
    pub fact_id: &'static str,
    pub why: String,
}

/// One typed prediction inside a draft — mapped onto [`crate::prediction::mint`]'s
/// `then`/window/polarity by the admitting caller. Object matching is prefix ("the
/// head idiom"), the same language `ObsMatch` speaks.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictionDraft {
    pub then_actor: String,
    pub then_action: String,
    #[serde(default)]
    pub then_object_prefix: String,
    pub within_secs: i64,
    /// "expect" | "expect_absent"
    pub polarity: String,
}

/// The strict admission contract for an LLM theorize reply (round 2, cross-cutting).
/// Unknown fields refuse — the model proposes inside the shape.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TheoryDraft {
    #[serde(default)]
    pub v: u32,
    /// Citations chosen ONLY from the eligible set the system enumerated (Q5) —
    /// observation ids ("obs-0042") or loop names ("loop:…"). Invented ids refuse.
    pub anchors: Vec<String>,
    #[serde(default)]
    pub subject: String,
    /// One of [`KNOWN_MECHANISMS`] or [`MEMBERSHIP_MECHANISMS`].
    pub mechanism: String,
    /// Observation classes ("actor|action") the draft claims are malfunctioning.
    /// Refused against designed-lifecycle facts. (v1 honesty gap, recorded in the
    /// dialogue: a draft that diagnoses in prose but leaves this empty slips the
    /// typed gate and is caught only by the rendered facts steering the consult.)
    #[serde(default)]
    pub defect_claims: Vec<String>,
    pub question: String,
    pub theory: String,
    pub direction: String,
    #[serde(default)]
    pub predictions: Vec<PredictionDraft>,
    /// The typed both-edges policy this theory proposes (T-102) — validated against
    /// the declared surface at admission, minted only on the human's explicit assent.
    #[serde(default)]
    pub rule_proposal: Option<crate::reaction_rule::RuleProposal>,
}

impl TheoryDraft {
    /// Strict parse — the one place a consult reply becomes typed.
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Validate a draft's typed claims against the registry. Order is deterministic:
/// mechanism first, then defect claims. Returns the first refusal.
pub fn validate(draft: &TheoryDraft) -> Result<(), Refusal> {
    let mech = draft.mechanism.trim().to_lowercase();
    if !KNOWN_MECHANISMS.contains(&mech.as_str()) && !MEMBERSHIP_MECHANISMS.contains(&mech.as_str())
    {
        return Err(Refusal {
            fact_id: "SF-2",
            why: format!("unknown mechanism \"{mech}\" — {MEMBERSHIP_RENDERING}"),
        });
    }
    for claim in &draft.defect_claims {
        let c = claim.trim().to_lowercase();
        for f in LIFECYCLE_FACTS {
            let key = format!("{}|{}", f.actor, f.action);
            if c == key || c.starts_with(&format!("{key}|")) {
                return Err(Refusal {
                    fact_id: f.id,
                    why: format!("\"{claim}\" is designed lifecycle — {}", f.rendering),
                });
            }
        }
    }
    Ok(())
}

/// Lexical guard for PROSE-ONLY mint paths (device theories, the needs muse) — NOT
/// typed enforcement (dialogue Q2, v1 gap named): the daemon cannot type what the
/// device never structured. Matches the two failure classes observed live on
/// 2026-08-15; when the console adopts the draft shape this guard retires.
pub fn lexical_guard(text: &str) -> Result<(), Refusal> {
    let t = text.to_lowercase();
    const MECHANISM_MARKS: &[&str] = &[
        "appleid", "apple id", "apple-id", "oauth", "sso", "password", "login",
    ];
    if MECHANISM_MARKS.iter().any(|m| t.contains(m)) {
        return Err(Refusal {
            fact_id: "SF-2",
            why: format!("proposes an external identity mechanism — {MEMBERSHIP_RENDERING}"),
        });
    }
    const PURGE_MARKS: &[&str] = &["purge", "purged", "purging"];
    const DEFECT_MARKS: &[&str] = &[
        "unreliable",
        "problem",
        "inefficien",
        "excessive",
        "failing",
        "struggling",
        "reduce",
        "prevent",
        "fix",
        "unnecessary",
    ];
    if PURGE_MARKS.iter().any(|m| t.contains(m)) && DEFECT_MARKS.iter().any(|m| t.contains(m)) {
        return Err(Refusal {
            fact_id: "SF-1",
            why: format!(
                "diagnoses designed purging as a defect — {}",
                LIFECYCLE_FACTS[0].rendering
            ),
        });
    }
    Ok(())
}

/// The bounded rendering every theorize prompt receives — the SAME registry the
/// validator enforces (one source of truth), plus the live deployment category:
/// declared surfaces with their action labels and a digest of the declaration.
pub fn render(dir: &Path) -> io::Result<String> {
    let mut out = String::new();
    out.push_str(&format!(
        "SYSTEM FACTS (registry v{FACTS_SCHEMA_VERSION} rev{FACTS_REVISION} — these are \
         how this system is BUILT; never diagnose them as defects, never propose \
         mechanisms outside them):\n"
    ));
    for f in LIFECYCLE_FACTS {
        out.push_str(&format!("- [{}] {}\n", f.id, f.rendering));
    }
    out.push_str(&format!("- [SF-2] {MEMBERSHIP_RENDERING}\n"));
    let (acts, _skipped) = actuator::load(dir)?;
    if acts.is_empty() {
        out.push_str(
            "- [SF-3] No control surfaces are declared right now — nothing may be acted on.\n",
        );
    } else {
        let mut listed: Vec<String> = Vec::new();
        for a in &acts {
            let labels: Vec<&str> = a.actions.keys().map(|s| s.as_str()).collect();
            listed.push(format!("{} ({})", a.surface, labels.join("/")));
        }
        out.push_str(&format!(
            "- [SF-3] The ONLY controllable surfaces (declared by the human, ADR-0032): {}. \
             Proposals act through these or not at all.\n",
            listed.join("; ")
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(mechanism: &str, defect_claims: &[&str]) -> TheoryDraft {
        TheoryDraft {
            v: 1,
            anchors: vec!["obs-0001".into()],
            subject: "ian".into(),
            mechanism: mechanism.into(),
            defect_claims: defect_claims.iter().map(|s| s.to_string()).collect(),
            question: "q".into(),
            theory: "t".into(),
            direction: "d".into(),
            predictions: Vec::new(),
            rule_proposal: None,
        }
    }

    /// The two failure classes observed live on 2026-08-15 refuse with the fact cited;
    /// a clean presence proposal passes. The floor holds what the mind may claim.
    #[test]
    fn the_floor_refuses_invented_mechanisms_and_designed_lifecycle_diagnoses() {
        let bad_mech = validate(&draft("appleid-login", &[])).unwrap_err();
        assert_eq!(bad_mech.fact_id, "SF-2");
        let bad_claim = validate(&draft("presence", &["familiar|purged"])).unwrap_err();
        assert_eq!(bad_claim.fact_id, "SF-1");
        assert!(validate(&draft("presence", &[])).is_ok());
        assert!(validate(&draft("grant", &[])).is_ok());
    }

    /// Prose paths get the labeled lexical guard: AppleID proposals and
    /// purge-as-defect diagnoses refuse; ordinary lighting prose passes.
    #[test]
    fn the_lexical_guard_catches_the_observed_prose_classes() {
        assert_eq!(
            lexical_guard("streamline visitor onboarding with a permanent AppleID login")
                .unwrap_err()
                .fact_id,
            "SF-2"
        );
        assert_eq!(
            lexical_guard("frequent visitor purges suggest presence detection is unreliable")
                .unwrap_err()
                .fact_id,
            "SF-1"
        );
        assert!(lexical_guard("dim the lights to candle when everyone is away").is_ok());
        // Purge mentioned WITHOUT a defect frame is fine — reporting is not diagnosing.
        assert!(lexical_guard("three visitors were purged today, as designed").is_ok());
    }

    /// The admission contract is strict: unknown fields refuse, the required core
    /// parses, and defaults fill the optional tail.
    #[test]
    fn the_draft_contract_is_strict_and_complete() {
        let good = r#"{"anchors":["obs-0009"],"mechanism":"presence","question":"q","theory":"t","direction":"d"}"#;
        let d = TheoryDraft::parse(good).unwrap();
        assert_eq!(d.anchors, vec!["obs-0009"]);
        assert!(d.predictions.is_empty());
        let unknown = r#"{"anchors":[],"mechanism":"presence","question":"q","theory":"t","direction":"d","surprise":true}"#;
        assert!(TheoryDraft::parse(unknown).is_err());
        let with_pred = r#"{"anchors":["obs-1"],"mechanism":"presence","question":"q","theory":"t","direction":"d",
            "predictions":[{"then_actor":"ian","then_action":"adjusted","then_object_prefix":"lighting:","within_secs":7200,"polarity":"expect_absent"}]}"#;
        assert_eq!(TheoryDraft::parse(with_pred).unwrap().predictions.len(), 1);
    }
}
