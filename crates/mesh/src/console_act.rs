//! Signed writes from the shared console.
//!
//! A console is a mesh member, never an ambient administrator. It proves the same
//! certificate/key binding as an observation or worldview read, stamps a fresh nonce, and may
//! perform only the deliberately narrow acts represented here:
//!
//! - disable an existing standing reaction rule (a reduction of authority),
//! - name the certified device that signed the request (never another device), or
//! - register a pre-provisioned partner or decide its private grant item, always as the
//!   established human derived from the signing device.
//!
//! Guests can read the projected worldview but cannot use this seam. The raw request bytes are
//! signed, so Swift and Rust do not need a shared JSON canonicalizer.

use crate::group::{self, Membership};
use crate::node::{fingerprint, NodeIdentity};
use crate::observe::{IngestGuard, REPLAY_WINDOW_SECS};
use crate::standing::{self, Standing};
use crate::{exactly_32, hex_decode, Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

/// One of the only remote console writes this endpoint accepts. Internally tagged JSON keeps the
/// wire legible and prevents free-form human or operation names from entering the authority seam.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConsoleAct {
    DisableRule {
        rule_id: String,
    },
    NameDevice {
        name: String,
    },
    RegisterPartner {
        registration_id: String,
    },
    DecideGrant {
        request_id: String,
        surface: String,
        allowed_operations: familiar_mcp::grant::OperationBounds,
        expires_at: i64,
        /// The human's per-grant invoke rate bound (ADR-0044). Absent (0) → the conservative
        /// default is applied; grant_request clamps to [1, ceiling]. `#[serde(default)]` keeps
        /// older console builds that never send it compatible.
        #[serde(default)]
        max_invokes_per_hour: i64,
    },
    DeclineGrant {
        request_id: String,
    },
    RevokeGrant {
        grant_id: String,
    },
    RefuseProposal {
        proposal_id: String,
    },
}

/// Identity, membership, freshness, and the narrow act itself. The node signs the exact encoded
/// bytes and sends that signature in `X-Familiar-Sig`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsoleActEnvelope {
    pub node: NodeIdentity,
    pub membership: Membership,
    pub ts: i64,
    pub nonce: String,
    pub act: ConsoleAct,
}

/// Dedicated signed read envelope. It carries no human and is never embedded in worldview.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartnerInboxEnvelope {
    pub node: NodeIdentity,
    pub membership: Membership,
    pub ts: i64,
    pub nonce: String,
}

