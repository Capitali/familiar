//! The mesh **brief** — what one node tells another.
//!
//! A brief carries the node's identity + its membership certificate, a small presence
//! summary (counts, never names), a capability manifest (host facts + a manifest of
//! shareable tools — *bodies* fetched on demand, not inlined), offered knowledge/patterns,
//! and — only when a human has opted a handle in — a scoped identity payload. The whole
//! body is signed by the node key; the membership cert binds that node key to the group.
//!
//! Verifying a brief is therefore two linked checks:
//! 1. the **membership cert** verifies against the group public key (peer is in-group), and
//! 2. the **brief signature** verifies against the node pubkey the cert certifies.
//!
//! Only then is the brief trusted enough to hand to the in-tick merge. Freshness
//! (`ts`/`nonce`) is enforced by the transport layer against replay.

use crate::group::{verify_membership, Membership};
use crate::node::{NodeIdentity, NodeKey};
use crate::Result;
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};

/// Wire/brief format version — bump on incompatible changes to the signed body.
pub const BRIEF_VERSION: u32 = 5;

/// `skip_serializing_if` helper: verifiers re-serialize the body ([`BriefBody::signing_bytes`]),
/// so a field an older peer doesn't know about breaks every signature it checks. Omitting the
/// zero default keeps briefs byte-identical to pre-`build_version` builds in both directions.
fn u64_zero(v: &u64) -> bool {
    *v == 0
}

/// Same as [`u64_zero`], for the i64 lifecycle timestamps.
fn u64_zero_i64(v: &i64) -> bool {
    *v == 0
}

/// Presence: how busy this node is and when it last served — **counts, never names**.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Presence {
    pub observer_count: u32,
    pub last_active: i64,
}

/// A shareable tool, described but not carried. The body is fetched on demand via
/// `GET /mesh/tool/{tool_id}` and re-hashed against `script_sha256` before use, so a brief
/// stays small and a node pulls only tools it lacks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolManifest {
    pub tool_id: String,
    pub name: String,
    pub purpose: String,
    pub keywords: Vec<String>,
    /// SHA-256 of the tool script body (hex) — the content address used to dedup + verify.
    pub script_sha256: String,
    pub uses: u64,
    pub last_exit_ok: bool,
}

/// Host capability summary + the tool manifest offered to peers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Capability {
    pub os: String,
    pub arch: String,
    pub env_summary: String,
    /// The familiar build this peer runs (crate version) — for the roster. `#[serde(default)]` for
    /// briefs that predate it.
    #[serde(default)]
    pub familiar_version: String,
    /// The OS release ("Ubuntu 24.04", "macOS 15.5") — the roster's OS-version detail. Empty on
    /// briefs that predate it.
    #[serde(default)]
    pub os_version: String,
    pub tools: Vec<ToolManifest>,
    /// What this node can actually *do* — `build-rust`, `build-apple`, `deploy-apple`, `execute`,
    /// `agent`, `llm`, … (discovered toolchain ∩ open gates; see `familiar_kernel::capabilities`).
    /// The mesh routes a goal to a node whose capabilities satisfy its `needs`. Empty on briefs that
    /// predate the field.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// The **orderable release version** of the core this node runs (`familiar_kernel::version`).
    /// Distinct from `familiar_version` (the static crate version): this is the monotonic counter
    /// self-upgrade compares, so a node can see a peer is running a newer blessed release. 0 on
    /// briefs that predate it / unstamped builds.
    #[serde(default, skip_serializing_if = "u64_zero")]
    pub build_version: u64,
    /// This node has an interactive human at its console (`!headless`). Skip-when-false so
    /// briefs from headless nodes stay byte-identical for pre-field verifiers.
    #[serde(default, skip_serializing_if = "bool_false")]
    pub interactive: bool,
    /// The human handle this node serves — shared only when that handle is opted into the
    /// group (`identity_optin`), the same consent gate as identity shares. Empty otherwise.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub human: String,
}

/// `skip_serializing_if` helper for the `interactive` flag (see [`u64_zero`]).
fn bool_false(b: &bool) -> bool {
    !*b
}

/// An abstract pattern offered for merge — never raw private data, a distilled regularity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternOffer {
    pub key: String,
    pub summary: String,
    pub support: u32,
}

