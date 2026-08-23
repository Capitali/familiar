//! The narrow bridge (ADR-0045 §2–§4). Crossings are typed envelopes carrying
//! provenance — never records, never paths.
//!
//! Outward (ship → household): [`AttentionNotice`]. What the household keeps of one is a
//! [`BridgeReceipt`] — instance, event id, kind, time. A receipt is citable for
//! "commissioned ship X reported low water at T" (authorship, time, delivery are real
//! events); the PAYLOAD never becomes household truth, never touches a theory, dossier,
//! presence, capacity, or service signal. The receipt type cannot carry the payload,
//! which is how that stays true without a discipline anyone has to remember.
//!
//! Inward (household → ship): exactly the human acts that create and end the
//! relationship — [`ControlEnvelope`]. The decoder refuses anything else; a household
//! observation shaped like an envelope fails to parse. A fresh Purr is unaware of the
//! household; it is not unrevocable.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::instance::{self, Lifecycle};
use crate::Error;

/// Where the household appends bridge receipts, relative to its data dir.
pub const RECEIPTS_FILE: &str = "worlds/receipts.jsonl";

/// Every crossing names where it came from and under what authority.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// The `WorldInstance` id from the provisioning registry.
    pub instance: String,
    /// Hex ed25519 public key the envelope claims to be signed with. Must match the
    /// registered instance key — a claim, checked, never trusted.
    pub source_key: String,
    /// The authority epoch this envelope was sent under. An older epoch than the
    /// registry's is a revoked voice and is refused.
    pub grant_epoch: u64,
    /// Envelope schema version, for honest evolution.
    pub schema_version: u32,
    /// Unique per event — the dedup and supersession handle.
    pub event_id: String,
}

/// Ship → household: bounded attention, not data ("low stores", "trade completed").
/// Payload fields stay small and typed; none of them ever lands in a household store.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttentionNotice {
    pub provenance: Provenance,
    /// What kind of attention ("low_stores", "trade_completed"). Routed on, never parsed
    /// into household reasoning.
    pub kind: String,
    /// The captain-facing line. Household code may deliver it; only a receipt survives.
    pub headline: String,
    pub observed_at: i64,
    pub sent_at: i64,
    /// After this, the notice is dead and refused on arrival.
    pub expires_at: i64,
    /// Event id this notice replaces, if any.
    #[serde(default)]
    pub supersedes: Option<String>,
}

/// Household → ship: the control plane, and only the control plane. `deny_unknown_fields`
/// plus the tagged enum means an envelope that is not one of these five human acts —
/// an observation, a dossier line, anything — fails to decode at the door.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "act", deny_unknown_fields)]
pub enum ControlEnvelope {
    /// The birth act: everything a fresh ship process needs, and nothing about the
    /// household. Carries the constitution hash so the Laws keep one source — the ship
    /// fails commissioning on mismatch (§7).
    CommissioningBundle {
        instance: String,
        label: String,
        commissioner: String,
        constitution_sha256: String,
        schema_version: u32,
    },
    /// A grant beginning, changing, or ending (ADR-0044 machinery when it arrives).
    GrantUpdate {
        instance: String,
        grant_epoch: u64,
        active_grants: Vec<String>,
    },
    /// The root boundary narrowed; the ship's lease is now stale and must be refreshed.
    BoundaryNarrowed { instance: String, grant_epoch: u64 },
    /// The human corrected the cosmetic label.
    Rename { instance: String, label: String },
    /// Authority ends now. The store's fate is a separate human retention act.
    Decommission { instance: String, grant_epoch: u64 },
}

/// Decode an inbound envelope. This is the ONLY door into a ship process from the
/// household side, and it accepts exactly the five control acts.
pub fn control_from_slice(bytes: &[u8]) -> Result<ControlEnvelope, Error> {
    serde_json::from_slice(bytes)
        .map_err(|e| Error::Refused(format!("not a control-plane act: {e}")))
}

/// What the household keeps of an outward crossing: authorship, time, delivery. There is
/// deliberately no payload field of any kind on this type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BridgeReceipt {
    pub instance: String,
    pub event_id: String,
    pub kind: String,
    pub received_at: i64,
}

fn receipts_path(dir: &Path) -> PathBuf {
    dir.join(RECEIPTS_FILE)
}

pub fn load_receipts(dir: &Path) -> Result<Vec<BridgeReceipt>, Error> {
    let raw = match fs::read_to_string(receipts_path(dir)) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        out.push(
            serde_json::from_str(line)
                .map_err(|e| Error::Refused(format!("receipt log is malformed: {e}")))?,
        );
    }
    Ok(out)
}

/// Receive one outward notice at the household bridge: verify the instance is
/// commissioned, the epoch current, the signature real (over the exact bytes sent), the
/// notice unexpired and unseen — then append the RECEIPT and hand the typed notice back
/// to the caller for captain-console delivery. The notice itself is never written to any
/// household store by this crate, and the receipt cannot carry it.
pub fn receive_notice(
    household_dir: &Path,
    raw: &[u8],
    sig_hex: &str,
    now: i64,
) -> Result<(AttentionNotice, BridgeReceipt), Error> {
    let notice: AttentionNotice = serde_json::from_slice(raw)
        .map_err(|e| Error::Refused(format!("not an attention notice: {e}")))?;
    let p = &notice.provenance;

    let w = instance::find(household_dir, &p.instance)?
        .ok_or_else(|| Error::Refused(format!("unknown world instance {}", p.instance)))?;
    if w.lifecycle != Lifecycle::Commissioned {
        return Err(Error::Refused("instance is decommissioned".into()));
    }
    if p.grant_epoch != w.grant_epoch {
        return Err(Error::Refused(
            "stale grant epoch — authority has moved".into(),
        ));
    }
    if p.source_key != w.instance_pubkey {
        return Err(Error::Refused(
            "source key is not the commissioned key".into(),
        ));
    }
    // Verify the signature over the exact bytes that crossed, with the registry's key.
    let identity = familiar_mesh::node::NodeIdentity {
        node_id: String::new(),
        pubkey: w.instance_pubkey.clone(),
        label: w.label.clone(),
    };
    identity
        .verify(raw, sig_hex)
        .map_err(|e| Error::Refused(format!("signature: {e}")))?;

    if now > notice.expires_at {
        return Err(Error::Refused("notice expired before delivery".into()));
    }
    if p.event_id.trim().is_empty() {
        return Err(Error::Refused("notice carries no event id".into()));
    }
    let seen = load_receipts(household_dir)?;
    if seen
        .iter()
        .any(|r| r.instance == p.instance && r.event_id == p.event_id)
    {
        return Err(Error::Refused(format!(
            "event {} already received",
            p.event_id
        )));
    }

    let receipt = BridgeReceipt {
        instance: p.instance.clone(),
        event_id: p.event_id.clone(),
        kind: notice.kind.clone(),
        received_at: now,
    };
    let path = receipts_path(household_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let mut line = serde_json::to_vec(&receipt).map_err(|e| Error::Io(e.to_string()))?;
    line.push(b'\n');
    f.write_all(&line)?;
    Ok((notice, receipt))
}
