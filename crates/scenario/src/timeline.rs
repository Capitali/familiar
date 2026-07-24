//! The timeline — component 6 of a scenario's anatomy (ADR-0010): a deterministic
//! sequence of events. The same scenario unfolds identically until the familiar
//! changes it. Each event may mutate the world and — only when `observable` —
//! become an observation the familiar is allowed to perceive (component 3:
//! observable information, and nothing more).

use familiar_kernel::observation::Observation;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

/// A world mutation that accompanies an event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Effect {
    /// Append `text` (plus a newline) to `path`, creating it if absent.
    Append { path: String, text: String },
    /// Overwrite `path` with `text`.
    Write { path: String, text: String },
    /// Remove `path` if present.
    Remove { path: String },
}

/// One deterministic event on the scenario's timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub actor: String,
    pub action: String,
    pub object: String,
    #[serde(default)]
    pub context: String,
    /// World mutations applied when this event fires.
    #[serde(default)]
    pub effects: Vec<Effect>,
    /// May the familiar perceive this event? Hidden events shape the world without
    /// ever entering the observation log.
    #[serde(default = "default_true")]
    pub observable: bool,
}

fn default_true() -> bool {
    true
}

/// Apply one effect to the world rooted at `root`.
pub fn apply(root: &Path, effect: &Effect) -> io::Result<()> {
    match effect {
        Effect::Append { path, text } => {
            let dest = root.join(path);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut body = fs::read_to_string(&dest).unwrap_or_default();
            body.push_str(text);
            body.push('\n');
            fs::write(dest, body)
        }
        Effect::Write { path, text } => {
            let dest = root.join(path);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(dest, text)
        }
        Effect::Remove { path } => {
            let dest = root.join(path);
            if dest.exists() {
                fs::remove_file(dest)?;
            }
            Ok(())
        }
    }
}

/// Replay the whole timeline against the world; return the observations the
/// familiar is permitted to perceive, stamped with simulated time
/// (`start_ts + index * step_secs` — no wall clock anywhere in the laboratory).
pub fn replay(
    root: &Path,
    events: &[Event],
    start_ts: i64,
    step_secs: i64,
) -> io::Result<Vec<Observation>> {
    let mut observable = Vec::new();
    for (i, ev) in events.iter().enumerate() {
        for effect in &ev.effects {
            apply(root, effect)?;
        }
        if ev.observable {
            observable.push(Observation::new(
                ev.actor.clone(),
                ev.action.clone(),
                ev.object.clone(),
                ev.context.clone(),
                "scenario",
                start_ts + (i as i64) * step_secs,
                1.0,
            ));
        }
    }
    Ok(observable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn replay_is_deterministic_and_bounds_perception() {
        let p = std::env::temp_dir().join(format!(
            "familiar_scenario_timeline_replay_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        let t = Temp(p.clone());

        let events = vec![
            Event {
                actor: "backup".into(),
                action: "failed".into(),
                object: "nightly".into(),
                context: String::new(),
                effects: vec![Effect::Append {
                    path: "logs/backup.log".into(),
                    text: "ERROR: copy failed".into(),
                }],
                observable: true,
            },
            Event {
                actor: "admin".into(),
                action: "rotated".into(),
                object: "keys".into(),
                context: String::new(),
                effects: vec![Effect::Write {
                    path: "secret/key".into(),
                    text: "k2".into(),
                }],
                observable: false,
            },
        ];
        let obs = replay(&t.0, &events, 1_700_000_000, 300).unwrap();
        // hidden events shape the world but never enter perception
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].actor, "backup");
        assert_eq!(obs[0].ts, 1_700_000_000);
        assert!(t.0.join("secret/key").exists());
        assert!(fs::read_to_string(t.0.join("logs/backup.log"))
            .unwrap()
            .contains("ERROR"));
    }
}
