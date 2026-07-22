//! Transport — the async half. **Does IO, never constitutional merge.**
//!
//! A background tokio runtime (spawned once at daemon start via [`spawn`]) does four
//! things while `allow_mesh` is open:
//! - **serves** `/mesh/hello`, `/mesh/brief`, `/mesh/tool/{id}` — bound whenever the gate
//!   is open, *including before any group exists*, so an ungrouped node is discoverable
//!   and can take part in auto-formation,
//! - **discovers** peers two ways: tailnet enumeration (`tailscale status --json`,
//!   read-only) and, when `lan_discovery` is on, UDP broadcast beacons on the local
//!   network — plus any `static_peers`. Discovery never grants trust; certs do,
//! - **gossips concurrently**: POSTs our brief to every discovered peer in parallel (one
//!   slow or dead peer no longer stalls the rest), taking theirs in return,
//! - **verifies at ingress**: every inbound brief's membership cert + node signature are
//!   checked against our group key *before* it touches disk; junk is dropped.
//!
//! Enrollment no longer depends on one founder host: a member that cannot mint (a
//! covenant-joined node) **relays** enroll requests/status to mint-capable peers, and two
//! ungrouped `auto_peer` nodes **auto-form** a group (lowest node id creates it and opens
//! a bounded invite window) — so the mesh establishes from any two nodes.
//!
//! What survives is written to `mesh/inbox/<node_id>.json` and referenced tool bodies are
//! pre-fetched (content-addressed) to `mesh/inbox_tools/<sha>.script`. The transport
//! **never** writes `tools.jsonl` / `observations.jsonl` / `patterns.jsonl` — that is the
//! in-tick merge's job ([`crate::merge`], Phase 4), so every federated change flows through
//! the same auditable metabolism and the boundary. If `allow_mesh` closes, the loop tears
//! the server down and idles.

use crate::brief::{verify_brief, MeshBrief};
use crate::config::{self, MeshConfig};
use crate::group::{self, GroupCredential};
use crate::{sha256_hex, Result};
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::{TcpListener, TcpStream};

// ---- on-disk mesh artifacts (also read by merge.rs) --------------------------------

/// Our current signed brief, written by the tick; served + gossiped by transport.
pub const OUTBOX_FILE: &str = "mesh/outbox.json";
/// Verified inbound briefs, one file per peer node id.
pub const INBOX_DIR: &str = "mesh/inbox";
/// Pre-fetched, content-addressed tool bodies awaiting in-tick merge.
pub const INBOX_TOOLS_DIR: &str = "mesh/inbox_tools";
/// Connected-peer roster (for Glass).
pub const PEERS_FILE: &str = "mesh/peers.json";
/// One-line human status (for Glass), like `connect_status.txt`.
pub const STATUS_FILE: &str = "mesh/status.txt";

/// Current unix seconds (real clock — this is runtime, not a Workflow script).
pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A peer as last seen — surfaced in Glass, refreshed each successful exchange.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PeerRecord {
    pub node_id: String,
    pub label: String,
    pub addr: String,
    pub group_id: String,
    pub last_seen: i64,
    pub tools_offered: usize,
    pub patterns_offered: usize,
    /// OS the peer reported in its brief (gossip peers). Empty for device peers, whose family is
    /// derived from their actor namespace instead. `#[serde(default)]` so older rosters still load.
    #[serde(default)]
    pub os: String,
    /// CPU arch the peer reported. Empty for device peers.
    #[serde(default)]
    pub arch: String,
    /// When this node first joined the roster (unix secs) — the "date joined". 0 for pre-existing
    /// rows written before this field; backfilled to `last_seen` on the next sighting.
    #[serde(default)]
    pub first_seen: i64,
    /// The familiar build the peer runs (from its brief), or a device's app build (reported on its
    /// worldview read). Empty for older rows.
    #[serde(default)]
    pub familiar_version: String,
    /// The OS release the node reported ("iPadOS 26.1", "18.5"). Empty for older rows / nodes that
    /// don't report it. The OS *family* is still derived from the actor; this is the version detail.
    #[serde(default)]
    pub os_version: String,
    /// When the current continuous-online run began (unix secs). Reset whenever the peer
    /// reappears after a gap longer than its freshness window. 0 on pre-field rows.
    #[serde(default)]
    pub session_start: i64,
    /// Total seconds of *completed* online runs — the peer's cumulative time in the mesh,
    /// excluding the live session (add `now - session_start` while it's online).
    #[serde(default)]
    pub total_online_secs: i64,
    /// The peer has an interactive human at its console (from its brief; !headless).
    #[serde(default)]
    pub interactive: bool,
    /// The human handle that node serves, when its brief shares one (identity opt-in gated).
    #[serde(default)]
    pub human: String,
    /// Where the node is (decimal degrees) — from its brief (gossip peers) or its worldview
    /// reads (devices with GPS). 0/0 = unknown.
    #[serde(default)]
    pub lat: f64,
    #[serde(default)]
    pub lon: f64,
}

/// A gossip peer beacons every ~30s — two missed rounds plus slack and it's no longer "online".
pub const GOSSIP_FRESH_SECS: i64 = 120;

/// A running mesh transport. Dropping or calling [`MeshHandle::shutdown`] stops it.
pub struct MeshHandle {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl MeshHandle {
    /// Signal the transport to stop and wait for its thread to wind down.
    pub fn shutdown(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for MeshHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Spawn the mesh transport on its own background thread + tokio runtime. **Synchronous
/// entry point** — the daemon calls this once at startup; the returned handle lives for the
/// process. The loop self-gates on `allow_mesh` each cycle: it only binds + gossips while
/// the human has the boundary open, and idles otherwise, so opening the flag later (via
/// Glass) is picked up without a restart.
pub fn spawn(dir: impl Into<PathBuf>) -> MeshHandle {
    let dir = dir.into();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = stop.clone();
    let thread = std::thread::Builder::new()
        .name("familiar-mesh".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = write_status(&dir, &format!("mesh runtime failed: {e}"));
                    return;
                }
            };
            rt.block_on(supervisor(dir, stop_thread));
        })
        .expect("spawn mesh thread");
    MeshHandle {
        stop,
        thread: Some(thread),
    }
}

/// The gate check: transport runs only while the human-owned boundary permits it.
fn mesh_allowed(dir: &Path) -> bool {
    familiar_kernel::boundary::load(dir)
        .map(|b| b.allow_mesh)
        .unwrap_or(false)
}

/// Peers seen via LAN broadcast beacons: ip → (gossip_port, last_seen). Discovery only —
/// an entry here earns nothing a membership cert doesn't prove.
#[derive(Default)]
struct LanState {
    peers: std::sync::Mutex<std::collections::HashMap<String, (u16, i64)>>,
}

