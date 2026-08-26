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
    /// A partner read a bound surface within an active grant (rung 4).
    Observe,
    /// A partner ran a bound act within an active grant (rung 5) — the execution edge.
    Invoke,
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
            Self::Observe => "observe",
            Self::Invoke => "invoke",
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
    /// A rung-4 read completed against a bound surface.
    Observed,
    /// A rung-5 act ran against a bound surface (the execution edge).
    Invoked,
    /// The executor refused or failed the surface I/O (shut `allow_actuate`, missing
    /// surface, tool failure) — the door did its authority checks, the primitive said no.
    ExecutionRefused,
    /// No executor is wired at this door — observe/invoke fail closed by construction.
    ExecutorUnavailable,
    /// A reserved act whose settlement was never recorded — the reserving process died, or
    /// its settlement append failed. The outcome is genuinely unknown: an exact replay
    /// refuses with this code and the device is never run again.
    OutcomeUnrecorded,
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
        /// The authority contract snapshotted at the human's decision (ADR-0044), so a later
        /// declaration edit cannot silently change what an active grant means or permits:
        /// the abstract→local role map, the affected-subject class, and the per-grant invoke
        /// rate. `#[serde(default)]` so grants minted before this shape still load (they carry
        /// no roles → the surface's declared roles are re-read, and the conservative default
        /// rate applies).
        #[serde(default)]
        roles: std::collections::BTreeMap<String, String>,
        #[serde(default)]
        affected_subject: String,
        #[serde(default)]
        max_invokes_per_hour: i64,
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
    /// A rung-4/5 effect RESERVED before the executor runs (ADR-0044 immediate-revocation +
    /// never-lose-an-act): the durable intent. Appended while the grant is live; the physical
    /// act happens after. `surface`/`operation`/`parameters`/`label` are the private local
    /// resolution and are NEVER serialized to a partner. `kind` is "observe" or "invoke".
    EffectReserved {
        effect_id: String,
        effect_kind: String,
        grant_id: String,
        surface: String,
        operation: String,
        parameters: Value,
        /// The resolved local act label (invoke only; empty for observe).
        #[serde(default)]
        label: String,
    },
    /// The typed outcome of a reserved effect, appended after the executor returns. `outcome`
    /// is "completed" or "failed". A reservation with no settlement is the explicit recovery
    /// state — a physical act whose outcome could not be persisted, never a vanished act.
    EffectSettled {
        effect_id: String,
        outcome: String,
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
    // effect_id -> (principal, reserved operation): a settlement must match BOTH — the same
    // principal and the operation the reservation declared (codex round 3: an observe
    // reservation settled as an invoke is an impossible history and must fail closed).
    let mut effects = BTreeMap::<String, (String, PartnerOperation)>::new();
    let mut effect_settlements = BTreeSet::<String>::new();

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
            // A reserved effect must reference a grant that exists, is owned by the same
            // principal, and had NOT terminated at this point in the stream — corrupt effect
            // history cannot be folded into a clean view. A reservation is not itself terminal
            // (a grant survives being observed or invoked); it registers an effect_id that a
            // later settlement may reference exactly once.
            PartnerActBody::EffectReserved {
                effect_id,
                effect_kind,
                grant_id,
                surface,
                ..
            } => {
                if grants.get(grant_id) != Some(&(event.principal.clone(), surface.clone()))
                    || grant_terminals.contains(grant_id)
                {
                    return Err(invalid("effect references missing or inactive authority"));
                }
                let operation = match effect_kind.as_str() {
                    "invoke" => PartnerOperation::Invoke,
                    "observe" => PartnerOperation::Observe,
                    _ => return Err(invalid("effect reservation carries an unknown kind")),
                };
                if event.operation != operation {
                    return Err(invalid(
                        "effect reservation operation does not match its kind",
                    ));
                }
                if effects
                    .insert(effect_id.clone(), (event.principal.clone(), operation))
                    .is_some()
                {
                    return Err(invalid("duplicate effect reservation"));
                }
            }
            // A settlement must reference an existing reservation by the same principal, carry
            // the operation that reservation declared with a typed outcome matching its recorded
            // one, and settle it at most once. It carries no grant-liveness requirement: the act
            // already ran, and recording its honest outcome must always be valid — even after a
            // revoke.
            PartnerActBody::EffectSettled { effect_id, outcome } => {
                match effects.get(effect_id) {
                    Some((principal, operation))
                        if principal == &event.principal && *operation == event.operation => {}
                    Some((principal, _)) if principal == &event.principal => {
                        return Err(invalid(
                            "effect settled under a different operation than it reserved",
                        ));
                    }
                    _ => {
                        return Err(invalid(
                            "effect settlement references a missing reservation",
                        ));
                    }
                }
                let outcome_matches = matches!(
                    (outcome.as_str(), event.outcome),
                    ("completed", PartnerOutcome::Completed) | ("failed", PartnerOutcome::Failed)
                );
                if !outcome_matches {
                    return Err(invalid(
                        "effect settlement outcome does not match its typed event",
                    ));
                }
                if !effect_settlements.insert(effect_id.clone()) {
                    return Err(invalid("effect settled more than once"));
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
                roles: std::collections::BTreeMap::new(),
                affected_subject: String::new(),
                max_invokes_per_hour: 0,
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
                roles: std::collections::BTreeMap::new(),
                affected_subject: String::new(),
                max_invokes_per_hour: 0,
            },
        )
        .unwrap()
        .by_human("ian");
        append(&dir, &event).unwrap();
        assert_eq!(load(&dir).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }

    // ---- The effect stream fails closed on operation/outcome mismatches (codex round 3) ----

    fn grant_event(request_id: &str, grant_id: &str) -> PartnerAct {
        PartnerAct::partner(
            &context(),
            2,
            PartnerOperation::GrantDecision,
            PartnerOutcome::Completed,
            ReasonCode::HumanGranted,
            request_id.into(),
            PartnerActBody::GrantGranted {
                request_id: request_id.into(),
                grant_id: grant_id.into(),
                surface: "ians-secret-lamp".into(),
                allowed_operations: serde_json::json!({}),
                epoch_nonce: "nonce".into(),
                granted_by: "ian".into(),
                granted_at: 2,
                expires_at: 300,
                roles: std::collections::BTreeMap::new(),
                affected_subject: String::new(),
                max_invokes_per_hour: 0,
            },
        )
        .unwrap()
        .by_human("ian")
    }

    fn reserved(
        effect_id: &str,
        grant_id: &str,
        kind: &str,
        operation: PartnerOperation,
    ) -> PartnerAct {
        PartnerAct::partner(
            &context(),
            3,
            operation,
            PartnerOutcome::Proposed,
            ReasonCode::Observed,
            effect_id.into(),
            PartnerActBody::EffectReserved {
                effect_id: effect_id.into(),
                effect_kind: kind.into(),
                grant_id: grant_id.into(),
                surface: "ians-secret-lamp".into(),
                operation: "set_state".into(),
                parameters: serde_json::json!({}),
                label: String::new(),
            },
        )
        .unwrap()
    }

    fn settled(
        effect_id: &str,
        operation: PartnerOperation,
        outcome: PartnerOutcome,
        recorded: &str,
    ) -> PartnerAct {
        PartnerAct::partner(
            &context(),
            4,
            operation,
            outcome,
            ReasonCode::Invoked,
            effect_id.into(),
            PartnerActBody::EffectSettled {
                effect_id: effect_id.into(),
                outcome: recorded.into(),
            },
        )
        .unwrap()
    }

    #[test]
    fn an_observe_reservation_settled_as_an_invoke_fails_closed() {
        let dir = temp("cross_settle");
        append(&dir, &request("request-a", "key")).unwrap();
        append(&dir, &grant_event("request-a", "grant-a")).unwrap();
        append(
            &dir,
            &reserved("effect-a", "grant-a", "observe", PartnerOperation::Observe),
        )
        .unwrap();
        append(
            &dir,
            &settled(
                "effect-a",
                PartnerOperation::Invoke,
                PartnerOutcome::Completed,
                "completed",
            ),
        )
        .unwrap();
        assert_eq!(load(&dir).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn a_settlement_whose_typed_outcome_disagrees_with_its_record_fails_closed() {
        let dir = temp("outcome_mismatch");
        append(&dir, &request("request-a", "key")).unwrap();
        append(&dir, &grant_event("request-a", "grant-a")).unwrap();
        append(
            &dir,
            &reserved("effect-a", "grant-a", "invoke", PartnerOperation::Invoke),
        )
        .unwrap();
        append(
            &dir,
            &settled(
                "effect-a",
                PartnerOperation::Invoke,
                PartnerOutcome::Failed,
                "completed",
            ),
        )
        .unwrap();
        assert_eq!(load(&dir).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn a_reservation_whose_operation_disagrees_with_its_kind_fails_closed() {
        let dir = temp("kind_mismatch");
        append(&dir, &request("request-a", "key")).unwrap();
        append(&dir, &grant_event("request-a", "grant-a")).unwrap();
        append(
            &dir,
            &reserved("effect-a", "grant-a", "observe", PartnerOperation::Invoke),
        )
        .unwrap();
        assert_eq!(load(&dir).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn a_matched_settlement_still_loads() {
        let dir = temp("matched_settle");
        append(&dir, &request("request-a", "key")).unwrap();
        append(&dir, &grant_event("request-a", "grant-a")).unwrap();
        append(
            &dir,
            &reserved("effect-a", "grant-a", "invoke", PartnerOperation::Invoke),
        )
        .unwrap();
        append(
            &dir,
            &settled(
                "effect-a",
                PartnerOperation::Invoke,
                PartnerOutcome::Completed,
                "completed",
            ),
        )
        .unwrap();
        assert_eq!(load(&dir).unwrap().len(), 4);
    }
}
