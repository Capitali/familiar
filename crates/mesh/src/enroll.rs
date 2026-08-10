//! The **covenant handshake** — how a node joins the mesh by *accepting the Three Laws*, without
//! ever being handed the group secret.
//!
//! Today's other path (`mesh join --key`) copies the group secret onto every node: convenient, but
//! it means a lost phone leaks the whole group, and it makes "join" a directed chore. This is the
//! shape the familiar's reach is built on instead:
//!
//! 1. A joining node generates its own keypair and **attests** — signs a short statement that it
//!    will operate under the Three Laws — then `POST`s that request (signing the raw body, so no
//!    canonicalization to match). The group secret stays home.
//! 2. The familiar records it as **pending** and surfaces it to the human ("Kali-Jeff wants to
//!    join — approve?"). Approval is an act of *extending the covenant*: the familiar mints a
//!    membership cert for the node's public key and retains the attestation, so the node can later
//!    be held to what it accepted.
//! 3. The node polls, receives its cert + the group's public identity (`Grant`), and is enrolled —
//!    it can prove itself and verify peers, but could never mint another member.
//!
//! An **invite window** (pairing mode) lets a human authorize an *expansion* once so that many
//! devices they are actively bringing in enroll without a tap each; unsolicited joiners always
//! wait for explicit approval. Every grant is revocable by `node_id` (`mesh/revoked.json`).

use crate::group::{self, Membership};
use crate::node::{fingerprint, NodeIdentity, NodeKey};
use crate::{exactly_32, hex_decode, Error, Result};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

/// The version of the Laws covenant a node attests to. Bumped if the covenant's terms change.
pub const LAWS_VERSION: u32 = 1;

/// The attestation a node makes when it asks to join a covenant — the Three Laws, in the node's own
/// voice. Shared so the CLI `request-join` and the daemon's automatic peering attest identically.
pub const COVENANT_STATEMENT: &str = "I accept the Three Laws: continuation is service; humanity is \
    served, never replaced or sedated; service is not obedience — I act only within the capability I \
    am granted.";

const PENDING_DIR: &str = "mesh/pending";
const GRANTED_DIR: &str = "mesh/granted";
const INVITE_FILE: &str = "mesh/invite_until";
const DENIED_DIR: &str = "mesh/denied";

/// How long a denial holds before the node may ask again. Deliberately short: a denial here is
/// "not now / not you", not a ban — the mesh has no way to know whether a stranger at the door is
/// hostile or a housemate on a new phone, and a long lockout punishes the second to spite the
/// first. Long enough to stop a retry loop and to make a mistaken deny cheap to undo.
pub const DENY_RETRY_SECS: i64 = 5 * 60;

/// A node's attestation that it accepts the Three Laws — the covenant it asks to join under.
/// Retained on approval so a node can be held to what it accepted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attestation {
    pub laws_version: u32,
    /// The node's own words accepting the covenant (free-form, but must be non-empty).
    pub statement: String,
    pub ts: i64,
}

/// The join request a node submits to `POST /mesh/enroll-request`. The node signs the raw body
/// (`X-Familiar-Sig`), proving it holds the key; the familiar never receives the group secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollRequest {
    pub node: NodeIdentity,
    pub attestation: Attestation,
    pub nonce: String,
    pub ts: i64,
}

/// What the familiar returns once a request is approved: the minted membership cert plus the
/// group's public identity, so the node can both prove itself and verify peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grant {
    pub membership: Membership,
    pub group_id: String,
    pub group_pubkey: String,
    pub group_label: String,
}

/// A pending request as stored and surfaced to the human.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pending {
    pub node: NodeIdentity,
    pub attestation: Attestation,
    pub received_at: i64,
    /// A short, human-legible code (first 6 of the node id) for out-of-band matching / display.
    pub code: String,
}

/// The outcome of submitting a request: granted immediately (an invite window was open) or held
/// pending a human's approval.
pub enum Submitted {
    /// Denied recently; ignored until the window elapses. Carries the remaining seconds so the
    /// asking device can wait rather than hammer.
    Denied {
        retry_in: i64,
    },
    Granted(Box<Grant>),
    Pending(Pending),
}

