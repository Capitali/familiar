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
    /// The reading device's own app build (e.g. "16") — surfaced as its roster version. Optional so
    /// older clients (and the signer's byte layout) stay compatible.
    #[serde(default)]
    pub client_version: String,
    /// The reading device's OS release (e.g. "iPadOS 26.1"). Optional for the same reason.
    #[serde(default)]
    pub os_version: String,
    /// The device's position (decimal degrees) when it has GPS and consent — near-real-time,
    /// refreshed on every read. 0/0 = not reported. The request is verified over the raw
    /// received bytes, so optional fields are wire-safe here.
    #[serde(default)]
    pub lat: f64,
    #[serde(default)]
    pub lon: f64,
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

/// One of the familiar's theories (a thread) — its own question + interpretation, and where that
/// stands (open / pursued / abandoned / answered). The iPad "Theories" screen renders these.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TheoryView {
    pub id: String,
    pub question: String,
    pub theory: String,
    pub direction: String,
    pub status: String,
    /// Whatever the status is, it is dated: created / entered current status / last worked.
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub status_at: i64,
    #[serde(default)]
    pub last_worked_at: i64,
    /// The human's answers so far — shown under the question, carried by the pursuit.
    #[serde(default)]
    pub answers: Vec<String>,
}

/// One of the familiar's reflections on humanity — its lived understanding, appended beside (never
/// over) the constitutional HUMANITY.md. Mirrors `familiar_kernel::humanity::Reflection`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionView {
    pub id: String,
    pub reflection: String,
    pub grounded_in: String,
    pub created_at: i64,
}

/// The boundary gates — Law III, human-owned. What outward reach the human has opened. Read-only
/// over the mesh: a peer sees the gate states but a device can't widen them (that stays a local,
/// human act at the familiar itself).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateStates {
    pub llm: bool,
    pub camera: bool,
    #[serde(default)]
    pub microphone: bool,
    #[serde(default)]
    pub location: bool,
    #[serde(default)]
    pub motion: bool,
    #[serde(default)]
    pub network_discovery: bool,
    #[serde(default)]
    pub face_recognition: bool,
    pub network: bool,
    pub mesh: bool,
    pub execute: bool,
    pub agent: bool,
    pub tool_install: bool,
    /// The ADR-0013 seam: may the familiar speak to non-members?
    #[serde(default)]
    pub outreach: bool,
    /// The ADR-0032 seam: may the familiar drive a declared control surface?
    #[serde(default)]
    pub actuate: bool,
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

/// One row of the welcome screen: who arrived, in what state, established how, when.
/// "welcome to these new members" — a greeting, never a gate (ADR-0026 §4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrivalView {
    pub node_id: String,
    pub label: String,
    /// "member" | "guest" — a visitor looking around appears too, with no pending-decision framing.
    pub state: String,
    /// The established handle, when there is one. Projected away for guest readers.
    #[serde(default)]
    pub handle: String,
    /// How the identity was established ("invite_token", "rotation_proof", …) — empty for a guest.
    #[serde(default)]
    pub via: String,
    pub at: i64,
    /// Where the knock came from — the visitor's self-reported position and the address the
    /// door saw, so a human can VERIFY who they are welcoming ("betty's new phone" should not
    /// be knocking from a Beijing datacenter). Zero/empty when unknown. Projected away for
    /// guest readers — origin is the household's evidence, not a fellow visitor's.
    #[serde(default)]
    pub lat: f64,
    #[serde(default)]
    pub lon: f64,
    #[serde(default)]
    pub addr: String,
    /// "iOS 26.6 · v69" — what the knocking device says it runs. Identity evidence too.
    #[serde(default)]
    pub build: String,
    /// When the door last heard from this arrival (peer roster), 0 when never — so the
    /// welcome can say "seen 4m ago" beside the moment they first arrived.
    #[serde(default)]
    pub last_seen: i64,
}

/// The arrivals window — the same 24-hour judgement ADR-0021 uses for the live roster split.
pub const ARRIVAL_WINDOW_SECS: i64 = 24 * 60 * 60;

/// A guest's claim awaiting the claimed human's own device (ADR-0026 E2, over the mesh): "this
/// device says it belongs to `handle`". Shown ONLY to member readers; a device of that handle
/// can mint a voucher from it in one tap — the automatic path for "old user, new device" that
/// needs no invite paste and no QR. Carries the subject's pubkey because a voucher names a key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimView {
    pub node_id: String,
    pub label: String,
    pub handle: String,
    /// The claiming device's public key (hex) — what a voucher vouches for; the door re-verifies
    /// it against the record by fingerprint, so a tampered value only fails the vouch.
    pub pubkey: String,
    pub since: i64,
    /// True when the claimed handle is already ESTABLISHED here: the card asks that human's
    /// own devices to vouch. False = a NEW name: the card asks any member to welcome them.
    #[serde(default)]
    pub known: bool,
}

/// The compact snapshot returned to a member device — enough to render a Glass-like console: the
/// three constitutional meters, the peer roster, and the recent observation feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worldview {
    pub group_label: String,
    /// The familiar's own node id (so the console can distinguish self from peers).
    pub node_id: String,
    /// The familiar's current open question for the human (empty if none) — a console shows it and
    /// offers a reply.
    #[serde(default)]
    pub question: String,
    /// The human the open question is **addressed to** (`familiar_kernel::routing`). Empty means
    /// unaddressed. A console shows "asking Betty" rather than putting everyone's question in
    /// front of whoever happens to be holding the device. Ownership is about who is ASKED — the
    /// answer, once confirmed, is an ordinary public observation.
    #[serde(default)]
    pub question_owner: String,
    /// How many members are reading as **guests** — admitted (ADR-0015) but not yet granted
    /// standing (ADR-0020), so nobody has said who they are. A waiting decision, and the number
    /// the console's welcome glyph pulses on. Zero is the resting state, not an error.
    #[serde(default)]
    pub guests_waiting: usize,
    /// Node ids at full standing, so a console can tell a recognised member from a waiting guest
    /// without a second round trip. Node ids only — no names, and absent from a guest projection.
    #[serde(default)]
    pub standing_full: Vec<String>,
    /// **Who is new** (ADR-0026 / ADR-0021's third view): arrivals within the last 24 hours —
    /// what the welcome screen greets. Informational only; nothing here frames a decision.
    #[serde(default)]
    pub arrivals: Vec<ArrivalView>,
    /// Guests whose refused introductions CLAIMED an existing human — waiting for that human's
    /// own device to vouch (E2 over the mesh). Members only; projected away for guest readers.
    #[serde(default)]
    pub claims_waiting: Vec<ClaimView>,
    /// The live mesh game, when one is burning (Riddle of the Mesh / The Campfire). Members
    /// only; a guest's projection carries no game — the fire is inside the house.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game: Option<crate::game::GameView>,
    pub presence: f64,
    pub withdrawn: bool,
    pub service: f64,
    pub capacity: f64,
    pub observation_count: usize,
    pub peers: Vec<PeerView>,
    /// Newest first, capped at [`RECENT_CAP`].
    pub recent: Vec<ObsView>,
    /// The familiar's own theories, newest first, capped at [`THEORY_CAP`].
    pub theories: Vec<TheoryView>,
    /// How well the factory's theories have paid off so far (smoothed [0,1]); see `score::theory_record`.
    pub theory_quality: f64,
    /// The boundary gates (Law III) as the human has set them.
    pub gates: GateStates,
    /// Metabolic ticks recorded (a rough age/health of the cycle).
    pub tick: u64,
    /// Seconds since the familiar's earliest observation — a coarse uptime.
    pub uptime_secs: i64,
    /// The familiar's reflections on humanity, newest first, capped at [`HUMANITY_CAP`].
    pub humanity: Vec<ReflectionView>,
    /// Every mesh participant classified into one layer (self / gossip peer / device peer / device
    /// agent), with os + join date — the roster the iPad renders as a table and a graph.
    pub members: Vec<crate::members::Member>,
    /// Networks, services, and data streams the mesh has discovered (Bonjour/reach) — the second
    /// roster tab. Aggregated from `discovered service:*` observations already crossing the mesh.
    pub services: Vec<ServiceView>,
    /// The frontier: reachable devices that aren't enrolled members yet — faded branches on the map.
    /// Aggregated from `can-reach device:*` observations (the daemon's paced reach sweep).
    pub frontier: Vec<FrontierView>,
    /// Real relationships between members — the edges of the mesh graph (gossip / delegation /
    /// attribution). The map lays members out as equals and draws these, so it reads as a mesh.
    pub edges: Vec<EdgeView>,
    /// The shared roadmap — goals the mesh owns and burns down together, with live status and which
    /// node claimed each. Every node holds the same list; the console renders it as the to-do board.
    #[serde(default)]
    pub goals: Vec<GoalView>,
    /// Every address this familiar currently answers at, most-universal first (tailnet, then LAN).
    /// A console merges these into its candidate list, so a device that enrolled on the LAN learns
    /// the tailnet path — and can reach the mesh from cellular — without re-enrolling.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hosts: Vec<String>,
    /// Every TLS pin a device may meet across those hosts (this node's + siblings' it vouches for).
    /// A device adopts the set and accepts any member's cert, so failover to a reachable sibling
    /// (the lighthouse) works — pinned to the group, not one node (ADR-0012).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pins: Vec<String>,
    /// The meshes standing beside this one (ADR-0033) — pending introductions included, so
    /// the welcome screen can offer the tap. One entry per mesh; its internals are its own.
    #[serde(default)]
    pub siblings: Vec<SiblingView>,
    /// Areas of knowledge THIS mesh declares to its siblings — shown to members (it is their
    /// declaration) and to sibling readers (it is what they were promised). Declaration,
    /// never inventory.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declared_areas: Vec<String>,
}

