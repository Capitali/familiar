//! Theory replication across the mesh (T-195) — record-sync's third twin.
//!
//! Theories were per-node with nothing reconciling them. Two doors formed their own and
//! neither knew about the other's, so a fleet-wide purge only ever meant "on the node I typed
//! it into": 443 theories were retired here on 2026-08-15 while a sibling kept its own 130,
//! and restarting that sibling republished a theory the human had already dismissed. Ian:
//! *"theories need to use the record-sync that exists within the mesh, but this needs to
//! happen quickly and it needs to be accurate."*
//!
//! Accuracy is the harder half and it lives in [`familiar_kernel::thread::merge_incoming`]:
//! a terminal verdict is sticky, so no sync can revive what a door has already retired. This
//! module is only the carriage — the same proof shape, window and cap as device-sync, riding
//! the same dial-out so a CGNAT'd door still participates.

use crate::group::Membership;
use crate::node::NodeKey;
use crate::{Error, Result};
use familiar_kernel::thread::Thread;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// How far back a door offers theories, and the cap per offer — deliberately the record-sync
/// values: every replication channel rides one dial.
pub const THREAD_SYNC_WINDOW_SECS: i64 = 48 * 60 * 60;
pub const THREAD_SYNC_CAP: usize = 64;

/// The signed body of a thread-sync — record-sync's proof shape verbatim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSyncBody {
    pub node: crate::node::NodeIdentity,
    pub membership: Membership,
    pub ts: i64,
    pub nonce: String,
    pub threads: Vec<Thread>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSync {
    pub body: ThreadSyncBody,
    /// ed25519 (hex) by the sending door's node key over `serde_json(body)`.
    pub sig: String,
}

/// What counts as "touched" for replication: the later of when the thread was created, when it
/// last changed status, and when it was last worked.
///
/// A retirement must travel, and a retirement changes `status_at` without touching
/// `created_at` — keying the window on creation alone would have left exactly the verdicts
/// this channel exists to carry sitting at home.
fn touched_at(t: &Thread) -> i64 {
    t.created_at.max(t.status_at).max(t.last_worked_at)
}

/// Build + sign this door's theory offer: threads touched inside the window, newest first,
/// capped. None when there is nothing to say — the caller skips the POST entirely.
pub fn build_thread_sync(
    dir: &Path,
    cred: &crate::group::GroupCredential,
    node: &NodeKey,
    now: i64,
) -> Result<Option<ThreadSync>> {
    let mut recent: Vec<Thread> = familiar_kernel::thread::load(dir)
        .map_err(|e| Error::Malformed(format!("thread-sync: {e}")))?
        .into_iter()
        .filter(|t| now - touched_at(t) <= THREAD_SYNC_WINDOW_SECS)
        .collect();
    if recent.is_empty() {
        return Ok(None);
    }
    recent.sort_by_key(|t| std::cmp::Reverse(touched_at(t)));
    recent.truncate(THREAD_SYNC_CAP);
    let body = ThreadSyncBody {
        node: node.identity(),
        membership: cred.membership.clone(),
        ts: now,
        nonce: format!("{now:016x}"),
        threads: recent,
    };
    let sig = node.sign(&serde_json::to_vec(&body)?);
    Ok(Some(ThreadSync { body, sig }))
}

/// Verify a thread-sync came from a live member of OUR group — the record-sync checks.
pub fn verify_thread_sync(
    sync: &ThreadSync,
    group_key: &ed25519_dalek::VerifyingKey,
    group_id: &str,
    now: i64,
    revoked: &[String],
) -> Result<()> {
    let b = &sync.body;
    crate::group::verify_membership(&b.membership, group_key, group_id, now, revoked)?;
    if b.membership.node_pubkey != b.node.pubkey || b.membership.node_id != b.node.node_id {
        return Err(Error::Untrusted(
            "thread-sync: membership cert does not match the signing node".into(),
        ));
    }
    if now - b.ts > THREAD_SYNC_WINDOW_SECS {
        return Err(Error::Untrusted("thread-sync: stale".into()));
    }
    b.node.verify(&serde_json::to_vec(b)?, &sync.sig)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const NOW: i64 = 1_780_000_000;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let p =
            std::env::temp_dir().join(format!("familiar_thread_sync_{}_{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn a_retirement_is_inside_the_window_even_when_the_theory_is_old() {
        // The case the channel exists for: created long ago, retired just now. Keyed on
        // `created_at` alone this would never be offered, and the verdict would never travel.
        let mut t = familiar_kernel::thread::Thread {
            id: "t1".into(),
            question: String::new(),
            theory: String::new(),
            direction: String::new(),
            created_at: NOW - 30 * 24 * 60 * 60,
            status: "retired".into(),
            status_at: NOW - 60,
            last_worked_at: 0,
            reinforced: 0,
            answers: Vec::new(),
            origin: String::new(),
            origin_human: String::new(),
            actor: String::new(),
            anchors: Vec::new(),
            facts_rev: 0,
            facts_digest: String::new(),
            v: 1,
            family_key: "f".into(),
            variant_key: "v".into(),
            superseded_by: String::new(),
            kind: String::new(),
            expires_at: 0,
            rule_proposal: None,
        };
        assert!(
            NOW - touched_at(&t) <= THREAD_SYNC_WINDOW_SECS,
            "a fresh verdict on an old theory must still be offered"
        );
        t.status_at = NOW - 30 * 24 * 60 * 60;
        assert!(
            NOW - touched_at(&t) > THREAD_SYNC_WINDOW_SECS,
            "a theory nothing has touched in a month stays home"
        );
    }

    #[test]
    fn an_empty_store_offers_nothing_rather_than_an_empty_envelope() {
        let dir = tmp("empty");
        let node = NodeKey::load_or_mint(&dir, "familiar").unwrap();
        let cred =
            crate::group::create_group(&dir, &node, "g", NOW, crate::group::DEFAULT_CERT_TTL_SECS)
                .unwrap();
        assert!(build_thread_sync(&dir, &cred, &node, NOW)
            .unwrap()
            .is_none());
    }
}
