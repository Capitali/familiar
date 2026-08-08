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

/// A membership certificate — a signature binding a node to the group. Signed either by the
/// group key directly (the founding doors), or — since ADR-0026 §6 — by a **warranted member
/// node's** key, with the warrant attached so any peer can walk cert → warrant → group key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Membership {
    pub node_id: String,
    /// The node public key this cert certifies (hex, 32 bytes).
    pub node_pubkey: String,
    pub issued: i64,
    pub expiry: i64,
    pub group_id: String,
    /// ed25519 signature over the canonical cert body (hex, 64 bytes) — by the group key, or
    /// by the warranted minter's node key when `warrant` is present.
    pub cert: String,
    /// The minting warrant, when this cert was minted by a member node rather than the group
    /// key. Absent (and skipped on the wire) for group-key certs, so old certs and old
    /// clients are untouched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warrant: Option<MintWarrant>,
}

/// The group key's signature authorising ONE member node to mint memberships (ADR-0026 §6).
/// What distributes is rule *evaluation*, not policy: a warranted door runs the same rules
/// engine as every other, and every cert it mints names it. Issued deliberately, revocable by
/// expiry (short-ish TTLs beat revocation lists for a fleet this size).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MintWarrant {
    /// The member node this warrant empowers.
    pub node_id: String,
    /// That node's public key (hex, 32 bytes) — certs it mints verify under this.
    pub node_pubkey: String,
    pub group_id: String,
    pub issued: i64,
    pub expiry: i64,
    /// ed25519 by the GROUP key over the canonical warrant body (hex, 64 bytes).
    pub sig: String,
}

/// Where a node stores the warrant issued to it.
pub const WARRANT_FILE: &str = "mesh/warrant.json";

/// Default warrant lifetime: 30 days — long enough that renewal is rare, short enough that a
/// lost device's warrant dies on its own.
pub const DEFAULT_WARRANT_TTL_SECS: i64 = 30 * 24 * 60 * 60;

#[derive(Serialize)]
struct WarrantBody<'a> {
    node_id: &'a str,
    node_pubkey: &'a str,
    group_id: &'a str,
    issued: i64,
    expiry: i64,
}

impl MintWarrant {
    fn body_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(&WarrantBody {
            node_id: &self.node_id,
            node_pubkey: &self.node_pubkey,
            group_id: &self.group_id,
            issued: self.issued,
            expiry: self.expiry,
        })?)
    }

    /// Verify this warrant under the group public key: signature, expiry, group, and the
    /// node_id ↔ pubkey binding (a warrant cannot rename a node).
    pub fn verify(&self, group_key: &VerifyingKey, group_id: &str, now: i64) -> Result<()> {
        if self.group_id != group_id {
            return Err(Error::Untrusted("warrant: wrong group".into()));
        }
        if now >= self.expiry {
            return Err(Error::Untrusted("warrant: expired".into()));
        }
        let pk = exactly_32(&hex_decode(&self.node_pubkey)?, "warrant node pubkey")?;
        if fingerprint(&pk) != self.node_id {
            return Err(Error::Untrusted(
                "warrant: node_id ≠ pubkey fingerprint".into(),
            ));
        }
        let sig_bytes = crate::node::exactly_64(&hex_decode(&self.sig)?, "warrant sig")?;
        let sig = ed25519_dalek::Signature::from_bytes(&sig_bytes);
        group_key
            .verify(&self.body_bytes()?, &sig)
            .map_err(|_| Error::Untrusted("warrant: group signature did not verify".into()))
    }
}

/// Issue a minting warrant to a member node. Only a secret-holding credential can (the group
/// key signs it) — this is the deliberate act that turns a peer into a door.
pub fn issue_warrant(
    cred: &GroupCredential,
    node_id: &str,
    node_pubkey: &str,
    now: i64,
    ttl_secs: i64,
) -> Result<MintWarrant> {
    let signing = cred.group_signing_key()?;
    let issued = now;
    let expiry = now + ttl_secs;
    let body = serde_json::to_vec(&WarrantBody {
        node_id,
        node_pubkey,
        group_id: &cred.group_id,
        issued,
        expiry,
    })?;
    let sig = signing.sign(&body);
    Ok(MintWarrant {
        node_id: node_id.to_string(),
        node_pubkey: node_pubkey.to_string(),
        group_id: cred.group_id.clone(),
        issued,
        expiry,
        sig: hex_encode(&sig.to_bytes()),
    })
}

