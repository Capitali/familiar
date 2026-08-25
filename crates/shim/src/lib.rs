//! The stdio→loopback shim — one reusable bridge from the familiar's HTTP MCP client to
//! the stdio-first MCP world (T-225's standing infra finding: NASA, orbital, victron-tcp,
//! signalk and most of the surveyed registry speak stdio, and the familiar's client
//! deliberately speaks only Streamable HTTP).
//!
//! **What this is NOT.** The shim mints no authority. A server behind it exists to the
//! familiar only if a human wrote it into `mcp/servers.json` (declaration is consent,
//! ADR-0032), a tool may be called only if the declaration names it, and every call still
//! rides `guard::evaluate` + `allow_network` exactly as before. The shim only moves bytes
//! between one loopback HTTP port and one child process's stdin/stdout.
//!
//! **The fence the loopback port needs.** Any local process could otherwise drive the
//! child through the shim's port, so every request must carry a bearer token minted at
//! startup (never a fixed value). The token can be written to an env-format key file
//! (mode 0600) — the same file `servers.json` names via `key_file`/`key_name`, so the
//! familiar's client sends it without either side learning anything new.
//!
//! **Honest limits.** The shim answers request→response and swallows nothing silently:
//! a server-initiated message that is not the awaited response (logging notifications,
//! sampling requests) is SKIPPED with a bounded budget — the target servers are pure
//! tool providers, and a server chatty past the budget is an error, not a hang. One
//! request runs at a time (the child's stdout is one ordered stream); bodies and
//! response lines are bounded; a dead child is a stated 502, never a retry.

use std::io::{BufRead, Read, Write};
use std::sync::Mutex;

/// Longest request body and longest child response line the shim will carry — the same
/// bound the familiar's own client puts on a response body.
pub const MAX_BODY: usize = 4 * 1024 * 1024;
/// Longest HTTP request head (request line + headers) accepted.
const MAX_HEAD: usize = 16 * 1024;
/// How many non-matching lines the shim will read past while awaiting a response before
/// declaring the server too chatty to bridge. Skipped lines are counted, never buffered.
pub const MAX_SKIPPED_LINES: usize = 256;

#[derive(Debug)]
pub enum Error {
    /// The child's stream ended or refused I/O — the server is gone.
    ChildGone(String),
    /// The child spoke something the bridge cannot honestly relay.
    Protocol(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ChildGone(what) => write!(f, "child gone: {what}"),
            Error::Protocol(what) => write!(f, "protocol: {what}"),
        }
    }
}

/// Write one JSON-RPC message to the child and, if it carries an `id`, read until the
/// response with that `id` arrives. Returns `None` for a notification (no `id` — nothing
/// is owed back). Non-matching lines are skipped within [`MAX_SKIPPED_LINES`].
///
/// MCP's stdio transport frames messages as newline-delimited JSON, so the outgoing
/// message must not contain a literal newline — `serde_json::to_string` guarantees that.
pub fn exchange<W: Write, R: BufRead>(
    child_in: &mut W,
    child_out: &mut R,
    message: &serde_json::Value,
) -> Result<Option<String>, Error> {
    let line = serde_json::to_string(message)
        .map_err(|e| Error::Protocol(format!("unencodable message: {e}")))?;
    child_in
        .write_all(line.as_bytes())
        .and_then(|()| child_in.write_all(b"\n"))
        .and_then(|()| child_in.flush())
        .map_err(|e| Error::ChildGone(format!("stdin write: {e}")))?;

    let Some(id) = message.get("id").filter(|id| !id.is_null()) else {
        return Ok(None); // a notification: delivered, nothing owed back
    };

    let mut skipped = 0usize;
    loop {
        let mut line = String::new();
        let read = bounded_read_line(child_out, &mut line)?;
        if read == 0 {
            return Err(Error::ChildGone("stdout closed before the response".into()));
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(value) if value.get("id") == Some(id) => return Ok(Some(trimmed.to_string())),
            Ok(_) => {
                // A server-initiated message (log notification, sampling request, another
                // call's leftovers). The shim is a request/response bridge and says so.
                skipped += 1;
                if skipped > MAX_SKIPPED_LINES {
                    return Err(Error::Protocol(format!(
                        "server sent {skipped} messages without answering id {id}"
                    )));
                }
            }
            Err(e) => return Err(Error::Protocol(format!("unparseable line from child: {e}"))),
        }
    }
}