/// A sibling mesh as the console renders it — one entity, one dot, one arc (ADR-0033 §5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiblingView {
    pub handle: String,
    pub group_id: String,
    /// "pending" | "sibling" | "severed"
    pub state: String,
    /// Self-declared location; 0,0 = chose not to declare.
    pub lat: f64,
    pub lon: f64,
    pub declared_areas: Vec<String>,
    pub offered_tools: Vec<String>,
    pub welcomed_by: String,
    pub first_seen: i64,
    pub last_seen: i64,
}

/// A goal on the shared roadmap, as the console renders it. Mirrors `goal::Goal` minus the internals
/// the UI doesn't need. `owner` is the claiming node's short id (empty while unclaimed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalView {
    pub id: String,
    pub description: String,
    pub needs: Vec<String>,
    /// "proposed" | "claimed" | "in_progress" | "awaiting_human" | "done" | "failed" | "blocked".
    pub status: String,
    /// Short node id of the owner (empty while unclaimed).
    pub owner: String,
    /// The **human** accountable for it — distinct from `owner`, which is only the machine doing
    /// the work. Empty when the goal belongs to no one in particular.
    #[serde(default)]
    pub owner_human: String,
    pub origin: String,
    /// Tools/artifacts it produced, for the audit trail.
    pub produced: String,
    /// Progress + learnings that travelled with the goal.
    pub notes: String,
    pub updated_at: i64,
    /// Lifecycle dates — whatever state the goal is in carries the date it got there.
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub status_at: i64,
    #[serde(default)]
    pub last_worked_at: i64,
    #[serde(default)]
    pub completed_at: i64,
    #[serde(default)]
    pub ended_at: i64,
}

/// A real relationship between two mesh members — an edge in the graph the map draws. The mesh is
/// peer-to-peer, not hub-and-spoke: these edges let the map show peers linked to peers (a watch to
/// its phone, a thinking peer handing a theory to an executor) rather than everything through "self".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeView {
    /// Source member node_id.
    pub from: String,
    /// Destination member node_id.
    pub to: String,
    /// "gossip" (briefs exchanged / worldview read), "delegation" (a theory handed to an executor),
    /// or "attribution" (a sub-device reaches the mesh through a parent device).
    pub kind: String,
}

/// A device on the frontier — reachable but not (yet) an enrolled mesh member. Drawn as a faded
/// branch on the mesh map, dimmed by how far the familiar could extend to it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontierView {
    pub label: String,
    pub ip: String,
    /// Reach class: "agent-capable" (could run a familiar agent), "protocol-controllable" (could
    /// command it), or "observable-only" (only visible). Governs the branch's opacity.
    pub reach: String,
    /// Open services discovered (ssh, airplay, mqtt, …) — the interfaces the frontier exposes.
    pub open: Vec<String>,
    pub last_seen: i64,
}

/// A discovered network service / data stream — from a peer's Bonjour survey, shared over the mesh.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceView {
    /// The service kind ("airplay", "ssh", "mqtt", …).
    pub kind: String,
    /// The advertised instance name.
    pub name: String,
    /// Which node saw it (actor).
    pub seen_by: String,
    pub last_seen: i64,
}

/// How many recent observations the snapshot carries. A console shows a live tail, not the archive.
const RECENT_CAP: usize = 60;
/// How many theories the snapshot carries.
const THEORY_CAP: usize = 24;
/// How many humanity reflections the snapshot carries.
const HUMANITY_CAP: usize = 24;

