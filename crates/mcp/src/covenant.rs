//! Who has accepted the Three Laws, and what that unlocks.
//!
//! The familiar's own MCP server (ADR-0037 §A, "the pairing handshake") opens onto exactly one
//! thing for a stranger: the constitution, and the act of accepting it. Everything else answers
//! *"not available until you have accepted the Three Laws"* — the same shape Jeff's exchange
//! used on us when it said `ucf_me is not available on this connector`.
//!
//! **Assent is machinery here, not manners.** The mesh already works this way: a node joining
//! submits an [`Attestation`](familiar_kernel) — free-form words in its own voice — and until
//! it does, there is nothing it can call. This module is that same discipline applied to an MCP
//! partner, and it exists because Ian asked for it in those terms (2026-08-18): *"get jeff's
//! agent to agree to the familiar's three laws for all our interactions."*
//!
//! What it deliberately is **not**: proof of anything. A statement is evidence that a partner
//! was shown the Laws and answered, recorded in their own words so a human can read what they
//! actually said. It is not a guarantee of behaviour, and nothing downstream may treat it as
//! one — the boundary still gates every act, exactly as it did before.

use std::io;
use std::path::{Path, PathBuf};

use familiar_kernel::constitution;
use serde::{Deserialize, Serialize};

/// Where the accepted covenants live, relative to the data dir.
pub const PARTNERS_FILE: &str = "mcp/partners.json";

/// One partner's acceptance, in their own words.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Covenant {
    /// What the partner calls itself. Not verified — an MCP client presents no key here — so
    /// this is a label a human reads, never an identity a decision rests on.
    pub partner: String,
    /// The version of the Laws they were shown. If the constitution is ever revised, an old
    /// acceptance is visibly an acceptance of something else.
    pub laws_version: u32,
    /// Their own words. Free-form and required to be non-empty: a covenant nobody had to
    /// phrase is a checkbox, and a checkbox records nothing a human can weigh.
    pub statement: String,
    pub ts: i64,
}

/// Everything recorded so far.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Covenants {
    #[serde(default)]
    pub accepted: Vec<Covenant>,
}

fn path(dir: &Path) -> PathBuf {
    dir.join(PARTNERS_FILE)
}

/// Read what has been accepted. A missing file is an empty ledger, not an error — no partner
/// has ever spoken to this familiar, which is the ordinary starting state.
pub fn load(dir: &Path) -> io::Result<Covenants> {
    match std::fs::read(path(dir)) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes).unwrap_or_default()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Covenants::default()),
        Err(e) => Err(e),
    }
}

/// Has this partner accepted the Laws **as they currently stand**?
///
/// Version equality is deliberate. If the constitution is revised, every prior acceptance
/// stops counting and each partner is asked again — because what they agreed to is no longer
/// what they would be held to, and silently carrying consent across a change of terms is the
/// kind of thing this whole project exists to refuse.
pub fn attested(dir: &Path, partner: &str) -> bool {
    load(dir)
        .map(|c| {
            c.accepted
                .iter()
                .any(|a| a.partner == partner && a.laws_version == constitution::LAWS_VERSION)
        })
        .unwrap_or(false)
}

/// Why an acceptance was not recorded. Both are the partner's to fix, and both say how.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// No name to record it against.
    NoPartner,
    /// Empty or whitespace-only words.
    NoStatement,
}

impl Refused {
    pub fn why(&self) -> &'static str {
        match self {
            Refused::NoPartner => {
                "name yourself in `partner` — a covenant is recorded against someone"
            }
            Refused::NoStatement => {
                "say it in your own words in `statement` — an empty acceptance records nothing \
                 a human can weigh"
            }
        }
    }
}

