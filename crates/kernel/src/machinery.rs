//! **MachineryFinding** (T-218; ADR-0043 §5) — a theory about the familiar's own machinery,
//! given a typed addressee instead of a dead end.
//!
//! The lesson it encodes (T-215): the reasoning engine produced a *correct causal
//! diagnosis of a real bug* — the guest purge loop — and the finding sat at `pursued`
//! with nothing connecting a theory to a fix, because its only possible shape was a
//! household question and its claim was refused by the facts floor. The valuable clue and
//! the mistaken framing died together.
//!
//! A `MachineryFinding` separates them: the claim is preserved with its evidence, its
//! counter-evidence (the facts that refused it), and its *explicit uncertainty* — the
//! engine misattributed the purge loop's subject, so no finding pretends to certainty
//! about subject or causality. Its addressee is the MAINTAINERS: it surfaces in a
//! development inbox (`familiar findings`), never as a household question, and it grants
//! no authority — promotion to the task board remains a human act, outside this system.
//!
//! **Terminal transitions are human acts only** (ADR-0043 §6: every terminal status names
//! who can cause the transition). The metabolism may observe and corroborate; only a named
//! human dismisses or accepts.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

use crate::store;

pub const FINDINGS_FILE: &str = "machinery_findings.jsonl";

/// The dispositions, in the order a finding walks them. `observed` and `corroborated` are
/// the metabolism's; `dismissed` and `accepted_by_human` are terminal and human-only.
pub const DISPOSITIONS: [&str; 4] = ["observed", "corroborated", "dismissed", "accepted_by_human"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MachineryFinding {
    pub id: String,
    /// The mechanism the claim concerns (a `KNOWN_MECHANISMS` entry, as drafted).
    pub mechanism: String,
    /// The system component or observation class claimed defective (e.g. `familiar|purged`).
    pub component: String,
    /// The claimed mechanism of failure, in the engine's words — preserved, not endorsed.
    pub claim: String,
    /// Supporting observation ids (the draft's admitted anchors).
    #[serde(default)]
    pub evidence: Vec<String>,
    /// What stands against it — the system-fact ids that refused the claim.
    #[serde(default)]
    pub counter_evidence: Vec<String>,
    /// Explicit uncertainty about subject and causality. Never empty: the engine that
    /// authored this has misattributed subjects before, and the finding says so.
    pub uncertainty: String,
    /// The capability or human need the claim says is affected.
    #[serde(default)]
    pub affected: String,
    /// One of [`DISPOSITIONS`].
    pub disposition: String,
    /// How many times the same (mechanism, component) claim has been re-observed.
    pub corroborations: u32,
    pub created_at: i64,
    pub updated_at: i64,
    /// Who moved it to a terminal state — a human handle, or empty while open.
    #[serde(default)]
    pub decided_by: String,
}

impl MachineryFinding {
    pub fn is_terminal(&self) -> bool {
        matches!(self.disposition.as_str(), "dismissed" | "accepted_by_human")
    }
}

pub fn load(dir: &Path) -> io::Result<Vec<MachineryFinding>> {
    store::load(dir, FINDINGS_FILE)
}

/// The metabolism's one write path: observe a machinery claim. The same (mechanism,
/// component) claim while a finding is OPEN corroborates it — evidence accumulates on one
/// finding instead of minting a duplicate per consult (the theory-churn lesson, T-127).
/// A claim matching only TERMINAL findings mints fresh: a dismissed diagnosis that keeps
/// being re-derived is itself signal, and silently swallowing it would un-say the record.
/// Returns the finding id written.
#[allow(clippy::too_many_arguments)]
pub fn observe(
    dir: &Path,
    mechanism: &str,
    component: &str,
    claim: &str,
    evidence: &[String],
    counter_evidence: &[String],
    affected: &str,
    now: i64,
) -> io::Result<String> {
    let all = load(dir)?;
    if let Some(open) = all
        .iter()
        .find(|f| f.mechanism == mechanism && f.component == component && !f.is_terminal())
    {
        let id = open.id.clone();
        store::update_by_id(dir, FINDINGS_FILE, &id, &{
            let mut f = open.clone();
            f.corroborations += 1;
            f.disposition = "corroborated".into();
            f.updated_at = now;
            for e in evidence {
                if !f.evidence.contains(e) {
                    f.evidence.push(e.clone());
                }
            }
            f
        })?;
        return Ok(id);
    }
    let id = format!("finding-{:04}", all.len() + 1);
    store::append(
        dir,
        FINDINGS_FILE,
        &MachineryFinding {
            id: id.clone(),
            mechanism: mechanism.to_string(),
            component: component.to_string(),
            claim: claim.to_string(),
            evidence: evidence.to_vec(),
            counter_evidence: counter_evidence.to_vec(),
            uncertainty: "authored by the reasoning engine; subject and causality unverified \
                          (it has misattributed subjects before — T-215)"
                .to_string(),
            affected: affected.to_string(),
            disposition: "observed".to_string(),
            corroborations: 0,
            created_at: now,
            updated_at: now,
            decided_by: String::new(),
        },
    )?;
    Ok(id)
}

/// A HUMAN closes a finding: `dismissed` or `accepted_by_human`, with the deciding handle
/// on the record. Refuses an unknown disposition, an anonymous decider, an unknown id, and
/// re-deciding a terminal finding — a decision stands until a human reopens the question
/// elsewhere; there is no machine path back.
pub fn decide(
    dir: &Path,
    id: &str,
    disposition: &str,
    decided_by: &str,
    now: i64,
) -> io::Result<Result<(), String>> {
    let disposition = disposition.trim();
    if !matches!(disposition, "dismissed" | "accepted_by_human") {
        return Ok(Err(format!(
            "'{disposition}' is not a terminal disposition (dismissed | accepted_by_human)"
        )));
    }
    let decided_by = decided_by.trim().to_lowercase();
    if decided_by.is_empty() {
        return Ok(Err(
            "a terminal transition is a human act — it carries the human's handle".into(),
        ));
    }
    let Some(f) = store::load_by_id::<MachineryFinding>(dir, FINDINGS_FILE, id)? else {
        return Ok(Err(format!("no finding '{id}'")));
    };
    if f.is_terminal() {
        return Ok(Err(format!(
            "finding '{id}' is already {} (by {})",
            f.disposition, f.decided_by
        )));
    }
    let mut f = f;
    f.disposition = disposition.to_string();
    f.decided_by = decided_by;
    f.updated_at = now;
    store::update_by_id(dir, FINDINGS_FILE, id, &f)?;
    Ok(Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct Temp(PathBuf);
    impl Temp {
        fn new(t: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "familiar_machinery_test_{}_{t}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Temp(p)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn seed(dir: &Path) -> String {
        observe(
            dir,
            "presence",
            "familiar|purged",
            "recurring purges destroy the temporal reference tree",
            &["obs-0001".to_string()],
            &["SF-1".to_string()],
            "multi-session observation",
            100,
        )
        .unwrap()
    }

    #[test]
    fn the_same_open_claim_corroborates_instead_of_duplicating() {
        let t = Temp::new("dedup");
        let id = seed(&t.0);
        let id2 = observe(
            &t.0,
            "presence",
            "familiar|purged",
            "purge loops again",
            &["obs-0002".to_string()],
            &[],
            "",
            200,
        )
        .unwrap();
        assert_eq!(id, id2, "one open finding per (mechanism, component)");
        let all = load(&t.0).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].disposition, "corroborated");
        assert_eq!(all[0].corroborations, 1);
        assert_eq!(
            all[0].evidence,
            vec!["obs-0001".to_string(), "obs-0002".to_string()],
            "evidence accumulates on the one finding"
        );
    }

    #[test]
    fn terminal_transitions_are_human_acts_only_and_final() {
        let t = Temp::new("terminal");
        let id = seed(&t.0);
        // Anonymous, unknown-disposition, unknown-id: all refuse.
        assert!(decide(&t.0, &id, "dismissed", "  ", 200).unwrap().is_err());
        assert!(decide(&t.0, &id, "pursued", "ian", 200).unwrap().is_err());
        assert!(decide(&t.0, "finding-9999", "dismissed", "ian", 200)
            .unwrap()
            .is_err());
        // A named human dismisses; the decision is on the record and final.
        decide(&t.0, &id, "dismissed", "Ian", 200).unwrap().unwrap();
        let f = &load(&t.0).unwrap()[0];
        assert_eq!(f.disposition, "dismissed");
        assert_eq!(f.decided_by, "ian");
        assert!(
            decide(&t.0, &id, "accepted_by_human", "ian", 300)
                .unwrap()
                .is_err(),
            "no path out of a terminal state"
        );
        // The claim re-derived AFTER dismissal mints fresh — re-derivation is signal.
        let id2 = observe(
            &t.0,
            "presence",
            "familiar|purged",
            "purge loops, still",
            &[],
            &[],
            "",
            400,
        )
        .unwrap();
        assert_ne!(id, id2);
        assert_eq!(load(&t.0).unwrap().len(), 2);
    }
}