impl LanState {
    /// LAN peers seen within `max_age_secs`, as `ip:port` gossip addresses.
    fn addrs(&self, max_age_secs: i64) -> Vec<String> {
        let now = now_secs();
        self.peers
            .lock()
            .map(|m| {
                m.iter()
                    .filter(|(_, (_, seen))| now - seen <= max_age_secs)
                    .map(|(ip, (port, _))| format!("{ip}:{port}"))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Same peers as bare hosts (for enroll/hello, which take host + port apart).
    fn hosts(&self, max_age_secs: i64) -> Vec<String> {
        self.addrs(max_age_secs)
            .into_iter()
            .map(|a| a.split(':').next().unwrap_or_default().to_string())
            .collect()
    }
}

/// The supervisor loop: keep the server + LAN discovery up while the gate is open (with or
/// without a group), auto-join/auto-form when ungrouped, gossip each interval when enrolled,
/// tear down and idle when the boundary closes.
async fn supervisor(dir: PathBuf, stop: Arc<AtomicBool>) {
    let mut server: Option<tokio::task::JoinHandle<()>> = None;
    let mut bound_port: u16 = 0;
    let mut lan_task: Option<tokio::task::JoinHandle<()>> = None;
    let mut lan_bound: u16 = 0;
    let lan = Arc::new(LanState::default());

    loop {
        if stop.load(Ordering::SeqCst) {
            if let Some(s) = server.take() {
                s.abort();
            }
            if let Some(l) = lan_task.take() {
                l.abort();
            }
            return;
        }

        let cfg = config::load(&dir).unwrap_or_default();
        let interval = Duration::from_secs(cfg.gossip_interval_secs.max(1));
        let lan_window = (cfg.gossip_interval_secs.saturating_mul(3).max(90)) as i64;

        // Gate 1: the human-owned boundary.
        if !mesh_allowed(&dir) {
            if let Some(s) = server.take() {
                s.abort();
            }
            if let Some(l) = lan_task.take() {
                l.abort();
            }
            bound_port = 0;
            lan_bound = 0;
            let _ = write_status(&dir, "mesh idle — allow_mesh is off");
            sleep_or_stop(&stop, interval).await;
            continue;
        }

        // The server binds whenever the gate is open — before any group exists, too, so an
        // ungrouped node answers /mesh/hello (auto-formation needs mutual visibility) and can
        // receive an enroll grant. Every state-changing endpoint still requires a group/cert.
        if server.is_none() || bound_port != cfg.gossip_port {
            if let Some(s) = server.take() {
                s.abort();
            }
            match TcpListener::bind(("0.0.0.0", cfg.gossip_port)).await {
                Ok(listener) => {
                    bound_port = cfg.gossip_port;
                    let ctx = Arc::new(ServerCtx {
                        dir: dir.clone(),
                        seen: std::sync::Mutex::new(crate::observe::IngestGuard::default()),
                    });
                    server = Some(tokio::spawn(serve(listener, ctx)));
                }
                Err(e) => {
                    let _ =
                        write_status(&dir, &format!("mesh bind :{} failed: {e}", cfg.gossip_port));
                    sleep_or_stop(&stop, interval).await;
                    continue;
                }
            }
        }

        // LAN discovery beacons (second discovery path beside the tailnet).
        if cfg.lan_discovery {
            if lan_task.is_none() || lan_bound != cfg.lan_port {
                if let Some(l) = lan_task.take() {
                    l.abort();
                }
                lan_bound = cfg.lan_port;
                let our_id = node_id_of(&dir);
                lan_task = Some(tokio::spawn(lan_loop(
                    cfg.lan_port,
                    cfg.gossip_port,
                    our_id,
                    lan.clone(),
                    stop.clone(),
                )));
            }
        } else if let Some(l) = lan_task.take() {
            l.abort();
            lan_bound = 0;
        }

        // Gate 2: an enrolled group (a human handed us a credential, or auto-peer earned one).
        let cred = match group::load(&dir).ok().flatten() {
            Some(c) => c,
            None => {
                if cfg.auto_peer {
                    // Bootstrap 1 — join: ask every discovered host (tailnet + LAN + static) to
                    // admit us by covenant; first admission wins.
                    if auto_join_round(&dir, &cfg, lan.hosts(lan_window)).await > 0 {
                        let _ = write_status(
                            &dir,
                            "✓ auto-peer — admitted by covenant, joining the mesh",
                        );
                        continue; // skip the sleep so we start serving/gossiping immediately
                    }
                    // Bootstrap 2 — form: no group in reach. If another ungrouped auto_peer node is
                    // visible and we hold the lowest node id, create the group + open an invite
                    // window; the others join by covenant on their next round.
                    if auto_form_round(&dir, &cfg, lan.hosts(lan_window)).await {
                        let _ = write_status(
                            &dir,
                            "✓ auto-peer — no group in reach; formed one (invite window open)",
                        );
                        continue;
                    }
                    let _ = write_status(&dir, "mesh auto-peer — seeking a covenant…");
                } else {
                    let _ = write_status(&dir, "mesh waiting — no group enrolled yet");
                }
                sleep_or_stop(&stop, interval).await;
                continue;
            }
        };

        // One concurrent gossip round, then report the count of peers we're actually federating
        // with — fresh entries in peers.json in EITHER direction, not just this round's reach.
        let _ = gossip_round(&dir, &cfg, &cred, lan.addrs(lan_window)).await;
        let _ = write_status(
            &dir,
            &format!(
                "✓ mesh open (group {}) — {} peer(s) connected",
                short(&cred.group_id),
                count_connected(&dir, cfg.gossip_interval_secs)
            ),
        );

        sleep_or_stop(&stop, interval).await;
    }
}

/// This node's stable id (minting the key on first use). Empty string on failure.
fn node_id_of(dir: &Path) -> String {
    crate::node::NodeKey::load_or_mint(dir, "familiar")
        .map(|n| n.node_id())
        .unwrap_or_default()
}

// ---- LAN discovery (UDP broadcast beacons) ------------------------------------------

/// A discovery beacon. Presence only — carries no trust; certs do.
#[derive(Serialize, Deserialize)]
struct LanBeacon {
    familiar_mesh: u32,
    node_id: String,
    gossip_port: u16,
}

/// Parse a beacon datagram; `None` for junk or a foreign format (pure — unit-tested).
fn parse_beacon(bytes: &[u8]) -> Option<LanBeacon> {
    let b: LanBeacon = serde_json::from_slice(bytes).ok()?;
    if b.familiar_mesh != 1 || b.node_id.is_empty() {
        return None;
    }
    Some(b)
}

/// Broadcast our beacon and collect peers' — the LAN half of discovery. Own beacons are
/// filtered by node id (broadcast loops back). Socket errors end the task; the supervisor
/// respawns it next interval.
async fn lan_loop(
    lan_port: u16,
    gossip_port: u16,
    our_id: String,
    state: Arc<LanState>,
    stop: Arc<AtomicBool>,
) {
    let sock = match tokio::net::UdpSocket::bind(("0.0.0.0", lan_port)).await {
        Ok(s) => s,
        Err(_) => return,
    };
    if sock.set_broadcast(true).is_err() {
        return;
    }
    let beacon = match serde_json::to_vec(&LanBeacon {
        familiar_mesh: 1,
        node_id: our_id.clone(),
        gossip_port,
    }) {
        Ok(b) => b,
        Err(_) => return,
    };
    let mut buf = [0u8; 512];
    let mut next_send = std::time::Instant::now();
    loop {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        if std::time::Instant::now() >= next_send {
            let _ = sock.send_to(&beacon, ("255.255.255.255", lan_port)).await;
            next_send = std::time::Instant::now() + Duration::from_secs(15);
        }
        // A timeout is normal (it paces the send check); transient recv errors are skipped.
        if let Ok(Ok((n, from))) =
            tokio::time::timeout(Duration::from_secs(1), sock.recv_from(&mut buf)).await
        {
            if let Some(b) = parse_beacon(&buf[..n]) {
                if b.node_id != our_id {
                    if let Ok(mut m) = state.peers.lock() {
                        m.insert(from.ip().to_string(), (b.gossip_port, now_secs()));
                    }
                }
            }
        }
    }
}

async fn sleep_or_stop(stop: &Arc<AtomicBool>, dur: Duration) {
    // Wake early if asked to stop, so shutdown isn't blocked by a long interval.
    let step = Duration::from_millis(200);
    let mut elapsed = Duration::ZERO;
    while elapsed < dur {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        tokio::time::sleep(step.min(dur - elapsed)).await;
        elapsed += step;
    }
}

// ---- server -------------------------------------------------------------------------

struct ServerCtx {
    dir: PathBuf,
    /// Anti-replay memory for `/mesh/observe`, shared across connections. In-process only —
    /// a restart forgets, but the `ts` window bounds a replay to the same short window anyway.
    seen: std::sync::Mutex<crate::observe::IngestGuard>,
}

async fn serve(listener: TcpListener, ctx: Arc<ServerCtx>) {
    loop {
        let (stream, remote) = match listener.accept().await {
            Ok(pair) => pair,
            Err(_) => continue,
        };
        let peer_ip = remote.ip().to_string();
        let io = TokioIo::new(stream);
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let svc = service_fn(move |req| handle(req, ctx.clone(), peer_ip.clone()));
            let _ = hyper::server::conn::http1::Builder::new()
                .serve_connection(io, svc)
                .await;
        });
    }
}

fn text(status: StatusCode, body: impl Into<Bytes>) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .body(Full::new(body.into()))
        .unwrap()
}

