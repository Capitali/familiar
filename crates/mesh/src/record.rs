//! The membership record and the rules engine — ADR-0026's two-filter admission.
//!
//! One [`MembershipRecord`] per device is the sole answer to *may this thing be here, and what
//! may it see?* A device becomes a member **automatically**, the moment two filters both hold:
//!
//! 1. **The device contract** — the covenant handshake ([`crate::enroll`]): a self-certifying
//!    identity signing an attestation of the Three Laws.
//! 2. **The human identity is established** — by *evidence*, never by assertion. A typed claim
//!    addresses; evidence establishes. The four classes are below.
//!
//! Until both hold the device is a guest reading the projection ([`crate::standing`]); nothing
//! here is approved by anybody. The engine is one pure function, [`evaluate_admission`], so the
//! rules can be tested class by class — including the two negatives ADR-0026 marks
//! non-negotiable: a typed claim to an existing handle does not admit, and a pure remote
//! stranger does not admit.
//!
//! Phase 1 of the rebuild builds the evidence paths and this engine. The door itself
//! (`/mesh/enroll-request` minting on evidence instead of on sight) swaps over in Phase 3, and
//! the record replaces the legacy stores (`granted/`, `standing.json`, `revoked.json`, …) in
//! the Phase 2 migration.

use crate::group::Membership;
use crate::node::{fingerprint, NodeKey};
use crate::{exactly_32, hex_decode, hex_encode, Error, Result};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Where device records live, one JSON file per `device_id`.
pub const RECORDS_DIR: &str = "mesh/records";
/// Spent invite tokens — a token id that has a file here has been used.
const SPENT_INVITES_DIR: &str = "mesh/spent_invites";

/// How long an invite token lives. Ten minutes: long enough to walk a phone across a room and
/// fumble a download, short enough that a leaked token is almost certainly already dead.
pub const INVITE_TOKEN_TTL_SECS: i64 = 10 * 60;

// ---- the record ---------------------------------------------------------------------

/// Where a device stands. `Guest` is the stable resting state, not a queue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecordState {
    Guest,
    Member,
    Severed { reason: String, at: i64 },
}

/// What a device *says* about who it serves. Addresses only; admits nothing (ADR-0019 as
/// amended: a claim addresses; establishment admits).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityClaim {
    pub handle: String,
    pub ts: i64,
}

/// The four ways an identity is actually established — each either cryptographic continuity or
/// a deliberate human act. `Migration` marks records folded in from the pre-record stores
/// (Phase 2), so "how was this established?" stays answerable even about the past.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceClass {
    RotationProof,
    DeviceVoucher,
    InviteToken,
    LocalIntroduction,
    Migration,
}

/// The fact that filter 2 held: which handle, by what class, on what artifact, when. The
/// artifact is kept so a `Disestablish` correction can hold out the *same* evidence for the
/// cool-off window rather than letting it be replayed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Establishment {
    pub handle: String,
    pub class: EvidenceClass,
    /// The evidence artifact — a signature, a token id, a content hash — never free text.
    pub artifact: String,
    pub at: i64,
}

/// Claim and establishment, side by side. "Someone says they're Betty" is worth displaying;
/// only `established` opens anything.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct IdentityStatus {
    #[serde(default)]
    pub claim: Option<IdentityClaim>,
    #[serde(default)]
    pub established: Option<Establishment>,
}

/// The signed fact of an admission: which door's rules engine minted, when, on what evidence.
/// Automatic never means anonymous — every admission is attributable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionFact {
    /// node_id of the door whose rules engine admitted.
    pub minted_by: String,
    pub at: i64,
    pub evidence: EvidenceClass,
    pub artifact: String,
}

/// A deliberate reversal. Corrections live on the roster card and the CLI, never the welcome
/// screen; transport (`POST /mesh/correct`) is Phase 3 work — the type and its merge live here
/// so the record's shape is complete from the start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Correction {
    pub act: CorrectionAct,
    pub subject_device: String,
    /// node_id of the member device that corrected.
    pub corrected_by: String,
    /// "that's not Betty" — the reason travels with the act.
    pub reason: String,
    pub ts: i64,
    pub nonce: String,
    /// Signature by the correcting member's node key (verified at the transport seam).
    pub sig: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrectionAct {
    Sever,
    Disestablish,
    Hold,
    Restore,
}

/// One record per device — the only answer to any membership question (ADR-0026 rule 3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MembershipRecord {
    /// The durable thing (ADR-0025). Until device identity fully lands in Phase 2, this is the
    /// device's current node_id — the migration re-keys it.
    pub device_id: String,
    /// node_ids of the keys this device has held, current first. Extending it takes an E1
    /// rotation proof.
    pub keys: Vec<String>,
    pub state: RecordState,
    #[serde(default)]
    pub identity: IdentityStatus,
    #[serde(default)]
    pub admitted: Option<AdmissionFact>,
    #[serde(default)]
    pub held_until: Option<i64>,
    #[serde(default)]
    pub corrections: Vec<Correction>,
    /// RETAINED — a node can be held to what it accepted. (The legacy path deleted it.)
    #[serde(default)]
    pub attestation: Option<crate::enroll::Attestation>,
    /// Free text carried over from the standing roll's notes ("ian's iPad") and editable later —
    /// so the record explains itself a year from now, exactly as the roll's notes did.
    #[serde(default)]
    pub note: String,
    pub first_seen: i64,
    pub last_seen: i64,
}

impl MembershipRecord {
    /// A fresh guest record: the knock succeeded (contract attested), nothing established.
    pub fn guest(
        device_id: &str,
        node_id: &str,
        attestation: crate::enroll::Attestation,
        now: i64,
    ) -> Self {
        MembershipRecord {
            device_id: device_id.to_string(),
            keys: vec![node_id.to_string()],
            state: RecordState::Guest,
            identity: IdentityStatus::default(),
            admitted: None,
            held_until: None,
            corrections: Vec::new(),
            attestation: Some(attestation),
            note: String::new(),
            first_seen: now,
            last_seen: now,
        }
    }

    /// Which filters are still unmet — what the client shows as the path to admission.
    /// Empty ⇒ both hold.
    pub fn missing_filters(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.attestation.is_none() {
            out.push("covenant");
        }
        if self.identity.established.is_none() {
            out.push("identity");
        }
        out
    }
}

// ---- merge: records replicate -------------------------------------------------------

/// Merge two replicas of the same device's record. [`AdmissionFact`]s merge **earliest-wins**
/// (idempotent — every door runs the same rules, so a second admission is the same fact);
/// corrections merge as a **union**, and the *latest* correction wins the derived state (a
/// correction is a deliberate reversal, not a race). Commutative and idempotent, so replicas
/// converge no matter the exchange order.
pub fn merge_records(a: &MembershipRecord, b: &MembershipRecord) -> MembershipRecord {
    debug_assert_eq!(a.device_id, b.device_id, "merge is per-device");

    let admitted = match (&a.admitted, &b.admitted) {
        (Some(x), Some(y)) => {
            // Earliest wins; ties break on the door's id so the pick is deterministic, not
            // whichever replica we happened to fold first.
            if (x.at, &x.minted_by) <= (y.at, &y.minted_by) {
                Some(x.clone())
            } else {
                Some(y.clone())
            }
        }
        (Some(x), None) => Some(x.clone()),
        (None, y) => y.clone(),
    };

    let established = match (&a.identity.established, &b.identity.established) {
        (Some(x), Some(y)) => {
            if (x.at, &x.artifact) <= (y.at, &y.artifact) {
                Some(x.clone())
            } else {
                Some(y.clone())
            }
        }
        (Some(x), None) => Some(x.clone()),
        (None, y) => y.clone(),
    };
    let claim = match (&a.identity.claim, &b.identity.claim) {
        (Some(x), Some(y)) => Some(if (x.ts, &x.handle) >= (y.ts, &y.handle) {
            x.clone()
        } else {
            y.clone()
        }),
        (Some(x), None) => Some(x.clone()),
        (None, y) => y.clone(),
    };

    let mut corrections = a.corrections.clone();
    for c in &b.corrections {
        if !corrections.contains(c) {
            corrections.push(c.clone());
        }
    }
    corrections.sort_by(|x, y| (x.ts, &x.nonce).cmp(&(y.ts, &y.nonce)));

    let mut keys = a.keys.clone();
    for k in &b.keys {
        if !keys.contains(k) {
            keys.push(k.clone());
        }
    }

    let mut merged = MembershipRecord {
        device_id: a.device_id.clone(),
        keys,
        state: RecordState::Guest, // derived below
        identity: IdentityStatus { claim, established },
        admitted,
        held_until: match (a.held_until, b.held_until) {
            (Some(x), Some(y)) => Some(x.max(y)),
            (x, y) => x.or(y),
        },
        corrections,
        attestation: a.attestation.clone().or_else(|| b.attestation.clone()),
        note: if a.note.is_empty() {
            b.note.clone()
        } else {
            a.note.clone()
        },
        first_seen: a.first_seen.min(b.first_seen),
        last_seen: a.last_seen.max(b.last_seen),
    };
    merged.state = derive_state(&merged);
    merged
}

