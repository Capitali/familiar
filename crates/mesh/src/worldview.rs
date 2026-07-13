//! The read seam — a member device asks the familiar for its worldview.
//!
//! Symmetric to [`crate::observe`]: a device is a pure client. `POST /mesh/observe` lets it
//! *write* derived observations; `POST /mesh/worldview` lets it *read* a compact snapshot of what
//! the familiar knows — so an iPad can present a Glass-like console (the familiar's own Glass reads
//! the data dir directly; a peer can't, so it asks).
//!
//! Same trust path as ingestion: the request is a signed, membership-bearing envelope, verified
//! exactly as an observe batch (membership cert under the group key, node signed the raw bytes,
//! fresh ts, unreplayed nonce). Only a verified in-group node gets an answer, and only while the
//! human has the mesh open. A read is less sensitive than a write, but we hold the same line — no
//! worldview leaks to a non-member.

use crate::group::{self, Membership};
use crate::node::{fingerprint, NodeIdentity};
use crate::observe::{IngestGuard, REPLAY_WINDOW_SECS};
use crate::{exactly_32, hex_decode, Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

/// A signed read request: identity + freshness, no payload. The same envelope shape as an observe
/// batch minus the observations, so the Swift client reuses its signer verbatim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewRequest {
    pub node: NodeIdentity,
    pub membership: Membership,
    pub ts: i64,
    pub nonce: String,
}

/// One observation as the console shows it — a flat view of the kernel's `Observation`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsView {
    pub actor: String,
    pub action: String,
    pub object: String,
    pub context: String,
    pub source: String,
    pub ts: i64,
    pub confidence: f64,
}

/// A federated peer as last seen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerView {
    pub node_id: String,
    pub label: String,
    pub last_seen: i64,
    pub tools_offered: usize,
    pub patterns_offered: usize,
}

/// The compact snapshot returned to a member device — enough to render a Glass-like console: the
/// three constitutional meters, the peer roster, and the recent observation feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worldview {
    pub group_label: String,
    /// The familiar's own node id (so the console can distinguish self from peers).
    pub node_id: String,
    pub presence: f64,
    pub withdrawn: bool,
    pub service: f64,
    pub capacity: f64,
    pub observation_count: usize,
    pub peers: Vec<PeerView>,
    /// Newest first, capped at [`RECENT_CAP`].
    pub recent: Vec<ObsView>,
}

/// How many recent observations the snapshot carries. A console shows a live tail, not the archive.
const RECENT_CAP: usize = 60;

