//! Group enrollment & the trust root.
//!
//! A **group keypair** is the trust anchor. The human who starts a group generates it
//! ("Create group") and gets a **join key** to copy to other nodes — the join key *is* the
//! group secret (hex), so holding it is what it means to be in the group: it is the power
//! to mint membership. Each node, given the group secret, mints a **membership
//! certificate** binding its own node key to the group:
//!
//! ```text
//! cert = sign_group( node_id ‖ node_pubkey ‖ issued ‖ expiry ‖ group_id )
//! ```
//!
//! A peer's brief is trusted iff (1) its membership cert verifies against the group
//! **public** key, its node id matches the fingerprint of the certified pubkey, it is
//! unexpired and not revoked; and (2) the brief's own signature verifies against that
//! now-trusted node pubkey (checked in [`crate::brief`]). Trust is cryptographic and
//! group-scoped — not IP- or discovery-based — so a discovered peer without a valid
//! in-group cert is ignored (Sybil-resistant).
//!
//! The human authorizes the *group* (enrolls this credential + opens `allow_mesh`); within
//! it, any peer with a valid cert is auto-trusted. The familiar never self-widens: it can
//! only mint a cert for a group whose secret a human already handed it.

use crate::node::{fingerprint, write_private, NodeKey};
use crate::{exactly_32, hex_decode, hex_encode, Error, Result};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// The group credential file (0600 — it holds the group secret).
pub const GROUP_FILE: &str = "mesh/group.json";
/// Revocation list: node ids no longer trusted, even with a valid-looking cert.
pub const REVOKED_FILE: &str = "mesh/revoked.json";

/// Default membership lifetime: 90 days. Expiry forces periodic re-minting (rotation).
pub const DEFAULT_CERT_TTL_SECS: i64 = 90 * 24 * 60 * 60;

/// A membership certificate — the group key's signature binding a node to the group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Membership {
    pub node_id: String,
    /// The node public key this cert certifies (hex, 32 bytes).
    pub node_pubkey: String,
    pub issued: i64,
    pub expiry: i64,
    pub group_id: String,
    /// ed25519 signature by the group key over the canonical cert body (hex, 64 bytes).
    pub cert: String,
}

/// The deterministic body that gets signed — kept as its own struct so `verify` can
/// reconstruct exactly the bytes `mint` signed (serde derives fixed field order; no maps).
#[derive(Serialize)]
struct CertBody<'a> {
    node_id: &'a str,
    node_pubkey: &'a str,
    issued: i64,
    expiry: i64,
    group_id: &'a str,
}

impl Membership {
    fn body_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(&CertBody {
            node_id: &self.node_id,
            node_pubkey: &self.node_pubkey,
            issued: self.issued,
            expiry: self.expiry,
            group_id: &self.group_id,
        })?)
    }
}

/// A node's stored group credential. Every member holds the group secret (that shared
/// secret *is* membership), plus its own certificate. File is 0600.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupCredential {
    pub group_id: String,
    /// Group public key (hex, 32 bytes) — the trust root others' certs verify against.
    pub group_pubkey: String,
    /// Group secret (hex, 32 bytes). Present because holding it *is* membership; it lets
    /// this node mint certs (invite peers). Kept 0600.
    pub group_secret: String,
    /// A cosmetic label for the group.
    pub label: String,
    /// This node's own membership certificate.
    pub membership: Membership,
}

impl GroupCredential {
    /// The group verifying (public) key.
    pub fn verifying_key(&self) -> Result<VerifyingKey> {
        let bytes = exactly_32(&hex_decode(&self.group_pubkey)?, "group pubkey")?;
        VerifyingKey::from_bytes(&bytes)
            .map_err(|e| Error::Malformed(format!("group pubkey: {e}")))
    }

    /// The join key to hand another node so it can enroll — this is the group secret.
    pub fn join_key(&self) -> String {
        self.group_secret.clone()
    }

    fn group_signing_key(&self) -> Result<SigningKey> {
        let bytes = exactly_32(&hex_decode(&self.group_secret)?, "group secret")?;
        Ok(SigningKey::from_bytes(&bytes))
    }

    /// Mint a fresh membership certificate for another node in this group (an invite).
    pub fn mint_membership(
        &self,
        node_id: &str,
        node_pubkey: &str,
        now: i64,
        ttl_secs: i64,
    ) -> Result<Membership> {
        mint_with(&self.group_signing_key()?, &self.group_id, node_id, node_pubkey, now, ttl_secs)
    }
}

