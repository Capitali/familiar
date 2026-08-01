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

    let pending = Pending {
        node: req.node.clone(),
        attestation: req.attestation.clone(),
        received_at: now,
        code: short_code(&req.node.node_id),
    };

    // Auto-admit if the human has set a standing auto-accept, or opened a timed invite window. A node
    // that attests the Laws (verified above: it signed a non-empty covenant statement with the key
    // its id fingerprints) is admitted without a second approval. This stays a *deliberate* switch,
    // not implied by `allow_mesh` — a headless node may serve the mesh yet route each enrollment to a
    // human for approval (the authority proxy). Opening auto-peering is its own human decision.
    let auto = crate::config::load(dir)
        .map(|c| c.auto_accept_enrollments)
        .unwrap_or(false);
    if auto || invite_open(dir, now) {
        let grant = mint_grant(dir, &cred, &req.node, now)?;
        remove_pending(dir, &req.node.node_id)?;
        return Ok(Submitted::Granted(Box::new(grant)));
    }

    write_json(dir, PENDING_DIR, &req.node.node_id, &pending)?;
    Ok(Submitted::Pending(pending))
}

/// A node polling for a decision on its request.
pub(crate) fn enroll_status(dir: &Path, node_id: &str) -> Result<StatusOutcome> {
    if let Some(grant) = load_grant(dir, node_id)? {
        return Ok(StatusOutcome::Granted(Box::new(grant)));
    }
    if pending_path(dir, node_id).exists() {
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
    let grant = mint_grant(dir, &cred, &pending.node, now)?;
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

/// Let a denied node ask again immediately — the undo for a mis-tap.
pub fn allow_retry(dir: &Path, node_id: &str) -> Result<bool> {
    let path = dir.join(DENIED_DIR).join(format!("{node_id}.json"));
    if path.exists() {
        std::fs::remove_file(&path)?;
        return Ok(true);
    }
    Ok(false)
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

/// Open a pairing/invite window until `until` (unix secs): requests that arrive before then are
/// auto-approved. Use for "authorize this expansion once" so many devices don't need many taps.
pub fn open_invite(dir: &Path, until: i64) -> Result<()> {
    write_raw(dir, INVITE_FILE, &until.to_string())
}

/// When the invite window closes (0 / absent = no window).
pub fn invite_until(dir: &Path) -> i64 {
    std::fs::read_to_string(dir.join(INVITE_FILE))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

fn invite_open(dir: &Path, now: i64) -> bool {
    now < invite_until(dir)
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
fn http(
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
    now: i64,
) -> Result<Grant> {
    let membership = cred.mint_membership(
        &node.node_id,
        &node.pubkey,
        now,
        group::DEFAULT_CERT_TTL_SECS,
    )?;
    let grant = Grant {
        membership,
        group_id: cred.group_id.clone(),
        group_pubkey: cred.group_pubkey.clone(),
        group_label: cred.label.clone(),
    };
    write_json(dir, GRANTED_DIR, &node.node_id, &grant)?;
    // Admission is automatic (ADR-0015); DISCLOSURE is not (ADR-0020) — a fresh member reads as
    // a guest until a human grants standing. That decision needs a human to actually be ASKED,
    // or a new member sits unnoticed forever (the 2026-07-29 installer sat in the roster and
    // nobody was asked anything). So admitting files a question on this node; devices reading
    // their worldview from here surface it, routing addresses it to whoever is present, and the
    // question system's own cadence (re-asking, rest periods) is the escalation. Origin "need":
    // a stranger awaiting a human decision IS an unmet need for human authority. Best-effort —
    // a failure to file must never fail the admission itself. Nothing ever auto-grants; an
    // unanswered question leaves the member a guest, which is the safe resting state.
    let short: String = node.node_id.chars().take(8).collect();
    let label = if node.label.trim().is_empty() {
        short.clone()
    } else {
        node.label.trim().to_string()
    };
    let _ = familiar_kernel::question::add(
        dir,
        &format!(
            "A new device joined the mesh: “{label}” ({short}). Who does it belong to? \
It reads as a guest until someone grants it standing."
        ),
        "need",
        now,
    );
    Ok(grant)
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
    fn request_pends_then_approval_grants_a_verifiable_cert() {
        let (host, joiner) = setup("approve");
        let (raw, sig) = signed_request(&joiner, NOW, "n1");

        // Submit → pending (no invite window).
        match submit_request(&host, &raw, &sig, NOW).unwrap() {
            Submitted::Pending(p) => assert_eq!(p.node.node_id, joiner.node_id()),
            _ => panic!("expected pending"),
        }
        assert!(matches!(
            enroll_status(&host, &joiner.node_id()).unwrap(),
            StatusOutcome::Pending
        ));
        assert_eq!(list_pending(&host).unwrap().len(), 1);

        // Human approves → a grant whose cert verifies under the group key.
        let grant = approve(&host, &joiner.node_id(), NOW).unwrap();
        let cred = group::load(&host).unwrap().unwrap();
        let gk = cred.verifying_key().unwrap();
        group::verify_membership(&grant.membership, &gk, &cred.group_id, NOW, &[]).unwrap();
        assert_eq!(grant.membership.node_id, joiner.node_id());
        assert!(list_pending(&host).unwrap().is_empty());

        // The joiner can now poll and receive the grant.
        assert!(matches!(
            enroll_status(&host, &joiner.node_id()).unwrap(),
            StatusOutcome::Granted(_)
        ));
    }

    #[test]
    fn admission_asks_who_the_new_device_belongs_to() {
        // ADR-0020: admission is automatic, disclosure is not — and the standing decision needs a
        // human to actually be ASKED. Every path that mints a grant must file the question; the
        // 2026-07-29 installer sat in the roster precisely because nothing did.
        let (host, joiner) = setup("asks");
        open_invite(&host, NOW + 300).unwrap();
        let (raw, sig) = signed_request(&joiner, NOW, "nq");
        assert!(matches!(
            submit_request(&host, &raw, &sig, NOW).unwrap(),
            Submitted::Granted(_)
        ));
        let qs = familiar_kernel::question::load(&host).unwrap();
        let q = qs
            .iter()
            .find(|q| q.text.contains("Kali-Jeff") && q.text.contains("standing"))
            .expect("admitting a device must file a who-is-this question");
        assert_eq!(
            q.origin, "need",
            "a stranger awaiting a decision outranks the root question"
        );
        // Idempotent per node: a second admission of the same device must not ask twice.
        let n = qs.iter().filter(|x| x.text == q.text).count();
        assert_eq!(n, 1);
    }

    #[test]
    fn a_denial_holds_for_five_minutes_then_lets_them_ask_again() {
        let (host, joiner) = setup("deny_window");
        let (raw, sig) = signed_request(&joiner, NOW, "d1");
        assert!(matches!(
            submit_request(&host, &raw, &sig, NOW).unwrap(),
            Submitted::Pending(_)
        ));

        assert!(
            deny(&host, &joiner.node_id(), NOW).unwrap(),
            "a pending record was there"
        );
        assert!(
            list_pending(&host).unwrap().is_empty(),
            "denying clears the pending"
        );

        // Inside the window: ignored outright, not re-queued — the whole point, so a retry loop
        // cannot train a human to dismiss the ask reflexively.
        let (raw2, sig2) = signed_request(&joiner, NOW + 10, "d2");
        match submit_request(&host, &raw2, &sig2, NOW + 10).unwrap() {
            Submitted::Denied { retry_in } => assert!((1..=DENY_RETRY_SECS).contains(&retry_in)),
            _ => panic!("expected Denied inside the retry window"),
        }
        assert!(
            list_pending(&host).unwrap().is_empty(),
            "a denied retry must not re-queue"
        );

        // After the window: they may ask again, and it pends normally.
        let (raw3, sig3) = signed_request(&joiner, NOW + DENY_RETRY_SECS + 1, "d3");
        assert!(matches!(
            submit_request(&host, &raw3, &sig3, NOW + DENY_RETRY_SECS + 1).unwrap(),
            Submitted::Pending(_)
        ));
        assert_eq!(list_pending(&host).unwrap().len(), 1);
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
    fn invite_window_auto_approves() {
        let (host, joiner) = setup("invite");
        open_invite(&host, NOW + 300).unwrap();
        let (raw, sig) = signed_request(&joiner, NOW, "n1");
        match submit_request(&host, &raw, &sig, NOW).unwrap() {
            Submitted::Granted(g) => assert_eq!(g.membership.node_id, joiner.node_id()),
            _ => panic!("invite window should auto-approve"),
        }
        // After the window closes, a new joiner pends again.
        let other = NodeKey::load_or_mint(&fresh("invite_other"), "phone").unwrap();
        let (raw2, sig2) = signed_request(&other, NOW + 400, "n2");
        assert!(matches!(
            submit_request(&host, &raw2, &sig2, NOW + 400).unwrap(),
            Submitted::Pending(_)
        ));
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
    fn deny_removes_a_pending_request() {
        let (host, joiner) = setup("deny");
        let (raw, sig) = signed_request(&joiner, NOW, "n1");
        submit_request(&host, &raw, &sig, NOW).unwrap();
        assert!(deny(&host, &joiner.node_id(), NOW).unwrap());
        assert!(!deny(&host, &joiner.node_id(), NOW).unwrap()); // already gone
        assert!(matches!(
            enroll_status(&host, &joiner.node_id()).unwrap(),
            StatusOutcome::Unknown
        ));
    }
}