/// One observation shared for replication. Carries its **origin node** so peers dedup globally and
/// preserve provenance — observation ids are node-local (`obs-NNNN`), so an id alone can't dedup
/// across nodes; the receiver keys on a content hash of (origin, actor, action, object, ts).
/// Derived data only, same discipline as everything that crosses the mesh. `confidence_pct` is an
/// integer so the brief body stays `Eq`/byte-deterministic for signing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObsShare {
    pub origin: String,
    pub actor: String,
    pub action: String,
    pub object: String,
    pub context: String,
    pub ts: i64,
    pub confidence_pct: u8,
}

/// A theory a node formed but **cannot test locally** (it can't write/execute code — `allow_execute`
/// off), offered to the mesh so a peer that CAN test it will. Distributed cognition: theorists
/// delegate testing to executors. Carries its origin so the outcome can find its way home.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TheoryRequest {
    pub origin: String,
    pub thread_id: String,
    pub question: String,
    /// What to *do* to test the theory — becomes a candidate's hypothesis on the executor.
    pub direction: String,
}

/// Knowledge offered: distilled patterns, a non-identifying observation summary, the recent
/// observation records themselves (when `share_observations` is on), and theories this node couldn't
/// test locally — so every peer holds the shared record, backs up a peer that goes away, and lends
/// its execution to a peer that has ideas but no way to try them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Knowledge {
    pub patterns: Vec<PatternOffer>,
    pub obs_summary: String,
    /// Recent observations for replication. `#[serde(default)]` so a brief from a node that predates
    /// this field still deserializes (as none).
    #[serde(default)]
    pub observations: Vec<ObsShare>,
    /// Theories this node can't test locally, seeking a peer that can. `#[serde(default)]` for
    /// back-compat with briefs that predate the field.
    #[serde(default)]
    pub theory_requests: Vec<TheoryRequest>,
    /// The shared roadmap — goals every node holds and burns down together. `#[serde(default)]` for
    /// back-compat with briefs that predate the field.
    #[serde(default)]
    pub goals: Vec<GoalShare>,
}

/// A goal shared for replication — the roadmap made mesh-native. Every node holds the same goal
/// list and its live status, so the mesh burns the roadmap down together: whoever's capabilities fit
/// claims it, and progress/ownership travels back to all. Deduped by `id` (goal ids are minted by the
/// seeding node and carried verbatim, unlike node-local observation ids). Mirrors `goal::Goal`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalShare {
    pub id: String,
    pub description: String,
    pub needs: Vec<String>,
    /// The goal's status as a slug ("proposed"/"claimed"/"in_progress"/"awaiting_human"/"done"/…).
    pub status: String,
    pub owner_node: String,
    pub origin: String,
    pub produced: String,
    pub notes: String,
    pub created_at: i64,
    pub updated_at: i64,
    /// Lifecycle dates (mirrors `goal::Goal`) — every status carries the date it was entered.
    /// All skip-when-zero so briefs stay byte-identical for verifiers built before these fields
    /// (they re-serialize the signed body; an unknown field would break every signature).
    #[serde(default, skip_serializing_if = "u64_zero_i64")]
    pub status_at: i64,
    #[serde(default, skip_serializing_if = "u64_zero_i64")]
    pub last_worked_at: i64,
    #[serde(default, skip_serializing_if = "u64_zero_i64")]
    pub completed_at: i64,
    #[serde(default, skip_serializing_if = "u64_zero_i64")]
    pub ended_at: i64,
}

/// A single opted-in human, shared only under explicit per-handle/per-group consent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityShare {
    pub handle: String,
    pub relation: String,
    /// The group this share was scoped to — a share never leaks beyond its group.
    pub group: String,
}

/// A human-gated act a **headless** node can't perform alone (it has no local human), routed to
/// human-facing peers so a human there can act. Authority always originates from a human — this only
/// moves *where* that human sits, never removes them. `kind` is "enrollment" | "question" (gate-open
/// is deliberately NOT proxied yet — that would change the boundary safety invariant).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityRequest {
    pub origin: String,
    pub kind: String,
    /// What the decision is about — a node id (enrollment) or a question id.
    pub ref_id: String,
    /// Human-legible summary to surface at the deciding peer.
    pub summary: String,
}