async fn handle(
    req: Request<hyper::body::Incoming>,
    ctx: Arc<ServerCtx>,
    peer_ip: String,
) -> std::result::Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let dir = ctx.dir.clone();

    let resp = match (method, path.as_str()) {
        (Method::GET, "/mesh/hello") => hello(&dir),
        (Method::POST, "/mesh/brief") => {
            let bytes = match collect(req).await {
                Ok(b) => b,
                Err(_) => return Ok(text(StatusCode::BAD_REQUEST, "bad body")),
            };
            recv_brief(&dir, &bytes, &peer_ip)
        }
        (Method::POST, "/mesh/observe") => {
            // The signature covers the raw body, so grab the header before the body is consumed.
            let sig = req
                .headers()
                .get("x-familiar-sig")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            let bytes = match collect(req).await {
                Ok(b) => b,
                Err(_) => return Ok(text(StatusCode::BAD_REQUEST, "bad body")),
            };
            recv_observe(&dir, &bytes, &sig, &ctx.seen)
        }
        (Method::POST, "/mesh/worldview") => {
            let sig = req
                .headers()
                .get("x-familiar-sig")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            let bytes = match collect(req).await {
                Ok(b) => b,
                Err(_) => return Ok(text(StatusCode::BAD_REQUEST, "bad body")),
            };
            recv_worldview(&dir, &bytes, &sig, &ctx.seen, &peer_ip)
        }
        (Method::GET, "/local/worldview") => {
            // A peer's own console (e.g. the macOS SwiftUI app) reads the worldview of the node
            // running on the same machine, without a mesh signature — it's reading itself, not a
            // remote peer. Strictly loopback-only: nothing leaves the machine.
            if peer_ip != "127.0.0.1" && peer_ip != "::1" {
                text(StatusCode::FORBIDDEN, "local only")
            } else {
                local_worldview(&dir)
            }
        }
        (Method::POST, "/local/answer") => {
            if peer_ip != "127.0.0.1" && peer_ip != "::1" {
                text(StatusCode::FORBIDDEN, "local only")
            } else {
                match collect(req).await {
                    Ok(b) => local_answer(&dir, &b),
                    Err(_) => text(StatusCode::BAD_REQUEST, "bad body"),
                }
            }
        }
        (Method::POST, "/local/gate") => {
            if peer_ip != "127.0.0.1" && peer_ip != "::1" {
                text(StatusCode::FORBIDDEN, "local only")
            } else {
                match collect(req).await {
                    Ok(b) => local_gate(&dir, &b),
                    Err(_) => text(StatusCode::BAD_REQUEST, "bad body"),
                }
            }
        }
        (Method::POST, "/mesh/enroll-request") => {
            let sig = req
                .headers()
                .get("x-familiar-sig")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            let relayed = req.headers().contains_key("x-familiar-relayed");
            let bytes = match collect(req).await {
                Ok(b) => b,
                Err(_) => return Ok(text(StatusCode::BAD_REQUEST, "bad body")),
            };
            enroll_or_relay(&dir, bytes, sig, relayed).await
        }
        (Method::GET, p) if p.starts_with("/mesh/enroll-status/") => {
            let node_id = p.trim_start_matches("/mesh/enroll-status/").to_string();
            let relayed = req.headers().contains_key("x-familiar-relayed");
            enroll_status_or_relay(&dir, node_id, relayed).await
        }
        (Method::GET, p) if p.starts_with("/mesh/tool/") => {
            let id = p.trim_start_matches("/mesh/tool/");
            serve_tool(&dir, id)
        }
        _ => text(StatusCode::NOT_FOUND, "not found"),
    };
    Ok(resp)
}

async fn collect(req: Request<hyper::body::Incoming>) -> Result<Bytes> {
    Ok(req
        .into_body()
        .collect()
        .await
        .map_err(|e| crate::Error::Malformed(format!("body: {e}")))?
        .to_bytes())
}

/// `GET /mesh/hello` → who we are + which group (cheap same-group precheck). An ungrouped
/// node answers too, with an empty `group_id` — that visibility is what lets two fresh
/// nodes find each other and auto-form (identity is public by design: node ids are
/// self-certifying fingerprints, and hello grants nothing).
fn hello(dir: &Path) -> Response<Full<Bytes>> {
    let (node_id, group_id, label) = match group::load(dir).ok().flatten() {
        Some(cred) => (
            cred.membership.node_id.clone(),
            cred.group_id.clone(),
            cred.label.clone(),
        ),
        None => (node_id_of(dir), String::new(), String::new()),
    };
    if node_id.is_empty() {
        return text(StatusCode::SERVICE_UNAVAILABLE, "no node identity");
    }
    let body = serde_json::json!({
        "node_id": node_id,
        "group_id": group_id,
        "label": label,
    });
    text(StatusCode::OK, body.to_string())
}

/// `POST /mesh/brief` → verify at ingress, stash if trusted, answer with our own brief.
fn recv_brief(dir: &Path, bytes: &[u8], peer_ip: &str) -> Response<Full<Bytes>> {
    match ingest_brief(dir, bytes, peer_ip) {
        Ok(()) => {
            // Hand our brief back so a single round exchanges both directions.
            match std::fs::read(dir.join(OUTBOX_FILE)) {
                Ok(b) => text(StatusCode::OK, b),
                Err(_) => text(StatusCode::NO_CONTENT, ""),
            }
        }
        Err(crate::Error::Untrusted(m)) => text(StatusCode::FORBIDDEN, m),
        Err(_) => text(StatusCode::BAD_REQUEST, "bad brief"),
    }
}

/// Verify an inbound brief against our group and, if trusted, write it to the inbox and
/// record the peer. Returns `Untrusted` if the cert/signature fail — the caller answers 403.
pub(crate) fn ingest_brief(dir: &Path, bytes: &[u8], addr: &str) -> Result<()> {
    let cred = group::load(dir)?.ok_or_else(|| crate::Error::Untrusted("no group".into()))?;
    let brief: MeshBrief = serde_json::from_slice(bytes)?;
    let revoked = group::load_revoked(dir).unwrap_or_default();
    let gk = cred.verifying_key()?;
    verify_brief(&brief, &gk, &cred.group_id, now_secs(), &revoked)?;

    // Trusted: write to inbox (one file per peer, latest wins).
    let inbox = dir.join(INBOX_DIR);
    std::fs::create_dir_all(&inbox)?;
    let node_id = brief.body.node.node_id.clone();
    std::fs::write(
        inbox.join(format!("{node_id}.json")),
        serde_json::to_vec_pretty(&brief)?,
    )?;
    upsert_peer(dir, &brief, addr)?;
    Ok(())
}

/// `POST /mesh/observe` → verify a device's signed observation batch and, if trusted + fresh,
/// append it to the store. `sig` is the `X-Familiar-Sig` header (ed25519 over the raw body).
/// 200 + count on success; 409 on a replayed nonce; 403 if untrusted; 400 if malformed.
fn recv_observe(
    dir: &Path,
    bytes: &[u8],
    sig: &str,
    ring: &std::sync::Mutex<crate::observe::IngestGuard>,
) -> Response<Full<Bytes>> {
    match crate::observe::ingest_observations(dir, bytes, sig, now_secs(), ring) {
        Ok(n) => text(StatusCode::OK, format!("recorded {n}")),
        Err(crate::Error::Untrusted(m)) if m.contains("replay") => text(StatusCode::CONFLICT, m),
        Err(crate::Error::Untrusted(m)) => text(StatusCode::FORBIDDEN, m),
        Err(_) => text(StatusCode::BAD_REQUEST, "bad batch"),
    }
}

