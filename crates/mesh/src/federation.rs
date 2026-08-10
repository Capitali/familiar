//! Federation — ADR-0033: **meshes are peers too.**
//!
//! A mesh is an entity: its group keypair (the trust anchor that already signs membership
//! certs) is the **mesh key**, and its label is its public **handle**. Federation begins the
//! way membership begins — with a deliberate act by someone already inside:
//!
//! 1. A member mints a **mesh invite** (same ten-minute, single-use, member-signed token
//!    shape as ADR-0026 E3, at mesh scale) and hands it to the other mesh's operator.
//! 2. The invited mesh's lighthouse redeems it at `POST /mesh/federate`: a signed
//!    **introduction** naming its handle, its mesh pubkey, its doors, and what it declares.
//! 3. The introduction lands as a pending sibling — a card on the welcome screen — and a
//!    member's tap (`/mesh/federate-welcome`) is the vouch. Never automatic.
//!
//! The result is a **sibling**: known, consented, never a member of this household. A sibling
//! reads the worldview at the sibling rung of the projection ladder ([`crate::standing`]) —
//! the guest projection plus our handle and what we declare. There is no path from sibling to
//! member; a mesh does not join a household, it stands beside one.
//!
//! On the redeeming side, pasting the invite *was* the human's deliberate act, so the
//! answering mesh's introduction is adopted as a sibling immediately; its reads of the
//! inviting mesh simply fail closed until the welcome tap happens there.

use crate::group::{GroupCredential, Membership};
use crate::node::NodeKey;
use crate::{hex_decode, hex_encode, Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Where sibling records live, one JSON file per sibling `group_id`.
pub const SIBLINGS_DIR: &str = "mesh/siblings";
/// Spent mesh-invite tokens — single-use, same mechanics as device invites.
const SPENT_MESH_INVITES_DIR: &str = "mesh/spent_mesh_invites";

/// Same TTL as a device invite: long enough to paste across a call, short enough that a
/// leaked token is almost certainly already dead.
pub const MESH_INVITE_TTL_SECS: i64 = crate::record::INVITE_TOKEN_TTL_SECS;

// ---- the invite ---------------------------------------------------------------------

/// A member's deliberate act that opens the door to another MESH. Carries the minting
/// member's cert (any door can verify member-signed without a roll), our mesh pubkey (so the
/// redeemer can verify our returned introduction), and our doors (so it knows where to
/// knock). No secrets ride in it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshInvite {
    /// Random id — also the single-use spend key.
    pub token_id: String,
    pub group_id: String,
    /// The inviting mesh's public key (hex) — what the redeemer pins our introduction to.
    pub group_pubkey: String,
    /// The inviting mesh's handle, so the redeemer's console can say who is inviting.
    pub handle: String,
    /// Addresses the inviting mesh's door answers at.
    pub hosts: Vec<String>,
    /// The minting member's certificate; proves the mint was a member's act.
    pub minted_by: Membership,
    pub issued: i64,
    pub expires: i64,
    /// ed25519 by the minting member's node key over the canonical body.
    pub sig: String,
}

#[derive(Serialize)]
struct MeshInviteBody<'a> {
    token_id: &'a str,
    group_id: &'a str,
    group_pubkey: &'a str,
    handle: &'a str,
    hosts: &'a [String],
    minted_by_node: &'a str,
    issued: i64,
    expires: i64,
}

/// Mint a mesh invite — any member may; it is their deliberate act that consents to the
/// introduction whenever the token is later presented.
pub fn mint_mesh_invite(
    node: &NodeKey,
    membership: &Membership,
    cred: &GroupCredential,
    hosts: Vec<String>,
    now: i64,
) -> Result<MeshInvite> {
    let token_id = hex_encode(&crate::os_random::<16>()?);
    let issued = now;
    let expires = now + MESH_INVITE_TTL_SECS;
    let body = serde_json::to_vec(&MeshInviteBody {
        token_id: &token_id,
        group_id: &membership.group_id,
        group_pubkey: &cred.group_pubkey,
        handle: &cred.label,
        hosts: &hosts,
        minted_by_node: &membership.node_id,
        issued,
        expires,
    })?;
    Ok(MeshInvite {
        token_id,
        group_id: membership.group_id.clone(),
        group_pubkey: cred.group_pubkey.clone(),
        handle: cred.label.clone(),
        hosts,
        minted_by: membership.clone(),
        issued,
        expires,
        sig: node.sign(&body),
    })
}

