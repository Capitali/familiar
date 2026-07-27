//! The member-status directory (ADR-0017) — the lighthouse as the mesh's always-on presence hub.
//!
//! Every member heartbeats its own status (signed with its membership) to the lighthouse, which
//! keeps a short-lived per-member directory. Any member reads it to build a complete, fresh roster —
//! even for members it does not directly serve. This is what keeps a member visible mesh-wide no
//! matter which path it is on: **status flows through the always-on lighthouse; data takes the best
//! confirmed path** (Tailscale when it works — Phases B/C). A member that drops off simply stops
//! heartbeating and expires ([`STATUS_TTL_SECS`]).
//!
//! Soft state and self-scoped: a heartbeat is kept only if the membership verifies against the group
//! key it names AND the status is for the sender's *own* node — a member can refresh only itself.
//! The group id is carried hashed (`group_ref`, from [`crate::rendezvous::group_ref`]) so a reader
//! filters to its own mesh without the id being published.

use crate::group::{self, Membership};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Where a node keeps the status directory it hosts (JSONL, under the data dir).
pub const STATUS_FILE: &str = "mesh/status.jsonl";
/// A status older than this is stale — the member is treated as gone. Shorter than the rendezvous
/// TTL: presence should go dark quickly when a member stops heartbeating.
pub const STATUS_TTL_SECS: i64 = 5 * 60;

/// One member's live status, as it heartbeats it. The connectivity fields are schema-forward for
/// the Tailscale phases; Phase A fills `connectivity="lighthouse"` (or `"local"`) and leaves the
/// tailnet fields empty.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemberStatus {
    pub node_id: String,
    /// The hashed group handle (see module docs) — lets a reader keep only its own mesh's rows.
    #[serde(default)]
    pub group_ref: String,
    #[serde(default)]
    pub actor: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub present_human: String,
    #[serde(default)]
    pub present_via: String,
    #[serde(default)]
    pub present_since: i64,
    /// How this member is currently reaching the mesh: "local" | "lighthouse" | "tailscale".
    #[serde(default)]
    pub connectivity: String,
    /// The member's tailnet (100.64/10) address when Tailscale is up (Phases B/C). Empty otherwise.
    #[serde(default)]
    pub tailnet_addr: String,
    #[serde(default)]
    pub tailnet_up: bool,
    /// Last heartbeat (unix secs) — the directory expires on it.
    pub updated_at: i64,
}

/// The signed heartbeat a member POSTs. Membership-bearing, so a stranger can read the directory but
/// only a member can place a status — and only for its own node (checked in [`record`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusReport {
    pub membership: Membership,
    /// The group's public key (hex) — the host checks the membership cert without holding the key.
    pub group_pubkey: String,
    pub status: MemberStatus,
    /// Replay guard at the transport.
    pub nonce: String,
    pub ts: i64,
}

impl StatusReport {
    /// The sender signed these exact bytes with the key its membership certifies (mirrors the
    /// rendezvous/enroll signature proof).
    pub fn verify_sig(&self, raw: &[u8], sig_hex: &str) -> Result<()> {
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};
        let pk = crate::exactly_32(
            &crate::hex_decode(&self.membership.node_pubkey)?,
            "node pubkey",
        )?;
        let key = VerifyingKey::from_bytes(&pk)
            .map_err(|_| Error::Untrusted("bad node pubkey".into()))?;
        let sig_bytes = crate::node::exactly_64(&crate::hex_decode(sig_hex)?, "sig")?;
        key.verify(raw, &Signature::from_bytes(&sig_bytes))
            .map_err(|_| Error::Untrusted("status: node signature did not verify".into()))
    }
}

fn load_raw(dir: &Path) -> Vec<MemberStatus> {
    std::fs::read_to_string(dir.join(STATUS_FILE))
        .map(|s| {
            s.lines()
                .filter_map(|l| serde_json::from_str::<MemberStatus>(l).ok())
                .collect()
        })
        .unwrap_or_default()
}