/// The state a record's facts add up to. Admission facts and establishment say *member*;
/// the latest correction can say otherwise. State is derived, never voted on.
pub fn derive_state(r: &MembershipRecord) -> RecordState {
    let base = if r.admitted.is_some() && r.identity.established.is_some() {
        RecordState::Member
    } else {
        RecordState::Guest
    };
    // Corrections are sorted by ts on merge; the latest deliberate act wins.
    match r.corrections.last() {
        Some(c) if c.act == CorrectionAct::Sever => RecordState::Severed {
            reason: c.reason.clone(),
            at: c.ts,
        },
        Some(c) if c.act == CorrectionAct::Disestablish => RecordState::Guest,
        _ => base,
    }
}

// ---- the evidence classes -----------------------------------------------------------

/// **E1** — the device's previous key signs its next one (ADR-0025). Same physical device, so
/// the established human link carries over; a reinstall stops minting a ghost.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationProof {
    pub device_id: String,
    /// The key being rotated away from — must belong to an established device.
    pub old_node_id: String,
    /// The key being rotated to (hex, 32 bytes) — the one now knocking.
    pub new_pubkey: String,
    pub ts: i64,
    /// ed25519 by the OLD key over the canonical body.
    pub sig: String,
}

#[derive(Serialize)]
struct RotationBody<'a> {
    device_id: &'a str,
    old_node_id: &'a str,
    new_pubkey: &'a str,
    ts: i64,
}

impl RotationProof {
    /// Mint on the old device while it still holds its key — at handoff or before a planned
    /// reinstall (the proof also rides Keychain-style backups).
    pub fn mint(old: &NodeKey, device_id: &str, new_pubkey: &str, ts: i64) -> Result<Self> {
        let body = serde_json::to_vec(&RotationBody {
            device_id,
            old_node_id: &old.node_id(),
            new_pubkey,
            ts,
        })?;
        Ok(RotationProof {
            device_id: device_id.to_string(),
            old_node_id: old.node_id(),
            new_pubkey: new_pubkey.to_string(),
            ts,
            sig: old.sign(&body),
        })
    }

    fn verify(&self, old_pubkey_hex: &str) -> Result<()> {
        let body = serde_json::to_vec(&RotationBody {
            device_id: &self.device_id,
            old_node_id: &self.old_node_id,
            new_pubkey: &self.new_pubkey,
            ts: self.ts,
        })?;
        verify_hex_sig(old_pubkey_hex, &body, &self.sig, "rotation proof")
    }
}

/// **E2** — a device already bound to the claimed handle vouches for a new one, out of a
/// deliberate physical act (scanning the handoff code on the old device; the phone→watch link).
/// The claimed human's own hardware is the second person.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceVoucher {
    /// The human whose hardware vouches.
    pub handle: String,
    /// The new device's key (hex, 32 bytes).
    pub subject_pubkey: String,
    /// node_id of the vouching device — looked up in the mesh's own establishment table, never
    /// trusted from the payload.
    pub voucher_node_id: String,
    pub ts: i64,
    pub nonce: String,
    /// ed25519 by the vouching device's key over the canonical body.
    pub sig: String,
}

#[derive(Serialize)]
struct VoucherBody<'a> {
    handle: &'a str,
    subject_pubkey: &'a str,
    voucher_node_id: &'a str,
    ts: i64,
    nonce: &'a str,
}

impl DeviceVoucher {
    /// Mint on the established device, at the moment of the physical act.
    pub fn mint(
        voucher: &NodeKey,
        handle: &str,
        subject_pubkey: &str,
        ts: i64,
        nonce: &str,
    ) -> Result<Self> {
        let body = serde_json::to_vec(&VoucherBody {
            handle,
            subject_pubkey,
            voucher_node_id: &voucher.node_id(),
            ts,
            nonce,
        })?;
        Ok(DeviceVoucher {
            handle: handle.to_string(),
            subject_pubkey: subject_pubkey.to_string(),
            voucher_node_id: voucher.node_id(),
            ts,
            nonce: nonce.to_string(),
            sig: voucher.sign(&body),
        })
    }

    fn verify(&self, voucher_pubkey_hex: &str) -> Result<()> {
        let body = serde_json::to_vec(&VoucherBody {
            handle: &self.handle,
            subject_pubkey: &self.subject_pubkey,
            voucher_node_id: &self.voucher_node_id,
            ts: self.ts,
            nonce: &self.nonce,
        })?;
        verify_hex_sig(voucher_pubkey_hex, &body, &self.sig, "device voucher")
    }
}

/// **E3** — a member's deliberate act, displaced in time: a single-use, ten-minute token. It
/// carries the minting member's cert so ANY door can verify it is member-signed without holding
/// a roll — and it carries **no group secret**, unlike the invite payload it replaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteToken {
    /// Random id — also the single-use spend key.
    pub token_id: String,
    pub group_id: String,
    /// The minting member's certificate; proves the mint was a member's act.
    pub minted_by: Membership,
    /// Names who this invite is for; empty = the newcomer introduces their own (new) handle.
    pub expected_handle: String,
    pub issued: i64,
    pub expires: i64,
    /// ed25519 by the minting member's node key over the canonical body.
    pub sig: String,
}

#[derive(Serialize)]
struct InviteBody<'a> {
    token_id: &'a str,
    group_id: &'a str,
    minted_by_node: &'a str,
    expected_handle: &'a str,
    issued: i64,
    expires: i64,
}

/// Mint an invite token — any member may, on any node; it is their deliberate act that
/// establishes, whenever and wherever the token is later presented.
pub fn mint_invite_token(
    node: &NodeKey,
    membership: &Membership,
    expected_handle: &str,
    now: i64,
) -> Result<InviteToken> {
    let token_id = hex_encode(&crate::os_random::<16>()?);
    let issued = now;
    let expires = now + INVITE_TOKEN_TTL_SECS;
    let body = serde_json::to_vec(&InviteBody {
        token_id: &token_id,
        group_id: &membership.group_id,
        minted_by_node: &membership.node_id,
        expected_handle,
        issued,
        expires,
    })?;
    Ok(InviteToken {
        token_id,
        group_id: membership.group_id.clone(),
        minted_by: membership.clone(),
        expected_handle: expected_handle.to_string(),
        issued,
        expires,
        sig: node.sign(&body),
    })
}

impl InviteToken {
    fn verify(&self, group_pubkey_hex: &str, group_id: &str, now: i64) -> Result<()> {
        if now >= self.expires {
            return Err(Error::Untrusted("invite: expired — ask for a fresh one".into()));
        }
        if self.group_id != group_id || self.minted_by.group_id != group_id {
            return Err(Error::Untrusted("invite: wrong mesh".into()));
        }
        // The minter must be a verifiable member of this group…
        crate::group::verify_membership_consistent(&self.minted_by, group_pubkey_hex, now)?;
        // …and must actually have signed this token with the key its cert certifies.
        let body = serde_json::to_vec(&InviteBody {
            token_id: &self.token_id,
            group_id: &self.group_id,
            minted_by_node: &self.minted_by.node_id,
            expected_handle: &self.expected_handle,
            issued: self.issued,
            expires: self.expires,
        })?;
        verify_hex_sig(&self.minted_by.node_pubkey, &body, &self.sig, "invite token")
    }
}

/// Spend a token id — **atomically single-use** per door (`create_new` is the lock). The door
/// spends *before* minting: a spent-but-failed admission wastes a token (cheap to re-mint); the
/// other order would let one token admit twice.
pub fn spend_invite(dir: &Path, token_id: &str) -> Result<()> {
    let d = dir.join(SPENT_INVITES_DIR);
    std::fs::create_dir_all(&d)?;
    // Token ids are minted hex; refuse anything path-shaped rather than sanitize it.
    if token_id.is_empty() || !token_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::Malformed("invite: token id is not hex".into()));
    }
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(d.join(token_id))
    {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(Error::Untrusted(
            "invite: already used — each token admits once".into(),
        )),
        Err(e) => Err(e.into()),
    }
}

/// **E4** — the introduce-yourself interaction: name entry, face, voice. Establishes only with
/// provenance — made in a place the mesh actually inhabits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Introduction {
    pub handle: String,
    /// The human's own words, kept as color; the artifact is a hash, never this text.
    pub statement: String,
    pub ts: i64,
}

