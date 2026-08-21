//! **The MCP seam, client half** (T-206, ADR-0037 §A) — the familiar reaching another
//! system's tools and resources over the Model Context Protocol.
//!
//! The inversion is the whole design: rather than a partner pushing telemetry at the familiar
//! through thirteen bespoke endpoints, **the partner runs an MCP server and the familiar is
//! its client.** A ship's systems then arrive as *tool discovery*, which MCP does natively,
//! and its state as resources — which is the shape ADR-0032's declared actuators already
//! have. Nothing new to invent, and nothing new to trust:
//!
//! - **The boundary governs it.** Every call passes `guard::evaluate` before a socket opens.
//!   A shut gate is a refusal with a rationale, not an error.
//! - **Undeclared is unactuatable** (ADR-0032). A server exists to the familiar because a
//!   human wrote it into `mcp/servers.json`, and a tool may be *called* only if that
//!   declaration names it. Discovery is not permission: the familiar may always ask a server
//!   what it offers, and may only invoke what its human wrote down.
//! - **A disconnected server is not an error condition**, it is a system that is not reachable
//!   — the no-oracle floor (ADR-0035). Callers get [`Error`], never a panic or a fabrication.
//!
//! What this crate deliberately does **not** do: expose the familiar's own MCP server
//! (`purr.say`, `purr.utterances`). That is the other half of T-206 and a later brick; the
//! counterparty that exists today (`ucf-exchange`) is read-only, so the client is the half
//! that does anything at all.
//!
//! **On trust:** an MCP server is a stranger with delegated capability (ADR-0041). It is
//! identified by its declaration, never by the protocol — nothing a server says about itself
//! (its name, its `readOnlyHint`s) widens what the familiar will do with it.

use std::path::Path;

use familiar_kernel::boundary;
use familiar_kernel::guard::{self, Action, ActionKind, Decision};

pub mod covenant;
pub mod declaration;
pub mod grant;
pub mod http;
pub mod inbox;
pub mod offering;
pub mod partner;
pub mod partner_act;
pub mod server;
pub mod serving;
pub mod session;
pub mod tls;

pub use declaration::{Server, ServerSet};
pub use session::{Annotations, Claimed, Session, Tool};

/// Everything that can go wrong reaching a partner, kept apart so a caller can tell a shut
/// gate from a broken wire from a server that answered rubbish.
#[derive(Debug)]
pub enum Error {
    /// The human-owned boundary said no. Carries the guard's rationale verbatim.
    Refused(String),
    /// No server by that name is declared, or the declaration is unreadable.
    Undeclared(String),
    /// The declaration names the server but not this tool — discovery is not permission.
    UndeclaredTool { server: String, tool: String },
    /// No verifying trust store, so a credential would travel unverified.
    NoTrustStore(String),
    /// The wire would have carried a credential in the clear.
    Insecure(String),
    /// Network, TLS, or framing failure.
    Io(String),
    /// The server answered, but not with what MCP says it should.
    Protocol(String),
    /// The server answered with a JSON-RPC error.
    Server { code: i64, message: String },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Refused(r) => write!(f, "refused by the boundary: {r}"),
            Error::Undeclared(s) => write!(f, "no declared MCP server named {s}"),
            Error::UndeclaredTool { server, tool } => {
                write!(
                    f,
                    "{server} offers {tool}, but the declaration does not name it"
                )
            }
            Error::NoTrustStore(w) => write!(f, "{w}"),
            Error::Insecure(w) => write!(f, "{w}"),
            Error::Io(w) => write!(f, "unreachable: {w}"),
            Error::Protocol(w) => write!(f, "the server broke the protocol: {w}"),
            Error::Server { code, message } => write!(f, "server error {code}: {message}"),
        }
    }
}

impl std::error::Error for Error {}

/// A parsed absolute URL — enough of one for this crate, and no more.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    pub https: bool,
    pub host: String,
    pub port: u16,
    pub path: String,
}