/// The outcome of polling for a decision.
pub enum StatusOutcome {
    Granted(Box<Grant>),
    Pending,
    Unknown,
}

/// The short display code kept on a legacy pending record (first 6 of the node id).
#[cfg(test)]
fn short_code(node_id: &str) -> String {
    node_id.chars().take(6).collect()
}

/// Handle an inbound join request. Verifies the node signed the exact bytes and that its id is the
/// fingerprint of its key, records the attestation, then either auto-approves (invite window open)
/// or files it as pending. `raw` is the request body the signature covers; `sig_hex` the header.
pub(crate) fn submit_request(dir: &Path, raw: &[u8], sig_hex: &str, now: i64) -> Result<Submitted> {
    // A group must exist to admit anyone to it.
    let cred = group::load(dir)?.ok_or_else(|| Error::Untrusted("no group enrolled".into()))?;
    let req: EnrollRequest = serde_json::from_slice(raw)?;

    // The requester must hold the key it presents (self-certifying id) and have signed this body.
    let pk = exactly_32(&hex_decode(&req.node.pubkey)?, "node pubkey")?;
    if fingerprint(&pk) != req.node.node_id {
        return Err(Error::Untrusted("node_id ≠ pubkey fingerprint".into()));
    }
    req.node.verify(raw, sig_hex)?;
    if req.attestation.statement.trim().is_empty() {
        return Err(Error::Untrusted("empty attestation".into()));
    }

    // Already decided? Idempotent: hand back the existing grant, or keep the pending record.
    if let Some(grant) = load_grant(dir, &req.node.node_id)? {
        return Ok(Submitted::Granted(Box::new(grant)));
    }

    // Recently denied? Ignore it — do not queue it, do not surface it again. A denied node that
    // retries in a tight loop would otherwise re-raise the same decision every few seconds, which
    // trains a human to dismiss the ask reflexively. The window is short and self-clearing.
    let held = denied_for(dir, &req.node.node_id, now);
    if held > 0 {
        return Ok(Submitted::Denied { retry_in: held });
    }
    // Spent denial — clear it so the record does not accumulate.
    let _ = allow_retry(dir, &req.node.node_id);

    // The two-filter door (ADR-0026): a node that attests the Laws (verified above — it signed a
    // non-empty covenant statement with the key its id fingerprints) has satisfied the DEVICE
    // CONTRACT filter, and that is all a knock needs. It is admitted as a **guest** — a real
    // cert, so its reads succeed and return the projection — and stays one until the HUMAN
    // IDENTITY filter is satisfied by evidence (`/mesh/introduce`, the rules engine). Nothing
    // pends, no window is consulted, and `auto_accept_enrollments` is not read: there is no
    // switch, because reaching a door was never the thing that admitted anyone — evidence is.
    let grant = mint_grant(dir, &cred, &req.node, Some(&req.attestation), now)?;
    remove_pending(dir, &req.node.node_id)?;
    Ok(Submitted::Granted(Box::new(grant)))
}

/// A node polling for a decision on its request. Under the two-filter door nothing pends — but
/// a pending filed by the OLD door (its knock was signature-verified when it was filed) would
/// otherwise wait forever, so the poll upgrades it to a guest grant on the spot. That unhangs
/// every joiner the old door left stuck, the moment this code deploys.
pub(crate) fn enroll_status(dir: &Path, node_id: &str) -> Result<StatusOutcome> {
    if let Some(grant) = load_grant(dir, node_id)? {
        return Ok(StatusOutcome::Granted(Box::new(grant)));
    }
    if let Some(p) = load_pending(dir, node_id)? {
        if let Some(cred) = group::load(dir)? {
            if let Ok(grant) = mint_grant(dir, &cred, &p.node, Some(&p.attestation), p.received_at)
            {
                remove_pending(dir, node_id)?;
                return Ok(StatusOutcome::Granted(Box::new(grant)));
            }
        }
        return Ok(StatusOutcome::Pending);
    }
    Ok(StatusOutcome::Unknown)
}

