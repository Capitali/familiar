//! Declared actuators — the familiar's hand on the world (ADR-0032).
//!
//! **Declaration is the consent.** The human writes `actuators.json`; an undeclared
//! device has no path to actuation whatever the gate says, and the familiar never
//! writes this file. Each surface declares how to read its state, the acts available
//! on it (`actions`), and how a raw reading buckets into a coarse state — and the
//! bucket set doubles as the **revert map**: every bucket names the action that
//! restores it, so any change the familiar makes it can also unmake. That
//! reversibility is the license to act at all (ADR-0031: act, read the reaction,
//! undo on a bad one).
//!
//! The kernel does not know what kind of device a surface is. The declaration names
//! typed quantities or enumerated modes, says how to extract them from the state
//! command's output, and expresses ordered buckets over those fields. Device grammar
//! therefore stays at the declared edge; the kernel owns only validation, bucketing,
//! and the closed revert-map invariant.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use crate::store;

/// Human-written declaration of the surfaces the familiar may drive. Read-only to the
/// familiar (the `devices.json` idiom: the human enables it by writing the config).
pub const ACTUATORS_FILE: &str = "actuators.json";
/// The familiar's working memory per surface: last bucket, poll stamp, rest, and any
/// act awaiting its reaction. Small JSON beside the other pointers, familiar-owned.
pub const ACTUATOR_STATE_FILE: &str = "actuator_state.json";

/// One control surface the human has declared.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Actuator {
    /// Short name the familiar and the human both use ("lights").
    pub surface: String,
    #[serde(default)]
    pub description: String,
    /// Shell command that prints the surface's state. [`StateContract`] declares how
    /// to extract and type its output; the kernel has no device-specific parser.
    pub state_cmd: String,
    /// Human-declared reading contract for this surface.
    #[serde(default)]
    pub state: StateContract,
    /// Act label → shell command. Labels that are also bucket names form the revert map.
    pub actions: BTreeMap<String, String>,
    /// Ordered bucket rules — first match wins, the last should be the unconditioned
    /// fallback. Every bucket name MUST be an `actions` key (that is the revert map);
    /// a surface violating this is skipped loudly at load.
    pub buckets: Vec<BucketRule>,
    /// Extra words a need-direction may use to name this surface ("lamp led dim …").
    #[serde(default)]
    pub keywords: String,
}

/// The typed reading contract for one surface.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct StateContract {
    /// Semantic field name → type and extraction source.
    #[serde(default)]
    pub fields: BTreeMap<String, StateField>,
}

/// One field reported by a surface. Quantities carry a unit and honest range;
/// enumerations carry the complete set of values the declaration understands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StateField {
    Quantity {
        unit: String,
        min: f64,
        max: f64,
        source: ReadingSource,
    },
    #[serde(rename = "enum")]
    Mode {
        values: Vec<String>,
        source: ReadingSource,
        /// Optional translations from device text into a declared opaque value.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        map: Vec<EnumMatch>,
        /// Used when no explicit translation matches (for example, every known
        /// non-off device mode may honestly collapse to the opaque value `on`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fallback: Option<String>,
    },
}

/// Where one field is read from the state command's stdout. JSON is the preferred
/// adapter contract; the line extractor lets an existing device protocol live in the
/// declaration rather than in kernel code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReadingSource {
    /// A scalar at a top-level JSON object key.
    Json { key: String },
    /// The text following a line prefix, optionally narrowed between two delimiters.
    Line {
        prefix: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        between: Option<[String; 2]>,
    },
}

/// A declaration-owned translation for an opaque enumerated mode. Matching is
/// case-insensitive and first-match-wins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumMatch {
    pub contains: String,
    pub value: String,
}

/// One bucket rule. Conditions are conjunctive; ordered rules are first-match-wins,
/// and the final rule must have no conditions so every valid reading has a bucket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BucketRule {
    pub name: String,
    #[serde(default)]
    pub when: Vec<BucketCondition>,
}

/// A typed predicate over a declared state field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum BucketCondition {
    Eq { field: String, value: ReadingValue },
    AtMost { field: String, value: f64 },
    AtLeast { field: String, value: f64 },
}

/// A value after extraction and type-checking against the declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ReadingValue {
    Quantity(f64),
    Mode(String),
}

#[derive(Debug, Clone, Deserialize)]
struct ActuatorsFile {
    #[serde(default)]
    actuators: Vec<Actuator>,
}