/// Verify a signed read request and, if trusted, assemble the familiar's worldview snapshot.
/// Fail-closed: an `Untrusted` error means the caller answers 403 (or 409 for a replay).
pub(crate) fn read_worldview(
    dir: &Path,
    raw: &[u8],
    sig_hex: &str,
    now: i64,
    guard: &Mutex<IngestGuard>,
) -> Result<Worldview> {
    if !familiar_kernel::boundary::load(dir).map_err(Error::Io)?.allow_mesh {
        return Err(Error::Untrusted("mesh gate closed".into()));
    }
    let cred = group::load(dir)?.ok_or_else(|| Error::Untrusted("no group enrolled".into()))?;
    let req: ViewRequest = serde_json::from_slice(raw)?;

    // Same trust path as ingestion (see observe.rs): cert under the group key, cross-bound to the
    // signing node, node signed these exact bytes, fresh ts, unreplayed nonce.
    let gk = cred.verifying_key()?;
    let revoked = group::load_revoked(dir).unwrap_or_default();
    group::verify_membership(&req.membership, &gk, &cred.group_id, now, &revoked)?;

    let pk = exactly_32(&hex_decode(&req.node.pubkey)?, "node pubkey")?;
    if fingerprint(&pk) != req.node.node_id
        || req.membership.node_pubkey != req.node.pubkey
        || req.membership.node_id != req.node.node_id
    {
        return Err(Error::Untrusted(
            "node identity does not match its membership".into(),
        ));
    }
    req.node.verify(raw, sig_hex)?;
    if (now - req.ts).abs() > REPLAY_WINDOW_SECS {
        return Err(Error::Untrusted("stale or future timestamp".into()));
    }
    {
        let mut g = guard.lock().unwrap_or_else(|p| p.into_inner());
        if !g.remember_nonce(&req.node.node_id, &req.nonce, now) {
            return Err(Error::Untrusted("replayed nonce".into()));
        }
    }

    // Trusted member — assemble the snapshot from the canonical store + the three signals + peers.
    let obs = familiar_kernel::observation::load(dir).map_err(Error::Io)?;
    let presence = familiar_kernel::presence::presence_signal(&obs, now);
    let service = familiar_kernel::service::service_signal(&obs);
    let capacity = familiar_kernel::capacities::capacities_signal(&obs);

    let recent: Vec<ObsView> = obs
        .iter()
        .rev()
        .take(RECENT_CAP)
        .map(|o| ObsView {
            actor: o.actor.clone(),
            action: o.action.clone(),
            object: o.object.clone(),
            context: o.context.clone(),
            source: o.source.clone(),
            ts: o.ts,
            confidence: o.confidence,
        })
        .collect();

    let peers: Vec<PeerView> = crate::transport::load_peers(dir)
        .into_iter()
        .map(|p| PeerView {
            node_id: p.node_id,
            label: p.label,
            last_seen: p.last_seen,
            tools_offered: p.tools_offered,
            patterns_offered: p.patterns_offered,
        })
        .collect();

    Ok(Worldview {
        group_label: cred.label,
        node_id: cred.membership.node_id,
        presence: presence.measure,
        withdrawn: presence.withdrawn,
        service: service.measure,
        capacity: capacity.measure,
        observation_count: obs.len(),
        peers,
        recent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::{self, GroupCredential, DEFAULT_CERT_TTL_SECS};
    use crate::node::NodeKey;
    use crate::observe::IngestGuard;
    use familiar_kernel::observation::{self, Observation};
    use std::path::{Path, PathBuf};

    const NOW: i64 = 1_000_000;

    fn fresh(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("familiar_worldview_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn open_gate(dir: &Path, on: bool) {
        let mut b = familiar_kernel::boundary::Boundary::closed();
        b.allow_mesh = on;
        std::fs::write(dir.join("boundary.json"), serde_json::to_vec(&b).unwrap()).unwrap();
    }

    fn setup(tag: &str) -> (PathBuf, GroupCredential, NodeKey) {
        let host = fresh(&format!("host_{tag}"));
        let host_node = NodeKey::load_or_mint(&host, "host").unwrap();
        let cred = group::create_group(&host, &host_node, "river", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        open_gate(&host, true);
        let device = NodeKey::load_or_mint(&fresh(&format!("dev_{tag}")), "iPad").unwrap();
        (host, cred, device)
    }

    fn signed_request(cred: &GroupCredential, device: &NodeKey, ts: i64, nonce: &str) -> (Vec<u8>, String) {
        let id = device.identity();
        let membership = cred.mint_membership(&id.node_id, &id.pubkey, NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        let req = ViewRequest { node: id, membership, ts, nonce: nonce.into() };
        let raw = serde_json::to_vec(&req).unwrap();
        let sig = device.sign(&raw);
        (raw, sig)
    }

    fn ring() -> Mutex<IngestGuard> {
        Mutex::new(IngestGuard::default())
    }

    #[test]
    fn a_trusted_member_gets_the_snapshot() {
        let (host, cred, device) = setup("ok");
        // Seed a served-facing observation so presence is non-zero and it appears in `recent`.
        observation::record(
            &host,
            Observation::new("ian", "asked", "the familiar for help", "", "local", NOW, 0.9),
        )
        .unwrap();

        let (raw, sig) = signed_request(&cred, &device, NOW, "v1");
        let view = read_worldview(&host, &raw, &sig, NOW, &ring()).unwrap();
        assert_eq!(view.group_label, "river");
        assert_eq!(view.observation_count, 1);
        assert_eq!(view.recent.len(), 1);
        assert_eq!(view.recent[0].object, "the familiar for help");
    }

    #[test]
    fn a_replayed_request_is_rejected() {
        let (host, cred, device) = setup("replay");
        let (raw, sig) = signed_request(&cred, &device, NOW, "v1");
        let r = ring();
        assert!(read_worldview(&host, &raw, &sig, NOW, &r).is_ok());
        let err = read_worldview(&host, &raw, &sig, NOW, &r).unwrap_err();
        assert!(matches!(err, Error::Untrusted(m) if m.contains("replay")));
    }

    #[test]
    fn a_non_member_is_refused() {
        let (host, _cred, device) = setup("nonmember");
        // A different group mints the device's cert — it won't verify under the host's group key.
        let other = group::create_group(
            &fresh("othergrp"),
            &NodeKey::load_or_mint(&fresh("othernode"), "h2").unwrap(),
            "other", NOW, DEFAULT_CERT_TTL_SECS,
        )
        .unwrap();
        let (raw, sig) = signed_request(&other, &device, NOW, "v1");
        let err = read_worldview(&host, &raw, &sig, NOW, &ring()).unwrap_err();
        assert!(matches!(err, Error::Untrusted(_)));
    }

    #[test]
    fn a_closed_gate_refuses() {
        let (host, cred, device) = setup("gate");
        open_gate(&host, false);
        let (raw, sig) = signed_request(&cred, &device, NOW, "v1");
        let err = read_worldview(&host, &raw, &sig, NOW, &ring()).unwrap_err();
        assert!(matches!(err, Error::Untrusted(m) if m.contains("gate closed")));
    }
}