/// `GET /local/worldview` → the host's own console reads the worldview, no mesh signature (loopback
/// gated by the caller). 200 + JSON, 503 if no group yet.
fn local_worldview(dir: &Path) -> Response<Full<Bytes>> {
    let Some(cred) = group::load(dir).ok().flatten() else {
        return text(StatusCode::SERVICE_UNAVAILABLE, "no group");
    };
    match crate::worldview::assemble_worldview(dir, &cred, now_secs()) {
        Ok(view) => match serde_json::to_vec(&view) {
            Ok(body) => text(StatusCode::OK, body),
            Err(_) => text(StatusCode::INTERNAL_SERVER_ERROR, "encode"),
        },
        Err(_) => text(StatusCode::INTERNAL_SERVER_ERROR, "assemble"),
    }
}

/// `POST /local/answer {"text": "..."}` → the human at this machine speaks to the familiar. Records
/// a served-facing observation and retires the current question. Loopback-gated by the caller.
fn local_answer(dir: &Path, body: &[u8]) -> Response<Full<Bytes>> {
    let text_val = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("text").and_then(|s| s.as_str()).map(String::from))
        .unwrap_or_default();
    let t = text_val.trim();
    if t.is_empty() {
        return text(StatusCode::BAD_REQUEST, "empty");
    }
    let obs = familiar_kernel::observation::Observation::new(
        "ian",
        "told the familiar",
        t,
        "console",
        "local",
        now_secs(),
        1.0,
    );
    let _ = familiar_kernel::observation::record(dir, obs);
    // Retire the open question so the cycle re-coordinates.
    let _ = std::fs::write(dir.join("question.txt"), "");
    let _ = std::fs::write(dir.join("active_question.txt"), "");
    text(StatusCode::OK, "ok")
}

/// `POST /local/gate {"gate":"allow_execute","open":true}` → the human at this machine opens or
/// closes a boundary gate through their own instrument (the same act the Glass performs). This is a
/// local human boundary-write, not the autonomous cycle. Loopback-gated by the caller.
fn local_gate(dir: &Path, body: &[u8]) -> Response<Full<Bytes>> {
    let v = match serde_json::from_slice::<serde_json::Value>(body) {
        Ok(v) => v,
        Err(_) => return text(StatusCode::BAD_REQUEST, "bad json"),
    };
    let gate = v.get("gate").and_then(|s| s.as_str()).unwrap_or("");
    let open = v.get("open").and_then(|b| b.as_bool()).unwrap_or(false);
    // Automatic-peering switches live in mesh/config.json, not the boundary — but the console reaches
    // them through the same gate control. Intercept them here (a local human write, like the gates).
    if gate == "auto_peer" || gate == "auto_accept" {
        let mut cfg = config::load(dir).unwrap_or_default();
        match gate {
            "auto_peer" => cfg.auto_peer = open,
            "auto_accept" => cfg.auto_accept_enrollments = open,
            _ => unreachable!(),
        }
        return match serde_json::to_vec_pretty(&cfg) {
            Ok(json) => {
                let _ = std::fs::create_dir_all(dir.join("mesh"));
                let _ = std::fs::write(dir.join(config::CONFIG_FILE), json);
                text(StatusCode::OK, "ok")
            }
            Err(_) => text(StatusCode::INTERNAL_SERVER_ERROR, "encode"),
        };
    }
    let mut b = familiar_kernel::boundary::load(dir)
        .unwrap_or_else(|_| familiar_kernel::boundary::Boundary::closed());
    match gate {
        "allow_llm" => b.allow_llm = open,
        "allow_camera" => b.allow_camera = open,
        "allow_network" => b.allow_network = open,
        "allow_mesh" => b.allow_mesh = open,
        "allow_execute" => b.allow_execute = open,
        "allow_authored_execute" => b.allow_authored_execute = open,
        "allow_agent" => b.allow_agent = open,
        "allow_tool_install" => b.allow_tool_install = open,
        "allow_self_upgrade" => b.allow_self_upgrade = open,
        _ => return text(StatusCode::BAD_REQUEST, "unknown gate"),
    }
    if b.phase == "closed" && open {
        b.phase = "phase-1".to_string();
    }
    match serde_json::to_string_pretty(&b) {
        Ok(json) => {
            let _ = std::fs::write(dir.join(familiar_kernel::boundary::BOUNDARY_FILE), json);
            text(StatusCode::OK, "ok")
        }
        Err(_) => text(StatusCode::INTERNAL_SERVER_ERROR, "encode"),
    }
}

/// `POST /mesh/worldview` → a member device asks for a snapshot of what the familiar knows.
/// Signed and membership-bearing (verified like an observe batch); 200 + JSON worldview, 409
/// replay, 403 untrusted, 400 malformed. The read seam that lets an iPad be a peer console,
/// not just a sensor.
fn recv_worldview(
    dir: &Path,
    bytes: &[u8],
    sig: &str,
    ring: &std::sync::Mutex<crate::observe::IngestGuard>,
    peer_ip: &str,
) -> Response<Full<Bytes>> {
    match crate::worldview::read_worldview(dir, bytes, sig, now_secs(), ring, peer_ip) {
        Ok(view) => match serde_json::to_vec(&view) {
            Ok(body) => text(StatusCode::OK, body),
            Err(_) => text(StatusCode::INTERNAL_SERVER_ERROR, "encode"),
        },
        Err(crate::Error::Untrusted(m)) if m.contains("replay") => text(StatusCode::CONFLICT, m),
        Err(crate::Error::Untrusted(m)) => text(StatusCode::FORBIDDEN, m),
        Err(_) => text(StatusCode::BAD_REQUEST, "bad request"),
    }
}

/// Mint-capable peers this member can relay an enrollment to: roster addresses first (already
/// `ip:port`), then the tailnet, deduped and bounded. The relay carries the joiner's own raw
/// body + signature, so a relaying node can't alter what's admitted — it is a courier, not an
/// authority.
fn relay_targets(dir: &Path, cfg: &MeshConfig) -> Vec<String> {
    let mut targets: Vec<String> = load_peers(dir)
        .into_iter()
        .filter(|p| !p.addr.is_empty())
        .map(|p| p.addr)
        .collect();
    for p in enumerate_peers().into_iter().filter(|p| p.online) {
        targets.push(with_port(&p.ip, cfg.gossip_port));
    }
    targets.sort();
    targets.dedup();
    targets.truncate(8);
    targets
}

/// Enrollment, from **any** member node: a mint-capable node admits directly; a covenant-joined
/// node (no group secret) **relays** the signed request to mint-capable peers and passes the
/// answer back — so a joiner can approach whichever node it can reach, not one founder host.
/// `relayed` (the `X-Familiar-Relayed` header) stops a relay from being re-relayed (no loops).
async fn enroll_or_relay(
    dir: &Path,
    bytes: Bytes,
    sig: String,
    relayed: bool,
) -> Response<Full<Bytes>> {
    let can_mint = group::load(dir)
        .ok()
        .flatten()
        .map(|c| c.can_mint())
        .unwrap_or(false);
    if can_mint {
        return recv_enroll_request(dir, &bytes, &sig);
    }
    if relayed {
        // One hop only: a relayed request that lands on another non-minting node stops here —
        // filing it as pending would be a dead end (approval could never mint).
        return text(StatusCode::FORBIDDEN, "relay target cannot mint");
    }
    if group::load(dir).ok().flatten().is_none() {
        return recv_enroll_request(dir, &bytes, &sig); // yields the honest "no group" 403
    }
    let cfg = config::load(dir).unwrap_or_default();
    for target in relay_targets(dir, &cfg) {
        let headers = [
            ("x-familiar-sig", sig.as_str()),
            ("x-familiar-relayed", "1"),
        ];
        if let Ok(resp) = http_send(
            &target,
            Method::POST,
            "/mesh/enroll-request",
            Some(bytes.to_vec()),
            &headers,
        )
        .await
        {
            if resp.status == StatusCode::OK || resp.status == StatusCode::ACCEPTED {
                return text(resp.status, resp.body);
            }
        }
    }
    text(
        StatusCode::FORBIDDEN,
        "no admitting peer reachable from this node",
    )
}

