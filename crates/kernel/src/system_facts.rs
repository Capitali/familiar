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

/// Which KIND of truth a fact is (T-136, dialogue Round 2 Answer 1). The categories are
/// kept apart on purpose: an invariant is compiled and permanent, a deployment fact is
/// derived from the human's current declaration and changes when they change it, and an
/// observation is EVIDENCE that never becomes a SystemFact by being rendered beside one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactKind {
    /// Compiled design invariant (SF-1, SF-2): how the system is built.
    Invariant,
    /// Derived live from the human's declaration (SF-3), carrying its digest.
    Deployment,
}

/// One fact as the registry holds it — id, kind, and the human rendering.
#[derive(Debug, Clone)]
pub struct Fact {
    pub id: String,
    pub kind: FactKind,
    pub rendering: String,
}

/// The registry's answer to "what is true here, right now", with the identity a draft
/// records so a later revision can supersede rather than silently reinterpret it.
#[derive(Debug, Clone)]
pub struct FactsView {
    pub schema: u32,
    pub revision: u32,
    /// Digest of the DECLARATION the deployment facts were derived from — changes
    /// whenever the human edits actuators.json, so a thread admitted against surfaces
    /// that no longer exist is detectable rather than quietly stale.
    pub declaration_digest: String,
    pub facts: Vec<Fact>,
}