/// Create a brand-new group: generate the group keypair, mint this node's membership, and
/// persist the credential (0600). Returns the credential (its `join_key()` is what you copy
/// to peers). `now` is caller-supplied (unix secs) so this stays deterministic/testable.
pub fn create_group(
    dir: &Path,
    node: &NodeKey,
    label: &str,
    now: i64,
    ttl_secs: i64,
) -> Result<GroupCredential> {
    let secret: [u8; 32] = crate::os_random()?;
    enroll(dir, node, &secret, label, now, ttl_secs)
}

/// Join an existing group from a join key (the group secret, hex). Mints this node's own
/// membership cert against the group key and persists the credential (0600).
pub fn join_group(
    dir: &Path,
    node: &NodeKey,
    join_key: &str,
    label: &str,
    now: i64,
    ttl_secs: i64,
) -> Result<GroupCredential> {
    let secret = exactly_32(&hex_decode(join_key)?, "join key")?;
    enroll(dir, node, &secret, label, now, ttl_secs)
}

/// Shared enrollment: given the group secret, derive the group id/pubkey, mint this node's
/// membership, and write `mesh/group.json` (0600).
fn enroll(
    dir: &Path,
    node: &NodeKey,
    group_secret: &[u8; 32],
    label: &str,
    now: i64,
    ttl_secs: i64,
) -> Result<GroupCredential> {
    let group_signing = SigningKey::from_bytes(group_secret);
    let group_pubkey = group_signing.verifying_key().to_bytes();
    let group_id = fingerprint(&group_pubkey);
    let id = node.identity();
    let membership = mint_with(
        &group_signing,
        &group_id,
        &id.node_id,
        &id.pubkey,
        now,
        ttl_secs,
    )?;
    let cred = GroupCredential {
        group_id,
        group_pubkey: hex_encode(&group_pubkey),
        group_secret: hex_encode(group_secret),
        label: label.to_string(),
        membership,
    };
    let json = serde_json::to_string_pretty(&cred)?;
    write_private(&dir.join(GROUP_FILE), &json)?;
    Ok(cred)
}

fn mint_with(
    group_signing: &SigningKey,
    group_id: &str,
    node_id: &str,
    node_pubkey: &str,
    now: i64,
    ttl_secs: i64,
) -> Result<Membership> {
    let issued = now;
    let expiry = now + ttl_secs;
    let body = serde_json::to_vec(&CertBody {
        node_id,
        node_pubkey,
        issued,
        expiry,
        group_id,
    })?;
    let sig = group_signing.sign(&body);
    Ok(Membership {
        node_id: node_id.to_string(),
        node_pubkey: node_pubkey.to_string(),
        issued,
        expiry,
        group_id: group_id.to_string(),
        cert: hex_encode(&sig.to_bytes()),
    })
}

/// Verify a peer's membership certificate against a group public key. Checks: the group's
/// signature over the canonical body; that `node_id` is the fingerprint of the certified
/// `node_pubkey` (self-consistency); expiry; and the revocation list. On success the caller
/// may trust `node_pubkey` as a group member.
pub fn verify_membership(
    m: &Membership,
    group_key: &VerifyingKey,
    group_id: &str,
    now: i64,
    revoked: &[String],
) -> Result<()> {
    if m.group_id != group_id {
        return Err(Error::Untrusted("membership: wrong group".into()));
    }
    if now >= m.expiry {
        return Err(Error::Untrusted("membership: expired".into()));
    }
    if revoked.iter().any(|r| r == &m.node_id) {
        return Err(Error::Untrusted("membership: node revoked".into()));
    }
    // node_id must be the fingerprint of the certified pubkey — a cert can't rename a node.
    let pk = exactly_32(&hex_decode(&m.node_pubkey)?, "cert node pubkey")?;
    if fingerprint(&pk) != m.node_id {
        return Err(Error::Untrusted("membership: node_id ≠ pubkey fingerprint".into()));
    }
    let sig_bytes = crate::node::exactly_64(&hex_decode(&m.cert)?, "cert")?;
    let sig = ed25519_dalek::Signature::from_bytes(&sig_bytes);
    group_key
        .verify(&m.body_bytes()?, &sig)
        .map_err(|_| Error::Untrusted("membership: group signature did not verify".into()))
}