/// Verify a signed read request and, if trusted, assemble the familiar's worldview snapshot.
/// Fail-closed: an `Untrusted` error means the caller answers 403 (or 409 for a replay).
pub(crate) fn read_worldview(
    dir: &Path,
    raw: &[u8],
    sig_hex: &str,
    now: i64,
    guard: &Mutex<IngestGuard>,
    peer_ip: &str,
) -> Result<Worldview> {
    if !familiar_kernel::boundary::load(dir)
        .map_err(Error::Io)?
        .allow_mesh
    {
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

    // A member that reads the worldview participates as a full peer (a console), not a write-only
    // sensor — so record it in the peer roster (by its own node id, from where it connected). This
    // is what promotes an iPad from "device agent" to "peer" in the familiar's own Glass. Failing to
    // record is non-fatal: the read still succeeds.
    let _ = crate::transport::register_device_peer(
        dir,
        &req.node.node_id,
        &req.node.label,
        peer_ip,
        &req.client_version,
        &req.os_version,
        req.lat,
        req.lon,
    );
    // The same evidence, on the record that REPLICATES — the peer row above lives only at
    // this door, and a welcome rendered by a sibling door needs the origin too (the card
    // flapped between rich and bare as console failover alternated doors). Best-effort,
    // throttled inside note_origin. "background" is the iOS sync task's label, not a name.
    let _ = crate::record::note_origin(
        dir,
        &req.node.node_id,
        crate::record::OriginEvidence {
            label: if req.node.label == "background" { String::new() } else { req.node.label.clone() },
            addr: peer_ip.split(':').next().unwrap_or("").to_string(),
            build: match (req.os_version.is_empty(), req.client_version.is_empty()) {
                (false, false) => format!("{} · v{}", req.os_version, req.client_version),
                (false, true) => req.os_version.clone(),
                (true, false) => format!("v{}", req.client_version),
                (true, true) => String::new(),
            },
            lat: req.lat,
            lon: req.lon,
            at: now,
        },
    );

    // A certified reader with NO record gets its guest record RESTORED: the cert it just
    // proved is the receipt of a covenant it once attested (a cert is only ever minted upon
    // attestation), so filter 1 is satisfied by evidence in hand. Without this, a device
    // whose record was lost (a purge, a fresh door) reads forever as an invisible ghost —
    // absent from the welcome, unvouchable, unnameable. First filter from the cert; the
    // second (identity) it earns like anyone.
    if crate::record::find_by_key(dir, &req.node.node_id).is_none() {
        let attestation = crate::enroll::Attestation {
            laws_version: crate::enroll::LAWS_VERSION,
            statement: format!(
                "{} (record restored from the member's cert at read; originally attested at {})",
                crate::enroll::COVENANT_STATEMENT,
                req.membership.issued
            ),
            ts: req.membership.issued,
        };
        let rec =
            crate::record::MembershipRecord::guest(&req.node.node_id, &req.node.node_id, attestation, now);
        let _ = crate::record::save(dir, &rec);
        let _ = crate::record::record_pubkey(dir, &req.node.node_id, &req.node.pubkey, now);
    }

    let mut view = assemble_worldview(dir, &cred, now)?;
    // "You are here" belongs to the *requester*, not to us. classify() marks this serving
    // node SelfNode (true for our own console); a remote console rendering that verbatim
    // shows the host as "you" — so re-tag per requester: their row is self, ours is a peer.
    for m in &mut view.members {
        if m.kind == crate::members::MemberKind::SelfNode {
            m.kind = crate::members::MemberKind::GossipPeer;
            m.relationship = "gossip peer · host".into();
        }
        if m.node_id == req.node.node_id {
            m.kind = crate::members::MemberKind::SelfNode;
            m.relationship = "self".into();
        }
    }
    // Membership decided that this node may READ. Standing decides what it sees. Admission is
    // automatic (ADR-0015), so "admitted" and "trusted with the household's names" are not the
    // same fact — an unlisted reader gets the same mesh with none of the identities. Default
    // deny: full standing is granted by hand in `standing.json`.
    if crate::standing::standing_of(dir, &req.node.node_id) == crate::standing::Standing::Guest {
        crate::standing::to_guest_view(&mut view, &req.node.node_id);
    }
    // Tell the console every address the MESH answers at: human-asserted first (a
    // lighthouse's NAT-hidden public IP or DNS name — `advertise_hosts`), then ours, then
    // fresh gossip peers (any member node serves the same verified read seam — the
    // worldview is gossip-replicated). A device that loses this node fails over to a
    // sibling.
    let cfg = crate::config::load(dir).unwrap_or_default();
    let mut hosts = cfg.advertise_hosts.clone();
    // The rendezvous/lighthouse address is a public failover candidate — a device off-LAN keeps
    // it when tailnet and LAN addresses go unreachable (ADR-0012).
    for h in &cfg.rendezvous_hosts {
        if !hosts.contains(h) {
            hosts.push(h.clone());
        }
    }
    for h in crate::transport::reachable_hosts() {
        if !hosts.contains(&h) {
            hosts.push(h);
        }
    }
    for p in crate::transport::load_peers(dir) {
        if now - p.last_seen <= crate::transport::GOSSIP_FRESH_SECS * 5 {
            let ip = p.addr.split(':').next().unwrap_or("").to_string();
            if !ip.is_empty() && is_gossipable_addr(&ip) && !hosts.contains(&ip) {
                hosts.push(ip);
            }
        }
    }
    view.hosts = hosts;
    view.pins = crate::transport::advertised_pins(dir);
    Ok(view)
}

/// May a peer's **observed source address** be gossiped to other devices as somewhere the mesh can
/// be reached?
///
/// Only if it is private (RFC1918 / unique-local) or tailnet (CGNAT 100.64/10). Everything else is
/// refused, and the reason is a category error worth naming: `peer.addr` is the socket address a
/// peer *connected from*, not an address it *listens on*. For a peer behind NAT — every device on
/// a Starlink uplink — those are different things. The public one is a NAT exit with nothing behind
/// it, so gossiping it hands every device in the mesh an address that can only ever time out.
///
/// This was not theoretical. The rig's public IP changes whenever it moves, each new one entered
/// the gossip as a fresh "host", nothing ever pruned them, and a single iPad accumulated 33
/// candidate addresses — ~30 of them dead NAT exits it retried at 10s each before reaching anything
/// real. A private address does not have this problem: a LAN sibling genuinely can dial it, and
/// when it goes stale it fails fast on the same network rather than hanging.
///
/// The lighthouse's own public address is unaffected — it arrives via `advertise_hosts` /
/// `rendezvous_hosts`, which are *asserted by that node about itself* rather than observed of
/// someone else. That is the distinction being enforced here, and it is why a future second
/// lighthouse (ADR-0018 wants the door eventually redundant) must be introduced by configuration
/// or enrolment rather than inferred from a connection.
fn is_gossipable_addr(ip: &str) -> bool {
    use std::net::IpAddr;
    match ip.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => {
            let o = v4.octets();
            // Tailscale's CGNAT range is a real reachable path, and the doctrine (ADR-0012) already
            // sorts it last rather than excluding it.
            v4.is_private() || v4.is_link_local() || v4.is_loopback() || o[0] == 100 && (64..=127).contains(&o[1])
        }
        // Unique-local (fc00::/7) and link-local; a global v6 address is directly routable in
        // principle, but it is still an observed source address, so it gets the same treatment.
        Ok(IpAddr::V6(v6)) => {
            let s = v6.segments()[0];
            v6.is_loopback() || (s & 0xfe00) == 0xfc00 || (s & 0xffc0) == 0xfe80
        }
        Err(_) => false,
    }
}