/// Where an introduction was made — supplied by the door from what it actually observed about
/// the connection, never by the introducer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Provenance {
    /// Arrived on a network where an established member device is colocated.
    MemberColocatedNetwork,
    /// Made on a device that is itself established (the shared iPad case — the device is the
    /// provenance; disclosure keeps following the device, ADR-0005).
    EstablishedDevice { device_node_id: String },
    /// The founding act: a one-node mesh introducing its founder.
    Founding,
    /// Rendezvous-only, relay-only, nowhere in particular. Establishes nothing.
    Remote,
}

// ---- the rules engine ---------------------------------------------------------------

/// The evidence a knocking device presents for filter 2.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "class", rename_all = "snake_case")]
pub enum Evidence {
    Rotation(RotationProof),
    Voucher(DeviceVoucher),
    Invite(InviteToken),
    Introduction {
        intro: Introduction,
        provenance: Provenance,
    },
}

/// A device the mesh already holds established: its current key, and whose it is.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstablishedDeviceRef {
    pub node_id: String,
    pub pubkey: String,
    pub handle: String,
}

/// What the door knows when the rules run. Assembled by the door from its own record store —
/// nothing in it comes from the knocking device.
pub struct AdmissionContext<'a> {
    pub now: i64,
    pub group_id: &'a str,
    pub group_pubkey: &'a str,
    /// Every established (member) device this door knows: the lookup table for E1/E2, and the
    /// existing-handle guardrail for E4.
    pub established: &'a [EstablishedDeviceRef],
}

impl AdmissionContext<'_> {
    fn device(&self, node_id: &str) -> Option<&EstablishedDeviceRef> {
        self.established.iter().find(|d| d.node_id == node_id)
    }
    fn handle_exists(&self, handle: &str) -> bool {
        self.established
            .iter()
            .any(|d| d.handle.eq_ignore_ascii_case(handle))
    }
}

/// The knocking device, as the door verified it at the covenant handshake.
pub struct Subject<'a> {
    pub node_id: &'a str,
    /// Whether the covenant attestation verified — filter 1. The engine re-checks it so the
    /// two-filter rule lives in one place, not spread across call sites.
    pub covenant_attested: bool,
}

/// **The rules engine.** One pure function over (subject, claim, evidence, context):
/// both filters hold ⇒ `Ok(Establishment)` and the door mints; anything else is an error
/// naming what is missing — which is exactly the text the guest's console shows.
///
/// The guardrails are rules, not judgements:
/// - an introduction (E4) can never claim an existing handle — that takes E1/E2, or an E3
///   naming it. Typing "I am Betty" cannot become Betty.
/// - an introduction with `Remote` provenance establishes nothing. A pure remote stranger
///   stays a guest, told plainly what would work: an invite, a handoff, or being in the room.
///
/// Token spending is IO and stays outside ([`spend_invite`]); the door's order is
/// evaluate → spend → mint.
pub fn evaluate_admission(
    subject: &Subject,
    claim: Option<&IdentityClaim>,
    evidence: &Evidence,
    ctx: &AdmissionContext,
) -> Result<Establishment> {
    if !subject.covenant_attested {
        return Err(Error::Untrusted(
            "covenant not attested — the device contract is the first filter".into(),
        ));
    }

    match evidence {
        Evidence::Rotation(p) => {
            let old = ctx.device(&p.old_node_id).ok_or_else(|| {
                Error::Untrusted("rotation: the previous key is not an established member".into())
            })?;
            p.verify(&old.pubkey)?;
            let new_pk = exactly_32(&hex_decode(&p.new_pubkey)?, "rotation new pubkey")?;
            if fingerprint(&new_pk) != subject.node_id {
                return Err(Error::Untrusted(
                    "rotation: proof names a different key than the one knocking".into(),
                ));
            }
            Ok(Establishment {
                handle: old.handle.clone(),
                class: EvidenceClass::RotationProof,
                artifact: p.sig.clone(),
                at: ctx.now,
            })
        }

        Evidence::Voucher(v) => {
            let voucher = ctx.device(&v.voucher_node_id).ok_or_else(|| {
                Error::Untrusted("voucher: the vouching device is not an established member".into())
            })?;
            if !voucher.handle.eq_ignore_ascii_case(&v.handle) {
                return Err(Error::Untrusted(
                    "voucher: that device does not belong to the handle it vouches for".into(),
                ));
            }
            v.verify(&voucher.pubkey)?;
            let subj_pk = exactly_32(&hex_decode(&v.subject_pubkey)?, "voucher subject pubkey")?;
            if fingerprint(&subj_pk) != subject.node_id {
                return Err(Error::Untrusted(
                    "voucher: vouches for a different key than the one knocking".into(),
                ));
            }
            Ok(Establishment {
                handle: voucher.handle.clone(),
                class: EvidenceClass::DeviceVoucher,
                artifact: v.sig.clone(),
                at: ctx.now,
            })
        }

        Evidence::Invite(t) => {
            t.verify(ctx.group_pubkey, ctx.group_id, ctx.now)?;
            let handle = if !t.expected_handle.trim().is_empty() {
                // The inviter named who this is for; their deliberate act covers that handle,
                // existing or new.
                t.expected_handle.trim().to_string()
            } else {
                let claimed = claim
                    .map(|c| c.handle.trim())
                    .filter(|h| !h.is_empty())
                    .ok_or_else(|| {
                        Error::Untrusted(
                            "invite: an unnamed invite still needs you to say who you are".into(),
                        )
                    })?;
                // An unnamed invite establishes a NEW human. Attaching to an existing one takes
                // the inviter naming it — or a handoff/voucher from that human's own device.
                if ctx.handle_exists(claimed) {
                    return Err(Error::Untrusted(format!(
                        "invite: “{claimed}” already exists here — ask for an invite naming \
                         that handle, or hand off from one of their devices"
                    )));
                }
                claimed.to_string()
            };
            Ok(Establishment {
                handle,
                class: EvidenceClass::InviteToken,
                artifact: t.token_id.clone(),
                at: ctx.now,
            })
        }

        Evidence::Introduction { intro, provenance } => {
            if *provenance == Provenance::Remote {
                return Err(Error::Untrusted(
                    "introduction: made from nowhere the mesh inhabits — use an invite, hand \
                     off from an established device, or introduce yourself on the mesh's own \
                     network"
                        .into(),
                ));
            }
            let handle = intro.handle.trim();
            if handle.is_empty() {
                return Err(Error::Untrusted(
                    "introduction: a name is the least an introduction carries".into(),
                ));
            }
            if ctx.handle_exists(handle) {
                return Err(Error::Untrusted(format!(
                    "introduction: “{handle}” already exists here — an introduction never \
                     attaches to an existing identity; that takes a handoff, a voucher, or an \
                     invite naming it"
                )));
            }
            // If the device provenance names a device, it must actually be established.
            if let Provenance::EstablishedDevice { device_node_id } = provenance {
                if ctx.device(device_node_id).is_none() {
                    return Err(Error::Untrusted(
                        "introduction: the introducing device is not established".into(),
                    ));
                }
            }
            let artifact = crate::sha256_hex(&serde_json::to_vec(&(intro, provenance))?);
            Ok(Establishment {
                handle: handle.to_string(),
                class: EvidenceClass::LocalIntroduction,
                artifact,
                at: ctx.now,
            })
        }
    }
}

// ---- record store -------------------------------------------------------------------

/// Persist a record (one file per device under [`RECORDS_DIR`]).
pub fn save(dir: &Path, record: &MembershipRecord) -> Result<()> {
    let d = dir.join(RECORDS_DIR);
    std::fs::create_dir_all(&d)?;
    std::fs::write(
        d.join(format!("{}.json", record.device_id)),
        serde_json::to_vec_pretty(record)?,
    )?;
    Ok(())
}