/// A human's decision on an [`AuthorityRequest`], relayed back to the node that asked. Carried in the
/// **granting node's signed brief**, so it is authenticated as "this member asserts a human here
/// decided X" — the covenant trust that a member only emits a grant when its human actually acted.
/// The target applies it: mint/deny an enrollment, record a question's answer, or — the one path that
/// writes the boundary — open a gate the target requested. That boundary write happens ONLY here, on
/// an authenticated human grant; the autonomous cycle still has no boundary-write path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityGrant {
    /// The node whose human made the decision.
    pub by: String,
    /// The node the decision is for (applies it).
    pub target: String,
    /// "enrollment" | "question" | "gate".
    pub kind: String,
    /// The subject: a node id (enrollment), a question id, or a gate name (`allow_execute`, …).
    pub ref_id: String,
    pub approved: bool,
    /// Optional human note (e.g. the answer to a question).
    #[serde(default)]
    pub note: String,
    /// When the human decided — for pruning old grants.
    pub ts: i64,
}

/// The scoped identity payload — present on a brief **only** when opt-in applies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ConsentedIdentityPayload {
    pub entries: Vec<IdentityShare>,
}

/// The signed body of a brief. Field order is fixed (serde derive, no maps) so the bytes
/// are deterministic across nodes and runs — that determinism is what makes the signature
/// verifiable elsewhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BriefBody {
    pub version: u32,
    pub node: NodeIdentity,
    pub membership: Membership,
    /// Unix seconds when built — freshness for replay defense.
    pub ts: i64,
    /// Random per-brief nonce (hex) — replay/dup defense at the transport layer.
    pub nonce: String,
    pub presence: Presence,
    pub capability: Capability,
    pub knowledge: Knowledge,
    /// Present only when a human opted a handle into this group's sharing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identities: Option<ConsentedIdentityPayload>,
    /// Human-authority needs a headless node routes to human-facing peers. `#[serde(default)]` for
    /// back-compat; empty on nodes that have their own human.
    #[serde(default)]
    pub authority_requests: Vec<AuthorityRequest>,
    /// Decisions a human here made on peers' authority requests, relayed back for them to apply.
    #[serde(default)]
    pub authority_grants: Vec<AuthorityGrant>,
}

impl BriefBody {
    /// The exact bytes signed / verified — deterministic JSON of the body.
    pub fn signing_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }
}

/// A signed brief: the body plus the node's signature over `body.signing_bytes()`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeshBrief {
    pub body: BriefBody,
    /// ed25519 signature (hex, 64 bytes) by the node key over the canonical body.
    pub sig: String,
}

/// Sign a brief body with this node's key.
pub fn sign_brief(body: BriefBody, node: &NodeKey) -> Result<MeshBrief> {
    let sig = node.sign(&body.signing_bytes()?);
    Ok(MeshBrief { body, sig })
}

