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

use crate::partner::PartnerContext;

/// Where the accepted covenants live, relative to the data dir.
pub const PARTNERS_FILE: &str = "mcp/partners.json";

/// How many distinct partners this ledger will hold.
///
/// `attest` is the one tool here that writes, and once the seam is exposed beyond this machine
/// (see [`crate::serving`]) it is a write reachable by strangers. A ledger with no ceiling is a
/// disk-filling vector wearing the costume of a covenant. Sixty-four is far more partners than
/// a household familiar will ever have, and small enough that the file stays something a human
/// can read in one sitting — which is the whole reason it is a file and not a database.
///
/// Re-accepting is never refused: a partner already in the ledger is updating their own words,
/// not consuming a new slot.
pub const MAX_PARTNERS: usize = 64;

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

/// Acceptance bound to a transport-authenticated principal. Kept as a separate record kind so
/// old caller-label covenants can never be mistaken for authority-bearing consent on upgrade.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrincipalCovenant {
    pub principal: String,
    pub credential_fingerprint: String,
    pub alias_snapshot: String,
    pub laws_version: u32,
    pub statement: String,
    pub ts: i64,
}

/// Everything recorded so far.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Covenants {
    #[serde(default)]
    pub accepted: Vec<Covenant>,
    #[serde(default)]
    pub principals: Vec<PrincipalCovenant>,
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

/// Has this authenticated principal accepted the Laws as they currently stand? Credential
/// fingerprints are retained as audit snapshots, while consent binds to the stable principal:
/// an explicit human credential re-binding does not silently create a new identity.
pub fn principal_attested(dir: &Path, context: &PartnerContext) -> bool {
    load(dir)
        .map(|c| {
            c.principals.iter().any(|a| {
                a.principal == context.principal
                    && a.laws_version == constitution::LAWS_VERSION
            })
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
    /// The ledger is full of other partners.
    LedgerFull,
    /// The authenticated context did not carry a stable principal.
    NoPrincipal,
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
            Refused::LedgerFull => {
                "this familiar is already holding as many covenants as it will keep; its human \
                 has to retire one before another is recorded"
            }
            Refused::NoPrincipal => {
                "the transport did not establish a partner principal; a caller label is not identity"
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
    let known = all.accepted.iter().any(|a| a.partner == entry.partner);
    // A ceiling on strangers, never on someone already here restating their own acceptance.
    if !known && all.accepted.len() + all.principals.len() >= MAX_PARTNERS {
        return Err(Refused::LedgerFull);
    }
    all.accepted
        .retain(|a| !(a.partner == entry.partner && a.laws_version == entry.laws_version));
    all.accepted.push(entry.clone());

    persist(dir, &all)?;
    Ok(entry)
}

/// Record the current authenticated principal's own acceptance. There is deliberately no
/// `partner` argument: identity has already been established by the credential.
pub fn accept_principal(
    dir: &Path,
    context: &PartnerContext,
    statement: &str,
    ts: i64,
) -> Result<PrincipalCovenant, Refused> {
    if context.principal.trim().is_empty() {
        return Err(Refused::NoPrincipal);
    }
    let statement = statement.trim();
    if statement.is_empty() {
        return Err(Refused::NoStatement);
    }
    let entry = PrincipalCovenant {
        principal: context.principal.clone(),
        credential_fingerprint: context.credential_fingerprint.clone(),
        alias_snapshot: context.alias.clone(),
        laws_version: constitution::LAWS_VERSION,
        statement: statement.to_string(),
        ts,
    };
    let mut all = load(dir).unwrap_or_default();
    let known = all
        .principals
        .iter()
        .any(|a| a.principal == context.principal);
    if !known && all.accepted.len() + all.principals.len() >= MAX_PARTNERS {
        return Err(Refused::LedgerFull);
    }
    all.principals.retain(|a| {
        !(a.principal == entry.principal && a.laws_version == entry.laws_version)
    });
    all.principals.push(entry.clone());
    persist(dir, &all)?;
    Ok(entry)
}

fn persist(dir: &Path, all: &Covenants) -> Result<(), Refused> {
    let Some(parent) = path(dir).parent() else {
        return Err(Refused::NoStatement);
    };
    std::fs::create_dir_all(parent).map_err(|_| Refused::NoStatement)?;
    let bytes = serde_json::to_vec_pretty(all).map_err(|_| Refused::NoStatement)?;
    let tmp = parent.join(format!(".partners-{}.tmp", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(|_| Refused::NoStatement)?;
    std::fs::rename(tmp, path(dir)).map_err(|_| Refused::NoStatement)
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

    #[test]
    fn a_principal_covenant_is_bound_and_old_labels_are_not_promoted() {
        let d = tmp("principal");
        accept(&d, "same-label", "old speech covenant", 1).unwrap();
        let context = PartnerContext {
            principal: "principal-a".into(),
            credential_fingerprint: "abc123".into(),
            alias: "same-label".into(),
        };
        assert!(attested(&d, "same-label"));
        assert!(
            !principal_attested(&d, &context),
            "a self-asserted covenant must not become principal consent"
        );
        let accepted = accept_principal(&d, &context, "bound words", 2).unwrap();
        assert_eq!(accepted.principal, "principal-a");
        assert_eq!(accepted.credential_fingerprint, "abc123");
        assert!(principal_attested(&d, &context));
    }

    #[test]
    fn two_principals_with_one_alias_do_not_share_consent() {
        let d = tmp("principal_distinct");
        let a = PartnerContext {
            principal: "principal-a".into(),
            credential_fingerprint: "a".into(),
            alias: "agent".into(),
        };
        let b = PartnerContext {
            principal: "principal-b".into(),
            credential_fingerprint: "b".into(),
            alias: "agent".into(),
        };
        accept_principal(&d, &a, "yes", 1).unwrap();
        assert!(principal_attested(&d, &a));
        assert!(!principal_attested(&d, &b));
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

    /// Once exposed, `accept` is a write a stranger can reach. It must have a ceiling — and
    /// the ceiling must never fall on someone already here restating their own words.
    #[test]
    fn the_ledger_has_a_ceiling_that_spares_existing_partners() {
        let d = tmp("ceiling");
        for i in 0..MAX_PARTNERS {
            accept(&d, &format!("partner-{i}"), "yes", 1).unwrap();
        }
        assert_eq!(
            accept(&d, "one-too-many", "yes", 1).unwrap_err(),
            Refused::LedgerFull
        );
        // Someone already in the ledger is updating, not consuming a slot.
        accept(&d, "partner-0", "revised words", 2).unwrap();
        let all = load(&d).unwrap();
        assert_eq!(all.accepted.len(), MAX_PARTNERS);
        assert!(all
            .accepted
            .iter()
            .any(|a| a.partner == "partner-0" && a.statement == "revised words"));
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