impl MeshInvite {
    /// Verify at the door it was minted for: unexpired, our mesh, a verifiable member's act.
    pub fn verify(&self, group_pubkey_hex: &str, group_id: &str, now: i64) -> Result<()> {
        if now >= self.expires {
            return Err(Error::Untrusted(
                "mesh invite: expired — ask for a fresh one".into(),
            ));
        }
        if self.group_id != group_id || self.minted_by.group_id != group_id {
            return Err(Error::Untrusted("mesh invite: wrong mesh".into()));
        }
        if self.group_pubkey != group_pubkey_hex {
            return Err(Error::Untrusted("mesh invite: wrong mesh key".into()));
        }
        crate::group::verify_membership_consistent(&self.minted_by, group_pubkey_hex, now)?;
        let body = serde_json::to_vec(&MeshInviteBody {
            token_id: &self.token_id,
            group_id: &self.group_id,
            group_pubkey: &self.group_pubkey,
            handle: &self.handle,
            hosts: &self.hosts,
            minted_by_node: &self.minted_by.node_id,
            issued: self.issued,
            expires: self.expires,
        })?;
        crate::record::verify_hex_sig(&self.minted_by.node_pubkey, &body, &self.sig, "mesh invite")
    }

    /// The pasteable form: hex(JSON), same convention as the device invite payload.
    pub fn encode(&self) -> Result<String> {
        Ok(hex_encode(&serde_json::to_vec(self)?))
    }

    pub fn decode(payload: &str) -> Result<MeshInvite> {
        let bytes = hex_decode(payload.trim())?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}

/// Spend a mesh-invite token — atomically single-use per door (`create_new` is the lock),
/// spent *before* adoption so one token can never introduce twice.
pub fn spend_mesh_invite(dir: &Path, token_id: &str) -> Result<()> {
    let d = dir.join(SPENT_MESH_INVITES_DIR);
    std::fs::create_dir_all(&d)?;
    if token_id.is_empty() || !token_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::Malformed("mesh invite: token id is not hex".into()));
    }
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(d.join(token_id))
    {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(Error::Untrusted(
            "mesh invite: already used — each token introduces once".into(),
        )),
        Err(e) => Err(e.into()),
    }
}

// ---- the introduction ---------------------------------------------------------------

/// What one mesh says to another, signed with its mesh key. Sharing is by declaration,
/// never by leakage: the declared areas and offered tools are what the mesh *chooses* to
/// say, and the location is self-declared or absent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshIntroduction {
    pub handle: String,
    pub group_id: String,
    /// The introducing mesh's public key (hex) — the introduction is signed by it, so the
    /// introduction proves possession; trust in *which* mesh comes from the invite.
    pub group_pubkey: String,
    /// Addresses this mesh's door answers at.
    pub hosts: Vec<String>,
    /// Self-declared location; 0,0 = chose not to declare.
    pub lat: f64,
    pub lon: f64,
    /// Areas of knowledge this mesh declares (ADR-0033 §4). Declaration, not inventory.
    pub declared_areas: Vec<String>,
    /// Tools this mesh offers. Empty until the exchange work lands; the field is the seam.
    pub offered_tools: Vec<String>,
    /// The invite token this introduction redeems (binds the intro to the consent act).
    pub token_id: String,
    pub ts: i64,
    /// ed25519 by the mesh (group) key over the canonical body.
    pub sig: String,
}

#[derive(Serialize)]
struct IntroBody<'a> {
    handle: &'a str,
    group_id: &'a str,
    group_pubkey: &'a str,
    hosts: &'a [String],
    lat: f64,
    lon: f64,
    declared_areas: &'a [String],
    offered_tools: &'a [String],
    token_id: &'a str,
    ts: i64,
}

impl MeshIntroduction {
    fn body(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(&IntroBody {
            handle: &self.handle,
            group_id: &self.group_id,
            group_pubkey: &self.group_pubkey,
            hosts: &self.hosts,
            lat: self.lat,
            lon: self.lon,
            declared_areas: &self.declared_areas,
            offered_tools: &self.offered_tools,
            token_id: &self.token_id,
            ts: self.ts,
        })?)
    }

    /// Signature check against the mesh key the introduction names. `expected_pubkey`, when
    /// known (from the invite we minted or the sibling record we hold), pins WHICH mesh —
    /// without it the check proves possession only.
    pub fn verify(&self, expected_pubkey: Option<&str>) -> Result<()> {
        if let Some(exp) = expected_pubkey {
            if exp != self.group_pubkey {
                return Err(Error::Untrusted(
                    "introduction: signed by a different mesh than expected".into(),
                ));
            }
        }
        if self.handle.trim().is_empty() {
            return Err(Error::Untrusted(
                "introduction: a mesh needs a handle".into(),
            ));
        }
        crate::record::verify_hex_sig(
            &self.group_pubkey,
            &self.body()?,
            &self.sig,
            "mesh introduction",
        )
    }
}

