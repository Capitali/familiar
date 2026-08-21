//! **The offering catalog** (ADR-0044 §1-§2, rung 2 of the ladder) — what the familiar
//! offers a covenanted partner AI: capability CLASSES, never the household.
//!
//! The anonymization guarantee here is structural, not procedural. A [`ClassDef`] is
//! **authored in this repository** — every field is `'static` vocabulary written by the
//! developers, and none is ever derived from household data. The compiler
//! ([`available`]) only decides whether a class is PRESENT, by matching declared
//! surfaces against a def's shape; what crosses the wire is built from the defs alone
//! ([`catalog_json`] takes nothing but `&[Availability]`, a type that cannot carry a
//! household string). Rule subjects, triggers, act commands, surface names, counts, and
//! free prose are not omitted by care — they are **unrepresentable** in the output type.
//!
//! Household-specific intelligence becomes a new `ClassDef` only through an explicit
//! human step: someone writes it here, in code review, as generic vocabulary (ADR-0044
//! §2 — the declassification rule). `PatternMemory` is never published.
//!
//! Instances do not exist at this rung: no ids, no names, and deliberately **no exact
//! counts** — a partner learns that "a reversible switchable surface exists here", never
//! how many, whose, or which room. Grant-bound instance handles are rung 3-5 material
//! (the codex-lane grant/ledger design) and are not represented in this module at all.

use serde_json::{json, Value};
use std::path::Path;

/// Coarse assurance, the only provenance a partner ever sees (ADR-0044 §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assurance {
    /// A human declared a matching surface (actuators.json). v1's only level.
    Declared,
    /// Reserved: the familiar has observed the surface behave as declared.
    Observed,
    /// Reserved: a proven recipe or trial history stands behind it.
    Proven,
}

impl Assurance {
    fn label(self) -> &'static str {
        match self {
            Assurance::Declared => "declared",
            Assurance::Observed => "observed",
            Assurance::Proven => "proven",
        }
    }
}

/// What kind of effect invoking this class would have — fixed vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectClass {
    /// A bounded, reversible actuation on a declared surface (closed revert map).
    ReversibleActuation,
    /// Reading an observable state field; changes nothing.
    Observation,
}

impl EffectClass {
    fn label(self) -> &'static str {
        match self {
            EffectClass::ReversibleActuation => "reversible_actuation",
            EffectClass::Observation => "observation",
        }
    }
}

/// Whose world an invocation would touch — a CLASS of subject, never a subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubjectClass {
    /// A shared physical environment quality (light, sound, temperature).
    SharedEnvironment,
}

impl SubjectClass {
    fn label(self) -> &'static str {
        match self {
            SubjectClass::SharedEnvironment => "shared_environment",
        }
    }
}

/// One generic operation a class offers (rung-3 contract): the id a grant's act leg
/// binds to. Repo-authored vocabulary — never an actuator's action string; the private
/// surface resolver maps an abstract operation to a local act only under a human grant.
pub struct ClassOperationDef {
    pub id: &'static str,
    /// Input schema as JSON-schema text, abstract slot names only.
    pub input_schema: &'static str,
}

/// One capability class: repo-authored, generic, versioned. Everything a partner may
/// learn about a capability lives in these `'static` fields and nowhere else.
pub struct ClassDef {
    /// Versioned class id ("switchable.reversible/v1").
    pub id: &'static str,
    /// One generic sentence — written here, reviewed here, never generated.
    pub summary: &'static str,
    /// Input schema as JSON-schema text, using ABSTRACT slot names only.
    pub input_schema: &'static str,
    /// What an `observe` grant could read, generically.
    pub observable: &'static str,
    pub effect: EffectClass,
    pub affected_subject: SubjectClass,
    /// Invoking twice with the same input is safe.
    pub idempotent: bool,
    /// Every act in the class has a declared revert (the actuator revert-map discipline).
    pub closed_revert: bool,
    /// What failure looks like, generically.
    pub failure: &'static str,
    /// Boundary gates an invocation would require — gate NAMES are code vocabulary.
    pub required_gates: &'static [&'static str],
    /// The generic operations a grant may bind to (rung 3). Each separately grantable.
    pub operations: &'static [ClassOperationDef],
}

/// The v1 class vocabulary. Adding to this list is the human declassification act.
pub const CLASS_DEFS: &[ClassDef] = &[ClassDef {
    id: "switchable.reversible/v1",
    summary: "A two-state switchable surface with a closed revert: whatever one act does, \
              the paired act undoes.",
    input_schema: r#"{"type":"object","properties":{"state":{"type":"string","enum":["primary","reverted"]}},"required":["state"]}"#,
    observable: "the surface's current state as one of the two declared states",
    effect: EffectClass::ReversibleActuation,
    affected_subject: SubjectClass::SharedEnvironment,
    idempotent: true,
    closed_revert: true,
    failure: "the act reports failure and the surface keeps its prior state; the revert \
              path remains available",
    required_gates: &["allow_actuate"],
    operations: &[ClassOperationDef {
        id: "set_state",
        input_schema: r#"{"type":"object","properties":{"state":{"type":"string","enum":["primary","reverted"]}},"required":["state"]}"#,
    }],
}];

/// A class the compiler found present, at some assurance. Deliberately contains no
/// household data AND no count — presence is the whole fact.
pub struct Availability {
    pub def: &'static ClassDef,
    pub assurance: Assurance,
}