/// Read one newline-terminated line, refusing past [`MAX_BODY`] — a runaway child must
/// not grow the shim's memory without limit.
fn bounded_read_line<R: BufRead>(reader: &mut R, out: &mut String) -> Result<usize, Error> {
    let mut bytes = Vec::new();
    loop {
        let buf = reader
            .fill_buf()
            .map_err(|e| Error::ChildGone(format!("stdout read: {e}")))?;
        if buf.is_empty() {
            break; // EOF
        }
        let (chunk, done) = match buf.iter().position(|&b| b == b'\n') {
            Some(at) => (&buf[..=at], true),
            None => (buf, false),
        };
        if bytes.len() + chunk.len() > MAX_BODY {
            return Err(Error::Protocol(format!(
                "child line exceeds the {MAX_BODY}-byte bound"
            )));
        }
        bytes.extend_from_slice(chunk);
        let used = chunk.len();
        reader.consume(used);
        if done {
            break;
        }
    }
    let text = String::from_utf8(bytes)
        .map_err(|e| Error::Protocol(format!("child line is not UTF-8: {e}")))?;
    let len = text.len();
    out.push_str(&text);
    Ok(len)
}

/// What the HTTP layer needs from the child side — a seam so the layer is testable
/// without spawning processes.
pub trait Bridge: Send + Sync {
    fn exchange(&self, message: &serde_json::Value) -> Result<Option<String>, Error>;
}

/// The real bridge: one child process, one exchange at a time.
pub struct ChildBridge {
    io: Mutex<(
        std::process::ChildStdin,
        std::io::BufReader<std::process::ChildStdout>,
    )>,
}

impl ChildBridge {
    /// Spawn `command args…` with piped stdin/stdout. Stderr is inherited on purpose —
    /// a vendor server's own logging belongs on the operator's terminal, not hidden.
    pub fn spawn(command: &str, args: &[String]) -> std::io::Result<(std::process::Child, Self)> {
        let mut child = std::process::Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = std::io::BufReader::new(child.stdout.take().expect("piped stdout"));
        Ok((
            child,
            Self {
                io: Mutex::new((stdin, stdout)),
            },
        ))
    }
}

impl Bridge for ChildBridge {
    fn exchange(&self, message: &serde_json::Value) -> Result<Option<String>, Error> {
        let mut io = self
            .io
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (child_in, child_out) = &mut *io;
        exchange(child_in, child_out, message)
    }
}

/// Mint the per-run bearer token: 32 hex chars of OS randomness, never a fixed value.
pub fn mint_token() -> std::io::Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes)
        .map_err(|e| std::io::Error::other(format!("getrandom: {e}")))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Write the token as an env-format key file, mode 0600 — the same shape
/// `mcp/servers.json` names with `key_file`/`key_name`, so declaring the shimmed server
/// needs nothing new.
pub fn write_key_file(path: &std::path::Path, token: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, format!("TOKEN={token}\n"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Serve the bridge on an already-bound loopback listener until the listener errors or
/// the child dies. Each connection is one request (`Connection: close`) — the familiar's
/// own client speaks exactly that.
pub fn serve(listener: std::net::TcpListener, token: &str, bridge: &dyn Bridge) {
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
        let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(30)));
        let (status, body) = match handle(&mut stream, token, bridge) {
            Ok(reply) => reply,
            Err(reply) => reply,
        };
        let _ = respond(&mut stream, status, body.as_deref());
        let _ = stream.shutdown(std::net::Shutdown::Both);
    }
}

