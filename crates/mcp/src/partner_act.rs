//! Append-only truth for every authenticated, rate-admitted partner interaction.
//!
//! Grants and proposals are folds over this stream. There is no update/delete API. The table
//! keeps just enough indexed metadata beside the serialized event to make idempotency and
//! terminal transitions one SQLite transaction rather than a load/check/append race.

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;

use crate::partner::{self, PartnerContext};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum PartnerActor {
    Partner(String),
    Human(String),
    Clock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartnerOperation {
    GrantRequest,
    GrantDecision,
    GrantRevocation,
    GrantExpiry,
    Proposal,
    ProposalDecision,
}

impl PartnerOperation {
    fn label(self) -> &'static str {
        match self {
            Self::GrantRequest => "grant_request",
            Self::GrantDecision => "grant_decision",
            Self::GrantRevocation => "grant_revocation",
            Self::GrantExpiry => "grant_expiry",
            Self::Proposal => "proposal",
            Self::ProposalDecision => "proposal_decision",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartnerOutcome {
    Refused,
    Proposed,
    Completed,
    Failed,
    Reverted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    Accepted,
    SchemaInvalid,
    CovenantMissing,
    UnknownClass,
    UnknownOperation,
    BoundsInvalid,
    IdempotencyConflict,
    RequestLimit,
    ProposalLimit,
    GrantMissing,
    GrantInactive,
    WrongPrincipal,
    BoundaryClosed,
    SurfaceMismatch,
    TransitionConflict,
    HumanGranted,
    HumanDeclined,
    HumanRevoked,
    HumanRefusedProposal,
    Expired,
    ProposalStored,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PartnerActBody {
    GrantRequested {
        request_id: String,
        request_key: String,
        class_id: String,
        requested_operations: Value,
        requested_duration_seconds: Option<i64>,
        reason: Option<String>,
    },
    GrantGranted {
        request_id: String,
        grant_id: String,
        surface: String,
        allowed_operations: Value,
        epoch_nonce: String,
        granted_by: String,
        granted_at: i64,
        expires_at: i64,
    },
    GrantDeclined {
        request_id: String,
        declined_by: String,
    },
    GrantRevoked {
        grant_id: String,
        surface: String,
        revoked_by: String,
    },
    GrantExpired {
        grant_id: String,
        surface: String,
    },
    ProposalSubmitted {
        proposal_id: String,
        proposal_key: String,
        grant_id: String,
        class_id: String,
        operation: String,
        parameters: Value,
        reason: Option<String>,
    },
    ProposalRefused {
        #[serde(default)]
        proposal_id: Option<String>,
        proposal_key: Option<String>,
        handle_fingerprint: Option<String>,
    },
    ProposalWithdrawn {
        proposal_id: String,
        withdrawn_by: String,
    },
    Refusal {
        idempotency_key: Option<String>,
        subject_ref: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartnerAct {
    pub id: String,
    pub at: i64,
    pub principal: String,
    pub credential: String,
    pub alias_snapshot: String,
    pub actor: PartnerActor,
    pub operation: PartnerOperation,
    pub outcome: PartnerOutcome,
    pub reason_code: ReasonCode,
    pub correlation: String,
    pub body: PartnerActBody,
}

impl PartnerAct {
    pub fn partner(
        context: &PartnerContext,
        at: i64,
        operation: PartnerOperation,
        outcome: PartnerOutcome,
        reason_code: ReasonCode,
        correlation: String,
        body: PartnerActBody,
    ) -> io::Result<Self> {
        Ok(Self {
            id: partner::random_id("partner-act")?,
            at,
            principal: context.principal.clone(),
            credential: context.credential_fingerprint.clone(),
            alias_snapshot: context.alias.clone(),
            actor: PartnerActor::Partner(context.principal.clone()),
            operation,
            outcome,
            reason_code,
            correlation,
            body,
        })
    }

    pub fn by_human(mut self, human: &str) -> Self {
        self.actor = PartnerActor::Human(human.to_string());
        self
    }

    pub fn clock(
        context: &PartnerContext,
        at: i64,
        correlation: String,
        body: PartnerActBody,
    ) -> io::Result<Self> {
        let mut event = Self::partner(
            context,
            at,
            PartnerOperation::GrantExpiry,
            PartnerOutcome::Completed,
            ReasonCode::Expired,
            correlation,
            body,
        )?;
        event.actor = PartnerActor::Clock;
        Ok(event)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum IdempotentAppend {
    Inserted(PartnerAct),
    Replay(PartnerAct),
    Conflict(PartnerAct),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransitionAppend {
    Inserted(PartnerAct),
    Existing(PartnerAct),
}

pub fn append_idempotent(
    dir: &Path,
    event: &PartnerAct,
    key: &str,
    payload_hash: &str,
    conflict: &PartnerAct,
) -> io::Result<IdempotentAppend> {
    let mut conn = connection(dir)?;
    let tx = conn.transaction().map_err(sqlite)?;
    let previous: Option<(String, String)> = tx
        .query_row(
            "SELECT data,payload_hash FROM partner_acts \
             WHERE principal=?1 AND operation=?2 AND idempotency_key=?3 AND original=1 \
             LIMIT 1",
            rusqlite::params![event.principal, event.operation.label(), key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(sqlite)?;
    let result = match previous {
        Some((data, previous_hash)) if previous_hash == payload_hash => {
            IdempotentAppend::Replay(decode(&data)?)
        }
        Some(_) => {
            insert(&tx, conflict, Some(key), Some(payload_hash), false, None)?;
            IdempotentAppend::Conflict(conflict.clone())
        }
        None => {
            insert(&tx, event, Some(key), Some(payload_hash), true, None)?;
            IdempotentAppend::Inserted(event.clone())
        }
    };
    tx.commit().map_err(sqlite)?;
    Ok(result)
}

/// Indexed read used to make capacity/expiry checks idempotency-aware: an exact replay returns
/// its original receipt even if the principal has since reached a ceiling or the grant expired.
pub fn idempotent_original(
    dir: &Path,
    principal: &str,
    operation: PartnerOperation,
    key: &str,
) -> io::Result<Option<(PartnerAct, String)>> {
    let conn = connection(dir)?;
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT data,payload_hash FROM partner_acts \
             WHERE principal=?1 AND operation=?2 AND idempotency_key=?3 AND original=1 \
             LIMIT 1",
            rusqlite::params![principal, operation.label(), key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(sqlite)?;
    row.map(|(data, hash)| Ok((decode(&data)?, hash)))
        .transpose()
}

/// Append one terminal transition exactly once. `transition_key` is server-authored (for
/// example `grant-decision:<request-id>`), so a partner cannot collide unrelated records.
pub fn append_transition(
    dir: &Path,
    event: &PartnerAct,
    transition_key: &str,
) -> io::Result<TransitionAppend> {
    let mut conn = connection(dir)?;
    let tx = conn.transaction().map_err(sqlite)?;
    let previous: Option<String> = tx
        .query_row(
            "SELECT data FROM partner_acts WHERE transition_key=?1 LIMIT 1",
            [transition_key],
            |row| row.get(0),
        )
        .optional()
        .map_err(sqlite)?;
    let result = if let Some(data) = previous {
        TransitionAppend::Existing(decode(&data)?)
    } else {
        insert(&tx, event, None, None, false, Some(transition_key))?;
        TransitionAppend::Inserted(event.clone())
    };
    tx.commit().map_err(sqlite)?;
    Ok(result)
}

pub fn append(dir: &Path, event: &PartnerAct) -> io::Result<()> {
    let conn = connection(dir)?;
    insert(&conn, event, None, None, false, None)
}

pub fn load(dir: &Path) -> io::Result<Vec<PartnerAct>> {
    let conn = connection(dir)?;
    let mut stmt = conn
        .prepare("SELECT data FROM partner_acts ORDER BY seq")
        .map_err(sqlite)?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(sqlite)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(decode(&row.map_err(sqlite)?)?);
    }
    validate_sequence(&out)?;
    Ok(out)
}

/// Fail closed on a syntactically valid stream whose transitions cannot have occurred through
/// this module. Folds must never turn corrupt authority history into an empty/default view.
fn validate_sequence(events: &[PartnerAct]) -> io::Result<()> {
    let invalid = |what: &str| io::Error::new(io::ErrorKind::InvalidData, what.to_string());
    let mut requests = BTreeMap::<String, String>::new();
    let mut request_decisions = BTreeSet::<String>::new();
    let mut grants = BTreeMap::<String, (String, String)>::new();
    let mut grant_terminals = BTreeSet::<String>::new();
    let mut proposals = BTreeMap::<String, String>::new();
    let mut proposal_terminals = BTreeSet::<String>::new();

    for event in events {
        match &event.body {
            PartnerActBody::GrantRequested { request_id, .. } => {
                if requests
                    .insert(request_id.clone(), event.principal.clone())
                    .is_some()
                {
                    return Err(invalid("duplicate grant request transition"));
                }
            }
            PartnerActBody::GrantGranted {
                request_id,
                grant_id,
                surface,
                ..
            } => {
                if requests.get(request_id) != Some(&event.principal) {
                    return Err(invalid("grant references a missing or foreign request"));
                }
                if !request_decisions.insert(request_id.clone()) {
                    return Err(invalid("grant request has multiple terminal decisions"));
                }
                if grants
                    .insert(grant_id.clone(), (event.principal.clone(), surface.clone()))
                    .is_some()
                {
                    return Err(invalid("duplicate grant transition"));
                }
            }
            PartnerActBody::GrantDeclined { request_id, .. } => {
                if requests.get(request_id) != Some(&event.principal) {
                    return Err(invalid("decline references a missing or foreign request"));
                }
                if !request_decisions.insert(request_id.clone()) {
                    return Err(invalid("grant request has multiple terminal decisions"));
                }
            }
            PartnerActBody::GrantRevoked {
                grant_id, surface, ..
            }
            | PartnerActBody::GrantExpired { grant_id, surface } => {
                if grants.get(grant_id) != Some(&(event.principal.clone(), surface.clone())) {
                    return Err(invalid(
                        "grant terminal references missing or changed authority",
                    ));
                }
                if !grant_terminals.insert(grant_id.clone()) {
                    return Err(invalid("grant has multiple terminal transitions"));
                }
            }
            PartnerActBody::ProposalSubmitted {
                proposal_id,
                grant_id,
                ..
            } => {
                if grants.get(grant_id).map(|(principal, _)| principal) != Some(&event.principal)
                    || grant_terminals.contains(grant_id)
                {
                    return Err(invalid("proposal references missing or inactive authority"));
                }
                if proposals
                    .insert(proposal_id.clone(), event.principal.clone())
                    .is_some()
                {
                    return Err(invalid("duplicate proposal transition"));
                }
            }
            PartnerActBody::ProposalRefused {
                proposal_id: Some(proposal_id),
                ..
            }
            | PartnerActBody::ProposalWithdrawn { proposal_id, .. } => {
                if proposals.get(proposal_id) != Some(&event.principal) {
                    return Err(invalid("proposal terminal references a missing proposal"));
                }
                if !proposal_terminals.insert(proposal_id.clone()) {
                    return Err(invalid("proposal has multiple terminal transitions"));
                }
            }
            PartnerActBody::ProposalRefused {
                proposal_id: None, ..
            }
            | PartnerActBody::Refusal { .. } => {}
        }
    }
    Ok(())
}

pub fn payload_hash<T: Serialize>(value: &T) -> io::Result<String> {
    let bytes = serde_json::to_vec(value).map_err(io::Error::other)?;
    Ok(partner::hex(&Sha256::digest(bytes)))
}

pub fn opaque_fingerprint(value: &str) -> String {
    partner::hex(&Sha256::digest(value.as_bytes()))
}

fn connection(dir: &Path) -> io::Result<Connection> {
    std::fs::create_dir_all(dir)?;
    let conn = Connection::open(dir.join(familiar_kernel::store::DB_FILE)).map_err(sqlite)?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000; PRAGMA synchronous=NORMAL;
         CREATE TABLE IF NOT EXISTS partner_acts (
             seq INTEGER PRIMARY KEY AUTOINCREMENT,
             event_id TEXT NOT NULL UNIQUE,
             principal TEXT NOT NULL,
             operation TEXT NOT NULL,
             idempotency_key TEXT,
             payload_hash TEXT,
             original INTEGER NOT NULL DEFAULT 0,
             transition_key TEXT,
             data TEXT NOT NULL
         );
         CREATE UNIQUE INDEX IF NOT EXISTS partner_acts_original_key
             ON partner_acts(principal,operation,idempotency_key)
             WHERE original=1 AND idempotency_key IS NOT NULL;
         CREATE UNIQUE INDEX IF NOT EXISTS partner_acts_transition
             ON partner_acts(transition_key) WHERE transition_key IS NOT NULL;",
    )
    .map_err(sqlite)?;
    Ok(conn)
}

fn insert(
    conn: &Connection,
    event: &PartnerAct,
    idempotency_key: Option<&str>,
    payload_hash: Option<&str>,
    original: bool,
    transition_key: Option<&str>,
) -> io::Result<()> {
    let data = serde_json::to_string(event).map_err(io::Error::other)?;
    conn.execute(
        "INSERT INTO partner_acts
         (event_id,principal,operation,idempotency_key,payload_hash,original,transition_key,data)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        rusqlite::params![
            event.id,
            event.principal,
            event.operation.label(),
            idempotency_key,
            payload_hash,
            if original { 1 } else { 0 },
            transition_key,
            data
        ],
    )
    .map_err(sqlite)?;
    Ok(())
}

fn decode(data: &str) -> io::Result<PartnerAct> {
    serde_json::from_str(data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn sqlite(error: rusqlite::Error) -> io::Error {
    io::Error::other(format!("partner-act sqlite: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("partner_act_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn context() -> PartnerContext {
        PartnerContext {
            principal: "principal-a".into(),
            credential_fingerprint: "key-a".into(),
            alias: "Workshop agent".into(),
        }
    }

    fn request(id: &str, key: &str) -> PartnerAct {
        PartnerAct::partner(
            &context(),
            1,
            PartnerOperation::GrantRequest,
            PartnerOutcome::Proposed,
            ReasonCode::Accepted,
            id.into(),
            PartnerActBody::GrantRequested {
                request_id: id.into(),
                request_key: key.into(),
                class_id: "switchable.reversible/v1".into(),
                requested_operations: serde_json::json!({}),
                requested_duration_seconds: None,
                reason: None,
            },
        )
        .unwrap()
    }

    #[test]
    fn identical_idempotency_replays_and_mismatch_appends_a_refusal() {
        let dir = temp("idem");
        let first = request("request-a", "same");
        let conflict = PartnerAct::partner(
            &context(),
            2,
            PartnerOperation::GrantRequest,
            PartnerOutcome::Refused,
            ReasonCode::IdempotencyConflict,
            "request-conflict".into(),
            PartnerActBody::Refusal {
                idempotency_key: Some("same".into()),
                subject_ref: None,
            },
        )
        .unwrap();
        assert!(matches!(
            append_idempotent(&dir, &first, "same", "hash-a", &conflict).unwrap(),
            IdempotentAppend::Inserted(_)
        ));
        assert!(matches!(
            append_idempotent(&dir, &request("new-id", "same"), "same", "hash-a", &conflict)
                .unwrap(),
            IdempotentAppend::Replay(previous) if previous.id == first.id
        ));
        assert!(matches!(
            append_idempotent(&dir, &request("other", "same"), "same", "hash-b", &conflict)
                .unwrap(),
            IdempotentAppend::Conflict(_)
        ));
        assert_eq!(load(&dir).unwrap().len(), 2);
    }

    #[test]
    fn a_terminal_transition_is_compare_and_insert_not_load_then_append() {
        let dir = temp("transition");
        append(&dir, &request("request-a", "key-a")).unwrap();
        let event = PartnerAct::partner(
            &context(),
            2,
            PartnerOperation::GrantDecision,
            PartnerOutcome::Refused,
            ReasonCode::HumanDeclined,
            "request-a".into(),
            PartnerActBody::GrantDeclined {
                request_id: "request-a".into(),
                declined_by: "ian".into(),
            },
        )
        .unwrap()
        .by_human("ian");
        assert!(matches!(
            append_transition(&dir, &event, "grant-decision:request-a").unwrap(),
            TransitionAppend::Inserted(_)
        ));
        assert!(matches!(
            append_transition(&dir, &event, "grant-decision:request-a").unwrap(),
            TransitionAppend::Existing(_)
        ));
        assert_eq!(load(&dir).unwrap().len(), 2);
    }

    #[test]
    fn private_surface_truth_is_local_and_append_only() {
        let dir = temp("private");
        append(&dir, &request("request-a", "key-a")).unwrap();
        let event = PartnerAct::partner(
            &context(),
            2,
            PartnerOperation::GrantDecision,
            PartnerOutcome::Completed,
            ReasonCode::HumanGranted,
            "request-a".into(),
            PartnerActBody::GrantGranted {
                request_id: "request-a".into(),
                grant_id: "grant-a".into(),
                surface: "ians-secret-lamp".into(),
                allowed_operations: serde_json::json!({}),
                epoch_nonce: "nonce".into(),
                granted_by: "ian".into(),
                granted_at: 2,
                expires_at: 3,
            },
        )
        .unwrap()
        .by_human("ian");
        append(&dir, &event).unwrap();
        assert!(serde_json::to_string(&load(&dir).unwrap())
            .unwrap()
            .contains("ians-secret-lamp"));
        let conn = connection(&dir).unwrap();
        let updates: i64 = conn
            .query_row("SELECT COUNT(*) FROM partner_acts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(updates, 2);
    }

    #[test]
    fn impossible_transition_history_surfaces_instead_of_folding_away() {
        let dir = temp("impossible");
        let event = PartnerAct::partner(
            &context(),
            2,
            PartnerOperation::GrantDecision,
            PartnerOutcome::Completed,
            ReasonCode::HumanGranted,
            "missing-request".into(),
            PartnerActBody::GrantGranted {
                request_id: "missing-request".into(),
                grant_id: "grant-a".into(),
                surface: "ians-secret-lamp".into(),
                allowed_operations: serde_json::json!({}),
                epoch_nonce: "nonce".into(),
                granted_by: "ian".into(),
                granted_at: 2,
                expires_at: 3,
            },
        )
        .unwrap()
        .by_human("ian");
        append(&dir, &event).unwrap();
        assert_eq!(load(&dir).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }
}