/// Load the declared surfaces. A missing file is simply no surfaces (clean no-op).
/// A surface with an invalid reading contract, bucket predicate, or revert map cannot
/// be read and restored honestly, so it is dropped — and returned by name so the caller
/// can say so.
pub fn load(dir: &Path) -> io::Result<(Vec<Actuator>, Vec<String>)> {
    let Some(f) = store::load_one::<ActuatorsFile>(dir, ACTUATORS_FILE)? else {
        return Ok((Vec::new(), Vec::new()));
    };
    let (ok, bad): (Vec<Actuator>, Vec<Actuator>) =
        f.actuators.into_iter().partition(valid_actuator);
    Ok((ok, bad.into_iter().map(|a| a.surface).collect()))
}

/// One successfully extracted and type-checked surface reading.
#[derive(Debug, Clone, PartialEq)]
pub struct StateReading {
    pub values: BTreeMap<String, ReadingValue>,
}

/// Extract a reading according to this surface's declaration. Every declared field
/// must be present, have the right scalar type, and lie inside its declared domain;
/// otherwise the state is unreadable and remains unknown.
pub fn parse_state(a: &Actuator, output: &str) -> Option<StateReading> {
    let json = serde_json::from_str::<serde_json::Value>(output.trim()).ok();
    let mut values = BTreeMap::new();
    for (name, field) in &a.state.fields {
        let value = match field {
            StateField::Quantity {
                min, max, source, ..
            } => {
                let raw = extract(source, output, json.as_ref())?;
                let n = match raw {
                    Extracted::Number(n) => n,
                    Extracted::Text(s) => s.trim().parse::<f64>().ok()?,
                };
                if !n.is_finite() || n < *min || n > *max {
                    return None;
                }
                ReadingValue::Quantity(n)
            }
            StateField::Mode {
                values: allowed,
                source,
                map,
                fallback,
            } => {
                let raw = match extract(source, output, json.as_ref())? {
                    Extracted::Text(s) => s,
                    Extracted::Number(_) => return None,
                };
                let mapped = if allowed.iter().any(|v| v == raw.trim()) {
                    raw.trim().to_string()
                } else {
                    map.iter()
                        .find(|m| contains_ignore_ascii_case(&raw, &m.contains))
                        .map(|m| m.value.clone())
                        .or_else(|| fallback.clone())?
                };
                if !allowed.contains(&mapped) {
                    return None;
                }
                ReadingValue::Mode(mapped)
            }
        };
        values.insert(name.clone(), value);
    }
    (!values.is_empty()).then_some(StateReading { values })
}

/// Which declared bucket a reading falls in — ordered rules, first match wins. A valid
/// declaration has an unconditional final rule, but "unknown" remains the fail-closed
/// result if this function is called with an invalid declaration.
pub fn bucket_of(a: &Actuator, s: &StateReading) -> String {
    for r in &a.buckets {
        if r.when.iter().all(|c| condition_matches(c, s)) {
            return r.name.clone();
        }
    }
    "unknown".to_string()
}

#[derive(Debug)]
enum Extracted {
    Number(f64),
    Text(String),
}

fn extract(
    source: &ReadingSource,
    output: &str,
    json: Option<&serde_json::Value>,
) -> Option<Extracted> {
    match source {
        ReadingSource::Json { key } => match json?.as_object()?.get(key)? {
            serde_json::Value::Number(n) => Some(Extracted::Number(n.as_f64()?)),
            serde_json::Value::String(s) => Some(Extracted::Text(s.clone())),
            _ => None,
        },
        ReadingSource::Line { prefix, between } => {
            let rest = output.lines().find_map(|line| {
                let line = line.trim();
                let head = line.get(..prefix.len())?;
                head.eq_ignore_ascii_case(prefix)
                    .then(|| line[prefix.len()..].trim())
            })?;
            let text = if let Some([start, end]) = between {
                let from = rest.find(start)? + start.len();
                let tail = &rest[from..];
                let to = tail.find(end)?;
                tail[..to].trim()
            } else {
                rest
            };
            Some(Extracted::Text(text.to_string()))
        }
    }
}

fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

fn valid_actuator(a: &Actuator) -> bool {
    if a.surface.trim().is_empty()
        || a.state_cmd.trim().is_empty()
        || a.actions.is_empty()
        || !valid_contract(&a.state)
        || a.buckets.is_empty()
    {
        return false;
    }
    let last = a.buckets.len() - 1;
    let mut names = std::collections::BTreeSet::new();
    a.buckets.iter().enumerate().all(|(i, bucket)| {
        !bucket.name.trim().is_empty()
            && names.insert(&bucket.name)
            && a.actions.contains_key(&bucket.name)
            && (bucket.when.is_empty() == (i == last))
            && bucket
                .when
                .iter()
                .all(|condition| valid_condition(condition, &a.state))
    })
}