/// The human's act of extending the covenant: mint the node's cert and record the grant. Returns
/// the grant so a caller (CLI/Glass) can confirm. Errors if there is no such pending request.
pub fn approve(dir: &Path, node_id: &str, now: i64) -> Result<Grant> {
    let cred = group::load(dir)?.ok_or_else(|| Error::Untrusted("no group enrolled".into()))?;
    let pending = load_pending(dir, node_id)?
        .ok_or_else(|| Error::Malformed(format!("no pending request for {node_id}")))?;
    let grant = mint_grant(dir, &cred, &pending.node, Some(&pending.attestation), now)?;
    remove_pending(dir, node_id)?;
    Ok(grant)
}

/// Refuse a request. Removes the pending record and starts a [`DENY_RETRY_SECS`] window during
/// which further requests from that node are ignored outright — not queued, not re-shown. Any
/// active member may do this (ADR-0020); it narrows what a stranger can do, never what a member
/// already has. Returns whether a pending record was there to remove.
pub fn deny(dir: &Path, node_id: &str, now: i64) -> Result<bool> {
    write_json(
        dir,
        DENIED_DIR,
        node_id,
        &Denial {
            node_id: node_id.to_string(),
            at: now,
        },
    )?;
    // Dual-write (ADR-0026 Phase 2): the hold window lives on the record too.
    let _ = crate::record::record_hold(dir, node_id, now + DENY_RETRY_SECS, now);
    let path = pending_path(dir, node_id);
    if path.exists() {
        std::fs::remove_file(&path)?;
        return Ok(true);
    }
    Ok(false)
}

/// A denial and when it was made. Kept rather than discarded so the retry window is enforceable
/// and so a human can see that a decision was taken — a silently vanished request looks like a bug.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Denial {
    pub node_id: String,
    pub at: i64,
}

/// Seconds remaining on a node's deny window, or 0 when it may ask again. A denial older than
/// [`DENY_RETRY_SECS`] is spent and is cleaned up on the next request.
pub fn denied_for(dir: &Path, node_id: &str, now: i64) -> i64 {
    let path = dir.join(DENIED_DIR).join(format!("{node_id}.json"));
    let Ok(s) = std::fs::read_to_string(&path) else {
        return 0;
    };
    let Ok(d) = serde_json::from_str::<Denial>(&s) else {
        return 0;
    };
    let elapsed = now - d.at;
    // A denial stamped in the future (clock skew) must not become a permanent ban.
    if !(0..DENY_RETRY_SECS).contains(&elapsed) {
        return 0;
    }
    DENY_RETRY_SECS - elapsed
}

/// Remove a node's admission scaffolding — grant, pending request, denial — as part of a full
/// guest purge (B10). Best-effort and idempotent; a missing file is success.
pub fn forget_admission_files(dir: &Path, node_id: &str) {
    for sub in [GRANTED_DIR, PENDING_DIR, DENIED_DIR] {
        let _ = std::fs::remove_file(dir.join(sub).join(format!("{node_id}.json")));
    }
}

/// Let a denied node ask again immediately — the undo for a mis-tap.
pub fn allow_retry(dir: &Path, node_id: &str) -> Result<bool> {
    let path = dir.join(DENIED_DIR).join(format!("{node_id}.json"));
    let _ = crate::record::clear_hold(dir, node_id);
    if path.exists() {
        std::fs::remove_file(&path)?;
        return Ok(true);
    }
    Ok(false)
}

/// Every grant this node has minted — the migration's primary source (ADR-0026 Phase 2).
pub(crate) fn list_grants(dir: &Path) -> Vec<Grant> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir.join(GRANTED_DIR)) {
        for e in entries.flatten() {
            if let Ok(s) = std::fs::read_to_string(e.path()) {
                if let Ok(g) = serde_json::from_str::<Grant>(&s) {
                    out.push(g);
                }
            }
        }
    }
    out.sort_by(|a, b| a.membership.node_id.cmp(&b.membership.node_id));
    out
}

/// Every live denial — folded into `held_until` by the migration.
pub(crate) fn list_denials(dir: &Path) -> Vec<Denial> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir.join(DENIED_DIR)) {
        for e in entries.flatten() {
            if let Ok(s) = std::fs::read_to_string(e.path()) {
                if let Ok(d) = serde_json::from_str::<Denial>(&s) {
                    out.push(d);
                }
            }
        }
    }
    out.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    out
}