/// Status polling, from any member node: answer locally when we know the request; otherwise
/// relay the poll to mint-capable peers (same one-hop guard), so a joiner can poll whichever
/// node it submitted through even if a different node holds the grant.
async fn enroll_status_or_relay(
    dir: &Path,
    node_id: String,
    relayed: bool,
) -> Response<Full<Bytes>> {
    let local = enroll_status(dir, &node_id);
    let unknown = local.status() == StatusCode::NOT_FOUND;
    let can_mint = group::load(dir)
        .ok()
        .flatten()
        .map(|c| c.can_mint())
        .unwrap_or(false);
    if !unknown || can_mint || relayed {
        return local;
    }
    let cfg = config::load(dir).unwrap_or_default();
    for target in relay_targets(dir, &cfg) {
        let headers = [("x-familiar-relayed", "1")];
        if let Ok(resp) = http_send(
            &target,
            Method::GET,
            &format!("/mesh/enroll-status/{node_id}"),
            None,
            &headers,
        )
        .await
        {
            if resp.status == StatusCode::OK || resp.status == StatusCode::ACCEPTED {
                return text(resp.status, resp.body);
            }
        }
    }
    local
}

/// `POST /mesh/enroll-request` → a node attests to the Laws and asks to join. `sig` is the
/// `X-Familiar-Sig` header (ed25519 over the raw body). 200 + the minted Grant if an invite
/// window auto-approved it; 202 + the pending record otherwise; 403 untrusted; 400 malformed.
fn recv_enroll_request(dir: &Path, bytes: &[u8], sig: &str) -> Response<Full<Bytes>> {
    match crate::enroll::submit_request(dir, bytes, sig, now_secs()) {
        Ok(crate::enroll::Submitted::Granted(g)) => match serde_json::to_vec(&*g) {
            Ok(b) => text(StatusCode::OK, b),
            Err(_) => text(StatusCode::INTERNAL_SERVER_ERROR, "grant encode"),
        },
        Ok(crate::enroll::Submitted::Pending(p)) => match serde_json::to_vec(&p) {
            Ok(b) => text(StatusCode::ACCEPTED, b),
            Err(_) => text(StatusCode::INTERNAL_SERVER_ERROR, "pending encode"),
        },
        Err(crate::Error::Untrusted(m)) => text(StatusCode::FORBIDDEN, m),
        Err(_) => text(StatusCode::BAD_REQUEST, "bad enroll request"),
    }
}

/// `GET /mesh/enroll-status/{node_id}` → a node polls for the human's decision. 200 + Grant once
/// approved (the cert is useless without the node's private key, so it is safe to serve openly);
/// 202 while pending; 404 if unknown.
fn enroll_status(dir: &Path, node_id: &str) -> Response<Full<Bytes>> {
    match crate::enroll::enroll_status(dir, node_id) {
        Ok(crate::enroll::StatusOutcome::Granted(g)) => match serde_json::to_vec(&*g) {
            Ok(b) => text(StatusCode::OK, b),
            Err(_) => text(StatusCode::INTERNAL_SERVER_ERROR, "grant encode"),
        },
        Ok(crate::enroll::StatusOutcome::Pending) => text(StatusCode::ACCEPTED, "pending approval"),
        Ok(crate::enroll::StatusOutcome::Unknown) => text(StatusCode::NOT_FOUND, "no such request"),
        Err(_) => text(StatusCode::INTERNAL_SERVER_ERROR, "status error"),
    }
}

/// `GET /mesh/tool/{id}` → the raw script body, if we have that tool and sharing is on.
/// The requester re-hashes the body against the manifest before trusting it.
fn serve_tool(dir: &Path, id: &str) -> Response<Full<Bytes>> {
    let cfg = config::load(dir).unwrap_or_default();
    if !cfg.share_tools {
        return text(StatusCode::FORBIDDEN, "tool sharing disabled");
    }
    let tools = familiar_kernel::tool::load(dir).unwrap_or_default();
    let Some(tool) = tools.into_iter().find(|t| t.id == id) else {
        return text(StatusCode::NOT_FOUND, "no such tool");
    };
    match std::fs::read(&tool.script_path) {
        Ok(body) => text(StatusCode::OK, body),
        Err(_) => text(StatusCode::NOT_FOUND, "tool body missing"),
    }
}

// ---- gossip client ------------------------------------------------------------------

/// One gossip round: exchange briefs with every discovered peer **concurrently** — the mesh
/// talks through multiple connections at once, so one dead/slow peer no longer stalls the
/// rest of the round. Returns the number reached.
async fn gossip_round(
    dir: &Path,
    cfg: &MeshConfig,
    cred: &GroupCredential,
    lan_addrs: Vec<String>,
) -> usize {
    let our_brief = match std::fs::read(dir.join(OUTBOX_FILE)) {
        Ok(b) => b,
        Err(_) => return live_peer_count(dir), // no outbox yet (first tick pending)
    };

    let mut addrs: Vec<String> = enumerate_peers()
        .into_iter()
        .filter(|p| p.online)
        .map(|p| with_port(&p.ip, cfg.gossip_port))
        .collect();
    for sp in &cfg.static_peers {
        addrs.push(with_port(sp, cfg.gossip_port));
    }
    addrs.extend(lan_addrs);
    addrs.sort();
    addrs.dedup();

    let _ = cred; // group identity is applied via ingest_brief on each reply
    let mut set = tokio::task::JoinSet::new();
    for addr in addrs {
        let dir = dir.to_path_buf();
        let brief = our_brief.clone();
        set.spawn(async move { exchange_with(&dir, &addr, &brief).await.is_ok() });
    }
    let mut reached = 0;
    while let Some(res) = set.join_next().await {
        if matches!(res, Ok(true)) {
            reached += 1;
        }
    }
    reached
}

/// Every candidate host for bootstrap: online tailnet peers + static peers + LAN-discovered,
/// bare hosts, deduped.
fn candidate_hosts(cfg: &MeshConfig, lan_hosts: Vec<String>) -> Vec<String> {
    let mut hosts: Vec<String> = enumerate_peers()
        .into_iter()
        .filter(|p| p.online)
        .map(|p| p.ip)
        .collect();
    for sp in &cfg.static_peers {
        // A static peer may carry an explicit `:port`; the enroll client takes host + port apart.
        hosts.push(sp.split(':').next().unwrap_or(sp).to_string());
    }
    hosts.extend(lan_hosts);
    hosts.sort();
    hosts.dedup();
    hosts
}

/// Automatic-peering bootstrap. With **no covenant yet** and `auto_peer` on, ask each discovered
/// host (tailnet + static + LAN) to admit us by covenant (attesting the Three Laws). The first that
/// admits us (it runs `auto_accept_enrollments`, an invite window is open, a mint-capable peer it
/// relays to admits, or its human approves) hands us a group credential; we stop and the
/// supervisor's next iteration proceeds to gossip. Best-effort — every failure is swallowed, and a
/// peer that only files us as *pending* simply leaves us waiting for the next round. Returns the
/// number of covenants gained (0 or 1). Never called once we hold a group (the supervisor gates
/// it), so it can never replace an existing covenant or switch groups.
async fn auto_join_round(dir: &Path, cfg: &MeshConfig, lan_hosts: Vec<String>) -> usize {
    let port = cfg.gossip_port;
    for host in candidate_hosts(cfg, lan_hosts) {
        let dir2 = dir.to_path_buf();
        let outcome = tokio::task::spawn_blocking(move || {
            let node = crate::node::NodeKey::load_or_mint(&dir2, "familiar")?;
            crate::enroll::request_join(
                &dir2,
                &host,
                port,
                &node,
                crate::enroll::COVENANT_STATEMENT,
                now_secs(),
            )
        })
        .await;
        if let Ok(Ok(crate::enroll::JoinOutcome::Admitted(_))) = outcome {
            return 1; // we now hold a covenant — let the supervisor pick it up and gossip
        }
    }
    0
}