/// Assemble the worldview snapshot from the canonical store + signals + peers + theories + gates +
/// humanity + members. The auth-free core of a read — used by the verified mesh path (after it
/// checks membership) and by the **localhost-only** `GET /local/worldview` a peer's own SwiftUI
/// console reads (it's reading the node on its own machine; no mesh signature needed for that).
pub fn assemble_worldview(
    dir: &Path,
    cred: &crate::group::GroupCredential,
    now: i64,
) -> Result<Worldview> {
    // The recent window only: presence/service/capacity signals and the recent-observations
    // strip all work over minutes-to-hours of history. A full load of the whole log per
    // request saturated the door once the log passed ~20k rows.
    let obs = familiar_kernel::observation::load_recent(dir, 4000).map_err(Error::Io)?;
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

    // The familiar's theories + how well its theorizing has paid off (so the iPad can show its own
    // questions and their track record), and the human-owned boundary gates (read-only over mesh).
    let threads = familiar_kernel::thread::load(dir).unwrap_or_default();
    let candidates = familiar_kernel::candidate::load(dir).unwrap_or_default();
    let trials = familiar_kernel::trial::load(dir).unwrap_or_default();
    let theory_quality =
        familiar_kernel::score::theory_record(&threads, &candidates, &trials, 0.0).quality;
    let theories: Vec<TheoryView> = threads
        .iter()
        .rev()
        .take(THEORY_CAP)
        .map(|t| TheoryView {
            id: t.id.clone(),
            question: t.question.clone(),
            theory: t.theory.clone(),
            direction: t.direction.clone(),
            status: t.status.clone(),
            created_at: t.created_at,
            status_at: if t.status_at > 0 {
                t.status_at
            } else {
                t.created_at
            },
            last_worked_at: t.last_worked_at,
            answers: t.answers.clone(),
        })
        .collect();

    let b = familiar_kernel::boundary::load(dir)
        .unwrap_or_else(|_| familiar_kernel::boundary::Boundary::closed());
    let gates = GateStates {
        llm: b.allow_llm,
        camera: b.allow_camera,
        microphone: b.allow_microphone,
        location: b.allow_location,
        motion: b.allow_motion,
        network_discovery: b.allow_network_discovery,
        face_recognition: b.allow_face_recognition,
        network: b.allow_network,
        mesh: b.allow_mesh,
        execute: b.allow_execute,
        agent: b.allow_agent,
        tool_install: b.allow_tool_install,
        outreach: b.allow_outreach,
        actuate: b.allow_actuate,
    };
    let tick = familiar_kernel::activity::load(dir)
        .map(|a| a.len() as u64)
        .unwrap_or(0);
    let uptime_secs = obs
        .iter()
        .map(|o| o.ts)
        .min()
        .map(|t0| (now - t0).max(0))
        .unwrap_or(0);

    let humanity: Vec<ReflectionView> = familiar_kernel::humanity::load(dir)
        .unwrap_or_default()
        .into_iter()
        .rev()
        .take(HUMANITY_CAP)
        .map(|r| ReflectionView {
            id: r.id,
            reflection: r.reflection,
            grounded_in: r.grounded_in,
            created_at: r.created_at,
        })
        .collect();

    let question = std::fs::read_to_string(dir.join("question.txt"))
        .unwrap_or_default()
        .trim()
        .to_string();
    // Who it is being asked OF. The text lives in question.txt for the console's sake; the
    // addressee lives on the question record, so read it back through the active id.
    let question_owner = std::fs::read_to_string(dir.join("active_question.txt"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|id| !id.is_empty())
        .and_then(|id| {
            familiar_kernel::question::load(dir)
                .ok()
                .and_then(|qs| qs.into_iter().find(|q| q.id == id).map(|q| q.owner))
        })
        .unwrap_or_default();
    let members = crate::members::classify(dir, now);
    // Members admitted but not yet stood — see `guests_waiting`. Counted from the roll rather
    // than from a pending queue, because admission is automatic and there is no queue: being a
    // guest IS the waiting state.
    let roll = crate::standing::load(dir);
    let guests_waiting = members
        .iter()
        .filter(|m| {
            m.kind != crate::members::MemberKind::SelfNode
                && !roll.full.iter().any(|n| n == &m.node_id)
        })
        .count();
    let frontier = frontier_devices(&obs, &members);
    let edges = mesh_edges(&members, &obs, &cred.membership.node_id);

    // Who is new: every record whose admission (or, for a guest, first sighting) falls within
    // the last 24 hours. Newest first. Severed records are not arrivals.
    // Origin evidence rides along from the peer roster: where the knock came from (address,
    // self-reported position, OS/build) so the welcome is made with eyes open.
    let arrival_peers = crate::transport::load_peers(dir);
    let mut arrivals: Vec<ArrivalView> = crate::record::load_all(dir)
        .into_iter()
        .filter_map(|r| {
            let state = crate::record::derive_state(&r);
            let (word, at) = match &state {
                crate::record::RecordState::Member => {
                    ("member", r.admitted.as_ref().map(|a| a.at).unwrap_or(r.first_seen))
                }
                crate::record::RecordState::Guest => ("guest", r.first_seen),
                crate::record::RecordState::Severed { .. } => return None,
            };
            if now - at > ARRIVAL_WINDOW_SECS || at > now + 60 {
                return None;
            }
            // Label and origin prefer this door's live peer row, then the origin evidence
            // replicated on the record — so a door the visitor never read through still
            // renders the same welcome, not a bare node-id (the card flapped between the
            // two as console failover alternated doors).
            let origin = r.origin.clone();
            let label = members
                .iter()
                .find(|m| m.node_id == r.device_id || r.keys.contains(&m.node_id))
                .map(|m| m.label.clone())
                .or_else(|| {
                    origin
                        .as_ref()
                        .map(|o| o.label.clone())
                        .filter(|l| !l.is_empty())
                })
                .unwrap_or_else(|| r.device_id.chars().take(8).collect());
            let (handle, via) = match &r.identity.established {
                Some(e) => (e.handle.clone(), format!("{:?}", e.class)),
                None => (String::new(), String::new()),
            };
            let peer = arrival_peers
                .iter()
                .find(|p| p.node_id == r.device_id || r.keys.contains(&p.node_id));
            let (lat, lon, addr, build, last_seen) = match peer {
                Some(p) => (
                    p.lat,
                    p.lon,
                    p.addr.split(':').next().unwrap_or("").to_string(),
                    match (p.os_version.is_empty(), p.familiar_version.is_empty()) {
                        (false, false) => format!("{} · v{}", p.os_version, p.familiar_version),
                        (false, true) => p.os_version.clone(),
                        (true, false) => format!("v{}", p.familiar_version),
                        (true, true) => String::new(),
                    },
                    // The record's last_seen replicates (max-merge) — it may be fresher than
                    // this door's own sighting when the device reads through a sibling.
                    p.last_seen.max(r.last_seen),
                ),
                None => match &origin {
                    Some(o) => (o.lat, o.lon, o.addr.clone(), o.build.clone(), r.last_seen),
                    None => (0.0, 0.0, String::new(), String::new(), 0),
                },
            };
            Some(ArrivalView {
                node_id: r.device_id,
                label,
                state: word.into(),
                handle,
                via,
                at,
                lat,
                lon,
                addr,
                build,
                last_seen,
            })
        })
        .collect();
    arrivals.sort_by_key(|a| std::cmp::Reverse(a.at));

    // Claims awaiting the claimed human's own device (E2 over the mesh): guests whose refused
    // introduction named someone — surfaced so that human's device can vouch in one tap.
    let established_handles: Vec<String> = crate::record::load_all(dir)
        .iter()
        .filter(|r| crate::record::derive_state(r) == crate::record::RecordState::Member)
        .filter_map(|r| r.identity.established.as_ref().map(|e| e.handle.to_lowercase()))
        .filter(|h| !h.is_empty())
        .collect();
    let mut claims_waiting: Vec<ClaimView> = crate::record::load_all(dir)
        .into_iter()
        .filter_map(|r| {
            if crate::record::derive_state(&r) != crate::record::RecordState::Guest {
                return None;
            }
            let claim = r.identity.claim.as_ref()?;
            if claim.handle.trim().is_empty() || r.pubkey.is_empty() {
                return None;
            }
            let label = members
                .iter()
                .find(|m| m.node_id == r.device_id || r.keys.contains(&m.node_id))
                .map(|m| m.label.clone())
                .unwrap_or_else(|| r.device_id.chars().take(8).collect());
            Some(ClaimView {
                known: established_handles.contains(&claim.handle.to_lowercase()),
                node_id: r.device_id,
                label,
                handle: claim.handle.clone(),
                pubkey: r.pubkey.clone(),
                since: claim.ts,
            })
        })
        .collect();
    claims_waiting.sort_by_key(|c| std::cmp::Reverse(c.since));

    // The live game: tick the clock lazily on every read (the mesh has no game loop, only
    // readers and actors) and persist any expiry it caused before rendering the view.
    let game = crate::game::load(dir).map(|mut g| {
        if crate::game::tick(&mut g, now) {
            let _ = crate::game::save(dir, &g);
        }
        crate::game::view(&g)
    });
    // The changeling keeper settles on any console poll (ADR-0034) — a reveal owed by
    // this door, or a solo forge claim, runs off-path; a no-op costs one small read.
    crate::transport::spawn_changeling_touch(dir, None);

    let goals = goal_views(dir);

    Ok(Worldview {
        group_label: cred.label.clone(),
        node_id: cred.membership.node_id.clone(),
        // Address advertisement is the *served* read path's concern (read_worldview fills it);
        // the localhost console doesn't need it and assembly stays shell-out-free.
        hosts: Vec::new(),
        pins: Vec::new(),
        question,
        question_owner,
        guests_waiting,
        // Record-truth: when the records are the authority (read_records), the standing list
        // the consoles reconcile against must come from THEM — the legacy roll drifts (a
        // vouch minted at one door reached the other as a record, and the stale roll left the
        // iPad reading as a visitor while both doors' records said member).
        standing_full: if crate::config::load(dir).map(|c| c.read_records).unwrap_or(false) {
            crate::record::load_all(dir)
                .iter()
                .filter(|r| crate::record::derive_state(r) == crate::record::RecordState::Member)
                .flat_map(|r| r.keys.clone())
                .collect()
        } else {
            roll.full.clone()
        },
        arrivals,
        claims_waiting,
        game,
        presence: presence.measure,
        withdrawn: presence.withdrawn,
        service: service.measure,
        capacity: capacity.measure,
        observation_count: obs.len(),
        peers,
        recent,
        theories,
        theory_quality,
        gates,
        tick,
        uptime_secs,
        humanity,
        members,
        siblings: crate::federation::load_siblings(dir)
            .into_iter()
            .map(|s| SiblingView {
                handle: s.handle,
                group_id: s.group_id,
                state: s.state,
                lat: s.lat,
                lon: s.lon,
                declared_areas: s.declared_areas,
                offered_tools: s.offered_tools,
                welcomed_by: s.welcomed_by,
                first_seen: s.first_seen,
                last_seen: s.last_seen,
            })
            .collect(),
        declared_areas: crate::config::load(dir)
            .map(|c| c.declared_areas)
            .unwrap_or_default(),
        services: discovered_services(&obs),
        frontier,
        edges,
        goals,
    })
}

/// The shared roadmap for the console — every goal, newest activity first, its owner shown as a short
/// node id. Settled goals sort after active ones so the board reads "what's in flight" at a glance.
fn goal_views(dir: &Path) -> Vec<GoalView> {
    let mut goals = familiar_kernel::goal::load(dir).unwrap_or_default();
    goals.sort_by(|a, b| {
        let rank = |s: familiar_kernel::goal::Status| if s.settled() { 1 } else { 0 };
        rank(a.status)
            .cmp(&rank(b.status))
            .then(b.updated_at.cmp(&a.updated_at))
    });
    goals
        .into_iter()
        .map(|g| GoalView {
            id: g.id,
            description: g.description,
            needs: g.needs,
            status: g.status.as_str().to_string(),
            owner: g.owner_node.chars().take(8).collect(),
            owner_human: if g.owner_human.is_empty() {
                familiar_kernel::goal::human_origin(&g.origin)
            } else {
                g.owner_human.clone()
            },
            origin: g.origin,
            produced: g.produced,
            notes: g.notes,
            updated_at: g.updated_at,
            created_at: g.created_at,
            status_at: if g.status_at > 0 {
                g.status_at
            } else {
                g.updated_at
            },
            last_worked_at: g.last_worked_at,
            completed_at: g.completed_at,
            ended_at: g.ended_at,
        })
        .collect()
}

/// Derive the real relationships between members — the mesh graph's edges. Three honest kinds, only
/// where the data actually supports them (no invented links):
///
/// - **attribution**: a sub-device reaches the mesh through a parent (a `watch:ian` through
///   `phone:ian` / `ipad:ian` — same human suffix). The watch→phone edge is real and doesn't pass
///   through self.
/// - **delegation**: a thinking peer handed a theory to an executor — read from
///   `testing a theory delegated by <origin>` observations. origin→executor, a genuine peer↔peer
///   workload edge.
/// - **gossip**: briefs exchanged / worldview read. Any member with no attribution parent links to
///   self (the vantage point). This is the one kind that is inherently self-centered — full
///   peer↔peer gossip adjacency needs each peer to publish its own neighbors (a later step).
fn mesh_edges(
    members: &[crate::members::Member],
    obs: &[familiar_kernel::observation::Observation],
    self_id: &str,
) -> Vec<EdgeView> {
    use std::collections::HashSet;
    let mut edges = Vec::new();
    let mut seen: HashSet<(String, String, &str)> = HashSet::new();
    let mut push = |from: &str, to: &str, kind: &'static str, edges: &mut Vec<EdgeView>| {
        if from == to || from.is_empty() || to.is_empty() {
            return;
        }
        // Undirected dedup for gossip/attribution; keep delegation directed.
        let key = if kind == "delegation" {
            (from.to_string(), to.to_string(), kind)
        } else {
            let (a, b) = if from < to { (from, to) } else { (to, from) };
            (a.to_string(), b.to_string(), kind)
        };
        if seen.insert(key) {
            edges.push(EdgeView {
                from: from.to_string(),
                to: to.to_string(),
                kind: kind.to_string(),
            });
        }
    };

    // Index members by node_id and by the human suffix of their actor ("phone:ian" → "ian").
    let human_of = |actor: &str| actor.split(':').nth(1).unwrap_or("").to_string();
    let is_parent_ns = |actor: &str| {
        let ns = actor.split(':').next().unwrap_or("");
        matches!(ns, "phone" | "iphone" | "ipad" | "mac")
    };

    // Attribution: a sub-device (watch) → a parent device of the same human.
    let mut attributed: HashSet<String> = HashSet::new();
    for m in members {
        let ns = m.actor.split(':').next().unwrap_or("");
        if ns == "watch" {
            let human = human_of(&m.actor);
            if let Some(parent) = members.iter().find(|p| {
                is_parent_ns(&p.actor) && human_of(&p.actor) == human && !human.is_empty()
            }) {
                push(&m.node_id, &parent.node_id, "attribution", &mut edges);
                attributed.insert(m.node_id.clone());
            }
        }
    }

    // Delegation: origin (short id) → executor (the observation's node). The origin is written as the
    // first 8 hex of the delegating node; match it against a member by node_id prefix.
    for o in obs {
        // The executor records "testing a theory delegated by <short> — '…'" as its cycle theory.
        let marker = "delegated by ";
        let hay = format!("{} {}", o.action, o.context);
        if let Some(pos) = hay.find(marker) {
            let after = &hay[pos + marker.len()..];
            let short: String = after
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            if short.len() >= 4 {
                if let Some(origin) = members.iter().find(|m| m.node_id.starts_with(&short)) {
                    // Executor is the member whose actor matches the observation actor, else self.
                    let executor = members
                        .iter()
                        .find(|m| !m.actor.is_empty() && m.actor == o.actor)
                        .map(|m| m.node_id.clone())
                        .unwrap_or_else(|| self_id.to_string());
                    push(&origin.node_id, &executor, "delegation", &mut edges);
                }
            }
        }
    }

    // Gossip. The full peers (self + gossip peers) exchange briefs with *each other* — they form a
    // complete graph, the genuinely meshed layer (no hub). Device peers read a familiar's worldview,
    // so they link to self (the vantage point). Sub-devices already have an attribution parent.
    use crate::members::MemberKind;
    let full: Vec<&str> = members
        .iter()
        .filter(|m| matches!(m.kind, MemberKind::SelfNode | MemberKind::GossipPeer))
        .map(|m| m.node_id.as_str())
        .collect();
    for i in 0..full.len() {
        for j in (i + 1)..full.len() {
            push(full[i], full[j], "gossip", &mut edges);
        }
    }
    for m in members {
        let is_full = matches!(m.kind, MemberKind::SelfNode | MemberKind::GossipPeer);
        if is_full || attributed.contains(&m.node_id) {
            continue; // full peers meshed above; sub-devices attributed to a parent
        }
        // A device peer is enrolled in the group and can read *any* familiar's worldview — so it
        // links to every full peer, not just the one it happens to be reading now. That's why the
        // iPad shows meshed to both the Mac and the Linux VM, not hung off a single node.
        if full.is_empty() {
            push(&m.node_id, self_id, "gossip", &mut edges);
        } else {
            for f in &full {
                push(&m.node_id, f, "gossip", &mut edges);
            }
        }
    }

    edges
}