/// Build and sign THIS mesh's introduction. Requires the mesh key (`can_mint`) — a
/// covenant-joined device cannot speak as the mesh; the lighthouse (or a founding node) can.
pub fn our_introduction(
    dir: &Path,
    cred: &GroupCredential,
    hosts: Vec<String>,
    token_id: &str,
    now: i64,
) -> Result<MeshIntroduction> {
    let cfg = crate::config::load(dir).unwrap_or_default();
    // Self-declared location: the mesh's own geo seam, or an honest 0,0 (chose not to declare).
    let (lat, lon) = crate::transport::self_geo(dir).unwrap_or((0.0, 0.0));
    let mut intro = MeshIntroduction {
        handle: if cred.label.trim().is_empty() {
            "a mesh".into()
        } else {
            cred.label.clone()
        },
        group_id: cred.group_id.clone(),
        group_pubkey: cred.group_pubkey.clone(),
        hosts,
        lat,
        lon,
        declared_areas: cfg.declared_areas.clone(),
        offered_tools: Vec::new(),
        token_id: token_id.to_string(),
        ts: now,
        sig: String::new(),
    };
    intro.sig = cred.sign_as_mesh(&intro.body()?)?;
    Ok(intro)
}

// ---- the sibling record -------------------------------------------------------------

/// A federated mesh as this mesh knows it. `pending` awaits the member's welcome tap;
/// `sibling` reads at the sibling rung; `severed` is standing withdrawal, not attack —
/// the record stays, with its reason, so the past remains answerable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiblingRecord {
    pub handle: String,
    pub group_id: String,
    pub group_pubkey: String,
    pub hosts: Vec<String>,
    pub lat: f64,
    pub lon: f64,
    pub declared_areas: Vec<String>,
    pub offered_tools: Vec<String>,
    /// "pending" | "sibling" | "severed"
    pub state: String,
    /// The member whose tap welcomed (empty while pending).
    pub welcomed_by: String,
    pub note: String,
    pub first_seen: i64,
    pub last_seen: i64,
}

fn sibling_file(dir: &Path, group_id: &str) -> Result<std::path::PathBuf> {
    if group_id.is_empty()
        || !group_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(Error::Malformed(
            "sibling: group id is not a clean id".into(),
        ));
    }
    Ok(dir.join(SIBLINGS_DIR).join(format!("{group_id}.json")))
}

pub fn save_sibling(dir: &Path, s: &SiblingRecord) -> Result<()> {
    std::fs::create_dir_all(dir.join(SIBLINGS_DIR))?;
    std::fs::write(
        sibling_file(dir, &s.group_id)?,
        serde_json::to_vec_pretty(s)?,
    )?;
    Ok(())
}

pub fn load_sibling(dir: &Path, group_id: &str) -> Option<SiblingRecord> {
    let f = sibling_file(dir, group_id).ok()?;
    let s = std::fs::read_to_string(f).ok()?;
    serde_json::from_str(&s).ok()
}

/// Every sibling this mesh knows, pending and severed included; stable order.
pub fn load_siblings(dir: &Path) -> Vec<SiblingRecord> {
    let mut out: Vec<SiblingRecord> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir.join(SIBLINGS_DIR)) {
        for e in entries.flatten() {
            if let Ok(s) = std::fs::read_to_string(e.path()) {
                if let Ok(r) = serde_json::from_str::<SiblingRecord>(&s) {
                    out.push(r);
                }
            }
        }
    }
    out.sort_by(|a, b| a.group_id.cmp(&b.group_id));
    out
}

/// Resolve a human-typed reference to a sibling: the full group_id, a unique id prefix, or
/// the handle (case-insensitive, unique). The consoles show short ids; a human should be
/// able to act on what they can see.
pub fn resolve_sibling(dir: &Path, wanted: &str) -> Option<SiblingRecord> {
    let w = wanted.trim();
    if w.is_empty() {
        return None;
    }
    let all = load_siblings(dir);
    if let Some(s) = all.iter().find(|s| s.group_id == w) {
        return Some(s.clone());
    }
    let by_prefix: Vec<&SiblingRecord> = all.iter().filter(|s| s.group_id.starts_with(w)).collect();
    if by_prefix.len() == 1 {
        return Some(by_prefix[0].clone());
    }
    let wl = w.to_lowercase();
    let by_handle: Vec<&SiblingRecord> = all
        .iter()
        .filter(|s| s.handle.to_lowercase() == wl)
        .collect();
    if by_handle.len() == 1 {
        return Some(by_handle[0].clone());
    }
    None
}

