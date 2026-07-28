//! The device-oracle consult seam (ADR-0014) — the mesh borrows a member device's on-device LLM.
//!
//! The familiar queues a prompt under its data dir; a member device pulls pending prompts over a
//! signed, membership-bearing seam, runs its own model, and pushes back a structured answer. Nothing
//! but the answer leaves the device. The transport moves opaque JSON — the adapter, not this seam,
//! judges the content. Soft state: a prompt that isn't answered within [`PROMPT_TTL_SECS`] is
//! abandoned (the device was asleep; the muse retries later).

use crate::group::{self, Membership};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Queue directory under the data dir: `<id>.prompt.json` in, `<id>.answer.json` out.
pub const QUEUE_DIR: &str = "llm/device-queue";
/// A prompt older than this is stale — the device never picked it up in time.
pub const PROMPT_TTL_SECS: i64 = 60 * 60;

/// A question the familiar wants a device to answer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsultPrompt {
    pub id: String,
    pub prompt: String,
    pub ts: i64,
    /// Which on-device generation strategy the answering device should use (ADR-0014): "script"
    /// and "theory" select `@Generable` guided generation matching the muse's contract; "" / "free"
    /// is free-form `respond(to:)`. Optional on the wire (serde default) so older peers and the
    /// Python adapter that omit it still deserialize — the device just falls back to free-form.
    #[serde(default)]
    pub kind: String,
}

/// The device's answer. `json` is opaque — a structured result the adapter parses, not this seam.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsultAnswer {
    pub id: String,
    pub json: String,
    pub node_id: String,
    pub ts: i64,
}

/// A signed pull — "any prompts for me?" — so only a member can drain the queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsultPull {
    pub membership: Membership,
    pub group_pubkey: String,
    pub nonce: String,
    pub ts: i64,
}

/// A signed answer push.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsultAnswerReport {
    pub membership: Membership,
    pub group_pubkey: String,
    pub answer: ConsultAnswer,
    pub nonce: String,
    pub ts: i64,
}

/// A hub's signed relay to a broker (ADR-0014): "these are the prompts I am still waiting on — hold
/// them for whichever device reaches you, and hand me back any answers you have."
///
/// One message carries both directions because the hub is the only side that can open a connection:
/// a home familiar behind CGNAT is unreachable from the lighthouse, so the broker can never push. The
/// prompt list is the *whole* current pending set, not a delta — that makes it self-healing (a lost
/// round is simply resent) and doubles as the acknowledgement that retires finished work: anything the
/// hub stops listing, the broker drops.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsultRelay {
    pub membership: Membership,
    pub group_pubkey: String,
    pub prompts: Vec<ConsultPrompt>,
    pub nonce: String,
    pub ts: i64,
}

impl ConsultRelay {
    pub fn verify_sig(&self, raw: &[u8], sig_hex: &str) -> Result<()> {
        verify_node_sig(&self.membership, raw, sig_hex)
    }
}

fn verify_node_sig(membership: &Membership, raw: &[u8], sig_hex: &str) -> Result<()> {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};
    let pk = crate::exactly_32(&crate::hex_decode(&membership.node_pubkey)?, "node pubkey")?;
    let key =
        VerifyingKey::from_bytes(&pk).map_err(|_| Error::Untrusted("bad node pubkey".into()))?;
    let sig_bytes = crate::node::exactly_64(&crate::hex_decode(sig_hex)?, "sig")?;
    key.verify(raw, &Signature::from_bytes(&sig_bytes))
        .map_err(|_| Error::Untrusted("consult: node signature did not verify".into()))
}

impl ConsultPull {
    pub fn verify_sig(&self, raw: &[u8], sig_hex: &str) -> Result<()> {
        verify_node_sig(&self.membership, raw, sig_hex)
    }
}
impl ConsultAnswerReport {
    pub fn verify_sig(&self, raw: &[u8], sig_hex: &str) -> Result<()> {
        verify_node_sig(&self.membership, raw, sig_hex)
    }
}

fn queue(dir: &Path) -> std::path::PathBuf {
    dir.join(QUEUE_DIR)
}

/// Queue a prompt for a device to answer. Returns the stored prompt (with its id + timestamp).
pub fn enqueue(dir: &Path, id: &str, prompt: &str, kind: &str, now: i64) -> Result<ConsultPrompt> {
    let p = ConsultPrompt {
        id: id.to_string(),
        prompt: prompt.to_string(),
        ts: now,
        kind: kind.to_string(),
    };
    std::fs::create_dir_all(queue(dir)).map_err(|e| Error::Malformed(format!("consult: {e}")))?;
    let path = queue(dir).join(format!("{id}.prompt.json"));
    let body = serde_json::to_vec(&p).map_err(|e| Error::Malformed(format!("consult: {e}")))?;
    std::fs::write(path, body).map_err(|e| Error::Malformed(format!("consult: {e}")))?;
    Ok(p)
}