/// Load one device's record.
pub fn load(dir: &Path, device_id: &str) -> Result<Option<MembershipRecord>> {
    match std::fs::read_to_string(dir.join(RECORDS_DIR).join(format!("{device_id}.json"))) {
        Ok(s) => Ok(Some(serde_json::from_str(&s)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Every record this node holds.
pub fn load_all(dir: &Path) -> Vec<MembershipRecord> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir.join(RECORDS_DIR)) {
        for e in entries.flatten() {
            if let Ok(s) = std::fs::read_to_string(e.path()) {
                if let Ok(r) = serde_json::from_str::<MembershipRecord>(&s) {
                    out.push(r);
                }
            }
        }
    }
    out.sort_by(|a, b| a.device_id.cmp(&b.device_id));
    out
}

/// Find the record that holds `node_id` — as its device_id (the Phase 2 window keys records by
/// current node_id) or anywhere in its key history.
pub fn find_by_key(dir: &Path, node_id: &str) -> Option<MembershipRecord> {
    if let Ok(Some(r)) = load(dir, node_id) {
        return Some(r);
    }
    load_all(dir)
        .into_iter()
        .find(|r| r.keys.iter().any(|k| k == node_id))
}

// ---- dual-write seams + the ONE migration (Phase 2) ---------------------------------
//
// Until the Phase 3 door swap, the legacy stores stay authoritative and every legacy write
// also lands here, so the two answer sets can be compared (`doctor`) before the read flag
// (`config.read_records`) makes the record the answer. Rollback is the flag, not a restore.

/// Load-or-create by device_id, mutate, re-derive state, save.
fn upsert<F: FnOnce(&mut MembershipRecord)>(dir: &Path, device_id: &str, now: i64, f: F) -> Result<()> {
    let mut r = load(dir, device_id)?.unwrap_or(MembershipRecord {
        device_id: device_id.to_string(),
        keys: vec![device_id.to_string()],
        state: RecordState::Guest,
        identity: IdentityStatus::default(),
        admitted: None,
        held_until: None,
        corrections: Vec::new(),
        attestation: None,
        note: String::new(),
        first_seen: now,
        last_seen: now,
    });
    f(&mut r);
    r.last_seen = r.last_seen.max(now);
    r.state = derive_state(&r);
    save(dir, &r)
}

/// Dual-write from the legacy enrolment path: a grant was minted, so a record exists — and the
/// attestation is RETAINED here even though the legacy flow drops its pending record.
pub(crate) fn upsert_enrolled(
    dir: &Path,
    node_id: &str,
    attestation: Option<&crate::enroll::Attestation>,
    now: i64,
) -> Result<()> {
    upsert(dir, node_id, now, |r| {
        if r.attestation.is_none() {
            r.attestation = attestation.cloned();
        }
    })
}

/// Dual-write from the legacy standing roll: full standing maps to *established + admitted*,
/// class `Migration` — the migration era covers the dual-write window, and the roll's note
/// rides along so nothing the file used to explain goes silent.
pub(crate) fn record_standing_grant(
    dir: &Path,
    minted_by: &str,
    node_id: &str,
    note: &str,
    now: i64,
) -> Result<()> {
    upsert(dir, node_id, now, |r| {
        if r.identity.established.is_none() {
            r.identity.established = Some(Establishment {
                handle: String::new(), // the roll never named the human; a false name would be worse
                class: EvidenceClass::Migration,
                artifact: "standing-roll".into(),
                at: now,
            });
        }
        if r.admitted.is_none() {
            r.admitted = Some(AdmissionFact {
                minted_by: minted_by.to_string(),
                at: now,
                evidence: EvidenceClass::Migration,
                artifact: "standing-roll".into(),
            });
        }
        if r.note.is_empty() && !note.trim().is_empty() {
            r.note = note.trim().to_string();
        }
    })
}

/// Dual-write from the legacy standing revoke: back to guest — establishment cleared, the
/// admission fact kept as history (state derives from both).
pub(crate) fn record_standing_revoke(dir: &Path, node_id: &str, now: i64) -> Result<()> {
    upsert(dir, node_id, now, |r| {
        r.identity.established = None;
    })
}

/// Dual-write from a denial: the hold window on the record.
pub(crate) fn record_hold(dir: &Path, node_id: &str, until: i64, now: i64) -> Result<()> {
    upsert(dir, node_id, now, |r| {
        r.held_until = Some(r.held_until.unwrap_or(0).max(until));
    })
}

/// Dual-write from `allow_retry`: the hold is lifted.
pub(crate) fn clear_hold(dir: &Path, node_id: &str) -> Result<()> {
    if let Ok(Some(mut r)) = load(dir, node_id) {
        r.held_until = None;
        save(dir, &r)?;
    }
    Ok(())
}

// ---- the door's acts (Phase 3) ------------------------------------------------------

/// What a knocking device sends to `POST /mesh/introduce`: who it says it serves, and the
/// evidence. Signed over the raw body with the node key, like every mesh write.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntroduceRequest {
    pub node: crate::node::NodeIdentity,
    #[serde(default)]
    pub claim: Option<IdentityClaim>,
    pub evidence: Evidence,
    pub nonce: String,
    pub ts: i64,
}

/// The door's act after the rules engine says yes: claim, establishment and the signed
/// admission fact land on the record — and the legacy roll is dual-written so the doctor's
/// equality holds in both directions for as long as the window lasts.
pub fn admit(
    dir: &Path,
    node_id: &str,
    claim: Option<IdentityClaim>,
    est: Establishment,
    minted_by: &str,
    now: i64,
) -> Result<MembershipRecord> {
    let handle = est.handle.clone();
    let class = est.class;
    let artifact = est.artifact.clone();
    upsert(dir, node_id, now, |r| {
        if claim.is_some() {
            r.identity.claim = claim.clone();
        }
        if r.identity.established.is_none() {
            r.identity.established = Some(est);
        }
        if r.admitted.is_none() {
            r.admitted = Some(AdmissionFact {
                minted_by: minted_by.to_string(),
                at: now,
                evidence: class,
                artifact,
            });
        }
    })?;
    // Legacy mirror — the roll's note says how, so the file keeps explaining itself.
    let note = if handle.is_empty() {
        format!("established via {class:?}")
    } else {
        format!("{handle} — established via {class:?}")
    };
    let _ = crate::standing::grant(dir, node_id, &note);
    load(dir, node_id)?.ok_or_else(|| Error::Malformed("record vanished during admit".into()))
}

/// A correction traveling the mesh: the correcting member's cert + the act, signed over the
/// raw body with the key the cert certifies — the same proof shape as every other mesh write.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectionEnvelope {
    pub membership: Membership,
    /// The group's public key (hex) so any door verifies the cert without holding a roll.
    pub group_pubkey: String,
    pub correction: Correction,
}

impl CorrectionEnvelope {
    pub fn verify_sig(&self, raw: &[u8], sig_hex: &str) -> Result<()> {
        verify_hex_sig(&self.membership.node_pubkey, raw, sig_hex, "correction")
    }
}

/// Apply a correction to the subject's record (idempotent by nonce) and mirror it into the
/// legacy stores for the dual-write window. The caller has already verified signature and
/// membership at the wire seam — or IS the local human (CLI / console), who needs no proof.
pub fn apply_correction(dir: &Path, c: &Correction, now: i64) -> Result<MembershipRecord> {
    let subject = c.subject_device.trim();
    if subject.is_empty() {
        return Err(Error::Malformed("correction: empty subject".into()));
    }
    if subject == c.corrected_by {
        return Err(Error::Untrusted(
            "correction: a device may not correct itself".into(),
        ));
    }
    upsert(dir, subject, now, |r| {
        if !r.corrections.iter().any(|x| x.nonce == c.nonce) {
            r.corrections.push(c.clone());
            r.corrections.sort_by(|x, y| (x.ts, &x.nonce).cmp(&(y.ts, &y.nonce)));
        }
        if c.act == CorrectionAct::Disestablish {
            r.identity.established = None;
        }
        if c.act == CorrectionAct::Hold {
            r.held_until = Some(r.held_until.unwrap_or(0).max(c.ts + crate::enroll::DENY_RETRY_SECS));
        }
        if c.act == CorrectionAct::Restore {
            r.held_until = None;
        }
    })?;
    // Legacy mirrors, best-effort: the record is the authority; these keep the window honest.
    // The roll is edited DIRECTLY — `standing::revoke`'s own dual-write would clear the
    // record's establishment, and a sever must not erase facts (restore derives them back).
    let roll_remove = |subject: &str| {
        let mut roll = crate::standing::load(dir);
        let before = roll.full.len();
        roll.full.retain(|n| n != subject);
        if roll.full.len() != before {
            let _ = crate::standing::save(dir, &roll);
        }
    };
    match c.act {
        CorrectionAct::Sever => {
            roll_remove(subject);
            let mut revoked = crate::group::load_revoked(dir).unwrap_or_default();
            if !revoked.iter().any(|n| n == subject) {
                revoked.push(subject.to_string());
                let _ = crate::group::write_json_public(
                    &dir.join(crate::group::REVOKED_FILE),
                    &revoked,
                );
            }
        }
        CorrectionAct::Disestablish => {
            roll_remove(subject);
        }
        CorrectionAct::Hold => {
            let _ = crate::enroll::deny(dir, subject, c.ts);
        }
        CorrectionAct::Restore => {
            let _ = crate::enroll::allow_retry(dir, subject);
            let revoked = crate::group::load_revoked(dir).unwrap_or_default();
            if revoked.iter().any(|n| n == subject) {
                let remaining: Vec<String> =
                    revoked.into_iter().filter(|n| n != subject).collect();
                let _ = crate::group::write_json_public(
                    &dir.join(crate::group::REVOKED_FILE),
                    &remaining,
                );
            }
            // If the facts still say member, the roll says so again too.
            if let Ok(Some(r)) = load(dir, subject) {
                if derive_state(&r) == RecordState::Member
                    && !crate::standing::load(dir).full.iter().any(|n| n == subject)
                {
                    let mut roll = crate::standing::load(dir);
                    roll.full.push(subject.to_string());
                    let _ = crate::standing::save(dir, &roll);
                }
            }
        }
    }
    load(dir, subject)?.ok_or_else(|| Error::Malformed("record vanished during correction".into()))
}