/// The sibling that holds this mesh key, if it stands (state == "sibling") — the read-auth
/// lookup for `/mesh/worldview-sibling`. Fail closed: pending and severed do not read.
pub fn standing_sibling_by_pubkey(dir: &Path, group_pubkey: &str) -> Option<SiblingRecord> {
    if group_pubkey.is_empty() {
        return None;
    }
    load_siblings(dir)
        .into_iter()
        .find(|s| s.group_pubkey == group_pubkey && s.state == "sibling")
}

fn record_from_intro(intro: &MeshIntroduction, state: &str, now: i64) -> SiblingRecord {
    SiblingRecord {
        handle: intro.handle.clone(),
        group_id: intro.group_id.clone(),
        group_pubkey: intro.group_pubkey.clone(),
        hosts: intro.hosts.clone(),
        lat: intro.lat,
        lon: intro.lon,
        declared_areas: intro.declared_areas.clone(),
        offered_tools: intro.offered_tools.clone(),
        state: state.into(),
        welcomed_by: String::new(),
        note: String::new(),
        first_seen: now,
        last_seen: now,
    }
}

// ---- the door (inviting side) -------------------------------------------------------

/// What the redeeming lighthouse POSTs to `/mesh/federate`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederateRequest {
    /// The invite our member minted — presented back whole, so this door verifies the
    /// member's act without holding a mint ledger.
    pub invite: MeshInvite,
    /// The redeeming mesh's signed introduction.
    pub introduction: MeshIntroduction,
}

/// Receive an introduction at the inviting door: verify the invite (our mesh, member-signed,
/// unexpired), spend it, verify the introduction's mesh signature, store the sender as a
/// **pending** sibling, and answer with our own signed introduction. The welcome tap is a
/// separate, human act.
pub fn receive_introduction(
    dir: &Path,
    cred: &GroupCredential,
    req: &FederateRequest,
    our_hosts: Vec<String>,
    now: i64,
) -> Result<MeshIntroduction> {
    req.invite.verify(&cred.group_pubkey, &cred.group_id, now)?;
    if req.introduction.token_id != req.invite.token_id {
        return Err(Error::Untrusted(
            "federate: introduction does not redeem this invite".into(),
        ));
    }
    if req.introduction.group_id == cred.group_id {
        return Err(Error::Untrusted(
            "federate: a mesh cannot be its own sibling".into(),
        ));
    }
    req.introduction.verify(None)?;
    // Spend before adopting — a spent-but-failed introduction wastes a token (cheap to
    // re-mint); the other order would let one token introduce twice.
    spend_mesh_invite(dir, &req.invite.token_id)?;
    // Re-introduction of a KNOWN sibling refreshes its declaration but never resurrects a
    // severed one and never un-welcomes a standing one.
    let state = match load_sibling(dir, &req.introduction.group_id) {
        Some(existing) if existing.state == "severed" => {
            return Err(Error::Untrusted(
                "federate: this mesh's standing was withdrawn — a human must restore it".into(),
            ))
        }
        Some(existing) => existing.state,
        None => "pending".into(),
    };
    let mut rec = record_from_intro(&req.introduction, &state, now);
    if let Some(existing) = load_sibling(dir, &req.introduction.group_id) {
        rec.first_seen = existing.first_seen;
        rec.welcomed_by = existing.welcomed_by;
        rec.note = existing.note;
    }
    save_sibling(dir, &rec)?;
    let _ = familiar_kernel::observation::record(
        dir,
        familiar_kernel::observation::Observation::new(
            "mesh",
            "mesh_introduced",
            format!(
                "the mesh “{}” introduces itself — awaiting a member's welcome",
                rec.handle
            ),
            "mesh",
            "mesh",
            now,
            1.0,
        ),
    );
    our_introduction(dir, cred, our_hosts, &req.invite.token_id, now)
}