/// Fully verify an inbound brief: membership in the group, then the node signature. On
/// success the brief may be trusted and handed to the in-tick merge.
pub fn verify_brief(
    brief: &MeshBrief,
    group_key: &VerifyingKey,
    group_id: &str,
    now: i64,
    revoked: &[String],
) -> Result<()> {
    let b = &brief.body;
    // 1. The membership cert must place this node in the group (unexpired, unrevoked,
    //    id↔pubkey self-consistent).
    verify_membership(&b.membership, group_key, group_id, now, revoked)?;
    // 2. The cert must certify the *same* key that signs the brief — no swapping a trusted
    //    cert onto a different node identity.
    if b.membership.node_pubkey != b.node.pubkey || b.membership.node_id != b.node.node_id {
        return Err(crate::Error::Untrusted(
            "brief: membership cert does not match the signing node".into(),
        ));
    }
    // 3. The brief body must be signed by that node key.
    b.node.verify(&b.signing_bytes()?, &brief.sig)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::{create_group, DEFAULT_CERT_TTL_SECS};
    use std::fs;

    const NOW: i64 = 1_770_000_000;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("familiar_mesh_brief_{tag}"));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn sample_body(node: &NodeKey, membership: Membership) -> BriefBody {
        BriefBody {
            version: BRIEF_VERSION,
            node: node.identity(),
            membership,
            ts: NOW,
            nonce: "deadbeef".into(),
            presence: Presence {
                observer_count: 1,
                last_active: NOW - 5,
            },
            capability: Capability {
                os: "macos".into(),
                arch: "aarch64".into(),
                env_summary: "wildhorse".into(),
                familiar_version: "0.1.0".into(),
                os_version: String::new(),
                interactive: false,
                human: String::new(),
                tools: vec![ToolManifest {
                    tool_id: "t1".into(),
                    name: "ping".into(),
                    purpose: "reach a host".into(),
                    keywords: vec!["net".into()],
                    script_sha256: "abc123".into(),
                    uses: 3,
                    last_exit_ok: true,
                }],
                capabilities: Vec::new(),
                build_version: 0,
            },
            knowledge: Knowledge {
                patterns: vec![PatternOffer {
                    key: "morning-check".into(),
                    summary: "battery queried at dawn".into(),
                    support: 7,
                }],
                obs_summary: "42 observations".into(),
                observations: Vec::new(),
                theory_requests: Vec::new(),
                goals: Vec::new(),
            },
            identities: None,
            authority_requests: Vec::new(),
            authority_grants: Vec::new(),
        }
    }

    #[test]
    fn sign_verify_round_trip() {
        let dir = tmp("rt");
        let node = NodeKey::load_or_mint(&dir, "n").unwrap();
        let cred = create_group(&dir, &node, "g", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        let brief = sign_brief(sample_body(&node, cred.membership.clone()), &node).unwrap();

        let gk = cred.verifying_key().unwrap();
        assert!(verify_brief(&brief, &gk, &cred.group_id, NOW + 1, &[]).is_ok());

        // Full JSON round-trip preserves verifiability (deterministic body bytes).
        let wire = serde_json::to_string(&brief).unwrap();
        let back: MeshBrief = serde_json::from_str(&wire).unwrap();
        assert!(verify_brief(&back, &gk, &cred.group_id, NOW + 1, &[]).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tampered_body_fails_verification() {
        let dir = tmp("tamper");
        let node = NodeKey::load_or_mint(&dir, "n").unwrap();
        let cred = create_group(&dir, &node, "g", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        let mut brief = sign_brief(sample_body(&node, cred.membership.clone()), &node).unwrap();
        // Mutate the payload after signing — the node signature must now fail.
        brief.body.knowledge.obs_summary = "999 observations".into();
        let gk = cred.verifying_key().unwrap();
        assert!(verify_brief(&brief, &gk, &cred.group_id, NOW + 1, &[]).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cert_from_another_group_is_untrusted() {
        // A brief signed by a real node, but whose membership belongs to a different group,
        // must not verify against our group key.
        let dir = tmp("other");
        let node = NodeKey::load_or_mint(&dir, "n").unwrap();
        let ours = create_group(&dir, &node, "ours", NOW, DEFAULT_CERT_TTL_SECS).unwrap();

        let dir2 = tmp("other2");
        let node2 = NodeKey::load_or_mint(&dir2, "n2").unwrap();
        let theirs = create_group(&dir2, &node2, "theirs", NOW, DEFAULT_CERT_TTL_SECS).unwrap();

        // node2 signs a brief with *their* membership; we verify against *our* group key.
        let brief = sign_brief(sample_body(&node2, theirs.membership.clone()), &node2).unwrap();
        let our_gk = ours.verifying_key().unwrap();
        assert!(verify_brief(&brief, &our_gk, &ours.group_id, NOW + 1, &[]).is_err());
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&dir2);
    }

    #[test]
    fn stolen_cert_on_a_different_key_is_untrusted() {
        // Attacker takes a valid member's cert but signs the brief with their own key. The
        // cert certifies the victim's pubkey, not the attacker's → mismatch, rejected.
        let dir = tmp("victim");
        let victim = NodeKey::load_or_mint(&dir, "v").unwrap();
        let cred = create_group(&dir, &victim, "g", NOW, DEFAULT_CERT_TTL_SECS).unwrap();

        let dir2 = tmp("attacker");
        let attacker = NodeKey::load_or_mint(&dir2, "a").unwrap();
        // attacker builds a body with the victim's stolen membership but their own node id.
        let body = sample_body(&attacker, cred.membership.clone());
        let brief = sign_brief(body, &attacker).unwrap();
        let gk = cred.verifying_key().unwrap();
        assert!(verify_brief(&brief, &gk, &cred.group_id, NOW + 1, &[]).is_err());
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&dir2);
    }
}