/// Live prompts a device could pick up — stale ones (past the TTL) are dropped as it lists them.
pub fn pending(dir: &Path, now: i64) -> Vec<ConsultPrompt> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(queue(dir)) else {
        return out;
    };
    for e in entries.flatten() {
        let path = e.path();
        if !path.to_string_lossy().ends_with(".prompt.json") {
            continue;
        }
        let Ok(txt) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(p) = serde_json::from_str::<ConsultPrompt>(&txt) else {
            continue;
        };
        if now - p.ts > PROMPT_TTL_SECS {
            let _ = std::fs::remove_file(&path); // abandoned — device never took it
            continue;
        }
        out.push(p);
    }
    out
}

/// Store a device's answer and clear the matching prompt — the muse reads the answer file.
pub fn store_answer(dir: &Path, answer: &ConsultAnswer) -> Result<()> {
    std::fs::create_dir_all(queue(dir)).map_err(|e| Error::Malformed(format!("consult: {e}")))?;
    // Refuse an answer for a prompt we never asked (no prompt file for that id).
    let prompt_path = queue(dir).join(format!("{}.prompt.json", answer.id));
    if !prompt_path.exists() {
        return Err(Error::Untrusted("consult: answer for an unknown prompt".into()));
    }
    let path = queue(dir).join(format!("{}.answer.json", answer.id));
    let body = serde_json::to_vec(answer).map_err(|e| Error::Malformed(format!("consult: {e}")))?;
    std::fs::write(path, body).map_err(|e| Error::Malformed(format!("consult: {e}")))?;
    let _ = std::fs::remove_file(prompt_path);
    Ok(())
}

/// Read the answer to a prompt, if one has arrived.
pub fn answer_of(dir: &Path, id: &str) -> Option<ConsultAnswer> {
    let path = queue(dir).join(format!("{id}.answer.json"));
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

/// Accept a signed pull: verify the puller is a member, then hand back the pending prompts. It
/// admits nothing and reveals no secret — just the questions waiting for a device.
pub fn accept_pull(dir: &Path, pull: &ConsultPull, now: i64) -> Result<Vec<ConsultPrompt>> {
    group::verify_membership_consistent(&pull.membership, &pull.group_pubkey, now)
        .map_err(|_| Error::Untrusted("consult: membership does not verify".into()))?;
    Ok(pending(dir, now))
}

/// Accept a signed answer: verify the pusher is a member and that it answers for its own node, then
/// store it against the prompt it names.
pub fn accept_answer(dir: &Path, report: &ConsultAnswerReport, now: i64) -> Result<()> {
    group::verify_membership_consistent(&report.membership, &report.group_pubkey, now)
        .map_err(|_| Error::Untrusted("consult: membership does not verify".into()))?;
    if report.answer.node_id != report.membership.node_id {
        return Err(Error::Untrusted(
            "consult: answer node_id does not match the pushing member".into(),
        ));
    }
    store_answer(dir, &report.answer)
}

/// Which member a relayed prompt came from, kept beside it as `<id>.origin`.
///
/// A sidecar rather than a field on [`ConsultPrompt`] for two reasons: the prompt is served verbatim
/// to devices, and routing is nobody's business but the broker's; and [`store_answer`] deletes the
/// prompt file when the answer lands, so a field there would vanish exactly when the broker still
/// needs to know where to send the answer.
fn origin_path(dir: &Path, id: &str) -> std::path::PathBuf {
    queue(dir).join(format!("{id}.origin"))
}

fn read_origin(dir: &Path, id: &str) -> Option<String> {
    std::fs::read_to_string(origin_path(dir, id))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Ids of everything in the queue with the given extension.
fn ids_with_suffix(dir: &Path, suffix: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(queue(dir)) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.strip_suffix(suffix).map(|s| s.to_string())
        })
        .collect()
}