/// The member's tap: a pending sibling stands. Idempotent; welcoming a severed sibling is the
/// deliberate restore.
pub fn welcome_sibling(
    dir: &Path,
    group_id: &str,
    welcomed_by: &str,
    now: i64,
) -> Result<SiblingRecord> {
    let mut s = resolve_sibling(dir, group_id)
        .ok_or_else(|| Error::Untrusted("welcome: no such introduction".into()))?;
    if s.state != "sibling" {
        s.state = "sibling".into();
        s.welcomed_by = welcomed_by.to_string();
        s.last_seen = now;
        save_sibling(dir, &s)?;
        let _ = familiar_kernel::observation::record(
            dir,
            familiar_kernel::observation::Observation::new(
                "mesh",
                "mesh_welcomed",
                format!(
                    "the mesh “{}” stands as a sibling — welcomed by {welcomed_by}",
                    s.handle
                ),
                "mesh",
                "mesh",
                now,
                1.0,
            ),
        );
    }
    Ok(s)
}

/// Standing withdrawal — the record stays, with its reason (ADR-0033 §6).
pub fn sever_sibling(dir: &Path, group_id: &str, reason: &str, now: i64) -> Result<SiblingRecord> {
    let mut s = resolve_sibling(dir, group_id)
        .ok_or_else(|| Error::Untrusted("sever: no such sibling".into()))?;
    s.state = "severed".into();
    s.note = reason.to_string();
    s.last_seen = now;
    save_sibling(dir, &s)?;
    Ok(s)
}

// ---- the door (redeeming side) ------------------------------------------------------

/// Adopt the inviting mesh from its answered introduction. Pasting the invite was OUR
/// human's deliberate act, so the answering mesh stands as a sibling here immediately; its
/// door still holds us pending until a member there taps welcome.
pub fn adopt_answered_introduction(
    dir: &Path,
    invite: &MeshInvite,
    answer: &MeshIntroduction,
    now: i64,
) -> Result<SiblingRecord> {
    answer.verify(Some(&invite.group_pubkey))?;
    if answer.group_id != invite.group_id {
        return Err(Error::Untrusted(
            "federate: the answer names a different mesh than the invite".into(),
        ));
    }
    let mut rec = record_from_intro(answer, "sibling", now);
    rec.welcomed_by = "invite".into();
    // The answer's advertised doors first, then the invite's — the redemption that just
    // succeeded PROVED the invite's address reachable, which an interface-derived guess
    // (a LAN IP that doesn't route from here) is not.
    for h in &invite.hosts {
        if !rec.hosts.contains(h) {
            rec.hosts.push(h.clone());
        }
    }
    if let Some(existing) = load_sibling(dir, &rec.group_id) {
        rec.first_seen = existing.first_seen;
    }
    save_sibling(dir, &rec)?;
    Ok(rec)
}

// ---- the member's tap, travelling ---------------------------------------------------

/// Welcome a pending sibling, or sever a standing one — a member's deliberate act carried
/// over the mesh (the console has no data dir; the act must travel to the door). Same
/// signed-member envelope shape as a standing vote.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederateAct {
    pub membership: Membership,
    /// The group's public key (hex) — lets the door verify the cert without a roll; the door
    /// additionally requires it to be its OWN group's key.
    pub group_pubkey: String,
    /// The sibling mesh being decided about.
    pub subject_group_id: String,
    /// "welcome" | "sever"
    pub act: String,
    /// Required for sever (standing withdrawal keeps its reason); ignored for welcome.
    #[serde(default)]
    pub reason: String,
    pub nonce: String,
    pub ts: i64,
}

impl FederateAct {
    /// The member signed these exact bytes with the key its membership certifies.
    pub fn verify_sig(&self, raw: &[u8], sig_hex: &str) -> Result<()> {
        crate::record::verify_hex_sig(&self.membership.node_pubkey, raw, sig_hex, "federate act")
    }
}

// ---- the sibling read ---------------------------------------------------------------

/// A sibling mesh's worldview read — signed by its mesh key, verified against the sibling
/// record we hold. Same replay discipline as a member read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiblingViewRequest {
    pub group_id: String,
    pub group_pubkey: String,
    pub ts: i64,
    pub nonce: String,
    /// ed25519 by the mesh key over the canonical body (the request with `sig` empty).
    pub sig: String,
}

impl SiblingViewRequest {
    fn body(&self) -> Result<Vec<u8>> {
        let mut unsigned = self.clone();
        unsigned.sig = String::new();
        Ok(serde_json::to_vec(&unsigned)?)
    }