/// Deterministic formation tie-break: among the ungrouped nodes that can see each other, the
/// strictly-lowest node id creates the group; everyone else waits and joins it. Pure —
/// unit-tested. (If views are asymmetric for a round, at worst nobody forms and the next
/// round retries; both forming requires each to believe it is the strict minimum, which two
/// mutually-visible nodes cannot both believe.)
fn should_form(our_id: &str, ungrouped_peer_ids: &[String]) -> bool {
    !our_id.is_empty()
        && !ungrouped_peer_ids.is_empty()
        && ungrouped_peer_ids.iter().all(|p| our_id < p.as_str())
}

/// How long the invite window stays open after auto-forming — long enough for the peers that
/// triggered formation to come back on their next join round, bounded so it isn't a standing
/// open door.
const AUTO_FORM_INVITE_SECS: i64 = 10 * 60;

/// Auto-formation. Reached only when `auto_peer` is on and a join round found **no group
/// anywhere in reach**. Probe every candidate's `/mesh/hello`; if another *ungrouped* node is
/// visible and [`should_form`] elects us, create the group and open a bounded invite window so
/// the others' next `auto_join_round` is admitted by covenant. Returns whether we formed.
async fn auto_form_round(dir: &Path, cfg: &MeshConfig, lan_hosts: Vec<String>) -> bool {
    let mut ungrouped: Vec<String> = Vec::new();
    for host in candidate_hosts(cfg, lan_hosts) {
        let addr = with_port(&host, cfg.gossip_port);
        let Ok(resp) = http_send(&addr, Method::GET, "/mesh/hello", None, &[]).await else {
            continue;
        };
        if resp.status != StatusCode::OK {
            continue;
        }
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(&resp.body) else {
            continue;
        };
        let node_id = v.get("node_id").and_then(|s| s.as_str()).unwrap_or("");
        let group_id = v.get("group_id").and_then(|s| s.as_str()).unwrap_or("");
        if !node_id.is_empty() && group_id.is_empty() {
            ungrouped.push(node_id.to_string());
        }
    }

    let dir2 = dir.to_path_buf();
    let formed = tokio::task::spawn_blocking(move || {
        let node = crate::node::NodeKey::load_or_mint(&dir2, "familiar")?;
        if !should_form(&node.node_id(), &ungrouped) {
            return Ok(false);
        }
        // Re-check under no races with an admission that landed mid-probe.
        if group::load(&dir2)?.is_some() {
            return Ok(false);
        }
        let now = now_secs();
        group::create_group(
            &dir2,
            &node,
            "auto-formed",
            now,
            group::DEFAULT_CERT_TTL_SECS,
        )?;
        crate::enroll::open_invite(&dir2, now + AUTO_FORM_INVITE_SECS)?;
        Ok::<bool, crate::Error>(true)
    })
    .await;
    matches!(formed, Ok(Ok(true)))
}

/// POST our brief to one peer, verify + stash its reply, and pre-fetch any tool bodies we
/// lack. Errors (connection refused, non-peer host, forged reply) are swallowed by design.
async fn exchange_with(dir: &Path, addr: &str, our_brief: &[u8]) -> Result<()> {
    let reply = http_send(
        addr,
        Method::POST,
        "/mesh/brief",
        Some(our_brief.to_vec()),
        &[],
    )
    .await?;
    if reply.status != StatusCode::OK || reply.body.is_empty() {
        return Ok(()); // peer accepted ours but had nothing to return
    }
    // Verify the peer's brief before it touches disk (defense at ingress).
    ingest_brief(dir, &reply.body, addr)?;
    // Pre-fetch tool bodies we don't already have, content-addressed for the in-tick merge.
    if let Ok(brief) = serde_json::from_slice::<MeshBrief>(&reply.body) {
        upsert_peer(dir, &brief, addr)?;
        let known = known_tool_shas(dir);
        for t in &brief.body.capability.tools {
            let sha = &t.script_sha256;
            if known.contains(sha) || inbox_tool_path(dir, sha).exists() {
                continue;
            }
            if let Ok(resp) = http_send(
                addr,
                Method::GET,
                &format!("/mesh/tool/{}", t.tool_id),
                None,
                &[],
            )
            .await
            {
                if resp.status == StatusCode::OK && &sha256_hex(&resp.body) == sha {
                    let _ = std::fs::create_dir_all(dir.join(INBOX_TOOLS_DIR));
                    let _ = std::fs::write(inbox_tool_path(dir, sha), &resp.body);
                }
            }
        }
    }
    Ok(())
}

fn inbox_tool_path(dir: &Path, sha: &str) -> PathBuf {
    // sha is hex from our own manifest / hashing; still sanitize to a bare filename.
    let safe: String = sha.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    dir.join(INBOX_TOOLS_DIR).join(format!("{safe}.script"))
}

/// SHA-256 of every local tool body — the dedup set so we don't re-fetch what we have.
fn known_tool_shas(dir: &Path) -> std::collections::HashSet<String> {
    familiar_kernel::tool::load(dir)
        .unwrap_or_default()
        .iter()
        .filter_map(|t| std::fs::read(&t.script_path).ok())
        .map(|b| sha256_hex(&b))
        .collect()
}

struct HttpResp {
    status: StatusCode,
    body: Bytes,
}

/// A minimal one-shot HTTP/1.1 request over a fresh tailnet TCP connection.
async fn http_send(
    addr: &str,
    method: Method,
    path: &str,
    body: Option<Vec<u8>>,
    headers: &[(&str, &str)],
) -> Result<HttpResp> {
    let connect = tokio::time::timeout(Duration::from_secs(4), TcpStream::connect(addr));
    let stream = connect
        .await
        .map_err(|_| {
            crate::Error::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "connect timeout",
            ))
        })?
        .map_err(crate::Error::Io)?;
    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
        .await
        .map_err(|e| crate::Error::Malformed(format!("handshake: {e}")))?;
    tokio::spawn(async move {
        let _ = conn.await;
    });
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("host", addr);
    for (k, v) in headers {
        builder = builder.header(*k, *v);
    }
    let req = builder
        .body(Full::new(Bytes::from(body.unwrap_or_default())))
        .map_err(|e| crate::Error::Malformed(format!("request: {e}")))?;
    let resp = tokio::time::timeout(Duration::from_secs(6), sender.send_request(req))
        .await
        .map_err(|_| {
            crate::Error::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "request timeout",
            ))
        })?
        .map_err(|e| crate::Error::Malformed(format!("send: {e}")))?;
    let status = resp.status();
    let body = resp
        .into_body()
        .collect()
        .await
        .map_err(|e| crate::Error::Malformed(format!("read: {e}")))?
        .to_bytes();
    Ok(HttpResp { status, body })
}

// ---- tailscale peer enumeration -----------------------------------------------------

/// A tailnet peer as reported by `tailscale status --json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailscalePeer {
    pub ip: String,
    pub host: String,
    pub online: bool,
}

/// Run the tailscale CLI with `args`, trying `tailscale` on PATH first and then the macOS app
/// bundle's CLI (the GUI install puts nothing on PATH). None if neither answers.
fn tailscale_output(args: &[&str]) -> Option<std::process::Output> {
    ["tailscale", "/Applications/Tailscale.app/Contents/MacOS/Tailscale"]
        .iter()
        .find_map(|bin| {
            std::process::Command::new(bin)
                .args(args)
                .output()
                .ok()
                .filter(|o| o.status.success())
        })
}

/// Enumerate tailnet peers (read-only shell-out). Empty if tailscale is absent/unreachable —
/// mesh then relies on `static_peers` only.
pub fn enumerate_peers() -> Vec<TailscalePeer> {
    match tailscale_output(&["status", "--json"]) {
        Some(o) => parse_tailscale_status(&String::from_utf8_lossy(&o.stdout)),
        None => Vec::new(),
    }
}