/// Verify and apply one console act. Verification completes before any mutation; a replay never
/// reaches the store. Success returns the door's short, human-readable acknowledgement.
pub(crate) fn apply(
    dir: &Path,
    raw: &[u8],
    sig_hex: &str,
    now: i64,
    guard: &Mutex<IngestGuard>,
) -> Result<String> {
    let env: ConsoleActEnvelope = serde_json::from_slice(raw)?;
    verify_member(
        dir,
        raw,
        sig_hex,
        now,
        guard,
        &env.node,
        &env.membership,
        env.ts,
        &env.nonce,
    )?;

    match env.act {
        ConsoleAct::DisableRule { rule_id } => {
            let rule_id = rule_id.trim();
            if rule_id.is_empty() {
                return Err(Error::Malformed("console act: empty rule id".into()));
            }
            let changed = familiar_kernel::reaction_rule::set_enabled(
                dir,
                rule_id,
                false,
                &format!("disabled from console by {}", env.node.node_id),
            )?;
            if !changed {
                return Err(Error::Malformed(format!(
                    "console act: no standing rule {rule_id}"
                )));
            }
            Ok(format!("disabled rule {rule_id}"))
        }
        ConsoleAct::NameDevice { name } => {
            let name = name.trim();
            if name.is_empty() {
                return Err(Error::Malformed(
                    "console act: a device name is required".into(),
                ));
            }
            if name.chars().count() > 64 || name.chars().any(char::is_control) {
                return Err(Error::Malformed(
                    "console act: device name must be 1–64 visible characters".into(),
                ));
            }
            // Deliberately no target id in the act: a device may name only the certified key
            // that signed this request. Naming another household device is a separate human act.
            crate::device::set_name(dir, &env.node.node_id, name, now)?;
            Ok(format!("named this device {name}"))
        }
        ConsoleAct::RegisterPartner { registration_id } => {
            let actor = decision_context(dir, &env.node.node_id)?;
            familiar_mcp::partner::register_staged(dir, &actor, &registration_id)
                .map_err(registration_error)?;
            partner_inbox_reply(dir, &actor, now)
        }
        ConsoleAct::DecideGrant {
            request_id,
            surface,
            allowed_operations,
            expires_at,
            max_invokes_per_hour,
        } => {
            let actor = decision_context(dir, &env.node.node_id)?;
            // 0 = the console did not name a rate; grant_request applies the conservative
            // default and clamps to the ceiling.
            let rate = if max_invokes_per_hour <= 0 {
                familiar_mcp::grant::DEFAULT_MAX_INVOKES_PER_HOUR
            } else {
                max_invokes_per_hour
            };
            familiar_mcp::grant::grant_request(
                dir,
                &actor,
                &request_id,
                &surface,
                allowed_operations,
                expires_at,
                rate,
                now,
            )
            .map_err(grant_refusal)?;
            partner_inbox_reply(dir, &actor, now)
        }
        ConsoleAct::DeclineGrant { request_id } => {
            let actor = decision_context(dir, &env.node.node_id)?;
            familiar_mcp::grant::decline_request(dir, &actor, &request_id, now)
                .map_err(grant_refusal)?;
            partner_inbox_reply(dir, &actor, now)
        }
        ConsoleAct::RevokeGrant { grant_id } => {
            let actor = decision_context(dir, &env.node.node_id)?;
            familiar_mcp::grant::revoke_grant(dir, &actor, &grant_id, now)
                .map_err(grant_refusal)?;
            partner_inbox_reply(dir, &actor, now)
        }
        ConsoleAct::RefuseProposal { proposal_id } => {
            let actor = decision_context(dir, &env.node.node_id)?;
            familiar_mcp::grant::refuse_proposal(dir, &actor, &proposal_id, now)
                .map_err(grant_refusal)?;
            partner_inbox_reply(dir, &actor, now)
        }
    }
}

/// Verify a dedicated inbox read and assemble the allowlisted local projection.
pub(crate) fn read_partner_inbox(
    dir: &Path,
    raw: &[u8],
    sig_hex: &str,
    now: i64,
    guard: &Mutex<IngestGuard>,
) -> Result<familiar_mcp::inbox::PartnerInboxView> {
    let env: PartnerInboxEnvelope = serde_json::from_slice(raw)?;
    verify_member(
        dir,
        raw,
        sig_hex,
        now,
        guard,
        &env.node,
        &env.membership,
        env.ts,
        &env.nonce,
    )?;
    let actor = decision_context(dir, &env.node.node_id)?;
    familiar_mcp::inbox::assemble(dir, &actor, now).map_err(Error::Io)
}

