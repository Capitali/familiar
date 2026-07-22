//! Observation ingestion — the **device seam**.
//!
//! A trusted mesh member that cannot serve gossip (an iPhone/Apple Watch agent — iOS can't run
//! a background TCP server) still needs to feed the familiar. So instead of a brief-for-brief
//! exchange, a device is a *pure client*: it POSTs a **signed batch of derived observations** to
//! `POST /mesh/observe`, and we verify it exactly as we verify a brief — the membership cert must
//! verify under the group key, and the node must have signed the payload — before appending each
//! observation to the canonical store.
//!
//! Two deliberate choices keep this safe and simple:
//! - **The signature covers the RAW request-body bytes** (carried in the `X-Familiar-Sig`
//!   header), not a re-serialization. So there is *no* cross-language JSON canonicalization to
//!   match — a Swift signer signs the literal bytes it transmits. The only thing it must
//!   byte-reproduce is the membership [`CertBody`](crate::group), which the group key signs.
//! - Every recorded observation is tagged `source = "mesh:<node_id>"`, so device data is
//!   **never laundered** into local sensing or the structural fingerprint (the same discipline
//!   [`crate::merge`] applies to federated patterns/presence).
//!
//! Freshness: the brief path declares `ts`/`nonce` but never enforced them; since this endpoint
//! is defined fresh, it *does* — a batch outside the replay window or reusing a nonce is rejected.

use crate::group::{self, Membership};
use crate::node::{fingerprint, NodeIdentity};
use crate::{exactly_32, hex_decode, Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::Path;
use std::sync::Mutex;

use familiar_kernel::observation::{self, Observation};

/// How far a batch's `ts` may drift from our clock before we reject it. Also how long the nonce
/// cache must remember a `(node,nonce)` pair — outside this window a replay fails the ts check.
pub const REPLAY_WINDOW_SECS: i64 = 300;

/// A single derived observation as sent by a device agent — the semantic triple plus optional
/// context and confidence. `id`, `source`, and `ts` are assigned by [`ingest_observations`]; the
/// device supplies only what it perceived.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsRecord {
    pub actor: String,
    pub action: String,
    pub object: String,
    #[serde(default)]
    pub context: String,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

fn default_confidence() -> f64 {
    0.9
}

/// The signed envelope a device POSTs to `/mesh/observe`. Carries the sender's public identity
/// and membership cert (so we can trust it exactly like a brief) alongside the batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObserveEnvelope {
    pub node: NodeIdentity,
    pub membership: Membership,
    /// Unix seconds the device stamped this batch — checked against the replay window.
    pub ts: i64,
    /// Per-batch random token (hex) — a repeat within the window is a replay.
    pub nonce: String,
    pub observations: Vec<ObsRecord>,
}

/// A same-`(actor,action,object)` triple more often than this is dropped as noise. Derived
/// observations are state-changes ("still" → "walking"); re-affirming an unchanged state every
/// second carries no information and only floods the store. Transitions differ in `object`, so
/// they always pass — only *identical* triples inside this window are suppressed.
pub const DEBOUNCE_SECS: i64 = 60;

/// The transport-held ingest state: anti-replay for `(node_id, nonce)` pairs, and a same-triple
/// debounce so a chatty or buggy in-group device can't flood the observation store. Both are
/// in-memory (reset on restart) and self-trimming — the `ts` window bounds their size.
#[derive(Default)]
pub struct IngestGuard {
    nonces: VecDeque<(String, String, i64)>, // (node_id, nonce, stamped_at)
    recent: std::collections::HashMap<String, i64>, // "actor\u{1}action\u{1}object" -> last ts
}

impl IngestGuard {
    /// Remember `(node_id, nonce)` at `now`. Returns `true` if fresh, `false` if already seen
    /// inside the replay window. Shared with the worldview read seam so both writes and reads are
    /// replay-bounded by the same ring.
    pub(crate) fn remember_nonce(&mut self, node_id: &str, nonce: &str, now: i64) -> bool {
        while let Some((_, _, t)) = self.nonces.front() {
            if now - *t > REPLAY_WINDOW_SECS {
                self.nonces.pop_front();
            } else {
                break;
            }
        }
        if self
            .nonces
            .iter()
            .any(|(n, x, _)| n == node_id && x == nonce)
        {
            return false;
        }
        self.nonces
            .push_back((node_id.to_string(), nonce.to_string(), now));
        true
    }

    /// Should this triple be recorded now? `false` if the identical triple was recorded within
    /// [`DEBOUNCE_SECS`]. Updates the last-seen stamp when it allows the triple.
    fn allow_triple(&mut self, actor: &str, action: &str, object: &str, now: i64) -> bool {
        let key = format!("{actor}\u{1}{action}\u{1}{object}");
        if let Some(&t) = self.recent.get(&key) {
            if now - t < DEBOUNCE_SECS {
                return false;
            }
        }
        self.recent.insert(key, now);
        if self.recent.len() > 4096 {
            self.recent.retain(|_, t| now - *t < DEBOUNCE_SECS);
        }
        true
    }
}