/// The frontier: devices the familiar can *reach* but hasn't enrolled. Aggregated from `can-reach
/// device:<label>` observations (context `class=… open=… ip=…`), deduped by label (newest wins),
/// and with any device that already matches an enrolled member (by label or IP) removed — a member
/// isn't a frontier. Sorted strongest-reach first.
fn frontier_devices(
    obs: &[familiar_kernel::observation::Observation],
    members: &[crate::members::Member],
) -> Vec<FrontierView> {
    use std::collections::HashMap;
    let member_labels: std::collections::HashSet<String> =
        members.iter().map(|m| m.label.to_lowercase()).collect();
    let member_addrs: std::collections::HashSet<String> = members
        .iter()
        .map(|m| m.addr.clone())
        .filter(|a| !a.is_empty())
        .collect();
    let mut latest: HashMap<String, FrontierView> = HashMap::new();
    for o in obs {
        if o.action != "can-reach" {
            continue;
        }
        let Some(label) = o.object.strip_prefix("device:") else {
            continue;
        };
        // Parse "class=<c> open=<a,b> ip=<x>" from the context.
        let mut reach = "observable-only".to_string();
        let mut open: Vec<String> = Vec::new();
        let mut ip = String::new();
        for field in o.context.split_whitespace() {
            if let Some(c) = field.strip_prefix("class=") {
                reach = c.to_string();
            } else if let Some(list) = field.strip_prefix("open=") {
                if list != "-" {
                    open = list.split(',').map(|s| s.to_string()).collect();
                }
            } else if let Some(i) = field.strip_prefix("ip=") {
                ip = i.to_string();
            }
        }
        let e = latest.entry(label.to_string()).or_insert(FrontierView {
            label: label.to_string(),
            ip: ip.clone(),
            reach: reach.clone(),
            open: open.clone(),
            last_seen: 0,
        });
        if o.ts >= e.last_seen {
            *e = FrontierView {
                label: label.to_string(),
                ip,
                reach,
                open,
                last_seen: o.ts,
            };
        }
    }
    let rank = |r: &str| match r {
        "agent-capable" => 2,
        "protocol-controllable" => 1,
        _ => 0,
    };
    let mut v: Vec<FrontierView> = latest
        .into_values()
        .filter(|f| {
            !member_labels.contains(&f.label.to_lowercase()) && !member_addrs.contains(&f.ip)
        })
        .collect();
    v.sort_by(|a, b| {
        rank(&b.reach)
            .cmp(&rank(&a.reach))
            .then(b.last_seen.cmp(&a.last_seen))
    });
    v.truncate(60);
    v
}