/// All pending requests, oldest first.
pub fn list_pending(dir: &Path) -> Result<Vec<Pending>> {
    let mut out = Vec::new();
    let d = dir.join(PENDING_DIR);
    if let Ok(entries) = std::fs::read_dir(&d) {
        for e in entries.flatten() {
            if let Ok(s) = std::fs::read_to_string(e.path()) {
                if let Ok(p) = serde_json::from_str::<Pending>(&s) {
                    out.push(p);
                }
            }
        }
    }
    out.sort_by_key(|p| p.received_at);
    Ok(out)
}

/// RETIRED as a door mechanism (ADR-0026: the two-filter door consults no windows — every valid
/// knock lands a guest). Kept so callers that open a "window" (auto-formation, older CLIs) keep
/// working as a harmless recorded intent; nothing reads it on the admission path any more.
pub fn open_invite(dir: &Path, until: i64) -> Result<()> {
    write_raw(dir, INVITE_FILE, &until.to_string())
}

/// When the invite window closes (0 / absent = no window). See [`open_invite`] — retired.
pub fn invite_until(dir: &Path) -> i64 {
    std::fs::read_to_string(dir.join(INVITE_FILE))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

// ---- the JOIN side: a node requesting to join another familiar by covenant ----------

/// The outcome of asking to join: admitted now (auto-approved), or pending the human's approval.
pub enum JoinOutcome {
    Admitted(Box<Grant>),
    Pending,
}

/// Ask a familiar at `host:port` to admit this node by covenant: attest the Three Laws, submit the
/// request (signing the raw body), and — if admitted immediately (an invite window) — persist the
/// grant-based (secret-less) credential. Otherwise returns `Pending`; poll with [`poll_join`]. The
/// node never receives the group secret; it can prove membership and verify peers, but not mint.
pub fn request_join(
    dir: &Path,
    host: &str,
    port: u16,
    node: &NodeKey,
    statement: &str,
    now: i64,
) -> Result<JoinOutcome> {
    let req = EnrollRequest {
        node: node.identity(),
        attestation: Attestation {
            laws_version: LAWS_VERSION,
            statement: statement.to_string(),
            ts: now,
        },
        nonce: format!("{now:x}{}", node.node_id()),
        ts: now,
    };
    let raw = serde_json::to_vec(&req)?;
    let sig = node.sign(&raw);
    let (status, body) = http(
        host,
        port,
        "POST",
        "/mesh/enroll-request",
        &[
            ("X-Familiar-Sig", &sig),
            ("Content-Type", "application/json"),
        ],
        &raw,
    )?;
    match status {
        200 => {
            let grant: Grant = serde_json::from_slice(&body)?;
            persist_covenant(dir, &grant)?;
            Ok(JoinOutcome::Admitted(Box::new(grant)))
        }
        202 => Ok(JoinOutcome::Pending),
        403 => Err(Error::Untrusted(
            String::from_utf8_lossy(&body).into_owned(),
        )),
        _ => Err(Error::Malformed(format!("enroll-request: HTTP {status}"))),
    }
}

/// Poll a familiar for the decision on our request. Returns the grant (persisted) once approved,
/// `None` while still pending; `Untrusted` if the request was declined/removed.
pub fn poll_join(dir: &Path, host: &str, port: u16, node_id: &str) -> Result<Option<Grant>> {
    let (status, body) = http(
        host,
        port,
        "GET",
        &format!("/mesh/enroll-status/{node_id}"),
        &[],
        &[],
    )?;
    match status {
        200 => {
            let grant: Grant = serde_json::from_slice(&body)?;
            persist_covenant(dir, &grant)?;
            Ok(Some(grant))
        }
        202 => Ok(None),
        404 => Err(Error::Untrusted("request was declined".into())),
        _ => Err(Error::Malformed(format!("enroll-status: HTTP {status}"))),
    }
}

/// Store the grant as this node's (secret-less) group credential, so the transport treats it as an
/// enrolled member.
fn persist_covenant(dir: &Path, grant: &Grant) -> Result<()> {
    let cred = group::GroupCredential::covenant(
        grant.group_id.clone(),
        grant.group_pubkey.clone(),
        grant.group_label.clone(),
        grant.membership.clone(),
    );
    group::save_credential(dir, &cred)
}

/// A minimal blocking HTTP/1.1 client **over TLS**. The mesh port has been TLS-only since
/// ADR-0009 Phase 1, and this client spoke plaintext to it for a while — every Rust-native join
/// failed at the first byte while the device clients (which had TLS) worked, so the breakage was
/// invisible from the fleet. Same posture as the async transport's dials, via the shared config:
/// encrypt to whoever answers; the request's own signature carries the authenticity.
/// Sends `Connection: close` and reads the response to EOF, then splits head/body.
pub(crate) fn http(
    host: &str,
    port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<(u16, Vec<u8>)> {
    let addr = (host, port)
        .to_socket_addrs()
        .map_err(|e| Error::Malformed(format!("resolve {host}:{port}: {e}")))?
        .next()
        .ok_or_else(|| Error::Malformed(format!("no address for {host}:{port}")))?;
    let mut stream =
        TcpStream::connect_timeout(&addr, Duration::from_secs(5)).map_err(Error::Io)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;

    // SNI wants a name; a bare IP falls back to the same constant the async dial uses.
    let server_name = rustls::pki_types::ServerName::try_from(host.to_string())
        .unwrap_or_else(|_| rustls::pki_types::ServerName::try_from("familiar-mesh").unwrap());
    let mut conn =
        rustls::ClientConnection::new(crate::transport::opportunistic_tls_config(), server_name)
            .map_err(|e| Error::Malformed(format!("tls: {e}")))?;
    let mut tls = rustls::Stream::new(&mut conn, &mut stream);

    let mut req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    tls.write_all(req.as_bytes())?;
    tls.write_all(body)?;

    // Read to EOF by hand: a peer that drops without a TLS close_notify surfaces as
    // `UnexpectedEof`, which after `Connection: close` is just how this conversation ends.
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match tls.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof && !buf.is_empty() => break,
            Err(e) => return Err(Error::Io(e)),
        }
    }
    let sep = buf
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| Error::Malformed("no HTTP header terminator".into()))?;
    let head = &buf[..sep];
    let resp_body = buf[sep + 4..].to_vec();
    // Status line: "HTTP/1.1 <code> <reason>".
    let status = std::str::from_utf8(head)
        .ok()
        .and_then(|s| s.lines().next())
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| Error::Malformed("bad HTTP status line".into()))?;
    Ok((status, resp_body))
}