/// The availability compiler (ADR-0044 §1): match declared surfaces against the class
/// vocabulary. A surface never contributes anything but a boolean "a def matched".
pub fn available(dir: &Path) -> Vec<Availability> {
    let (surfaces, _skipped) = familiar_kernel::actuator::load(dir).unwrap_or_default();
    CLASS_DEFS
        .iter()
        .filter(|def| surfaces.iter().any(|s| matches_def(def, s)))
        .map(|def| Availability {
            def,
            assurance: Assurance::Declared,
        })
        .collect()
}

/// Does one declared surface have this class's SHAPE? Structural only — names, commands
/// and descriptions are never consulted, so nothing household-specific can influence
/// anything a partner sees beyond bare presence.
fn matches_def(def: &ClassDef, s: &familiar_kernel::actuator::Actuator) -> bool {
    match def.id {
        "switchable.reversible/v1" => {
            // Exactly two acts forming a full revert pair: every bucket names an act and
            // every act is a bucket (loader already enforces bucket ⊆ actions; require
            // the converse and the arity here).
            s.actions.len() == 2
                && s.buckets.len() == 2
                && s.buckets.iter().all(|b| s.actions.contains_key(&b.name))
                && s.actions
                    .keys()
                    .all(|a| s.buckets.iter().any(|b| &b.name == a))
        }
        _ => false,
    }
}

/// The catalog as served to an attested partner. Takes ONLY `Availability` — a type
/// whose fields are `'static` defs and enums — so household content is unrepresentable
/// in this function's output by construction (the sentinel test pins it anyway).
pub fn catalog_json(avail: &[Availability]) -> Value {
    json!({
        "classes": avail.iter().map(|a| json!({
            "id": a.def.id,
            "summary": a.def.summary,
            "input_schema": serde_json::from_str::<Value>(a.def.input_schema)
                .unwrap_or(Value::Null),
            "observable": a.def.observable,
            "effect": a.def.effect.label(),
            "affected_subject": a.def.affected_subject.label(),
            "idempotent": a.def.idempotent,
            "closed_revert": a.def.closed_revert,
            "failure": a.def.failure,
            "required_gates": a.def.required_gates,
            "operations": a.def.operations.iter().map(|o| json!({
                "id": o.id,
                "input_schema": serde_json::from_str::<Value>(o.input_schema)
                    .unwrap_or(Value::Null),
            })).collect::<Vec<_>>(),
            "assurance": a.assurance.label(),
        })).collect::<Vec<_>>(),
        "note": "Classes are affordances, not authority: nothing here is invocable. \
                 Observation and invocation exist only under a grant a human makes \
                 deliberately, per capability, per partner, per bounds (ADR-0044)."
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> std::path::PathBuf {
        let p =
            std::env::temp_dir().join(format!("familiar_offering_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    /// A household surface dripping with identifying sentinels, shaped to match
    /// switchable.reversible/v1 (a valid two-state closed-revert declaration).
    fn sentinel_surface(dir: &std::path::Path) {
        let actuators = serde_json::json!({ "actuators": [{
            "surface": "ians-secret-lamp",
            "description": "the lamp beside Ian's bed at 192.168.108.44",
            "state_cmd": "curl -s http://192.168.108.44/state",
            "state": { "fields": { "power": { "kind": "enum",
                "values": ["dimmed", "restored"],
                "source": { "kind": "json", "key": "power" } } } },
            "actions": {
                "dim-ians-lamp": "curl -s http://192.168.108.44/dim",
                "restore-ians-lamp": "curl -s http://192.168.108.44/restore"
            },
            "buckets": [
                { "name": "dim-ians-lamp",
                  "when": [ { "op": "eq", "field": "power", "value": "restored" } ] },
                { "name": "restore-ians-lamp", "when": [] }
            ],
            "keywords": "ian bedroom lamp"
        }]});
        std::fs::write(
            dir.join("actuators.json"),
            serde_json::to_vec(&actuators).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn the_catalog_offers_the_class_and_serializes_no_household_content() {
        let d = temp("leak");
        sentinel_surface(&d);
        let avail = available(&d);
        assert_eq!(
            avail.len(),
            1,
            "the shaped surface makes the class available"
        );
        assert_eq!(avail[0].def.id, "switchable.reversible/v1");
        assert_eq!(avail[0].assurance.label(), "declared");
        let out = serde_json::to_string(&catalog_json(&avail)).unwrap();
        for leak in [
            "ians-secret-lamp",
            "ian",
            "192.168.108.44",
            "dim-ians-lamp",
            "restore-ians-lamp",
            "bedroom",
            "curl",
        ] {
            assert!(
                !out.to_lowercase().contains(leak),
                "household content reached the catalog: {leak}"
            );
        }
        // And no instance count crosses — presence is the whole fact.
        assert!(!out.contains("count"));
    }

    #[test]
    fn an_unshaped_surface_offers_nothing_and_an_empty_household_offers_nothing() {
        let d = temp("shape");
        // Three actions, one bucket: not a closed two-state pair — no class.
        let actuators = serde_json::json!({ "actuators": [{
            "surface": "sprinkler",
            "state_cmd": "true",
            "state": { "fields": { "power": { "kind": "enum",
                "values": ["on", "off"],
                "source": { "kind": "json", "key": "power" } } } },
            "actions": { "on": "true", "off": "true", "pulse": "true" },
            "buckets": [ { "name": "off", "when": [] } ],
        }]});
        std::fs::write(
            d.join("actuators.json"),
            serde_json::to_vec(&actuators).unwrap(),
        )
        .unwrap();
        assert!(available(&d).is_empty(), "shape mismatch offers nothing");
        let e = temp("empty");
        assert!(
            available(&e).is_empty(),
            "a fresh install offers an empty catalog"
        );
    }
}