/// What the migration folded, for the operator to read back.
#[derive(Debug, Default, Serialize)]
pub struct MigrationReport {
    pub records: usize,
    pub from_granted: usize,
    pub from_pending: usize,
    pub from_peers: usize,
    pub established_from_roll: usize,
    pub severed_from_revoked: usize,
    pub held_from_denials: usize,
}

/// **The ONE migration** (ADR-0026 / Phase 2). Folds every legacy membership store into
/// `mesh/records/`:
///
/// - `mesh/granted/*.json` → a record per grant (attestation is gone for old grants — the
///   legacy flow deleted it; the dual-write keeps it from now on)
/// - `mesh/pending/*.json` → a guest record WITH its attestation
/// - `mesh/peers.json`     → a record per peer (first/last seen carried over)
/// - `standing.json`       → established + admitted, class `Migration`, note preserved
/// - `mesh/revoked.json`   → a `Sever` correction (nonce-keyed, so re-running cannot stack)
/// - `mesh/denied/*.json`  → `held_until`
///
/// `mesh/candidates.json` is deliberately NOT folded: a candidate is a key passing through,
/// not a device joining (ADR-0025) — records minted from it would be exactly the ghosts this
/// rebuild exists to stop. The ledger keeps pruning itself and dies with the Phase 3 door.
///
/// Idempotent: upserts never overwrite an earlier establishment/admission, corrections dedupe
/// by nonce, and running it twice produces byte-identical records. The legacy stores are left
/// in place untouched — they stay authoritative until `read_records` flips, and that flag is
/// the rollback.
pub fn migrate(dir: &Path, now: i64) -> Result<MigrationReport> {
    let mut report = MigrationReport::default();
    let self_node = std::fs::read_to_string(dir.join(crate::node::NODE_FILE))
        .ok()
        .and_then(|s| serde_json::from_str::<crate::node::NodeIdentity>(&s).ok())
        .map(|n| n.node_id)
        .unwrap_or_else(|| "migration".into());

    for grant in crate::enroll::list_grants(dir) {
        let issued = grant.membership.issued;
        upsert(dir, &grant.membership.node_id, now, |r| {
            r.first_seen = r.first_seen.min(issued);
        })?;
        report.from_granted += 1;
    }

    for p in crate::enroll::list_pending(dir)? {
        let (received, att) = (p.received_at, p.attestation.clone());
        upsert(dir, &p.node.node_id, now, |r| {
            r.first_seen = r.first_seen.min(received);
            if r.attestation.is_none() {
                r.attestation = Some(att);
            }
        })?;
        report.from_pending += 1;
    }

    for peer in crate::transport::load_peers(dir) {
        let (first, last) = (peer.first_seen, peer.last_seen);
        upsert(dir, &peer.node_id, now, |r| {
            if first > 0 {
                r.first_seen = r.first_seen.min(first);
            }
            r.last_seen = r.last_seen.max(last);
        })?;
        report.from_peers += 1;
    }

    let roll = crate::standing::load(dir);
    for node_id in &roll.full {
        let note = roll.notes.get(node_id).map(String::as_str).unwrap_or("");
        record_standing_grant(dir, &self_node, node_id, note, now)?;
        report.established_from_roll += 1;
    }

    for node_id in crate::group::load_revoked(dir)? {
        let nonce = format!("migration-revoked-{node_id}");
        upsert(dir, &node_id, now, |r| {
            if !r.corrections.iter().any(|c| c.nonce == nonce) {
                r.corrections.push(Correction {
                    act: CorrectionAct::Sever,
                    subject_device: node_id.clone(),
                    corrected_by: self_node.clone(),
                    reason: "folded from mesh/revoked.json".into(),
                    ts: now,
                    nonce: nonce.clone(),
                    sig: String::new(),
                });
            }
        })?;
        report.severed_from_revoked += 1;
    }

    for d in crate::enroll::list_denials(dir) {
        record_hold(
            dir,
            &d.node_id,
            d.at + crate::enroll::DENY_RETRY_SECS,
            now,
        )?;
        report.held_from_denials += 1;
    }

    report.records = load_all(dir).len();
    Ok(report)
}

// ---- the doctor ---------------------------------------------------------------------

