//! Transport — the async half. **Does IO, never constitutional merge.**
//!
//! A background tokio runtime (spawned once at daemon start via [`spawn`]) does three
//! things while `allow_mesh` is open:
//! - **serves** `/mesh/hello`, `/mesh/brief`, `/mesh/tool/{id}` on the tailnet,
//! - **gossips**: enumerates tailnet peers (`tailscale status --json`, read-only) plus any
//!   `static_peers`, POSTs our brief to each, and takes theirs in return,
//! - **verifies at ingress**: every inbound brief's membership cert + node signature are
//!   checked against our group key *before* it touches disk; junk is dropped.
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerRecord {
    pub node_id: String,
    pub label: String,
    pub addr: String,
    pub group_id: String,
    pub last_seen: i64,
    pub tools_offered: usize,
    pub patterns_offered: usize,
}

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

/// The supervisor loop: (re)bind the server while active, gossip each interval, tear down
/// and idle when the boundary closes or the group isn't enrolled.
async fn supervisor(dir: PathBuf, stop: Arc<AtomicBool>) {
    let mut server: Option<tokio::task::JoinHandle<()>> = None;
    let mut bound_port: u16 = 0;

    loop {
        if stop.load(Ordering::SeqCst) {
            if let Some(s) = server.take() {
                s.abort();
            }
            return;
        }

        let cfg = config::load(&dir).unwrap_or_default();
        let interval = Duration::from_secs(cfg.gossip_interval_secs.max(1));

        // Gate 1: the human-owned boundary.
        if !mesh_allowed(&dir) {
            if let Some(s) = server.take() {
                s.abort();
            }
            bound_port = 0;
            let _ = write_status(&dir, "mesh idle — allow_mesh is off");
            sleep_or_stop(&stop, interval).await;
            continue;
        }

        // Gate 2: an enrolled group (a human handed us a credential).
        let cred = match group::load(&dir).ok().flatten() {
            Some(c) => c,
            None => {
                if let Some(s) = server.take() {
                    s.abort();
                }
                bound_port = 0;
                let _ = write_status(&dir, "mesh waiting — no group enrolled yet");
                sleep_or_stop(&stop, interval).await;
                continue;
            }
        };

        // Ensure the server is bound (rebind if the port changed).
        if server.is_none() || bound_port != cfg.gossip_port {
            if let Some(s) = server.take() {
                s.abort();
            }
            match TcpListener::bind(("0.0.0.0", cfg.gossip_port)).await {
                Ok(listener) => {
                    bound_port = cfg.gossip_port;
                    let ctx = Arc::new(ServerCtx {
                        dir: dir.clone(),
                        seen: std::sync::Mutex::new(crate::observe::NonceRing::default()),
                    });
                    server = Some(tokio::spawn(serve(listener, ctx)));
                }
                Err(e) => {
                    let _ = write_status(&dir, &format!("mesh bind :{} failed: {e}", cfg.gossip_port));
                    sleep_or_stop(&stop, interval).await;
                    continue;
                }
            }
        }

        // One gossip round.
        let peers = gossip_round(&dir, &cfg, &cred).await;
        let _ = write_status(
            &dir,
            &format!(
                "✓ mesh open (group {}) — {} peer(s) connected",
                short(&cred.group_id),
                peers
            ),
        );

        sleep_or_stop(&stop, interval).await;
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
    seen: std::sync::Mutex<crate::observe::NonceRing>,
}

async fn serve(listener: TcpListener, ctx: Arc<ServerCtx>) {
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(pair) => pair,
            Err(_) => continue,
        };
        let io = TokioIo::new(stream);
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let svc = service_fn(move |req| handle(req, ctx.clone()));
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
            recv_brief(&dir, &bytes)
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

/// `GET /mesh/hello` → who we are + which group (cheap same-group precheck).
fn hello(dir: &Path) -> Response<Full<Bytes>> {
    let Some(cred) = group::load(dir).ok().flatten() else {
        return text(StatusCode::SERVICE_UNAVAILABLE, "no group");
    };
    let node = cred.membership.node_id.clone();
    let body = serde_json::json!({
        "node_id": node,
        "group_id": cred.group_id,
        "label": cred.label,
    });
    text(StatusCode::OK, body.to_string())
}

/// `POST /mesh/brief` → verify at ingress, stash if trusted, answer with our own brief.
fn recv_brief(dir: &Path, bytes: &[u8]) -> Response<Full<Bytes>> {
    match ingest_brief(dir, bytes) {
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
pub(crate) fn ingest_brief(dir: &Path, bytes: &[u8]) -> Result<()> {
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
    upsert_peer(dir, &brief, "")?;
    Ok(())
}

/// `POST /mesh/observe` → verify a device's signed observation batch and, if trusted + fresh,
/// append it to the store. `sig` is the `X-Familiar-Sig` header (ed25519 over the raw body).
/// 200 + count on success; 409 on a replayed nonce; 403 if untrusted; 400 if malformed.
fn recv_observe(
    dir: &Path,
    bytes: &[u8],
    sig: &str,
    ring: &std::sync::Mutex<crate::observe::NonceRing>,
) -> Response<Full<Bytes>> {
    match crate::observe::ingest_observations(dir, bytes, sig, now_secs(), ring) {
        Ok(n) => text(StatusCode::OK, format!("recorded {n}")),
        Err(crate::Error::Untrusted(m)) if m.contains("replay") => text(StatusCode::CONFLICT, m),
        Err(crate::Error::Untrusted(m)) => text(StatusCode::FORBIDDEN, m),
        Err(_) => text(StatusCode::BAD_REQUEST, "bad batch"),
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

/// One gossip round: enumerate peers, exchange briefs with each, return the number reached.
async fn gossip_round(dir: &Path, cfg: &MeshConfig, cred: &GroupCredential) -> usize {
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
    addrs.sort();
    addrs.dedup();

    let _ = cred; // group identity is applied via ingest_brief on each reply
    let mut reached = 0;
    for addr in addrs {
        if exchange_with(dir, &addr, &our_brief).await.is_ok() {
            reached += 1;
        }
    }
    reached
}

/// POST our brief to one peer, verify + stash its reply, and pre-fetch any tool bodies we
/// lack. Errors (connection refused, non-peer host, forged reply) are swallowed by design.
async fn exchange_with(dir: &Path, addr: &str, our_brief: &[u8]) -> Result<()> {
    let reply = http_send(addr, Method::POST, "/mesh/brief", Some(our_brief.to_vec())).await?;
    if reply.status != StatusCode::OK || reply.body.is_empty() {
        return Ok(()); // peer accepted ours but had nothing to return
    }
    // Verify the peer's brief before it touches disk (defense at ingress).
    ingest_brief(dir, &reply.body)?;
    // Pre-fetch tool bodies we don't already have, content-addressed for the in-tick merge.
    if let Ok(brief) = serde_json::from_slice::<MeshBrief>(&reply.body) {
        upsert_peer(dir, &brief, addr)?;
        let known = known_tool_shas(dir);
        for t in &brief.body.capability.tools {
            let sha = &t.script_sha256;
            if known.contains(sha) || inbox_tool_path(dir, sha).exists() {
                continue;
            }
            if let Ok(resp) = http_send(addr, Method::GET, &format!("/mesh/tool/{}", t.tool_id), None).await
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
async fn http_send(addr: &str, method: Method, path: &str, body: Option<Vec<u8>>) -> Result<HttpResp> {
    let connect = tokio::time::timeout(Duration::from_secs(4), TcpStream::connect(addr));
    let stream = connect
        .await
        .map_err(|_| crate::Error::Io(std::io::Error::new(std::io::ErrorKind::TimedOut, "connect timeout")))?
        .map_err(crate::Error::Io)?;
    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
        .await
        .map_err(|e| crate::Error::Malformed(format!("handshake: {e}")))?;
    tokio::spawn(async move {
        let _ = conn.await;
    });
    let req = Request::builder()
        .method(method)
        .uri(path)
        .header("host", addr)
        .body(Full::new(Bytes::from(body.unwrap_or_default())))
        .map_err(|e| crate::Error::Malformed(format!("request: {e}")))?;
    let resp = tokio::time::timeout(Duration::from_secs(6), sender.send_request(req))
        .await
        .map_err(|_| crate::Error::Io(std::io::Error::new(std::io::ErrorKind::TimedOut, "request timeout")))?
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

/// Enumerate tailnet peers (read-only shell-out). Empty if tailscale is absent/unreachable —
/// mesh then relies on `static_peers` only.
pub fn enumerate_peers() -> Vec<TailscalePeer> {
    let out = std::process::Command::new("tailscale")
        .args(["status", "--json"])
        .output();
    match out {
        Ok(o) if o.status.success() => parse_tailscale_status(&String::from_utf8_lossy(&o.stdout)),
        _ => Vec::new(),
    }
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
            online: peer.get("Online").and_then(|o| o.as_bool()).unwrap_or(false),
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
    let rec = PeerRecord {
        node_id: brief.body.node.node_id.clone(),
        label: brief.body.node.label.clone(),
        addr: addr.to_string(),
        group_id: brief.body.membership.group_id.clone(),
        last_seen: now_secs(),
        tools_offered: brief.body.capability.tools.len(),
        patterns_offered: brief.body.knowledge.patterns.len(),
    };
    match peers.iter_mut().find(|p| p.node_id == rec.node_id) {
        Some(existing) => {
            let addr_keep = if rec.addr.is_empty() {
                existing.addr.clone()
            } else {
                rec.addr.clone()
            };
            *existing = PeerRecord {
                addr: addr_keep,
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

fn live_peer_count(dir: &Path) -> usize {
    std::fs::read_to_string(dir.join(PEERS_FILE))
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<PeerRecord>>(&s).ok())
        .map(|p| p.len())
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
    fn is_ipv4_discriminates() {
        assert!(is_ipv4("100.64.0.10"));
        assert!(!is_ipv4("fd7a:1::1"));
        assert!(!is_ipv4("300.1.1.1"));
        assert!(!is_ipv4("1.2.3"));
    }
}