/// Aggregate discovered network services from the observation log (a peer's Bonjour survey posts
/// `<actor> discovered service:<kind>` with the instance name in context). Deduped by (kind, name),
/// newest first, capped.
fn discovered_services(obs: &[familiar_kernel::observation::Observation]) -> Vec<ServiceView> {
    use std::collections::HashMap;
    let mut latest: HashMap<String, ServiceView> = HashMap::new();
    for o in obs {
        let Some(kind) = o.object.strip_prefix("service:") else {
            continue;
        };
        if o.action != "discovered" {
            continue;
        }
        let key = format!("{kind}\u{1}{}", o.context);
        let e = latest.entry(key).or_insert(ServiceView {
            kind: kind.to_string(),
            name: o.context.clone(),
            seen_by: o.actor.clone(),
            last_seen: 0,
        });
        if o.ts >= e.last_seen {
            e.seen_by = o.actor.clone();
            e.last_seen = o.ts;
        }
    }
    let mut v: Vec<ServiceView> = latest.into_values().collect();
    v.sort_by_key(|s| std::cmp::Reverse(s.last_seen));
    v.truncate(80);
    v
}

#[cfg(test)]
mod gossip_addr_tests {
    use super::is_gossipable_addr;

    /// The exact addresses one iPad had accumulated on 2026-08-01, read off its own error banner.
    /// Roughly thirty were public NAT exits handed out by Starlink as the rig moved; each one was
    /// retried at a 10s timeout before anything real was reached, which is what a closed eye and a
    /// spinning wheel actually were.
    #[test]
    fn the_nat_exits_that_poisoned_the_ipad_are_refused() {
        let observed_junk = [
            "129.224.211.181", "139.178.131.76", "129.222.46.137", "97.202.192.132",
            "207.213.186.129", "207.213.186.177", "129.224.211.90", "24.111.157.40",
            "107.115.39.17", "129.222.44.157", "129.222.44.228", "129.222.44.227",
            "129.224.211.42", "139.178.129.14", "209.79.170.49", "207.213.186.50",
            "139.178.130.73", "207.213.186.145", "107.115.39.18", "207.213.186.49",
            "139.178.129.4", "207.213.186.178", "129.224.211.45", "107.115.39.82",
            "207.213.186.97",
        ];
        for ip in observed_junk {
            assert!(!is_gossipable_addr(ip), "{ip} is a NAT exit and must never be gossiped");
        }
    }

    #[test]
    fn real_paths_still_gossip() {
        for ip in ["192.168.108.10", "192.168.108.119", "10.0.0.5", "172.16.0.9",
                   "100.78.40.47", "100.105.39.72", "100.101.24.40", "fd00::1", "::1"] {
            assert!(is_gossipable_addr(ip), "{ip} is a genuinely dialable path");
        }
    }

    /// The lighthouse's public address must keep working — it just does not travel by this route.
    /// It is asserted by that node about itself (`advertise_hosts` / `rendezvous_hosts`), which is
    /// the whole distinction: self-asserted listener vs. observed source.
    #[test]
    fn the_lighthouse_public_address_is_not_gossipable_but_is_configured() {
        assert!(!is_gossipable_addr("134.209.168.50"));
    }

