//! The smallest HTTP client that can speak MCP's Streamable HTTP transport: one blocking
//! `POST`, a bounded response, and no dependency beyond the rustls this workspace already
//! carries. Same posture as `mesh::enroll::http` — deliberately small rather than general.
//!
//! It understands two response framings, because MCP servers may answer either way:
//! a plain `application/json` body, or `text/event-stream` where the JSON rides in `data:`
//! lines. Anything else is an error, never a guess.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::{Error, Url};

/// Longest response this client will read. A partner server is not a reason to accept an
/// unbounded body into memory.
const MAX_BODY: usize = 4 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const IO_TIMEOUT: Duration = Duration::from_secs(30);

/// What a server answered: the status, the headers as sent (an MCP server may hand back an
/// `Mcp-Session-Id` there), and the decoded body.
#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub headers: String,
    pub body: Vec<u8>,
}

impl Response {
    /// One header by name, case-insensitively — HTTP field names are not case-sensitive and a
    /// client that assumes otherwise breaks on the first server that disagrees.
    pub fn header(&self, name: &str) -> Option<String> {
        let want = name.to_ascii_lowercase();
        self.headers.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            (k.trim().to_ascii_lowercase() == want).then(|| v.trim().to_string())
        })
    }
}

/// POST `body` as JSON.
///
/// `https` verifies the certificate chain ([`crate::tls`]); `http` is permitted only for
/// loopback, so a test stub can exist without opening a way to send a token in the clear.
pub fn post_json(url: &Url, headers: &[(String, String)], body: &[u8]) -> Result<Response, Error> {
    request("POST", url, headers, Some(body))
}

/// GET, under the same rules as [`post_json`] — same verifying TLS, same
/// loopback-only-plain-http floor, same bounded read. A body is never sent.
pub fn get(url: &Url, headers: &[(String, String)]) -> Result<Response, Error> {
    request("GET", url, headers, None)
}

fn request(
    method: &str,
    url: &Url,
    headers: &[(String, String)],
    body: Option<&[u8]>,
) -> Result<Response, Error> {
    if !url.https && !url.is_loopback() {
        return Err(Error::Insecure(format!(
            "{} is plain http and not loopback — a credential never travels in the clear",
            url.origin()
        )));
    }
    // Try EVERY resolved address, not just the first. A dual-stack host whose
    // IPv6 is dead (the UCF exchange over the RV's Starlink link, 2026-09-01:
    // AAAA present but refusing, A fine) would otherwise fail whenever the
    // resolver hands back the v6 address first — connection refused on a coin
    // flip, while curl's happy-eyeballs sails through. v4 is tried first because
    // on this network it is the working family; the loop still tries v6 if v4 is
    // the one that is down. The last error is surfaced if none connect.
    let mut addrs: Vec<_> = (url.host.as_str(), url.port)
        .to_socket_addrs()
        .map_err(|e| Error::Io(format!("resolve {}: {e}", url.origin())))?
        .collect();
    if addrs.is_empty() {
        return Err(Error::Io(format!("no address for {}", url.origin())));
    }
    addrs.sort_by_key(|a| a.is_ipv6());
    let mut sock = {
        let mut last = None;
        let mut connected = None;
        for addr in &addrs {
            match TcpStream::connect_timeout(addr, CONNECT_TIMEOUT) {
                Ok(s) => {
                    connected = Some(s);
                    break;
                }
                Err(e) => last = Some(e),
            }
        }
        connected.ok_or_else(|| {
            Error::Io(
                last.map(|e| e.to_string())
                    .unwrap_or_else(|| "connect failed".into()),
            )
        })?
    };
    sock.set_read_timeout(Some(IO_TIMEOUT))
        .map_err(|e| Error::Io(e.to_string()))?;
    sock.set_write_timeout(Some(IO_TIMEOUT))
        .map_err(|e| Error::Io(e.to_string()))?;

    let body = body.unwrap_or(&[]);
    let mut head = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\n\
         Accept: application/json, text/event-stream\r\nContent-Length: {}\r\n\
         Connection: close\r\nUser-Agent: familiar-mcp/0.1\r\n",
        method,
        url.path,
        url.host_header(),
        body.len()
    );
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");

    let raw = if url.https {
        let cfg = crate::tls::verifying_config()?;
        let name = rustls::pki_types::ServerName::try_from(url.host.clone())
            .map_err(|_| Error::Insecure(format!("{} is not a valid server name", url.host)))?;
        let mut conn =
            rustls::ClientConnection::new(cfg, name).map_err(|e| Error::Io(format!("tls: {e}")))?;
        let mut tls = rustls::Stream::new(&mut conn, &mut sock);
        write_all(&mut tls, head.as_bytes(), body)?;
        read_bounded(&mut tls)?
    } else {
        write_all(&mut sock, head.as_bytes(), body)?;
        read_bounded(&mut sock)?
    };
    split_response(&raw)
}

