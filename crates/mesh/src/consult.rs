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
}
