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
/// Nodes seen in the status directory but not yet admitted to the roster (ADR-0025).
const CANDIDATES_FILE: &str = "mesh/candidates.json";
/// How long a node must keep showing up before it becomes a member. Adoption used to happen on a
/// SINGLE heartbeat, which is how four reinstalls in one afternoon left three ghost "iPhone"
/// members in the roster forever: each throwaway key heartbeated once and was promoted for good.
/// A key is not a device (ADR-0025), so a key that appears once and vanishes was never a device
/// joining — it was a keypair passing through.
const ADOPT_AFTER_SECS: i64 = 10 * 60;
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
/// `Default` so a row adopted from the status directory can fill the fields that only a brief or a
/// worldview read can teach us (tools, arch, geo) without inventing values for them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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
    /// True when the fix came from a device's own GPS (worldview read) rather than a peer's
    /// brief. Only device fixes seed `self_geo` — a brief-carried position may itself be
    /// inherited or stale, and trusting it circularly spread one bad fix mesh-wide.
    #[serde(default)]
    pub geo_device: bool,
    /// "" (active, default) | "abandoned" — a human's call that this peer is gone for good
    /// (decommissioned hardware, a retired VM), set via `familiar mesh forget <node_id>`.
    /// Never deletes the record — the full history (first_seen, total_online_secs, tools/
    /// patterns it once offered) stays; abandoned peers are just excluded from the active
    /// roster/worldview so they stop being carried around in every gossip round and device
    /// read. Self-healing: any fresh contact (a brief, a worldview read) revives it to active
    /// automatically — renewed contact is itself evidence it isn't defunct after all. A human
    /// re-abandons if it turns out to be a one-off.
    #[serde(default)]
    pub status: String,
    /// How this member is reaching the mesh — "local" | "lighthouse" | "tailscale" — as it reported
    /// in its status heartbeat (ADR-0017). Empty when unknown. Surfaced as a roster badge.
    #[serde(default)]
    pub connectivity: String,
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
        // SAFETY: one-time OS thread spawn at mesh startup, not reachable from any network
        // input — a failure here means the process can't spawn threads at all (OOM-adjacent),
        // which nothing downstream could gracefully continue from anyway.
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
    let mut local_server: Option<tokio::task::JoinHandle<()>> = None;
    let mut bound_port: u16 = 0;
    let mut lan_task: Option<tokio::task::JoinHandle<()>> = None;
    let mut lan_bound: u16 = 0;
    let lan = Arc::new(LanState::default());
    // The consult relay keeps its own (much tighter) clock — see consult_relay_loop.
    let mut relay_task: Option<tokio::task::JoinHandle<()>> = None;

    loop {
        if stop.load(Ordering::SeqCst) {
            if let Some(s) = server.take() {
                s.abort();
            }
            if let Some(l) = lan_task.take() {
                l.abort();
            }
            if let Some(r) = relay_task.take() {
                r.abort();
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
            if let Some(s) = local_server.take() {
                s.abort();
            }
            match TcpListener::bind(("0.0.0.0", cfg.gossip_port)).await {
                Ok(listener) => {
                    bound_port = cfg.gossip_port;
                    let ctx = Arc::new(ServerCtx {
                        dir: dir.clone(),
                        seen: std::sync::Mutex::new(crate::observe::IngestGuard::default()),
                    });
                    let acceptor = match tls_acceptor(&dir) {
                        Ok(a) => Some(a),
                        Err(e) => {
                            let _ = write_status(&dir, &format!("mesh tls init failed: {e}"));
                            None
                        }
                    };
                    server = Some(tokio::spawn(serve(listener, ctx.clone(), acceptor)));
                    // The /local seams (worldview/answer/gate) stay PLAIN on loopback, one
                    // port up — local consoles are reading the machine they run on; the
                    // wire never leaves the host.
                    if let Ok(local) = TcpListener::bind(("127.0.0.1", cfg.gossip_port + 1)).await {
                        local_server = Some(tokio::spawn(serve(local, ctx, None)));
                    }
                }
                Err(e) => {
                    let _ =
                        write_status(&dir, &format!("mesh bind :{} failed: {e}", cfg.gossip_port));
                    sleep_or_stop(&stop, interval).await;
                    continue;
                }
            }
        }

        // The consult relay (ADR-0014) runs for as long as the mesh gate is open; it re-reads config
        // and credential itself each pass, so it needs starting only once.
        if relay_task.is_none() {
            relay_task = Some(tokio::spawn(consult_relay_loop(dir.clone(), stop.clone())));
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
        // Keep our door listed at the rendezvous so new devices can find us without a QR.
        register_at_rendezvous(&dir, &cfg, &cred).await;
        // ADR-0017: heartbeat our own status to the lighthouse, then pull the mesh-wide status it
        // holds so our roster shows members fresh even when they read from the lighthouse, not us.
        heartbeat_status(&dir, &cfg, &cred).await;
        pull_status(&dir, &cfg).await;
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

/// Register this mesh with each configured rendezvous host (the lighthouse) so a fresh device can
/// discover where to join without a QR (ADR-0012). Advertises the addresses a joiner can reach this
/// familiar at (`reachable_hosts`) under the group's label, signed by this node's key + membership.
/// Best-effort and idempotent — refreshed every gossip round; a lapse just lets the entry expire.
async fn register_at_rendezvous(
    dir: &Path,
    cfg: &MeshConfig,
    cred: &crate::group::GroupCredential,
) {
    if cfg.rendezvous_hosts.is_empty() {
        return;
    }
    let Ok(node) = crate::node::NodeKey::load_or_mint(dir, "familiar") else {
        return;
    };
    let hosts = reachable_hosts();
    if hosts.is_empty() {
        return; // nothing a joiner could reach us at
    }
    let now = now_secs();
    let reg = crate::rendezvous::Registration {
        membership: cred.membership.clone(),
        group_pubkey: cred.group_pubkey.clone(),
        group_label: cred.label.clone(),
        hosts,
        port: cfg.gossip_port,
        pins: advertised_pins(dir),
        nonce: format!("{now:x}{}", node.node_id()),
        ts: now,
    };
    let Ok(raw) = serde_json::to_vec(&reg) else {
        return;
    };
    let sig = node.sign(&raw);
    for rh in &cfg.rendezvous_hosts {
        let addr = format!("{}:{}", rh, cfg.gossip_port);
        let _ = http_send(
            &addr,
            Method::POST,
            "/mesh/rendezvous-register",
            Some(raw.clone()),
            &[
                ("X-Familiar-Sig", &sig),
                ("Content-Type", "application/json"),
            ],
        )
        .await;
    }
}

/// Heartbeat this node's own status to each rendezvous host (ADR-0017). Best-effort, every gossip
/// round — the lighthouse holds a live, mesh-wide picture so no member is invisible to another,
/// whatever path each is on. Phase A reports `connectivity="local"`; the tailnet fields fill in the
/// Tailscale phases.
async fn heartbeat_status(dir: &Path, cfg: &MeshConfig, cred: &crate::group::GroupCredential) {
    if cfg.rendezvous_hosts.is_empty() {
        return;
    }
    let Ok(node) = crate::node::NodeKey::load_or_mint(dir, "familiar") else {
        return;
    };
    let now = now_secs();
    let status = crate::status::MemberStatus {
        node_id: node.node_id(),
        group_ref: String::new(), // stamped by the host from the membership
        actor: String::new(),     // the home familiar isn't a device actor
        label: String::new(),     // a reader keeps its own label for a node it already knows
        present_human: familiar_kernel::identity::current(dir).unwrap_or_default(),
        // The daemon's own notion of who is here comes from the observation stream, not from a
        // device's identification ladder (ADR-0019), so it asserts no provenance or confidence.
        present_via: String::new(),
        present_since: 0,
        present_confidence: 0.0,
        connectivity: "local".into(),
        tailnet_addr: String::new(),
        tailnet_up: false,
        updated_at: now,
    };
    let report = crate::status::StatusReport {
        membership: cred.membership.clone(),
        group_pubkey: cred.group_pubkey.clone(),
        status,
        nonce: format!("{now:x}{}", node.node_id()),
        ts: now,
    };
    let Ok(raw) = serde_json::to_vec(&report) else {
        return;
    };
    let sig = node.sign(&raw);
    for rh in &cfg.rendezvous_hosts {
        let addr = format!("{}:{}", rh, cfg.gossip_port);
        let _ = http_send(
            &addr,
            Method::POST,
            "/mesh/status",
            Some(raw.clone()),
            &[
                ("X-Familiar-Sig", &sig),
                ("Content-Type", "application/json"),
            ],
        )
        .await;
    }
}

/// How often the relay talks to the broker while consults are outstanding. Deliberately far tighter
/// than the gossip interval: a consult's cost is dominated by how long a queued prompt sits before
/// anyone notices it, and a minute of that on each leg is most of the round trip.
const CONSULT_RELAY_BUSY: Duration = Duration::from_secs(2);
/// How often it checks back when there is nothing outstanding.
const CONSULT_RELAY_IDLE: Duration = Duration::from_secs(10);

/// Broker this node's consults through the rendezvous hosts (ADR-0014), on its own clock.
///
/// This is what lets a device answer for a familiar it cannot reach. A home node behind CGNAT is
/// unreachable from the lighthouse, so the lighthouse can never push; the hub therefore drives both
/// directions from here — it sends the prompts it is waiting on and takes back whatever answers have
/// accumulated. It runs beside the gossip loop rather than inside it because the gossip interval is a
/// minute and a consult should not wait that long twice.
async fn consult_relay_loop(dir: PathBuf, stop: Arc<AtomicBool>) {
    // True when the broker may still be holding something of ours — so that after the last prompt
    // clears we make one final call, which is what tells the broker to retire the finished work.
    let mut outstanding = false;
    loop {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        let cfg = config::load(&dir).unwrap_or_default();
        if cfg.rendezvous_hosts.is_empty() || !mesh_allowed(&dir) {
            sleep_or_stop(&stop, CONSULT_RELAY_IDLE).await;
            continue;
        }
        let Some(cred) = group::load(&dir).ok().flatten() else {
            sleep_or_stop(&stop, CONSULT_RELAY_IDLE).await;
            continue;
        };
        let prompts = crate::consult::pending(&dir, now_secs());
        let busy = !prompts.is_empty();
        if busy || outstanding {
            relay_consults(&dir, &cfg, &cred, prompts).await;
            outstanding = busy;
        }
        sleep_or_stop(
            &stop,
            if busy {
                CONSULT_RELAY_BUSY
            } else {
                CONSULT_RELAY_IDLE
            },
        )
        .await;
    }
}

/// One relay exchange with each broker: hand over the prompts we are waiting on, store what comes
/// back. Best-effort — a failed round changes nothing, because the next one resends the same list.
async fn relay_consults(
    dir: &Path,
    cfg: &MeshConfig,
    cred: &crate::group::GroupCredential,
    prompts: Vec<crate::consult::ConsultPrompt>,
) {
    let Ok(node) = crate::node::NodeKey::load_or_mint(dir, "familiar") else {
        return;
    };
    let now = now_secs();
    let relay = crate::consult::ConsultRelay {
        membership: cred.membership.clone(),
        group_pubkey: cred.group_pubkey.clone(),
        prompts,
        nonce: format!("{now:x}{}", node.node_id()),
        ts: now,
    };
    let Ok(raw) = serde_json::to_vec(&relay) else {
        return;
    };
    let sig = node.sign(&raw);
    for rh in &cfg.rendezvous_hosts {
        let addr = format!("{}:{}", rh, cfg.gossip_port);
        let Ok(resp) = http_send(
            &addr,
            Method::POST,
            "/mesh/consult-relay",
            Some(raw.clone()),
            &[
                ("X-Familiar-Sig", &sig),
                ("Content-Type", "application/json"),
            ],
        )
        .await
        else {
            continue;
        };
        let Ok(answers) = serde_json::from_slice::<Vec<crate::consult::ConsultAnswer>>(&resp.body)
        else {
            continue;
        };
        for a in answers {
            // Clears the local prompt, which is what stops us relaying it again next round.
            let _ = crate::consult::store_answer(dir, &a);
        }
    }
}

/// Pull the mesh-wide status directory from a rendezvous host and bump our own peer records forward
/// to match (ADR-0017). This is what keeps the home node's roster fresh about a device that reads its
/// worldview from the lighthouse, not from us: the lighthouse saw it seconds ago, so we learn that.
async fn pull_status(dir: &Path, cfg: &MeshConfig) {
    if cfg.rendezvous_hosts.is_empty() {
        return;
    }
    let now = now_secs();
    for rh in &cfg.rendezvous_hosts {
        let addr = format!("{}:{}", rh, cfg.gossip_port);
        if let Ok(resp) = http_send(&addr, Method::GET, "/mesh/status", None, &[]).await {
            if let Ok(list) = serde_json::from_slice::<Vec<crate::status::MemberStatus>>(&resp.body)
            {
                apply_status_freshness(dir, &list, now);
                break; // one authoritative source is enough
            }
        }
    }
}

/// Bump a known peer's `last_seen` forward when the status directory reports it fresher than we hold
/// (ADR-0017), and **adopt members we have never met**. Only ever moves last_seen forward — a stale
/// directory row never regresses local truth — and a fresh heartbeat revives an abandoned peer,
/// mirroring `register_device_peer`.
///
/// The adoption half matters as much as the bump. This used to `find()` and silently drop anything
/// it didn't already hold, which meant a member admitted at the lighthouse — the ONLY minting door
/// (ADR-0018), so in practice every remote member — never reached any other node's roster. A tester
/// on the far side of the country could be enrolled, live and heartbeating, and simply not exist as
/// far as the rest of the mesh was concerned. The lighthouse knew; nobody asked it to say.
fn apply_status_freshness(dir: &Path, statuses: &[crate::status::MemberStatus], now: i64) {
    let path = dir.join(PEERS_FILE);
    let mut peers: Vec<PeerRecord> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    // Only ever adopt rows for OUR mesh, and never adopt ourselves.
    let our_ref = group::load(dir)
        .ok()
        .flatten()
        .map(|c| crate::rendezvous::group_ref(&c.group_id))
        .unwrap_or_default();
    let our_node = node_id_of(dir);
    let mut changed = false;
    for st in statuses {
        if st.node_id.is_empty() {
            continue;
        }
        if peers.iter().all(|p| p.node_id != st.node_id) {
            // Unknown member. Adopt it if it is demonstrably one of ours: same group_ref, not us.
            // A row carrying no group_ref is from an older node and is not trusted into the roster.
            if st.node_id == our_node || our_ref.is_empty() || st.group_ref != our_ref {
                continue;
            }
            // …and if it has PERSISTED. One heartbeat is not a device arriving; it is a key
            // passing through, and promoting it permanently is how ghosts are made (ADR-0025).
            if !candidate_is_ripe(dir, &st.node_id, now) {
                continue;
            }
            peers.push(PeerRecord {
                node_id: st.node_id.clone(),
                label: if st.label.is_empty() {
                    st.node_id.chars().take(8).collect()
                } else {
                    st.label.clone()
                },
                addr: String::new(),
                group_id: String::new(),
                last_seen: st.updated_at,
                // We are meeting it now; the lighthouse may have known it far longer, but this is
                // the first moment WE can honestly attest to. `first_seen` is our own record.
                first_seen: now,
                human: st.present_human.clone(),
                connectivity: st.connectivity.clone(),
                ..Default::default()
            });
            changed = true;
            continue;
        }
        if let Some(p) = peers.iter_mut().find(|p| p.node_id == st.node_id) {
            if st.updated_at > p.last_seen {
                p.last_seen = st.updated_at;
                p.status = String::new();
                changed = true;
            }
            // Adopt the member's self-reported connectivity mode (ADR-0017 Phase B) — how it says it
            // is reaching the mesh, for the roster badge. Only a member reports its own mode.
            if !st.connectivity.is_empty() && st.connectivity != p.connectivity {
                p.connectivity = st.connectivity.clone();
                changed = true;
            }
        }
    }
    if changed {
        if let Ok(s) = serde_json::to_string(&peers) {
            let _ = std::fs::write(&path, s);
        }
    }
}

/// Has this node been showing up long enough to be a member rather than a passer-by?
///
/// Records first sighting and returns true only once [`ADOPT_AFTER_SECS`] has elapsed since it.
/// Deliberately a *duration*, not a count: a device that heartbeats every 60s and a device that
/// heartbeats hourly are both real, and counting pulls would punish the second one.
fn candidate_is_ripe(dir: &Path, node_id: &str, now: i64) -> bool {
    let path = dir.join(CANDIDATES_FILE);
    let mut seen: std::collections::BTreeMap<String, i64> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let first = *seen.entry(node_id.to_string()).or_insert(now);
    // Forget candidates that stopped showing up, so the ledger cannot grow without bound — and so
    // a key that vanishes never ripens into a member later.
    seen.retain(|_, t| now - *t < ADOPT_AFTER_SECS * 6);
    if let Ok(s) = serde_json::to_string(&seen) {
        let _ = std::fs::create_dir_all(dir.join("mesh"));
        let _ = std::fs::write(&path, s);
    }
    // A first sighting stamped in the future (clock skew) must not ripen instantly.
    (0..=ADOPT_AFTER_SECS * 1000).contains(&(now - first)) && now - first >= ADOPT_AFTER_SECS
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

// ---- TLS (ADR-0009 Phase 1): covenant-pinned transport security --------------------
//
// The mesh port speaks TLS. Authenticity still comes from the covenant signatures on
// every payload — TLS here adds confidentiality and integrity on any path (open wifi,
// cellular, raw internet). Each node holds a persistent P-256 TLS key (separate from the
// ed25519 node key: Apple TLS stacks don't handshake EdDSA certificates), and the key's
// SPKI SHA-256 rides in the enrollment payload so devices can PIN the node they joined.
// Peer-to-peer dials accept any certificate (opportunistic encryption): a forged server
// cannot forge brief/worldview signatures, so active MITM gains nothing it didn't have
// in the plaintext era — while passive observation dies entirely.

const TLS_KEY_FILE: &str = "mesh/tls_key.der";

/// rustls needs a process-level crypto provider exactly once.
fn ensure_crypto_provider() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// This node's persistent TLS keypair (PKCS#8 DER), minted on first use.
fn tls_keypair(dir: &Path) -> Result<rcgen::KeyPair> {
    let path = dir.join(TLS_KEY_FILE);
    if let Ok(der) = std::fs::read(&path) {
        if let Ok(kp) = rcgen::KeyPair::try_from(der.as_slice()) {
            return Ok(kp);
        }
    }
    let kp = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)
        .map_err(|e| crate::Error::Malformed(format!("tls keygen: {e}")))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(crate::Error::Io)?;
    }
    std::fs::write(&path, kp.serialize_der()).map_err(crate::Error::Io)?;
    Ok(kp)
}

/// SHA-256 of this node's TLS SubjectPublicKeyInfo, hex — the pin a device stores at
/// enrollment (`mesh qr` carries it) and checks on every connection thereafter.
pub fn tls_spki_pin(dir: &Path) -> Result<String> {
    use sha2::Digest;
    let kp = tls_keypair(dir)?;
    Ok(crate::hex_encode(&sha2::Sha256::digest(
        kp.public_key_der(),
    )))
}

/// Every TLS pin a device may legitimately meet in this group: this node's own, plus the
/// pins of sibling members it might fail over to (`advertise_pins` — the lighthouse, peers).
/// A device adopts this set and accepts a cert whose SPKI is any of them, so it can reach the
/// mesh through whichever member is reachable — pinned to the GROUP, not to one node (ADR-0012).
pub fn advertised_pins(dir: &Path) -> Vec<String> {
    let mut pins = Vec::new();
    if let Ok(p) = tls_spki_pin(dir) {
        pins.push(p);
    }
    for p in config::load(dir).unwrap_or_default().advertise_pins {
        if !p.is_empty() && !pins.contains(&p) {
            pins.push(p);
        }
    }
    pins
}

/// The server's TLS acceptor: a self-signed cert over the persistent key. The cert is
/// re-minted each boot (cheap); the KEY persists, so the SPKI pin never changes.
fn tls_acceptor(dir: &Path) -> Result<tokio_rustls::TlsAcceptor> {
    ensure_crypto_provider();
    let kp = tls_keypair(dir)?;
    let mut params = rcgen::CertificateParams::new(vec!["familiar-mesh".into()])
        .map_err(|e| crate::Error::Malformed(format!("tls cert params: {e}")))?;
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "familiar-mesh");
    // Explicit, sanely-encoded validity. rcgen's defaults (1975 … 4096) encode the far-future
    // year in a way iOS misreads as 1996 — an *expired* cert — and iOS enforces validity strictly
    // on public IPs (the lighthouse) even when a custom pin delegate would otherwise accept it, so
    // device reads over cellular failed with an opaque TLS error. Keep both bounds below 2050 so
    // they serialize as unambiguous 2-digit UTCTime that every stack parses identically. The key
    // persists across re-mints, so the SPKI pin devices hold is unchanged.
    params.not_before = rcgen::date_time_ymd(2020, 1, 1);
    params.not_after = rcgen::date_time_ymd(2049, 12, 31);
    let cert = params
        .self_signed(&kp)
        .map_err(|e| crate::Error::Malformed(format!("tls cert: {e}")))?;
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(kp.serialize_der().into());
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert.der().clone()], key)
        .map_err(|e| crate::Error::Malformed(format!("tls config: {e}")))?;
    Ok(tokio_rustls::TlsAcceptor::from(Arc::new(config)))
}

