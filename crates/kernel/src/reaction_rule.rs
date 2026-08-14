//! Standing reaction rules (ADR-0039 §3, completing ADR-0032's deferred half).
//!
//! A confirmed intent becomes an OBJECT — "away → lights dim, back → lights bright" —
//! instead of a recurring question. The consent moment is the MINT (a human's explicit
//! instruction today; an affirmed theory next), once, not per firing. Every firing still
//! walks the whole ADR-0032 discipline: the `allow_actuate` gate, the guard, a declared
//! surface, the revert map, one act per surface per window — and a reverted firing
//! **disables the rule and says so**, because a standing rule the human undid by hand
//! is a standing mistake.
//!
//! Rules are node-local (they fire on THIS node's declared surfaces), subject-owned,
//! and listed in plain words wherever consents are listed. Under ADR-0039 they live on
//! the subject's HumanRecord as routines; until that record exists they live here.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use crate::store;

/// Rules and the presence memory they trigger from, one small familiar-owned JSON.
pub const RULES_FILE: &str = "reaction_rules.json";

/// What sets a rule off. Presence transitions are the first (and the lighting ask's)
/// trigger kind; schedule windows and habit thresholds arrive with their own bricks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Trigger {
    /// The subject's presence at this node ends.
    Away,
    /// The subject's presence at this node returns.
    Back,
}

impl Trigger {
    /// The rule read as a sentence fragment — consoles and the CLI both say it this way.
    pub fn word(&self) -> &'static str {
        match self {
            Trigger::Away => "away",
            Trigger::Back => "back",
        }
    }
}

/// One standing rule: when `subject` goes `trigger`, set `surface` to `act`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReactionRule {
    pub id: String,
    /// Whose rule this is — the human it serves, and whose hand can kill it.
    pub subject: String,
    pub trigger: Trigger,
    /// A DECLARED surface (ADR-0032) and one of its acts, checked at fire time — a rule
    /// outliving its surface simply never fires (and lists as such).
    pub surface: String,
    pub act: String,
    /// Where the consent came from: "cli", or "thread:<id>" once theory-minting lands.
    #[serde(default)]
    pub minted_from: String,
    pub minted_at: i64,
    pub enabled: bool,
    /// Why it stopped, when it did ("reverted by hand", "disabled from the console").
    #[serde(default)]
    pub disabled_reason: String,
    #[serde(default)]
    pub last_fired: i64,
}

impl ReactionRule {
    /// "away → lights dim (for ian)" — one honest line, everywhere it is listed.
    pub fn sentence(&self) -> String {
        format!(
            "{} → {} {} (for {})",
            self.trigger.word(),
            self.surface,
            self.act,
            self.subject
        )
    }
}

/// The store: the rules plus the per-subject presence memory transitions derive from.
/// Presence here is a MEMORY, not a judgement — `evaluate` is handed who is present now
/// (the same routing evidence everything else uses) and compares.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RulesFile {
    #[serde(default)]
    pub rules: Vec<ReactionRule>,
    /// handle → was present at the last evaluation. Absent = never seen (seed silently).
    #[serde(default)]
    pub presence: BTreeMap<String, bool>,
}

pub fn load(dir: &Path) -> RulesFile {
    store::load_one(dir, RULES_FILE)
        .ok()
        .flatten()
        .unwrap_or_default()
}

pub fn save(dir: &Path, f: &RulesFile) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let s = serde_json::to_string_pretty(f)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    std::fs::write(dir.join(RULES_FILE), s)
}

/// Mint a rule — the consent moment. Refuses an empty subject/surface/act; the surface's
/// existence is checked at FIRE time (declaring after minting is fine, and a surface can
/// come and go without eating its rules).
pub fn mint(
    dir: &Path,
    subject: &str,
    trigger: Trigger,
    surface: &str,
    act: &str,
    minted_from: &str,
    now: i64,
) -> io::Result<ReactionRule> {
    let (subject, surface, act) = (subject.trim(), surface.trim(), act.trim());
    if subject.is_empty() || surface.is_empty() || act.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a rule needs a subject, a surface, and an act",
        ));
    }
    let mut f = load(dir);
    // One rule per (subject, trigger, surface): minting again re-points the act and
    // re-enables — the human's newest word is the rule.
    if let Some(r) = f
        .rules
        .iter_mut()
        .find(|r| r.subject == subject && r.trigger == trigger && r.surface == surface)
    {
        r.act = act.to_string();
        r.minted_from = minted_from.to_string();
        r.minted_at = now;
        r.enabled = true;
        r.disabled_reason.clear();
        let out = r.clone();
        save(dir, &f)?;
        return Ok(out);
    }
    let rule = ReactionRule {
        id: format!(
            "{:08x}",
            (now as u64).wrapping_mul(0x9e37_79b9) & 0xffff_ffff
        ),
        subject: subject.to_string(),
        trigger,
        surface: surface.to_string(),
        act: act.to_string(),
        minted_from: minted_from.to_string(),
        minted_at: now,
        enabled: true,
        disabled_reason: String::new(),
        last_fired: 0,
    };
    f.rules.push(rule.clone());
    save(dir, &f)?;
    Ok(rule)
}