#[allow(clippy::too_many_arguments)]
fn verify_member(
    dir: &Path,
    raw: &[u8],
    sig_hex: &str,
    now: i64,
    guard: &Mutex<IngestGuard>,
    node: &NodeIdentity,
    membership: &Membership,
    ts: i64,
    nonce: &str,
) -> Result<()> {
    if !familiar_kernel::boundary::load(dir)
        .map_err(Error::Io)?
        .allow_mesh
    {
        return Err(Error::Untrusted("mesh gate closed".into()));
    }
    let cred = group::load(dir)?.ok_or_else(|| Error::Untrusted("no group enrolled".into()))?;
    let group_key = cred.verifying_key()?;
    let revoked = group::load_revoked(dir).unwrap_or_default();
    group::verify_membership(membership, &group_key, &cred.group_id, now, &revoked)?;

    let public_key = exactly_32(&hex_decode(&node.pubkey)?, "node pubkey")?;
    if fingerprint(&public_key) != node.node_id
        || membership.node_pubkey != node.pubkey
        || membership.node_id != node.node_id
    {
        return Err(Error::Untrusted(
            "node identity does not match its membership".into(),
        ));
    }
    node.verify(raw, sig_hex)?;
    if (now - ts).abs() > REPLAY_WINDOW_SECS {
        return Err(Error::Untrusted("stale or future timestamp".into()));
    }
    if standing::standing_of(dir, &node.node_id) != Standing::Full {
        return Err(Error::Untrusted(
            "full standing is required for this private console surface".into(),
        ));
    }
    let mut seen = guard.lock().unwrap_or_else(|p| p.into_inner());
    if !seen.remember_nonce(&node.node_id, nonce, now) {
        return Err(Error::Untrusted("replayed nonce".into()));
    }
    Ok(())
}

fn decision_context(
    dir: &Path,
    node_id: &str,
) -> Result<familiar_mcp::partner::HumanDecisionContext> {
    let record = crate::record::find_by_key(dir, node_id)
        .ok_or_else(|| Error::Untrusted("no membership record for the signing device".into()))?;
    let human = crate::record::effective_establishment(&record)
        .map(|establishment| establishment.handle.clone())
        .ok_or_else(|| Error::Untrusted("the signing device has no established human".into()))?;
    familiar_mcp::partner::HumanDecisionContext::from_verified_mesh(node_id.to_string(), human)
        .ok_or_else(|| Error::Untrusted("the established human handle is invalid".into()))
}

fn grant_refusal(refusal: familiar_mcp::grant::Refusal) -> Error {
    Error::Malformed(format!("partner decision: {}", refusal.message))
}

fn registration_error(error: familiar_mcp::partner::RegistrationError) -> Error {
    match error {
        familiar_mcp::partner::RegistrationError::Io(message) => {
            Error::Io(std::io::Error::other(message))
        }
        other => Error::Malformed(format!("partner registration: {other}")),
    }
}