/// Install a warrant on this node (it must verify for this node and this group first — a
/// warrant for someone else on disk would be a confusing lie).
pub fn install_warrant(dir: &Path, warrant: &MintWarrant, now: i64) -> Result<()> {
    let cred = load(dir)?.ok_or_else(|| Error::Untrusted("no group enrolled".into()))?;
    warrant.verify(&cred.verifying_key()?, &cred.group_id, now)?;
    if warrant.node_id != cred.membership.node_id {
        return Err(Error::Untrusted(format!(
            "warrant names {}, this node is {}",
            warrant.node_id, cred.membership.node_id
        )));
    }
    write_json_public(&dir.join(WARRANT_FILE), warrant)
}

/// This node's installed warrant, if it verifies right now. Expired or invalid → `None`, so a
/// dead warrant quietly stops the door rather than serving bad certs.
pub fn load_warrant(dir: &Path, now: i64) -> Option<MintWarrant> {
    let w: MintWarrant =
        serde_json::from_str(&fs::read_to_string(dir.join(WARRANT_FILE)).ok()?).ok()?;
    let cred = load(dir).ok()??;
    let gk = cred.verifying_key().ok()?;
    w.verify(&gk, &cred.group_id, now).ok()?;
    Some(w)
}

/// Mint a membership as a **warranted member node**: the cert is signed with this node's own
/// key and carries the warrant, so any peer verifies it without the group secret existing
/// anywhere near this door.
pub fn mint_membership_warranted(
    node: &NodeKey,
    warrant: &MintWarrant,
    subject_node_id: &str,
    subject_pubkey: &str,
    now: i64,
    ttl_secs: i64,
) -> Result<Membership> {
    if node.node_id() != warrant.node_id {
        return Err(Error::Untrusted(
            "warrant does not name this node — cannot mint with it".into(),
        ));
    }
    let issued = now;
    let expiry = now + ttl_secs;
    let body = serde_json::to_vec(&CertBody {
        node_id: subject_node_id,
        node_pubkey: subject_pubkey,
        issued,
        expiry,
        group_id: &warrant.group_id,
    })?;
    Ok(Membership {
        node_id: subject_node_id.to_string(),
        node_pubkey: subject_pubkey.to_string(),
        issued,
        expiry,
        group_id: warrant.group_id.clone(),
        cert: node.sign(&body),
        warrant: Some(warrant.clone()),
    })
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
        VerifyingKey::from_bytes(&bytes).map_err(|e| Error::Malformed(format!("group pubkey: {e}")))
    }

    /// The join key to hand another node so it can enroll — this is the group secret.
    pub fn join_key(&self) -> String {
        self.group_secret.clone()
    }

    /// Whether this node holds the group secret. A **covenant-joined** node does not (it was
    /// admitted via the handshake and holds only its own cert): it can prove membership and verify
    /// peers, but cannot mint members or invite. `false` ⇒ `mint_membership`/`join_key` are inert.
    pub fn can_mint(&self) -> bool {
        !self.group_secret.trim().is_empty()
    }

    /// Build the credential a node stores after joining by covenant (no secret). See [`can_mint`].
    pub fn covenant(
        group_id: String,
        group_pubkey: String,
        label: String,
        membership: Membership,
    ) -> Self {
        GroupCredential {
            group_id,
            group_pubkey,
            group_secret: String::new(),
            label,
            membership,
        }
    }

    fn group_signing_key(&self) -> Result<SigningKey> {
        if !self.can_mint() {
            return Err(Error::Untrusted(
                "covenant-joined node holds no group secret — cannot mint members".into(),
            ));
        }
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
        mint_with(
            &self.group_signing_key()?,
            &self.group_id,
            node_id,
            node_pubkey,
            now,
            ttl_secs,
        )
    }
}