/// This node's own tailnet IPv4 (`tailscale ip -4`), if tailscale is up.
pub fn self_tailnet_ip() -> Option<String> {
    tailscale_output(&["ip", "-4"]).and_then(|o| {
        String::from_utf8_lossy(&o.stdout)
            .lines()
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
    })
}

/// The primary LAN IPv4 — the source address the OS would route toward the internet. A connected
/// UDP socket never sends a packet; it just resolves routing. Std-only, macOS and Linux alike.
pub fn self_lan_ip() -> Option<String> {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("1.1.1.1:53").ok()?;
    let ip = sock.local_addr().ok()?.ip();
    if ip.is_loopback() || ip.is_unspecified() {
        return None;
    }
    Some(ip.to_string())
}

/// Where this node is (decimal degrees), if it can know. Sources, in order:
/// 1. `mesh/geo.json` — `{"lat":..,"lon":..}`, written by the human or a shell with a better
///    source (a GPS feed, a survey). Always wins.
/// 2. The freshest real GPS fix reported by a member device (phones/tablets report theirs on
///    every worldview read) — the devices are with the mesh, so their fix locates it.
/// Returns None when neither exists — an honest unknown, never an invented place. (IP
/// geolocation was tried and rejected: on satellite links it reports the ground station,
/// hundreds of km off; a wrong city is worse than no city.)
pub fn self_geo(dir: &Path) -> Option<(f64, f64)> {
    #[derive(serde::Deserialize)]
    struct Geo {
        lat: f64,
        lon: f64,
    }
    if let Ok(s) = std::fs::read_to_string(dir.join("mesh/geo.json")) {
        if let Ok(g) = serde_json::from_str::<Geo>(&s) {
            if g.lat != 0.0 || g.lon != 0.0 {
                return Some((g.lat, g.lon));
            }
        }
    }
    freshest_device_fix(dir)
}

/// The most recently seen member that reported a real GPS fix. Devices refresh theirs on
/// every worldview read, so this tracks the mesh's location in near-real-time.
pub fn freshest_device_fix(dir: &Path) -> Option<(f64, f64)> {
    load_peers(dir)
        .into_iter()
        .filter(|p| p.lat != 0.0 || p.lon != 0.0)
        .max_by_key(|p| p.last_seen)
        .map(|p| (p.lat, p.lon))
}

/// Every address a device could reach this node at, most-universal first: the tailnet IP
/// (reachable from any interface when the device also runs tailscale — cellular included), then
/// the LAN IP (same-wifi fallback that needs no VPN). Cached for 60s — consoles poll the
/// worldview every few seconds and this shells out to tailscale.
pub fn reachable_hosts() -> Vec<String> {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    static CACHE: Mutex<Option<(Instant, Vec<String>)>> = Mutex::new(None);
    let mut guard = CACHE.lock().unwrap_or_else(|p| p.into_inner());
    if let Some((at, hosts)) = guard.as_ref() {
        if at.elapsed() < Duration::from_secs(60) {
            return hosts.clone();
        }
    }
    let mut hosts = Vec::new();
    if let Some(ts) = self_tailnet_ip() {
        hosts.push(ts);
    }
    if let Some(lan) = self_lan_ip() {
        if !hosts.contains(&lan) {
            hosts.push(lan);
        }
    }
    *guard = Some((Instant::now(), hosts.clone()));
    hosts
}

/// Parse `tailscale status --json` into peers (pure — unit-tested against a fixture). Takes
/// the first IPv4 in each peer's `TailscaleIPs`.
pub fn parse_tailscale_status(json: &str) -> Vec<TailscalePeer> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let Some(peers) = v.get("Peer").and_then(|p| p.as_object()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for peer in peers.values() {
        let ip = peer
            .get("TailscaleIPs")
            .and_then(|a| a.as_array())
            .and_then(|a| a.iter().find_map(|x| x.as_str().filter(|s| is_ipv4(s))))
            .map(|s| s.to_string());
        let Some(ip) = ip else { continue };
        out.push(TailscalePeer {
            ip,
            host: peer
                .get("HostName")
                .and_then(|h| h.as_str())
                .unwrap_or("")
                .to_string(),
            online: peer
                .get("Online")
                .and_then(|o| o.as_bool())
                .unwrap_or(false),
        });
    }
    out
}

fn is_ipv4(s: &str) -> bool {
    let mut parts = 0;
    for p in s.split('.') {
        if p.parse::<u8>().is_err() {
            return false;
        }
        parts += 1;
    }
    parts == 4
}

fn with_port(addr: &str, default_port: u16) -> String {
    // Leave an explicit ip:port alone; otherwise append the default. (IPv4/hostname only.)
    if addr.contains(':') {
        addr.to_string()
    } else {
        format!("{addr}:{default_port}")
    }
}

fn short(s: &str) -> String {
    s.chars().take(8).collect()
}

// ---- peer roster + status -----------------------------------------------------------

fn upsert_peer(dir: &Path, brief: &MeshBrief, addr: &str) -> Result<()> {
    let path = dir.join(PEERS_FILE);
    let mut peers: Vec<PeerRecord> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let now = now_secs();
    let rec = PeerRecord {
        node_id: brief.body.node.node_id.clone(),
        label: brief.body.node.label.clone(),
        addr: addr.to_string(),
        group_id: brief.body.membership.group_id.clone(),
        last_seen: now,
        tools_offered: brief.body.capability.tools.len(),
        patterns_offered: brief.body.knowledge.patterns.len(),
        os: brief.body.capability.os.clone(),
        arch: brief.body.capability.arch.clone(),
        first_seen: now,
        familiar_version: brief.body.capability.familiar_version.clone(),
        os_version: brief.body.capability.os_version.clone(),
        session_start: now,
        total_online_secs: 0,
        interactive: brief.body.capability.interactive,
        human: brief.body.capability.human.clone(),
        lat: brief.body.capability.lat,
        lon: brief.body.capability.lon,
    };
    match peers.iter_mut().find(|p| p.node_id == rec.node_id) {
        Some(existing) => {
            let addr_keep = if rec.addr.is_empty() {
                existing.addr.clone()
            } else {
                rec.addr.clone()
            };
            // Preserve the original join date (backfill 0 from a pre-field row to now).
            let first_seen = if existing.first_seen > 0 {
                existing.first_seen
            } else {
                now
            };
            // Session accounting: a sighting within the freshness window continues the
            // current run; a longer gap closes it (bank its duration) and starts a new one.
            let (session_start, total_online_secs) =
                if now - existing.last_seen <= GOSSIP_FRESH_SECS {
                    (
                        if existing.session_start > 0 {
                            existing.session_start
                        } else {
                            existing.last_seen
                        },
                        existing.total_online_secs,
                    )
                } else {
                    let closed = if existing.session_start > 0 {
                        (existing.last_seen - existing.session_start).max(0)
                    } else {
                        0
                    };
                    (now, existing.total_online_secs + closed)
                };
            // A brief without a fix (0/0) never erases a position we already know.
            let (lat, lon) = if rec.lat != 0.0 || rec.lon != 0.0 {
                (rec.lat, rec.lon)
            } else {
                (existing.lat, existing.lon)
            };
            *existing = PeerRecord {
                addr: addr_keep,
                first_seen,
                session_start,
                total_online_secs,
                lat,
                lon,
                ..rec
            };
        }
        None => peers.push(rec),
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_vec_pretty(&peers)?)?;
    Ok(())
}

