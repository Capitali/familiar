//! Rung 3 of ADR-0044: ask, grant, and propose — never observe or invoke.
//!
//! A partner requests a repo-authored class and abstract operation bounds. A named local human
//! may bind one private surface and narrower bounds. The partner receives only an opaque,
//! principal-bound handle. `propose` validates a desired effect and appends it for human
//! consideration; this module has no actuator, observation, command, worldview, or LLM edge.

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;

use crate::partner::{self, HumanDecisionContext, PartnerContext};
use crate::partner_act::{
    self, IdempotentAppend, PartnerAct, PartnerActBody, PartnerOperation, PartnerOutcome,
    ReasonCode, TransitionAppend,
};

pub const HANDLE_KEY_FILE: &str = "mcp/grant-handle.key";
pub const MAX_REASON_BYTES: usize = 512;
pub const MAX_IDEMPOTENCY_BYTES: usize = 64;
pub const MAX_OPEN_REQUESTS: usize = 16;
pub const MAX_OPEN_PROPOSALS: usize = 64;
pub const MIN_GRANT_SECONDS: i64 = 60;
pub const MAX_GRANT_SECONDS: i64 = 30 * 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ParameterBound {
    Enum { values: Vec<String> },
    Number { min: f64, max: f64 },
}

pub type ParameterBounds = BTreeMap<String, ParameterBound>;
pub type OperationBounds = BTreeMap<String, ParameterBounds>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantRequestInput {
    pub request_key: String,
    pub class_id: String,
    pub requested_operations: OperationBounds,
    #[serde(default)]
    pub requested_duration_seconds: Option<i64>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalInput {
    pub proposal_key: String,
    pub instance: String,
    pub operation: String,
    pub parameters: BTreeMap<String, Value>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PublicGrantReceipt {
    pub request_id: String,
    pub class_id: String,
    #[serde(flatten)]
    pub state: PublicGrantState,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PublicGrantState {
    Pending,
    Declined,
    Granted {
        grant_id: String,
        instance: String,
        allowed_operations: OperationBounds,
        expires_at: i64,
    },
    Revoked {
        grant_id: String,
    },
    Expired {
        grant_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PublicProposalReceipt {
    pub proposal_id: String,
    pub state: &'static str,
    pub class_id: String,
    pub operation: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub code: ReasonCode,
    pub message: &'static str,
}

impl Refusal {
    fn new(code: ReasonCode, message: &'static str) -> Self {
        Self { code, message }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for Refusal {}

#[derive(Debug, Clone)]
struct RequestView {
    context: PartnerContext,
    class_id: String,
    requested_operations: OperationBounds,
    requested_duration_seconds: Option<i64>,
}

#[derive(Debug, Clone)]
struct GrantView {
    context: PartnerContext,
    request_id: String,
    grant_id: String,
    class_id: String,
    surface: String,
    allowed_operations: OperationBounds,
    epoch_nonce: String,
    expires_at: i64,
    terminal: Option<GrantTerminal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GrantTerminal {
    Revoked,
    Expired,
}

pub fn request_grant(
    dir: &Path,
    context: &PartnerContext,
    input: GrantRequestInput,
    now: i64,
) -> Result<PublicGrantReceipt, Refusal> {
    if partner::registered_by(dir, &context.principal).is_none() {
        let refusal = Refusal::new(
            ReasonCode::WrongPrincipal,
            "this principal has no human-bound rung-3 registration",
        );
        audit_refusal(
            dir,
            context,
            PartnerOperation::GrantRequest,
            input.request_key.as_str(),
            refusal.code,
            now,
            None,
        );
        return Err(refusal);
    }
    if let Err(refusal) = validate_request(dir, &input) {
        audit_refusal(
            dir,
            context,
            PartnerOperation::GrantRequest,
            input.request_key.as_str(),
            refusal.code,
            now,
            None,
        );
        return Err(refusal);
    }
    let payload_hash = partner_act::payload_hash(&input).map_err(internal)?;
    if let Some((previous, previous_hash)) = partner_act::idempotent_original(
        dir,
        &context.principal,
        PartnerOperation::GrantRequest,
        &input.request_key,
    )
    .map_err(internal)?
    {
        if previous_hash == payload_hash {
            let request_id = match previous.body {
                PartnerActBody::GrantRequested { request_id, .. } => request_id,
                _ => {
                    return Err(internal(io::Error::other(
                        "idempotency row was not a request",
                    )))
                }
            };
            return receipt_for_request(dir, &request_id, now);
        }
        audit_refusal(
            dir,
            context,
            PartnerOperation::GrantRequest,
            &input.request_key,
            ReasonCode::IdempotencyConflict,
            now,
            None,
        );
        return Err(Refusal::new(
            ReasonCode::IdempotencyConflict,
            "that request_key was already used with a different request",
        ));
    }
    if open_request_count(dir, &context.principal)? >= MAX_OPEN_REQUESTS {
        let refusal = Refusal::new(ReasonCode::RequestLimit, "too many open grant requests");
        audit_refusal(
            dir,
            context,
            PartnerOperation::GrantRequest,
            &input.request_key,
            refusal.code,
            now,
            None,
        );
        return Err(refusal);
    }

    let request_id = partner::random_id("grant-request").map_err(internal)?;
    let requested_json = serde_json::to_value(&input.requested_operations).map_err(internal)?;
    let event = PartnerAct::partner(
        context,
        now,
        PartnerOperation::GrantRequest,
        PartnerOutcome::Proposed,
        ReasonCode::Accepted,
        request_id.clone(),
        PartnerActBody::GrantRequested {
            request_id: request_id.clone(),
            request_key: input.request_key.clone(),
            class_id: input.class_id.clone(),
            requested_operations: requested_json,
            requested_duration_seconds: input.requested_duration_seconds,
            reason: input.reason.clone(),
        },
    )
    .map_err(internal)?;
    let conflict = PartnerAct::partner(
        context,
        now,
        PartnerOperation::GrantRequest,
        PartnerOutcome::Refused,
        ReasonCode::IdempotencyConflict,
        request_id,
        PartnerActBody::Refusal {
            idempotency_key: Some(input.request_key.clone()),
            subject_ref: None,
        },
    )
    .map_err(internal)?;
    match partner_act::append_idempotent(dir, &event, &input.request_key, &payload_hash, &conflict)
        .map_err(internal)?
    {
        IdempotentAppend::Inserted(inserted) | IdempotentAppend::Replay(inserted) => {
            let request_id = match inserted.body {
                PartnerActBody::GrantRequested { request_id, .. } => request_id,
                _ => {
                    return Err(internal(io::Error::other(
                        "idempotency row was not a request",
                    )))
                }
            };
            receipt_for_request(dir, &request_id, now)
        }
        IdempotentAppend::Conflict(_) => Err(Refusal::new(
            ReasonCode::IdempotencyConflict,
            "that request_key was already used with a different request",
        )),
    }
}

/// Named-human transition. It is intentionally not exposed as an MCP tool; the signed private
/// console inbox calls this typed primitive after deriving the human from the verified device.
pub fn grant_request(
    dir: &Path,
    actor: &HumanDecisionContext,
    request_id: &str,
    surface: &str,
    allowed_operations: OperationBounds,
    expires_at: i64,
    now: i64,
) -> Result<PublicGrantReceipt, Refusal> {
    let events = partner_act::load(dir).map_err(internal)?;
    let request = request_view(&events, request_id)?.ok_or_else(|| {
        Refusal::new(
            ReasonCode::GrantMissing,
            "that grant request does not exist",
        )
    })?;
    require_addressee(dir, &request.context.principal, actor)?;
    if request_terminal(&events, request_id) {
        return Err(Refusal::new(
            ReasonCode::TransitionConflict,
            "that grant request already has a terminal decision",
        ));
    }
    let boundary = familiar_kernel::boundary::load(dir).map_err(internal)?;
    if !boundary.allow_agent {
        return Err(Refusal::new(
            ReasonCode::BoundaryClosed,
            "allow_agent is closed; a global ceiling cannot be inferred from this request",
        ));
    }
    let duration = expires_at.saturating_sub(now);
    if !(MIN_GRANT_SECONDS..=MAX_GRANT_SECONDS).contains(&duration)
        || request
            .requested_duration_seconds
            .is_some_and(|asked| duration > asked)
    {
        return Err(Refusal::new(
            ReasonCode::BoundsInvalid,
            "the grant duration is outside the request or hard ceiling",
        ));
    }
    if !bounds_narrow(&request.requested_operations, &allowed_operations) {
        return Err(Refusal::new(
            ReasonCode::BoundsInvalid,
            "the grant operations must be a nonempty narrowing of the request",
        ));
    }
    if !surface_matches(dir, surface, &request.class_id) {
        return Err(Refusal::new(
            ReasonCode::SurfaceMismatch,
            "the selected private surface does not implement that class",
        ));
    }

    let grant_id = partner::random_id("grant").map_err(internal)?;
    let epoch_nonce = partner::hex(&partner::random_bytes::<32>().map_err(internal)?);
    let event = PartnerAct::partner(
        &request.context,
        now,
        PartnerOperation::GrantDecision,
        PartnerOutcome::Completed,
        ReasonCode::HumanGranted,
        request_id.to_string(),
        PartnerActBody::GrantGranted {
            request_id: request_id.to_string(),
            grant_id,
            surface: surface.to_string(),
            allowed_operations: serde_json::to_value(allowed_operations).map_err(internal)?,
            epoch_nonce,
            granted_by: actor.human().to_string(),
            granted_at: now,
            expires_at,
        },
    )
    .map_err(internal)?
    .by_human(actor.human());
    match partner_act::append_transition(dir, &event, &format!("grant-decision:{request_id}"))
        .map_err(internal)?
    {
        TransitionAppend::Inserted(_) => receipt_for_request(dir, request_id, now),
        TransitionAppend::Existing(_) => Err(Refusal::new(
            ReasonCode::TransitionConflict,
            "that grant request was decided concurrently",
        )),
    }
}

pub fn decline_request(
    dir: &Path,
    actor: &HumanDecisionContext,
    request_id: &str,
    now: i64,
) -> Result<PublicGrantReceipt, Refusal> {
    let events = partner_act::load(dir).map_err(internal)?;
    let request = request_view(&events, request_id)?.ok_or_else(|| {
        Refusal::new(
            ReasonCode::GrantMissing,
            "that grant request does not exist",
        )
    })?;
    require_addressee(dir, &request.context.principal, actor)?;
    let event = PartnerAct::partner(
        &request.context,
        now,
        PartnerOperation::GrantDecision,
        PartnerOutcome::Refused,
        ReasonCode::HumanDeclined,
        request_id.to_string(),
        PartnerActBody::GrantDeclined {
            request_id: request_id.to_string(),
            declined_by: actor.human().to_string(),
        },
    )
    .map_err(internal)?
    .by_human(actor.human());
    match partner_act::append_transition(dir, &event, &format!("grant-decision:{request_id}"))
        .map_err(internal)?
    {
        TransitionAppend::Inserted(_) => receipt_for_request(dir, request_id, now),
        TransitionAppend::Existing(_) => Err(Refusal::new(
            ReasonCode::TransitionConflict,
            "that grant request already has a terminal decision",
        )),
    }
}

pub fn revoke_grant(
    dir: &Path,
    actor: &HumanDecisionContext,
    grant_id: &str,
    now: i64,
) -> Result<(), Refusal> {
    let events = partner_act::load(dir).map_err(internal)?;
    let grant = grant_views(&events)?
        .into_iter()
        .find(|g| g.grant_id == grant_id)
        .ok_or_else(|| Refusal::new(ReasonCode::GrantMissing, "that grant does not exist"))?;
    require_addressee(dir, &grant.context.principal, actor)?;
    if grant.terminal.is_some() || now >= grant.expires_at {
        return Err(Refusal::new(
            ReasonCode::GrantInactive,
            "that grant is already inactive",
        ));
    }
    let event = PartnerAct::partner(
        &grant.context,
        now,
        PartnerOperation::GrantRevocation,
        PartnerOutcome::Completed,
        ReasonCode::HumanRevoked,
        grant_id.to_string(),
        PartnerActBody::GrantRevoked {
            grant_id: grant_id.to_string(),
            surface: grant.surface,
            revoked_by: actor.human().to_string(),
        },
    )
    .map_err(internal)?
    .by_human(actor.human());
    match partner_act::append_transition(dir, &event, &format!("grant-terminal:{grant_id}"))
        .map_err(internal)?
    {
        TransitionAppend::Inserted(_) => Ok(()),
        TransitionAppend::Existing(_) => Err(Refusal::new(
            ReasonCode::TransitionConflict,
            "that grant was made inactive concurrently",
        )),
    }
}

pub fn propose(
    dir: &Path,
    context: &PartnerContext,
    input: ProposalInput,
    now: i64,
) -> Result<PublicProposalReceipt, Refusal> {
    if let Err(refusal) = validate_proposal_shape(&input) {
        audit_proposal_refusal(dir, context, &input, refusal.code, now);
        return Err(refusal);
    }
    if partner::registered_by(dir, &context.principal).is_none() {
        let refusal = Refusal::new(
            ReasonCode::WrongPrincipal,
            "this principal has no human-bound rung-3 registration",
        );
        audit_proposal_refusal(dir, context, &input, refusal.code, now);
        return Err(refusal);
    }
    let boundary = familiar_kernel::boundary::load(dir).map_err(internal)?;
    if !boundary.allow_agent {
        let refusal = Refusal::new(
            ReasonCode::BoundaryClosed,
            "allow_agent is closed; no new partner proposal may enter",
        );
        audit_proposal_refusal(dir, context, &input, refusal.code, now);
        return Err(refusal);
    }
    let payload_hash = partner_act::payload_hash(&input).map_err(internal)?;
    if let Some((previous, previous_hash)) = partner_act::idempotent_original(
        dir,
        &context.principal,
        PartnerOperation::Proposal,
        &input.proposal_key,
    )
    .map_err(internal)?
    {
        if previous_hash == payload_hash {
            return proposal_receipt(previous);
        }
        audit_proposal_refusal(dir, context, &input, ReasonCode::IdempotencyConflict, now);
        return Err(Refusal::new(
            ReasonCode::IdempotencyConflict,
            "that proposal_key was already used with a different proposal",
        ));
    }
    if open_proposal_count(dir, &context.principal)? >= MAX_OPEN_PROPOSALS {
        let refusal = Refusal::new(ReasonCode::ProposalLimit, "too many open proposals");
        audit_proposal_refusal(dir, context, &input, refusal.code, now);
        return Err(refusal);
    }
    let events = partner_act::load(dir).map_err(internal)?;
    let Some(mut grant) = resolve_handle(dir, &events, context, &input.instance)? else {
        let refusal = Refusal::new(
            ReasonCode::GrantMissing,
            "that instance handle is not active for this principal",
        );
        audit_proposal_refusal(dir, context, &input, refusal.code, now);
        return Err(refusal);
    };
    if grant.terminal.is_some() {
        let refusal = Refusal::new(ReasonCode::GrantInactive, "that grant is inactive");
        audit_proposal_refusal(dir, context, &input, refusal.code, now);
        return Err(refusal);
    }
    if now >= grant.expires_at {
        expire_grant(dir, &grant, now)?;
        grant.terminal = Some(GrantTerminal::Expired);
        let refusal = Refusal::new(ReasonCode::GrantInactive, "that grant has expired");
        audit_proposal_refusal(dir, context, &input, refusal.code, now);
        return Err(refusal);
    }
    let Some(bounds) = grant.allowed_operations.get(&input.operation) else {
        let refusal = Refusal::new(
            ReasonCode::UnknownOperation,
            "that operation is not in this grant",
        );
        audit_proposal_refusal(dir, context, &input, refusal.code, now);
        return Err(refusal);
    };
    if !parameters_fit(bounds, &input.parameters) {
        let refusal = Refusal::new(
            ReasonCode::BoundsInvalid,
            "proposal parameters are outside the grant",
        );
        audit_proposal_refusal(dir, context, &input, refusal.code, now);
        return Err(refusal);
    }

    let proposal_id = partner::random_id("proposal").map_err(internal)?;
    let event = PartnerAct::partner(
        context,
        now,
        PartnerOperation::Proposal,
        PartnerOutcome::Proposed,
        ReasonCode::ProposalStored,
        proposal_id.clone(),
        PartnerActBody::ProposalSubmitted {
            proposal_id: proposal_id.clone(),
            proposal_key: input.proposal_key.clone(),
            grant_id: grant.grant_id,
            class_id: grant.class_id.clone(),
            operation: input.operation.clone(),
            parameters: serde_json::to_value(&input.parameters).map_err(internal)?,
            reason: input.reason.clone(),
        },
    )
    .map_err(internal)?;
    let conflict = PartnerAct::partner(
        context,
        now,
        PartnerOperation::Proposal,
        PartnerOutcome::Refused,
        ReasonCode::IdempotencyConflict,
        proposal_id,
        PartnerActBody::ProposalRefused {
            proposal_id: None,
            proposal_key: Some(input.proposal_key.clone()),
            handle_fingerprint: Some(partner_act::opaque_fingerprint(&input.instance)),
        },
    )
    .map_err(internal)?;
    match partner_act::append_idempotent(dir, &event, &input.proposal_key, &payload_hash, &conflict)
        .map_err(internal)?
    {
        IdempotentAppend::Inserted(inserted) | IdempotentAppend::Replay(inserted) => {
            proposal_receipt(inserted)
        }
        IdempotentAppend::Conflict(_) => Err(Refusal::new(
            ReasonCode::IdempotencyConflict,
            "that proposal_key was already used with a different proposal",
        )),
    }
}

/// Named-human terminal transition for a proposal. The private inbox exposes refusal only, so the
/// open-proposal ceiling cannot become a permanent self-denial after sixty-four otherwise-valid
/// proposals and no acceptance or execution edge is introduced.
pub fn refuse_proposal(
    dir: &Path,
    actor: &HumanDecisionContext,
    proposal_id: &str,
    now: i64,
) -> Result<(), Refusal> {
    let events = partner_act::load(dir).map_err(internal)?;
    let submitted = events
        .iter()
        .find(|event| {
            matches!(
                &event.body,
                PartnerActBody::ProposalSubmitted {
                    proposal_id: id,
                    ..
                } if id == proposal_id
            )
        })
        .ok_or_else(|| Refusal::new(ReasonCode::GrantMissing, "that proposal does not exist"))?;
    require_addressee(dir, &submitted.principal, actor)?;
    let event = PartnerAct::partner(
        &context_of(submitted),
        now,
        PartnerOperation::ProposalDecision,
        PartnerOutcome::Refused,
        ReasonCode::HumanRefusedProposal,
        proposal_id.to_string(),
        PartnerActBody::ProposalRefused {
            proposal_id: Some(proposal_id.to_string()),
            proposal_key: None,
            handle_fingerprint: None,
        },
    )
    .map_err(internal)?
    .by_human(actor.human());
    match partner_act::append_transition(dir, &event, &format!("proposal-decision:{proposal_id}"))
        .map_err(internal)?
    {
        TransitionAppend::Inserted(_) => Ok(()),
        TransitionAppend::Existing(_) => Err(Refusal::new(
            ReasonCode::TransitionConflict,
            "that proposal already has a terminal decision",
        )),
    }
}

// ---------------------------------------------------------------------------------------
// Rungs 4 (observe) and 5 (invoke) — the execution edge (ADR-0044).
//
// These are the first tools that reach a real surface on a partner's call. The whole safety
// posture is: THREE independent human acts must already hold before anything happens — a
// human opened `allow_actuate`, a human granted this exact operation on this surface within
// bounds and an expiry, and a human declared the surface at all. This code adds the authority
// checks (active grant, operation in grant, bounds, boundary) and then delegates the raw
// surface I/O to the injected executor, which re-checks `allow_actuate` as the final floor.
// A partner-facing receipt never carries the private surface name or the concrete act label.
// ---------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObserveInput {
    /// The opaque instance handle a grant receipt gave the partner.
    pub instance: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvokeInput {
    pub instance: String,
    /// The abstract class operation to run (e.g. "set_state"). Never a local act label.
    pub operation: String,
    pub parameters: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ObserveReceipt {
    pub instance: String,
    /// The generic observable reading. Carries only what the class declares as observable.
    pub reading: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InvokeReceipt {
    pub instance: String,
    /// The ABSTRACT operation the partner named — never the private local act label.
    pub operation: String,
    /// A short generic effect line. Never the surface name.
    pub effect: String,
}

/// Shared front half of both rungs: the principal must be human-bound and the handle must
/// resolve to an active, unexpired grant that carries `operation`. Returns the live grant.
fn authorized_grant(
    dir: &Path,
    context: &PartnerContext,
    instance: &str,
    operation: &str,
    audit_as: PartnerOperation,
    now: i64,
) -> Result<GrantView, Refusal> {
    if partner::registered_by(dir, &context.principal).is_none() {
        let refusal = Refusal::new(
            ReasonCode::WrongPrincipal,
            "this principal has no human-bound rung-3 registration",
        );
        audit_access_refusal(dir, context, audit_as, refusal.code, now);
        return Err(refusal);
    }
    let events = partner_act::load(dir).map_err(internal)?;
    let Some(mut grant) = resolve_handle(dir, &events, context, instance)? else {
        let refusal = Refusal::new(
            ReasonCode::GrantMissing,
            "that instance handle is not active for this principal",
        );
        audit_access_refusal(dir, context, audit_as, refusal.code, now);
        return Err(refusal);
    };
    if grant.terminal.is_some() {
        let refusal = Refusal::new(ReasonCode::GrantInactive, "that grant is inactive");
        audit_access_refusal(dir, context, audit_as, refusal.code, now);
        return Err(refusal);
    }
    if now >= grant.expires_at {
        expire_grant(dir, &grant, now)?;
        grant.terminal = Some(GrantTerminal::Expired);
        let refusal = Refusal::new(ReasonCode::GrantInactive, "that grant has expired");
        audit_access_refusal(dir, context, audit_as, refusal.code, now);
        return Err(refusal);
    }
    if !grant.allowed_operations.contains_key(operation) {
        let refusal = Refusal::new(
            ReasonCode::UnknownOperation,
            "that operation is not in this grant",
        );
        audit_access_refusal(dir, context, audit_as, refusal.code, now);
        return Err(refusal);
    }
    Ok(grant)
}

/// Rung 4: a granted partner reads a bound surface's declared observable. Read-only, but the
/// read still runs the surface's declared command, so the executor re-checks the boundary.
pub fn observe(
    dir: &Path,
    context: &PartnerContext,
    input: ObserveInput,
    now: i64,
    executor: &dyn crate::executor::SurfaceExecutor,
) -> Result<ObserveReceipt, Refusal> {
    let grant = authorized_grant(
        dir,
        context,
        &input.instance,
        crate::offering::OBSERVE_OP,
        PartnerOperation::Observe,
        now,
    )?;
    match executor.observe(dir, &grant.surface) {
        Ok(concrete_bucket) => {
            // Map the concrete current bucket to the class's ABSTRACT state before it reaches
            // the partner — the receipt carries "primary"/"reverted", never a local label.
            let reading = abstract_state_of(dir, &grant.surface, &concrete_bucket)
                .unwrap_or_else(|| "unknown".to_string());
            record_effect(
                dir,
                context,
                now,
                PartnerOperation::Observe,
                ReasonCode::Observed,
                PartnerActBody::Observed {
                    grant_id: grant.grant_id.clone(),
                    surface: grant.surface.clone(),
                },
            );
            Ok(ObserveReceipt {
                instance: input.instance,
                reading,
            })
        }
        Err(_) => {
            audit_access_refusal(
                dir,
                context,
                PartnerOperation::Observe,
                ReasonCode::ExecutionRefused,
                now,
            );
            Err(Refusal::new(
                ReasonCode::ExecutionRefused,
                "the surface could not be read",
            ))
        }
    }
}

/// Rung 5: a granted partner runs a bound act — the execution edge. Checked twice for two
/// different truths: the grant authorizes this operation within bounds (here), and
/// `allow_actuate` permits the effect (here as an honest early gate, and again in the executor
/// as the final floor). The abstract operation is mapped to a local act only now, under the
/// live grant, and the mapping never leaves this crate.
pub fn invoke(
    dir: &Path,
    context: &PartnerContext,
    input: InvokeInput,
    now: i64,
    executor: &dyn crate::executor::SurfaceExecutor,
) -> Result<InvokeReceipt, Refusal> {
    let grant = authorized_grant(
        dir,
        context,
        &input.instance,
        &input.operation,
        PartnerOperation::Invoke,
        now,
    )?;
    // Bounds: the parameters must fit the grant's narrowing of this operation.
    let bounds = grant
        .allowed_operations
        .get(&input.operation)
        .expect("authorized_grant proved the operation is present");
    if !parameters_fit(bounds, &input.parameters) {
        let refusal = Refusal::new(
            ReasonCode::BoundsInvalid,
            "invoke parameters are outside the grant",
        );
        audit_access_refusal(dir, context, PartnerOperation::Invoke, refusal.code, now);
        return Err(refusal);
    }
    // The boundary, as an honest early gate (the executor re-checks it as the final floor).
    let boundary = familiar_kernel::boundary::load(dir).map_err(internal)?;
    if !boundary.allow_actuate {
        let refusal = Refusal::new(
            ReasonCode::BoundaryClosed,
            "allow_actuate is closed; the effect channel is not open",
        );
        audit_access_refusal(dir, context, PartnerOperation::Invoke, refusal.code, now);
        return Err(refusal);
    }
    // Resolve the abstract operation to a concrete local act, under the grant, now.
    let label = resolve_local_act(dir, &grant.surface, &input.operation, &input.parameters)?;
    match executor.invoke(dir, &grant.surface, &label) {
        Ok(()) => {
            record_effect(
                dir,
                context,
                now,
                PartnerOperation::Invoke,
                ReasonCode::Invoked,
                PartnerActBody::Invoked {
                    grant_id: grant.grant_id.clone(),
                    surface: grant.surface.clone(),
                    operation: input.operation.clone(),
                    parameters: serde_json::to_value(&input.parameters).map_err(internal)?,
                    label,
                },
            );
            // The partner-facing effect is generic — it echoes only the ABSTRACT operation it
            // asked for, never the surface, the local act, or device output.
            Ok(InvokeReceipt {
                effect: format!("{} applied within grant", input.operation),
                instance: input.instance,
                operation: input.operation,
            })
        }
        Err(_) => {
            audit_access_refusal(
                dir,
                context,
                PartnerOperation::Invoke,
                ReasonCode::ExecutionRefused,
                now,
            );
            Err(Refusal::new(
                ReasonCode::ExecutionRefused,
                "the act did not run",
            ))
        }
    }
}

/// Map an abstract class operation + parameters to a concrete local act label, under a live
/// grant on `surface`. This is ADR-0044's "private surface resolver," and it deliberately
/// lives here, never crossing to a partner.
///
/// RESOLVER SEMANTICS (flagged for review — the one genuinely underspecified choice in this
/// feature): for `switchable.reversible/v1`, the abstract `state` maps to the actuator's
/// human-authored bucket order — `primary` → the first bucket's act, `reverted` → the second.
/// Bucket order is written by the human in `actuators.json`, so a human controls the mapping;
/// but which physical state is "primary" is not otherwise labelled, so a human granting
/// `set_state:primary` is trusting this ordering. A future revision may want an explicit
/// primary/reverted label on the surface declaration or captured into the grant at decision time.
fn resolve_local_act(
    dir: &Path,
    surface: &str,
    operation: &str,
    parameters: &BTreeMap<String, Value>,
) -> Result<String, Refusal> {
    if operation != "set_state" {
        return Err(Refusal::new(
            ReasonCode::UnknownOperation,
            "no local act resolves this operation",
        ));
    }
    let state = parameters
        .get("state")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Refusal::new(ReasonCode::BoundsInvalid, "set_state needs a state"))?;
    let (acts, _skipped) = familiar_kernel::actuator::load(dir).map_err(internal)?;
    let actuator = acts
        .iter()
        .find(|a| a.surface == surface)
        .ok_or_else(|| Refusal::new(ReasonCode::SurfaceMismatch, "the bound surface is gone"))?;
    if !actuator_matches_class(actuator, "switchable.reversible/v1") {
        return Err(Refusal::new(
            ReasonCode::SurfaceMismatch,
            "the bound surface no longer implements its class",
        ));
    }
    let index = match state {
        "primary" => 0,
        "reverted" => 1,
        _ => {
            return Err(Refusal::new(
                ReasonCode::BoundsInvalid,
                "state is neither primary nor reverted",
            ))
        }
    };
    actuator
        .buckets
        .get(index)
        .map(|bucket| bucket.name.clone())
        .ok_or_else(|| Refusal::new(ReasonCode::SurfaceMismatch, "the revert pair is incomplete"))
}

/// The reverse of [`resolve_local_act`]: map a concrete current bucket back to the class's
/// abstract state, so an observe reading reaches a partner as "primary"/"reverted" and never a
/// local label. `None` when the surface is gone or no longer matches its class.
fn abstract_state_of(dir: &Path, surface: &str, concrete_bucket: &str) -> Option<String> {
    let (acts, _skipped) = familiar_kernel::actuator::load(dir).ok()?;
    let actuator = acts.iter().find(|a| a.surface == surface)?;
    if !actuator_matches_class(actuator, "switchable.reversible/v1") {
        return None;
    }
    match actuator
        .buckets
        .iter()
        .position(|b| b.name == concrete_bucket)
    {
        Some(0) => Some("primary".to_string()),
        Some(1) => Some("reverted".to_string()),
        _ => None,
    }
}

/// Append a completed rung-4/5 effect to the partner-act ledger. Household-internal truth;
/// best-effort like the audit trail — a ledger write must not undo an effect that happened.
fn record_effect(
    dir: &Path,
    context: &PartnerContext,
    now: i64,
    operation: PartnerOperation,
    code: ReasonCode,
    body: PartnerActBody,
) {
    if let Ok(correlation) = partner::random_id("effect") {
        if let Ok(event) = PartnerAct::partner(
            context,
            now,
            operation,
            PartnerOutcome::Completed,
            code,
            correlation,
            body,
        ) {
            let _ = partner_act::append(dir, &event);
        }
    }
}

/// Audit a strict-schema refusal discovered by the MCP serializer before it can construct an
/// input type. Unauthenticated and pre-rate-admission bytes never call this function.
pub fn audit_schema_refusal(
    dir: &Path,
    context: &PartnerContext,
    operation: PartnerOperation,
    now: i64,
) {
    audit_refusal(
        dir,
        context,
        operation,
        "",
        ReasonCode::SchemaInvalid,
        now,
        None,
    );
}

pub fn audit_access_refusal(
    dir: &Path,
    context: &PartnerContext,
    operation: PartnerOperation,
    code: ReasonCode,
    now: i64,
) {
    audit_refusal(dir, context, operation, "", code, now, None);
}

fn receipt_for_request(
    dir: &Path,
    request_id: &str,
    now: i64,
) -> Result<PublicGrantReceipt, Refusal> {
    let events = partner_act::load(dir).map_err(internal)?;
    let request = request_view(&events, request_id)?.ok_or_else(|| {
        Refusal::new(
            ReasonCode::GrantMissing,
            "that grant request does not exist",
        )
    })?;
    if events.iter().any(|event| {
        matches!(&event.body, PartnerActBody::GrantDeclined { request_id: id, .. } if id == request_id)
    }) {
        return Ok(PublicGrantReceipt {
            request_id: request_id.to_string(),
            class_id: request.class_id,
            state: PublicGrantState::Declined,
        });
    }
    if let Some(grant) = grant_views(&events)?
        .into_iter()
        .find(|grant| grant.request_id == request_id)
    {
        let state = match grant.terminal {
            Some(GrantTerminal::Revoked) => PublicGrantState::Revoked {
                grant_id: grant.grant_id,
            },
            Some(GrantTerminal::Expired) => PublicGrantState::Expired {
                grant_id: grant.grant_id,
            },
            None if now >= grant.expires_at => PublicGrantState::Expired {
                grant_id: grant.grant_id,
            },
            None => PublicGrantState::Granted {
                grant_id: grant.grant_id.clone(),
                instance: instance_handle(dir, &grant).map_err(internal)?,
                allowed_operations: grant.allowed_operations,
                expires_at: grant.expires_at,
            },
        };
        return Ok(PublicGrantReceipt {
            request_id: request_id.to_string(),
            class_id: request.class_id,
            state,
        });
    }
    Ok(PublicGrantReceipt {
        request_id: request_id.to_string(),
        class_id: request.class_id,
        state: PublicGrantState::Pending,
    })
}

fn proposal_receipt(event: PartnerAct) -> Result<PublicProposalReceipt, Refusal> {
    match event.body {
        PartnerActBody::ProposalSubmitted {
            proposal_id,
            class_id,
            operation,
            ..
        } => Ok(PublicProposalReceipt {
            proposal_id,
            state: "proposed",
            class_id,
            operation,
        }),
        _ => Err(internal(io::Error::other(
            "idempotency row was not a proposal",
        ))),
    }
}

fn validate_request(dir: &Path, input: &GrantRequestInput) -> Result<(), Refusal> {
    validate_key(&input.request_key)?;
    validate_reason(input.reason.as_deref())?;
    if input
        .requested_duration_seconds
        .is_some_and(|s| !(MIN_GRANT_SECONDS..=MAX_GRANT_SECONDS).contains(&s))
    {
        return Err(Refusal::new(
            ReasonCode::BoundsInvalid,
            "requested duration is outside the hard ceiling",
        ));
    }
    if input.requested_operations.is_empty() || input.requested_operations.len() > 8 {
        return Err(Refusal::new(
            ReasonCode::BoundsInvalid,
            "requested_operations must contain one to eight operations",
        ));
    }
    let class = crate::offering::CLASS_DEFS
        .iter()
        .find(|class| class.id == input.class_id)
        .ok_or_else(|| Refusal::new(ReasonCode::UnknownClass, "unknown capability class"))?;
    if !crate::offering::available(dir)
        .iter()
        .any(|available| available.def.id == class.id)
    {
        return Err(Refusal::new(
            ReasonCode::UnknownClass,
            "that class is not currently offered",
        ));
    }
    for (operation_id, bounds) in &input.requested_operations {
        let operation = class
            .operations
            .iter()
            .find(|operation| operation.id == operation_id)
            .ok_or_else(|| Refusal::new(ReasonCode::UnknownOperation, "unknown class operation"))?;
        validate_schema_bounds(operation.input_schema, bounds)?;
    }
    Ok(())
}

fn validate_schema_bounds(schema: &str, bounds: &ParameterBounds) -> Result<(), Refusal> {
    if bounds.len() > 16 {
        return Err(Refusal::new(
            ReasonCode::BoundsInvalid,
            "an operation takes at most sixteen parameter bounds",
        ));
    }
    let schema: Value = serde_json::from_str(schema).map_err(|_| {
        Refusal::new(
            ReasonCode::Internal,
            "the repo-authored operation schema is invalid",
        )
    })?;
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| Refusal::new(ReasonCode::Internal, "operation schema has no properties"))?;
    // A parameterless operation (rung-4 `observe`) takes no bounds; one with parameters takes
    // at least one. This is what lets observe be requested and granted like any other leg.
    if properties.is_empty() {
        return if bounds.is_empty() {
            Ok(())
        } else {
            Err(Refusal::new(
                ReasonCode::BoundsInvalid,
                "this operation takes no parameter bounds",
            ))
        };
    }
    if bounds.is_empty() {
        return Err(Refusal::new(
            ReasonCode::BoundsInvalid,
            "an operation with parameters needs one to sixteen bounds",
        ));
    }
    let required: BTreeSet<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if !required.iter().all(|key| bounds.contains_key(*key)) {
        return Err(Refusal::new(
            ReasonCode::BoundsInvalid,
            "bounds must name every required operation parameter",
        ));
    }
    for (name, bound) in bounds {
        let property = properties.get(name).ok_or_else(|| {
            Refusal::new(
                ReasonCode::BoundsInvalid,
                "bound names an unknown parameter",
            )
        })?;
        match bound {
            ParameterBound::Enum { values } => {
                let allowed: BTreeSet<&str> = property
                    .get("enum")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                let unique: BTreeSet<&str> = values.iter().map(String::as_str).collect();
                if values.is_empty()
                    || values.len() != unique.len()
                    || !unique.iter().all(|value| allowed.contains(value))
                {
                    return Err(Refusal::new(
                        ReasonCode::BoundsInvalid,
                        "enum bound is empty, duplicated, or outside the class schema",
                    ));
                }
            }
            ParameterBound::Number { min, max } => {
                if property.get("type").and_then(Value::as_str) != Some("number")
                    || !min.is_finite()
                    || !max.is_finite()
                    || min > max
                    || property
                        .get("minimum")
                        .and_then(Value::as_f64)
                        .is_some_and(|floor| *min < floor)
                    || property
                        .get("maximum")
                        .and_then(Value::as_f64)
                        .is_some_and(|ceiling| *max > ceiling)
                {
                    return Err(Refusal::new(
                        ReasonCode::BoundsInvalid,
                        "numeric bound is invalid or outside the class schema",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_proposal_shape(input: &ProposalInput) -> Result<(), Refusal> {
    validate_key(&input.proposal_key)?;
    validate_reason(input.reason.as_deref())?;
    if input.instance.len() > 128
        || input.operation.is_empty()
        || input.operation.len() > 80
        || input.parameters.is_empty()
        || input.parameters.len() > 16
    {
        return Err(Refusal::new(
            ReasonCode::SchemaInvalid,
            "proposal handle, operation, or parameters are malformed",
        ));
    }
    Ok(())
}

fn validate_key(key: &str) -> Result<(), Refusal> {
    if key.is_empty()
        || key.len() > MAX_IDEMPOTENCY_BYTES
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(Refusal::new(
            ReasonCode::SchemaInvalid,
            "idempotency keys use 1-64 ASCII letters, digits, dot, dash, or underscore",
        ));
    }
    Ok(())
}

fn validate_reason(reason: Option<&str>) -> Result<(), Refusal> {
    if reason.is_some_and(|reason| {
        reason.len() > MAX_REASON_BYTES || reason.chars().any(char::is_control)
    }) {
        return Err(Refusal::new(
            ReasonCode::SchemaInvalid,
            "reason is too long or contains control characters",
        ));
    }
    Ok(())
}

fn require_addressee(
    dir: &Path,
    principal: &str,
    actor: &HumanDecisionContext,
) -> Result<(), Refusal> {
    if !partner::is_registered_for(dir, principal, actor) {
        return Err(Refusal::new(
            ReasonCode::WrongPrincipal,
            "this human is not the registered addressee for that partner",
        ));
    }
    Ok(())
}

fn bounds_narrow(requested: &OperationBounds, granted: &OperationBounds) -> bool {
    !granted.is_empty()
        && granted.iter().all(|(operation, grant_bounds)| {
            requested
                .get(operation)
                .is_some_and(|request_bounds| parameter_bounds_narrow(request_bounds, grant_bounds))
        })
}

fn parameter_bounds_narrow(requested: &ParameterBounds, granted: &ParameterBounds) -> bool {
    // A parameterless operation (rung-4 `observe` takes no bounds) narrows only to itself:
    // empty grants empty, and nothing else. Every operation that DOES carry parameters still
    // requires a nonempty, same-arity narrowing below.
    if requested.is_empty() {
        return granted.is_empty();
    }
    !granted.is_empty()
        && granted.len() == requested.len()
        && granted.iter().all(|(name, grant)| {
            requested
                .get(name)
                .is_some_and(|request| match (request, grant) {
                    (
                        ParameterBound::Enum { values: requested },
                        ParameterBound::Enum { values: granted },
                    ) => {
                        !granted.is_empty() && granted.iter().all(|value| requested.contains(value))
                    }
                    (
                        ParameterBound::Number {
                            min: request_min,
                            max: request_max,
                        },
                        ParameterBound::Number {
                            min: grant_min,
                            max: grant_max,
                        },
                    ) => {
                        grant_min >= request_min
                            && grant_max <= request_max
                            && grant_min <= grant_max
                    }
                    _ => false,
                })
        })
}

fn parameters_fit(bounds: &ParameterBounds, parameters: &BTreeMap<String, Value>) -> bool {
    parameters.len() == bounds.len()
        && parameters.iter().all(|(name, value)| {
            bounds.get(name).is_some_and(|bound| match bound {
                ParameterBound::Enum { values } => value
                    .as_str()
                    .is_some_and(|value| values.iter().any(|allowed| allowed == value)),
                ParameterBound::Number { min, max } => value
                    .as_f64()
                    .is_some_and(|value| value.is_finite() && value >= *min && value <= *max),
            })
        })
}

fn request_view(events: &[PartnerAct], request_id: &str) -> Result<Option<RequestView>, Refusal> {
    for event in events {
        if let PartnerActBody::GrantRequested {
            request_id: id,
            class_id,
            requested_operations,
            requested_duration_seconds,
            ..
        } = &event.body
        {
            if id == request_id {
                return Ok(Some(RequestView {
                    context: context_of(event),
                    class_id: class_id.clone(),
                    requested_operations: serde_json::from_value(requested_operations.clone())
                        .map_err(internal)?,
                    requested_duration_seconds: *requested_duration_seconds,
                }));
            }
        }
    }
    Ok(None)
}

fn request_terminal(events: &[PartnerAct], request_id: &str) -> bool {
    events.iter().any(|event| match &event.body {
        PartnerActBody::GrantGranted { request_id: id, .. }
        | PartnerActBody::GrantDeclined { request_id: id, .. } => id == request_id,
        _ => false,
    })
}

fn grant_views(events: &[PartnerAct]) -> Result<Vec<GrantView>, Refusal> {
    let mut grants = Vec::new();
    for event in events {
        if let PartnerActBody::GrantGranted {
            request_id,
            grant_id,
            surface,
            allowed_operations,
            epoch_nonce,
            expires_at,
            ..
        } = &event.body
        {
            let request = request_view(events, request_id)?.ok_or_else(|| {
                internal(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "grant has no request",
                ))
            })?;
            let allowed_operations =
                serde_json::from_value(allowed_operations.clone()).map_err(internal)?;
            let terminal = events.iter().find_map(|later| match &later.body {
                PartnerActBody::GrantRevoked { grant_id: id, .. } if id == grant_id => {
                    Some(GrantTerminal::Revoked)
                }
                PartnerActBody::GrantExpired { grant_id: id, .. } if id == grant_id => {
                    Some(GrantTerminal::Expired)
                }
                _ => None,
            });
            grants.push(GrantView {
                context: request.context,
                request_id: request_id.clone(),
                grant_id: grant_id.clone(),
                class_id: request.class_id,
                surface: surface.clone(),
                allowed_operations,
                epoch_nonce: epoch_nonce.clone(),
                expires_at: *expires_at,
                terminal,
            });
        }
    }
    Ok(grants)
}

fn context_of(event: &PartnerAct) -> PartnerContext {
    PartnerContext {
        principal: event.principal.clone(),
        credential_fingerprint: event.credential.clone(),
        alias: event.alias_snapshot.clone(),
    }
}

fn open_request_count(dir: &Path, principal: &str) -> Result<usize, Refusal> {
    let events = partner_act::load(dir).map_err(internal)?;
    Ok(events
        .iter()
        .filter_map(|event| match &event.body {
            PartnerActBody::GrantRequested { request_id, .. } if event.principal == principal => {
                Some(request_id)
            }
            _ => None,
        })
        .filter(|request_id| !request_terminal(&events, request_id))
        .count())
}

fn open_proposal_count(dir: &Path, principal: &str) -> Result<usize, Refusal> {
    let events = partner_act::load(dir).map_err(internal)?;
    Ok(events
        .iter()
        .filter_map(|event| match &event.body {
            PartnerActBody::ProposalSubmitted { proposal_id, .. }
                if event.principal == principal =>
            {
                Some(proposal_id)
            }
            _ => None,
        })
        .filter(|proposal_id| {
            !events.iter().any(|event| match &event.body {
                PartnerActBody::ProposalRefused {
                    proposal_id: Some(id),
                    ..
                }
                | PartnerActBody::ProposalWithdrawn {
                    proposal_id: id, ..
                } => id == *proposal_id,
                _ => false,
            })
        })
        .count())
}

fn resolve_handle(
    dir: &Path,
    events: &[PartnerAct],
    context: &PartnerContext,
    presented: &str,
) -> Result<Option<GrantView>, Refusal> {
    let mut matched = None;
    for grant in grant_views(events)?
        .into_iter()
        .filter(|grant| grant.context.principal == context.principal)
    {
        let expected = instance_handle(dir, &grant).map_err(internal)?;
        if partner::same_secret(&expected, presented) {
            matched = Some(grant);
        }
    }
    Ok(matched)
}

fn instance_handle(dir: &Path, grant: &GrantView) -> io::Result<String> {
    let key = handle_key(dir)?;
    let mut mac = Hmac::<Sha256>::new_from_slice(&key).map_err(io::Error::other)?;
    mac.update(b"familiar-grant-handle-v1\0");
    for field in [
        grant.context.principal.as_str(),
        grant.grant_id.as_str(),
        grant.surface.as_str(),
        grant.epoch_nonce.as_str(),
    ] {
        mac.update(&(field.len() as u64).to_be_bytes());
        mac.update(field.as_bytes());
    }
    Ok(format!(
        "instance-{}",
        partner::hex(&mac.finalize().into_bytes())
    ))
}

fn handle_key(dir: &Path) -> io::Result<[u8; 32]> {
    let path = dir.join(HANDLE_KEY_FILE);
    if let Ok(bytes) = std::fs::read(&path) {
        return bytes
            .as_slice()
            .try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "grant handle key length"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("no handle-key parent"))?;
    std::fs::create_dir_all(parent)?;
    let key = partner::random_bytes::<32>()?;
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
            }
            file.write_all(&key)?;
            Ok(key)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let bytes = std::fs::read(path)?;
            bytes
                .as_slice()
                .try_into()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "grant handle key length"))
        }
        Err(error) => Err(error),
    }
}

fn expire_grant(dir: &Path, grant: &GrantView, now: i64) -> Result<(), Refusal> {
    let event = PartnerAct::clock(
        &grant.context,
        now,
        grant.grant_id.clone(),
        PartnerActBody::GrantExpired {
            grant_id: grant.grant_id.clone(),
            surface: grant.surface.clone(),
        },
    )
    .map_err(internal)?;
    let _ =
        partner_act::append_transition(dir, &event, &format!("grant-terminal:{}", grant.grant_id))
            .map_err(internal)?;
    Ok(())
}

fn surface_matches(dir: &Path, surface: &str, class_id: &str) -> bool {
    let Ok((surfaces, _)) = familiar_kernel::actuator::load(dir) else {
        return false;
    };
    let Some(surface) = surfaces
        .iter()
        .find(|candidate| candidate.surface == surface)
    else {
        return false;
    };
    // The offering seam remains chair-owned. Keep this exhaustive and pinned against every
    // ClassDef until that seam grows a private matcher token suitable for grant binding.
    actuator_matches_class(surface, class_id)
}

pub(crate) fn actuator_matches_class(
    surface: &familiar_kernel::actuator::Actuator,
    class_id: &str,
) -> bool {
    match class_id {
        "switchable.reversible/v1" => {
            surface.actions.len() == 2
                && surface.buckets.len() == 2
                && surface
                    .buckets
                    .iter()
                    .all(|bucket| surface.actions.contains_key(&bucket.name))
                && surface
                    .actions
                    .keys()
                    .all(|action| surface.buckets.iter().any(|bucket| &bucket.name == action))
        }
        _ => false,
    }
}

fn audit_refusal(
    dir: &Path,
    context: &PartnerContext,
    operation: PartnerOperation,
    key: &str,
    code: ReasonCode,
    now: i64,
    subject_ref: Option<String>,
) {
    if let Ok(correlation) = partner::random_id("refusal") {
        if let Ok(event) = PartnerAct::partner(
            context,
            now,
            operation,
            PartnerOutcome::Refused,
            code,
            correlation.clone(),
            PartnerActBody::Refusal {
                idempotency_key: (!key.is_empty()).then(|| key.to_string()),
                subject_ref,
            },
        ) {
            let _ = partner_act::append(dir, &event);
        }
    }
}

fn audit_proposal_refusal(
    dir: &Path,
    context: &PartnerContext,
    input: &ProposalInput,
    code: ReasonCode,
    now: i64,
) {
    if let Ok(correlation) = partner::random_id("proposal-refusal") {
        if let Ok(event) = PartnerAct::partner(
            context,
            now,
            PartnerOperation::Proposal,
            PartnerOutcome::Refused,
            code,
            correlation,
            PartnerActBody::ProposalRefused {
                proposal_id: None,
                proposal_key: (!input.proposal_key.is_empty()).then(|| input.proposal_key.clone()),
                handle_fingerprint: (!input.instance.is_empty())
                    .then(|| partner_act::opaque_fingerprint(&input.instance)),
            },
        ) {
            let _ = partner_act::append(dir, &event);
        }
    }
}

fn internal<E: std::fmt::Display>(error: E) -> Refusal {
    let _ = error;
    Refusal::new(
        ReasonCode::Internal,
        "the local partner ledger could not record this act",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 10_000;

    fn temp(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mcp_grant_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        declared_surface(&dir, "ians-secret-lamp");
        std::fs::create_dir_all(dir.join("mcp")).unwrap();
        let registry = partner::PrincipalRegistry {
            principals: ["principal-a", "principal-b"]
                .into_iter()
                .map(|id| partner::PrincipalRecord {
                    id: id.into(),
                    alias: "Workshop agent".into(),
                    credential_file: "mcp/test.env".into(),
                    credential_key: "TOKEN".into(),
                    credential_fingerprint: format!("{id}-fingerprint"),
                    registered_by: "ian".into(),
                    enabled: true,
                })
                .collect(),
        };
        std::fs::write(
            dir.join(partner::PRINCIPALS_FILE),
            serde_json::to_vec(&registry).unwrap(),
        )
        .unwrap();
        dir
    }

    fn declared_surface(dir: &Path, name: &str) {
        let file = serde_json::json!({ "actuators": [{
            "surface": name,
            "state_cmd": "private-state-command",
            "state": { "fields": { "power": { "kind": "enum",
                "values": ["on", "off"], "source": { "kind": "json", "key": "power" }
            } } },
            "actions": { "private-on": "secret command on", "private-off": "secret command off" },
            "buckets": [
                { "name": "private-on", "when": [{ "op": "eq", "field": "power", "value": "off" }] },
                { "name": "private-off", "when": [] }
            ]
        }] });
        std::fs::write(
            dir.join(familiar_kernel::actuator::ACTUATORS_FILE),
            serde_json::to_vec(&file).unwrap(),
        )
        .unwrap();
    }

    fn context(id: &str, key: &str) -> PartnerContext {
        PartnerContext {
            principal: id.into(),
            credential_fingerprint: key.into(),
            alias: "Workshop agent".into(),
        }
    }

    fn human(name: &str) -> HumanDecisionContext {
        HumanDecisionContext::from_verified_mesh(format!("{name}-device"), name.into()).unwrap()
    }

    fn operations(values: &[&str]) -> OperationBounds {
        BTreeMap::from([(
            "set_state".into(),
            BTreeMap::from([(
                "state".into(),
                ParameterBound::Enum {
                    values: values.iter().map(|value| value.to_string()).collect(),
                },
            )]),
        )])
    }

    fn request(key: &str) -> GrantRequestInput {
        GrantRequestInput {
            request_key: key.into(),
            class_id: "switchable.reversible/v1".into(),
            requested_operations: operations(&["primary", "reverted"]),
            requested_duration_seconds: Some(600),
            reason: Some("for the human to consider".into()),
        }
    }

    fn open_agent(dir: &Path) {
        let mut boundary = familiar_kernel::boundary::Boundary::closed();
        boundary.allow_agent = true;
        std::fs::write(
            dir.join(familiar_kernel::boundary::BOUNDARY_FILE),
            serde_json::to_vec(&boundary).unwrap(),
        )
        .unwrap();
    }

    fn granted(dir: &Path, context: &PartnerContext, key: &str) -> PublicGrantReceipt {
        let pending = request_grant(dir, context, request(key), NOW).unwrap();
        open_agent(dir);
        grant_request(
            dir,
            &human("ian"),
            &pending.request_id,
            "ians-secret-lamp",
            operations(&["primary"]),
            NOW + 300,
            NOW + 1,
        )
        .unwrap()
    }

    fn handle(receipt: &PublicGrantReceipt) -> String {
        match &receipt.state {
            PublicGrantState::Granted { instance, .. } => instance.clone(),
            other => panic!("expected grant, got {other:?}"),
        }
    }

    // ---- Rungs 4/5: the execution edge, hostile tests ----

    struct FakeExec {
        fail: bool,
        reading: String,
        observed: std::sync::Mutex<Vec<String>>,
        invoked: std::sync::Mutex<Vec<(String, String)>>,
    }
    impl FakeExec {
        fn ok(reading: &str) -> Self {
            Self {
                fail: false,
                reading: reading.into(),
                observed: std::sync::Mutex::new(Vec::new()),
                invoked: std::sync::Mutex::new(Vec::new()),
            }
        }
        fn failing() -> Self {
            Self {
                fail: true,
                ..Self::ok("")
            }
        }
    }
    impl crate::executor::SurfaceExecutor for FakeExec {
        fn observe(&self, _dir: &Path, surface: &str) -> Result<String, String> {
            self.observed.lock().unwrap().push(surface.into());
            if self.fail {
                Err("read failed".into())
            } else {
                Ok(self.reading.clone())
            }
        }
        fn invoke(&self, _dir: &Path, surface: &str, label: &str) -> Result<(), String> {
            self.invoked
                .lock()
                .unwrap()
                .push((surface.into(), label.into()));
            if self.fail {
                Err("act failed".into())
            } else {
                Ok(())
            }
        }
    }

    fn observe_request(key: &str) -> GrantRequestInput {
        GrantRequestInput {
            request_key: key.into(),
            class_id: "switchable.reversible/v1".into(),
            requested_operations: BTreeMap::from([("observe".into(), BTreeMap::new())]),
            requested_duration_seconds: Some(600),
            reason: Some("to read".into()),
        }
    }

    fn open_actuate(dir: &Path) {
        let mut boundary = familiar_kernel::boundary::Boundary::closed();
        boundary.allow_agent = true;
        boundary.allow_actuate = true;
        std::fs::write(
            dir.join(familiar_kernel::boundary::BOUNDARY_FILE),
            serde_json::to_vec(&boundary).unwrap(),
        )
        .unwrap();
    }

    /// An active grant carrying only `observe`.
    fn granted_observe(dir: &Path, context: &PartnerContext, key: &str) -> String {
        let pending = request_grant(dir, context, observe_request(key), NOW).unwrap();
        open_agent(dir);
        let receipt = grant_request(
            dir,
            &human("ian"),
            &pending.request_id,
            "ians-secret-lamp",
            BTreeMap::from([("observe".into(), BTreeMap::new())]),
            NOW + 300,
            NOW + 1,
        )
        .unwrap();
        handle(&receipt)
    }

    #[test]
    fn invoke_runs_within_bounds_records_and_leaks_nothing() {
        let dir = temp("invoke_ok");
        let ctx = context("principal-a", "key-a");
        let instance = handle(&granted(&dir, &ctx, "g")); // set_state:primary
        open_actuate(&dir);
        let exec = FakeExec::ok("private-on");
        let receipt = invoke(
            &dir,
            &ctx,
            InvokeInput {
                instance: instance.clone(),
                operation: "set_state".into(),
                parameters: BTreeMap::from([("state".into(), Value::String("primary".into()))]),
            },
            NOW + 10,
            &exec,
        )
        .expect("invoke within grant succeeds");
        // primary maps to the first bucket's act label.
        assert_eq!(
            *exec.invoked.lock().unwrap(),
            vec![("ians-secret-lamp".to_string(), "private-on".to_string())]
        );
        // The partner-facing receipt carries no private surface, label, or command.
        let raw = serde_json::to_string(&receipt).unwrap();
        for private in ["ians-secret-lamp", "private-on", "secret command"] {
            assert!(!raw.contains(private), "receipt leaked {private}");
        }
        // The effect is recorded in the ledger.
        assert!(partner_act::load(&dir)
            .unwrap()
            .iter()
            .any(|e| matches!(&e.body, PartnerActBody::Invoked { .. })));
    }

    #[test]
    fn invoke_out_of_bounds_is_refused_and_never_reaches_the_executor() {
        let dir = temp("invoke_bounds");
        let ctx = context("principal-a", "key-a");
        let instance = handle(&granted(&dir, &ctx, "g")); // primary only
        open_actuate(&dir);
        let exec = FakeExec::ok("private-on");
        let refusal = invoke(
            &dir,
            &ctx,
            InvokeInput {
                instance,
                operation: "set_state".into(),
                parameters: BTreeMap::from([("state".into(), Value::String("reverted".into()))]),
            },
            NOW + 10,
            &exec,
        )
        .unwrap_err();
        assert_eq!(refusal.code, ReasonCode::BoundsInvalid);
        assert!(
            exec.invoked.lock().unwrap().is_empty(),
            "out-of-bounds still actuated"
        );
    }

    #[test]
    fn invoke_fails_closed_when_allow_actuate_is_shut() {
        let dir = temp("invoke_gate");
        let ctx = context("principal-a", "key-a");
        let instance = handle(&granted(&dir, &ctx, "g"));
        // allow_agent is on (granted), but allow_actuate is NOT opened.
        let exec = FakeExec::ok("private-on");
        let refusal = invoke(
            &dir,
            &ctx,
            InvokeInput {
                instance,
                operation: "set_state".into(),
                parameters: BTreeMap::from([("state".into(), Value::String("primary".into()))]),
            },
            NOW + 10,
            &exec,
        )
        .unwrap_err();
        assert_eq!(refusal.code, ReasonCode::BoundaryClosed);
        assert!(
            exec.invoked.lock().unwrap().is_empty(),
            "shut gate still actuated"
        );
    }

    #[test]
    fn invoke_on_an_expired_grant_is_refused() {
        let dir = temp("invoke_expired");
        let ctx = context("principal-a", "key-a");
        let instance = handle(&granted(&dir, &ctx, "g")); // expires at NOW+300
        open_actuate(&dir);
        let exec = FakeExec::ok("private-on");
        let refusal = invoke(
            &dir,
            &ctx,
            InvokeInput {
                instance,
                operation: "set_state".into(),
                parameters: BTreeMap::from([("state".into(), Value::String("primary".into()))]),
            },
            NOW + 10_000, // well past expiry
            &exec,
        )
        .unwrap_err();
        assert_eq!(refusal.code, ReasonCode::GrantInactive);
        assert!(exec.invoked.lock().unwrap().is_empty());
    }

    #[test]
    fn invoke_with_a_bad_handle_is_refused() {
        let dir = temp("invoke_nohandle");
        let ctx = context("principal-a", "key-a");
        open_actuate(&dir);
        let exec = FakeExec::ok("private-on");
        let refusal = invoke(
            &dir,
            &ctx,
            InvokeInput {
                instance: "instance-not-a-real-handle".into(),
                operation: "set_state".into(),
                parameters: BTreeMap::from([("state".into(), Value::String("primary".into()))]),
            },
            NOW + 10,
            &exec,
        )
        .unwrap_err();
        assert_eq!(refusal.code, ReasonCode::GrantMissing);
        assert!(exec.invoked.lock().unwrap().is_empty());
    }

    #[test]
    fn a_set_state_grant_does_not_authorize_observe() {
        let dir = temp("observe_denied");
        let ctx = context("principal-a", "key-a");
        let instance = handle(&granted(&dir, &ctx, "g")); // set_state only, no observe
        let exec = FakeExec::ok("private-off");
        let refusal = observe(&dir, &ctx, ObserveInput { instance }, NOW + 10, &exec).unwrap_err();
        assert_eq!(refusal.code, ReasonCode::UnknownOperation);
        assert!(exec.observed.lock().unwrap().is_empty());
    }

    #[test]
    fn observe_reads_maps_to_abstract_and_leaks_nothing() {
        let dir = temp("observe_ok");
        let ctx = context("principal-a", "key-a");
        let instance = granted_observe(&dir, &ctx, "g");
        // The surface reports the "private-off" bucket = second bucket = abstract "reverted".
        let exec = FakeExec::ok("private-off");
        let receipt = observe(&dir, &ctx, ObserveInput { instance }, NOW + 10, &exec)
            .expect("observe within grant succeeds");
        assert_eq!(receipt.reading, "reverted");
        let raw = serde_json::to_string(&receipt).unwrap();
        for private in ["ians-secret-lamp", "private-off", "private-state-command"] {
            assert!(!raw.contains(private), "observe leaked {private}");
        }
        assert!(partner_act::load(&dir)
            .unwrap()
            .iter()
            .any(|e| matches!(&e.body, PartnerActBody::Observed { .. })));
    }

    #[test]
    fn an_execution_failure_is_a_refusal_not_a_false_success() {
        let dir = temp("invoke_execfail");
        let ctx = context("principal-a", "key-a");
        let instance = handle(&granted(&dir, &ctx, "g"));
        open_actuate(&dir);
        let exec = FakeExec::failing();
        let refusal = invoke(
            &dir,
            &ctx,
            InvokeInput {
                instance,
                operation: "set_state".into(),
                parameters: BTreeMap::from([("state".into(), Value::String("primary".into()))]),
            },
            NOW + 10,
            &exec,
        )
        .unwrap_err();
        assert_eq!(refusal.code, ReasonCode::ExecutionRefused);
        // No Invoked effect is recorded when the executor said no.
        assert!(!partner_act::load(&dir)
            .unwrap()
            .iter()
            .any(|e| matches!(&e.body, PartnerActBody::Invoked { .. })));
    }

    #[test]
    fn request_is_class_only_idempotent_and_does_not_mint_authority() {
        let dir = temp("request");
        let context = context("principal-a", "key-a");
        let first = request_grant(&dir, &context, request("one"), NOW).unwrap();
        let replay = request_grant(&dir, &context, request("one"), NOW + 1).unwrap();
        assert_eq!(first, replay);
        assert!(matches!(first.state, PublicGrantState::Pending));
        assert_eq!(partner_act::load(&dir).unwrap().len(), 1);
        let raw = serde_json::to_string(&first).unwrap();
        for private in [
            "ians-secret-lamp",
            "private-state-command",
            "secret command",
        ] {
            assert!(!raw.contains(private));
        }
    }

    #[test]
    fn an_exact_retry_survives_the_open_request_ceiling() {
        let dir = temp("request_ceiling");
        let context = context("principal-a", "key-a");
        let mut first = None;
        for index in 0..MAX_OPEN_REQUESTS {
            let receipt =
                request_grant(&dir, &context, request(&format!("request-{index}")), NOW).unwrap();
            if index == 0 {
                first = Some(receipt);
            }
        }
        let replay = request_grant(&dir, &context, request("request-0"), NOW + 1).unwrap();
        assert_eq!(replay, first.unwrap());
        assert_eq!(
            request_grant(&dir, &context, request("one-too-many"), NOW + 1)
                .unwrap_err()
                .code,
            ReasonCode::RequestLimit
        );
    }

    #[test]
    fn mismatch_replay_is_refused_and_audited() {
        let dir = temp("conflict");
        let context = context("principal-a", "key-a");
        request_grant(&dir, &context, request("same"), NOW).unwrap();
        let mut changed = request("same");
        changed.reason = Some("different".into());
        assert_eq!(
            request_grant(&dir, &context, changed, NOW + 1)
                .unwrap_err()
                .code,
            ReasonCode::IdempotencyConflict
        );
        assert_eq!(partner_act::load(&dir).unwrap().len(), 2);
    }

    #[test]
    fn only_a_named_human_under_the_global_ceiling_binds_a_private_surface() {
        let dir = temp("human");
        let context = context("principal-a", "key-a");
        let pending = request_grant(&dir, &context, request("one"), NOW).unwrap();
        assert_eq!(
            grant_request(
                &dir,
                &human("ian"),
                &pending.request_id,
                "ians-secret-lamp",
                operations(&["primary"]),
                NOW + 300,
                NOW + 1,
            )
            .unwrap_err()
            .code,
            ReasonCode::BoundaryClosed
        );
        open_agent(&dir);
        assert_eq!(
            grant_request(
                &dir,
                &human("betty"),
                &pending.request_id,
                "ians-secret-lamp",
                operations(&["primary"]),
                NOW + 300,
                NOW + 1,
            )
            .unwrap_err()
            .code,
            ReasonCode::WrongPrincipal
        );
        let receipt = grant_request(
            &dir,
            &human("ian"),
            &pending.request_id,
            "ians-secret-lamp",
            operations(&["primary"]),
            NOW + 300,
            NOW + 1,
        )
        .unwrap();
        let public = serde_json::to_string(&receipt).unwrap();
        assert!(public.contains("instance-"));
        assert!(!public.contains("ians-secret-lamp"));
        assert!(!public.contains("Workshop agent"));
        assert!(!public.contains("key-a"));
    }

    #[test]
    fn partner_and_epoch_bound_handles_revoke_immediately() {
        let dir = temp("handles");
        let a = context("principal-a", "key-a");
        let b = context("principal-b", "key-b");
        let receipt = granted(&dir, &a, "grant-a");
        let instance = handle(&receipt);
        let other_partner = granted(&dir, &b, "grant-b-partner");
        assert_ne!(instance, handle(&other_partner));
        let proposal = |_context: &PartnerContext, key: &str| ProposalInput {
            proposal_key: key.into(),
            instance: instance.clone(),
            operation: "set_state".into(),
            parameters: BTreeMap::from([("state".into(), Value::String("primary".into()))]),
            reason: None,
        };
        assert_eq!(
            propose(&dir, &b, proposal(&b, "wrong"), NOW + 2)
                .unwrap_err()
                .code,
            ReasonCode::GrantMissing
        );
        let grant_id = match &receipt.state {
            PublicGrantState::Granted { grant_id, .. } => grant_id.clone(),
            _ => unreachable!(),
        };
        revoke_grant(&dir, &human("ian"), &grant_id, NOW + 3).unwrap();
        assert_eq!(
            propose(&dir, &a, proposal(&a, "revoked"), NOW + 4)
                .unwrap_err()
                .code,
            ReasonCode::GrantInactive
        );

        let regrant = granted(&dir, &a, "grant-b");
        assert_ne!(instance, handle(&regrant));
    }

    #[test]
    fn concurrent_human_decisions_mint_at_most_one_grant() {
        let dir = temp("concurrent_decision");
        let context = context("principal-a", "key-a");
        let pending = request_grant(&dir, &context, request("one"), NOW).unwrap();
        open_agent(&dir);
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let mut joins = Vec::new();
        for device in ["ian-device-one", "ian-device-two"] {
            let dir = dir.clone();
            let request_id = pending.request_id.clone();
            let barrier = barrier.clone();
            joins.push(std::thread::spawn(move || {
                barrier.wait();
                let actor =
                    HumanDecisionContext::from_verified_mesh(device.into(), "ian".into()).unwrap();
                grant_request(
                    &dir,
                    &actor,
                    &request_id,
                    "ians-secret-lamp",
                    operations(&["primary"]),
                    NOW + 300,
                    NOW + 1,
                )
            }));
        }
        barrier.wait();
        let results: Vec<_> = joins.into_iter().map(|join| join.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            partner_act::load(&dir)
                .unwrap()
                .iter()
                .filter(|event| matches!(event.body, PartnerActBody::GrantGranted { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn proposal_is_typed_idempotent_and_has_no_private_or_actuator_result() {
        let dir = temp("proposal");
        let context = context("principal-a", "key-a");
        let receipt = granted(&dir, &context, "grant");
        let input = ProposalInput {
            proposal_key: "proposal-one".into(),
            instance: handle(&receipt),
            operation: "set_state".into(),
            parameters: BTreeMap::from([("state".into(), Value::String("primary".into()))]),
            reason: Some("quoted partner data".into()),
        };
        let first = propose(&dir, &context, input.clone(), NOW + 2).unwrap();
        let replay = propose(&dir, &context, input, NOW + 3).unwrap();
        assert_eq!(first, replay);
        let public = serde_json::to_string(&first).unwrap();
        assert!(public.contains("proposed"));
        for absent in ["completed", "ians-secret-lamp", "private-on", "key-a"] {
            assert!(!public.contains(absent));
        }
        assert!(!dir.join("actuator_state.json").exists());
    }

    #[test]
    fn closing_allow_agent_immediately_stops_new_proposals_under_an_active_grant() {
        let dir = temp("proposal_boundary");
        let context = context("principal-a", "key-a");
        let receipt = granted(&dir, &context, "grant");
        std::fs::write(
            dir.join(familiar_kernel::boundary::BOUNDARY_FILE),
            serde_json::to_vec(&familiar_kernel::boundary::Boundary::closed()).unwrap(),
        )
        .unwrap();
        let input = ProposalInput {
            proposal_key: "closed-now".into(),
            instance: handle(&receipt),
            operation: "set_state".into(),
            parameters: BTreeMap::from([("state".into(), Value::String("primary".into()))]),
            reason: None,
        };
        assert_eq!(
            propose(&dir, &context, input, NOW + 2).unwrap_err().code,
            ReasonCode::BoundaryClosed
        );
        assert!(!partner_act::load(&dir)
            .unwrap()
            .iter()
            .any(|event| matches!(event.body, PartnerActBody::ProposalSubmitted { .. })));
    }

    #[test]
    fn a_named_human_can_close_a_proposal_without_creating_an_act() {
        let dir = temp("proposal_refusal");
        let context = context("principal-a", "key-a");
        let receipt = granted(&dir, &context, "grant");
        let proposal = propose(
            &dir,
            &context,
            ProposalInput {
                proposal_key: "proposal-one".into(),
                instance: handle(&receipt),
                operation: "set_state".into(),
                parameters: BTreeMap::from([("state".into(), Value::String("primary".into()))]),
                reason: None,
            },
            NOW + 2,
        )
        .unwrap();
        assert_eq!(open_proposal_count(&dir, &context.principal).unwrap(), 1);
        assert_eq!(
            refuse_proposal(&dir, &human("betty"), &proposal.proposal_id, NOW + 3)
                .unwrap_err()
                .code,
            ReasonCode::WrongPrincipal
        );
        refuse_proposal(&dir, &human("ian"), &proposal.proposal_id, NOW + 3).unwrap();
        assert_eq!(open_proposal_count(&dir, &context.principal).unwrap(), 0);
        assert_eq!(
            refuse_proposal(&dir, &human("ian"), &proposal.proposal_id, NOW + 4)
                .unwrap_err()
                .code,
            ReasonCode::TransitionConflict
        );
        assert!(!dir.join("actuator_state.json").exists());
    }

    #[test]
    fn out_of_bounds_and_expired_proposals_refuse_and_persist() {
        let dir = temp("bounds");
        let context = context("principal-a", "key-a");
        let receipt = granted(&dir, &context, "grant");
        let bad = ProposalInput {
            proposal_key: "bad".into(),
            instance: handle(&receipt),
            operation: "set_state".into(),
            parameters: BTreeMap::from([("state".into(), Value::String("reverted".into()))]),
            reason: None,
        };
        assert_eq!(
            propose(&dir, &context, bad, NOW + 2).unwrap_err().code,
            ReasonCode::BoundsInvalid
        );
        let expired = ProposalInput {
            proposal_key: "expired".into(),
            instance: handle(&receipt),
            operation: "set_state".into(),
            parameters: BTreeMap::from([("state".into(), Value::String("primary".into()))]),
            reason: None,
        };
        assert_eq!(
            propose(&dir, &context, expired, NOW + 301)
                .unwrap_err()
                .code,
            ReasonCode::GrantInactive
        );
        let events = partner_act::load(&dir).unwrap();
        assert!(events
            .iter()
            .any(|event| matches!(event.body, PartnerActBody::GrantExpired { .. })));
    }

    #[test]
    fn reason_is_bounded_data_and_private_ledger_keeps_the_surface() {
        let dir = temp("privacy");
        let context = context("principal-a", "key-a");
        let mut oversized = request("large");
        oversized.reason = Some("x".repeat(MAX_REASON_BYTES + 1));
        assert_eq!(
            request_grant(&dir, &context, oversized, NOW)
                .unwrap_err()
                .code,
            ReasonCode::SchemaInvalid
        );
        let receipt = granted(&dir, &context, "normal");
        let public = serde_json::to_string(&receipt).unwrap();
        assert!(!public.contains("ians-secret-lamp"));
        let private = serde_json::to_string(&partner_act::load(&dir).unwrap()).unwrap();
        assert!(private.contains("ians-secret-lamp"));
        assert!(private.contains("Workshop agent"));
    }

    #[test]
    fn every_offered_class_has_a_private_surface_matcher() {
        let dir = temp("matcher");
        for class in crate::offering::CLASS_DEFS {
            assert!(
                surface_matches(&dir, "ians-secret-lamp", class.id),
                "grant matcher missing for {}",
                class.id
            );
        }
    }
}