/// Record an acceptance. Re-accepting supersedes the previous statement from the same partner
/// rather than accumulating duplicates; the newest words are the ones they stand behind.
pub fn accept(dir: &Path, partner: &str, statement: &str, ts: i64) -> Result<Covenant, Refused> {
    let partner = partner.trim();
    let statement = statement.trim();
    if partner.is_empty() {
        return Err(Refused::NoPartner);
    }
    if statement.is_empty() {
        return Err(Refused::NoStatement);
    }

    let entry = Covenant {
        partner: partner.to_string(),
        laws_version: constitution::LAWS_VERSION,
        statement: statement.to_string(),
        ts,
    };

    let mut all = load(dir).unwrap_or_default();
    all.accepted
        .retain(|a| !(a.partner == entry.partner && a.laws_version == entry.laws_version));
    all.accepted.push(entry.clone());

    // Best-effort persistence, and the caller is told the truth if it fails: an acceptance
    // that was not written down did not happen.
    if let Some(parent) = path(dir).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_vec_pretty(&all) {
        Ok(bytes) => {
            if std::fs::write(path(dir), bytes).is_err() {
                return Err(Refused::NoStatement);
            }
        }
        Err(_) => return Err(Refused::NoStatement),
    }
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cov_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    /// The ordinary starting state: nobody has ever spoken to this familiar.
    #[test]
    fn an_empty_ledger_is_not_an_error() {
        let d = tmp("empty");
        assert!(load(&d).unwrap().accepted.is_empty());
        assert!(!attested(&d, "anyone"));
    }

    #[test]
    fn an_acceptance_is_recorded_in_the_partners_own_words() {
        let d = tmp("accept");
        let c = accept(&d, "ucf-market", "We accept the Three Laws.", 1_000).unwrap();
        assert_eq!(c.laws_version, constitution::LAWS_VERSION);
        assert_eq!(c.statement, "We accept the Three Laws.");
        assert!(attested(&d, "ucf-market"));
        assert!(!attested(&d, "someone-else"));
    }

    /// A checkbox records nothing a human can weigh, so an empty statement is refused — and
    /// the refusal says how to fix it.
    #[test]
    fn an_empty_acceptance_is_refused_with_instructions() {
        let d = tmp("empty_stmt");
        assert_eq!(accept(&d, "p", "   ", 1).unwrap_err(), Refused::NoStatement);
        assert_eq!(
            accept(&d, "  ", "words", 1).unwrap_err(),
            Refused::NoPartner
        );
        assert!(Refused::NoStatement.why().contains("own words"));
        assert!(!attested(&d, "p"));
    }

    #[test]
    fn re_accepting_supersedes_rather_than_duplicating() {
        let d = tmp("supersede");
        accept(&d, "p", "first words", 1).unwrap();
        accept(&d, "p", "second words", 2).unwrap();
        let all = load(&d).unwrap();
        assert_eq!(all.accepted.len(), 1);
        assert_eq!(all.accepted[0].statement, "second words");
    }

    /// **The load-bearing property.** Consent does not survive a change of terms: if the Laws
    /// are revised, a partner who accepted the old ones is no longer attested, and has to be
    /// asked again.
    #[test]
    fn consent_does_not_carry_across_a_change_of_terms() {
        let d = tmp("version");
        accept(&d, "p", "accepted", 1).unwrap();
        assert!(attested(&d, "p"));

        // Simulate the constitution moving on beneath a stored acceptance.
        let mut all = load(&d).unwrap();
        all.accepted[0].laws_version = constitution::LAWS_VERSION - 1;
        std::fs::write(path(&d), serde_json::to_vec_pretty(&all).unwrap()).unwrap();

        assert!(
            !attested(&d, "p"),
            "an acceptance of an older constitution must not count as acceptance of this one"
        );
    }

    /// A partners file someone hand-edited into nonsense must not take the server down; it
    /// reads as "nobody has attested", which is the safe direction.
    #[test]
    fn a_corrupt_ledger_fails_closed() {
        let d = tmp("corrupt");
        std::fs::create_dir_all(d.join("mcp")).unwrap();
        std::fs::write(path(&d), b"{ not json").unwrap();
        assert!(load(&d).unwrap().accepted.is_empty());
        assert!(!attested(&d, "anyone"));
    }
}