type Reply = (u16, Option<String>);

/// One request → one reply. Refusals are typed by status and never touch the child:
/// the token check comes before the body is even read.
fn handle(
    stream: &mut std::net::TcpStream,
    token: &str,
    bridge: &dyn Bridge,
) -> Result<Reply, Reply> {
    let (head, leftover) = read_head(stream)?;
    let mut lines = head.lines();
    let request = lines.next().unwrap_or_default();
    let mut parts = request.split_whitespace();
    let (method, path) = (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
    );
    let header = |name: &str| -> Option<String> {
        let want = name.to_ascii_lowercase();
        head.lines().skip(1).find_map(|l| {
            let (k, v) = l.split_once(':')?;
            (k.trim().to_ascii_lowercase() == want).then(|| v.trim().to_string())
        })
    };

    // The fence first: no token, no shim — before the body, before the path.
    if header("authorization") != Some(format!("Bearer {token}")) {
        return Err((401, Some("missing or wrong bearer token".into())));
    }
    if path != "/mcp" {
        return Err((404, Some("the shim serves /mcp only".into())));
    }
    if method != "POST" {
        return Err((405, Some("the shim answers POST only".into())));
    }
    let length: usize = header("content-length")
        .and_then(|v| v.parse().ok())
        .ok_or((411, Some("Content-Length required".into())))?;
    if length > MAX_BODY {
        return Err((413, Some(format!("body exceeds the {MAX_BODY}-byte bound"))));
    }
    // The body may have arrived with the head (leftover) or may still be on the wire —
    // count what is already here BEFORE growing the buffer, or the read never happens.
    let mut body = leftover;
    body.truncate(length);
    let already = body.len();
    if already < length {
        body.resize(length, 0);
        stream
            .read_exact(&mut body[already..])
            .map_err(|e| (400, Some(format!("short body: {e}"))))?;
    }
    let message: serde_json::Value = serde_json::from_slice(&body[..length])
        .map_err(|e| (400, Some(format!("body is not JSON: {e}"))))?;
    if !message.is_object() {
        // Single messages only. A batch would interleave responses on one stdout stream —
        // refusing it honestly beats relaying it wrong.
        return Err((
            400,
            Some("one JSON-RPC object per request; no batches".into()),
        ));
    }
    match bridge.exchange(&message) {
        Ok(Some(reply)) => Ok((200, Some(reply))),
        Ok(None) => Ok((202, None)),
        Err(e) => Err((502, Some(e.to_string()))),
    }
}

/// Read up to the blank line, bounded by [`MAX_HEAD`]; returns the head and whatever
/// body bytes were already read past it.
fn read_head(stream: &mut std::net::TcpStream) -> Result<(String, Vec<u8>), Reply> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        if let Some(at) = find_blank(&buf) {
            let head = String::from_utf8_lossy(&buf[..at]).into_owned();
            return Ok((head, buf[at + 4..].to_vec()));
        }
        if buf.len() > MAX_HEAD {
            return Err((431, Some("request head too large".into())));
        }
        let n = stream
            .read(&mut chunk)
            .map_err(|e| (400, Some(format!("read: {e}"))))?;
        if n == 0 {
            return Err((400, Some("connection closed mid-head".into())));
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

fn find_blank(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn respond(
    stream: &mut std::net::TcpStream,
    status: u16,
    body: Option<&str>,
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        411 => "Length Required",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        502 => "Bad Gateway",
        _ => "Error",
    };
    let (content_type, body) = match (status, body) {
        (200, Some(body)) => ("application/json", body.to_string()),
        (_, Some(body)) => ("text/plain", body.to_string()),
        (_, None) => ("text/plain", String::new()),
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[cfg(test)]
mod tests;