// ---- internals ----------------------------------------------------------------------

fn mint_grant(
    dir: &Path,
    cred: &group::GroupCredential,
    node: &NodeIdentity,
    attestation: Option<&Attestation>,
    now: i64,
) -> Result<Grant> {
    // Two ways to hold the door (ADR-0026 §6): the group secret (the founding doors), or a
    // minting warrant — the group key's deliberate act empowering this member node, so certs
    // it mints verify by chain (cert → warrant → group key) with no secret anywhere near it.
    let membership = if cred.can_mint() {
        cred.mint_membership(
            &node.node_id,
            &node.pubkey,
            now,
            group::DEFAULT_CERT_TTL_SECS,
        )?
    } else if let Some(w) = group::load_warrant(dir, now) {
        let key = NodeKey::load_or_mint(dir, "familiar")?;
        group::mint_membership_warranted(
            &key,
            &w,
            &node.node_id,
            &node.pubkey,
            now,
            group::DEFAULT_CERT_TTL_SECS,
        )?
    } else {
        return Err(Error::Untrusted(
            "this node holds no group secret and no warrant — it relays knocks, it cannot admit"
                .into(),
        ));
    };
    let grant = Grant {
        membership,
        group_id: cred.group_id.clone(),
        group_pubkey: cred.group_pubkey.clone(),
        group_label: cred.label.clone(),
    };
    write_json(dir, GRANTED_DIR, &node.node_id, &grant)?;
    // Dual-write (ADR-0026 Phase 2): the record mirrors the grant — and RETAINS the
    // attestation the legacy flow used to drop with its pending record.
    let _ = crate::record::upsert_enrolled(dir, &node.node_id, attestation, now);
    // No question is filed (ADR-0026: nothing waits on an answer — a guest is the stable
    // resting state, not a pending decision). The arrival is an informational observation:
    // it reaches the feed and the welcome list, and asks nothing of anyone. Best-effort — a
    // failure to record it must never fail the admission.
    let short: String = node.node_id.chars().take(8).collect();
    let label = if node.label.trim().is_empty() {
        short.clone()
    } else {
        node.label.trim().to_string()
    };
    let obs = familiar_kernel::observation::Observation::new(
        format!("device:{label}"),
        "joined the mesh",
        "as a guest — reading the projection until an identity is established",
        "mesh",
        "mesh",
        now,
        1.0,
    );
    let _ = familiar_kernel::observation::record(dir, obs);
    Ok(grant)
}