impl Url {
    pub fn parse(raw: &str) -> Result<Self, Error> {
        let (scheme, rest) = raw
            .split_once("://")
            .ok_or_else(|| Error::Protocol(format!("{raw} is not an absolute URL")))?;
        let https = match scheme {
            "https" => true,
            "http" => false,
            other => return Err(Error::Protocol(format!("unsupported scheme {other}"))),
        };
        let (authority, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        if authority.contains('@') {
            return Err(Error::Protocol(
                "credentials in the URL — put the token in the declaration's key file".into(),
            ));
        }
        let (host, port) = match authority.rsplit_once(':') {
            Some((h, p)) => (
                h.to_string(),
                p.parse()
                    .map_err(|_| Error::Protocol(format!("bad port in {raw}")))?,
            ),
            None => (authority.to_string(), if https { 443 } else { 80 }),
        };
        if host.is_empty() {
            return Err(Error::Protocol(format!("{raw} has no host")));
        }
        Ok(Url {
            https,
            host,
            port,
            path: path.to_string(),
        })
    }

    pub fn origin(&self) -> String {
        format!(
            "{}://{}",
            if self.https { "https" } else { "http" },
            self.host_header()
        )
    }

    pub fn host_header(&self) -> String {
        let default = if self.https { 443 } else { 80 };
        if self.port == default {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    pub fn is_loopback(&self) -> bool {
        self.host == "localhost"
            || self
                .host
                .parse::<std::net::IpAddr>()
                .map(|ip| ip.is_loopback())
                .unwrap_or(false)
    }
}

/// The boundary check every outward MCP act passes, before any socket opens.
///
/// Reaching a partner's server is a network act, so it rides `allow_network` — the same gate
/// as any other outward reach, with the target named so the refusal rationale can say where
/// the familiar was about to go. `boundary::load` falls back to `closed()`, so an unreadable
/// boundary means no call, which is the correct direction to fail.
pub fn permitted(dir: &Path, url: &Url) -> Result<(), Error> {
    let b = boundary::load(dir).map_err(|e| Error::Io(e.to_string()))?;
    let verdict = guard::evaluate(&Action::new(ActionKind::Network, url.origin()), &b);
    if verdict.decision != Decision::Allow {
        return Err(Error::Refused(verdict.rationale));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_parse_or_refuse_and_never_carry_credentials() {
        let u = Url::parse("https://srv1328560.hstgr.cloud/mcp").unwrap();
        assert!(u.https && u.port == 443 && u.path == "/mcp");
        assert_eq!(u.origin(), "https://srv1328560.hstgr.cloud");
        assert_eq!(u.host_header(), "srv1328560.hstgr.cloud");

        let p = Url::parse("http://127.0.0.1:8181/mcp").unwrap();
        assert!(!p.https && p.port == 8181 && p.is_loopback());
        assert_eq!(p.origin(), "http://127.0.0.1:8181");

        assert!(Url::parse("srv/mcp").is_err());
        assert!(Url::parse("ftp://srv/mcp").is_err());
        assert!(Url::parse("https:///mcp").is_err());
        // A token belongs in a 0600 key file, never in a URL that lands in logs.
        assert!(Url::parse("https://user:tok@srv/mcp").is_err());
    }

    /// A shut gate is a refusal carrying the guard's own words — and it happens before any
    /// address is resolved, so a closed boundary cannot leak so much as a DNS query.
    #[test]
    fn a_shut_network_gate_refuses_before_the_socket() {
        let d = familiar_kernel::testing::temp_root("mcp_gate");
        // Written as a file rather than through a setter, because the kernel deliberately has
        // no way to widen a boundary from code (`narrow_gate` only closes). A test that wants
        // an open gate has to do what a human does.
        let write = |b: &boundary::Boundary| {
            std::fs::write(
                d.join(boundary::BOUNDARY_FILE),
                serde_json::to_string(b).unwrap(),
            )
            .unwrap()
        };
        write(&boundary::Boundary::closed());
        let u = Url::parse("https://srv1328560.hstgr.cloud/mcp").unwrap();
        match permitted(&d, &u) {
            Err(Error::Refused(r)) => assert!(!r.is_empty(), "the refusal explains itself"),
            other => panic!("a closed boundary must refuse: {other:?}"),
        }

        let mut open = boundary::Boundary::closed();
        open.allow_network = true;
        write(&open);
        assert!(
            permitted(&d, &u).is_ok(),
            "an opened gate permits the reach"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
