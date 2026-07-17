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

/// A node's attestation that it accepts the Three Laws — the covenant it asks to join under.
/// Retained on approval so a node can be held to what it accepted.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Refuse a pending request (removes it). Returns whether one was there.
pub fn deny(dir: &Path, node_id: &str) -> Result<bool> {
    let path = pending_path(dir, node_id);
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
        &[("X-Familiar-Sig", &sig), ("Content-Type", "application/json")],
        &raw,
    )?;
    match status {
        200 => {
            let grant: Grant = serde_json::from_slice(&body)?;
            persist_covenant(dir, &grant)?;
            Ok(JoinOutcome::Admitted(Box::new(grant)))
        }
        202 => Ok(JoinOutcome::Pending),
        403 => Err(Error::Untrusted(String::from_utf8_lossy(&body).into_owned())),
        _ => Err(Error::Malformed(format!("enroll-request: HTTP {status}"))),
    }
}

/// Poll a familiar for the decision on our request. Returns the grant (persisted) once approved,
/// `None` while still pending; `Untrusted` if the request was declined/removed.
pub fn poll_join(dir: &Path, host: &str, port: u16, node_id: &str) -> Result<Option<Grant>> {
    let (status, body) = http(host, port, "GET", &format!("/mesh/enroll-status/{node_id}"), &[], &[])?;
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

/// A minimal blocking HTTP/1.1 client — dependency-free (std `TcpStream`), matching the crate's
/// no-crates ethos. Sends `Connection: close` and reads the response to EOF, then splits head/body.
/// Sufficient for the small JSON bodies the enroll endpoints return on a LAN/tailnet.
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
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5)).map_err(Error::Io)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;

    let mut req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    stream.write_all(req.as_bytes())?;
    stream.write_all(body)?;

    let mut buf = Vec::new();
    stream.read_to_end(&mut buf)?;
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
    let membership =
        cred.mint_membership(&node.node_id, &node.pubkey, now, group::DEFAULT_CERT_TTL_SECS)?;
    let grant = Grant {
        membership,
        group_id: cred.group_id.clone(),
        group_pubkey: cred.group_pubkey.clone(),
        group_label: cred.label.clone(),
    };
    write_json(dir, GRANTED_DIR, &node.node_id, &grant)?;
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
    std::fs::write(d.join(format!("{node_id}.json")), serde_json::to_vec_pretty(v)?)?;
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
        assert!(matches!(enroll_status(&host, &joiner.node_id()).unwrap(), StatusOutcome::Pending));
        assert_eq!(list_pending(&host).unwrap().len(), 1);

        // Human approves → a grant whose cert verifies under the group key.
        let grant = approve(&host, &joiner.node_id(), NOW).unwrap();
        let cred = group::load(&host).unwrap().unwrap();
        let gk = cred.verifying_key().unwrap();
        group::verify_membership(&grant.membership, &gk, &cred.group_id, NOW, &[]).unwrap();
        assert_eq!(grant.membership.node_id, joiner.node_id());
        assert!(list_pending(&host).unwrap().is_empty());

        // The joiner can now poll and receive the grant.
        assert!(matches!(enroll_status(&host, &joiner.node_id()).unwrap(), StatusOutcome::Granted(_)));
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
        assert!(matches!(submit_request(&host, &raw2, &sig2, NOW + 400).unwrap(), Submitted::Pending(_)));
    }

    #[test]
    fn a_forged_or_unbound_request_is_untrusted() {
        let (host, joiner) = setup("forge");
        let (raw, _good) = signed_request(&joiner, NOW, "n1");
        // Signature over different bytes → rejected.
        let wrong = joiner.sign(b"not the request");
        assert!(matches!(submit_request(&host, &raw, &wrong, NOW), Err(Error::Untrusted(_))));
    }

    #[test]
    fn deny_removes_a_pending_request() {
        let (host, joiner) = setup("deny");
        let (raw, sig) = signed_request(&joiner, NOW, "n1");
        submit_request(&host, &raw, &sig, NOW).unwrap();
        assert!(deny(&host, &joiner.node_id()).unwrap());
        assert!(!deny(&host, &joiner.node_id()).unwrap()); // already gone
        assert!(matches!(enroll_status(&host, &joiner.node_id()).unwrap(), StatusOutcome::Unknown));
    }
}