    pub fn sign(cred: &GroupCredential, now: i64) -> Result<SiblingViewRequest> {
        let mut req = SiblingViewRequest {
            group_id: cred.group_id.clone(),
            group_pubkey: cred.group_pubkey.clone(),
            ts: now,
            nonce: hex_encode(&crate::os_random::<16>()?),
            sig: String::new(),
        };
        req.sig = cred.sign_as_mesh(&req.body()?)?;
        Ok(req)
    }

    pub fn verify(&self) -> Result<()> {
        crate::record::verify_hex_sig(&self.group_pubkey, &self.body()?, &self.sig, "sibling read")
    }
}

// ---- the redeeming walk (blocking — the CLI path) -----------------------------------

fn split_host_port(addr: &str) -> (String, u16) {
    match addr.rsplit_once(':') {
        Some((h, p)) => match p.parse::<u16>() {
            Ok(port) => (h.to_string(), port),
            Err(_) => (addr.to_string(), 47_100),
        },
        None => (addr.to_string(), 47_100),
    }
}

/// Redeem a mesh invite against the inviting mesh's doors (the redeeming side of ADR-0033
/// §2): introduce ourselves at each address until one answers, then adopt the answer as a
/// standing sibling — pasting the invite was our human's deliberate act. Blocking; TLS.
pub fn federate_with(dir: &Path, invite_payload: &str) -> Result<SiblingRecord> {
    let invite = MeshInvite::decode(invite_payload)?;
    let Some(cred) = crate::group::load(dir).ok().flatten() else {
        return Err(Error::Untrusted("no group enrolled here".into()));
    };
    let now = crate::transport::now_secs();
    let intro = our_introduction(
        dir,
        &cred,
        crate::transport::federation_hosts(dir),
        &invite.token_id,
        now,
    )?;
    let body = serde_json::to_vec(&FederateRequest {
        invite: invite.clone(),
        introduction: intro,
    })?;
    let mut last_err = String::from("no hosts in the invite");
    for addr in &invite.hosts {
        let (host, port) = split_host_port(addr);
        match crate::enroll::http(
            &host,
            port,
            "POST",
            "/mesh/federate",
            &[("Content-Type", "application/json")],
            &body,
        ) {
            Ok((200, resp)) => {
                let answer: MeshIntroduction = serde_json::from_slice(&resp)?;
                return adopt_answered_introduction(dir, &invite, &answer, now);
            }
            Ok((code, resp)) => {
                last_err = format!("{addr}: HTTP {code} {}", String::from_utf8_lossy(&resp))
            }
            Err(e) => last_err = format!("{addr}: {e}"),
        }
    }
    Err(Error::Untrusted(format!("no door answered — {last_err}")))
}