/// Create a brand-new group: generate the group keypair, mint this node's membership, and
/// persist the credential (0600). Returns the credential (its `join_key()` is what you copy
/// to peers). `now` is caller-supplied (unix secs) so this stays deterministic/testable.
///
/// Founding is also the first admission (ADR-0026 E4, provenance `Founding`): the founder's
/// own record is written established + admitted, so a one-node mesh is a mesh of one member
/// rather than a mesh of one guest. Best-effort — a record failure must not fail the founding.
pub fn create_group(
    dir: &Path,
    node: &NodeKey,
    label: &str,
    now: i64,
    ttl_secs: i64,
) -> Result<GroupCredential> {
    let secret: [u8; 32] = crate::os_random()?;
    let cred = enroll(dir, node, &secret, label, now, ttl_secs)?;
    let id = node.identity();
    let _ = crate::record::admit(
        dir,
        &id.node_id,
        None,
        crate::record::Establishment {
            handle: String::new(), // the founding human names themself on their own console
            class: crate::record::EvidenceClass::LocalIntroduction,
            artifact: "founding".into(),
            at: now,
        },
        &id.node_id,
        now,
    );
    Ok(cred)
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

/// Persist a group credential to `mesh/group.json` (0600). Used by the covenant-join path to
/// store the grant-based (secret-less) credential a node receives when admitted by handshake.
pub fn save_credential(dir: &Path, cred: &GroupCredential) -> Result<()> {
    let json = serde_json::to_string_pretty(cred)?;
    write_private(&dir.join(GROUP_FILE), &json)
}

// ---- escrow: surviving the loss of the minting door (ADR-0018) ---------------------------

/// The group's recovery material. ADR-0018 concentrates minting on the lighthouse, which makes
/// that one rented box the sole holder of the group secret *in service*. Escrow is what keeps
/// that a decision about operations rather than a single point of extinction: with this, losing
/// the lighthouse is an outage; without it, no new device can ever join the group again.
///
/// This is **the** secret. It is not a backup of a node — it is the authority to mint members.
/// Anyone holding it can admit anyone. It belongs offline, in the human's keeping, and never on a
/// running host other than the minting door.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupEscrow {
    /// Format marker, so a restore twenty years from now knows what it is holding.
    pub kind: String,
    pub group_id: String,
    pub label: String,
    /// The group public key (hex) — carried so a restore can verify it reconstituted the RIGHT
    /// group before writing anything.
    pub group_pubkey: String,
    /// The group secret (hex, 32 bytes).
    pub group_secret: String,
    pub exported_at: i64,
}

pub const ESCROW_KIND: &str = "familiar-group-escrow-v1";

/// Export the recovery material from a mint-capable credential. Fails on a covenant credential —
/// there is nothing to escrow, and silently writing an empty escrow would be the worst possible
/// outcome (a file that looks like insurance and is not).
pub fn export_escrow(dir: &Path, now: i64) -> Result<GroupEscrow> {
    let cred = load(dir)?.ok_or_else(|| Error::Untrusted("no group enrolled".into()))?;
    if !cred.can_mint() {
        return Err(Error::Untrusted(
            "this node holds no group secret — a covenant credential has nothing to escrow".into(),
        ));
    }
    Ok(GroupEscrow {
        kind: ESCROW_KIND.to_string(),
        group_id: cred.group_id.clone(),
        label: cred.label.clone(),
        group_pubkey: cred.group_pubkey.clone(),
        group_secret: cred.group_secret.clone(),
        exported_at: now,
    })
}