/// Register a **device peer** — a member that reads the worldview (`/mesh/worldview`) rather than
/// only pushing observations. It participates as a full peer (an iPad console), so it belongs in the
/// peer roster, not the device-agent list. It can't serve gossip, so `tools/patterns` are 0 and the
/// gossip loop never dials it (that loop reaches Tailscale-discovered addrs, not `peers.json`);
/// `addr` is the observed source IP, for display only. Upserts by node_id like [`upsert_peer`].
pub(crate) fn register_device_peer(
    dir: &Path,
    node_id: &str,
    label: &str,
    addr: &str,
    client_version: &str,
    os_version: &str,
    lat: f64,
    lon: f64,
) -> Result<()> {
    let path = dir.join(PEERS_FILE);
    let mut peers: Vec<PeerRecord> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let group_id = group::load(dir)
        .ok()
        .flatten()
        .map(|c| c.group_id)
        .unwrap_or_default();
    let now = now_secs();
    match peers.iter_mut().find(|p| p.node_id == node_id) {
        Some(existing) => {
            // Session accounting (device window): a read within the freshness window
            // continues the run; a longer gap banks the old run and starts a new one.
            if now - existing.last_seen <= crate::members::ONLINE_WINDOW_SECS {
                if existing.session_start == 0 {
                    existing.session_start = existing.last_seen;
                }
            } else {
                if existing.session_start > 0 {
                    existing.total_online_secs +=
                        (existing.last_seen - existing.session_start).max(0);
                }
                existing.session_start = now;
            }
            existing.interactive = true; // a console read is a human-facing surface
            existing.last_seen = now;
            if existing.first_seen == 0 {
                existing.first_seen = now;
            }
            if !label.is_empty() {
                existing.label = label.to_string();
            }
            if !addr.is_empty() {
                existing.addr = addr.to_string();
            }
            if !client_version.is_empty() {
                existing.familiar_version = client_version.to_string();
            }
            if !os_version.is_empty() {
                existing.os_version = os_version.to_string();
            }
            // A device with GPS reports where it is on every read; 0/0 means "not reported"
            // and never overwrites a real fix.
            if lat != 0.0 || lon != 0.0 {
                existing.lat = lat;
                existing.lon = lon;
            }
        }
        None => peers.push(PeerRecord {
            node_id: node_id.to_string(),
            label: label.to_string(),
            addr: addr.to_string(),
            group_id,
            last_seen: now,
            tools_offered: 0,
            patterns_offered: 0,
            os: String::new(),
            arch: String::new(),
            first_seen: now,
            familiar_version: client_version.to_string(),
            os_version: os_version.to_string(),
            session_start: now,
            total_online_secs: 0,
            interactive: true,
            human: String::new(),
            lat,
            lon,
        }),
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_vec_pretty(&peers)?)?;
    Ok(())
}

/// Forget a peer: drop it from the roster by node id (`mesh forget`). The record — join
/// dates, accumulated online time — is gone for good; a live node will simply re-enroll as
/// new on its next exchange. Returns whether the id was present.
pub fn remove_peer(dir: &Path, node_id: &str) -> Result<bool> {
    let path = dir.join(PEERS_FILE);
    let mut peers: Vec<PeerRecord> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let before = peers.len();
    peers.retain(|p| p.node_id != node_id);
    if peers.len() == before {
        return Ok(false);
    }
    std::fs::write(&path, serde_json::to_vec_pretty(&peers)?)?;
    Ok(true)
}

/// Load the peer records as last seen — for the worldview read seam (an iPad console shows them).
pub(crate) fn load_peers(dir: &Path) -> Vec<PeerRecord> {
    std::fs::read_to_string(dir.join(PEERS_FILE))
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<PeerRecord>>(&s).ok())
        .unwrap_or_default()
}

fn live_peer_count(dir: &Path) -> usize {
    std::fs::read_to_string(dir.join(PEERS_FILE))
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<PeerRecord>>(&s).ok())
        .map(|p| p.len())
        .unwrap_or(0)
}

/// How many peers we're actually federating with: entries in `peers.json` seen within a few gossip
/// intervals, in *either* direction. The gossip round's own return counts only this cycle's
/// OUTBOUND reach, which undercounts a peer that reaches us but that we didn't reach this round —
/// the cause of a confusing "0 peer(s) connected" while the tick reports 1.
fn count_connected(dir: &Path, interval_secs: u64) -> usize {
    let window = interval_secs.saturating_mul(3).max(90) as i64;
    let now = now_secs();
    std::fs::read_to_string(dir.join(PEERS_FILE))
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<PeerRecord>>(&s).ok())
        .map(|ps| ps.iter().filter(|p| now - p.last_seen <= window).count())
        .unwrap_or(0)
}

fn write_status(dir: &Path, msg: &str) -> Result<()> {
    let path = dir.join(STATUS_FILE);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, msg)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tailscale_status_fixture() {
        let json = r#"{
          "Self": {"HostName":"wildhorse","TailscaleIPs":["100.64.0.10"],"Online":true},
          "Peer": {
            "keyA": {"HostName":"cpn","TailscaleIPs":["100.111.113.96","fd7a:1::1"],"Online":true},
            "keyB": {"HostName":"cerbo","TailscaleIPs":["100.99.1.2"],"Online":false},
            "keyC": {"HostName":"noips","TailscaleIPs":[],"Online":true}
          }
        }"#;
        let mut peers = parse_tailscale_status(json);
        peers.sort_by(|a, b| a.host.cmp(&b.host));
        assert_eq!(peers.len(), 2); // keyC has no IPv4, dropped
        assert_eq!(peers[0].host, "cerbo");
        assert!(!peers[0].online);
        assert_eq!(peers[1].host, "cpn");
        assert_eq!(peers[1].ip, "100.111.113.96"); // first IPv4, not the v6
        assert!(peers[1].online);
    }

    #[test]
    fn malformed_status_is_empty_not_a_panic() {
        assert!(parse_tailscale_status("not json").is_empty());
        assert!(parse_tailscale_status("{}").is_empty());
    }

    #[test]
    fn with_port_respects_explicit_port() {
        assert_eq!(with_port("100.64.0.1", 47100), "100.64.0.1:47100");
        assert_eq!(with_port("127.0.0.1:9000", 47100), "127.0.0.1:9000");
    }

    #[test]
    fn beacon_roundtrip_and_junk_rejection() {
        let b = LanBeacon {
            familiar_mesh: 1,
            node_id: "abc123".into(),
            gossip_port: 47100,
        };
        let bytes = serde_json::to_vec(&b).unwrap();
        let parsed = parse_beacon(&bytes).unwrap();
        assert_eq!(parsed.node_id, "abc123");
        assert_eq!(parsed.gossip_port, 47100);
        // Junk, foreign versions, and empty ids are all dropped.
        assert!(parse_beacon(b"not json").is_none());
        assert!(parse_beacon(br#"{"familiar_mesh":2,"node_id":"x","gossip_port":1}"#).is_none());
        assert!(parse_beacon(br#"{"familiar_mesh":1,"node_id":"","gossip_port":1}"#).is_none());
    }

    #[test]
    fn formation_tiebreak_elects_exactly_the_strict_minimum() {
        let peers = vec!["bbb".to_string(), "ccc".to_string()];
        assert!(should_form("aaa", &peers)); // strictly lowest → forms
        assert!(!should_form("bbb", &peers)); // ties never form (both would create)
        assert!(!should_form("zzz", &peers)); // higher waits for the lower to form
        assert!(!should_form("aaa", &[])); // nobody visible → nothing to form with
        assert!(!should_form("", &peers)); // no identity → never form
    }

    #[test]
    fn lan_state_ages_out_stale_peers() {
        let lan = LanState::default();
        let now = now_secs();
        {
            let mut m = lan.peers.lock().unwrap();
            m.insert("192.168.1.7".into(), (47100, now));
            m.insert("192.168.1.9".into(), (47100, now - 1000));
        }
        let addrs = lan.addrs(90);
        assert_eq!(addrs, vec!["192.168.1.7:47100".to_string()]);
        assert_eq!(lan.hosts(90), vec!["192.168.1.7".to_string()]);
    }

    #[test]
    fn is_ipv4_discriminates() {
        assert!(is_ipv4("100.64.0.10"));
        assert!(!is_ipv4("fd7a:1::1"));
        assert!(!is_ipv4("300.1.1.1"));
        assert!(!is_ipv4("1.2.3"));
    }
}