fn valid_contract(contract: &StateContract) -> bool {
    !contract.fields.is_empty()
        && contract.fields.iter().all(|(name, field)| {
            !name.trim().is_empty()
                && match field {
                    StateField::Quantity {
                        unit,
                        min,
                        max,
                        source,
                    } => {
                        !unit.trim().is_empty()
                            && min.is_finite()
                            && max.is_finite()
                            && min <= max
                            && valid_source(source)
                    }
                    StateField::Mode {
                        values,
                        source,
                        map,
                        fallback,
                    } => {
                        let unique: std::collections::BTreeSet<_> = values.iter().collect();
                        !values.is_empty()
                            && unique.len() == values.len()
                            && values.iter().all(|v| !v.trim().is_empty())
                            && valid_source(source)
                            && map
                                .iter()
                                .all(|m| !m.contains.is_empty() && values.contains(&m.value))
                            && fallback.as_ref().is_none_or(|value| values.contains(value))
                    }
                }
        })
}

fn valid_source(source: &ReadingSource) -> bool {
    match source {
        ReadingSource::Json { key } => !key.trim().is_empty(),
        ReadingSource::Line { prefix, between } => {
            !prefix.is_empty()
                && between
                    .as_ref()
                    .is_none_or(|[start, end]| !start.is_empty() && !end.is_empty())
        }
    }
}

fn valid_condition(condition: &BucketCondition, contract: &StateContract) -> bool {
    match condition {
        BucketCondition::Eq { field, value } => match (contract.fields.get(field), value) {
            (Some(StateField::Quantity { min, max, .. }), ReadingValue::Quantity(value)) => {
                value.is_finite() && value >= min && value <= max
            }
            (Some(StateField::Mode { values, .. }), ReadingValue::Mode(value)) => {
                values.contains(value)
            }
            _ => false,
        },
        BucketCondition::AtMost { field, value } | BucketCondition::AtLeast { field, value } => {
            matches!(
                contract.fields.get(field),
                Some(StateField::Quantity { min, max, .. })
                    if value.is_finite() && value >= min && value <= max
            )
        }
    }
}

fn condition_matches(condition: &BucketCondition, reading: &StateReading) -> bool {
    match condition {
        BucketCondition::Eq { field, value } => reading.values.get(field) == Some(value),
        BucketCondition::AtMost { field, value } => matches!(
            reading.values.get(field),
            Some(ReadingValue::Quantity(actual)) if actual <= value
        ),
        BucketCondition::AtLeast { field, value } => matches!(
            reading.values.get(field),
            Some(ReadingValue::Quantity(actual)) if actual >= value
        ),
    }
}

/// An act awaiting its reaction — who it was for, what it set, what restores the
/// world, and how much of the thread's answer history predates it (so a *new* answer
/// is recognizable).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PendingAct {
    pub thread_id: String,
    #[serde(default)]
    pub candidate_id: String,
    /// The bucket the familiar set.
    pub label: String,
    /// The bucket to restore on a negative reaction.
    pub prev: String,
    pub at: i64,
    #[serde(default)]
    pub answers_seen: usize,
}

/// The familiar's working memory for one surface.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SurfaceState {
    /// Last known bucket. Written at act time too (the self-debounce: the poller
    /// structurally cannot see the familiar's own change as a transition).
    pub bucket: String,
    pub polled_at: i64,
    /// After a reverted act the surface rests — no new act until this passes.
    #[serde(default)]
    pub rest_until: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub act: Option<PendingAct>,
}

pub fn load_state(dir: &Path) -> BTreeMap<String, SurfaceState> {
    store::load_one(dir, ACTUATOR_STATE_FILE)
        .ok()
        .flatten()
        .unwrap_or_default()
}

pub fn save_state(dir: &Path, m: &BTreeMap<String, SurfaceState>) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let s = serde_json::to_string_pretty(m)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    std::fs::write(dir.join(ACTUATOR_STATE_FILE), s)
}

