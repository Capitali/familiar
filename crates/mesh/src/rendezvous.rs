//! The rendezvous directory (ADR-0012) — how a new device finds a mesh without a QR.
//!
//! A public node (the lighthouse) keeps a short-lived directory of the meshes that register
//! with it. A familiar that knows a rendezvous address registers its group and its reachable
//! hosts, signed with its membership; a fresh device reads the directory and learns *where to
//! ask to join* — labels and addresses only, never a secret. Admission stays a human act: the
//! directory helps a device find the door and knock, nothing more (see the ADR).
//!
//! Soft state: entries expire ([`ENTRY_TTL_SECS`]); a familiar that stops registering falls out.
//! The group id is carried **hashed** (`group_ref`) — enough for a device to tell two meshes
//! apart and for the familiar to recognize its own entry, without publishing the group id itself.

use crate::group::{self, Membership};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Where the lighthouse keeps the directory (JSONL, under the data dir).
pub const DIRECTORY_FILE: &str = "mesh/rendezvous.jsonl";
/// A registration older than this is stale — dropped on the next read/write.
pub const ENTRY_TTL_SECS: i64 = 15 * 60;

/// One mesh, as a registering familiar presents it. Signed at the transport (membership-bearing),
/// so a stranger can read the directory but only a member can place or refresh its mesh's entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registration {
    /// The registering node's membership — verified self-consistent before the entry is kept.
    pub membership: Membership,
    /// The group's public key (hex) — so the lighthouse can check the membership cert without
    /// holding the group key itself. Public by nature; it admits no one (ADR-0012).
    pub group_pubkey: String,
    /// Human-facing mesh name ("TheRiver").
    pub group_label: String,
    /// Addresses a joiner can reach this mesh at (the familiar's `reachable_hosts` + advertised).
    pub hosts: Vec<String>,
    /// The gossip/enroll port (usually 47100).
    pub port: u16,
    /// Signing nonce (replay guard at the transport).
    pub nonce: String,
    pub ts: i64,
}

/// A directory entry as stored and as served to a joining device — **no secret, no raw group id**.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    /// A stable, non-reversible handle for the group (`group_ref`) — tells meshes apart and lets
    /// a familiar find its own row, without publishing the group id.
    pub group_ref: String,
    pub group_label: String,
    pub hosts: Vec<String>,
    pub port: u16,
    /// When this entry was last refreshed (unix secs) — the directory expires on it.
    pub updated_at: i64,
}

/// The non-reversible handle a group is listed under: `sha256("rendezvous:" || group_id)`, hex.
/// Deterministic (a familiar recomputes it to find its own row) but not the group id itself.
pub fn group_ref(group_id: &str) -> String {
    let digest = Sha256::digest(format!("rendezvous:{group_id}").as_bytes());
    crate::hex_encode(&digest[..12])
}

impl Registration {
    /// Verify the registering node signed these exact bytes with the key its membership certifies
    /// — the transport-level proof that the sender holds the node key (mirrors the enroll path).
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
            .map_err(|_| Error::Untrusted("rendezvous: node signature did not verify".into()))
    }
}

fn load_raw(dir: &Path) -> Vec<Entry> {
    std::fs::read_to_string(dir.join(DIRECTORY_FILE))
        .map(|s| {
            s.lines()
                .filter_map(|l| serde_json::from_str::<Entry>(l).ok())
                .collect()
        })
        .unwrap_or_default()
}

fn save(dir: &Path, entries: &[Entry]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir.join("mesh"))?;
    let mut out = String::new();
    for e in entries {
        out.push_str(&serde_json::to_string(e).map_err(std::io::Error::other)?);
        out.push('\n');
    }
    std::fs::write(dir.join(DIRECTORY_FILE), out)
}

/// Live entries only — the stored directory with anything past its TTL dropped.
pub fn directory(dir: &Path, now: i64) -> Vec<Entry> {
    load_raw(dir)
        .into_iter()
        .filter(|e| now - e.updated_at <= ENTRY_TTL_SECS)
        .collect()
}