/// Outbound TLS: encrypt to whoever answers. Payload signatures carry the authenticity.
fn tls_connector() -> tokio_rustls::TlsConnector {
    tokio_rustls::TlsConnector::from(opportunistic_tls_config())
}

/// The opportunistic-encryption client config, shared by every outbound dial this crate makes —
/// the async transport above and the blocking enrolment client in [`crate::enroll`] alike. One
/// config, so the posture ("encrypt to whoever answers; payload signatures carry authenticity")
/// cannot quietly diverge between the two.
pub(crate) fn opportunistic_tls_config() -> Arc<rustls::ClientConfig> {
    ensure_crypto_provider();
    #[derive(Debug)]
    struct AcceptAny;
    impl rustls::client::danger::ServerCertVerifier for AcceptAny {
        fn verify_server_cert(
            &self,
            _end_entity: &rustls::pki_types::CertificateDer<'_>,
            _intermediates: &[rustls::pki_types::CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName<'_>,
            _ocsp: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error>
        {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }
        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &rustls::pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
        {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }
        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &rustls::pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
        {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }
        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            rustls::crypto::ring::default_provider()
                .signature_verification_algorithms
                .supported_schemes()
        }
    }
    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAny))
        .with_no_client_auth();
    Arc::new(config)
}

async fn serve(listener: TcpListener, ctx: Arc<ServerCtx>, tls: Option<tokio_rustls::TlsAcceptor>) {
    loop {
        let (stream, remote) = match listener.accept().await {
            Ok(pair) => pair,
            Err(_) => continue,
        };
        let peer_ip = remote.ip().to_string();
        let ctx = ctx.clone();
        let tls = tls.clone();
        tokio::spawn(async move {
            let svc = service_fn(move |req| handle(req, ctx.clone(), peer_ip.clone()));
            match tls {
                Some(acceptor) => {
                    let Ok(stream) = acceptor.accept(stream).await else {
                        return;
                    };
                    let _ = hyper::server::conn::http1::Builder::new()
                        .serve_connection(TokioIo::new(stream), svc)
                        .await;
                }
                None => {
                    let _ = hyper::server::conn::http1::Builder::new()
                        .serve_connection(TokioIo::new(stream), svc)
                        .await;
                }
            }
        });
    }
}