/// Restore minting authority onto a node that holds a covenant credential for the same group.
///
/// Deliberately narrow: it will not create a group, will not adopt a foreign one, and will not
/// overwrite a credential for a different group. The node keeps its own identity and its own
/// membership cert; all this restores is the authority to mint.
pub fn restore_from_escrow(dir: &Path, escrow: &GroupEscrow) -> Result<GroupCredential> {
    if escrow.kind != ESCROW_KIND {
        return Err(Error::Malformed(format!(
            "escrow: unknown format {:?}",
            escrow.kind
        )));
    }
    let mut cred = load(dir)?.ok_or_else(|| {
        Error::Untrusted(
            "restore needs an enrolled node: join the group first, then restore".into(),
        )
    })?;
    if cred.group_id != escrow.group_id {
        return Err(Error::Untrusted(format!(
            "escrow is for group {}, this node belongs to {}",
            escrow.group_id, cred.group_id
        )));
    }
    // The secret must actually be this group's — a mismatched key would produce certs no member
    // could verify, which is worse than refusing.
    let bytes = exactly_32(&hex_decode(&escrow.group_secret)?, "group secret")?;
    let derived = hex_encode(&SigningKey::from_bytes(&bytes).verifying_key().to_bytes());
    if derived != cred.group_pubkey {
        return Err(Error::Untrusted(
            "escrow secret does not match this group's public key".into(),
        ));
    }
    cred.group_secret = escrow.group_secret.clone();
    save_credential(dir, &cred)?;
    Ok(cred)
}

/// Strip the group secret from this node, leaving the covenant credential a peer should hold
/// (ADR-0018). Irreversible without the escrow, which is why it refuses to run until an escrow
/// has been exported and verified by the caller.
pub fn reduce_to_covenant(dir: &Path) -> Result<GroupCredential> {
    let mut cred = load(dir)?.ok_or_else(|| Error::Untrusted("no group enrolled".into()))?;
    cred.group_secret = String::new();
    save_credential(dir, &cred)?;
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
        warrant: None,
    })
}

/// Verify a peer's membership certificate against a group public key. Checks: the signature
/// chain over the canonical body — the group key directly, or (warranted certs, ADR-0026 §6)
/// cert → warranted minter's key → warrant → group key; that `node_id` is the fingerprint of
/// the certified `node_pubkey` (self-consistency); expiry; and the revocation list — for the
/// subject AND, on a warranted cert, for the minting door (severing a door kills the certs
/// only it vouched for). On success the caller may trust `node_pubkey` as a group member.
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
        return Err(Error::Untrusted(
            "membership: node_id ≠ pubkey fingerprint".into(),
        ));
    }
    let sig_bytes = crate::node::exactly_64(&hex_decode(&m.cert)?, "cert")?;
    let sig = ed25519_dalek::Signature::from_bytes(&sig_bytes);
    match &m.warrant {
        None => group_key
            .verify(&m.body_bytes()?, &sig)
            .map_err(|_| Error::Untrusted("membership: group signature did not verify".into())),
        Some(w) => {
            w.verify(group_key, group_id, now)?;
            if revoked.iter().any(|r| r == &w.node_id) {
                return Err(Error::Untrusted("membership: minting door revoked".into()));
            }
            let wk_bytes = exactly_32(&hex_decode(&w.node_pubkey)?, "warrant pubkey")?;
            let wk = VerifyingKey::from_bytes(&wk_bytes)
                .map_err(|_| Error::Untrusted("membership: bad warrant pubkey".into()))?;
            wk.verify(&m.body_bytes()?, &sig).map_err(|_| {
                Error::Untrusted("membership: minter signature did not verify".into())
            })
        }
    }
}