/// Accept a registration on the rendezvous side: verify the member truly belongs to the group it
/// claims (the same membership check an observe/worldview push runs), then upsert its entry by
/// `group_ref` and prune stale rows. Returns the stored entry.
///
/// This node need not be *in* that group — a lighthouse hosts the meeting for meshes it is not a
/// member of. What it verifies is that the registration is internally consistent: the membership
/// cert is signed by the group key it names, and the registering node signed the request (checked
/// at the transport before this is called). It does **not** admit anyone; it lists a door.
pub fn register(dir: &Path, reg: &Registration, now: i64) -> Result<Entry> {
    // The membership must be self-consistent: its cert verifies under the group public key the
    // registration supplies (a lighthouse holds no group key for foreign meshes). This proves the
    // door is well-formed — a forged cert or a node_id that isn't its key's fingerprint can't
    // survive it — not that the mesh is one we vouch for. Listing a door is not admitting anyone.
    group::verify_membership_consistent(&reg.membership, &reg.group_pubkey, now)
        .map_err(|_| Error::Untrusted("rendezvous: membership does not verify".into()))?;
    if reg.hosts.is_empty() {
        return Err(Error::Malformed("rendezvous: no hosts to advertise".into()));
    }
    let group_ref = group_ref(&reg.membership.group_id);
    let entry = Entry {
        group_ref: group_ref.clone(),
        group_label: reg.group_label.clone(),
        hosts: reg.hosts.clone(),
        port: reg.port,
        updated_at: now,
    };
    let mut entries: Vec<Entry> = directory(dir, now)
        .into_iter()
        .filter(|e| e.group_ref != group_ref)
        .collect();
    entries.push(entry.clone());
    save(dir, &entries).map_err(|e| Error::Malformed(format!("rendezvous: {e}")))?;
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::{create_group, DEFAULT_CERT_TTL_SECS};
    use crate::node::NodeKey;

    const NOW: i64 = 1_000_000;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("rdv_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn a_registration(dir: &Path, label: &str) -> Registration {
        let node = NodeKey::load_or_mint(dir, "n").unwrap();
        let cred = create_group(dir, &node, label, NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        Registration {
            membership: cred.membership,
            group_pubkey: cred.group_pubkey,
            group_label: label.to_string(),
            hosts: vec!["100.64.0.5".into(), "lighthouse.river.io".into()],
            port: 47_100,
            nonce: "n1".into(),
            ts: NOW,
        }
    }

    #[test]
    fn group_ref_is_stable_and_hides_the_id() {
        assert_eq!(group_ref("abc123"), group_ref("abc123"));
        assert_ne!(group_ref("abc123"), group_ref("def456"));
        assert!(!group_ref("abc123").contains("abc123"));
    }

    #[test]
    fn register_lists_a_door_then_refreshes_in_place() {
        let home = tmp("reg_home");
        let reg = a_registration(&home, "TheRiver");
        // The lighthouse is a *different* node, hosting a mesh it isn't in.
        let lh = tmp("reg_lh");
        let e = register(&lh, &reg, NOW).unwrap();
        assert_eq!(e.group_label, "TheRiver");
        assert_eq!(directory(&lh, NOW).len(), 1);
        // A second registration for the same group refreshes, never duplicates.
        let mut reg2 = reg.clone();
        reg2.hosts = vec!["203.0.113.9".into()];
        register(&lh, &reg2, NOW + 60).unwrap();
        let d = directory(&lh, NOW + 60);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].hosts, vec!["203.0.113.9".to_string()]);
        assert_eq!(d[0].updated_at, NOW + 60);
    }

    #[test]
    fn the_directory_expires_stale_doors() {
        let home = tmp("exp_home");
        let reg = a_registration(&home, "Quiet");
        let lh = tmp("exp_lh");
        register(&lh, &reg, NOW).unwrap();
        assert_eq!(directory(&lh, NOW + ENTRY_TTL_SECS).len(), 1);
        assert_eq!(directory(&lh, NOW + ENTRY_TTL_SECS + 1).len(), 0);
    }

    #[test]
    fn a_hostless_or_forged_registration_is_refused() {
        let home = tmp("bad_home");
        let lh = tmp("bad_lh");
        let mut reg = a_registration(&home, "NoHosts");
        reg.hosts.clear();
        assert!(register(&lh, &reg, NOW).is_err());

        // A membership whose cert doesn't verify (tampered group_id) is refused.
        let mut forged = a_registration(&home, "Forged");
        forged.membership.group_id = "deadbeef".into();
        assert!(register(&lh, &forged, NOW).is_err());
    }
}