/// Load the group credential, if this node has enrolled.
pub fn load(dir: &Path) -> Result<Option<GroupCredential>> {
    let path = dir.join(GROUP_FILE);
    match fs::read_to_string(&path) {
        Ok(s) => Ok(Some(serde_json::from_str(&s)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Load the revocation list (node ids). Missing file → empty.
pub fn load_revoked(dir: &Path) -> Result<Vec<String>> {
    match fs::read_to_string(dir.join(REVOKED_FILE)) {
        Ok(s) => Ok(serde_json::from_str(&s)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

/// Write a public (non-secret) JSON record with pretty formatting and default perms.
pub(crate) fn write_json_public<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("familiar_mesh_group_{tag}"));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    const NOW: i64 = 1_770_000_000;

    #[test]
    fn create_join_and_cross_verify() {
        // Node A creates a group; Node B joins with A's join key. Each node's own cert must
        // verify against the shared group key, and A must trust B's cert (and vice-versa).
        let dir_a = tmp("a");
        let dir_b = tmp("b");
        let a = NodeKey::load_or_mint(&dir_a, "a").unwrap();
        let b = NodeKey::load_or_mint(&dir_b, "b").unwrap();

        let cred_a = create_group(&dir_a, &a, "river", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        let cred_b =
            join_group(&dir_b, &b, &cred_a.join_key(), "river", NOW, DEFAULT_CERT_TTL_SECS).unwrap();

        // Same group id + pubkey derived independently.
        assert_eq!(cred_a.group_id, cred_b.group_id);
        assert_eq!(cred_a.group_pubkey, cred_b.group_pubkey);

        let gk = cred_a.verifying_key().unwrap();
        // A trusts B's membership and B trusts A's — cross verification.
        verify_membership(&cred_b.membership, &gk, &cred_a.group_id, NOW + 10, &[]).unwrap();
        verify_membership(&cred_a.membership, &gk, &cred_a.group_id, NOW + 10, &[]).unwrap();

        let _ = fs::remove_dir_all(&dir_a);
        let _ = fs::remove_dir_all(&dir_b);
    }

    #[test]
    fn rejects_expired_revoked_wrong_group_and_forged() {
        let dir = tmp("reject");
        let node = NodeKey::load_or_mint(&dir, "n").unwrap();
        let cred = create_group(&dir, &node, "g", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        let gk = cred.verifying_key().unwrap();
        let m = &cred.membership;

        // Valid now.
        assert!(verify_membership(m, &gk, &cred.group_id, NOW + 1, &[]).is_ok());
        // Expired.
        assert!(verify_membership(m, &gk, &cred.group_id, m.expiry, &[]).is_err());
        // Revoked.
        assert!(
            verify_membership(m, &gk, &cred.group_id, NOW + 1, &[m.node_id.clone()]).is_err()
        );
        // Wrong group id.
        assert!(verify_membership(m, &gk, "deadbeef", NOW + 1, &[]).is_err());
        // Forged cert: flip a signature byte.
        let mut bad = m.clone();
        bad.cert.replace_range(0..2, if &bad.cert[0..2] == "00" { "01" } else { "00" });
        assert!(verify_membership(&bad, &gk, &cred.group_id, NOW + 1, &[]).is_err());
        // Different group key can't validate this cert.
        let dir2 = tmp("reject2");
        let n2 = NodeKey::load_or_mint(&dir2, "n2").unwrap();
        let other = create_group(&dir2, &n2, "other", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        assert!(
            verify_membership(m, &other.verifying_key().unwrap(), &cred.group_id, NOW + 1, &[])
                .is_err()
        );

        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&dir2);
    }

    #[test]
    fn node_id_cannot_be_forged_against_a_pubkey() {
        // A cert whose node_id doesn't match its pubkey fingerprint is rejected even if the
        // group signature were valid — the binding is pubkey→id, self-checking.
        let dir = tmp("bind");
        let node = NodeKey::load_or_mint(&dir, "n").unwrap();
        let cred = create_group(&dir, &node, "g", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        let mut m = cred.membership.clone();
        m.node_id = "0000000000000000".into(); // lie about the id
        assert!(
            verify_membership(&m, &cred.verifying_key().unwrap(), &cred.group_id, NOW + 1, &[])
                .is_err()
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