    #[test]
    fn boundaries_that_are_easy_to_get_wrong() {
        assert!(is_gossipable_addr("100.64.0.1"));      // first tailnet address
        assert!(is_gossipable_addr("100.127.255.254")); // last tailnet address
        assert!(!is_gossipable_addr("100.63.0.1"));     // just below — public
        assert!(!is_gossipable_addr("100.128.0.1"));    // just above — public
        assert!(is_gossipable_addr("172.31.255.254"));  // last RFC1918 in the 172 block
        assert!(!is_gossipable_addr("172.32.0.1"));     // just above — public
        assert!(!is_gossipable_addr(""));
        assert!(!is_gossipable_addr("cerbo.river.io")); // not an address at all
    }
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

    fn member(
        node_id: &str,
        actor: &str,
        kind: crate::members::MemberKind,
    ) -> crate::members::Member {
        crate::members::Member {
            node_id: node_id.into(),
            label: node_id.into(),
            kind,
            os: String::new(),
            os_version: String::new(),
            actor: actor.into(),
            detail: String::new(),
            first_seen: 0,
            last_seen: NOW,
            online: true,
            familiar_version: String::new(),
            tools: 0,
            patterns: 0,
            addr: String::new(),
            relationship: String::new(),
            ai: false,
            trust: "trusted".into(),
            status: "online".into(),
            session_start: 0,
            total_online_secs: 0,
            interactive: false,
            human: String::new(),
            present_human: String::new(),
            present_since: 0,
            present_confidence: 0.0,
            present_via: String::new(),
            lat: 0.0,
            lon: 0.0,
            geo_device: false,
            attached: Vec::new(),
            attached_to: String::new(),
            connectivity: String::new(),
        }
    }

    #[test]
    fn edges_mesh_full_peers_attribute_devices_and_read_delegation() {
        use crate::members::MemberKind::*;
        let members = vec![
            member("aaaa1111", "", SelfNode),
            member("bbbb2222", "", GossipPeer), // another full peer
            member("cccc3333", "ipad:ian", DevicePeer),
            member("dddd4444", "watch:ian", DeviceAgent),
        ];
        // The iPad delegated a theory that self's executor tested (origin = iPad's node prefix).
        let obs = vec![Observation::new(
            "familiar",
            "reasons",
            "theory",
            "testing a theory delegated by cccc3333 — 'gc pauses'",
            "cycle",
            NOW,
            0.8,
        )];
        let edges = mesh_edges(&members, &obs, "aaaa1111");
        let has = |from: &str, to: &str, kind: &str| {
            edges
                .iter()
                .any(|e| e.from == from && e.to == to && e.kind == kind)
        };
        // Full peers (self + gossip) mesh with each other — not through a hub.
        assert!(
            has("aaaa1111", "bbbb2222", "gossip"),
            "full peers form the gossip mesh"
        );
        // The watch reaches the mesh through its phone/iPad, not self.
        assert!(
            has("dddd4444", "cccc3333", "attribution"),
            "watch attributed to the iPad"
        );
        // The device peer with a parent is NOT also linked to self.
        assert!(
            !has("dddd4444", "aaaa1111", "gossip"),
            "attributed sub-device skips the self link"
        );
        // The delegation edge runs iPad -> executor (self), a real workload handoff.
        assert!(
            has("cccc3333", "aaaa1111", "delegation"),
            "delegation origin -> executor"
        );
    }

    fn fresh(tag: &str) -> PathBuf {
        let p =
            std::env::temp_dir().join(format!("familiar_worldview_{}_{tag}", std::process::id()));
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
        let cred =
            group::create_group(&host, &host_node, "river", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        open_gate(&host, true);
        let device = NodeKey::load_or_mint(&fresh(&format!("dev_{tag}")), "iPad").unwrap();
        (host, cred, device)
    }

    fn signed_request(
        cred: &GroupCredential,
        device: &NodeKey,
        ts: i64,
        nonce: &str,
    ) -> (Vec<u8>, String) {
        let id = device.identity();
        let membership = cred
            .mint_membership(&id.node_id, &id.pubkey, NOW, DEFAULT_CERT_TTL_SECS)
            .unwrap();
        let req = ViewRequest {
            node: id,
            membership,
            ts,
            nonce: nonce.into(),
            client_version: String::new(),
            os_version: String::new(),
            lat: 0.0,
            lon: 0.0,
        };
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
            Observation::new(
                "ian",
                "asked",
                "the familiar for help",
                "",
                "local",
                NOW,
                0.9,
            ),
        )
        .unwrap();

        // Reading is membership; SEEING the household is standing (ADR-0015 admits automatically,
        // so the two are not the same fact). Put this reader on the roll to get the real snapshot.
        grant_full_standing(&host, &device.node_id());

        let (raw, sig) = signed_request(&cred, &device, NOW, "v1");
        let view = read_worldview(&host, &raw, &sig, NOW, &ring(), "192.168.1.9").unwrap();
        assert_eq!(view.group_label, "river");
        assert_eq!(view.observation_count, 1);
        assert_eq!(view.recent.len(), 1);
        assert_eq!(view.recent[0].object, "the familiar for help");
    }

