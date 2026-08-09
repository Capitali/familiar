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
//! Buckets are deliberately coarse (a mode flag and a brightness percentage) because
//! that is what devices honestly report — the SP548E, for instance, never echoes its
//! colour back, and its state block cannot even show *off*; a surface like that simply
//! declares no `off` bucket, and "off" remains an act it can take but not a state it
//! can verify.

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
    /// Shell command that prints the surface's state (the motorlights text contract:
    /// a `light mode :` line and a `brightness : N/255  (NN%)` line).
    pub state_cmd: String,
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

/// One bucket rule. `off` matches a state whose mode reads off; `max_brightness_pct`
/// matches any on-state at or below the threshold; a rule with neither is the fallback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BucketRule {
    pub name: String,
    #[serde(default)]
    pub off: bool,
    #[serde(default)]
    pub max_brightness_pct: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
struct ActuatorsFile {
    #[serde(default)]
    actuators: Vec<Actuator>,
}

/// Load the declared surfaces. A missing file is simply no surfaces (clean no-op).
/// A surface whose buckets are not closed over its actions cannot honor the revert
/// promise, so it is dropped — and returned by name so the caller can say so.
pub fn load(dir: &Path) -> io::Result<(Vec<Actuator>, Vec<String>)> {
    let Some(f) = store::load_one::<ActuatorsFile>(dir, ACTUATORS_FILE)? else {
        return Ok((Vec::new(), Vec::new()));
    };
    let (ok, bad): (Vec<Actuator>, Vec<Actuator>) = f
        .actuators
        .into_iter()
        .partition(|a| a.buckets.iter().all(|b| a.actions.contains_key(&b.name)));
    Ok((ok, bad.into_iter().map(|a| a.surface).collect()))
}

/// A raw reading: is the surface on, and how bright.
#[derive(Debug, Clone, PartialEq)]
pub struct RawState {
    pub on: bool,
    pub brightness_pct: f64,
}

/// Parse the motorlights-shaped state text: a `light mode :` line (a mode name
/// containing "off" reads as off) and a brightness line's `(NN%)`. Returns `None`
/// when neither line is present — an unreadable state is unknown, never guessed.
pub fn parse_state(output: &str) -> Option<RawState> {
    let mut on: Option<bool> = None;
    let mut pct: Option<f64> = None;
    for line in output.lines() {
        let l = line.trim().to_lowercase();
        if let Some(rest) = l.strip_prefix("light mode") {
            on = Some(!rest.contains("off"));
        }
        if l.starts_with("brightness") {
            if let (Some(a), Some(b)) = (l.rfind('('), l.rfind("%)")) {
                if a + 1 < b {
                    pct = l[a + 1..b].trim().parse::<f64>().ok();
                }
            }
        }
    }
    match (on, pct) {
        (Some(on), Some(p)) => Some(RawState {
            on,
            brightness_pct: p,
        }),
        (Some(on), None) => Some(RawState {
            on,
            brightness_pct: 0.0,
        }),
        _ => None,
    }
}

/// Which declared bucket a raw state falls in — ordered rules, first match wins, the
/// last rule catches whatever remains. An empty rule set yields "unknown".
pub fn bucket_of(a: &Actuator, s: &RawState) -> String {
    for r in &a.buckets {
        if r.off {
            if !s.on {
                return r.name.clone();
            }
            continue;
        }
        if let Some(max) = r.max_brightness_pct {
            if s.on && s.brightness_pct <= max {
                return r.name.clone();
            }
            continue;
        }
        return r.name.clone(); // the unconditioned fallback
    }
    "unknown".to_string()
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

    fn lights() -> Actuator {
        Actuator {
            surface: "lights".into(),
            description: "test strip".into(),
            state_cmd: "true".into(),
            actions: BTreeMap::from([
                ("off".to_string(), "cmd-off".to_string()),
                ("dim".to_string(), "cmd-dim".to_string()),
                ("bright".to_string(), "cmd-bright".to_string()),
            ]),
            buckets: vec![
                BucketRule {
                    name: "off".into(),
                    off: true,
                    max_brightness_pct: None,
                },
                BucketRule {
                    name: "dim".into(),
                    off: false,
                    max_brightness_pct: Some(40.0),
                },
                BucketRule {
                    name: "bright".into(),
                    off: false,
                    max_brightness_pct: None,
                },
            ],
            keywords: "lamp led".into(),
        }
    }

    #[test]
    fn loads_declared_actuators_and_rejects_a_bucket_without_a_revert_action() {
        let p = dir("familiar_actuator_load");
        fs::write(
            p.join(ACTUATORS_FILE),
            r#"{"actuators":[
                {"surface":"lights","state_cmd":"s","actions":{"dim":"d","bright":"b"},
                 "buckets":[{"name":"dim","max_brightness_pct":40},{"name":"bright"}]},
                {"surface":"heater","state_cmd":"s","actions":{"on":"o"},
                 "buckets":[{"name":"on"},{"name":"cold"}]}
            ]}"#,
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
    fn parses_the_motorlights_state_block_into_buckets() {
        let out = "light mode : 0x01  Static Color\nbrightness : 51/255  (20%)\nidentity   : V3.0.10 , GIIWEO";
        let s = parse_state(out).unwrap();
        assert!(s.on);
        assert!((s.brightness_pct - 20.0).abs() < 0.01);
        assert_eq!(bucket_of(&lights(), &s), "dim");
        assert!(
            parse_state("no state reply").is_none(),
            "unreadable is unknown, never guessed"
        );
    }

    #[test]
    fn bucket_rules_first_match_wins_and_the_last_is_the_fallback() {
        let a = lights();
        let off = RawState {
            on: false,
            brightness_pct: 0.0,
        };
        let mid = RawState {
            on: true,
            brightness_pct: 40.0,
        };
        let full = RawState {
            on: true,
            brightness_pct: 90.0,
        };
        assert_eq!(bucket_of(&a, &off), "off");
        assert_eq!(bucket_of(&a, &mid), "dim", "at the threshold is within it");
        assert_eq!(
            bucket_of(&a, &full),
            "bright",
            "the fallback catches the rest"
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