fn write_all<W: Write>(w: &mut W, head: &[u8], body: &[u8]) -> Result<(), Error> {
    w.write_all(head).map_err(|e| Error::Io(e.to_string()))?;
    w.write_all(body).map_err(|e| Error::Io(e.to_string()))?;
    w.flush().map_err(|e| Error::Io(e.to_string()))
}

fn read_bounded<R: Read>(r: &mut R) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match r.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                out.extend_from_slice(&buf[..n]);
                if out.len() > MAX_BODY {
                    return Err(Error::Io(format!("response exceeded {MAX_BODY} bytes")));
                }
            }
            // A server that closes without a clean TLS shutdown is ordinary in the wild; what
            // was already read still counts.
            Err(e) if !out.is_empty() && e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(Error::Io(e.to_string())),
        }
    }
    Ok(out)
}

/// Split a raw HTTP/1.1 response, undoing chunked framing and SSE.
fn split_response(raw: &[u8]) -> Result<Response, Error> {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| Error::Io("truncated response — no header terminator".into()))?;
    let status: u16 = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| Error::Io("unreadable status line".into()))?;
    let lower = head.to_ascii_lowercase();
    let chunked = lower.contains("transfer-encoding: chunked");
    let body = if chunked {
        dechunk(body)
    } else {
        body.to_string()
    };
    let body = if lower.contains("text/event-stream") {
        sse_data(&body)
    } else {
        body
    };
    Ok(Response {
        status,
        headers: head.to_string(),
        body: body.into_bytes(),
    })
}

/// Chunked transfer, undone. Sizes are hex, each chunk followed by CRLF, zero ends it.
fn dechunk(body: &str) -> String {
    let mut out = String::new();
    let mut rest = body;
    while let Some((size_line, tail)) = rest.split_once("\r\n") {
        let size =
            usize::from_str_radix(size_line.trim().split(';').next().unwrap_or("0").trim(), 16)
                .unwrap_or(0);
        if size == 0 || tail.len() < size {
            out.push_str(&tail[..tail.len().min(size)]);
            break;
        }
        out.push_str(&tail[..size]);
        rest = tail[size..].trim_start_matches("\r\n");
    }
    out
}

/// The JSON carried by an SSE stream: every `data:` line, concatenated in order.
fn sse_data(body: &str) -> String {
    body.lines()
        .filter_map(|l| l.strip_prefix("data:"))
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both framings a real MCP server may answer with, and the status alongside.
    #[test]
    fn json_and_sse_bodies_both_arrive_as_json() {
        let plain = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                      Mcp-Session-Id: abc123\r\n\r\n{\"ok\":true}";
        let r = split_response(plain).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(String::from_utf8(r.body.clone()).unwrap(), "{\"ok\":true}");
        // Header names are case-insensitive on the wire, so the lookup must be too.
        assert_eq!(r.header("mcp-session-id").as_deref(), Some("abc123"));
        assert_eq!(r.header("MCP-SESSION-ID").as_deref(), Some("abc123"));
        assert!(r.header("nope").is_none());

        let sse = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n\
                    event: message\r\ndata: {\"ok\":true}\r\n\r\n";
        assert_eq!(
            String::from_utf8(split_response(sse).unwrap().body).unwrap(),
            "{\"ok\":true}"
        );

        let chunked = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                        Transfer-Encoding: chunked\r\n\r\n5\r\n{\"a\":\r\n2\r\n1}\r\n0\r\n\r\n";
        assert_eq!(
            String::from_utf8(split_response(chunked).unwrap().body).unwrap(),
            "{\"a\":1}"
        );

        assert_eq!(
            split_response(b"HTTP/1.1 401 x\r\n\r\nno").unwrap().status,
            401
        );
        assert!(split_response(b"garbage").is_err());
    }

    /// A token never goes out in the clear. Loopback is exempt so a stub server can exist.
    #[test]
    fn plain_http_is_refused_off_loopback() {
        let remote = Url::parse("http://example.invalid/mcp").unwrap();
        let e = post_json(&remote, &[], b"{}").unwrap_err();
        assert!(matches!(e, Error::Insecure(_)), "{e:?}");
        assert!(Url::parse("http://127.0.0.1:9/mcp").unwrap().is_loopback());
    }
}