/// One node's answers, old store vs. record. `ok` means every answer agrees.
#[derive(Debug, Serialize)]
pub struct DoctorRow {
    pub node_id: String,
    pub legacy_standing: &'static str,
    pub record_standing: &'static str,
    pub legacy_revoked: bool,
    pub record_severed: bool,
    pub has_record: bool,
    pub ok: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct DoctorReport {
    pub rows: Vec<DoctorRow>,
    pub divergent: usize,
    /// Labels appearing in the peer roster under more than one key with a stale entry — the
    /// reinstall-ghost signature (three "iPhone"s, one alive). Candidates for `mesh abandon`.
    pub ghost_suspects: Vec<(String, Vec<String>)>,
    /// Size of the transient adoption ledger (not folded; dies with the Phase 3 door).
    pub candidates_pending: usize,
}

/// Compare every membership answer the legacy stores give against the record's answer — the
/// Phase 2 deploy gate: `read_records` flips only on a clean report, on the real nodes.
pub fn doctor(dir: &Path, now: i64) -> DoctorReport {
    let roll = crate::standing::load(dir);
    let revoked = crate::group::load_revoked(dir).unwrap_or_default();

    let mut ids: Vec<String> = Vec::new();
    let push = |id: &str, ids: &mut Vec<String>| {
        if !id.is_empty() && !ids.iter().any(|x| x == id) {
            ids.push(id.to_string());
        }
    };
    for g in crate::enroll::list_grants(dir) {
        push(&g.membership.node_id, &mut ids);
    }
    for p in crate::transport::load_peers(dir) {
        push(&p.node_id, &mut ids);
    }
    for n in roll.full.iter().chain(revoked.iter()) {
        push(n, &mut ids);
    }
    for r in load_all(dir) {
        push(&r.device_id, &mut ids);
        for k in &r.keys {
            push(k, &mut ids);
        }
    }
    ids.sort();

    let mut rows = Vec::new();
    let mut divergent = 0;
    for id in &ids {
        let legacy_full = roll.full.iter().any(|n| n == id);
        let legacy_revoked = revoked.iter().any(|n| n == id);
        let rec = find_by_key(dir, id);
        let (record_full, record_severed, has_record) = match &rec {
            Some(r) => match derive_state(r) {
                RecordState::Member => (true, false, true),
                RecordState::Severed { .. } => (false, true, true),
                RecordState::Guest => (false, false, true),
            },
            None => (false, false, false),
        };
        let ok = legacy_full == record_full && legacy_revoked == record_severed && has_record;
        if !ok {
            divergent += 1;
        }
        rows.push(DoctorRow {
            node_id: id.clone(),
            legacy_standing: if legacy_full { "full" } else { "guest" },
            record_standing: if record_full { "full" } else { "guest" },
            legacy_revoked,
            record_severed,
            has_record,
            ok,
        });
    }

    // Ghost signature: one label, several keys, at least one stale — reinstalls that each
    // minted a fresh key and stuck (the 2026-07-31 lighthouse roster, in miniature).
    let mut by_label: std::collections::BTreeMap<String, Vec<(String, i64)>> = Default::default();
    for p in crate::transport::load_peers(dir) {
        if p.status == "abandoned" {
            continue;
        }
        by_label
            .entry(p.label.clone())
            .or_default()
            .push((p.node_id.clone(), p.last_seen));
    }
    const STALE_GHOST_SECS: i64 = 7 * 24 * 60 * 60;
    let ghost_suspects = by_label
        .into_iter()
        .filter(|(_, v)| v.len() > 1 && v.iter().any(|(_, seen)| now - seen > STALE_GHOST_SECS))
        .map(|(label, v)| (label, v.into_iter().map(|(id, _)| id).collect()))
        .collect();

    let candidates_pending = std::fs::read_to_string(dir.join("mesh/candidates.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<std::collections::BTreeMap<String, i64>>(&s).ok())
        .map(|m| m.len())
        .unwrap_or(0);

    DoctorReport {
        rows,
        divergent,
        ghost_suspects,
        candidates_pending,
    }
}

fn verify_hex_sig(pubkey_hex: &str, body: &[u8], sig_hex: &str, what: &str) -> Result<()> {
    let pk = exactly_32(&hex_decode(pubkey_hex)?, "pubkey")?;
    let key = VerifyingKey::from_bytes(&pk)
        .map_err(|_| Error::Untrusted(format!("{what}: bad pubkey")))?;
    let sig = crate::node::exactly_64(&hex_decode(sig_hex)?, "sig")?;
    key.verify(body, &Signature::from_bytes(&sig))
        .map_err(|_| Error::Untrusted(format!("{what}: signature did not verify")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::{self, DEFAULT_CERT_TTL_SECS};
    use crate::node::NodeKey;
    use std::path::PathBuf;

    const NOW: i64 = 3_000_000;

    fn fresh(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("familiar_record_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn key(tag: &str) -> NodeKey {
        NodeKey::load_or_mint(&fresh(tag), tag).unwrap()
    }

    /// A mesh with one established device: Betty's iPad.
    fn ctx_with<'a>(established: &'a [EstablishedDeviceRef], gid: &'a str, gpk: &'a str) -> AdmissionContext<'a> {
        AdmissionContext {
            now: NOW,
            group_id: gid,
            group_pubkey: gpk,
            established,
        }
    }

    fn betty_ipad(tag: &str) -> (NodeKey, Vec<EstablishedDeviceRef>) {
        let k = key(tag);
        let id = k.identity();
        let est = vec![EstablishedDeviceRef {
            node_id: id.node_id,
            pubkey: id.pubkey,
            handle: "betty".into(),
        }];
        (k, est)
    }

    fn subject<'a>(node_id: &'a str) -> Subject<'a> {
        Subject {
            node_id,
            covenant_attested: true,
        }
    }

    // ---- E1: rotation ----

    #[test]
    fn e1_a_rotation_proof_carries_the_established_identity_across_a_reinstall() {
        let (old, est) = betty_ipad("e1_old");
        let new = key("e1_new");
        let proof =
            RotationProof::mint(&old, "device-ipad", &new.identity().pubkey, NOW).unwrap();
        let ctx = ctx_with(&est, "g", "unused");
        let e = evaluate_admission(
            &subject(&new.node_id()),
            None,
            &Evidence::Rotation(proof),
            &ctx,
        )
        .unwrap();
        assert_eq!(e.handle, "betty");
        assert_eq!(e.class, EvidenceClass::RotationProof);
    }

    #[test]
    fn e1_rejects_an_unestablished_previous_key_a_forged_sig_and_a_swapped_key() {
        let (old, est) = betty_ipad("e1_neg_old");
        let new = key("e1_neg_new");
        let stranger = key("e1_neg_stranger");
        let ctx = ctx_with(&est, "g", "unused");

        // Previous key nobody established → nothing to inherit.
        let ghost = RotationProof::mint(&stranger, "d", &new.identity().pubkey, NOW).unwrap();
        assert!(evaluate_admission(
            &subject(&new.node_id()),
            None,
            &Evidence::Rotation(ghost),
            &ctx
        )
        .is_err());

        // Forged signature.
        let mut forged =
            RotationProof::mint(&old, "d", &new.identity().pubkey, NOW).unwrap();
        forged.sig = stranger.sign(b"not the body");
        assert!(evaluate_admission(
            &subject(&new.node_id()),
            None,
            &Evidence::Rotation(forged),
            &ctx
        )
        .is_err());

        // A proof for one key presented by another — the knocking key must be the named one.
        let proof = RotationProof::mint(&old, "d", &new.identity().pubkey, NOW).unwrap();
        assert!(evaluate_admission(
            &subject(&stranger.node_id()),
            None,
            &Evidence::Rotation(proof),
            &ctx
        )
        .is_err());
    }

    // ---- E2: voucher ----

    #[test]
    fn e2_a_voucher_from_the_humans_own_device_establishes_the_new_one() {
        let (ipad, est) = betty_ipad("e2_ipad");
        let phone = key("e2_phone");
        let v =
            DeviceVoucher::mint(&ipad, "betty", &phone.identity().pubkey, NOW, "n1").unwrap();
        let ctx = ctx_with(&est, "g", "unused");
        let e = evaluate_admission(
            &subject(&phone.node_id()),
            None,
            &Evidence::Voucher(v),
            &ctx,
        )
        .unwrap();
        assert_eq!(e.handle, "betty");
        assert_eq!(e.class, EvidenceClass::DeviceVoucher);
    }

    #[test]
    fn e2_rejects_a_voucher_for_someone_elses_handle_and_a_forged_one() {
        let (ipad, est) = betty_ipad("e2_neg_ipad");
        let phone = key("e2_neg_phone");
        let ctx = ctx_with(&est, "g", "unused");

        // Betty's iPad cannot vouch anyone in as "ian".
        let wrong =
            DeviceVoucher::mint(&ipad, "ian", &phone.identity().pubkey, NOW, "n1").unwrap();
        assert!(evaluate_admission(
            &subject(&phone.node_id()),
            None,
            &Evidence::Voucher(wrong),
            &ctx
        )
        .is_err());

        // A voucher whose signature is not the vouching device's.
        let stranger = key("e2_neg_stranger");
        let mut forged =
            DeviceVoucher::mint(&ipad, "betty", &phone.identity().pubkey, NOW, "n2").unwrap();
        forged.sig = stranger.sign(b"garbage");
        assert!(evaluate_admission(
            &subject(&phone.node_id()),
            None,
            &Evidence::Voucher(forged),
            &ctx
        )
        .is_err());
    }

    // ---- E3: invite token ----

    /// A group whose member `ian` can mint tokens; returns (ian's key, his membership,
    /// group_id, group_pubkey).
    fn group_with_ian(tag: &str) -> (NodeKey, Membership, String, String) {
        let dir = fresh(tag);
        let ian = NodeKey::load_or_mint(&dir, "ian-mac").unwrap();
        let cred = group::create_group(&dir, &ian, "river", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        (
            ian,
            cred.membership.clone(),
            cred.group_id.clone(),
            cred.group_pubkey.clone(),
        )
    }

    #[test]
    fn e3_a_named_invite_admits_and_is_single_use() {
        let (ian, m, gid, gpk) = group_with_ian("e3_named");
        let newcomer = key("e3_newcomer");
        let t = mint_invite_token(&ian, &m, "jeff", NOW).unwrap();
        let ctx = ctx_with(&[], &gid, &gpk);
        let e = evaluate_admission(
            &subject(&newcomer.node_id()),
            None,
            &Evidence::Invite(t.clone()),
            &ctx,
        )
        .unwrap();
        assert_eq!(e.handle, "jeff");
        assert_eq!(e.artifact, t.token_id);

        // Single use: the second spend of the same token id fails, atomically.
        let door = fresh("e3_named_door");
        spend_invite(&door, &t.token_id).unwrap();
        assert!(matches!(
            spend_invite(&door, &t.token_id),
            Err(Error::Untrusted(_))
        ));
    }

    #[test]
    fn e3_expires_after_ten_minutes_and_refuses_a_non_member_mint() {
        let (ian, m, gid, gpk) = group_with_ian("e3_exp");
        let newcomer = key("e3_exp_newcomer");
        let t = mint_invite_token(&ian, &m, "jeff", NOW).unwrap();

        // Expired: the same token, presented eleven minutes later.
        let late = AdmissionContext {
            now: NOW + INVITE_TOKEN_TTL_SECS + 60,
            group_id: &gid,
            group_pubkey: &gpk,
            established: &[],
        };
        assert!(evaluate_admission(
            &subject(&newcomer.node_id()),
            None,
            &Evidence::Invite(t),
            &late
        )
        .is_err());

        // A stranger minting a token with someone else's cert: the signature is not the
        // certified key's, so it refuses.
        let stranger = key("e3_exp_stranger");
        let forged = mint_invite_token(&stranger, &m, "jeff", NOW).unwrap();
        let ctx = ctx_with(&[], &gid, &gpk);
        assert!(evaluate_admission(
            &subject(&newcomer.node_id()),
            None,
            &Evidence::Invite(forged),
            &ctx
        )
        .is_err());
    }

    #[test]
    fn e3_an_unnamed_invite_establishes_a_new_human_but_never_an_existing_one() {
        let (ian, m, gid, gpk) = group_with_ian("e3_unnamed");
        let newcomer = key("e3_unnamed_dev");
        let (_ipad, est) = betty_ipad("e3_unnamed_ipad");
        let ctx = ctx_with(&est, &gid, &gpk);

        // With a fresh handle: establishes.
        let t = mint_invite_token(&ian, &m, "", NOW).unwrap();
        let claim = IdentityClaim {
            handle: "jeff".into(),
            ts: NOW,
        };
        let e = evaluate_admission(
            &subject(&newcomer.node_id()),
            Some(&claim),
            &Evidence::Invite(t),
            &ctx,
        )
        .unwrap();
        assert_eq!(e.handle, "jeff");

        // Claiming Betty through an unnamed invite: refused — attaching to an existing human
        // takes the inviter naming it, or Betty's own hardware.
        let t2 = mint_invite_token(&ian, &m, "", NOW).unwrap();
        let betty_claim = IdentityClaim {
            handle: "Betty".into(),
            ts: NOW,
        };
        assert!(evaluate_admission(
            &subject(&newcomer.node_id()),
            Some(&betty_claim),
            &Evidence::Invite(t2),
            &ctx
        )
        .is_err());

        // And with no claim at all there is nothing to establish.
        let t3 = mint_invite_token(&ian, &m, "", NOW).unwrap();
        assert!(evaluate_admission(
            &subject(&newcomer.node_id()),
            None,
            &Evidence::Invite(t3),
            &ctx
        )
        .is_err());
    }

    // ---- E4: introduction ----

    #[test]
    fn e4_an_introduction_on_the_mesh_network_establishes_a_new_human() {
        let dev = key("e4_dev");
        let ctx = ctx_with(&[], "g", "unused");
        let e = evaluate_admission(
            &subject(&dev.node_id()),
            None,
            &Evidence::Introduction {
                intro: Introduction {
                    handle: "jeff".into(),
                    statement: "hi, I'm Jeff, Ian's friend off Wildhorse".into(),
                    ts: NOW,
                },
                provenance: Provenance::MemberColocatedNetwork,
            },
            &ctx,
        )
        .unwrap();
        assert_eq!(e.handle, "jeff");
        assert_eq!(e.class, EvidenceClass::LocalIntroduction);
    }

    #[test]
    fn e4_founding_establishes_the_founder() {
        let dev = key("e4_founder");
        let ctx = ctx_with(&[], "g", "unused");
        assert!(evaluate_admission(
            &subject(&dev.node_id()),
            None,
            &Evidence::Introduction {
                intro: Introduction {
                    handle: "ian".into(),
                    statement: "founding".into(),
                    ts: NOW,
                },
                provenance: Provenance::Founding,
            },
            &ctx
        )
        .is_ok());
    }

    /// **Non-negotiable negative 1** (ADR-0026 Phase 1d): a typed claim to an existing handle
    /// does not admit. Typing "I am Betty" cannot become Betty.
    #[test]
    fn negative_a_typed_claim_to_an_existing_handle_does_not_admit() {
        let (_ipad, est) = betty_ipad("neg1_ipad");
        let imposter = key("neg1_imposter");
        let ctx = ctx_with(&est, "g", "unused");
        let err = evaluate_admission(
            &subject(&imposter.node_id()),
            None,
            &Evidence::Introduction {
                intro: Introduction {
                    handle: "Betty".into(), // case-insensitive on purpose
                    statement: "I am Betty".into(),
                    ts: NOW,
                },
                provenance: Provenance::MemberColocatedNetwork,
            },
            &ctx,
        )
        .unwrap_err();
        assert!(matches!(err, Error::Untrusted(_)));
    }

    /// **Non-negotiable negative 2**: a rendezvous-only stranger with a typed name does not
    /// admit — they stay a guest, and the error names what would work.
    #[test]
    fn negative_a_remote_stranger_with_a_typed_name_does_not_admit() {
        let stranger = key("neg2_stranger");
        let ctx = ctx_with(&[], "g", "unused");
        let err = evaluate_admission(
            &subject(&stranger.node_id()),
            None,
            &Evidence::Introduction {
                intro: Introduction {
                    handle: "totallyreal".into(),
                    statement: "let me in".into(),
                    ts: NOW,
                },
                provenance: Provenance::Remote,
            },
            &ctx,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(matches!(err, Error::Untrusted(_)));
        assert!(
            msg.contains("invite") && msg.contains("network"),
            "the refusal must name the paths that WOULD work: {msg}"
        );
    }

    /// The first filter is checked in the same function — no evidence admits an unattested
    /// device.
    #[test]
    fn no_evidence_admits_a_device_that_never_attested_the_covenant() {
        let (ipad, est) = betty_ipad("filter1_ipad");
        let phone = key("filter1_phone");
        let v = DeviceVoucher::mint(&ipad, "betty", &phone.identity().pubkey, NOW, "n").unwrap();
        let ctx = ctx_with(&est, "g", "unused");
        let unattested = Subject {
            node_id: &phone.node_id(),
            covenant_attested: false,
        };
        assert!(evaluate_admission(&unattested, None, &Evidence::Voucher(v), &ctx).is_err());
    }

    // ---- merge ----

    fn base_record(tag: &str) -> MembershipRecord {
        MembershipRecord::guest(
            tag,
            tag,
            crate::enroll::Attestation {
                laws_version: 1,
                statement: "I accept the Three Laws.".into(),
                ts: NOW,
            },
            NOW,
        )
    }

    #[test]
    fn merge_is_commutative_idempotent_and_admission_facts_take_the_earliest() {
        let mut a = base_record("dev1");
        a.identity.established = Some(Establishment {
            handle: "jeff".into(),
            class: EvidenceClass::InviteToken,
            artifact: "t1".into(),
            at: NOW,
        });
        a.admitted = Some(AdmissionFact {
            minted_by: "door-a".into(),
            at: NOW + 5,
            evidence: EvidenceClass::InviteToken,
            artifact: "t1".into(),
        });
        a.last_seen = NOW + 5;

        let mut b = base_record("dev1");
        b.identity.established = a.identity.established.clone();
        b.admitted = Some(AdmissionFact {
            minted_by: "door-b".into(),
            at: NOW + 9, // the same rules ran later elsewhere
            evidence: EvidenceClass::InviteToken,
            artifact: "t1".into(),
        });
        b.last_seen = NOW + 9;

        let ab = merge_records(&a, &b);
        let ba = merge_records(&b, &a);
        assert_eq!(ab, ba, "merge must not depend on exchange order");
        assert_eq!(ab.admitted.as_ref().unwrap().minted_by, "door-a");
        assert_eq!(ab.state, RecordState::Member);
        assert_eq!(ab.last_seen, NOW + 9);
        assert_eq!(merge_records(&ab, &ab), ab, "idempotent");
    }

    #[test]
    fn merge_corrections_union_and_the_latest_deliberate_act_wins() {
        let mut a = base_record("dev2");
        a.identity.established = Some(Establishment {
            handle: "jeff".into(),
            class: EvidenceClass::LocalIntroduction,
            artifact: "x".into(),
            at: NOW,
        });
        a.admitted = Some(AdmissionFact {
            minted_by: "door".into(),
            at: NOW,
            evidence: EvidenceClass::LocalIntroduction,
            artifact: "x".into(),
        });
        let mut b = a.clone();

        // One replica saw a sever; the other, a later restore.
        a.corrections.push(Correction {
            act: CorrectionAct::Sever,
            subject_device: "dev2".into(),
            corrected_by: "ian-mac".into(),
            reason: "that's not Jeff".into(),
            ts: NOW + 10,
            nonce: "c1".into(),
            sig: String::new(),
        });
        b.corrections.push(Correction {
            act: CorrectionAct::Restore,
            subject_device: "dev2".into(),
            corrected_by: "ian-mac".into(),
            reason: "it was Jeff after all".into(),
            ts: NOW + 20,
            nonce: "c2".into(),
            sig: String::new(),
        });

        let merged = merge_records(&a, &b);
        assert_eq!(merged.corrections.len(), 2);
        assert_eq!(
            merged.state,
            RecordState::Member,
            "the later restore outranks the earlier sever"
        );

        // And the other order of history: sever last → severed, whichever replica folds first.
        let mut c = b.clone();
        c.corrections.push(Correction {
            act: CorrectionAct::Sever,
            subject_device: "dev2".into(),
            corrected_by: "ian-mac".into(),
            reason: "gone for good".into(),
            ts: NOW + 30,
            nonce: "c3".into(),
            sig: String::new(),
        });
        let m2 = merge_records(&a, &c);
        assert!(matches!(m2.state, RecordState::Severed { .. }));
        assert_eq!(merge_records(&a, &c), merge_records(&c, &a));
    }

    // ---- Phase 2: the migration, the doctor, the flag ----

    /// A legacy world in one dir: A granted + full standing, B pending, C denied, a hand-written
    /// revocation, and a peer row — then the records wiped, as on a fleet node that predates them.
    fn legacy_world(tag: &str) -> (PathBuf, NodeKey, NodeKey, NodeKey) {
        let host = fresh(&format!("legacy_{tag}"));
        let host_node = NodeKey::load_or_mint(&host, "host").unwrap();
        group::create_group(&host, &host_node, "river", NOW, DEFAULT_CERT_TTL_SECS).unwrap();

        let submit = |k: &NodeKey| {
            let req = crate::enroll::EnrollRequest {
                node: k.identity(),
                attestation: crate::enroll::Attestation {
                    laws_version: 1,
                    statement: "I accept the Three Laws.".into(),
                    ts: NOW,
                },
                nonce: format!("n-{}", k.node_id()),
                ts: NOW,
            };
            let raw = serde_json::to_vec(&req).unwrap();
            let sig = k.sign(&raw);
            crate::enroll::submit_request(&host, &raw, &sig, NOW).unwrap();
        };

        let a = key(&format!("{tag}_a"));
        let b = key(&format!("{tag}_b"));
        let c = key(&format!("{tag}_c"));
        submit(&a);
        crate::standing::grant(&host, &a.node_id(), "ian's iPad").unwrap();
        // B: a pending left behind by the OLD door (nothing files pendings any more) —
        // hand-written the way the old flow persisted it.
        let pend = crate::enroll::Pending {
            node: b.identity(),
            attestation: crate::enroll::Attestation {
                laws_version: 1,
                statement: "I accept the Three Laws.".into(),
                ts: NOW,
            },
            received_at: NOW - 50,
            code: b.node_id().chars().take(6).collect(),
        };
        std::fs::create_dir_all(host.join("mesh/pending")).unwrap();
        std::fs::write(
            host.join(format!("mesh/pending/{}.json", b.node_id())),
            serde_json::to_vec_pretty(&pend).unwrap(),
        )
        .unwrap();
        crate::enroll::deny(&host, &c.node_id(), NOW).unwrap();
        std::fs::write(
            host.join(crate::group::REVOKED_FILE),
            serde_json::to_vec(&vec!["deadcafe00001111".to_string()]).unwrap(),
        )
        .unwrap();
        std::fs::write(
            host.join(crate::transport::PEERS_FILE),
            serde_json::json!([{
                "node_id": "peerx11122233344", "label": "FamTalker01", "addr": "",
                "group_id": "", "last_seen": NOW, "tools_offered": 0,
                "patterns_offered": 0, "first_seen": NOW - 500
            }])
            .to_string(),
        )
        .unwrap();

        // The fleet's real nodes predate the record store: wipe what dual-writes created.
        let _ = std::fs::remove_dir_all(host.join(RECORDS_DIR));
        (host, a, b, c)
    }

    #[test]
    fn the_migration_folds_every_legacy_store_and_is_idempotent() {
        let (host, a, b, c) = legacy_world("fold");
        let report = migrate(&host, NOW + 100).unwrap();
        // Two on the roll: A (granted standing above) and the host itself (founding admits
        // the founder since the Phase 3 door).
        assert_eq!(report.established_from_roll, 2);
        assert_eq!(report.severed_from_revoked, 1);
        assert_eq!(report.held_from_denials, 1);
        assert!(report.from_granted >= 1 && report.from_pending >= 1 && report.from_peers >= 1);

        // A: full standing → member, note carried, admission attributable to the fold.
        let ra = load(&host, &a.node_id()).unwrap().unwrap();
        assert_eq!(derive_state(&ra), RecordState::Member);
        assert_eq!(ra.note, "ian's iPad");
        assert_eq!(
            ra.identity.established.as_ref().unwrap().class,
            EvidenceClass::Migration
        );

        // B: pending → guest WITH its attestation retained.
        let rb = load(&host, &b.node_id()).unwrap().unwrap();
        assert_eq!(derive_state(&rb), RecordState::Guest);
        assert!(rb.attestation.is_some(), "a pending's attestation must survive");

        // C: denied → held.
        let rc = load(&host, &c.node_id()).unwrap().unwrap();
        assert_eq!(rc.held_until, Some(NOW + crate::enroll::DENY_RETRY_SECS));

        // The hand-revoked node → severed; the peer row → a guest with its history.
        let rd = load(&host, "deadcafe00001111").unwrap().unwrap();
        assert!(matches!(derive_state(&rd), RecordState::Severed { .. }));
        let re = load(&host, "peerx11122233344").unwrap().unwrap();
        assert_eq!(re.first_seen, NOW - 500);

        // Idempotent: a second run at the same instant is byte-identical.
        let before: Vec<_> = load_all(&host);
        migrate(&host, NOW + 100).unwrap();
        assert_eq!(load_all(&host), before, "re-running the fold must change nothing");
    }

    #[test]
    fn the_doctor_passes_after_the_fold_and_catches_divergence_and_ghosts() {
        let (host, _a, _b, _c) = legacy_world("doctor");
        migrate(&host, NOW + 100).unwrap();
        let rep = doctor(&host, NOW + 100);
        assert!(!rep.rows.is_empty());
        assert_eq!(
            rep.divergent, 0,
            "after the fold every answer must agree: {:?}",
            rep.rows.iter().filter(|r| !r.ok).collect::<Vec<_>>()
        );

        // Tamper behind the record's back: a roll entry with no record → divergent, loudly.
        let mut roll = crate::standing::load(&host);
        roll.full.push("aaaa000011112222".into());
        crate::standing::save(&host, &roll).unwrap();
        assert!(doctor(&host, NOW + 100).divergent >= 1);

        // The ghost signature: one label under several keys, one of them long stale.
        std::fs::write(
            host.join(crate::transport::PEERS_FILE),
            serde_json::json!([
                {"node_id": "ghost111", "label": "iPhone", "addr": "", "group_id": "",
                 "last_seen": NOW - 9 * 24 * 3600, "tools_offered": 0, "patterns_offered": 0},
                {"node_id": "alive222", "label": "iPhone", "addr": "", "group_id": "",
                 "last_seen": NOW + 100, "tools_offered": 0, "patterns_offered": 0}
            ])
            .to_string(),
        )
        .unwrap();
        let rep2 = doctor(&host, NOW + 100);
        assert!(
            rep2.ghost_suspects.iter().any(|(l, ids)| l == "iPhone" && ids.len() == 2),
            "three iPhones one alive is the exact failure this flags"
        );
    }

    #[test]
    fn the_read_flag_flips_answers_to_the_record_and_back() {
        use crate::standing::{standing_of, Standing};
        let dirp = fresh("flag");
        crate::standing::grant(&dirp, "node-full-000001", "ian's mac").unwrap();
        assert_eq!(standing_of(&dirp, "node-full-000001"), Standing::Full);

        // Flag on: the record (dual-written by the grant) answers; the unknown stays a guest.
        std::fs::create_dir_all(dirp.join("mesh")).unwrap();
        std::fs::write(dirp.join(crate::config::CONFIG_FILE), r#"{"read_records":true}"#).unwrap();
        assert_eq!(standing_of(&dirp, "node-full-000001"), Standing::Full);
        assert_eq!(standing_of(&dirp, "total-stranger"), Standing::Guest);

        // No record at all under the flag → guest. Failing closed, same as a missing roll.
        let _ = std::fs::remove_dir_all(dirp.join(RECORDS_DIR));
        assert_eq!(standing_of(&dirp, "node-full-000001"), Standing::Guest);

        // Flag off: the roll answers again — that is the whole rollback.
        std::fs::write(dirp.join(crate::config::CONFIG_FILE), r#"{"read_records":false}"#).unwrap();
        assert_eq!(standing_of(&dirp, "node-full-000001"), Standing::Full);
    }

    #[test]
    fn a_record_names_its_missing_filters_and_round_trips_disk() {
        let dir = fresh("store");
        let mut r = base_record("dev3");
        assert_eq!(r.missing_filters(), vec!["identity"]);

        r.identity.claim = Some(IdentityClaim {
            handle: "jeff".into(),
            ts: NOW,
        });
        save(&dir, &r).unwrap();
        let loaded = load(&dir, "dev3").unwrap().unwrap();
        assert_eq!(loaded, r);
        assert_eq!(load_all(&dir).len(), 1);
        assert!(load(&dir, "nope").unwrap().is_none());
    }
}
