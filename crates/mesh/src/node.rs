//! Node identity — a stable ed25519 keypair per node.
//!
//! Each node mints an ed25519 signing key on first enrollment. The **node id** is a short
//! fingerprint of the public key (the first 8 bytes of `SHA-256(pubkey)`, hex) — stable,
//! self-certifying (anyone can recompute it from the pubkey), and human-legible when
//! paired with a hostname label. The private key lives in `mesh/node_key` (0600); the
//! public record in `mesh/node.json`. A node signs every brief it emits with this key;
//! peers verify the signature against the pubkey named in the brief, which the group
//! membership certificate in turn binds to the group.

use crate::{exactly_32, hex_decode, hex_encode, os_random, Error, Result};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

/// Private key file (0600) — 32 raw secret bytes, hex-encoded.
pub const NODE_KEY_FILE: &str = "mesh/node_key";
/// Public node record.
pub const NODE_FILE: &str = "mesh/node.json";

/// A node's public identity — what peers learn about it. Self-certifying: `node_id` is
/// recomputable from `pubkey`, so a mismatched pair is rejected on load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeIdentity {
    /// Short fingerprint of the public key — the stable network name.
    pub node_id: String,
    /// ed25519 public key, hex-encoded (32 bytes).
    pub pubkey: String,
    /// Human-readable label (e.g. hostname). Cosmetic; never trusted for identity.
    pub label: String,
}

impl NodeIdentity {
    /// The verifying (public) key parsed from `pubkey`.
    pub fn verifying_key(&self) -> Result<VerifyingKey> {
        let bytes = exactly_32(&hex_decode(&self.pubkey)?, "node pubkey")?;
        VerifyingKey::from_bytes(&bytes).map_err(|e| Error::Malformed(format!("node pubkey: {e}")))
    }

    /// Verify a signature made by this node over `msg`.
    pub fn verify(&self, msg: &[u8], sig_hex: &str) -> Result<()> {
        let vk = self.verifying_key()?;
        let sig_bytes = exactly_64(&hex_decode(sig_hex)?, "node signature")?;
        let sig = ed25519_dalek::Signature::from_bytes(&sig_bytes);
        vk.verify(msg, &sig)
            .map_err(|_| Error::Untrusted("node signature did not verify".into()))
    }
}

/// A node's full keypair — the private half, held only locally.
pub struct NodeKey {
    signing: SigningKey,
    label: String,
}

impl NodeKey {
    /// The public identity derived from this key.
    pub fn identity(&self) -> NodeIdentity {
        let pubkey = self.signing.verifying_key().to_bytes();
        NodeIdentity {
            node_id: fingerprint(&pubkey),
            pubkey: hex_encode(&pubkey),
            label: self.label.clone(),
        }
    }

    /// This node's id (short pubkey fingerprint).
    pub fn node_id(&self) -> String {
        fingerprint(&self.signing.verifying_key().to_bytes())
    }

    /// Sign a message, returning a hex-encoded signature.
    pub fn sign(&self, msg: &[u8]) -> String {
        hex_encode(&self.signing.sign(msg).to_bytes())
    }

    /// Load the node key from disk, minting a fresh one on first run. `label` is used only
    /// when minting (an existing record keeps its stored label).
    pub fn load_or_mint(dir: &Path, label: &str) -> Result<NodeKey> {
        let key_path = dir.join(NODE_KEY_FILE);
        if key_path.exists() {
            let secret = exactly_32(&hex_decode(&fs::read_to_string(&key_path)?)?, "node key")?;
            let signing = SigningKey::from_bytes(&secret);
            // Prefer the persisted label if the public record exists.
            let label = fs::read_to_string(dir.join(NODE_FILE))
                .ok()
                .and_then(|s| serde_json::from_str::<NodeIdentity>(&s).ok())
                .map(|id| id.label)
                .unwrap_or_else(|| label.to_string());
            return Ok(NodeKey { signing, label });
        }
        // Mint. A generic/empty label becomes the hostname — a node is named by its host
        // (FamTalker01, wildhorse, …), so a fleet's roster never reads "familiar" seven times.
        let label = match label {
            "" | "familiar" => hostname().unwrap_or_else(|| label.to_string()),
            other => other.to_string(),
        };
        let secret: [u8; 32] = os_random()?;
        let signing = SigningKey::from_bytes(&secret);
        let node = NodeKey { signing, label };
        write_private(&key_path, &hex_encode(&secret))?;
        crate::group::write_json_public(&dir.join(NODE_FILE), &node.identity())?;
        Ok(node)
    }
}

/// The host's short name (`uname -n`, first dot-segment), or `None` when unavailable.
fn hostname() -> Option<String> {
    let out = std::process::Command::new("uname").arg("-n").output().ok()?;
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let short = name.split('.').next().unwrap_or("").to_string();
    (!short.is_empty()).then_some(short)
}

/// The short fingerprint of a 32-byte public key: first 8 bytes of its SHA-256, hex.
pub fn fingerprint(pubkey: &[u8; 32]) -> String {
    let digest = Sha256::digest(pubkey);
    hex_encode(&digest[..8])
}

pub(crate) fn exactly_64(bytes: &[u8], what: &str) -> Result<[u8; 64]> {
    bytes
        .try_into()
        .map_err(|_| Error::Malformed(format!("{what}: expected 64 bytes, got {}", bytes.len())))
}

/// Write a secret to a 0600 file, creating parent dirs. Best-effort perms on non-unix.
pub(crate) fn write_private(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;
    set_owner_only(path)?;
    Ok(())
}

#[cfg(unix)]
fn set_owner_only(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_owner_only(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("familiar_mesh_node_{tag}"));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn mint_is_stable_across_reload_and_id_matches_pubkey() {
        let dir = tmp("mint");
        let a = NodeKey::load_or_mint(&dir, "wildhorse").unwrap();
        let id_a = a.identity();
        // Reload: same key, same id, label persisted (ignoring the passed label).
        let b = NodeKey::load_or_mint(&dir, "ignored").unwrap();
        assert_eq!(a.node_id(), b.node_id());
        assert_eq!(b.identity().label, "wildhorse");
        // node_id is recomputable from the pubkey — self-certifying.
        let pk = exactly_32(&hex_decode(&id_a.pubkey).unwrap(), "pk").unwrap();
        assert_eq!(id_a.node_id, fingerprint(&pk));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sign_then_verify_round_trips_and_rejects_tampering() {
        let dir = tmp("sign");
        let node = NodeKey::load_or_mint(&dir, "n").unwrap();
        let id = node.identity();
        let sig = node.sign(b"hello mesh");
        assert!(id.verify(b"hello mesh", &sig).is_ok());
        assert!(id.verify(b"tampered", &sig).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn private_key_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tmp("perms");
        NodeKey::load_or_mint(&dir, "n").unwrap();
        let mode = fs::metadata(dir.join(NODE_KEY_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        let _ = fs::remove_dir_all(&dir);
    }
}
