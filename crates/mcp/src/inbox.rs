//! Private, human-addressed projection of the rung-3 partner ledger.
//!
//! This is deliberately not a worldview or MCP resource. It joins partner identity to local
//! surface ids only after a signed mesh door has derived [`HumanDecisionContext`]. Any corrupt
//! or impossible fold fails the whole view; an authority surface must never make a partial
//! ledger look complete.

use crate::grant::OperationBounds;
use crate::partner::{self, HumanDecisionContext};
use crate::partner_act::{self, PartnerAct, PartnerActBody};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PartnerInboxView {
    pub pending_registrations: Vec<partner::PendingRegistrationView>,
    pub pending_requests: Vec<HumanGrantRequestView>,
    pub active_grants: Vec<HumanGrantView>,
    pub pending_proposals: Vec<HumanProposalView>,
    /// Local declaration problems. These are shown to the human and never enter partner output.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HumanGrantRequestView {
    pub request_id: String,
    pub partner_alias: String,
    pub credential_fingerprint: String,
    pub class_id: String,
    pub requested_operations: OperationBounds,
    pub requested_duration_seconds: Option<i64>,
    pub reason_quote: Option<String>,
    pub eligible_surfaces: Vec<PrivateSurfaceChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrivateSurfaceChoice {
    pub surface: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HumanGrantView {
    pub grant_id: String,
    pub partner_alias: String,
    pub credential_fingerprint: String,
    pub surface: String,
    pub allowed_operations: OperationBounds,
    pub expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HumanProposalView {
    pub proposal_id: String,
    pub partner_alias: String,
    pub credential_fingerprint: String,
    pub surface: String,
    pub class_id: String,
    pub operation: String,
    pub parameters: BTreeMap<String, Value>,
    pub reason_quote: Option<String>,
}

#[derive(Debug, Clone)]
struct GrantJoin {
    principal: String,
    partner_alias: String,
    credential_fingerprint: String,
    surface: String,
    allowed_operations: OperationBounds,
    expires_at: i64,
}

pub fn assemble(
    dir: &Path,
    actor: &HumanDecisionContext,
    now: i64,
) -> io::Result<PartnerInboxView> {
    let pending_registrations = partner::pending_for(dir, actor)?;
    let registry = partner::load(dir)?;
    let addressed: BTreeSet<String> = registry
        .principals
        .into_iter()
        .filter(|record| record.enabled && record.registered_by == actor.human())
        .map(|record| record.id)
        .collect();
    let events = partner_act::load(dir)?;
    let events: Vec<&PartnerAct> = events
        .iter()
        .filter(|event| addressed.contains(&event.principal))
        .collect();

    let mut request_terminals = BTreeSet::new();
    let mut grant_terminals = BTreeSet::new();
    let mut proposal_terminals = BTreeSet::new();
    for event in &events {
        match &event.body {
            PartnerActBody::GrantGranted { request_id, .. }
            | PartnerActBody::GrantDeclined { request_id, .. } => {
                request_terminals.insert(request_id.clone());
            }
            PartnerActBody::GrantRevoked { grant_id, .. }
            | PartnerActBody::GrantExpired { grant_id, .. } => {
                grant_terminals.insert(grant_id.clone());
            }
            PartnerActBody::ProposalRefused {
                proposal_id: Some(id),
                ..
            }
            | PartnerActBody::ProposalWithdrawn {
                proposal_id: id, ..
            } => {
                proposal_terminals.insert(id.clone());
            }
            _ => {}
        }
    }

    let (surfaces, invalid_surfaces) = familiar_kernel::actuator::load(dir)?;
    let warnings = invalid_surfaces
        .into_iter()
        .map(|surface| format!("{surface} is not offered because its declaration is invalid"))
        .collect();

    let mut pending_requests = Vec::new();
    let mut grants = BTreeMap::<String, GrantJoin>::new();
    for event in &events {
        match &event.body {
            PartnerActBody::GrantRequested {
                request_id,
                class_id,
                requested_operations,
                requested_duration_seconds,
                reason,
                ..
            } if !request_terminals.contains(request_id) => {
                let requested_operations = decode_bounds(requested_operations)?;
                let eligible_surfaces = surfaces
                    .iter()
                    .filter(|surface| crate::grant::actuator_matches_class(surface, class_id))
                    .map(|surface| PrivateSurfaceChoice {
                        surface: surface.surface.clone(),
                        description: surface.description.clone(),
                    })
                    .collect();
                pending_requests.push(HumanGrantRequestView {
                    request_id: request_id.clone(),
                    partner_alias: event.alias_snapshot.clone(),
                    credential_fingerprint: event.credential.clone(),
                    class_id: class_id.clone(),
                    requested_operations,
                    requested_duration_seconds: *requested_duration_seconds,
                    reason_quote: reason.clone(),
                    eligible_surfaces,
                });
            }
            PartnerActBody::GrantGranted {
                request_id,
                grant_id,
                surface,
                allowed_operations,
                expires_at,
                ..
            } => {
                if !events.iter().any(|candidate| {
                    candidate.principal == event.principal
                        && matches!(
                            &candidate.body,
                            PartnerActBody::GrantRequested { request_id: id, .. } if id == request_id
                        )
                }) {
                    return Err(invalid("grant references a missing request"));
                }
                if grants
                    .insert(
                        grant_id.clone(),
                        GrantJoin {
                            principal: event.principal.clone(),
                            partner_alias: event.alias_snapshot.clone(),
                            credential_fingerprint: event.credential.clone(),
                            surface: surface.clone(),
                            allowed_operations: decode_bounds(allowed_operations)?,
                            expires_at: *expires_at,
                        },
                    )
                    .is_some()
                {
                    return Err(invalid("duplicate grant in partner ledger"));
                }
            }
            _ => {}
        }
    }

    let active_grants = grants
        .iter()
        .filter(|(id, grant)| !grant_terminals.contains(*id) && grant.expires_at > now)
        .map(|(grant_id, grant)| HumanGrantView {
            grant_id: grant_id.clone(),
            partner_alias: grant.partner_alias.clone(),
            credential_fingerprint: grant.credential_fingerprint.clone(),
            surface: grant.surface.clone(),
            allowed_operations: grant.allowed_operations.clone(),
            expires_at: grant.expires_at,
        })
        .collect();

    let mut pending_proposals = Vec::new();
    for event in &events {
        if let PartnerActBody::ProposalSubmitted {
            proposal_id,
            grant_id,
            class_id,
            operation,
            parameters,
            reason,
            ..
        } = &event.body
        {
            if proposal_terminals.contains(proposal_id) {
                continue;
            }
            let grant = grants
                .get(grant_id)
                .ok_or_else(|| invalid("proposal references a missing grant"))?;
            if grant.principal != event.principal {
                return Err(invalid("proposal and grant principals differ"));
            }
            let parameters = serde_json::from_value::<BTreeMap<String, Value>>(parameters.clone())
                .map_err(|_| invalid("proposal parameters are not an object"))?;
            pending_proposals.push(HumanProposalView {
                proposal_id: proposal_id.clone(),
                partner_alias: event.alias_snapshot.clone(),
                credential_fingerprint: event.credential.clone(),
                surface: grant.surface.clone(),
                class_id: class_id.clone(),
                operation: operation.clone(),
                parameters,
                reason_quote: reason.clone(),
            });
        }
    }

    Ok(PartnerInboxView {
        pending_registrations,
        pending_requests,
        active_grants,
        pending_proposals,
        warnings,
    })
}

fn decode_bounds(value: &Value) -> io::Result<OperationBounds> {
    serde_json::from_value(value.clone()).map_err(|_| invalid("operation bounds are malformed"))
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grant::{self, GrantRequestInput, ParameterBound, ParameterBounds};
    use crate::partner::{PartnerContext, PrincipalRecord, PrincipalRegistry};

    const NOW: i64 = 20_000;

    fn actor(human: &str) -> HumanDecisionContext {
        HumanDecisionContext::from_verified_mesh(format!("{human}-device"), human.into()).unwrap()
    }

    // One tag per TEST: temp_root deletes and recreates the tagged directory, and the
    // harness runs tests concurrently in one process — a shared tag lets one test sweep
    // the fixture out from under another's open ledger (seen as a sqlite disk I/O error).
    fn setup(tag: &str) -> (std::path::PathBuf, PartnerContext, PartnerContext) {
        let dir = familiar_kernel::testing::temp_root(tag);
        std::fs::create_dir_all(dir.join("mcp")).unwrap();
        let records = [
            ("p-ian", "Ian partner", "ian"),
            ("p-betty", "Betty partner", "betty"),
        ]
        .into_iter()
        .map(|(id, alias, human)| PrincipalRecord {
            id: id.into(),
            alias: alias.into(),
            credential_file: "mcp/unused.env".into(),
            credential_key: "TOKEN".into(),
            credential_fingerprint: format!("fp-{id}"),
            registered_by: human.into(),
            enabled: true,
        })
        .collect();
        std::fs::write(
            dir.join(partner::PRINCIPALS_FILE),
            serde_json::to_vec(&PrincipalRegistry {
                principals: records,
            })
            .unwrap(),
        )
        .unwrap();
        let declaration = serde_json::json!({"actuators":[{
            "surface":"private-lamp", "description":"the reading lamp",
            "state_cmd":"private", "state":{"fields":{"power":{"kind":"enum",
                "values":["on","off"],"source":{"kind":"json","key":"power"}}}},
            "actions":{"on":"secret on","off":"secret off"},
            "buckets":[{"name":"on","when":[{"op":"eq","field":"power","value":"off"}]},{"name":"off","when":[]}]
        }]});
        std::fs::write(
            dir.join(familiar_kernel::actuator::ACTUATORS_FILE),
            serde_json::to_vec(&declaration).unwrap(),
        )
        .unwrap();
        (
            dir,
            PartnerContext {
                principal: "p-ian".into(),
                credential_fingerprint: "fp-p-ian".into(),
                alias: "Ian partner".into(),
            },
            PartnerContext {
                principal: "p-betty".into(),
                credential_fingerprint: "fp-p-betty".into(),
                alias: "Betty partner".into(),
            },
        )
    }

    fn operations() -> OperationBounds {
        BTreeMap::from([(
            "set_state".into(),
            ParameterBounds::from([(
                "state".into(),
                ParameterBound::Enum {
                    values: vec!["primary".into(), "reverted".into()],
                },
            )]),
        )])
    }

    #[test]
    fn projection_is_private_to_the_registered_human_and_joins_only_valid_surfaces() {
        let (dir, ian, betty) = setup("partner_inbox_private");
        for (context, key) in [(&ian, "ian-request"), (&betty, "betty-request")] {
            grant::request_grant(
                &dir,
                context,
                GrantRequestInput {
                    request_key: key.into(),
                    class_id: "switchable.reversible/v1".into(),
                    requested_operations: operations(),
                    requested_duration_seconds: Some(300),
                    reason: Some(format!("private reason from {key}")),
                },
                NOW,
            )
            .unwrap();
        }

        let view = assemble(&dir, &actor("ian"), NOW).unwrap();
        assert_eq!(view.pending_requests.len(), 1);
        assert_eq!(view.pending_requests[0].partner_alias, "Ian partner");
        assert_eq!(
            view.pending_requests[0].eligible_surfaces[0].surface,
            "private-lamp"
        );
        let raw = serde_json::to_string(&view).unwrap();
        assert!(!raw.contains("Betty partner"));
        assert!(!raw.contains("betty-request"));
    }

    #[test]
    fn legacy_principals_are_not_assigned_and_corruption_fails_the_whole_view() {
        let (dir, _, _) = setup("partner_inbox_legacy");
        let mut registry = partner::load(&dir).unwrap();
        registry.principals[0].registered_by.clear();
        std::fs::write(
            dir.join(partner::PRINCIPALS_FILE),
            serde_json::to_vec(&registry).unwrap(),
        )
        .unwrap();
        assert!(assemble(&dir, &actor("ian"), NOW)
            .unwrap()
            .pending_requests
            .is_empty());
        std::fs::write(dir.join(partner::PRINCIPALS_FILE), "{ broken").unwrap();
        assert!(assemble(&dir, &actor("ian"), NOW).is_err());
    }
}