/// Verify a signed observation batch and, if trusted and fresh, append every observation to the
/// canonical store tagged `mesh:<node_id>`. Returns how many were recorded.
///
/// `raw` is the exact request-body bytes the signature covers; `sig_hex` is the `X-Familiar-Sig`
/// header value. Fail-closed: an `Untrusted` error means the caller answers 403 (or 409 for a
/// replay); it must never partially apply an unverified batch.
pub(crate) fn ingest_observations(
    dir: &Path,
    raw: &[u8],
    sig_hex: &str,
    now: i64,
    guard: &Mutex<IngestGuard>,
) -> Result<usize> {
    // Gate: the human must have opened the mesh and not disabled device ingestion.
    if !familiar_kernel::boundary::load(dir)
        .map_err(Error::Io)?
        .allow_mesh
    {
        return Err(Error::Untrusted("mesh gate closed".into()));
    }
    if !crate::config::load(dir)?.accept_observations {
        return Err(Error::Untrusted("observation ingestion disabled".into()));
    }
    let cred = group::load(dir)?.ok_or_else(|| Error::Untrusted("no group enrolled".into()))?;
    let env: ObserveEnvelope = serde_json::from_slice(raw)?;

    // 1. Membership cert verifies under the group key (the same trust path a brief takes).
    let gk = cred.verifying_key()?;
    let revoked = group::load_revoked(dir).unwrap_or_default();
    group::verify_membership(&env.membership, &gk, &cred.group_id, now, &revoked)?;

    // 2. Cross-bind: the cert certifies the very key that signs this batch, and node_id is the
    //    fingerprint of that key — so a valid cert can't be replayed onto a different node key.
    let pk = exactly_32(&hex_decode(&env.node.pubkey)?, "node pubkey")?;
    if fingerprint(&pk) != env.node.node_id
        || env.membership.node_pubkey != env.node.pubkey
        || env.membership.node_id != env.node.node_id
    {
        return Err(Error::Untrusted(
            "node identity does not match its membership".into(),
        ));
    }

    // 3. The node signed these exact bytes.
    env.node.verify(raw, sig_hex)?;

    // 4. Freshness: within the replay window.
    if (now - env.ts).abs() > REPLAY_WINDOW_SECS {
        return Err(Error::Untrusted("stale or future timestamp".into()));
    }

    // Under one short lock (no IO held): reject a replayed nonce, then pick the observations to
    // keep — complete triples that aren't an unchanged repeat within the debounce window. The
    // debounce protects the store from a chatty/buggy in-group device flooding it.
    let keep: Vec<usize> = {
        let mut g = guard.lock().unwrap_or_else(|p| p.into_inner());
        if !g.remember_nonce(&env.node.node_id, &env.nonce, now) {
            return Err(Error::Untrusted("replayed nonce".into()));
        }
        env.observations
            .iter()
            .enumerate()
            .filter(|(_, o)| {
                !o.actor.trim().is_empty()
                    && !o.action.trim().is_empty()
                    && !o.object.trim().is_empty()
            })
            .filter(|(_, o)| g.allow_triple(&o.actor, &o.action, &o.object, now))
            .map(|(i, _)| i)
            .collect()
    };

    // Trusted + fresh: append the survivors, tagged so they stay quarantined from local sensing.
    let source = format!("mesh:{}", env.node.node_id);
    for &i in &keep {
        let o = &env.observations[i];
        let obs = Observation::new(
            o.actor.clone(),
            o.action.clone(),
            o.object.clone(),
            o.context.clone(),
            source.clone(),
            env.ts,
            o.confidence.clamp(0.0, 1.0),
        );
        // A device's answer aimed at a thread ("thread:<id>" context) attaches as that
        // thread's evidence — the same non-dead-end path as a local console answer.
        if let Some(thread_id) = obs.context.strip_prefix("thread:") {
            let _ = familiar_kernel::thread::add_answer(dir, thread_id, &obs.object, env.ts);
        }
        observation::record(dir, obs).map_err(Error::Io)?;
    }
    Ok(keep.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::{self, GroupCredential, DEFAULT_CERT_TTL_SECS};
    use crate::node::NodeKey;
    use std::path::PathBuf;

    const NOW: i64 = 1_000_000;

    /// A host dir enrolled in a group with the mesh gate open, plus a standalone device node key
    /// (its own dir so it doesn't clobber the host's). Returns (host_dir, group_cred, device).
    fn setup(tag: &str) -> (PathBuf, GroupCredential, NodeKey) {
        let host = fresh(&format!("host_{tag}"));
        let host_node = NodeKey::load_or_mint(&host, "host").unwrap();
        let cred =
            group::create_group(&host, &host_node, "river", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        open_gate(&host, true);
        let device = NodeKey::load_or_mint(&fresh(&format!("dev_{tag}")), "iPhone").unwrap();
        (host, cred, device)
    }

    fn fresh(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("familiar_observe_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn open_gate(dir: &Path, on: bool) {
        let mut b = familiar_kernel::boundary::Boundary::closed();
        b.allow_mesh = on;
        std::fs::write(dir.join("boundary.json"), serde_json::to_vec(&b).unwrap()).unwrap();
    }

    fn obs(object: &str) -> ObsRecord {
        ObsRecord {
            actor: "phone:ian".into(),
            action: "reports".into(),
            object: object.into(),
            context: "acc=12m".into(),
            confidence: 0.95,
        }
    }

    /// Build a signed batch. `issued`/`ttl` control the cert window; `ts`/`nonce` the freshness.
    fn signed(
        cred: &GroupCredential,
        device: &NodeKey,
        ts: i64,
        nonce: &str,
        issued: i64,
        ttl: i64,
        records: Vec<ObsRecord>,
    ) -> (Vec<u8>, String) {
        let id = device.identity();
        let membership = cred
            .mint_membership(&id.node_id, &id.pubkey, issued, ttl)
            .unwrap();
        let env = ObserveEnvelope {
            node: id,
            membership,
            ts,
            nonce: nonce.into(),
            observations: records,
        };
        let raw = serde_json::to_vec(&env).unwrap();
        let sig = device.sign(&raw);
        (raw, sig)
    }

    fn ring() -> Mutex<IngestGuard> {
        Mutex::new(IngestGuard::default())
    }

    #[test]
    fn nonce_guard_rejects_repeats_and_prunes_old() {
        let mut r = IngestGuard::default();
        assert!(r.remember_nonce("nodeA", "n1", 1000));
        assert!(!r.remember_nonce("nodeA", "n1", 1000)); // exact repeat rejected
        assert!(r.remember_nonce("nodeA", "n2", 1000)); // different nonce ok
        assert!(r.remember_nonce("nodeB", "n1", 1000)); // different node, same nonce ok
                                                        // after the window the old entry is pruned, so the nonce is accepted again
        assert!(r.remember_nonce("nodeA", "n1", 1000 + REPLAY_WINDOW_SECS + 1));
    }

    #[test]
    fn triple_debounce_suppresses_unchanged_repeats_but_lets_changes_through() {
        let mut g = IngestGuard::default();
        assert!(g.allow_triple("phone:ian", "reports", "motion:still", 1000));
        assert!(!g.allow_triple("phone:ian", "reports", "motion:still", 1010)); // repeat within window
        assert!(g.allow_triple("phone:ian", "reports", "motion:walking", 1010)); // a change passes
                                                                                 // once the debounce window elapses, the same state may be re-affirmed
        assert!(g.allow_triple(
            "phone:ian",
            "reports",
            "motion:still",
            1000 + DEBOUNCE_SECS + 1
        ));
    }

    #[test]
    fn a_chatty_device_is_debounced_across_batches() {
        let (host, cred, device) = setup("debounce");
        let g = ring();
        // Same triple in three quick batches (distinct nonces): only the first records.
        for (i, nonce) in ["a", "b", "c"].iter().enumerate() {
            let ts = NOW + i as i64; // all within the debounce window
            let (raw, sig) = signed(
                &cred,
                &device,
                ts,
                nonce,
                NOW,
                DEFAULT_CERT_TTL_SECS,
                vec![obs("motion:still")],
            );
            let n = ingest_observations(&host, &raw, &sig, ts, &g).unwrap();
            assert_eq!(
                n,
                if i == 0 { 1 } else { 0 },
                "only the first unchanged repeat records"
            );
        }
        assert_eq!(observation::load(&host).unwrap().len(), 1);
    }

    #[test]
    fn trusted_batch_lands_tagged_then_replay_is_rejected() {
        let (host, cred, device) = setup("happy");
        let r = ring();
        let (raw, sig) = signed(
            &cred,
            &device,
            NOW,
            "n1",
            NOW,
            DEFAULT_CERT_TTL_SECS,
            vec![obs("location:home")],
        );

        let n = ingest_observations(&host, &raw, &sig, NOW, &r).unwrap();
        assert_eq!(n, 1);
        let stored = observation::load(&host).unwrap();
        let rec = stored
            .iter()
            .find(|o| o.object == "location:home")
            .expect("recorded");
        assert_eq!(
            rec.source,
            format!("mesh:{}", device.node_id()),
            "tagged with the device node"
        );
        assert_eq!(rec.actor, "phone:ian");

        // Same nonce again → rejected as a replay, and nothing new is written.
        let err = ingest_observations(&host, &raw, &sig, NOW, &r).unwrap_err();
        assert!(matches!(err, Error::Untrusted(m) if m.contains("replay")));
        assert_eq!(observation::load(&host).unwrap().len(), stored.len());
    }

    #[test]
    fn a_bad_signature_is_untrusted() {
        let (host, cred, device) = setup("badsig");
        let (raw, _good) = signed(
            &cred,
            &device,
            NOW,
            "n1",
            NOW,
            DEFAULT_CERT_TTL_SECS,
            vec![obs("x")],
        );
        // A signature over *different* bytes: the cert still verifies, but node.verify(raw) fails.
        let wrong = device.sign(b"not the body");
        let err = ingest_observations(&host, &raw, &wrong, NOW, &ring()).unwrap_err();
        assert!(matches!(err, Error::Untrusted(_)));
        assert!(observation::load(&host).unwrap().is_empty());
    }

    #[test]
    fn a_cert_from_another_group_is_untrusted() {
        let (host, _cred, device) = setup("wronggrp");
        // A different group mints the device's cert — its group_id won't match the host's.
        let other = group::create_group(
            &fresh("othergrp"),
            &NodeKey::load_or_mint(&fresh("othernode"), "h2").unwrap(),
            "other",
            NOW,
            DEFAULT_CERT_TTL_SECS,
        )
        .unwrap();
        let (raw, sig) = signed(
            &other,
            &device,
            NOW,
            "n1",
            NOW,
            DEFAULT_CERT_TTL_SECS,
            vec![obs("x")],
        );
        let err = ingest_observations(&host, &raw, &sig, NOW, &ring()).unwrap_err();
        assert!(matches!(err, Error::Untrusted(_)));
    }

    #[test]
    fn an_expired_cert_is_untrusted() {
        let (host, cred, device) = setup("expired");
        // issued 100s ago with a 50s ttl → expired well before NOW.
        let (raw, sig) = signed(&cred, &device, NOW, "n1", NOW - 100, 50, vec![obs("x")]);
        let err = ingest_observations(&host, &raw, &sig, NOW, &ring()).unwrap_err();
        assert!(matches!(err, Error::Untrusted(m) if m.contains("expired")));
    }

    #[test]
    fn a_revoked_node_is_untrusted() {
        let (host, cred, device) = setup("revoked");
        std::fs::write(
            host.join(group::REVOKED_FILE),
            serde_json::to_vec(&vec![device.node_id()]).unwrap(),
        )
        .unwrap();
        let (raw, sig) = signed(
            &cred,
            &device,
            NOW,
            "n1",
            NOW,
            DEFAULT_CERT_TTL_SECS,
            vec![obs("x")],
        );
        let err = ingest_observations(&host, &raw, &sig, NOW, &ring()).unwrap_err();
        assert!(matches!(err, Error::Untrusted(m) if m.contains("revoked")));
    }

    #[test]
    fn a_stale_timestamp_is_rejected() {
        let (host, cred, device) = setup("stale");
        let ts = NOW - (REPLAY_WINDOW_SECS + 10); // outside the window; cert itself still valid
        let (raw, sig) = signed(
            &cred,
            &device,
            ts,
            "n1",
            NOW,
            DEFAULT_CERT_TTL_SECS,
            vec![obs("x")],
        );
        let err = ingest_observations(&host, &raw, &sig, NOW, &ring()).unwrap_err();
        assert!(matches!(err, Error::Untrusted(m) if m.contains("timestamp")));
    }

    #[test]
    fn a_closed_gate_or_disabled_ingestion_refuses() {
        let (host, cred, device) = setup("gate");
        let (raw, sig) = signed(
            &cred,
            &device,
            NOW,
            "n1",
            NOW,
            DEFAULT_CERT_TTL_SECS,
            vec![obs("x")],
        );

        open_gate(&host, false); // allow_mesh off
        assert!(matches!(
            ingest_observations(&host, &raw, &sig, NOW, &ring()).unwrap_err(),
            Error::Untrusted(m) if m.contains("gate closed")
        ));

        // Gate back on, but device ingestion switched off in config.
        open_gate(&host, true);
        let cfg = crate::config::MeshConfig {
            accept_observations: false,
            ..Default::default()
        };
        std::fs::write(
            host.join("mesh/config.json"),
            serde_json::to_vec(&cfg).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            ingest_observations(&host, &raw, &sig, NOW, &ring()).unwrap_err(),
            Error::Untrusted(m) if m.contains("disabled")
        ));
    }
}
