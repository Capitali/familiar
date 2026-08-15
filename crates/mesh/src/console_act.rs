//! Signed writes from the shared console.
//!
//! A console is a mesh member, never an ambient administrator. It proves the same
//! certificate/key binding as an observation or worldview read, stamps a fresh nonce, and may
//! perform only the two deliberately narrow acts represented here:
//!
//! - disable an existing standing reaction rule (a reduction of authority), or
//! - name the certified device that signed the request (never another device).
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
/// wire legible: `{ "kind": "disable_rule", "rule_id": "…" }` or
/// `{ "kind": "name_device", "name": "…" }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConsoleAct {
    DisableRule { rule_id: String },
    NameDevice { name: String },
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

/// Verify and apply one console act. Verification completes before any mutation; a replay never
/// reaches the store. Success returns the door's short, human-readable acknowledgement.
pub(crate) fn apply(
    dir: &Path,
    raw: &[u8],
    sig_hex: &str,
    now: i64,
    guard: &Mutex<IngestGuard>,
) -> Result<String> {
    if !familiar_kernel::boundary::load(dir)
        .map_err(Error::Io)?
        .allow_mesh
    {
        return Err(Error::Untrusted("mesh gate closed".into()));
    }

    let cred = group::load(dir)?.ok_or_else(|| Error::Untrusted("no group enrolled".into()))?;
    let env: ConsoleActEnvelope = serde_json::from_slice(raw)?;
    let group_key = cred.verifying_key()?;
    let revoked = group::load_revoked(dir).unwrap_or_default();
    group::verify_membership(&env.membership, &group_key, &cred.group_id, now, &revoked)?;

    let public_key = exactly_32(&hex_decode(&env.node.pubkey)?, "node pubkey")?;
    if fingerprint(&public_key) != env.node.node_id
        || env.membership.node_pubkey != env.node.pubkey
        || env.membership.node_id != env.node.node_id
    {
        return Err(Error::Untrusted(
            "node identity does not match its membership".into(),
        ));
    }
    env.node.verify(raw, sig_hex)?;

    if (now - env.ts).abs() > REPLAY_WINDOW_SECS {
        return Err(Error::Untrusted("stale or future timestamp".into()));
    }
    if standing::standing_of(dir, &env.node.node_id) != Standing::Full {
        return Err(Error::Untrusted(
            "full standing is required for console writes".into(),
        ));
    }
    {
        let mut seen = guard.lock().unwrap_or_else(|p| p.into_inner());
        if !seen.remember_nonce(&env.node.node_id, &env.nonce, now) {
            return Err(Error::Untrusted("replayed nonce".into()));
        }
    }

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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeKey;
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
            membership,
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
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(guest_dir);
    }
}