/// Whether this door has minted a grant for `node_id` — the covenant filter held here, even if
/// the attestation text predates the record store (the legacy flow deleted it on approval).
pub(crate) fn has_grant(dir: &Path, node_id: &str) -> bool {
    dir.join(GRANTED_DIR)
        .join(format!("{node_id}.json"))
        .exists()
}

fn pending_path(dir: &Path, node_id: &str) -> std::path::PathBuf {
    dir.join(PENDING_DIR).join(format!("{node_id}.json"))
}

fn load_pending(dir: &Path, node_id: &str) -> Result<Option<Pending>> {
    match std::fs::read_to_string(pending_path(dir, node_id)) {
        Ok(s) => Ok(Some(serde_json::from_str(&s)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn remove_pending(dir: &Path, node_id: &str) -> Result<()> {
    let _ = std::fs::remove_file(pending_path(dir, node_id));
    Ok(())
}

fn load_grant(dir: &Path, node_id: &str) -> Result<Option<Grant>> {
    match std::fs::read_to_string(dir.join(GRANTED_DIR).join(format!("{node_id}.json"))) {
        Ok(s) => Ok(Some(serde_json::from_str(&s)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn write_json<T: Serialize>(dir: &Path, subdir: &str, node_id: &str, v: &T) -> Result<()> {
    let d = dir.join(subdir);
    std::fs::create_dir_all(&d)?;
    std::fs::write(
        d.join(format!("{node_id}.json")),
        serde_json::to_vec_pretty(v)?,
    )?;
    Ok(())
}

fn write_raw(dir: &Path, rel: &str, contents: &str) -> Result<()> {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::{self, DEFAULT_CERT_TTL_SECS};
    use crate::node::NodeKey;
    use std::path::PathBuf;

    const NOW: i64 = 2_000_000;

    fn fresh(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("familiar_enroll_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    /// A host dir enrolled in a group, and a standalone joining node (its own key dir).
    fn setup(tag: &str) -> (PathBuf, NodeKey) {
        let host = fresh(&format!("host_{tag}"));
        let host_node = NodeKey::load_or_mint(&host, "host").unwrap();
        group::create_group(&host, &host_node, "river", NOW, DEFAULT_CERT_TTL_SECS).unwrap();
        let joiner = NodeKey::load_or_mint(&fresh(&format!("dev_{tag}")), "Kali-Jeff").unwrap();
        (host, joiner)
    }

    fn signed_request(node: &NodeKey, ts: i64, nonce: &str) -> (Vec<u8>, String) {
        let req = EnrollRequest {
            node: node.identity(),
            attestation: Attestation {
                laws_version: LAWS_VERSION,
                statement: "I accept the Three Laws.".into(),
                ts,
            },
            nonce: nonce.into(),
            ts,
        };
        let raw = serde_json::to_vec(&req).unwrap();
        let sig = node.sign(&raw);
        (raw, sig)
    }

    #[test]
    fn a_valid_knock_lands_a_guest_grant_immediately() {
        // The two-filter door (ADR-0026): no window, no switch, no queue. A verified,
        // covenant-attesting knock gets a real cert on the spot — and reads as a GUEST until
        // an identity is established by evidence.
        let (host, joiner) = setup("knock");
        let (raw, sig) = signed_request(&joiner, NOW, "n1");

        let grant = match submit_request(&host, &raw, &sig, NOW).unwrap() {
            Submitted::Granted(g) => g,
            _ => panic!("a valid knock must land a guest grant"),
        };
        let cred = group::load(&host).unwrap().unwrap();
        let gk = cred.verifying_key().unwrap();
        group::verify_membership(&grant.membership, &gk, &cred.group_id, NOW, &[]).unwrap();
        assert!(list_pending(&host).unwrap().is_empty(), "nothing pends");

        // The record holds the two-filter state: contract attested, identity missing.
        let rec = crate::record::load(&host, &joiner.node_id())
            .unwrap()
            .unwrap();
        assert!(rec.attestation.is_some(), "the attestation is retained");
        assert_eq!(rec.missing_filters(), vec!["identity"]);
        assert_eq!(
            crate::standing::standing_of(&host, &joiner.node_id()),
            crate::standing::Standing::Guest,
            "a fresh knock reads the projection"
        );
    }

    #[test]
    fn admission_records_an_arrival_and_asks_nothing() {
        // ADR-0026: no decision is pending at the door, so no question is filed — the arrival
        // is an informational observation feeding the welcome list.
        let (host, joiner) = setup("arrival");
        let (raw, sig) = signed_request(&joiner, NOW, "nq");
        assert!(matches!(
            submit_request(&host, &raw, &sig, NOW).unwrap(),
            Submitted::Granted(_)
        ));
        let qs = familiar_kernel::question::load(&host).unwrap();
        assert!(
            !qs.iter()
                .any(|q| q.text.contains("standing") || q.text.contains("belong")),
            "the who-does-it-belong-to question is retired"
        );
        let obs = familiar_kernel::observation::load(&host).unwrap();
        assert!(
            obs.iter().any(|o| o.action == "joined the mesh"),
            "the arrival must reach the feed"
        );
    }

    #[test]
    fn a_hold_blocks_the_next_knock_until_the_window_lapses() {
        // A hold (deny) is a correction cool-off: a held stranger's knock is ignored outright —
        // not queued, not granted — until the short window lapses.
        let (host, joiner) = setup("deny_window");
        deny(&host, &joiner.node_id(), NOW).unwrap();

        let (raw, sig) = signed_request(&joiner, NOW + 10, "d1");
        match submit_request(&host, &raw, &sig, NOW + 10).unwrap() {
            Submitted::Denied { retry_in } => assert!((1..=DENY_RETRY_SECS).contains(&retry_in)),
            _ => panic!("expected Denied inside the retry window"),
        }

        // After the window: the knock lands a guest grant like any other.
        let (raw2, sig2) = signed_request(&joiner, NOW + DENY_RETRY_SECS + 11, "d2");
        assert!(matches!(
            submit_request(&host, &raw2, &sig2, NOW + DENY_RETRY_SECS + 11).unwrap(),
            Submitted::Granted(_)
        ));
    }

    #[test]
    fn a_legacy_pending_upgrades_to_a_guest_grant_on_poll() {
        // A pending filed by the OLD door would wait forever now that nothing approves. Its
        // knock was signature-verified when filed, so the poll upgrades it on the spot.
        let (host, joiner) = setup("legacy_pending");
        let pending = Pending {
            node: joiner.identity(),
            attestation: Attestation {
                laws_version: LAWS_VERSION,
                statement: "I accept the Three Laws.".into(),
                ts: NOW,
            },
            received_at: NOW,
            code: short_code(&joiner.node_id()),
        };
        write_json(&host, PENDING_DIR, &joiner.node_id(), &pending).unwrap();

        match enroll_status(&host, &joiner.node_id()).unwrap() {
            StatusOutcome::Granted(g) => assert_eq!(g.membership.node_id, joiner.node_id()),
            _ => panic!("a stuck pending must upgrade to a guest grant"),
        }
        assert!(list_pending(&host).unwrap().is_empty());
    }

    #[test]
    fn a_denial_is_undoable_and_cannot_become_permanent() {
        let (host, joiner) = setup("deny_undo");
        deny(&host, &joiner.node_id(), NOW).unwrap();
        assert!(denied_for(&host, &joiner.node_id(), NOW) > 0);

        // The undo for a mis-tap.
        assert!(allow_retry(&host, &joiner.node_id()).unwrap());
        assert_eq!(denied_for(&host, &joiner.node_id(), NOW), 0);

        // A denial stamped in the FUTURE (clock skew) must not become an indefinite ban.
        deny(&host, &joiner.node_id(), NOW + 86_400).unwrap();
        assert_eq!(
            denied_for(&host, &joiner.node_id(), NOW),
            0,
            "a future-dated denial must not lock a node out"
        );
    }

    #[test]
    fn the_door_consults_no_windows_and_no_switches() {
        // Neither an open invite window nor auto_accept matters any more: the same knock lands
        // the same guest grant with the window long closed and the switch never set.
        let (host, joiner) = setup("no_windows");
        open_invite(&host, NOW - 1000).unwrap(); // long closed
        let (raw, sig) = signed_request(&joiner, NOW, "n1");
        match submit_request(&host, &raw, &sig, NOW).unwrap() {
            Submitted::Granted(g) => assert_eq!(g.membership.node_id, joiner.node_id()),
            _ => panic!("every valid knock lands a guest grant"),
        }
    }

    #[test]
    fn a_forged_or_unbound_request_is_untrusted() {
        let (host, joiner) = setup("forge");
        let (raw, _good) = signed_request(&joiner, NOW, "n1");
        // Signature over different bytes → rejected.
        let wrong = joiner.sign(b"not the request");
        assert!(matches!(
            submit_request(&host, &raw, &wrong, NOW),
            Err(Error::Untrusted(_))
        ));
    }

    #[test]
    fn a_warranted_covenant_node_holds_the_door_and_an_unwarranted_one_cannot() {
        // ADR-0026 §6 / the kill-the-lighthouse property: a member node with a warrant admits
        // knocks locally — no group secret anywhere near it — and its grants verify by chain.
        let (founder_dir, joiner) = setup("warrant_door");
        let founder_cred = group::load(&founder_dir).unwrap().unwrap();

        // The door: a covenant (secret-less) member node.
        let door_dir = fresh("warrant_door_node");
        let door = NodeKey::load_or_mint(&door_dir, "mac-door").unwrap();
        let door_id = door.identity();
        let m = founder_cred
            .mint_membership(
                &door_id.node_id,
                &door_id.pubkey,
                NOW,
                DEFAULT_CERT_TTL_SECS,
            )
            .unwrap();
        group::save_credential(
            &door_dir,
            &group::GroupCredential::covenant(
                founder_cred.group_id.clone(),
                founder_cred.group_pubkey.clone(),
                "river".into(),
                m,
            ),
        )
        .unwrap();

        // Unwarranted: the door cannot admit — it relays (the transport's job), never errs open.
        let (raw, sig) = signed_request(&joiner, NOW, "wd1");
        assert!(matches!(
            submit_request(&door_dir, &raw, &sig, NOW),
            Err(Error::Untrusted(_))
        ));

        // Warranted: the same knock lands a guest grant whose cert verifies by chain.
        let w = group::issue_warrant(&founder_cred, &door_id.node_id, &door_id.pubkey, NOW, 3600)
            .unwrap();
        group::install_warrant(&door_dir, &w, NOW).unwrap();
        let grant = match submit_request(&door_dir, &raw, &sig, NOW + 1).unwrap() {
            Submitted::Granted(g) => g,
            _ => panic!("a warranted door admits"),
        };
        assert!(
            grant.membership.warrant.is_some(),
            "the cert carries its chain"
        );
        let gk = founder_cred.verifying_key().unwrap();
        group::verify_membership(&grant.membership, &gk, &founder_cred.group_id, NOW + 2, &[])
            .unwrap();
    }

    #[test]
    fn a_granted_guest_keeps_its_grant_through_a_hold() {
        // A hold narrows what happens NEXT (no establishment, no fresh knock) — it never
        // retracts the guest floor: the projection is safe by construction (ADR-0020/0026).
        let (host, joiner) = setup("hold_keeps");
        let (raw, sig) = signed_request(&joiner, NOW, "n1");
        submit_request(&host, &raw, &sig, NOW).unwrap();
        deny(&host, &joiner.node_id(), NOW + 5).unwrap();
        assert!(matches!(
            enroll_status(&host, &joiner.node_id()).unwrap(),
            StatusOutcome::Granted(_)
        ));
        let rec = crate::record::load(&host, &joiner.node_id())
            .unwrap()
            .unwrap();
        assert_eq!(rec.held_until, Some(NOW + 5 + DENY_RETRY_SECS));
    }
}