fn text(status: StatusCode, body: impl Into<Bytes>) -> Response<Full<Bytes>> {
    // SAFETY: `body()` only fails on invalid header state set earlier in the builder chain —
    // never called here, and `body` (arbitrary bytes, no header validation) can't trigger it.
    // Every caller passes a fixed StatusCode and our own response bytes, never attacker input.
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
        (Method::GET, "/local/invite") => {
            // The enrollment payload (contains the group secret — trusted screen only), for
            // the local console to render as a QR. Loopback-gated like every /local seam.
            if peer_ip != "127.0.0.1" && peer_ip != "::1" {
                text(StatusCode::FORBIDDEN, "local only")
            } else {
                local_invite(&dir)
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
        // Recognise a guest, or hold them off (ADR-0020). Loopback-only, same trust class as
        // /local/gate: a human at this machine's own console is the authority, and ADR-0020 lets
        // ANY active member decide rather than only a steward.
        (Method::POST, "/local/standing") => {
            if peer_ip != "127.0.0.1" && peer_ip != "::1" {
                text(StatusCode::FORBIDDEN, "local only")
            } else {
                match collect(req).await {
                    Ok(b) => local_standing(&dir, &b),
                    Err(_) => text(StatusCode::BAD_REQUEST, "bad body"),
                }
            }
        }
        (Method::POST, "/local/observe") => {
            if peer_ip != "127.0.0.1" && peer_ip != "::1" {
                text(StatusCode::FORBIDDEN, "local only")
            } else {
                match collect(req).await {
                    Ok(b) => local_observe(&dir, &b),
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
        // Rendezvous (ADR-0012): a familiar registers its mesh here; a fresh device reads the
        // directory to find a door. The directory holds no secret and admits no one.
        (Method::POST, "/mesh/rendezvous-register") => {
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
            recv_rendezvous_register(&dir, &bytes, &sig)
        }
        (Method::GET, "/mesh/rendezvous") => rendezvous_directory(&dir),
        // Status hub (ADR-0017): a member heartbeats its own status here; anyone reads the live
        // mesh-wide directory. Status only — no secret, admits no one.
        // A member's decision about a guest (ADR-0020). Signed like a status heartbeat; any
        // active member may vote, and the first decision wins.
        (Method::POST, "/mesh/standing") => {
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
            recv_standing_vote(&dir, &bytes, &sig)
        }
        // The second filter (ADR-0026): a guest presents evidence of who it serves; the rules
        // engine — not a human — decides, and the decision is a signed, attributable fact.
        (Method::POST, "/mesh/introduce") => {
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
            recv_introduce(&dir, &bytes, &sig, &peer_ip, relayed)
        }
        // The mesh games: a member's signed move. The door runs the rules; the console only
        // renders. One game at a time, judged deterministically — the familiar is the referee.
        (Method::POST, "/mesh/game/act") => {
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
            recv_game_act(&dir, &bytes, &sig)
        }
        // Record replication (ADR-0026): a sibling door offers its recent records; merge
        // reconciles. GET is this door's own offer, POST accepts a sibling's — both are called
        // by the dial-OUT side of a gossip exchange, so CGNAT'd doors sync in both directions.
        (Method::GET, "/mesh/records") => offer_records(&dir),
        (Method::POST, "/mesh/record-sync") => {
            let bytes = match collect(req).await {
                Ok(b) => b,
                Err(_) => return Ok(text(StatusCode::BAD_REQUEST, "bad body")),
            };
            recv_record_sync(&dir, &bytes)
        }
        // E2 traveling over the mesh: an established device of the claimed handle confirms
        // "that new device is mine" from its own console — same rules engine as a handoff.
        (Method::POST, "/mesh/vouch") => {
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
            recv_vouch(&dir, &bytes, &sig)
        }
        // A member's deliberate reversal (ADR-0026 §5): sever / disestablish / hold / restore.
        // Corrections travel; approval never existed to.
        (Method::POST, "/mesh/correct") => {
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
            recv_correction(&dir, &bytes, &sig)
        }
        (Method::POST, "/mesh/status") => {
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
            recv_status(&dir, &bytes, &sig)
        }
        (Method::GET, "/mesh/status") => status_directory_response(&dir),
        // Device oracle (ADR-0014): a member device pulls pending prompts and pushes back answers.
        (Method::POST, "/mesh/consult") => {
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
            recv_consult_pull(&dir, &bytes, &sig)
        }
        (Method::POST, "/mesh/consult-answer") => {
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
            recv_consult_answer(&dir, &bytes, &sig)
        }
        // Broker (ADR-0014): another familiar hands us the consults it is waiting on, so a device
        // that can only reach *us* can answer them, and collects the answers we hold for it.
        (Method::POST, "/mesh/consult-relay") => {
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
            recv_consult_relay(&dir, &bytes, &sig)
        }
        (Method::GET, p) if p.starts_with("/mesh/tool/") => {
            let id = p.trim_start_matches("/mesh/tool/");
            serve_tool(&dir, id)
        }
        (Method::POST, "/mesh/tool-push") => match collect(req).await {
            Ok(b) => push_tool(&dir, &b),
            Err(_) => text(StatusCode::BAD_REQUEST, "bad body"),
        },
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
    let v = serde_json::from_slice::<serde_json::Value>(body).unwrap_or_default();
    let text_val = v
        .get("text")
        .and_then(|s| s.as_str())
        .unwrap_or_default()
        .to_string();
    let t = text_val.trim();
    if t.is_empty() {
        return text(StatusCode::BAD_REQUEST, "empty");
    }
    // An answer aimed at a specific THREAD attaches as that thread's evidence and travels
    // with its pursuit (kernel::thread::add_answer) — never a dead end. An untargeted
    // answer is the console channel: recorded, and the open question retired.
    if let Some(thread_id) = v.get("thread").and_then(|s| s.as_str()) {
        let now = now_secs();
        let _ = familiar_kernel::thread::add_answer(dir, thread_id, t, now);
        let obs = familiar_kernel::observation::Observation::new(
            "ian",
            "answered",
            t,
            format!("thread:{thread_id}"),
            "local",
            now,
            1.0,
        );
        let _ = familiar_kernel::observation::record(dir, obs);
        return text(StatusCode::OK, "ok");
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

/// `GET /local/invite` → the invite payload a new device scans/pastes: every address the mesh
/// answers at, the TLS pins, and a **single-use ten-minute invite token** (ADR-0026 E3) — the
/// minting member's deliberate act, displaced in time. **It no longer carries the group
/// secret**: an invite was never supposed to be the power to mint members, and under the
/// two-filter door the token establishes identity while the knock itself earns the guest cert.
fn local_invite(dir: &Path) -> Response<Full<Bytes>> {
    let Some(cred) = group::load(dir).ok().flatten() else {
        return text(StatusCode::SERVICE_UNAVAILABLE, "no group");
    };
    let cfg = config::load(dir).unwrap_or_default();
    let port = cfg.gossip_port;
    // Every address the mesh answers at: reachable (tailnet, LAN) first, then the
    // rendezvous/lighthouse public address so a device off-LAN keeps a failover candidate
    // (ADR-0012). Same set `mesh qr` carries — the two invite paths must not disagree.
    let mut hosts = reachable_hosts();
    for h in &cfg.rendezvous_hosts {
        if !hosts.contains(h) {
            hosts.push(h.clone());
        }
    }
    let invite = crate::node::NodeKey::load_or_mint(dir, &cred.label)
        .ok()
        .and_then(|node| {
            crate::record::mint_invite_token(&node, &cred.membership, "", now_secs()).ok()
        });
    let payload = serde_json::json!({
        "v": 2,
        "group": cred.group_id,
        "label": cred.label,
        "host": hosts.first().cloned().unwrap_or_default(),
        "hosts": hosts,
        "port": port,
        "tlspin": tls_spki_pin(dir).unwrap_or_default(),
        // Every pin a device may meet across the group's hosts — so a failover target's cert
        // is accepted (ADR-0012). `tlspin` stays for v1 clients that pin a single node.
        "pins": advertised_pins(dir),
        // The E3 token. A scanning device knocks (guest), then presents this to establish.
        "invite": invite,
    });
    text(StatusCode::OK, payload.to_string())
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
        "allow_microphone" => b.allow_microphone = open,
        "allow_location" => b.allow_location = open,
        "allow_motion" => b.allow_motion = open,
        "allow_network_discovery" => b.allow_network_discovery = open,
        "allow_face_recognition" => b.allow_face_recognition = open,
        "allow_network" => b.allow_network = open,
        "allow_mesh" => b.allow_mesh = open,
        "allow_execute" => b.allow_execute = open,
        "allow_authored_execute" => b.allow_authored_execute = open,
        "allow_agent" => b.allow_agent = open,
        "allow_tool_install" => b.allow_tool_install = open,
        "allow_self_upgrade" => b.allow_self_upgrade = open,
        "allow_outreach" => b.allow_outreach = open,
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

/// `POST /local/observe {"actor":"host","action":"...","object":"...","context":"","confidence":0.9}`
/// → the local console records an observation directly, no mesh signature (loopback-gated by the
/// caller, same trust class as `/local/answer`). Closes two gaps macOS specifically has: it has no
/// NodeKey/membership cert of its own to sign a `/mesh/observe` push with (unlike iOS/watchOS,
/// which already have that path), so network-discovery findings and confirmed face identities had
/// nowhere real to go — this is that seam. `actor`/`context`/`confidence` are optional; `action`
/// and `object` are required. A `"recognized" "face:<name>"` pair also reaches the identity
/// registry, same as the signed mesh path (`observe::ingest_observations`).
fn local_observe(dir: &Path, body: &[u8]) -> Response<Full<Bytes>> {
    let v = match serde_json::from_slice::<serde_json::Value>(body) {
        Ok(v) => v,
        Err(_) => return text(StatusCode::BAD_REQUEST, "bad json"),
    };
    let action = v
        .get("action")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .trim();
    let object = v
        .get("object")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .trim();
    if action.is_empty() || object.is_empty() {
        return text(StatusCode::BAD_REQUEST, "action and object are required");
    }
    let actor = v.get("actor").and_then(|s| s.as_str()).unwrap_or("host");
    let context = v.get("context").and_then(|s| s.as_str()).unwrap_or("");
    let confidence = v
        .get("confidence")
        .and_then(|c| c.as_f64())
        .unwrap_or(0.9)
        .clamp(0.0, 1.0);
    let now = now_secs();
    let _ = familiar_kernel::identity::maybe_learn_from_observation(dir, action, object, now);
    let obs = familiar_kernel::observation::Observation::new(
        actor, action, object, context, "local", now, confidence,
    );
    match familiar_kernel::observation::record(dir, obs) {
        Ok(_) => text(StatusCode::OK, "ok"),
        Err(_) => text(StatusCode::INTERNAL_SERVER_ERROR, "record"),
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
    if can_admit(dir) {
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
    if !unknown || can_admit(dir) || relayed {
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

/// Whether this node can admit a knock itself: it holds the group secret (a founding door) or
/// a live minting warrant (ADR-0026 §6 — any warranted member is a door). Everyone else relays.
fn can_admit(dir: &Path) -> bool {
    group::load(dir)
        .ok()
        .flatten()
        .map(|c| c.can_mint())
        .unwrap_or(false)
        || group::load_warrant(dir, now_secs()).is_some()
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
        // Recently denied — 429 with the wait, so the asking device backs off instead of
        // hammering, and so the answer is legibly "not yet" rather than a silent drop.
        Ok(crate::enroll::Submitted::Denied { retry_in }) => {
            let mut r = text(
                StatusCode::TOO_MANY_REQUESTS,
                format!("denied — may ask again in {retry_in}s"),
            );
            if let Ok(v) = hyper::header::HeaderValue::from_str(&retry_in.to_string()) {
                r.headers_mut().insert(hyper::header::RETRY_AFTER, v);
            }
            r
        }
        Err(crate::Error::Untrusted(m)) => text(StatusCode::FORBIDDEN, m),
        Err(_) => text(StatusCode::BAD_REQUEST, "bad enroll request"),
    }
}

/// `POST /mesh/rendezvous-register` → a familiar lists its mesh in the directory (ADR-0012). The
/// registering node signs the raw body (`X-Familiar-Sig`); the entry is kept only if that signature
/// and the membership are self-consistent. 200 + the stored Entry; 403 untrusted; 400 malformed.
/// Listing a door admits no one — the group secret never touches this path.
fn recv_rendezvous_register(dir: &Path, bytes: &[u8], sig: &str) -> Response<Full<Bytes>> {
    let reg: crate::rendezvous::Registration = match serde_json::from_slice(bytes) {
        Ok(r) => r,
        Err(_) => return text(StatusCode::BAD_REQUEST, "bad registration"),
    };
    if reg.verify_sig(bytes, sig).is_err() {
        return text(StatusCode::FORBIDDEN, "signature did not verify");
    }
    match crate::rendezvous::register(dir, &reg, now_secs()) {
        Ok(entry) => match serde_json::to_vec(&entry) {
            Ok(b) => text(StatusCode::OK, b),
            Err(_) => text(StatusCode::INTERNAL_SERVER_ERROR, "entry encode"),
        },
        Err(crate::Error::Untrusted(m)) => text(StatusCode::FORBIDDEN, m),
        Err(_) => text(StatusCode::BAD_REQUEST, "registration rejected"),
    }
}

/// `GET /mesh/rendezvous` → the live directory of meshes registered here, for a device to discover
/// a door. Labels and addresses only — no secret, no raw group id (ADR-0012).
fn rendezvous_directory(dir: &Path) -> Response<Full<Bytes>> {
    let entries = crate::rendezvous::directory(dir, now_secs());
    match serde_json::to_vec(&entries) {
        Ok(b) => text(StatusCode::OK, b),
        Err(_) => text(StatusCode::INTERNAL_SERVER_ERROR, "directory encode"),
    }
}

/// `POST /local/standing` → `{ act: "grant"|"deny", node_id }`. Recognise a waiting guest, or
/// hold them off for [`crate::enroll::DENY_RETRY_SECS`]. Nothing here auto-grants and nothing
/// removes a member — "not now" narrows what a stranger sees and starts a short retry window;
/// removing a member is `mesh abandon`, a different and heavier act.
fn local_standing(dir: &Path, body: &[u8]) -> Response<Full<Bytes>> {
    #[derive(serde::Deserialize)]
    struct Decision {
        act: String,
        node_id: String,
    }
    let Ok(d) = serde_json::from_slice::<Decision>(body) else {
        return text(StatusCode::BAD_REQUEST, "expected {act, node_id}");
    };
    if d.node_id.trim().is_empty() {
        return text(StatusCode::BAD_REQUEST, "empty node_id");
    }
    match d.act.as_str() {
        "grant" => match crate::standing::grant(dir, &d.node_id, "recognised from the console") {
            Ok(_) => text(StatusCode::OK, "recognised"),
            Err(e) => text(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        },
        "deny" => {
            let now = now_secs();
            let _ = crate::standing::revoke(dir, &d.node_id);
            match crate::enroll::deny(dir, &d.node_id, now) {
                Ok(_) => text(StatusCode::OK, "held off"),
                Err(e) => text(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            }
        }
        other => text(
            StatusCode::BAD_REQUEST,
            format!("act {other}? — grant | deny"),
        ),
    }
}

/// `POST /mesh/status` → a member heartbeats its own status (ADR-0017). Signed + membership-bearing;
/// kept only for the sender's own node. 200 + the stored status; 403 untrusted; 400 malformed.
fn recv_status(dir: &Path, bytes: &[u8], sig: &str) -> Response<Full<Bytes>> {
    let report: crate::status::StatusReport = match serde_json::from_slice(bytes) {
        Ok(r) => r,
        Err(_) => return text(StatusCode::BAD_REQUEST, "bad status"),
    };
    if report.verify_sig(bytes, sig).is_err() {
        return text(StatusCode::FORBIDDEN, "signature did not verify");
    }
    match crate::status::record(dir, &report, now_secs()) {
        Ok(entry) => match serde_json::to_vec(&entry) {
            Ok(b) => text(StatusCode::OK, b),
            Err(_) => text(StatusCode::INTERNAL_SERVER_ERROR, "status encode"),
        },
        Err(crate::Error::Untrusted(m)) => text(StatusCode::FORBIDDEN, m),
        Err(_) => text(StatusCode::BAD_REQUEST, "status rejected"),
    }
}

/// `POST /mesh/standing` → a member recognises a guest, or holds them off.
///
/// This is what lets a phone decide: it has no data dir to write and no daemon beside it, so the
/// decision has to travel. It goes to whichever node is serving — in practice the minting door —
/// and everyone else converges on that roll at the next exchange.
///
/// 409 on an already-decided node rather than an overwrite: two consoles must not produce a roll
/// that flips with packet order.
fn recv_standing_vote(dir: &Path, bytes: &[u8], sig: &str) -> Response<Full<Bytes>> {
    let vote: crate::standing::StandingVote = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(_) => return text(StatusCode::BAD_REQUEST, "bad vote"),
    };
    if vote.verify_sig(bytes, sig).is_err() {
        return text(StatusCode::FORBIDDEN, "signature did not verify");
    }
    let now = now_secs();
    // The voter must be a member in good standing of the group it names — same check the status
    // channel runs, and the reason a stranger cannot vote itself in.
    if crate::group::verify_membership_consistent(&vote.membership, &vote.group_pubkey, now)
        .is_err()
    {
        return text(StatusCode::FORBIDDEN, "membership does not verify");
    }
    match crate::standing::apply_vote(dir, &vote, now) {
        Ok(word) => text(StatusCode::OK, word),
        Err(crate::Error::Untrusted(m)) if m.contains("already decided") => {
            text(StatusCode::CONFLICT, m)
        }
        Err(crate::Error::Untrusted(m)) => text(StatusCode::FORBIDDEN, m),
        Err(e) => text(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

/// Where an introduction was actually made, as this door observed it — never as the introducer
/// claims. Loopback is this node's own console (the shared-iPad case: the device itself is the
/// provenance). A private or tailnet address is the mesh's own network — a member device (this
/// one, at minimum) is colocated there. Everything else — public internet, or a relayed hop —
/// is nowhere the mesh inhabits, and establishes nothing (ADR-0026's second guardrail).
fn observed_provenance(self_node_id: &str, peer_ip: &str, relayed: bool) -> crate::record::Provenance {
    use crate::record::Provenance;
    if relayed {
        return Provenance::Remote;
    }
    if peer_ip == "127.0.0.1" || peer_ip == "::1" {
        return Provenance::EstablishedDevice {
            device_node_id: self_node_id.to_string(),
        };
    }
    let private = match peer_ip.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(v4)) => {
            let o = v4.octets();
            v4.is_private()
                || v4.is_link_local()
                // The tailnet (CGNAT range) is the household's own overlay — a stranger
                // cannot be on it.
                || (o[0] == 100 && (64..128).contains(&o[1]))
        }
        Ok(std::net::IpAddr::V6(v6)) => {
            let s = v6.segments();
            (s[0] & 0xfe00) == 0xfc00 || (s[0] & 0xffc0) == 0xfe80
        }
        Err(_) => false,
    };
    if private {
        crate::record::Provenance::MemberColocatedNetwork
    } else {
        crate::record::Provenance::Remote
    }
}

/// Every established member device this door can anchor evidence to: node ids + handles from
/// the records, public keys from the grants this door minted (its own cert included). A device
/// admitted elsewhere still guards the existing-handle rule; it just cannot anchor an E1/E2
/// until its key is known here — the warrant work (Phase 4) closes that.
fn established_devices(dir: &Path, cred: &group::GroupCredential) -> Vec<crate::record::EstablishedDeviceRef> {
    let mut pubkeys: std::collections::HashMap<String, String> = crate::enroll::list_grants(dir)
        .into_iter()
        .map(|g| (g.membership.node_id.clone(), g.membership.node_pubkey.clone()))
        .collect();
    pubkeys.insert(
        cred.membership.node_id.clone(),
        cred.membership.node_pubkey.clone(),
    );
    let mut out = Vec::new();
    for r in crate::record::load_all(dir) {
        if crate::record::derive_state(&r) != crate::record::RecordState::Member {
            continue;
        }
        let Some(est) = &r.identity.established else {
            continue;
        };
        for key in &r.keys {
            // The record's own pubkey covers the CURRENT key at doors that never granted this
            // device (record-sync brought the record; there is no local grant to consult).
            let fallback = if *key == r.device_id { r.pubkey.clone() } else { String::new() };
            out.push(crate::record::EstablishedDeviceRef {
                node_id: key.clone(),
                pubkey: pubkeys.get(key).cloned().unwrap_or(fallback),
                handle: est.handle.clone(),
            });
        }
    }
    out
}

/// `POST /mesh/introduce` → the identity filter. Verifies the sender holds its key and has
/// knocked here (the contract filter), replaces any claimed E4 provenance with what the door
/// itself observed, runs the rules engine, and — on yes — admits: record, roll, feed. The 403
/// text on refusal is exactly what the guest's console shows as the path to admission.
fn recv_introduce(
    dir: &Path,
    bytes: &[u8],
    sig: &str,
    peer_ip: &str,
    relayed: bool,
) -> Response<Full<Bytes>> {
    let req: crate::record::IntroduceRequest = match serde_json::from_slice(bytes) {
        Ok(r) => r,
        Err(_) => return text(StatusCode::BAD_REQUEST, "bad introduce request"),
    };
    let now = now_secs();
    if req.node.verify(bytes, sig).is_err() {
        return text(StatusCode::FORBIDDEN, "signature did not verify");
    }
    let Ok(pk) = crate::hex_decode(&req.node.pubkey) else {
        return text(StatusCode::BAD_REQUEST, "bad pubkey");
    };
    match crate::exactly_32(&pk, "node pubkey") {
        Ok(arr) if crate::node::fingerprint(&arr) == req.node.node_id => {}
        _ => return text(StatusCode::FORBIDDEN, "node_id ≠ pubkey fingerprint"),
    }
    let Some(cred) = group::load(dir).ok().flatten() else {
        return text(StatusCode::SERVICE_UNAVAILABLE, "no group here");
    };
    let Some(rec) = crate::record::find_by_key(dir, &req.node.node_id) else {
        return text(
            StatusCode::FORBIDDEN,
            "knock first — the covenant handshake is the first filter",
        );
    };
    if let Some(held) = rec.held_until {
        if now < held {
            let mut r = text(
                StatusCode::TOO_MANY_REQUESTS,
                format!("held — try again in {}s", held - now),
            );
            if let Ok(v) = hyper::header::HeaderValue::from_str(&(held - now).to_string()) {
                r.headers_mut().insert(hyper::header::RETRY_AFTER, v);
            }
            return r;
        }
    }
    if crate::record::derive_state(&rec) == crate::record::RecordState::Member {
        let handle = rec
            .identity
            .established
            .as_ref()
            .map(|e| e.handle.clone())
            .unwrap_or_default();
        return text(
            StatusCode::OK,
            serde_json::json!({"state": "member", "handle": handle}).to_string(),
        );
    }
    let covenant_attested =
        rec.attestation.is_some() || crate::enroll::has_grant(dir, &req.node.node_id);

    // The door's own observation outranks whatever provenance the introducer claimed.
    let evidence = match req.evidence.clone() {
        crate::record::Evidence::Introduction { intro, .. } => {
            crate::record::Evidence::Introduction {
                intro,
                provenance: observed_provenance(&cred.membership.node_id, peer_ip, relayed),
            }
        }
        e => e,
    };

    let established = established_devices(dir, &cred);
    let ctx = crate::record::AdmissionContext {
        now,
        group_id: &cred.group_id,
        group_pubkey: &cred.group_pubkey,
        established: &established,
    };
    let subject = crate::record::Subject {
        node_id: &req.node.node_id,
        covenant_attested,
    };
    match crate::record::evaluate_admission(&subject, req.claim.as_ref(), &evidence, &ctx) {
        Ok(est) => {
            // Single-use before mint: a spent-but-failed admission wastes a cheap token; the
            // other order would let one token admit twice.
            if let crate::record::Evidence::Invite(t) = &evidence {
                if let Err(e) = crate::record::spend_invite(dir, &t.token_id) {
                    return text(StatusCode::FORBIDDEN, e.to_string());
                }
            }
            let handle = est.handle.clone();
            match crate::record::admit(
                dir,
                &req.node.node_id,
                req.claim.clone(),
                est,
                &cred.membership.node_id,
                now,
            ) {
                Ok(_) => {
                    // The key the door just verified travels WITH the record (record-sync) —
                    // a sibling door can then check a voucher against it with no grant of its own.
                    let _ = crate::record::record_pubkey(dir, &req.node.node_id, &req.node.pubkey, now);
                    let label = if req.node.label.trim().is_empty() {
                        req.node.node_id.chars().take(8).collect()
                    } else {
                        req.node.label.trim().to_string()
                    };
                    let obs = familiar_kernel::observation::Observation::new(
                        format!("device:{label}"),
                        "was established",
                        &if handle.is_empty() {
                            "and admitted to the mesh".to_string()
                        } else {
                            format!("as {handle} — admitted to the mesh")
                        },
                        "mesh",
                        "mesh",
                        now,
                        1.0,
                    );
                    let _ = familiar_kernel::observation::record(dir, obs);
                    text(
                        StatusCode::OK,
                        serde_json::json!({"state": "member", "handle": handle}).to_string(),
                    )
                }
                Err(e) => text(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            }
        }
        // The refusal text IS the guest's path-to-admission copy — pass it through verbatim.
        // But a claim ADDRESSES even when the evidence fails (ADR-0019): keep it on the record,
        // with the claimant's key, so the claimed human's own devices see "this device says it
        // is yours" and one of them can vouch (E2 over the mesh) — the automatic path that
        // needs no invite paste and no QR. Only claims naming an ESTABLISHED handle are kept;
        // an unknown name has nobody to ask.
        Err(crate::Error::Untrusted(m)) => {
            let claimed_existing = req
                .claim
                .as_ref()
                .map(|c| {
                    established
                        .iter()
                        .any(|d| d.handle.eq_ignore_ascii_case(c.handle.trim()))
                })
                .unwrap_or(false);
            if claimed_existing {
                let claim = req.claim.as_ref().expect("checked above");
                let _ =
                    crate::record::record_claim(dir, &req.node.node_id, claim, &req.node.pubkey, now);
                return text(
                    StatusCode::FORBIDDEN,
                    format!("{m} — {}'s devices have been asked to confirm this one is theirs", claim.handle.trim()),
                );
            }
            text(StatusCode::FORBIDDEN, m)
        }
        Err(_) => text(StatusCode::BAD_REQUEST, "bad evidence"),
    }
}

/// The vouching side of E2, traveling over the mesh: `POST /mesh/vouch` — an established
/// device of the claimed handle confirms "that new device is mine" from its own console. The
/// voucher is minted on the vouching device (its key, its signature) and the door runs the
/// same rules engine an in-person handoff would: nothing here is a second admission path,
/// only a second way for the same evidence to arrive.
#[derive(serde::Deserialize)]
struct VouchEnvelope {
    node: crate::node::NodeIdentity,
    voucher: crate::record::DeviceVoucher,
}

fn recv_vouch(dir: &Path, bytes: &[u8], sig: &str) -> Response<Full<Bytes>> {
    let env: VouchEnvelope = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(_) => return text(StatusCode::BAD_REQUEST, "bad vouch"),
    };
    let now = now_secs();
    if env.node.verify(bytes, sig).is_err() {
        return text(StatusCode::FORBIDDEN, "signature did not verify");
    }
    let Ok(pk) = crate::hex_decode(&env.node.pubkey) else {
        return text(StatusCode::BAD_REQUEST, "bad pubkey");
    };
    match crate::exactly_32(&pk, "node pubkey") {
        Ok(arr) if crate::node::fingerprint(&arr) == env.node.node_id => {}
        _ => return text(StatusCode::FORBIDDEN, "node_id ≠ pubkey fingerprint"),
    }
    // The envelope must be signed by the device that minted the voucher — nobody vouches in
    // someone else's name.
    if env.node.node_id != env.voucher.voucher_node_id {
        return text(StatusCode::FORBIDDEN, "the voucher is not the sender's own");
    }
    let Some(cred) = group::load(dir).ok().flatten() else {
        return text(StatusCode::SERVICE_UNAVAILABLE, "no group here");
    };
    // The subject: looked up by the vouched key's fingerprint — the record the claim landed on.
    let Ok(subj_pk) = crate::hex_decode(&env.voucher.subject_pubkey) else {
        return text(StatusCode::BAD_REQUEST, "bad subject pubkey");
    };
    let subject_node_id = match crate::exactly_32(&subj_pk, "subject pubkey") {
        Ok(arr) => crate::node::fingerprint(&arr),
        Err(_) => return text(StatusCode::BAD_REQUEST, "bad subject pubkey"),
    };
    let Some(rec) = crate::record::find_by_key(dir, &subject_node_id) else {
        return text(StatusCode::FORBIDDEN, "no such guest — they must knock first");
    };
    if crate::record::derive_state(&rec) == crate::record::RecordState::Member {
        let handle = rec
            .identity
            .established
            .as_ref()
            .map(|e| e.handle.clone())
            .unwrap_or_default();
        return text(
            StatusCode::OK,
            serde_json::json!({"state": "member", "handle": handle}).to_string(),
        );
    }
    let covenant_attested =
        rec.attestation.is_some() || crate::enroll::has_grant(dir, &subject_node_id);
    let established = established_devices(dir, &cred);
    let ctx = crate::record::AdmissionContext {
        now,
        group_id: &cred.group_id,
        group_pubkey: &cred.group_pubkey,
        established: &established,
    };
    let subject = crate::record::Subject {
        node_id: &subject_node_id,
        covenant_attested,
    };
    let evidence = crate::record::Evidence::Voucher(env.voucher.clone());
    match crate::record::evaluate_admission(&subject, rec.identity.claim.as_ref(), &evidence, &ctx)
    {
        Ok(est) => {
            let handle = est.handle.clone();
            match crate::record::admit(
                dir,
                &subject_node_id,
                rec.identity.claim.clone(),
                est,
                &cred.membership.node_id,
                now,
            ) {
                Ok(_) => {
                    let obs = familiar_kernel::observation::Observation::new(
                        format!("device:{}", env.node.label),
                        "vouched",
                        format!("for a new device of {handle} — admitted to the mesh"),
                        "mesh",
                        "mesh",
                        now,
                        1.0,
                    );
                    let _ = familiar_kernel::observation::record(dir, obs);
                    text(
                        StatusCode::OK,
                        serde_json::json!({"state": "member", "handle": handle}).to_string(),
                    )
                }
                Err(e) => text(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            }
        }
        Err(crate::Error::Untrusted(m)) => text(StatusCode::FORBIDDEN, m),
        Err(_) => text(StatusCode::BAD_REQUEST, "bad evidence"),
    }
}

/// Who can hold a turn: standing-full, human-facing devices — phones, iPads, consoles. The
/// daemons and the lighthouse keep score and judge; they don't take turns at the fire.
fn game_players(dir: &Path, now: i64) -> Vec<crate::game::Player> {
    let roll = crate::standing::load(dir);
    crate::members::classify(dir, now)
        .into_iter()
        .filter(|m| {
            let console = m.label.to_lowercase().ends_with(" console");
            let device = m.kind == crate::members::MemberKind::DevicePeer;
            (device || console)
                && roll.full.iter().any(|n| n == &m.node_id)
                && m.status != "offline"
        })
        .map(|m| crate::game::Player {
            node_id: m.node_id,
            label: m.label,
            handle: m.human,
            score: 0,
            strikes: 0,
            eliminated: false,
        })
        .collect()
}

/// The signed wrapper every game act arrives in — same proof shape as an introduce.
#[derive(serde::Deserialize)]
struct GameActEnvelope {
    node: crate::node::NodeIdentity,
    #[serde(flatten)]
    act: crate::game::GameAct,
    #[allow(dead_code)]
    ts: i64,
    #[allow(dead_code)]
    nonce: String,
}

/// `POST /mesh/game/act` → verify the member and apply the move. The reply body is the
/// judge's words ("✓ solved!", "not it — the ember moves on"), shown to the player verbatim.
fn recv_game_act(dir: &Path, bytes: &[u8], sig: &str) -> Response<Full<Bytes>> {
    let env: GameActEnvelope = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(_) => return text(StatusCode::BAD_REQUEST, "bad game act"),
    };
    let now = now_secs();
    if env.node.verify(bytes, sig).is_err() {
        return text(StatusCode::FORBIDDEN, "signature did not verify");
    }
    let Ok(pk) = crate::hex_decode(&env.node.pubkey) else {
        return text(StatusCode::BAD_REQUEST, "bad pubkey");
    };
    match crate::exactly_32(&pk, "node pubkey") {
        Ok(arr) if crate::node::fingerprint(&arr) == env.node.node_id => {}
        _ => return text(StatusCode::FORBIDDEN, "node_id ≠ pubkey fingerprint"),
    }
    if crate::standing::standing_of(dir, &env.node.node_id) != crate::standing::Standing::Full {
        return text(StatusCode::FORBIDDEN, "members only — the fire is inside the house");
    }
    let mut state = crate::game::load(dir);
    let players = if env.act.act == "begin" {
        game_players(dir, now)
    } else {
        Vec::new()
    };
    let label = if env.node.label.trim().is_empty() {
        env.node.node_id.chars().take(8).collect()
    } else {
        env.node.label.trim().to_string()
    };
    match crate::game::apply_act(&mut state, &env.act, &env.node.node_id, &label, &players, now) {
        Ok(reply) => {
            if let Some(s) = &state {
                let _ = crate::game::save(dir, s);
            }
            text(StatusCode::OK, reply)
        }
        Err(crate::Error::Untrusted(m)) => text(StatusCode::FORBIDDEN, m),
        Err(e) => text(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

/// `GET /mesh/records` → this door's signed offer of recently-changed records. 204 when the
/// window is quiet — the common steady state, and the caller skips absorption entirely.
fn offer_records(dir: &Path) -> Response<Full<Bytes>> {
    let Some(cred) = group::load(dir).ok().flatten() else {
        return text(StatusCode::SERVICE_UNAVAILABLE, "no group here");
    };
    let Ok(node) = crate::node::NodeKey::load_or_mint(dir, "familiar") else {
        return text(StatusCode::SERVICE_UNAVAILABLE, "no node key");
    };
    match crate::record::build_record_sync(dir, &cred, &node, now_secs()) {
        Ok(Some(sync)) => match serde_json::to_vec(&sync) {
            Ok(body) => text(StatusCode::OK, body),
            Err(_) => text(StatusCode::INTERNAL_SERVER_ERROR, "encode"),
        },
        Ok(None) => text(StatusCode::NO_CONTENT, ""),
        Err(_) => text(StatusCode::INTERNAL_SERVER_ERROR, "assemble"),
    }
}

/// `POST /mesh/record-sync` → absorb a sibling door's records. The envelope is self-proving
/// (cert + signature inside the body, verified against OUR group key), so no header sig.
fn recv_record_sync(dir: &Path, bytes: &[u8]) -> Response<Full<Bytes>> {
    let sync: crate::record::RecordSync = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(_) => return text(StatusCode::BAD_REQUEST, "bad record-sync"),
    };
    let Some(cred) = group::load(dir).ok().flatten() else {
        return text(StatusCode::SERVICE_UNAVAILABLE, "no group here");
    };
    let Ok(gk) = cred.verifying_key() else {
        return text(StatusCode::INTERNAL_SERVER_ERROR, "bad group key");
    };
    let revoked = group::load_revoked(dir).unwrap_or_default();
    if let Err(e) =
        crate::record::verify_record_sync(&sync, &gk, &cred.group_id, now_secs(), &revoked)
    {
        return text(StatusCode::FORBIDDEN, e.to_string());
    }
    let mut absorbed = 0usize;
    for r in &sync.body.records {
        if crate::record::absorb(dir, r).is_ok() {
            absorbed += 1;
        }
    }
    if let Some(g) = &sync.body.game {
        let _ = crate::game::absorb(dir, g);
    }
    text(StatusCode::OK, format!("absorbed {absorbed}"))
}

/// The dial-out half of record replication, run right after a brief exchange: offer ours,
/// absorb theirs. Best-effort — an old peer 404s both and nothing is lost.
async fn sync_records_with(dir: &Path, addr: &str) {
    let now = now_secs();
    if let (Ok(Some(cred)), Ok(node)) = (
        group::load(dir),
        crate::node::NodeKey::load_or_mint(dir, "familiar"),
    ) {
        if let Ok(Some(ours)) = crate::record::build_record_sync(dir, &cred, &node, now) {
            if let Ok(raw) = serde_json::to_vec(&ours) {
                let _ = http_send(
                    addr,
                    Method::POST,
                    "/mesh/record-sync",
                    Some(raw),
                    &[("content-type", "application/json")],
                )
                .await;
            }
        }
        if let Ok(resp) = http_send(addr, Method::GET, "/mesh/records", None, &[]).await {
            if resp.status == StatusCode::OK {
                if let Ok(theirs) = serde_json::from_slice::<crate::record::RecordSync>(&resp.body)
                {
                    let revoked = group::load_revoked(dir).unwrap_or_default();
                    if let Ok(gk) = cred.verifying_key() {
                        if crate::record::verify_record_sync(
                            &theirs,
                            &gk,
                            &cred.group_id,
                            now,
                            &revoked,
                        )
                        .is_ok()
                        {
                            for r in &theirs.body.records {
                                let _ = crate::record::absorb(dir, r);
                            }
                            if let Some(g) = &theirs.body.game {
                                let _ = crate::game::absorb(dir, g);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// `POST /mesh/correct` → a member's deliberate reversal, traveling. Same verification shape as
/// every signed member write; a device cannot correct itself, and the correcting member must be
/// the signer — nobody corrects in someone else's name.
fn recv_correction(dir: &Path, bytes: &[u8], sig: &str) -> Response<Full<Bytes>> {
    let env: crate::record::CorrectionEnvelope = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(_) => return text(StatusCode::BAD_REQUEST, "bad correction"),
    };
    if env.verify_sig(bytes, sig).is_err() {
        return text(StatusCode::FORBIDDEN, "signature did not verify");
    }
    let now = now_secs();
    if crate::group::verify_membership_consistent(&env.membership, &env.group_pubkey, now).is_err()
    {
        return text(StatusCode::FORBIDDEN, "membership does not verify");
    }
    if env.correction.corrected_by != env.membership.node_id {
        return text(
            StatusCode::FORBIDDEN,
            "a correction is the signer's own act",
        );
    }
    match crate::record::apply_correction(dir, &env.correction, now) {
        Ok(r) => text(
            StatusCode::OK,
            serde_json::json!({"state": format!("{:?}", crate::record::derive_state(&r))})
                .to_string(),
        ),
        Err(crate::Error::Untrusted(m)) => text(StatusCode::FORBIDDEN, m),
        Err(e) => text(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

/// `GET /mesh/status` → the live mesh-wide status directory (ADR-0017): explicit heartbeats, plus
/// this node's own fresh peers surfaced as status rows (a device that reads our worldview but hasn't
/// heartbeat yet is still visible to the rest of the mesh — Phase A's fix for the "away" bug). An
/// explicit heartbeat wins over the inferred row for the same node.
fn status_directory_response(dir: &Path) -> Response<Full<Bytes>> {
    let now = now_secs();
    let mut out = crate::status::directory(dir, now);
    let seen: std::collections::HashSet<String> = out.iter().map(|s| s.node_id.clone()).collect();
    for p in load_peers(dir) {
        if p.status == "abandoned" || seen.contains(&p.node_id) {
            continue;
        }
        if now - p.last_seen > crate::status::STATUS_TTL_SECS {
            continue;
        }
        out.push(crate::status::MemberStatus {
            node_id: p.node_id.clone(),
            group_ref: String::new(),
            actor: String::new(),
            label: p.label.clone(),
            present_human: p.human.clone(),
            // Relayed from our peer roster, which records who a node SERVES, not a live claim
            // about who is at it — so this row carries no provenance and no confidence.
            present_via: String::new(),
            present_since: 0,
            present_confidence: 0.0,
            connectivity: p.connectivity.clone(),
            tailnet_addr: String::new(),
            tailnet_up: false,
            updated_at: p.last_seen,
        });
    }
    match serde_json::to_vec(&out) {
        Ok(b) => text(StatusCode::OK, b),
        Err(_) => text(StatusCode::INTERNAL_SERVER_ERROR, "status encode"),
    }
}

/// `POST /mesh/consult` → a member device pulls the prompts waiting for it (ADR-0014). Signed +
/// membership-bearing. 200 + the pending prompts; 403 untrusted; 400 malformed.
fn recv_consult_pull(dir: &Path, bytes: &[u8], sig: &str) -> Response<Full<Bytes>> {
    let pull: crate::consult::ConsultPull = match serde_json::from_slice(bytes) {
        Ok(p) => p,
        Err(_) => return text(StatusCode::BAD_REQUEST, "bad pull"),
    };
    if pull.verify_sig(bytes, sig).is_err() {
        return text(StatusCode::FORBIDDEN, "signature did not verify");
    }
    match crate::consult::accept_pull(dir, &pull, now_secs()) {
        Ok(prompts) => match serde_json::to_vec(&prompts) {
            Ok(b) => text(StatusCode::OK, b),
            Err(_) => text(StatusCode::INTERNAL_SERVER_ERROR, "consult encode"),
        },
        Err(crate::Error::Untrusted(m)) => text(StatusCode::FORBIDDEN, m),
        Err(_) => text(StatusCode::BAD_REQUEST, "consult rejected"),
    }
}

/// `POST /mesh/consult-answer` → a member device pushes its answer (ADR-0014). Signed; kept only for
/// a prompt we actually asked and only from the device that answers for its own node. 200; else 403/400.
fn recv_consult_answer(dir: &Path, bytes: &[u8], sig: &str) -> Response<Full<Bytes>> {
    let report: crate::consult::ConsultAnswerReport = match serde_json::from_slice(bytes) {
        Ok(r) => r,
        Err(_) => return text(StatusCode::BAD_REQUEST, "bad answer"),
    };
    if report.verify_sig(bytes, sig).is_err() {
        return text(StatusCode::FORBIDDEN, "signature did not verify");
    }
    match crate::consult::accept_answer(dir, &report, now_secs()) {
        Ok(()) => text(StatusCode::OK, "recorded"),
        Err(crate::Error::Untrusted(m)) => text(StatusCode::FORBIDDEN, m),
        Err(_) => text(StatusCode::BAD_REQUEST, "answer rejected"),
    }
}

/// `POST /mesh/consult-relay` → a peer familiar brokers its consults through us (ADR-0014). Signed +
/// membership-bearing, like every other write on this seam. 200 + the answers we hold for that peer;
/// 403 untrusted; 400 malformed.
fn recv_consult_relay(dir: &Path, bytes: &[u8], sig: &str) -> Response<Full<Bytes>> {
    let relay: crate::consult::ConsultRelay = match serde_json::from_slice(bytes) {
        Ok(r) => r,
        Err(_) => return text(StatusCode::BAD_REQUEST, "bad relay"),
    };
    if relay.verify_sig(bytes, sig).is_err() {
        return text(StatusCode::FORBIDDEN, "signature did not verify");
    }
    match crate::consult::accept_relay(dir, &relay, now_secs()) {
        Ok(answers) => match serde_json::to_vec(&answers) {
            Ok(b) => text(StatusCode::OK, b),
            Err(_) => text(StatusCode::INTERNAL_SERVER_ERROR, "relay encode"),
        },
        Err(crate::Error::Untrusted(m)) => text(StatusCode::FORBIDDEN, m),
        Err(_) => text(StatusCode::BAD_REQUEST, "relay rejected"),
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

/// `POST /mesh/tool-push {"manifest": ToolManifest, "body_b64": "..."}` → a peer proactively
/// hands us a tool it has that our reply brief showed we lack — the fix for a structural gap
/// `GET /mesh/tool/{id}` alone can't close: that pull only ever runs on the *dialing* side
/// (`exchange_with`), so a peer that can only ever be dialed *into* (the lighthouse — CGNAT'd
/// fleet members dial it, it can never dial them back) could never accumulate tools from
/// anyone, no matter how long they stayed peered. This is the other half: the dialer pushes
/// what the dialed side is missing, discovered from the same round-trip brief exchange.
///
/// Same trust posture as the existing pull endpoint (`serve_tool`) — no extra signature beyond
/// the TLS transport and `share_tools` being on; the content-hash check is what a forged or
/// corrupted body can't pass, exactly like the pull path a peer already trusts.
fn push_tool(dir: &Path, body: &[u8]) -> Response<Full<Bytes>> {
    let cfg = config::load(dir).unwrap_or_default();
    if !cfg.share_tools {
        return text(StatusCode::FORBIDDEN, "tool sharing disabled");
    }
    #[derive(serde::Deserialize)]
    struct Push {
        manifest: crate::brief::ToolManifest,
        body_hex: String,
        /// The pusher's own node id — same provenance field the pull-based tick-merge records
        /// (`origin: node_id`, merge.rs). Not independently re-verified at this endpoint (same
        /// trust posture as the pull path); the content-hash check is the integrity guard.
        from_node_id: String,
    }
    let Ok(push) = serde_json::from_slice::<Push>(body) else {
        return text(StatusCode::BAD_REQUEST, "bad json");
    };
    let Ok(script_body) = crate::hex_decode(&push.body_hex) else {
        return text(StatusCode::BAD_REQUEST, "bad hex");
    };
    if sha256_hex(&script_body) != push.manifest.script_sha256 {
        return text(StatusCode::BAD_REQUEST, "hash mismatch");
    }
    // Refuse a pushed tool that reaches the network: it was authored against the pusher's LAN and
    // has no honest meaning here, and installing a foreign scan/probe is exactly the intrusion the
    // outbound filter (`push_missing_tools`) already declines to spread. Defense in depth — a peer
    // on an older build, or a hostile one, doesn't get to plant one on us.
    if familiar_kernel::review::reaches_network(&String::from_utf8_lossy(&script_body)) {
        return text(
            StatusCode::FORBIDDEN,
            "network-reaching tools are not federated",
        );
    }
    if known_tool_shas(dir).contains(&push.manifest.script_sha256) {
        return text(StatusCode::OK, "already known");
    }
    let ws = crate::merge_workspace(dir);
    if std::fs::create_dir_all(&ws).is_err() {
        return text(StatusCode::INTERNAL_SERVER_ERROR, "workspace");
    }
    let seq = familiar_kernel::tool::load(dir)
        .map(|t| t.len())
        .unwrap_or(0)
        + 1;
    let id = format!("tool-{seq:04}");
    let script_path = ws.join(format!("{id}.sh"));
    if std::fs::write(&script_path, &script_body).is_err() {
        return text(StatusCode::INTERNAL_SERVER_ERROR, "write");
    }
    let t = familiar_kernel::tool::Tool {
        id,
        name: push.manifest.name,
        purpose: push.manifest.purpose,
        keywords: push.manifest.keywords.join(" "),
        script_path: script_path.display().to_string(),
        created_at: now_secs(),
        uses: 0,
        last_used: 0,
        last_exit_ok: push.manifest.last_exit_ok,
        last_status: String::new(),
        origin: push.from_node_id,
        origin_verified_at: now_secs(),
    };
    match familiar_kernel::tool::append(dir, &t) {
        Ok(_) => text(StatusCode::OK, "ok"),
        Err(_) => text(StatusCode::INTERNAL_SERVER_ERROR, "append"),
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
    // Records replicate on the same dial-out connection path (a CGNAT'd door can only be
    // reached by whoever dials out) — offer ours, absorb theirs, best-effort.
    sync_records_with(dir, addr).await;
    // Pre-fetch tool bodies we don't already have, content-addressed for the in-tick merge.
    if let Ok(brief) = serde_json::from_slice::<MeshBrief>(&reply.body) {
        upsert_peer(dir, &brief, addr)?;
        let known = known_tool_shas(dir);
        let peer_known: std::collections::HashSet<String> = brief
            .body
            .capability
            .tools
            .iter()
            .map(|t| t.script_sha256.clone())
            .collect();
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
        // The other half of the fix: push tools WE have that the peer's own brief shows it
        // lacks. Needed for any peer that can only ever be dialed *into* (the lighthouse) —
        // it would otherwise never accumulate a tool from anyone, since the pull above only
        // ever runs on the dialing side. See push_tool's doc comment.
        push_missing_tools(dir, addr, &peer_known).await;
    }
    Ok(())
}

/// Push every local tool whose content hash isn't in `peer_known` to `addr`'s
/// `/mesh/tool-push`. Best-effort: a failed push just means the peer catches it on someone
/// else's round, or the next time we dial this same peer.
async fn push_missing_tools(
    dir: &Path,
    addr: &str,
    peer_known: &std::collections::HashSet<String>,
) {
    let Ok(id) = crate::node::NodeKey::load_or_mint(dir, "familiar") else {
        return;
    };
    let our_node_id = id.identity().node_id;
    for t in familiar_kernel::tool::load(dir).unwrap_or_default() {
        let Ok(body) = std::fs::read(&t.script_path) else {
            continue;
        };
        // A tool that reaches the network is authored against *this* host's LAN — its target IPs,
        // its router, its neighbours. Replicating it to another peer plants a scan/probe that is
        // meaningless there at best and intrusive on that peer's network at worst. Keep such tools
        // local; only portable tools (local computation, text/host introspection) federate.
        if familiar_kernel::review::reaches_network(&String::from_utf8_lossy(&body)) {
            continue;
        }
        let sha = sha256_hex(&body);
        if peer_known.contains(&sha) {
            continue;
        }
        let manifest = crate::brief::ToolManifest {
            tool_id: t.id.clone(),
            name: t.name.clone(),
            purpose: t.purpose.clone(),
            keywords: t.keywords.split_whitespace().map(String::from).collect(),
            script_sha256: sha,
            uses: t.uses as u64,
            last_exit_ok: t.last_exit_ok,
        };
        let payload = serde_json::json!({
            "manifest": manifest,
            "body_hex": crate::hex_encode(&body),
            "from_node_id": our_node_id,
        });
        let Ok(bytes) = serde_json::to_vec(&payload) else {
            continue;
        };
        let _ = http_send(addr, Method::POST, "/mesh/tool-push", Some(bytes), &[]).await;
    }
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
    // Encrypt to whoever answers (payload signatures carry authenticity — see tls_connector).
    // SAFETY: the outer try_from can fail on a malformed `addr` (our own configured peer
    // address, not attacker input — this is the outbound dial path); the fallback parses the
    // hardcoded constant "familiar-mesh", a valid DNS-name-shaped string that always succeeds.
    let host_only = addr.split(':').next().unwrap_or(addr).to_string();
    let server_name = rustls::pki_types::ServerName::try_from(host_only)
        .unwrap_or_else(|_| rustls::pki_types::ServerName::try_from("familiar-mesh").unwrap());
    let stream = tls_connector()
        .connect(server_name, stream)
        .await
        .map_err(|e| crate::Error::Malformed(format!("tls: {e}")))?;
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

/// Run a child to completion or kill it at `timeout` — a hung subprocess must never hang
/// its caller. `Command::output()` has no such bound, and this is shelled out to from inside
/// async request handlers: one stuck child there is one permanently blocked tokio worker
/// thread, and enough of them stall the entire mesh server (observed 2026-07-22 — a
/// `Tailscale ip -4` child spawned by the daemon never returned, even though the same binary
/// answered instantly when run interactively; cause unconfirmed, but any hang here is now
/// bounded regardless of cause).
fn run_with_timeout(
    bin: &str,
    args: &[&str],
    timeout: std::time::Duration,
) -> Option<std::process::Output> {
    use std::io::Read;
    use std::process::Stdio;
    use std::time::Instant;
    let mut child = std::process::Command::new(bin)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(mut o) = child.stdout.take() {
                    let _ = o.read_to_end(&mut stdout);
                }
                if let Some(mut e) = child.stderr.take() {
                    let _ = e.read_to_end(&mut stderr);
                }
                return Some(std::process::Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(_) => return None,
        }
    }
}

/// Run the tailscale CLI with `args`, trying `tailscale` on PATH first and then the macOS app
/// bundle's CLI (the GUI install puts nothing on PATH). None if neither answers within 3s.
fn tailscale_output(args: &[&str]) -> Option<std::process::Output> {
    [
        "tailscale",
        "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
    ]
    .iter()
    .find_map(|bin| {
        run_with_timeout(bin, args, std::time::Duration::from_secs(3))
            .filter(|o| o.status.success())
    })
}

// ---- tailscale LocalAPI --------------------------------------------------------------
//
// The status document comes from tailscaled's LocalAPI — the localhost HTTP endpoint the
// CLI itself reads — whenever it's reachable, with the CLI shell-out only as a fallback.
// Spawning the CLI is not neutral on macOS: the GUI install's CLI *is* the Tailscale app
// binary, and launching it on every worldview poll read as Tailscale itself quitting and
// relaunching nonstop (disruptive; observed 2026-07-24).

/// Standard base64 with padding — just enough for the LocalAPI Basic-auth header,
/// hand-rolled to honor the workspace's no-new-deps discipline. Only the macOS GUI needs
/// auth (unix-socket peers are credential-trusted), hence the cfg.
#[cfg(any(target_os = "macos", test))]
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = u32::from_be_bytes([
            0,
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ]);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// One HTTP/1.0 GET over an already-connected stream, response read to EOF. HTTP/1.0 on
/// purpose: a 1.0 response can't be chunked (tailscaled answers HTTP/1.1 chunked even with
/// `Connection: close`), so close-delimited identity is the entire framing.
fn http10_get<S: std::io::Read + std::io::Write>(
    mut stream: S,
    host: &str,
    path: &str,
    auth: Option<&str>,
) -> Option<Vec<u8>> {
    let mut req = format!("GET {path} HTTP/1.0\r\nHost: {host}\r\n");
    if let Some(a) = auth {
        req.push_str(&format!("Authorization: Basic {a}\r\n"));
    }
    req.push_str("\r\n");
    stream.write_all(req.as_bytes()).ok()?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).ok()?;
    let sep = buf.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = std::str::from_utf8(&buf[..sep]).ok()?;
    if head.lines().next()?.split_whitespace().nth(1)? != "200" {
        return None;
    }
    Some(buf[sep + 4..].to_vec())
}

/// Where the macOS Tailscale GUI serves LocalAPI: `(port, token)`. The standalone /
/// system-extension install symlinks `/Library/Tailscale/ipnport` → `<port>` and keeps the
/// token in `sameuserproof-<port>` (admin-group readable); the App Store sandbox variant
/// instead drops an empty `sameuserproof-<port>-<token>` file in its group container, the
/// secret carried in the name. This is the same discovery the tailscale CLI performs.
#[cfg(target_os = "macos")]
fn macos_localapi_endpoint() -> Option<(u16, String)> {
    if let Ok(link) = std::fs::read_link("/Library/Tailscale/ipnport") {
        if let Some(port) = link.to_str().and_then(|s| s.parse::<u16>().ok()) {
            if let Ok(tok) =
                std::fs::read_to_string(format!("/Library/Tailscale/sameuserproof-{port}"))
            {
                let tok = tok.trim();
                if !tok.is_empty() {
                    return Some((port, tok.to_string()));
                }
            }
        }
    }
    let home = std::env::var("HOME").ok()?;
    let container = Path::new(&home)
        .join("Library/Group Containers/W5364U7YZB.group.tailscale.io.tailscale.ipn.macos");
    for entry in std::fs::read_dir(container).ok()?.flatten() {
        if let Some(rest) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.strip_prefix("sameuserproof-"))
        {
            if let Some((port, tok)) = rest.split_once('-') {
                if let (Ok(port), false) = (port.parse::<u16>(), tok.is_empty()) {
                    return Some((port, tok.to_string()));
                }
            }
        }
    }
    None
}

/// Fetch `/localapi/v0/status` without spawning anything. macOS GUI: TCP + Basic auth at the
/// published port. Unix servers (Linux peers/VMs, open-source tailscaled): the tailscaled
/// socket, peer-credential trusted, no token. iOS compiles the unix arm but neither path
/// exists inside the sandbox, so it answers None there — as does any host without tailscale —
/// and the caller falls back to the CLI.
fn localapi_status_json() -> Option<String> {
    #[cfg(target_os = "macos")]
    if let Some((port, token)) = macos_localapi_endpoint() {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        if let Ok(stream) =
            std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(3))
        {
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(3)));
            let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(3)));
            let auth = base64(format!(":{token}").as_bytes());
            if let Some(body) = http10_get(
                stream,
                &format!("127.0.0.1:{port}"),
                "/localapi/v0/status",
                Some(&auth),
            ) {
                return String::from_utf8(body).ok();
            }
        }
    }
    #[cfg(unix)]
    {
        let sock = Path::new("/var/run/tailscale/tailscaled.sock");
        if sock.exists() {
            if let Ok(stream) = std::os::unix::net::UnixStream::connect(sock) {
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(3)));
                let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(3)));
                if let Some(body) =
                    http10_get(stream, "local-tailscaled.sock", "/localapi/v0/status", None)
                {
                    return String::from_utf8(body).ok();
                }
            }
        }
    }
    None
}

/// The `tailscale status --json` document, however it can be had cheapest: LocalAPI first
/// (an HTTP read, no subprocess), CLI shell-out as a last resort. Cached 60s — this sits
/// under `members::classify` on the worldview path, which consoles poll every few seconds.
fn tailscale_status_doc() -> Option<String> {
    use std::sync::Mutex;
    use std::time::Instant;
    static CACHE: Mutex<Option<(Instant, Option<String>)>> = Mutex::new(None);
    let mut guard = CACHE.lock().unwrap_or_else(|p| p.into_inner());
    if let Some((at, doc)) = guard.as_ref() {
        if at.elapsed() < std::time::Duration::from_secs(60) {
            return doc.clone();
        }
    }
    let doc = localapi_status_json().or_else(|| {
        tailscale_output(&["status", "--json"])
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
    });
    *guard = Some((Instant::now(), doc.clone()));
    doc
}

/// Enumerate tailnet peers (read-only). Empty if tailscale is absent/unreachable — mesh then
/// relies on `static_peers` only.
pub fn enumerate_peers() -> Vec<TailscalePeer> {
    tailscale_status_doc()
        .map(|d| parse_tailscale_status(&d))
        .unwrap_or_default()
}

/// This node's own tailnet IPv4, from `Self.TailscaleIPs` in the (cached) status document —
/// the separate `tailscale ip -4` spawn is gone. The entry must PARSE as an IPv4 address:
/// the CLI has been seen exiting 0 while printing an error line ("The Tailscale GUI failed
/// to start…"), and an advertised address is only ever an address — a node once gossiped
/// that error text as its host and poisoned its devices' candidate lists.
pub fn self_tailnet_ip() -> Option<String> {
    parse_self_ip4(&tailscale_status_doc()?)
}

/// First entry in `Self.TailscaleIPs` that parses as IPv4 (pure — unit-tested).
pub fn parse_self_ip4(json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()?
        .get("Self")?
        .get("TailscaleIPs")?
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str())
        .find_map(|s| s.parse::<std::net::Ipv4Addr>().ok())
        .map(|ip| ip.to_string())
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
///
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

/// The most recently seen member whose fix came from its own GPS (`geo_device`). Devices
/// refresh theirs on every worldview read, so this tracks the mesh's location in
/// near-real-time. Brief-carried fixes are excluded — they may themselves be inherited.
pub fn freshest_device_fix(dir: &Path) -> Option<(f64, f64)> {
    load_peers(dir)
        .into_iter()
        .filter(|p| p.geo_device && (p.lat != 0.0 || p.lon != 0.0))
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
        geo_device: false,
        status: String::new(),
        connectivity: String::new(),
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
            // A brief without a fix (0/0) never erases a position we already know — and a
            // device-reported fix (real GPS) is never downgraded by a brief-carried one.
            let (lat, lon, geo_device) =
                if existing.geo_device && (existing.lat != 0.0 || existing.lon != 0.0) {
                    (existing.lat, existing.lon, true)
                } else if rec.lat != 0.0 || rec.lon != 0.0 {
                    (rec.lat, rec.lon, false)
                } else {
                    (existing.lat, existing.lon, existing.geo_device)
                };
            *existing = PeerRecord {
                addr: addr_keep,
                first_seen,
                session_start,
                total_online_secs,
                lat,
                lon,
                geo_device,
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
// A device report is naturally this wide (identity + versions + position); a params
// struct would only rename the width.
#[allow(clippy::too_many_arguments)]
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
    // "background" is the iOS BackgroundSync task's internal node label, not a device name — never
    // let it become the roster label (it would relabel the phone "background" on a background read).
    // Treated as empty, so the real device name from a foreground read stands.
    let label = if label == "background" { "" } else { label };
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
            existing.status = String::new(); // a fresh worldview read revives an abandoned peer
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
                existing.geo_device = true;
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
            geo_device: lat != 0.0 || lon != 0.0,
            status: String::new(),
            connectivity: String::new(),
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

/// A human's call that `node_id` is gone for good — decommissioned hardware, a retired VM
/// (`familiar mesh abandon <node_id>`). The record is never deleted, only excluded from the
/// active roster/worldview (`members::classify`) — full history stays queryable. Self-healing:
/// any fresh contact from that node (a brief, a worldview read) revives it automatically, since
/// renewed contact is itself evidence it isn't defunct after all — see `upsert_peer`/
/// `register_device_peer`. Returns `false` if no peer with that id exists.
pub fn abandon_peer(dir: &Path, node_id: &str) -> Result<bool> {
    let path = dir.join(PEERS_FILE);
    let mut peers = load_peers(dir);
    let Some(p) = peers.iter_mut().find(|p| p.node_id == node_id) else {
        return Ok(false);
    };
    p.status = "abandoned".to_string();
    std::fs::write(&path, serde_json::to_vec_pretty(&peers)?)?;
    Ok(true)
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
    use crate::node::NodeKey;

    fn fresh_dir(tag: &str) -> std::path::PathBuf {
        let p =
            std::env::temp_dir().join(format!("familiar_transport_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn body_status(resp: &Response<Full<Bytes>>) -> StatusCode {
        resp.status()
    }

    // ---- the two-filter door over the wire (ADR-0026, Phase 3) ----

    /// A door with a group, and a stranger who has knocked (guest grant in hand).
    fn door_and_guest(tag: &str) -> (std::path::PathBuf, NodeKey, group::GroupCredential, NodeKey) {
        let dir = fresh_dir(&format!("door_{tag}"));
        let host = NodeKey::load_or_mint(&dir, "door").unwrap();
        let now = now_secs();
        let cred =
            group::create_group(&dir, &host, "river", now, group::DEFAULT_CERT_TTL_SECS).unwrap();
        let guest = NodeKey::load_or_mint(&fresh_dir(&format!("door_{tag}_guest")), "Kali-Jeff")
            .unwrap();
        let req = crate::enroll::EnrollRequest {
            node: guest.identity(),
            attestation: crate::enroll::Attestation {
                laws_version: crate::enroll::LAWS_VERSION,
                statement: "I accept the Three Laws.".into(),
                ts: now,
            },
            nonce: format!("knock-{tag}"),
            ts: now,
        };
        let raw = serde_json::to_vec(&req).unwrap();
        let sig = guest.sign(&raw);
        assert!(matches!(
            crate::enroll::submit_request(&dir, &raw, &sig, now).unwrap(),
            crate::enroll::Submitted::Granted(_)
        ));
        (dir, host, cred, guest)
    }

    fn signed_introduce(
        guest: &NodeKey,
        claim: Option<crate::record::IdentityClaim>,
        evidence: crate::record::Evidence,
    ) -> (Vec<u8>, String) {
        let req = crate::record::IntroduceRequest {
            node: guest.identity(),
            claim,
            evidence,
            nonce: "i1".into(),
            ts: now_secs(),
        };
        let raw = serde_json::to_vec(&req).unwrap();
        let sig = guest.sign(&raw);
        (raw, sig)
    }

    #[test]
    fn an_invite_token_completes_admission_from_anywhere_and_is_single_use() {
        let (dir, host, cred, guest) = door_and_guest("invite");
        let token =
            crate::record::mint_invite_token(&host, &cred.membership, "jeff", now_secs()).unwrap();
        let (raw, sig) =
            signed_introduce(&guest, None, crate::record::Evidence::Invite(Box::new(token.clone())));

        // From a PUBLIC address — the inviter's deliberate act carries it.
        let resp = recv_introduce(&dir, &raw, &sig, "203.0.113.7", false);
        assert_eq!(body_status(&resp), StatusCode::OK);
        let rec = crate::record::load(&dir, &guest.node_id()).unwrap().unwrap();
        assert_eq!(crate::record::derive_state(&rec), crate::record::RecordState::Member);
        assert_eq!(rec.identity.established.as_ref().unwrap().handle, "jeff");
        assert_eq!(
            crate::standing::standing_of(&dir, &guest.node_id()),
            crate::standing::Standing::Full,
            "the legacy roll is dual-written on admit"
        );
        // The token is spent on this door — a second device cannot ride it.
        assert!(matches!(
            crate::record::spend_invite(&dir, &token.token_id),
            Err(crate::Error::Untrusted(_))
        ));
        // Re-introducing is idempotent, not an error.
        assert_eq!(body_status(&recv_introduce(&dir, &raw, &sig, "203.0.113.7", false)), StatusCode::OK);
    }

    #[test]
    fn an_introduction_needs_the_meshes_own_ground_and_cannot_take_an_existing_name() {
        let (dir, _host, _cred, guest) = door_and_guest("intro");
        let intro = crate::record::Evidence::Introduction {
            intro: crate::record::Introduction {
                handle: "jeff".into(),
                statement: "hi, I'm Jeff".into(),
                ts: now_secs(),
            },
            // Whatever the client CLAIMS, the door substitutes what it observed:
            provenance: crate::record::Provenance::Founding,
        };
        let (raw, sig) = signed_introduce(&guest, None, intro);

        // From the public internet: refused — the claimed Founding provenance is ignored.
        assert_eq!(
            body_status(&recv_introduce(&dir, &raw, &sig, "203.0.113.7", false)),
            StatusCode::FORBIDDEN
        );
        // Relayed through a member: still not the mesh's own ground.
        assert_eq!(
            body_status(&recv_introduce(&dir, &raw, &sig, "192.168.1.40", true)),
            StatusCode::FORBIDDEN
        );
        // On the mesh's own network: establishes.
        assert_eq!(
            body_status(&recv_introduce(&dir, &raw, &sig, "192.168.1.40", false)),
            StatusCode::OK
        );

        // And a second stranger typing the SAME name on the same LAN is refused — an
        // introduction never attaches to an existing identity (the impersonation guardrail).
        let (dir2, _h2, _c2, imposter) = {
            // second guest knocking at the SAME door
            let imposter =
                NodeKey::load_or_mint(&fresh_dir("door_intro_imposter"), "sneaky").unwrap();
            let now = now_secs();
            let req = crate::enroll::EnrollRequest {
                node: imposter.identity(),
                attestation: crate::enroll::Attestation {
                    laws_version: crate::enroll::LAWS_VERSION,
                    statement: "I accept the Three Laws.".into(),
                    ts: now,
                },
                nonce: "knock-imposter".into(),
                ts: now,
            };
            let raw = serde_json::to_vec(&req).unwrap();
            let sig = imposter.sign(&raw);
            crate::enroll::submit_request(&dir, &raw, &sig, now).unwrap();
            (dir.clone(), (), (), imposter)
        };
        let intro2 = crate::record::Evidence::Introduction {
            intro: crate::record::Introduction {
                handle: "Jeff".into(),
                statement: "I am Jeff".into(),
                ts: now_secs(),
            },
            provenance: crate::record::Provenance::Remote,
        };
        let (raw2, sig2) = signed_introduce(&imposter, None, intro2);
        assert_eq!(
            body_status(&recv_introduce(&dir2, &raw2, &sig2, "192.168.1.41", false)),
            StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn a_refused_claim_waits_and_the_humans_own_device_vouches_it_in() {
        let (dir, host, cred, ipad) = door_and_guest("vouch");
        // "ian" exists: the iPad established via a named invite.
        let token =
            crate::record::mint_invite_token(&host, &cred.membership, "ian", now_secs()).unwrap();
        let (raw, sig) = signed_introduce(&ipad, None, crate::record::Evidence::Invite(Box::new(token)));
        assert_eq!(body_status(&recv_introduce(&dir, &raw, &sig, "192.168.1.9", false)), StatusCode::OK);

        // A new device (the Air) knocks at the same door, then introduces itself as "ian" from
        // the public internet — refused twice over (remote provenance, existing handle) …
        let air = NodeKey::load_or_mint(&fresh_dir("door_vouch_air"), "Air console").unwrap();
        let now = now_secs();
        let knock = crate::enroll::EnrollRequest {
            node: air.identity(),
            attestation: crate::enroll::Attestation {
                laws_version: crate::enroll::LAWS_VERSION,
                statement: "I accept the Three Laws.".into(),
                ts: now,
            },
            nonce: "knock-air".into(),
            ts: now,
        };
        let kraw = serde_json::to_vec(&knock).unwrap();
        let ksig = air.sign(&kraw);
        crate::enroll::submit_request(&dir, &kraw, &ksig, now).unwrap();
        let claim = crate::record::IdentityClaim { handle: "ian".into(), ts: now };
        let intro = crate::record::Evidence::Introduction {
            intro: crate::record::Introduction {
                handle: "ian".into(),
                statement: "this Air is mine".into(),
                ts: now,
            },
            provenance: crate::record::Provenance::Remote,
        };
        let (araw, asig) = signed_introduce(&air, Some(claim), intro);
        assert_eq!(
            body_status(&recv_introduce(&dir, &araw, &asig, "203.0.113.9", false)),
            StatusCode::FORBIDDEN
        );
        // … but the claim ADDRESSES: it is on the record, with the claimant's key, so ian's
        // own devices can be shown it.
        let rec = crate::record::find_by_key(&dir, &air.node_id()).unwrap();
        assert_eq!(rec.identity.claim.as_ref().unwrap().handle, "ian");
        assert_eq!(rec.pubkey, air.identity().pubkey, "the key a voucher will name");
        assert!(rec.identity.established.is_none(), "a claim admits nothing by itself");

        // A guest cannot vouch — the Air vouching for itself is refused.
        let self_vouch =
            crate::record::DeviceVoucher::mint(&air, "ian", &air.identity().pubkey, now, "n-self")
                .unwrap();
        let env = serde_json::json!({"node": air.identity(), "voucher": self_vouch});
        let vraw = serde_json::to_vec(&env).unwrap();
        let vsig = air.sign(&vraw);
        assert_eq!(body_status(&recv_vouch(&dir, &vraw, &vsig)), StatusCode::FORBIDDEN);

        // ian's iPad vouches from its console: one tap, and the rules engine admits.
        let voucher =
            crate::record::DeviceVoucher::mint(&ipad, "ian", &air.identity().pubkey, now, "n-air")
                .unwrap();
        let env = serde_json::json!({"node": ipad.identity(), "voucher": voucher});
        let vraw = serde_json::to_vec(&env).unwrap();
        let vsig = ipad.sign(&vraw);
        assert_eq!(body_status(&recv_vouch(&dir, &vraw, &vsig)), StatusCode::OK);
        let rec = crate::record::find_by_key(&dir, &air.node_id()).unwrap();
        assert_eq!(crate::record::derive_state(&rec), crate::record::RecordState::Member);
        let est = rec.identity.established.as_ref().unwrap();
        assert_eq!(est.handle, "ian");
        assert_eq!(est.class, crate::record::EvidenceClass::DeviceVoucher);

        // Idempotent: vouching again answers member, not an error.
        assert_eq!(body_status(&recv_vouch(&dir, &vraw, &vsig)), StatusCode::OK);

        // And the envelope must be the voucher device's own signature — the host relaying the
        // iPad's voucher under its own key is refused.
        let env = serde_json::json!({"node": host.identity(), "voucher":
            crate::record::DeviceVoucher::mint(&ipad, "ian", &air.identity().pubkey, now, "n-relay").unwrap()});
        let vraw = serde_json::to_vec(&env).unwrap();
        let vsig = host.sign(&vraw);
        assert_eq!(body_status(&recv_vouch(&dir, &vraw, &vsig)), StatusCode::FORBIDDEN);
    }

    #[test]
    fn records_replicate_between_doors_and_the_vouch_loop_closes_across_them() {
        // Door A: the lighthouse — where the Air's claim lands. Door B: the household door the
        // consoles poll and the iPhone vouches at. Without sync they are private truths.
        let (dir_a, host_a, cred_a, ipad) = door_and_guest("sync_a");
        let now = now_secs();

        // Door B is a second member of the SAME group, with its own store.
        let dir_b = fresh_dir("sync_b");
        let node_b = NodeKey::load_or_mint(&dir_b, "door-b").unwrap();
        let m_b = cred_a
            .mint_membership(&node_b.node_id(), &node_b.identity().pubkey, now, 3600)
            .unwrap();
        let cred_b = crate::group::GroupCredential {
            membership: m_b,
            ..cred_a.clone()
        };
        crate::group::save_credential(&dir_b, &cred_b).unwrap();

        // "ian" established on the iPad at door A (named invite), then the Air's refused claim
        // lands at A — exactly the live shape.
        let token =
            crate::record::mint_invite_token(&host_a, &cred_a.membership, "ian", now).unwrap();
        let (raw, sig) =
            signed_introduce(&ipad, None, crate::record::Evidence::Invite(Box::new(token)));
        assert_eq!(body_status(&recv_introduce(&dir_a, &raw, &sig, "192.168.1.9", false)), StatusCode::OK);
        let air = NodeKey::load_or_mint(&fresh_dir("sync_air"), "Air console").unwrap();
        let knock = crate::enroll::EnrollRequest {
            node: air.identity(),
            attestation: crate::enroll::Attestation {
                laws_version: crate::enroll::LAWS_VERSION,
                statement: "I accept the Three Laws.".into(),
                ts: now,
            },
            nonce: "knock-air-sync".into(),
            ts: now,
        };
        let kraw = serde_json::to_vec(&knock).unwrap();
        let ksig = air.sign(&kraw);
        crate::enroll::submit_request(&dir_a, &kraw, &ksig, now).unwrap();
        let (araw, asig) = signed_introduce(
            &air,
            Some(crate::record::IdentityClaim { handle: "ian".into(), ts: now }),
            crate::record::Evidence::Introduction {
                intro: crate::record::Introduction {
                    handle: "ian".into(),
                    statement: "mine".into(),
                    ts: now,
                },
                provenance: crate::record::Provenance::Remote,
            },
        );
        assert_eq!(
            body_status(&recv_introduce(&dir_a, &araw, &asig, "203.0.113.9", false)),
            StatusCode::FORBIDDEN
        );

        // A offers its records; B absorbs — the claim (and the iPad's establishment) now
        // exist at B, where they never happened.
        let offer_a = crate::record::build_record_sync(&dir_a, &cred_a, &host_a, now)
            .unwrap()
            .expect("door A has recent records");
        let bytes = serde_json::to_vec(&offer_a).unwrap();
        assert_eq!(body_status(&recv_record_sync(&dir_b, &bytes)), StatusCode::OK);
        let rec_b = crate::record::find_by_key(&dir_b, &air.node_id()).unwrap();
        assert_eq!(rec_b.identity.claim.as_ref().unwrap().handle, "ian");
        assert_eq!(rec_b.pubkey, air.identity().pubkey);
        assert_eq!(
            crate::record::find_by_key(&dir_b, &ipad.node_id())
                .unwrap()
                .identity
                .established
                .unwrap()
                .handle,
            "ian",
            "the voucher's authority travelled too"
        );

        // The iPhone-at-door-B moment: the iPad vouches AT B. The rules engine there has
        // everything it needs — the loop closes at a door the claim never visited.
        let voucher =
            crate::record::DeviceVoucher::mint(&ipad, "ian", &air.identity().pubkey, now, "n-b")
                .unwrap();
        let env = serde_json::json!({"node": ipad.identity(), "voucher": voucher});
        let vraw = serde_json::to_vec(&env).unwrap();
        let vsig = ipad.sign(&vraw);
        assert_eq!(body_status(&recv_vouch(&dir_b, &vraw, &vsig)), StatusCode::OK);

        // And B's offer carries the admission back to A: merged, the Air is a member there too.
        let offer_b = crate::record::build_record_sync(&dir_b, &cred_b, &node_b, now)
            .unwrap()
            .expect("door B has the admission");
        let bytes = serde_json::to_vec(&offer_b).unwrap();
        assert_eq!(body_status(&recv_record_sync(&dir_a, &bytes)), StatusCode::OK);
        let rec_a = crate::record::find_by_key(&dir_a, &air.node_id()).unwrap();
        assert_eq!(crate::record::derive_state(&rec_a), crate::record::RecordState::Member);
        assert_eq!(rec_a.identity.established.unwrap().handle, "ian");

        // A stranger's self-signed sync is refused — records only travel between members.
        let stranger = NodeKey::load_or_mint(&fresh_dir("sync_stranger"), "stranger").unwrap();
        let mut forged = offer_a.clone();
        forged.body.node = stranger.identity();
        let fraw = serde_json::to_vec(&forged.body).unwrap();
        forged.sig = stranger.sign(&fraw);
        let fbytes = serde_json::to_vec(&forged).unwrap();
        assert_eq!(body_status(&recv_record_sync(&dir_b, &fbytes)), StatusCode::FORBIDDEN);
    }

    #[test]
    fn a_traveling_correction_severs_then_restores() {
        let (dir, host, cred, guest) = door_and_guest("correct");
        // Establish the guest first (via a token), so there is a member to correct.
        let token =
            crate::record::mint_invite_token(&host, &cred.membership, "jeff", now_secs()).unwrap();
        let (raw, sig) = signed_introduce(&guest, None, crate::record::Evidence::Invite(Box::new(token)));
        assert_eq!(body_status(&recv_introduce(&dir, &raw, &sig, "192.168.1.9", false)), StatusCode::OK);

        let send = |act: crate::record::CorrectionAct, nonce: &str| {
            let env = crate::record::CorrectionEnvelope {
                membership: cred.membership.clone(),
                group_pubkey: cred.group_pubkey.clone(),
                correction: crate::record::Correction {
                    act,
                    subject_device: guest.node_id(),
                    corrected_by: host.node_id(),
                    reason: "that's not Jeff".into(),
                    ts: now_secs(),
                    nonce: nonce.into(),
                    sig: String::new(),
                },
            };
            let raw = serde_json::to_vec(&env).unwrap();
            let sig = host.sign(&raw);
            recv_correction(&dir, &raw, &sig)
        };

        assert_eq!(body_status(&send(crate::record::CorrectionAct::Sever, "c1")), StatusCode::OK);
        let rec = crate::record::load(&dir, &guest.node_id()).unwrap().unwrap();
        assert!(matches!(
            crate::record::derive_state(&rec),
            crate::record::RecordState::Severed { .. }
        ));
        assert!(
            group::load_revoked(&dir).unwrap().contains(&guest.node_id()),
            "sever mirrors into revoked.json during the window"
        );

        assert_eq!(body_status(&send(crate::record::CorrectionAct::Restore, "c2")), StatusCode::OK);
        let rec = crate::record::load(&dir, &guest.node_id()).unwrap().unwrap();
        assert_eq!(crate::record::derive_state(&rec), crate::record::RecordState::Member);
        assert!(!group::load_revoked(&dir).unwrap().contains(&guest.node_id()));

        // A device cannot correct itself — even with a valid membership and signature.
        let env = crate::record::CorrectionEnvelope {
            membership: cred.membership.clone(),
            group_pubkey: cred.group_pubkey.clone(),
            correction: crate::record::Correction {
                act: crate::record::CorrectionAct::Sever,
                subject_device: host.node_id(),
                corrected_by: host.node_id(),
                reason: "no".into(),
                ts: now_secs(),
                nonce: "c3".into(),
                sig: String::new(),
            },
        };
        let raw = serde_json::to_vec(&env).unwrap();
        let sig = host.sign(&raw);
        assert_eq!(body_status(&recv_correction(&dir, &raw, &sig)), StatusCode::FORBIDDEN);
    }

    #[test]
    fn local_observe_requires_action_and_object() {
        let dir = fresh_dir("observe_missing");
        assert_eq!(
            body_status(&local_observe(&dir, br#"{"action":"discovered"}"#)),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            body_status(&local_observe(&dir, br#"{"object":"service:x"}"#)),
            StatusCode::BAD_REQUEST
        );
        assert!(familiar_kernel::observation::load(&dir).unwrap().is_empty());
    }

    #[test]
    fn local_observe_records_a_discovery_with_defaults() {
        let dir = fresh_dir("observe_discovery");
        let body = br#"{"action":"discovered","object":"service:_airplay._tcp:Living Room"}"#;
        assert_eq!(body_status(&local_observe(&dir, body)), StatusCode::OK);
        let recorded = familiar_kernel::observation::load(&dir).unwrap();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].actor, "host"); // default when the caller doesn't set one
        assert_eq!(recorded[0].action, "discovered");
        assert_eq!(recorded[0].source, "local");
    }

    #[test]
    fn local_observe_recognized_face_reaches_the_identity_registry() {
        // The other half of the gap this closes — macOS has no NodeKey to sign a /mesh/observe
        // push with, so this loopback seam is its only path to the identity registry.
        let dir = fresh_dir("observe_identity");
        let body = br#"{"actor":"host","action":"recognized","object":"face:Betty"}"#;
        assert_eq!(body_status(&local_observe(&dir, body)), StatusCode::OK);
        assert_eq!(
            familiar_kernel::identity::current(&dir).as_deref(),
            Some("betty")
        );
    }

    #[test]
    fn push_tool_rejects_malformed_and_mismatched_input() {
        let dir = fresh_dir("push_tool_bad");
        assert_eq!(
            body_status(&push_tool(&dir, b"not json")),
            StatusCode::BAD_REQUEST
        );

        let body = br#"{"manifest":{"tool_id":"t-1","name":"n","purpose":"p","keywords":[],
            "script_sha256":"deadbeef","uses":0,"last_exit_ok":true},
            "body_hex":"68656c6c6f","from_node_id":"peer1"}"#;
        // sha256("hello") != "deadbeef" — the integrity check must reject it.
        assert_eq!(body_status(&push_tool(&dir, body)), StatusCode::BAD_REQUEST);
        assert!(familiar_kernel::tool::load(&dir).unwrap().is_empty());
    }

    #[test]
    fn push_tool_installs_a_valid_push_with_correct_provenance() {
        let dir = fresh_dir("push_tool_ok");
        let script = b"#!/bin/sh\necho hi\n";
        let sha = sha256_hex(script);
        let manifest = serde_json::json!({
            "tool_id": "t-1", "name": "greet", "purpose": "say hi",
            "keywords": ["greet", "hi"], "script_sha256": sha, "uses": 3, "last_exit_ok": true,
        });
        let payload = serde_json::json!({
            "manifest": manifest,
            "body_hex": crate::hex_encode(script),
            "from_node_id": "peer-node-id",
        });
        let body = serde_json::to_vec(&payload).unwrap();
        assert_eq!(body_status(&push_tool(&dir, &body)), StatusCode::OK);

        let tools = familiar_kernel::tool::load(&dir).unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "greet");
        assert_eq!(tools[0].origin, "peer-node-id");
        assert_eq!(std::fs::read(&tools[0].script_path).unwrap(), script);

        // Pushing the exact same content again is a harmless no-op, not a duplicate.
        assert_eq!(body_status(&push_tool(&dir, &body)), StatusCode::OK);
        assert_eq!(familiar_kernel::tool::load(&dir).unwrap().len(), 1);
    }

    #[test]
    fn push_tool_refuses_a_network_reaching_tool() {
        let dir = fresh_dir("push_tool_net");
        // A LAN scan authored against the pusher's network — hash-valid, share_tools on, but it
        // reaches the network, so it must not be federated onto us.
        let script = b"#!/bin/sh\nnmap -sn 192.168.1.0/24\n";
        let sha = sha256_hex(script);
        let manifest = serde_json::json!({
            "tool_id": "t-scan", "name": "lan_scan", "purpose": "sweep the lan",
            "keywords": ["scan"], "script_sha256": sha, "uses": 1, "last_exit_ok": true,
        });
        let payload = serde_json::json!({
            "manifest": manifest,
            "body_hex": crate::hex_encode(script),
            "from_node_id": "peer-node-id",
        });
        let body = serde_json::to_vec(&payload).unwrap();
        assert_eq!(body_status(&push_tool(&dir, &body)), StatusCode::FORBIDDEN);
        assert!(familiar_kernel::tool::load(&dir).unwrap().is_empty());
    }

    #[test]
    fn push_tool_respects_the_share_tools_gate() {
        let dir = fresh_dir("push_tool_gated");
        let cfg = config::MeshConfig {
            share_tools: false,
            ..Default::default()
        };
        let _ = std::fs::create_dir_all(dir.join("mesh"));
        std::fs::write(
            dir.join(config::CONFIG_FILE),
            serde_json::to_vec(&cfg).unwrap(),
        )
        .unwrap();
        let body = br#"{"manifest":{"tool_id":"t","name":"n","purpose":"p","keywords":[],
            "script_sha256":"x","uses":0,"last_exit_ok":true},"body_hex":"","from_node_id":"p"}"#;
        assert_eq!(body_status(&push_tool(&dir, body)), StatusCode::FORBIDDEN);
    }

    #[test]
    fn abandon_peer_marks_status_without_deleting_history() {
        let dir = fresh_dir("abandon");
        let peers = vec![PeerRecord {
            node_id: "node1".into(),
            label: "Old Box".into(),
            addr: "10.0.0.5:47100".into(),
            group_id: "g".into(),
            last_seen: 100,
            tools_offered: 3,
            patterns_offered: 5,
            os: "linux".into(),
            arch: "x86_64".into(),
            first_seen: 1,
            familiar_version: "0.1.0".into(),
            os_version: String::new(),
            session_start: 0,
            total_online_secs: 500,
            interactive: false,
            human: String::new(),
            lat: 0.0,
            lon: 0.0,
            geo_device: false,
            status: String::new(),
            connectivity: String::new(),
        }];
        std::fs::create_dir_all(dir.join(PEERS_FILE).parent().unwrap()).unwrap();
        std::fs::write(
            dir.join(PEERS_FILE),
            serde_json::to_vec_pretty(&peers).unwrap(),
        )
        .unwrap();

        assert!(abandon_peer(&dir, "node1").unwrap());
        let after = load_peers(&dir);
        assert_eq!(after.len(), 1, "the record is never deleted");
        assert_eq!(after[0].status, "abandoned");
        assert_eq!(after[0].total_online_secs, 500, "history is preserved");

        // An unknown node id is a no-op, not an error.
        assert!(!abandon_peer(&dir, "nobody").unwrap());
    }

    #[test]
    fn the_status_directory_teaches_us_members_we_have_never_met() {
        // The bug this covers: a member admitted at the lighthouse (the only minting door,
        // ADR-0018) heartbeats there, we pull the directory — and used to drop every row we did
        // not already hold. A remote tester was live, enrolled, and invisible to the whole mesh.
        let dir = fresh_dir("adopt_status");
        let node = crate::node::NodeKey::load_or_mint(&dir, "familiar").unwrap();
        let cred =
            crate::group::create_group(&dir, &node, "TheRiver", 1_785_000_000, 86_400).unwrap();
        let ours = crate::rendezvous::group_ref(&cred.group_id);

        let mk =
            |id: &str, gref: &str, label: &str, human: &str, at: i64| crate::status::MemberStatus {
                node_id: id.into(),
                group_ref: gref.into(),
                actor: String::new(),
                label: label.into(),
                present_human: human.into(),
                present_via: String::new(),
                present_since: 0,
                present_confidence: 0.0,
                connectivity: "lighthouse".into(),
                tailnet_addr: String::new(),
                tailnet_up: false,
                updated_at: at,
            };

        let statuses = vec![
            mk("remote01", &ours, "Ivan's iPhone", "ivan", 1_785_100_000),
            // A different mesh entirely — must never be adopted.
            mk(
                "stranger1",
                "someoneelsesgroup",
                "Not ours",
                "",
                1_785_100_000,
            ),
            // A pre-group_ref node — unattributable, so not trusted into the roster.
            mk("legacy01", "", "Old node", "", 1_785_100_000),
            // Ourselves — never adopt our own row as a peer.
            mk(&node.node_id(), &ours, "Wildhorse", "ian", 1_785_100_000),
        ];

        apply_status_freshness(&dir, &statuses, 1_785_100_000);
        let peers = load_peers(&dir);
        let ids: Vec<&str> = peers.iter().map(|p| p.node_id.as_str()).collect();

        // A node seen ONCE is a key passing through, not a device arriving (ADR-0025). Adopting
        // on a single heartbeat is what left three ghost "iPhone" members in the roster.
        assert!(
            !ids.contains(&"remote01"),
            "a node seen once must not be adopted, got {ids:?}"
        );
        assert!(
            !ids.contains(&"stranger1"),
            "another mesh's member must never be adopted"
        );
        assert!(
            !ids.contains(&"legacy01"),
            "an unattributable row must not be adopted"
        );
        assert!(
            !ids.contains(&node.node_id().as_str()),
            "we must not adopt ourselves"
        );

        // It ripens once it keeps showing up.
        let later = 1_785_100_000 + ADOPT_AFTER_SECS + 1;
        apply_status_freshness(&dir, &statuses, later);
        let peers = load_peers(&dir);
        let ids: Vec<String> = peers.iter().map(|p| p.node_id.clone()).collect();
        assert!(
            ids.iter().any(|i| i == "remote01"),
            "a member that persists must be adopted, got {ids:?}"
        );
        assert!(
            !ids.iter().any(|i| i == "stranger1"),
            "persistence must not launder another mesh's member in"
        );

        let ivan = peers
            .iter()
            .find(|p| p.node_id == "remote01")
            .expect("remote01 should be adopted once it has persisted");
        assert_eq!(ivan.label, "Ivan's iPhone");
        assert_eq!(ivan.last_seen, 1_785_100_000);
        assert_eq!(
            ivan.first_seen, later,
            "our own first sighting is when WE adopted it, not the lighthouse's"
        );
        assert_eq!(ivan.human, "ivan");
        assert_eq!(ivan.connectivity, "lighthouse");

        // Idempotent: a second pull must not duplicate the row.
        apply_status_freshness(&dir, &statuses, later + 100);
        assert_eq!(
            load_peers(&dir)
                .iter()
                .filter(|p| p.node_id == "remote01")
                .count(),
            1
        );
    }

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
    fn self_ip4_takes_the_first_v4_and_rejects_non_addresses() {
        let json = r#"{"Self":{"TailscaleIPs":["fd7a:115c::1","100.64.0.10","100.64.0.11"]}}"#;
        assert_eq!(parse_self_ip4(json).as_deref(), Some("100.64.0.10"));
        // The CLI-era poisoning guard carries over: prose is never an address.
        assert_eq!(
            parse_self_ip4(r#"{"Self":{"TailscaleIPs":["The Tailscale GUI failed to start"]}}"#),
            None
        );
        assert_eq!(parse_self_ip4("not json"), None);
        assert_eq!(parse_self_ip4("{}"), None);
    }

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b":hunter2token"), "Omh1bnRlcjJ0b2tlbg==");
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