/// Verify a membership is **internally consistent** against a caller-supplied group public key,
/// without the verifier holding that key itself. Checks the cert signature under the provided
/// pubkey, the node_id ↔ node_pubkey fingerprint binding, and expiry. This proves the membership
/// is *well-formed* — not that its group pre-exists or that the verifier vouches for it. Used by
/// the rendezvous directory (ADR-0012), which lists doors it does not stand behind: a lighthouse
/// hosts meetings for meshes it is not a member of, so it cannot check against a known group key.
pub fn verify_membership_consistent(
    m: &Membership,
    group_pubkey_hex: &str,
    now: i64,
) -> Result<()> {
    if now >= m.expiry {
        return Err(Error::Untrusted("membership: expired".into()));
    }
    let pk = exactly_32(&hex_decode(&m.node_pubkey)?, "cert node pubkey")?;
    if fingerprint(&pk) != m.node_id {
        return Err(Error::Untrusted(
            "membership: node_id ≠ pubkey fingerprint".into(),
        ));
    }
    let gk_bytes = exactly_32(&hex_decode(group_pubkey_hex)?, "group pubkey")?;
    let group_key = VerifyingKey::from_bytes(&gk_bytes)
        .map_err(|_| Error::Untrusted("membership: bad group pubkey".into()))?;
    let sig_bytes = crate::node::exactly_64(&hex_decode(&m.cert)?, "cert")?;
    let sig = ed25519_dalek::Signature::from_bytes(&sig_bytes);
    match &m.warrant {
        None => group_key
            .verify(&m.body_bytes()?, &sig)
            .map_err(|_| Error::Untrusted("membership: group signature did not verify".into())),
        Some(w) => {
            w.verify(&group_key, &m.group_id, now)?;
            let wk_bytes = exactly_32(&hex_decode(&w.node_pubkey)?, "warrant pubkey")?;
            let wk = VerifyingKey::from_bytes(&wk_bytes)
                .map_err(|_| Error::Untrusted("membership: bad warrant pubkey".into()))?;
            wk.verify(&m.body_bytes()?, &sig).map_err(|_| {
                Error::Untrusted("membership: minter signature did not verify".into())
            })
        }
    }
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

    /// **The escrow rehearsal.** ADR-0018 concentrates minting on the lighthouse and says losing
    /// it should be an outage rather than the end of the group. That claim rests entirely on being
    /// able to restore — so it is rehearsed here, every run, rather than written down and hoped for.
    ///
    /// Walks the whole procedure: a mint-capable node exports its escrow, is reduced to a covenant
    /// credential (proving it then CANNOT mint), and is restored from the escrow (proving it can
    /// again, and that the certs it mints still verify under the original group key).
    #[test]
    fn the_escrow_survives_losing_the_minting_door() {
        let dir = tmp("escrow_rehearsal");
        let node = NodeKey::load_or_mint(&dir, "lighthouse").unwrap();
        let cred = create_group(&dir, &node, "TheRiver", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        let original_pubkey = cred.group_pubkey.clone();
        assert!(cred.can_mint());

        // 1. Export while we still can.
        let escrow = export_escrow(&dir, NOW).unwrap();
        assert_eq!(escrow.kind, ESCROW_KIND);
        assert_eq!(escrow.group_id, cred.group_id);
        assert!(
            !escrow.group_secret.is_empty(),
            "an empty escrow is worse than none"
        );

        // 2. Lose the minting door — reduce this node to what a peer holds.
        let reduced = reduce_to_covenant(&dir).unwrap();
        assert!(!reduced.can_mint(), "a covenant credential must not mint");
        assert!(
            reduced
                .mint_membership(&node.identity().node_id, &node.identity().pubkey, NOW, 3600)
                .is_err(),
            "minting must FAIL, not silently produce an unverifiable cert"
        );
        assert!(
            export_escrow(&dir, NOW).is_err(),
            "exporting from a secret-less node must fail rather than write empty insurance"
        );

        // 3. Restore.
        let back = restore_from_escrow(&dir, &escrow).unwrap();
        assert!(back.can_mint(), "restore must return minting authority");
        assert_eq!(
            back.group_pubkey, original_pubkey,
            "it must be the SAME group"
        );

        // 4. And the authority is real: a cert minted after restore verifies under the group key
        //    every existing member already trusts. Minted for a genuine second keypair, since a
        //    membership binds node_id to the fingerprint of the key it certifies.
        let joiner_dir = tmp("escrow_rehearsal_joiner");
        let joiner = NodeKey::load_or_mint(&joiner_dir, "newcomer")
            .unwrap()
            .identity();
        let m = back
            .mint_membership(&joiner.node_id, &joiner.pubkey, NOW, 3600)
            .unwrap();
        let gk = back.verifying_key().unwrap();
        verify_membership(&m, &gk, &back.group_id, NOW + 10, &[]).unwrap();
    }

    #[test]
    fn a_restore_refuses_the_wrong_group_and_the_wrong_secret() {
        let dir = tmp("escrow_refuses");
        let node = NodeKey::load_or_mint(&dir, "n").unwrap();
        let cred = create_group(&dir, &node, "TheRiver", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        let mut escrow = export_escrow(&dir, NOW).unwrap();

        // A different group's escrow must never graft onto this node.
        let mut foreign = escrow.clone();
        foreign.group_id = "someoneelse".into();
        assert!(restore_from_escrow(&dir, &foreign).is_err());

        // A secret that does not derive this group's public key would mint certs nobody can
        // verify — refusing is strictly better than "restoring" into a broken group.
        escrow.group_secret = hex_encode(&[9u8; 32]);
        assert!(restore_from_escrow(&dir, &escrow).is_err());

        // An unknown format is refused rather than guessed at.
        let mut odd = export_escrow(&dir, NOW).unwrap();
        odd.kind = "something-else".into();
        assert!(restore_from_escrow(&dir, &odd).is_err());

        assert_eq!(cred.group_id, load(&dir).unwrap().unwrap().group_id);
    }

    use super::*;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("familiar_mesh_group_{tag}"));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    const NOW: i64 = 1_770_000_000;

    /// ADR-0026 §6: a warranted member's certs verify by chain — and every link is guarded.
    #[test]
    fn a_warranted_member_mints_verifiable_certs_and_every_chain_link_guards() {
        let dir = tmp("warrant");
        let founder = NodeKey::load_or_mint(&dir, "founder").unwrap();
        let cred = create_group(&dir, &founder, "river", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        let gk = cred.verifying_key().unwrap();

        // The door-to-be, and the newcomer it will admit.
        let door = NodeKey::load_or_mint(&tmp("warrant_door"), "mac").unwrap();
        let door_id = door.identity();
        let newcomer = NodeKey::load_or_mint(&tmp("warrant_new"), "phone").unwrap();
        let new_id = newcomer.identity();

        let w = issue_warrant(&cred, &door_id.node_id, &door_id.pubkey, NOW, 3600).unwrap();
        let m =
            mint_membership_warranted(&door, &w, &new_id.node_id, &new_id.pubkey, NOW, 3600)
                .unwrap();

        // The chain verifies — full check and the consistency check devices use.
        verify_membership(&m, &gk, &cred.group_id, NOW + 10, &[]).unwrap();
        verify_membership_consistent(&m, &cred.group_pubkey, NOW + 10).unwrap();

        // An EXPIRED warrant kills the cert even while the cert itself is unexpired.
        assert!(verify_membership(&m, &gk, &cred.group_id, NOW + 3601, &[]).is_err());

        // A SEVERED door kills the certs only it vouched for.
        assert!(verify_membership(
            &m,
            &gk,
            &cred.group_id,
            NOW + 10,
            std::slice::from_ref(&door_id.node_id)
        )
        .is_err());

        // A warrant signed by a stranger's key is refused.
        let stranger_dir = tmp("warrant_stranger");
        let stranger = NodeKey::load_or_mint(&stranger_dir, "s").unwrap();
        let foreign = create_group(&stranger_dir, &stranger, "x", NOW, 3600).unwrap();
        let forged = issue_warrant(&foreign, &door_id.node_id, &door_id.pubkey, NOW, 3600).unwrap();
        let mut m2 = m.clone();
        m2.warrant = Some(forged);
        assert!(verify_membership(&m2, &gk, &cred.group_id, NOW + 10, &[]).is_err());

        // A node cannot mint with a warrant naming someone else.
        assert!(mint_membership_warranted(
            &newcomer,
            &w,
            &new_id.node_id,
            &new_id.pubkey,
            NOW,
            3600
        )
        .is_err());

        // And a warranted cert whose body was tampered with fails like any forgery.
        let mut m3 = m.clone();
        m3.expiry += 1;
        assert!(verify_membership(&m3, &gk, &cred.group_id, NOW + 10, &[]).is_err());
    }

    #[test]
    fn a_warrant_installs_only_on_the_node_it_names() {
        // The issuing side.
        let a_dir = tmp("winstall_a");
        let a = NodeKey::load_or_mint(&a_dir, "a").unwrap();
        let cred_a = create_group(&a_dir, &a, "g", NOW, DEFAULT_CERT_TTL_SECS).unwrap();

        // The target: a covenant (secret-less) member node.
        let b_dir = tmp("winstall_b");
        let b = NodeKey::load_or_mint(&b_dir, "b").unwrap();
        let b_id = b.identity();
        let b_membership = cred_a
            .mint_membership(&b_id.node_id, &b_id.pubkey, NOW, DEFAULT_CERT_TTL_SECS)
            .unwrap();
        save_credential(
            &b_dir,
            &GroupCredential::covenant(
                cred_a.group_id.clone(),
                cred_a.group_pubkey.clone(),
                "g".into(),
                b_membership,
            ),
        )
        .unwrap();

        let w = issue_warrant(&cred_a, &b_id.node_id, &b_id.pubkey, NOW, 3600).unwrap();
        install_warrant(&b_dir, &w, NOW + 1).unwrap();
        assert!(load_warrant(&b_dir, NOW + 2).is_some());
        assert!(
            load_warrant(&b_dir, NOW + 3601).is_none(),
            "a dead warrant quietly stops the door"
        );

        // A warrant naming a different node refuses to install here.
        let other = NodeKey::load_or_mint(&tmp("winstall_other"), "o").unwrap();
        let oid = other.identity();
        let wrong = issue_warrant(&cred_a, &oid.node_id, &oid.pubkey, NOW, 3600).unwrap();
        assert!(install_warrant(&b_dir, &wrong, NOW + 1).is_err());
    }

    #[test]
    fn create_join_and_cross_verify() {
        // Node A creates a group; Node B joins with A's join key. Each node's own cert must
        // verify against the shared group key, and A must trust B's cert (and vice-versa).
        let dir_a = tmp("a");
        let dir_b = tmp("b");
        let a = NodeKey::load_or_mint(&dir_a, "a").unwrap();
        let b = NodeKey::load_or_mint(&dir_b, "b").unwrap();

        let cred_a = create_group(&dir_a, &a, "river", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        let cred_b = join_group(
            &dir_b,
            &b,
            &cred_a.join_key(),
            "river",
            NOW,
            DEFAULT_CERT_TTL_SECS,
        )
        .unwrap();

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
        assert!(verify_membership(
            m,
            &gk,
            &cred.group_id,
            NOW + 1,
            std::slice::from_ref(&m.node_id)
        )
        .is_err());
        // Wrong group id.
        assert!(verify_membership(m, &gk, "deadbeef", NOW + 1, &[]).is_err());
        // Forged cert: flip a signature byte.
        let mut bad = m.clone();
        bad.cert
            .replace_range(0..2, if &bad.cert[0..2] == "00" { "01" } else { "00" });
        assert!(verify_membership(&bad, &gk, &cred.group_id, NOW + 1, &[]).is_err());
        // Different group key can't validate this cert.
        let dir2 = tmp("reject2");
        let n2 = NodeKey::load_or_mint(&dir2, "n2").unwrap();
        let other = create_group(&dir2, &n2, "other", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        assert!(verify_membership(
            m,
            &other.verifying_key().unwrap(),
            &cred.group_id,
            NOW + 1,
            &[]
        )
        .is_err());

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
        assert!(verify_membership(
            &m,
            &cred.verifying_key().unwrap(),
            &cred.group_id,
            NOW + 1,
            &[]
        )
        .is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