/// Read a sibling mesh's worldview at the sibling rung — signed with OUR mesh key, presented
/// at THEIR door. Returns the raw worldview JSON (the caller renders). Blocking; TLS.
pub fn read_sibling_worldview(dir: &Path, group_id: &str) -> Result<String> {
    let sib = resolve_sibling(dir, group_id)
        .ok_or_else(|| Error::Untrusted("no such sibling here".into()))?;
    let Some(cred) = crate::group::load(dir).ok().flatten() else {
        return Err(Error::Untrusted("no group enrolled here".into()));
    };
    let req = SiblingViewRequest::sign(&cred, crate::transport::now_secs())?;
    let body = serde_json::to_vec(&req)?;
    let mut last_err = String::from("the sibling record lists no hosts");
    for addr in &sib.hosts {
        let (host, port) = split_host_port(addr);
        match crate::enroll::http(
            &host,
            port,
            "POST",
            "/mesh/worldview-sibling",
            &[("Content-Type", "application/json")],
            &body,
        ) {
            Ok((200, resp)) => {
                // A fresh sighting of the sibling's door, worth keeping.
                let mut s = sib.clone();
                s.last_seen = crate::transport::now_secs();
                let _ = save_sibling(dir, &s);
                return Ok(String::from_utf8_lossy(&resp).into_owned());
            }
            Ok((code, resp)) => {
                last_err = format!("{addr}: HTTP {code} {}", String::from_utf8_lossy(&resp))
            }
            Err(e) => last_err = format!("{addr}: {e}"),
        }
    }
    Err(Error::Untrusted(format!("no door answered — {last_err}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group;
    use crate::node::NodeKey;

    const NOW: i64 = 1_785_000_000;

    struct TestMesh(std::path::PathBuf);
    impl TestMesh {
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TestMesh {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn mesh(label: &str) -> (TestMesh, NodeKey, GroupCredential) {
        // Unique per CALL, not just per process+label — tests run concurrently and two
        // meshes named "river" must never share (or Drop-delete) one directory.
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let p =
            std::env::temp_dir().join(format!("familiar_fed_{}_{n}_{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        let node = NodeKey::load_or_mint(&p, label).unwrap();
        let cred = group::create_group(&p, &node, label, NOW, 90 * 24 * 3600).unwrap();
        (TestMesh(p), node, cred)
    }

    #[test]
    fn the_whole_door_end_to_end() {
        let (a_dir, a_node, a_cred) = mesh("river");
        let (b_dir, _b_node, b_cred) = mesh("cedar");

        // A member of river mints; cedar's lighthouse introduces.
        let invite = mint_mesh_invite(
            &a_node,
            &a_cred.membership,
            &a_cred,
            vec!["203.0.113.1".into()],
            NOW,
        )
        .unwrap();
        let payload = invite.encode().unwrap();
        let decoded = MeshInvite::decode(&payload).unwrap();
        let b_intro = our_introduction(
            b_dir.path(),
            &b_cred,
            vec!["203.0.113.2".into()],
            &decoded.token_id,
            NOW,
        )
        .unwrap();
        let freq = FederateRequest {
            invite: decoded.clone(),
            introduction: b_intro,
        };

        // River's door: verify, spend, store pending, answer with river's introduction.
        let a_answer = receive_introduction(
            a_dir.path(),
            &a_cred,
            &freq,
            vec!["203.0.113.1".into()],
            NOW,
        )
        .unwrap();
        let pending = load_sibling(a_dir.path(), &b_cred.group_id).unwrap();
        assert_eq!(
            pending.state, "pending",
            "consent is a human tap, never automatic"
        );
        assert_eq!(pending.handle, "cedar");

        // Cedar adopts river's answer immediately — pasting the invite was cedar's act.
        let b_side = adopt_answered_introduction(b_dir.path(), &decoded, &a_answer, NOW).unwrap();
        assert_eq!(b_side.state, "sibling");
        assert_eq!(b_side.handle, "river");

        // A second redemption of the same token is refused.
        assert!(receive_introduction(a_dir.path(), &a_cred, &freq, vec![], NOW).is_err());

        // Pending does not read; the welcome tap makes it stand; severed stops it again.
        assert!(standing_sibling_by_pubkey(a_dir.path(), &b_cred.group_pubkey).is_none());
        welcome_sibling(a_dir.path(), &b_cred.group_id, "ian", NOW).unwrap();
        assert!(standing_sibling_by_pubkey(a_dir.path(), &b_cred.group_pubkey).is_some());
        sever_sibling(a_dir.path(), &b_cred.group_id, "test severance", NOW).unwrap();
        assert!(standing_sibling_by_pubkey(a_dir.path(), &b_cred.group_pubkey).is_none());

        // A severed mesh cannot re-introduce itself back in — restore is a human act.
        let invite2 = mint_mesh_invite(&a_node, &a_cred.membership, &a_cred, vec![], NOW).unwrap();
        let b_intro2 =
            our_introduction(b_dir.path(), &b_cred, vec![], &invite2.token_id, NOW).unwrap();
        let freq2 = FederateRequest {
            invite: invite2,
            introduction: b_intro2,
        };
        assert!(receive_introduction(a_dir.path(), &a_cred, &freq2, vec![], NOW).is_err());
    }

    #[test]
    fn an_expired_or_foreign_invite_is_refused() {
        let (a_dir, a_node, a_cred) = mesh("river");
        let (b_dir, _bn, b_cred) = mesh("cedar");
        let invite = mint_mesh_invite(&a_node, &a_cred.membership, &a_cred, vec![], NOW).unwrap();
        let intro = our_introduction(b_dir.path(), &b_cred, vec![], &invite.token_id, NOW).unwrap();
        // Expired.
        let freq = FederateRequest {
            invite: invite.clone(),
            introduction: intro.clone(),
        };
        assert!(receive_introduction(
            a_dir.path(),
            &a_cred,
            &freq,
            vec![],
            NOW + MESH_INVITE_TTL_SECS + 1
        )
        .is_err());
        // Presented at the wrong mesh (cedar's own door).
        assert!(receive_introduction(b_dir.path(), &b_cred, &freq, vec![], NOW).is_err());
        // A tampered introduction (handle swapped after signing) fails the mesh signature.
        let mut forged = intro;
        forged.handle = "definitely-cedar".into();
        let freq3 = FederateRequest {
            invite,
            introduction: forged,
        };
        assert!(receive_introduction(a_dir.path(), &a_cred, &freq3, vec![], NOW).is_err());
    }

    #[test]
    fn a_mesh_cannot_sibling_itself_and_a_covenant_node_cannot_speak_as_the_mesh() {
        let (a_dir, a_node, a_cred) = mesh("river");
        let invite = mint_mesh_invite(&a_node, &a_cred.membership, &a_cred, vec![], NOW).unwrap();
        let self_intro =
            our_introduction(a_dir.path(), &a_cred, vec![], &invite.token_id, NOW).unwrap();
        let freq = FederateRequest {
            invite,
            introduction: self_intro,
        };
        assert!(receive_introduction(a_dir.path(), &a_cred, &freq, vec![], NOW).is_err());

        // A covenant credential (no group secret) cannot sign an introduction.
        let cov = GroupCredential::covenant(
            a_cred.group_id.clone(),
            a_cred.group_pubkey.clone(),
            a_cred.label.clone(),
            a_cred.membership.clone(),
        );
        assert!(our_introduction(a_dir.path(), &cov, vec![], "deadbeef", NOW).is_err());
    }

    #[test]
    fn the_projection_ladder_shows_strictly_more_per_rung() {
        let (a_dir, _an, a_cred) = mesh("river");
        // Two siblings: cedar stands, willow is pending.
        for (handle, gid, state) in [
            ("cedar", "gid-cedar", "sibling"),
            ("willow", "gid-willow", "pending"),
        ] {
            save_sibling(
                a_dir.path(),
                &SiblingRecord {
                    handle: handle.into(),
                    group_id: gid.into(),
                    group_pubkey: format!("pk-{handle}"),
                    hosts: vec![],
                    lat: 44.0,
                    lon: -93.0,
                    declared_areas: vec!["weather".into()],
                    offered_tools: vec![],
                    state: state.into(),
                    welcomed_by: "ian".into(),
                    note: String::new(),
                    first_seen: NOW,
                    last_seen: NOW,
                },
            )
            .unwrap();
        }
        let mut cfg = crate::config::load(a_dir.path()).unwrap_or_default();
        cfg.declared_areas = vec!["energy".into(), "birds".into()];
        // Config is human-owned plain text (no save API, deliberately) — write it as the human would.
        std::fs::write(
            a_dir.path().join(crate::config::CONFIG_FILE),
            serde_json::to_string_pretty(&cfg).unwrap(),
        )
        .unwrap();

        let full = crate::worldview::assemble_worldview(a_dir.path(), &a_cred, NOW).unwrap();
        assert_eq!(
            full.siblings.len(),
            2,
            "members see every sibling, pending included"
        );
        assert!(full.siblings.iter().any(|s| s.handle == "cedar"));
        assert_eq!(full.declared_areas, vec!["energy", "birds"]);

        // Guest rung: federation is shape, not names.
        let mut guest = full.clone();
        crate::standing::to_guest_view(&mut guest, "some-guest-node");
        assert_eq!(
            guest.siblings.len(),
            2,
            "that we federate is shape — the count stays"
        );
        assert!(
            guest
                .siblings
                .iter()
                .all(|s| s.handle != "cedar" && s.declared_areas.is_empty()),
            "with whom is the household's business"
        );
        assert!(guest.declared_areas.is_empty());

        // Sibling rung: our handle, our declaration, and the reader's own standing — no more.
        let mut sib = full.clone();
        crate::standing::to_sibling_view(&mut sib, "gid-cedar", &a_cred.label);
        assert_eq!(
            sib.group_label, "river",
            "a sibling knows whose door it reads"
        );
        assert_eq!(sib.declared_areas, vec!["energy", "birds"]);
        assert!(
            sib.siblings
                .iter()
                .any(|s| s.handle == "cedar" && s.group_id == "gid-cedar"),
            "the reader sees itself as itself"
        );
        assert!(
            !sib.siblings.iter().any(|s| s.handle == "willow"),
            "other siblings stay pseudonymized"
        );
        // And the ladder is strict: names the household sees never reach the lower rungs.
        assert!(sib
            .members
            .iter()
            .all(|m| m.human.is_empty() || m.human == "someone"));
    }

    #[test]
    fn sibling_read_requests_verify_and_pin() {
        let (_a_dir, _an, a_cred) = mesh("river");
        let req = SiblingViewRequest::sign(&a_cred, NOW).unwrap();
        req.verify().unwrap();
        let mut tampered = req;
        tampered.group_id = "someone-else".into();
        assert!(tampered.verify().is_err());
    }
}