fn save(dir: &Path, entries: &[MemberStatus]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir.join("mesh"))?;
    let mut out = String::new();
    for e in entries {
        if let Ok(line) = serde_json::to_string(e) {
            out.push_str(&line);
            out.push('\n');
        }
    }
    std::fs::write(dir.join(STATUS_FILE), out)
}

/// Live statuses only — the stored directory with anything past its TTL dropped.
pub fn directory(dir: &Path, now: i64) -> Vec<MemberStatus> {
    load_raw(dir)
        .into_iter()
        .filter(|e| now - e.updated_at <= STATUS_TTL_SECS)
        .collect()
}

/// Accept a heartbeat: verify the member belongs to the group it names (the same cert check the
/// rendezvous runs — the host need not be *in* that group), that the status is for the sender's own
/// node, then upsert by node_id, stamp `group_ref` + `updated_at`, and prune stale rows. Returns the
/// stored status. This admits no one and grants nothing; it only records "member N is alive, here".
pub fn record(dir: &Path, report: &StatusReport, now: i64) -> Result<MemberStatus> {
    group::verify_membership_consistent(&report.membership, &report.group_pubkey, now)
        .map_err(|_| Error::Untrusted("status: membership does not verify".into()))?;
    if report.status.node_id != report.membership.node_id {
        return Err(Error::Untrusted(
            "status: a member may only place its own status".into(),
        ));
    }
    let mut entry = report.status.clone();
    entry.group_ref = crate::rendezvous::group_ref(&report.membership.group_id);
    entry.updated_at = now;
    let mut entries: Vec<MemberStatus> = directory(dir, now)
        .into_iter()
        .filter(|e| e.node_id != entry.node_id)
        .collect();
    entries.push(entry.clone());
    save(dir, &entries).map_err(|e| Error::Malformed(format!("status: {e}")))?;
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::{create_group, DEFAULT_CERT_TTL_SECS};
    use crate::node::NodeKey;

    const NOW: i64 = 1_785_000_000;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("familiar_status_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn a_report(dir: &Path, node_id: &str, actor: &str) -> (StatusReport, GroupCredentialLite) {
        let node = NodeKey::load_or_mint(dir, "n").unwrap();
        let cred = create_group(dir, &node, "TheRiver", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        let status = MemberStatus {
            node_id: node_id.to_string(),
            group_ref: String::new(),
            actor: actor.to_string(),
            label: "iPad".into(),
            present_human: "betty".into(),
            present_via: "face".into(),
            present_since: NOW - 60,
            connectivity: "lighthouse".into(),
            tailnet_addr: String::new(),
            tailnet_up: false,
            updated_at: 0,
        };
        (
            StatusReport {
                membership: cred.membership.clone(),
                group_pubkey: cred.group_pubkey.clone(),
                status,
                nonce: "n1".into(),
                ts: NOW,
            },
            GroupCredentialLite {
                group_id: cred.membership.group_id.clone(),
            },
        )
    }

    struct GroupCredentialLite {
        group_id: String,
    }

    #[test]
    fn records_and_expires_a_members_status() {
        let home = tmp("home");
        // Report must be for the member's OWN node — use the membership's node_id.
        let node = NodeKey::load_or_mint(&home, "n").unwrap();
        let (mut report, gl) = a_report(&home, "wrong", "ipad:betty");
        // Wrong node_id is refused (a member may only place its own status).
        let lh = tmp("lh");
        assert!(record(&lh, &report, NOW).is_err());
        // Corrected to the real node id: accepted, listed under the group_ref, present.
        report.status.node_id = node.node_id();
        let stored = record(&lh, &report, NOW).unwrap();
        assert_eq!(stored.group_ref, crate::rendezvous::group_ref(&gl.group_id));
        assert_eq!(stored.present_human, "betty");
        assert_eq!(directory(&lh, NOW).len(), 1);
        // A refresh upserts, never duplicates.
        record(&lh, &report, NOW + 30).unwrap();
        assert_eq!(directory(&lh, NOW + 30).len(), 1);
        // Past the TTL measured from the last heartbeat (NOW + 30), the member is gone.
        assert_eq!(directory(&lh, NOW + 30 + STATUS_TTL_SECS + 1).len(), 0);
    }
}