fn partner_inbox_reply(
    dir: &Path,
    actor: &familiar_mcp::partner::HumanDecisionContext,
    now: i64,
) -> Result<String> {
    let view = familiar_mcp::inbox::assemble(dir, actor, now).map_err(Error::Io)?;
    serde_json::to_string(&view).map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeKey;
    use sha2::{Digest, Sha256};
    use std::sync::atomic::{AtomicU64, Ordering};

    const NOW: i64 = 1_780_000_000;
    static NEXT: AtomicU64 = AtomicU64::new(1);

    fn fresh(tag: &str) -> std::path::PathBuf {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!(
            "familiar-console-act-{}-{tag}-{n}",
            std::process::id()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn open_mesh(dir: &Path) {
        let mut boundary = familiar_kernel::boundary::Boundary::closed();
        boundary.allow_mesh = true;
        std::fs::write(
            dir.join(familiar_kernel::boundary::BOUNDARY_FILE),
            serde_json::to_vec_pretty(&boundary).unwrap(),
        )
        .unwrap();
    }

    fn member(tag: &str) -> (std::path::PathBuf, NodeKey, group::GroupCredential) {
        let dir = fresh(tag);
        open_mesh(&dir);
        let node = NodeKey::load_or_mint(&dir, "console").unwrap();
        let cred =
            group::create_group(&dir, &node, "river", NOW, group::DEFAULT_CERT_TTL_SECS).unwrap();
        standing::grant(&dir, &node.node_id(), "test member").unwrap();
        (dir, node, cred)
    }

    fn signed(
        node: &NodeKey,
        membership: Membership,
        nonce: &str,
        act: ConsoleAct,
    ) -> (Vec<u8>, String) {
        let env = ConsoleActEnvelope {
            node: node.identity(),
            membership,
            ts: NOW,
            nonce: nonce.into(),
            act,
        };
        let raw = serde_json::to_vec(&env).unwrap();
        let sig = node.sign(&raw);
        (raw, sig)
    }

    fn signed_inbox(
        node: &NodeKey,
        membership: Membership,
        nonce: &str,
        ts: i64,
    ) -> (Vec<u8>, String) {
        let env = PartnerInboxEnvelope {
            node: node.identity(),
            membership,
            ts,
            nonce: nonce.into(),
        };
        let raw = serde_json::to_vec(&env).unwrap();
        let sig = node.sign(&raw);
        (raw, sig)
    }

    fn establish(dir: &Path, node: &NodeKey, human: &str) {
        let mut record = crate::record::MembershipRecord::guest(
            &node.node_id(),
            &node.node_id(),
            crate::enroll::Attestation {
                laws_version: 1,
                statement: "I accept the Three Laws.".into(),
                ts: NOW,
            },
            NOW,
        );
        record.identity.established = Some(crate::record::Establishment {
            handle: human.into(),
            class: crate::record::EvidenceClass::LocalIntroduction,
            artifact: format!("test-{human}"),
            at: NOW,
        });
        crate::record::save(dir, &record).unwrap();
    }

    fn stage_registration(dir: &Path, id: &str, addressed_to: &str, secret: &str) {
        let credential_file = format!("mcp/{id}.env");
        std::fs::create_dir_all(dir.join("mcp")).unwrap();
        std::fs::write(dir.join(&credential_file), format!("TOKEN={secret}\n")).unwrap();
        let mut digest = Sha256::new();
        digest.update(b"familiar-partner-credential-v1\0");
        digest.update(secret.as_bytes());
        let fingerprint = digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let pending = dir.join(familiar_mcp::partner::PENDING_REGISTRATIONS_DIR);
        std::fs::create_dir_all(&pending).unwrap();
        std::fs::write(
            pending.join(format!("{id}.json")),
            serde_json::to_vec_pretty(&serde_json::json!({
                "version": 1,
                "id": id,
                "alias": "Envoy (on-device)",
                "credential_file": credential_file,
                "credential_key": "TOKEN",
                "credential_fingerprint": fingerprint,
                "addressed_to": addressed_to,
                "created_at": NOW
            }))
            .unwrap(),
        )
        .unwrap();
    }

    fn partner_request(dir: &Path, human: &str) -> (String, familiar_mcp::partner::PartnerContext) {
        std::fs::create_dir_all(dir.join("mcp")).unwrap();
        let context = familiar_mcp::partner::PartnerContext {
            principal: format!("principal-{human}"),
            credential_fingerprint: format!("fingerprint-{human}"),
            alias: format!("{human} partner"),
        };
        let registry = familiar_mcp::partner::PrincipalRegistry {
            principals: vec![familiar_mcp::partner::PrincipalRecord {
                id: context.principal.clone(),
                alias: context.alias.clone(),
                credential_file: "mcp/unused.env".into(),
                credential_key: "TOKEN".into(),
                credential_fingerprint: context.credential_fingerprint.clone(),
                registered_by: human.into(),
                enabled: true,
            }],
        };
        std::fs::write(
            dir.join(familiar_mcp::partner::PRINCIPALS_FILE),
            serde_json::to_vec(&registry).unwrap(),
        )
        .unwrap();
        let declaration = serde_json::json!({"actuators":[{
            "surface":"private-lamp", "state_cmd":"private",
            "state":{"fields":{"power":{"kind":"enum","values":["on","off"],
                "source":{"kind":"json","key":"power"}}}},
            "actions":{"on":"secret on","off":"secret off"},
            "buckets":[{"name":"on","when":[{"op":"eq","field":"power","value":"off"}]},
                       {"name":"off","when":[]}],
            "roles":{"primary":"on","reverted":"off"}
        }]});
        std::fs::write(
            dir.join(familiar_kernel::actuator::ACTUATORS_FILE),
            serde_json::to_vec(&declaration).unwrap(),
        )
        .unwrap();
        let operations = std::collections::BTreeMap::from([(
            "set_state".into(),
            std::collections::BTreeMap::from([(
                "state".into(),
                familiar_mcp::grant::ParameterBound::Enum {
                    values: vec!["primary".into(), "reverted".into()],
                },
            )]),
        )]);
        let receipt = familiar_mcp::grant::request_grant(
            dir,
            &context,
            familiar_mcp::grant::GrantRequestInput {
                request_key: format!("request-{human}"),
                class_id: "switchable.reversible/v1".into(),
                requested_operations: operations,
                requested_duration_seconds: Some(300),
                reason: Some("please consider this".into()),
            },
            NOW,
        )
        .unwrap();
        (receipt.request_id, context)
    }

    #[test]
    fn member_disables_a_rule_and_replay_is_refused() {
        let (dir, node, cred) = member("disable");
        let rule = familiar_kernel::reaction_rule::mint(
            &dir,
            "ian",
            familiar_kernel::reaction_rule::Trigger::Away,
            "lights",
            "dim",
            "test",
            NOW,
        )
        .unwrap();
        let (raw, sig) = signed(
            &node,
            cred.membership,
            "disable-1",
            ConsoleAct::DisableRule {
                rule_id: rule.id.clone(),
            },
        );
        let guard = Mutex::new(IngestGuard::default());

        assert_eq!(
            apply(&dir, &raw, &sig, NOW, &guard).unwrap(),
            format!("disabled rule {}", rule.id)
        );
        let stored = familiar_kernel::reaction_rule::load(&dir);
        assert!(!stored.rules[0].enabled);
        assert!(stored.rules[0].disabled_reason.contains(&node.node_id()));
        assert!(matches!(
            apply(&dir, &raw, &sig, NOW, &guard),
            Err(Error::Untrusted(m)) if m.contains("replayed")
        ));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn member_names_only_the_signing_device() {
        let (dir, node, cred) = member("name");
        let (raw, sig) = signed(
            &node,
            cred.membership,
            "name-1",
            ConsoleAct::NameDevice {
                name: "Aphelion".into(),
            },
        );

        assert_eq!(
            apply(&dir, &raw, &sig, NOW, &Mutex::new(IngestGuard::default())).unwrap(),
            "named this device Aphelion"
        );
        let record = crate::device::load(&dir, &node.node_id()).unwrap().unwrap();
        assert_eq!(record.device_id, node.node_id());
        assert_eq!(record.name, "Aphelion");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn registration_act_names_only_a_staging_id_and_derives_the_human() {
        let (dir, node, cred) = member("partner-register");
        establish(&dir, &node, "ian");
        let id = "registration-0123456789abcdef";
        stage_registration(&dir, id, "ian", "envoy-secret");
        let (raw, sig) = signed(
            &node,
            cred.membership,
            "partner-register-1",
            ConsoleAct::RegisterPartner {
                registration_id: id.into(),
            },
        );
        let wire = String::from_utf8(raw.clone()).unwrap();
        assert!(wire.contains(id));
        for forbidden in [
            "ian",
            "Envoy (on-device)",
            "envoy-secret",
            "credential_file",
            "credential_key",
            "registered_by",
        ] {
            assert!(!wire.contains(forbidden), "wire leaked {forbidden}");
        }

        let reply = apply(&dir, &raw, &sig, NOW, &Mutex::new(IngestGuard::default())).unwrap();
        let view: serde_json::Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(view["pending_registrations"], serde_json::json!([]));
        let registry = familiar_mcp::partner::load(&dir).unwrap();
        assert_eq!(registry.principals.len(), 1);
        assert_eq!(registry.principals[0].registered_by, "ian");
        assert_eq!(registry.principals[0].alias, "Envoy (on-device)");
        assert!(familiar_mcp::partner::authenticate(&dir, "envoy-secret").is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn certified_guest_cannot_write() {
        let (dir, _member_node, cred) = member("guest-door");
        let guest_dir = fresh("guest-key");
        let guest = NodeKey::load_or_mint(&guest_dir, "visitor").unwrap();
        let membership = cred
            .mint_membership(
                &guest.node_id(),
                &guest.identity().pubkey,
                NOW,
                group::DEFAULT_CERT_TTL_SECS,
            )
            .unwrap();
        let (raw, sig) = signed(
            &guest,
            membership.clone(),
            "guest-1",
            ConsoleAct::NameDevice {
                name: "Not mine".into(),
            },
        );

        assert!(matches!(
            apply(
                &dir,
                &raw,
                &sig,
                NOW,
                &Mutex::new(IngestGuard::default())
            ),
            Err(Error::Untrusted(m)) if m.contains("full standing")
        ));
        assert!(crate::device::load(&dir, &guest.node_id())
            .unwrap()
            .is_none());
        let (read_raw, read_sig) = signed_inbox(&guest, membership, "guest-read-1", NOW);
        assert!(matches!(
            read_partner_inbox(
                &dir,
                &read_raw,
                &read_sig,
                NOW,
                &Mutex::new(IngestGuard::default())
            ),
            Err(Error::Untrusted(message)) if message.contains("full standing")
        ));
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(guest_dir);
    }

    #[test]
    fn partner_actor_is_derived_from_the_verified_device_and_payload_has_no_human() {
        let (dir, node, cred) = member("partner-derived");
        establish(&dir, &node, "ian");
        let (request_id, _) = partner_request(&dir, "ian");
        let (raw, sig) = signed(
            &node,
            cred.membership,
            "partner-decline-1",
            ConsoleAct::DeclineGrant {
                request_id: request_id.clone(),
            },
        );
        let wire = String::from_utf8(raw.clone()).unwrap();
        assert!(!wire.contains("ian"));
        assert!(!wire.contains("human"));
        let reply = apply(&dir, &raw, &sig, NOW, &Mutex::new(IngestGuard::default())).unwrap();
        let view: serde_json::Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(view["pending_requests"], serde_json::json!([]));
        let event = familiar_mcp::partner_act::load(&dir)
            .unwrap()
            .into_iter()
            .find(|event| {
                matches!(
                    &event.body,
                    familiar_mcp::partner_act::PartnerActBody::GrantDeclined {
                        request_id: id,
                        ..
                    } if id == &request_id
                )
            })
            .unwrap();
        assert_eq!(
            event.actor,
            familiar_mcp::partner_act::PartnerActor::Human("ian".into())
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn another_established_human_gets_no_rows_and_cannot_decide() {
        let (dir, _ian_node, cred) = member("partner-other-human");
        let (request_id, _) = partner_request(&dir, "ian");
        let other_dir = fresh("partner-other-key");
        let betty = NodeKey::load_or_mint(&other_dir, "betty phone").unwrap();
        let membership = cred
            .mint_membership(
                &betty.node_id(),
                &betty.identity().pubkey,
                NOW,
                group::DEFAULT_CERT_TTL_SECS,
            )
            .unwrap();
        standing::grant(&dir, &betty.node_id(), "full but another human").unwrap();
        establish(&dir, &betty, "betty");
        let guard = Mutex::new(IngestGuard::default());
        let (read_raw, read_sig) = signed_inbox(&betty, membership.clone(), "betty-read-1", NOW);
        let view = read_partner_inbox(&dir, &read_raw, &read_sig, NOW, &guard).unwrap();
        assert!(view.pending_requests.is_empty());

        let (act_raw, act_sig) = signed(
            &betty,
            membership,
            "betty-decline-1",
            ConsoleAct::DeclineGrant { request_id },
        );
        assert!(matches!(
            apply(&dir, &act_raw, &act_sig, NOW, &guard),
            Err(Error::Malformed(message)) if message.contains("not the registered addressee")
        ));
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(other_dir);
    }

    #[test]
    fn full_standing_without_an_established_human_cannot_read_or_decide() {
        let (dir, _founder, cred) = member("partner-no-human");
        let (request_id, _) = partner_request(&dir, "ian");
        let unestablished_dir = fresh("partner-no-human-key");
        let node = NodeKey::load_or_mint(&unestablished_dir, "shared station").unwrap();
        let membership = cred
            .mint_membership(
                &node.node_id(),
                &node.identity().pubkey,
                NOW,
                group::DEFAULT_CERT_TTL_SECS,
            )
            .unwrap();
        standing::grant(&dir, &node.node_id(), "full but no human").unwrap();
        let record = crate::record::MembershipRecord::guest(
            &node.node_id(),
            &node.node_id(),
            crate::enroll::Attestation {
                laws_version: 1,
                statement: "I accept the Three Laws.".into(),
                ts: NOW,
            },
            NOW,
        );
        crate::record::save(&dir, &record).unwrap();
        let guard = Mutex::new(IngestGuard::default());
        let (read_raw, read_sig) =
            signed_inbox(&node, membership.clone(), "unestablished-read-1", NOW);
        assert!(matches!(
            read_partner_inbox(&dir, &read_raw, &read_sig, NOW, &guard),
            Err(Error::Untrusted(message)) if message.contains("no established human")
        ));

        let (act_raw, act_sig) = signed(
            &node,
            membership,
            "unestablished-decline-1",
            ConsoleAct::DeclineGrant { request_id },
        );
        assert!(matches!(
            apply(&dir, &act_raw, &act_sig, NOW, &guard),
            Err(Error::Untrusted(message)) if message.contains("no established human")
        ));
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(unestablished_dir);
    }

    #[test]
    fn inbox_read_is_fresh_signed_and_replay_protected() {
        let (dir, node, cred) = member("partner-read-replay");
        establish(&dir, &node, "ian");
        let _ = partner_request(&dir, "ian");
        stage_registration(
            &dir,
            "registration-private-inbox",
            "ian",
            "private-envoy-secret",
        );
        let worldview =
            serde_json::to_string(&crate::worldview::assemble_worldview(&dir, &cred, NOW).unwrap())
                .unwrap();
        for private_value in [
            "private-lamp",
            "ian partner",
            "fingerprint-ian",
            "please consider this",
            "Envoy (on-device)",
            "registration-private-inbox",
            "private-envoy-secret",
        ] {
            assert!(!worldview.contains(private_value));
        }
        let guard = Mutex::new(IngestGuard::default());
        let (stale_raw, stale_sig) = signed_inbox(
            &node,
            cred.membership.clone(),
            "stale-read",
            NOW - REPLAY_WINDOW_SECS - 1,
        );
        assert!(matches!(
            read_partner_inbox(&dir, &stale_raw, &stale_sig, NOW, &guard),
            Err(Error::Untrusted(message)) if message.contains("stale")
        ));

        let (raw, sig) = signed_inbox(&node, cred.membership, "read-once", NOW);
        let forger_dir = fresh("partner-read-forger");
        let forger = NodeKey::load_or_mint(&forger_dir, "forger").unwrap();
        let forged_sig = forger.sign(&raw);
        assert!(read_partner_inbox(&dir, &raw, &forged_sig, NOW, &guard).is_err());
        let inbox = read_partner_inbox(&dir, &raw, &sig, NOW, &guard).unwrap();
        assert_eq!(inbox.pending_requests.len(), 1);
        assert_eq!(inbox.pending_registrations.len(), 1);
        assert!(matches!(
            read_partner_inbox(&dir, &raw, &sig, NOW, &guard),
            Err(Error::Untrusted(message)) if message.contains("replayed")
        ));
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(forger_dir);
    }
}