/// A small deterministic digest (FNV-1a, 64-bit, hex). Not a security primitive — an
/// identity for "which declaration did this draft see", compared for equality only.
fn digest_of(parts: &[String]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for b in p.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h ^= 0xff;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// THE runtime source of system truth (T-136/D4). Every consumer — the theorize prompt,
/// the request-answering path, any future one — renders a VIEW of this. There is no
/// sibling assembly of "what is true here"; a second assembly is how a floor starts
/// lying while every part of it stays individually correct.
pub fn view(dir: &Path) -> io::Result<FactsView> {
    let mut facts: Vec<Fact> = LIFECYCLE_FACTS
        .iter()
        .map(|f| Fact {
            id: f.id.to_string(),
            kind: FactKind::Invariant,
            rendering: f.rendering.to_string(),
        })
        .collect();
    facts.push(Fact {
        id: "SF-2".into(),
        kind: FactKind::Invariant,
        rendering: MEMBERSHIP_RENDERING.to_string(),
    });
    let (acts, _skipped) = actuator::load(dir)?;
    let mut parts: Vec<String> = Vec::new();
    let rendering = if acts.is_empty() {
        "No control surfaces are declared right now — nothing may be acted on.".to_string()
    } else {
        let mut listed: Vec<String> = Vec::new();
        for a in &acts {
            let labels: Vec<&str> = a.actions.keys().map(|s| s.as_str()).collect();
            parts.push(format!("{}:{}", a.surface, labels.join(",")));
            listed.push(format!("{} ({})", a.surface, labels.join("/")));
        }
        format!(
            "The ONLY controllable surfaces (declared by the human, ADR-0032): {}. \
             Proposals act through these or not at all.",
            listed.join("; ")
        )
    };
    facts.push(Fact {
        id: "SF-3".into(),
        kind: FactKind::Deployment,
        rendering,
    });
    Ok(FactsView {
        schema: FACTS_SCHEMA_VERSION,
        revision: FACTS_REVISION,
        declaration_digest: digest_of(&parts),
        facts,
    })
}

/// The bounded rendering a THEORIZE prompt receives — a view of [`view`], never its own
/// assembly. Kept identical in content to what [`validate`] enforces (one source).
pub fn render(dir: &Path) -> io::Result<String> {
    let v = view(dir)?;
    let mut out = format!(
        "SYSTEM FACTS (registry v{} rev{} decl:{} — these are how this system is BUILT; \
         never diagnose them as defects, never propose mechanisms outside them):\n",
        v.schema, v.revision, v.declaration_digest
    );
    for f in &v.facts {
        out.push_str(&format!("- [{}] {}\n", f.id, f.rendering));
    }
    Ok(out)
}

/// The bounded rendering the REQUEST-ANSWERING path receives (T-136). Same registry,
/// different consumer: the answering path previously assembled census/interfaces/
/// observations with NO design invariants at all, so a question about a designed
/// behaviour could be answered by a path that had never heard of it. Observations stay
/// evidence — they are labelled separately and never rendered as system facts.
pub fn render_for_answering(dir: &Path) -> io::Result<String> {
    let v = view(dir)?;
    let mut out = format!(
        "SYSTEM FACTS (registry v{} rev{} decl:{} — how this system is built and what it \
         may act on; these are not observations):\n",
        v.schema, v.revision, v.declaration_digest
    );
    for f in &v.facts {
        out.push_str(&format!("- [{}] {}\n", f.id, f.rendering));
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

#[cfg(test)]
mod t136_tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("facts_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn declare(dir: &std::path::Path, surface: &str, labels: &[&str]) {
        let actions: String = labels
            .iter()
            .map(|l| format!("\"{l}\":\"true\""))
            .collect::<Vec<_>>()
            .join(",");
        let buckets: String = labels
            .iter()
            .enumerate()
            .map(|(i, l)| {
                if i + 1 == labels.len() {
                    format!("{{\"name\":\"{l}\"}}")
                } else {
                    format!(
                        "{{\"name\":\"{l}\",\"when\":[{{\"op\":\"eq\",\"field\":\"bucket\",\"value\":\"{l}\"}}]}}"
                    )
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        let values: String = labels
            .iter()
            .map(|l| format!("\"{l}\""))
            .collect::<Vec<_>>()
            .join(",");
        fs::write(
            dir.join(ACTUATORS_FILE_FOR_TEST),
            format!(
                "{{\"actuators\":[{{\"surface\":\"{surface}\",\"state_cmd\":\"true\",\
                 \"state\":{{\"fields\":{{\"bucket\":{{\"kind\":\"enum\",\"values\":[{values}],\
                 \"source\":{{\"kind\":\"json\",\"key\":\"bucket\"}}}}}}}},\
                 \"actions\":{{{actions}}},\"buckets\":[{buckets}]}}]}}"
            ),
        )
        .unwrap();
    }
    const ACTUATORS_FILE_FOR_TEST: &str = crate::actuator::ACTUATORS_FILE;

    /// The registry is ONE source with THREE distinguishable kinds: compiled invariants
    /// stay invariant, the declaration-derived fact is marked deployment and carries a
    /// digest that MOVES when the human edits their declaration — so a draft admitted
    /// against surfaces that no longer exist is detectable, not silently stale.
    #[test]
    fn one_source_three_kinds_and_a_digest_that_moves() {
        let d = tmp("kinds");
        let bare = view(&d).unwrap();
        assert!(bare
            .facts
            .iter()
            .any(|f| f.id == "SF-1" && f.kind == FactKind::Invariant));
        assert!(bare
            .facts
            .iter()
            .any(|f| f.id == "SF-2" && f.kind == FactKind::Invariant));
        let sf3 = bare.facts.iter().find(|f| f.id == "SF-3").unwrap();
        assert_eq!(
            sf3.kind,
            FactKind::Deployment,
            "declared surfaces are deployment truth"
        );
        assert!(sf3.rendering.contains("No control surfaces"));

        declare(&d, "lights", &["dim", "bright"]);
        let one = view(&d).unwrap();
        assert_ne!(
            one.declaration_digest, bare.declaration_digest,
            "the digest tracks the declaration"
        );
        assert!(one
            .facts
            .iter()
            .any(|f| f.id == "SF-3" && f.rendering.contains("lights")));

        declare(&d, "lights", &["dim", "bright", "off"]);
        let two = view(&d).unwrap();
        assert_ne!(
            two.declaration_digest, one.declaration_digest,
            "adding an action moves it"
        );
        // Same declaration, same digest — it is an identity, not a clock.
        declare(&d, "lights", &["dim", "bright", "off"]);
        assert_eq!(view(&d).unwrap().declaration_digest, two.declaration_digest);
        let _ = fs::remove_dir_all(&d);
    }

    /// Both consumers render the SAME registry — the theorize prompt and the answering
    /// path can no longer hold different pictures of what is true here (T-136/D4).
    #[test]
    fn both_renderings_are_views_of_one_registry() {
        let d = tmp("views");
        declare(&d, "lights", &["dim", "bright"]);
        let theorize = render(&d).unwrap();
        let answering = render_for_answering(&d).unwrap();
        for id in ["SF-1", "SF-2", "SF-3"] {
            assert!(theorize.contains(id), "theorize view missing {id}");
            assert!(
                answering.contains(id),
                "answering view missing {id} — the old blindness"
            );
        }
        assert!(
            answering.contains("lights"),
            "the answering path sees the declaration"
        );
        let digest = view(&d).unwrap().declaration_digest;
        assert!(
            theorize.contains(&digest) && answering.contains(&digest),
            "both renderings name the declaration they stand on"
        );
        let _ = fs::remove_dir_all(&d);
    }
}