/// Broker a hub's consults (ADR-0014). Verifies the relaying member, parks its pending prompts in
/// this node's own queue — so the ordinary `/mesh/consult` pull serves them to devices with no
/// device-side change at all — and returns the answers gathered for that hub.
///
/// The hub's list is authoritative for its own prompts, which keeps both sides converging without
/// any extra bookkeeping:
///   * a prompt already answered here is **not** re-parked — otherwise the hub, which keeps listing
///     it until it receives the answer, would have it answered again on every round;
///   * a prompt the hub no longer lists is finished (answered and stored there, or abandoned), so its
///     prompt, answer and origin marker are all dropped — the hub's silence *is* the acknowledgement.
///
/// Only prompts this broker is holding **for that same origin** are ever touched. The broker's own
/// local consults carry no origin marker and are invisible to this path.
pub fn accept_relay(dir: &Path, relay: &ConsultRelay, now: i64) -> Result<Vec<ConsultAnswer>> {
    group::verify_membership_consistent(&relay.membership, &relay.group_pubkey, now)
        .map_err(|_| Error::Untrusted("consult: relay membership does not verify".into()))?;
    let origin = relay.membership.node_id.clone();
    std::fs::create_dir_all(queue(dir)).map_err(|e| Error::Malformed(format!("consult: {e}")))?;

    let listed: std::collections::HashSet<&str> =
        relay.prompts.iter().map(|p| p.id.as_str()).collect();

    // Park anything new. An existing prompt file is left exactly as it is: rewriting it would reset
    // the timestamp the TTL sweep measures, and could swap the body under a device mid-pull.
    for p in &relay.prompts {
        if answer_of(dir, &p.id).is_some() {
            continue; // answered here already, waiting to be collected below
        }
        let path = queue(dir).join(format!("{}.prompt.json", p.id));
        if !path.exists() {
            let body =
                serde_json::to_vec(p).map_err(|e| Error::Malformed(format!("consult: {e}")))?;
            std::fs::write(&path, body).map_err(|e| Error::Malformed(format!("consult: {e}")))?;
        }
        let _ = std::fs::write(origin_path(dir, &p.id), &origin);
    }

    // Hand back this origin's answers, and retire whatever it has stopped asking about.
    let mut out = Vec::new();
    for id in ids_with_suffix(dir, ".origin") {
        if read_origin(dir, &id).as_deref() != Some(origin.as_str()) {
            continue;
        }
        if listed.contains(id.as_str()) {
            if let Some(a) = answer_of(dir, &id) {
                out.push(a);
            }
        } else {
            let _ = std::fs::remove_file(queue(dir).join(format!("{id}.prompt.json")));
            let _ = std::fs::remove_file(queue(dir).join(format!("{id}.answer.json")));
            let _ = std::fs::remove_file(origin_path(dir, &id));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::{create_group, DEFAULT_CERT_TTL_SECS};
    use crate::node::NodeKey;

    const NOW: i64 = 1_785_000_000;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("familiar_consult_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn queue_round_trips_and_expires() {
        let dir = tmp("rt");
        enqueue(&dir, "c1", "what is the weather theory?", "", NOW).unwrap();
        assert_eq!(pending(&dir, NOW).len(), 1);
        // An answer for the prompt clears it; a fresh read finds the answer.
        let ans = ConsultAnswer {
            id: "c1".into(),
            json: "{\"theory\":\"x\"}".into(),
            node_id: "node1".into(),
            ts: NOW,
        };
        store_answer(&dir, &ans).unwrap();
        assert_eq!(pending(&dir, NOW).len(), 0, "answered prompt is cleared");
        assert_eq!(answer_of(&dir, "c1").unwrap().json, "{\"theory\":\"x\"}");
        // Stale prompts expire on the next listing.
        enqueue(&dir, "c2", "q", "", NOW).unwrap();
        assert_eq!(pending(&dir, NOW + PROMPT_TTL_SECS + 1).len(), 0);
    }

    #[test]
    fn refuses_answer_for_an_unknown_prompt() {
        let dir = tmp("unknown");
        let ans = ConsultAnswer {
            id: "never-asked".into(),
            json: "{}".into(),
            node_id: "n".into(),
            ts: NOW,
        };
        assert!(store_answer(&dir, &ans).is_err());
    }

    #[test]
    fn accept_answer_checks_membership_and_own_node() {
        let home = tmp("home");
        let node = NodeKey::load_or_mint(&home, "n").unwrap();
        let cred = create_group(&home, &node, "TheRiver", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        enqueue(&home, "c9", "q", "", NOW).unwrap();
        // Answer claiming a different node_id than the membership is refused.
        let mut report = ConsultAnswerReport {
            membership: cred.membership.clone(),
            group_pubkey: cred.group_pubkey.clone(),
            answer: ConsultAnswer {
                id: "c9".into(),
                json: "{}".into(),
                node_id: "someone-else".into(),
                ts: NOW,
            },
            nonce: "x".into(),
            ts: NOW,
        };
        assert!(accept_answer(&home, &report, NOW).is_err());
        // Corrected to its own node: accepted, prompt cleared.
        report.answer.node_id = node.node_id();
        accept_answer(&home, &report, NOW).unwrap();
        assert_eq!(pending(&home, NOW).len(), 0);
    }

    /// The whole broker lifecycle, in the order it actually happens across gossip rounds.
    #[test]
    fn broker_relays_a_consult_and_returns_the_answer_once() {
        let hub = tmp("relay_hub");
        let broker = tmp("relay_broker");
        let node = NodeKey::load_or_mint(&hub, "n").unwrap();
        let cred = create_group(&hub, &node, "TheRiver", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        let relay_of = |prompts: Vec<ConsultPrompt>| ConsultRelay {
            membership: cred.membership.clone(),
            group_pubkey: cred.group_pubkey.clone(),
            prompts,
            nonce: "n1".into(),
            ts: NOW,
        };

        // Round 1 — the hub relays a pending prompt; the broker parks it where its own
        // /mesh/consult pull will serve it, and has nothing to hand back yet.
        let p = enqueue(&hub, "c1", "a question", "theory", NOW).unwrap();
        assert!(accept_relay(&broker, &relay_of(vec![p.clone()]), NOW)
            .unwrap()
            .is_empty());
        assert_eq!(pending(&broker, NOW).len(), 1, "device can now see it");

        // A device pulls from the broker and answers there.
        let ans = ConsultAnswer {
            id: "c1".into(),
            json: "{\"theory\":\"x\"}".into(),
            node_id: "device".into(),
            ts: NOW,
        };
        store_answer(&broker, &ans).unwrap();

        // Round 2 — the hub still lists c1 (it has not got the answer yet). The broker must NOT
        // re-park it, or the device would answer the same prompt every round; it returns the answer.
        let got = accept_relay(&broker, &relay_of(vec![p.clone()]), NOW).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].json, "{\"theory\":\"x\"}");
        assert_eq!(
            pending(&broker, NOW).len(),
            0,
            "an answered prompt is not offered to devices again"
        );

        // The hub stores it, which clears its own prompt.
        store_answer(&hub, &got[0]).unwrap();
        assert_eq!(pending(&hub, NOW).len(), 0);

        // Round 3 — the hub no longer lists c1. That silence is the acknowledgement: the broker
        // retires the answer and its origin marker, and stops handing it back.
        assert!(accept_relay(&broker, &relay_of(vec![]), NOW)
            .unwrap()
            .is_empty());
        assert!(answer_of(&broker, "c1").is_none());
        assert!(read_origin(&broker, "c1").is_none());
    }

    #[test]
    fn broker_abandons_a_prompt_the_hub_gave_up_on() {
        let hub = tmp("relay_giveup");
        let broker = tmp("relay_giveup_b");
        let node = NodeKey::load_or_mint(&hub, "n").unwrap();
        let cred = create_group(&hub, &node, "TheRiver", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        let p = enqueue(&hub, "c2", "q", "", NOW).unwrap();
        let mut relay = ConsultRelay {
            membership: cred.membership.clone(),
            group_pubkey: cred.group_pubkey.clone(),
            prompts: vec![p],
            nonce: "n".into(),
            ts: NOW,
        };
        accept_relay(&broker, &relay, NOW).unwrap();
        assert_eq!(pending(&broker, NOW).len(), 1);
        // The hub timed out and dropped it — the broker must not keep offering it to devices.
        relay.prompts.clear();
        accept_relay(&broker, &relay, NOW).unwrap();
        assert_eq!(pending(&broker, NOW).len(), 0);
    }

    /// A relay must never disturb consults the broker queued for itself.
    #[test]
    fn broker_never_touches_its_own_local_consults() {
        let broker = tmp("relay_local");
        let hub = tmp("relay_local_hub");
        let node = NodeKey::load_or_mint(&hub, "n").unwrap();
        let cred = create_group(&hub, &node, "TheRiver", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        enqueue(&broker, "mine", "the broker's own question", "", NOW).unwrap();
        let relay = ConsultRelay {
            membership: cred.membership,
            group_pubkey: cred.group_pubkey,
            prompts: vec![],
            nonce: "n".into(),
            ts: NOW,
        };
        accept_relay(&broker, &relay, NOW).unwrap();
        assert_eq!(
            pending(&broker, NOW).len(),
            1,
            "a local consult has no origin marker and is not the relay's business"
        );
    }
}