/// Does an answer read as *no*? Deterministic and whole-word on purpose — no model
/// judges a reaction; when in doubt the answer is not negative and the change stands,
/// because the human can always still undo it by hand (which the poller honors).
/// Does an answer read as an explicit *yes*? Deterministic and whole-word, like
/// [`is_negative`] — no model judges consent. Minting a STANDING policy demands this
/// (T-102): silence and mere non-negativity keep a one-shot act, but a rule that will
/// fire forever needs the human to have actually said so. Callers must also check
/// `!is_negative` — "no, not okay" contains "okay".
pub fn is_affirmative(text: &str) -> bool {
    let t = text.to_lowercase();
    if t.contains("do it") || t.contains("go ahead") || t.contains("sounds good") {
        return true;
    }
    t.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .any(|w| {
            matches!(
                w,
                "yes"
                    | "yeah"
                    | "yep"
                    | "sure"
                    | "please"
                    | "ok"
                    | "okay"
                    | "absolutely"
                    | "definitely"
            )
        })
}

pub fn is_negative(text: &str) -> bool {
    let t = text.to_lowercase();
    if t.contains("put it back") || t.contains("leave it") {
        return true;
    }
    t.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .any(|w| {
            matches!(
                w,
                "no" | "not" | "don't" | "dont" | "stop" | "undo" | "revert" | "wrong"
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn dir(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn motorlights() -> Actuator {
        Actuator {
            surface: "lights".into(),
            description: "test strip".into(),
            state_cmd: "true".into(),
            state: StateContract {
                fields: BTreeMap::from([
                    (
                        "power".into(),
                        StateField::Mode {
                            values: vec!["off".into(), "on".into()],
                            source: ReadingSource::Line {
                                prefix: "light mode :".into(),
                                between: None,
                            },
                            map: vec![EnumMatch {
                                contains: "off".into(),
                                value: "off".into(),
                            }],
                            fallback: Some("on".into()),
                        },
                    ),
                    (
                        "level".into(),
                        StateField::Quantity {
                            unit: "percent".into(),
                            min: 0.0,
                            max: 100.0,
                            source: ReadingSource::Line {
                                prefix: "brightness :".into(),
                                between: Some(["(".into(), "%)".into()]),
                            },
                        },
                    ),
                ]),
            },
            actions: BTreeMap::from([
                ("off".to_string(), "cmd-off".to_string()),
                ("dim".to_string(), "cmd-dim".to_string()),
                ("bright".to_string(), "cmd-bright".to_string()),
            ]),
            buckets: vec![
                BucketRule {
                    name: "off".into(),
                    when: vec![BucketCondition::Eq {
                        field: "power".into(),
                        value: ReadingValue::Mode("off".into()),
                    }],
                },
                BucketRule {
                    name: "dim".into(),
                    when: vec![
                        BucketCondition::Eq {
                            field: "power".into(),
                            value: ReadingValue::Mode("on".into()),
                        },
                        BucketCondition::AtMost {
                            field: "level".into(),
                            value: 40.0,
                        },
                    ],
                },
                BucketRule {
                    name: "bright".into(),
                    when: Vec::new(),
                },
            ],
            keywords: "lamp led".into(),
        }
    }

    #[test]
    fn loads_declared_actuators_and_rejects_a_bucket_without_a_revert_action() {
        let p = dir("familiar_actuator_load");
        let good = motorlights();
        let mut broken = good.clone();
        broken.surface = "heater".into();
        broken.actions.remove("bright");
        fs::write(
            p.join(ACTUATORS_FILE),
            serde_json::json!({"actuators": [good, broken]}).to_string(),
        )
        .unwrap();
        let (ok, bad) = load(&p).unwrap();
        assert_eq!(ok.len(), 1, "lights loads");
        assert_eq!(
            bad,
            vec!["heater"],
            "a bucket with no restoring action breaks the revert promise — dropped loudly"
        );
        // A missing file is no surfaces, not an error.
        let empty = dir("familiar_actuator_none");
        assert!(load(&empty).unwrap().0.is_empty());
        let _ = fs::remove_dir_all(&p);
    }

    #[test]
    fn the_motorlights_grammar_lives_in_its_declaration() {
        let a = motorlights();
        let out = "light mode : 0x01  Static Color\nbrightness : 51/255  (20%)\nidentity   : V3.0.10 , GIIWEO";
        let s = parse_state(&a, out).unwrap();
        assert_eq!(
            s.values.get("power"),
            Some(&ReadingValue::Mode("on".into()))
        );
        assert_eq!(s.values.get("level"), Some(&ReadingValue::Quantity(20.0)));
        assert_eq!(bucket_of(&a, &s), "dim");
        assert!(
            parse_state(&a, "no state reply").is_none(),
            "unreadable is unknown, never guessed"
        );
    }

    #[test]
    fn bucket_rules_first_match_wins_and_the_last_is_the_fallback() {
        let a = motorlights();
        let reading = |power: &str, level: f64| StateReading {
            values: BTreeMap::from([
                ("power".into(), ReadingValue::Mode(power.into())),
                ("level".into(), ReadingValue::Quantity(level)),
            ]),
        };
        let off = reading("off", 0.0);
        let mid = reading("on", 40.0);
        let full = reading("on", 90.0);
        assert_eq!(bucket_of(&a, &off), "off");
        assert_eq!(bucket_of(&a, &mid), "dim", "at the threshold is within it");
        assert_eq!(
            bucket_of(&a, &full),
            "bright",
            "the fallback catches the rest"
        );
    }

    #[test]
    fn a_fridge_and_a_vent_need_no_kernel_device_types() {
        let fridge: Actuator = serde_json::from_value(serde_json::json!({
            "surface": "fridge",
            "state_cmd": "fridge-state --json",
            "state": {"fields": {
                "temperature": {
                    "kind": "quantity", "unit": "celsius", "min": -10, "max": 20,
                    "source": {"kind": "json", "key": "temperature_c"}
                }
            }},
            "actions": {"cold": "set 3", "warm": "set 7"},
            "buckets": [
                {"name": "warm", "when": [
                    {"op": "at_least", "field": "temperature", "value": 5}
                ]},
                {"name": "cold"}
            ]
        }))
        .unwrap();
        let vent: Actuator = serde_json::from_value(serde_json::json!({
            "surface": "vent",
            "state_cmd": "vent-state --json",
            "state": {"fields": {
                "position": {
                    "kind": "enum", "values": ["open", "closed"],
                    "source": {"kind": "json", "key": "position"}
                }
            }},
            "actions": {"open": "open-it", "closed": "close-it"},
            "buckets": [
                {"name": "closed", "when": [
                    {"op": "eq", "field": "position", "value": "closed"}
                ]},
                {"name": "open"}
            ]
        }))
        .unwrap();

        assert!(valid_actuator(&fridge));
        assert!(valid_actuator(&vent));
        let warm = parse_state(&fridge, r#"{"temperature_c":6.5}"#).unwrap();
        assert_eq!(bucket_of(&fridge, &warm), "warm");
        let open = parse_state(&vent, r#"{"position":"open"}"#).unwrap();
        assert_eq!(bucket_of(&vent, &open), "open");
        assert!(
            parse_state(&fridge, r#"{"temperature_c":50}"#).is_none(),
            "a reading outside its declared range is unknown, never clamped"
        );
    }

    #[test]
    fn declaration_conditions_are_typed_and_the_final_fallback_is_required() {
        let mut a = motorlights();
        a.buckets[0].when = vec![BucketCondition::AtMost {
            field: "power".into(),
            value: 1.0,
        }];
        assert!(!valid_actuator(&a), "numeric comparison on an enum refuses");

        let mut no_fallback = motorlights();
        no_fallback.buckets.last_mut().unwrap().when = vec![BucketCondition::Eq {
            field: "power".into(),
            value: ReadingValue::Mode("on".into()),
        }];
        assert!(
            !valid_actuator(&no_fallback),
            "every valid reading must map to a restoring action"
        );
    }

    #[test]
    fn negative_reaction_words_are_deterministic_and_whole_word() {
        assert!(is_negative("No, too dark"));
        assert!(is_negative("please put it back"));
        assert!(is_negative("don't do that"));
        assert!(
            !is_negative("nothing better than this"),
            "'nothing' is not 'no'"
        );
        assert!(!is_negative("yes, lovely"), "assent stands");
        assert!(
            !is_negative(""),
            "silence is not a no — the poller reads hands, not words"
        );
    }

    #[test]
    fn surface_state_round_trips_with_a_pending_act() {
        let p = dir("familiar_actuator_state");
        let mut m = BTreeMap::new();
        m.insert(
            "lights".to_string(),
            SurfaceState {
                bucket: "dim".into(),
                polled_at: 100,
                rest_until: 0,
                act: Some(PendingAct {
                    thread_id: "thread-0007".into(),
                    candidate_id: "candidate-0042".into(),
                    label: "dim".into(),
                    prev: "bright".into(),
                    at: 100,
                    answers_seen: 0,
                }),
            },
        );
        save_state(&p, &m).unwrap();
        assert_eq!(load_state(&p), m);
        assert!(load_state(&dir("familiar_actuator_state_none")).is_empty());
        let _ = fs::remove_dir_all(&p);
    }
}
