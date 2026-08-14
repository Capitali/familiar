//! One classing contract for every lens (dialogue Q7, decided 2026-08-14).
//!
//! A1's co-occurrence, B1's predictions, the scenario oracles' output contracts, and
//! any future absence lens all need to say "this kind of observation" — and if each
//! grew its own string heuristic, composition would inherit translation seams between
//! them. This module is the ONE vocabulary: a versioned class for grouping ("which
//! behaviour is this?") and a typed matcher for prediction ("does this observation
//! satisfy the claim?"). The VERSION rides every persisted class and matcher, so a
//! later, sharper classing never silently reinterprets history — an old record means
//! what it meant on the day it was written.

use crate::observation::Observation;
use serde::{Deserialize, Serialize};

/// The classing scheme in force. v1 is A1's head heuristic, given a name and a home:
/// `actor | action | object-head`, the head being the object up to its first `=` or
/// `:` — so `lights=dim` and `lights=bright` are one behaviour, and `thread:1` and
/// `thread:2` are one channel.
pub const CLASS_VERSION: u32 = 1;

/// A versioned observation class — the grouping key lenses share.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObsClass {
    /// Scheme version that produced `key` (see [`CLASS_VERSION`]).
    pub v: u32,
    /// The class key, `actor|action|object_head` under v1.
    pub key: String,
}

/// The v1 object head: everything before the first `=` or `:`.
fn object_head(object: &str) -> &str {
    let end = object.find(['=', ':']).unwrap_or(object.len());
    &object[..end]
}

/// Class an observation under the CURRENT scheme.
pub fn class_of(o: &Observation) -> ObsClass {
    ObsClass {
        v: CLASS_VERSION,
        key: format!("{}|{}|{}", o.actor, o.action, object_head(&o.object)),
    }
}

/// The class key alone, for callers that key maps by string (A1's pair tables).
pub fn class_key(o: &Observation) -> String {
    class_of(o).key
}

/// A human-legible rendering of a class key ("ian adjusted lights").
pub fn class_pretty(key: &str) -> String {
    key.replace('|', " ")
}

/// How one field of an observation may be matched (dialogue Q1: exact and prefix cover
/// the record's real vocabulary — `lighting:` heads, `soil-moisture:` heads — without
/// regex or glob ambiguity). Extending this enum REQUIRES bumping [`MATCH_VERSION`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum FieldMatch {
    /// Matches anything, including empty.
    Any,
    /// Byte-exact equality.
    Exact(String),
    /// `starts_with` — the head idiom ("lighting:" matches every lighting object).
    Prefix(String),
}

/// The matcher scheme version, persisted beside every stored matcher.
pub const MATCH_VERSION: u32 = 1;

impl FieldMatch {
    pub fn matches(&self, field: &str) -> bool {
        match self {
            FieldMatch::Any => true,
            FieldMatch::Exact(v) => field == v,
            FieldMatch::Prefix(p) => field.starts_with(p.as_str()),
        }
    }
}

/// A typed observation matcher — B1's `when`/`then` building block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObsMatch {
    /// Scheme version that wrote this matcher (see [`MATCH_VERSION`]).
    #[serde(default = "match_version_default")]
    pub v: u32,
    pub actor: FieldMatch,
    pub action: FieldMatch,
    pub object: FieldMatch,
}

fn match_version_default() -> u32 {
    MATCH_VERSION
}

impl ObsMatch {
    pub fn matches(&self, o: &Observation) -> bool {
        self.actor.matches(&o.actor)
            && self.action.matches(&o.action)
            && self.object.matches(&o.object)
    }

    /// The matcher as a bounded sentence fragment for narration and listings —
    /// "ian · adjusted · lighting:*" — never invented detail.
    pub fn sentence(&self) -> String {
        let part = |m: &FieldMatch| match m {
            FieldMatch::Any => "*".to_string(),
            FieldMatch::Exact(v) => v.clone(),
            FieldMatch::Prefix(p) => format!("{p}*"),
        };
        format!(
            "{} · {} · {}",
            part(&self.actor),
            part(&self.action),
            part(&self.object)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn o(actor: &str, action: &str, object: &str) -> Observation {
        Observation::new(actor, action, object, "", "test", 1_000, 1.0)
    }

    #[test]
    fn v1_classes_group_behaviour_not_payload() {
        assert_eq!(
            class_key(&o("ian", "adjusted", "lights=dim")),
            class_key(&o("ian", "adjusted", "lights=bright")),
            "one behaviour, two payloads"
        );
        assert_eq!(
            class_key(&o("ian", "answered", "thread:1")),
            class_key(&o("ian", "answered", "thread:9"))
        );
        assert_ne!(
            class_key(&o("ian", "adjusted", "lights=dim")),
            class_key(&o("betty", "adjusted", "lights=dim")),
            "whose behaviour is part of the class"
        );
        assert_eq!(class_of(&o("x", "y", "z")).v, CLASS_VERSION);
        assert_eq!(class_pretty("ian|adjusted|lights"), "ian adjusted lights");
    }

    #[test]
    fn matchers_are_typed_versioned_and_literal() {
        let m = ObsMatch {
            v: MATCH_VERSION,
            actor: FieldMatch::Exact("ian".into()),
            action: FieldMatch::Any,
            object: FieldMatch::Prefix("lighting:".into()),
        };
        assert!(m.matches(&o("ian", "adjusted", "lighting:dim")));
        assert!(m.matches(&o("ian", "reported", "lighting:75%")));
        assert!(!m.matches(&o("betty", "adjusted", "lighting:dim")));
        assert!(
            !m.matches(&o("ian", "adjusted", "lights=dim")),
            "prefix is literal — no fuzz"
        );
        assert_eq!(m.sentence(), "ian · * · lighting:*");
        // A matcher persisted without a version reads as v1 — history keeps its meaning.
        let old: ObsMatch = serde_json::from_str(
            r#"{"actor":{"kind":"any"},"action":{"kind":"exact","value":"adjusted"},"object":{"kind":"any"}}"#,
        )
        .unwrap();
        assert_eq!(old.v, MATCH_VERSION);
        assert!(old.matches(&o("anyone", "adjusted", "anything")));
    }
}