/// Enable/disable by id (the console's one-tap and the CLI both land here).
pub fn set_enabled(dir: &Path, id: &str, enabled: bool, reason: &str) -> io::Result<bool> {
    let mut f = load(dir);
    let Some(r) = f.rules.iter_mut().find(|r| r.id == id) else {
        return Ok(false);
    };
    r.enabled = enabled;
    r.disabled_reason = if enabled {
        String::new()
    } else {
        reason.to_string()
    };
    save(dir, &f)?;
    Ok(true)
}

/// A rule-initiated act was undone by the human's hand — the standing rule is a standing
/// mistake: disable it, with the reversal as the reason. Returns the rule's sentence.
pub fn disable_reverted(dir: &Path, id: &str, now: i64) -> io::Result<Option<String>> {
    let mut f = load(dir);
    let Some(r) = f.rules.iter_mut().find(|r| r.id == id) else {
        return Ok(None);
    };
    r.enabled = false;
    r.disabled_reason = format!("reverted by hand at {now}");
    let s = r.sentence();
    save(dir, &f)?;
    Ok(Some(s))
}

/// A firing due right now: which enabled rules' triggers match the presence transitions
/// between the remembered state and `present_now`. Updates the memory in place; the
/// FIRST evaluation seeds silently (launch-silent, like every edge in this codebase).
/// Only subjects that appear in `known_subjects` are tracked — presence evidence about
/// nobody-in-particular must not fabricate transitions.
pub fn due(dir: &Path, present_now: &[String], now: i64) -> io::Result<Vec<ReactionRule>> {
    let mut f = load(dir);
    if f.rules.is_empty() {
        return Ok(Vec::new());
    }
    let mut fired = Vec::new();
    let subjects: Vec<String> = f
        .rules
        .iter()
        .map(|r| r.subject.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    for s in subjects {
        let is_present = present_now.iter().any(|p| p == &s);
        match f.presence.get(&s) {
            None => {
                // First sight of this subject: remember, never fire.
                f.presence.insert(s.clone(), is_present);
            }
            Some(&was) if was != is_present => {
                f.presence.insert(s.clone(), is_present);
                let trig = if is_present {
                    Trigger::Back
                } else {
                    Trigger::Away
                };
                for r in f
                    .rules
                    .iter_mut()
                    .filter(|r| r.enabled && r.subject == s && r.trigger == trig)
                {
                    r.last_fired = now;
                    fired.push(r.clone());
                }
            }
            _ => {}
        }
    }
    save(dir, &f)?;
    Ok(fired)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("familiar_rules_{tag}"));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    const NOW: i64 = 1_790_000_000;

    #[test]
    fn a_rule_fires_on_the_transition_and_only_the_transition() {
        let d = dir("fire");
        mint(&d, "ian", Trigger::Away, "lights", "dim", "cli", NOW).unwrap();
        mint(&d, "ian", Trigger::Back, "lights", "bright", "cli", NOW).unwrap();

        // First evaluation seeds silently, whatever the state.
        assert!(due(&d, &["ian".into()], NOW + 1).unwrap().is_empty());
        // Present → present: nothing.
        assert!(due(&d, &["ian".into()], NOW + 2).unwrap().is_empty());
        // Present → absent: the away rule, exactly once.
        let fired = due(&d, &[], NOW + 3).unwrap();
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].act, "dim");
        assert!(
            due(&d, &[], NOW + 4).unwrap().is_empty(),
            "no refiring while away"
        );
        // Absent → present: the back rule.
        let fired = due(&d, &["ian".into()], NOW + 5).unwrap();
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].act, "bright");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_reverted_rule_is_disabled_and_a_remint_reenables() {
        let d = dir("revert");
        let r = mint(&d, "ian", Trigger::Away, "lights", "dim", "cli", NOW).unwrap();
        let said = disable_reverted(&d, &r.id, NOW + 10).unwrap().unwrap();
        assert!(said.contains("away → lights dim"));
        // Disabled: the transition passes it by.
        let _ = due(&d, &["ian".into()], NOW + 11).unwrap(); // seed
        assert!(
            due(&d, &[], NOW + 12).unwrap().is_empty(),
            "disabled rules never fire"
        );
        // The human's newest word re-mints the SAME slot, re-enabled, new act.
        let r2 = mint(&d, "ian", Trigger::Away, "lights", "off", "cli", NOW + 20).unwrap();
        assert_eq!(r2.id, r.id, "one rule per (subject, trigger, surface)");
        assert!(r2.enabled);
        let fired = due(&d, &["ian".into()], NOW + 21).unwrap(); // back present (seed said absent)
        assert_eq!(fired.len(), 0, "back transition has no rule here");
        let fired = due(&d, &[], NOW + 22).unwrap();
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].act, "off");
        let _ = std::fs::remove_dir_all(&d);
    }
}