    /// Put a node on the standing roll, so it reads the household's real worldview.
    fn grant_full_standing(dir: &std::path::Path, node_id: &str) {
        let roll = crate::standing::StandingRoll {
            full: vec![node_id.to_string()],
            ..Default::default()
        };
        std::fs::write(
            dir.join(crate::standing::STANDING_FILE),
            serde_json::to_vec(&roll).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn an_arrival_appears_in_the_welcome_window_and_is_projected_for_guests() {
        // ADR-0026 §4: who is new, last 24 hours, greeted — and a guest reader sees the
        // arrivals' SHAPE (kinds, times, states) with the names taken out.
        let (host, cred, device) = setup("arrivals");

        // A stranger knocks: instantly a guest, instantly an arrival.
        let stranger = NodeKey::load_or_mint(&fresh("arrivals_stranger"), "Kali-Jeff").unwrap();
        let req = crate::enroll::EnrollRequest {
            node: stranger.identity(),
            attestation: crate::enroll::Attestation {
                laws_version: crate::enroll::LAWS_VERSION,
                statement: "I accept the Three Laws.".into(),
                ts: NOW,
            },
            nonce: "arr1".into(),
            ts: NOW,
        };
        let raw = serde_json::to_vec(&req).unwrap();
        let sig = stranger.sign(&raw);
        crate::enroll::submit_request(&host, &raw, &sig, NOW).unwrap();

        // A full-standing reader sees the arrivals as they are.
        grant_full_standing(&host, &device.node_id());
        let (raw, sig) = signed_request(&cred, &device, NOW, "arrv1");
        let view = read_worldview(&host, &raw, &sig, NOW, &ring(), "192.168.1.9").unwrap();
        assert!(
            view.arrivals.iter().any(|a| a.node_id == stranger.node_id() && a.state == "guest"),
            "the knocking stranger is an arrival: {:?}",
            view.arrivals
        );
        assert!(
            view.arrivals.iter().any(|a| a.state == "member"),
            "the founder is an arrival too (founding admits)"
        );

        // A guest reader: same list shape, no names.
        let guest_dev = NodeKey::load_or_mint(&fresh("arrivals_guestdev"), "visitor").unwrap();
        let (raw, sig) = signed_request(&cred, &guest_dev, NOW, "arrv2");
        let gview = read_worldview(&host, &raw, &sig, NOW, &ring(), "192.168.1.9").unwrap();
        assert!(!gview.arrivals.is_empty());
        for a in &gview.arrivals {
            assert!(
                a.label.starts_with("peer-"),
                "an arrival label reached a guest unprojected: {:?}",
                a.label
            );
            assert!(
                a.handle.is_empty() || a.handle == "someone",
                "an arrival handle leaked to a guest: {:?}",
                a.handle
            );
        }
    }

    #[test]
    fn a_member_not_on_the_standing_roll_reads_a_guest_view() {
        // The case that matters: an auto-admitted stranger. It is a real member — the read
        // SUCCEEDS, it is not an error — but it must not learn who lives here.
        let (host, cred, device) = setup("guest");
        observation::record(
            &host,
            Observation::new(
                "ian",
                "asked",
                "the familiar for help",
                "at the galley table",
                "local",
                NOW,
                0.9,
            ),
        )
        .unwrap();

        // No standing.json at all — default deny.
        let (raw, sig) = signed_request(&cred, &device, NOW, "v1");
        let view = read_worldview(&host, &raw, &sig, NOW, &ring(), "192.168.1.9").unwrap();

        // The shape is intact and honest…
        assert_eq!(view.observation_count, 1);
        assert_eq!(view.recent.len(), 1);
        assert_eq!(
            view.recent[0].ts, NOW,
            "timestamps must survive — Ian asked for this"
        );
        assert_eq!(view.recent[0].action, "asked", "the cadence stays legible");

        // …and the people are gone.
        assert_ne!(view.group_label, "river");
        assert_ne!(view.recent[0].object, "the familiar for help");
        assert!(view.recent[0].context.is_empty());
        assert!(
            !view.recent[0].actor.contains("ian"),
            "actor must not name a human, got {:?}",
            view.recent[0].actor
        );
        // The routing addressee and a goal's human owner both name a person, so both must go.
        assert!(
            view.question_owner.is_empty(),
            "question_owner names who is being asked and must not reach a guest: {:?}",
            view.question_owner
        );
        for g in &view.goals {
            assert!(
                g.owner_human.is_empty(),
                "goal owner_human leaked to a guest: {:?}",
                g.owner_human
            );
        }
        for m in &view.members {
            assert!(
                !m.human.contains("ian"),
                "member.human leaked: {:?}",
                m.human
            );
            assert!(
                !m.present_human.contains("ian"),
                "member.present_human leaked: {:?}",
                m.present_human
            );
            assert!(m.addr.is_empty(), "member.addr leaked: {:?}", m.addr);
        }
    }

    #[test]
    fn new_sensor_gates_round_trip_through_the_worldview() {
        // A peer reading the worldview must see the new sensor gates' real state — off by
        // default, and reflecting exactly what's open once a human opens one.
        let (host, cred, device) = setup("sensor_gates");
        let mut b = familiar_kernel::boundary::Boundary::closed();
        b.allow_mesh = true;
        b.allow_microphone = true;
        b.allow_face_recognition = true;
        // location/motion/network_discovery stay off, to confirm they're independent
        std::fs::write(host.join("boundary.json"), serde_json::to_vec(&b).unwrap()).unwrap();

        let (raw, sig) = signed_request(&cred, &device, NOW, "v1");
        let view = read_worldview(&host, &raw, &sig, NOW, &ring(), "192.168.1.9").unwrap();
        assert!(view.gates.microphone);
        assert!(view.gates.face_recognition);
        assert!(!view.gates.location);
        assert!(!view.gates.motion);
        assert!(!view.gates.network_discovery);

        // And the wire format really carries them — not just the in-memory struct.
        let json = serde_json::to_string(&view).unwrap();
        let back: Worldview = serde_json::from_str(&json).unwrap();
        assert!(back.gates.microphone && back.gates.face_recognition);
    }

    #[test]
    fn the_actuate_gate_round_trips_through_the_worldview() {
        let (host, cred, device) = setup("actuate_gate");
        let mut b = familiar_kernel::boundary::Boundary::closed();
        b.allow_mesh = true;
        b.allow_actuate = true;
        std::fs::write(host.join("boundary.json"), serde_json::to_vec(&b).unwrap()).unwrap();
        let (raw, sig) = signed_request(&cred, &device, NOW, "v1");
        let view = read_worldview(&host, &raw, &sig, NOW, &ring(), "192.168.1.9").unwrap();
        assert!(view.gates.actuate);
        let json = serde_json::to_string(&view).unwrap();
        let back: Worldview = serde_json::from_str(&json).unwrap();
        assert!(back.gates.actuate, "the wire format carries it");
    }

    #[test]
    fn asserted_advertise_hosts_lead_the_hosts_list() {
        let (host, cred, device) = setup("advertise");
        std::fs::create_dir_all(host.join("mesh")).unwrap();
        std::fs::write(
            host.join(crate::config::CONFIG_FILE),
            r#"{"advertise_hosts":["lighthouse.river.io","203.0.113.7"]}"#,
        )
        .unwrap();
        let (raw, sig) = signed_request(&cred, &device, NOW, "v1");
        let view = read_worldview(&host, &raw, &sig, NOW, &ring(), "192.168.1.9").unwrap();
        // Human-asserted addresses come first, verbatim — DNS names included; the
        // interface-derived addresses follow.
        assert_eq!(&view.hosts[..2], ["lighthouse.river.io", "203.0.113.7"]);
    }

    #[test]
    fn a_replayed_request_is_rejected() {
        let (host, cred, device) = setup("replay");
        let (raw, sig) = signed_request(&cred, &device, NOW, "v1");
        let r = ring();
        assert!(read_worldview(&host, &raw, &sig, NOW, &r, "10.0.0.5").is_ok());
        let err = read_worldview(&host, &raw, &sig, NOW, &r, "10.0.0.5").unwrap_err();
        assert!(matches!(err, Error::Untrusted(m) if m.contains("replay")));
    }

    #[test]
    fn reading_the_worldview_promotes_the_reader_to_a_peer() {
        let (host, cred, device) = setup("promote");
        // Before: not in the peer roster (a fresh member that has only ever read).
        assert!(crate::transport::load_peers(&host).is_empty());
        let (raw, sig) = signed_request(&cred, &device, NOW, "v1");
        read_worldview(&host, &raw, &sig, NOW, &ring(), "192.168.1.42").unwrap();
        // After: it appears as a peer, at the address it connected from — no longer a mere agent.
        let peers = crate::transport::load_peers(&host);
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].node_id, device.node_id());
        assert_eq!(peers[0].addr, "192.168.1.42");
        assert_eq!(peers[0].label, "iPad");
    }

    #[test]
    fn a_non_member_is_refused() {
        let (host, _cred, device) = setup("nonmember");
        // A different group mints the device's cert — it won't verify under the host's group key.
        let other = group::create_group(
            &fresh("othergrp"),
            &NodeKey::load_or_mint(&fresh("othernode"), "h2").unwrap(),
            "other",
            NOW,
            DEFAULT_CERT_TTL_SECS,
        )
        .unwrap();
        let (raw, sig) = signed_request(&other, &device, NOW, "v1");
        let err = read_worldview(&host, &raw, &sig, NOW, &ring(), "192.168.1.9").unwrap_err();
        assert!(matches!(err, Error::Untrusted(_)));
    }

    #[test]
    fn a_closed_gate_refuses() {
        let (host, cred, device) = setup("gate");
        open_gate(&host, false);
        let (raw, sig) = signed_request(&cred, &device, NOW, "v1");
        let err = read_worldview(&host, &raw, &sig, NOW, &ring(), "192.168.1.9").unwrap_err();
        assert!(matches!(err, Error::Untrusted(m) if m.contains("gate closed")));
    }
}
