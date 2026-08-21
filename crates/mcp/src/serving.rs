//! **Who may reach the familiar's own MCP server** — the inbound mirror of [`crate::declaration`].
//!
//! The outbound side asks *which partners may this familiar call, and which of their tools*.
//! This side asks the harder question: *may anyone outside this machine call US at all.*
//!
//! Ian's word, 2026-08-18: *"We should expose it and make it ready for Jeff's agent to reach
//! it."* This module is the shape that request takes without becoming an open door.
//!
//! Three rules, and each of them fails in the safe direction:
//!
//! 1. **A missing declaration means not exposed.** Same discipline as `boundary::load` falling
//!    back to `closed()`: the absence of a decision is never read as permission.
//! 2. **The exposed route has no loopback exemption at all.** This is the correction to a real
//!    bug, caught the first time a reverse proxy sat in front of this seam (2026-08-18): Caddy
//!    terminates TLS and forwards from `127.0.0.1`, so *every request from the internet arrived
//!    looking like a neighbour* and an earlier version of this module waved them all through
//!    without a token. A proxy is not a neighbour. The exemption is gone, and the console keeps
//!    its unauthenticated path by using a different route on the loopback-only listener — a
//!    door the proxy cannot reach by construction, rather than one it is trusted not to use.
//! 3. **Exposure without a key serves nobody.** A public surface with no key is not something
//!    this project ships. If `expose` is true and no key resolves, every request is refused —
//!    the misconfiguration closes the door rather than opening it.
//!
//! The token is compared in constant time. It is a small thing, but a comparison that returns
//! early on the first wrong byte tells an attacker how much of the key they guessed.

use std::io;
use std::path::Path;

use serde::Deserialize;

use crate::partner::{self, PartnerContext};

/// Where the human writes this decision, relative to the data dir.
pub const SERVING_FILE: &str = "mcp/serving.json";

/// The human's decision about the inbound seam.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, default)]
pub struct Serving {
    /// May anything outside this machine reach the MCP server? Absent is **false**.
    pub expose: bool,
    /// Path (relative to the data dir) of an env-format file holding the bearer token, and the
    /// key to read from it. Same convention as the outbound declaration and the LLM key.
    pub key_file: String,
    pub key_name: String,
    /// Free text for the human's own benefit; never read by any decision.
    pub note: String,
}

/// Read the decision. A missing file is "not exposed", which is the ordinary state and not an
/// error. A malformed file is also "not exposed": a declaration nobody can parse is not a
/// decision, and guessing at one is how a door gets left open.
pub fn load(dir: &Path) -> Serving {
    match std::fs::read(dir.join(SERVING_FILE)) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => Serving::default(),
    }
}

impl Serving {
    /// The configured token, if the key file resolves to one.
    pub fn token(&self, dir: &Path) -> io::Result<Option<String>> {
        if self.key_file.is_empty() || self.key_name.is_empty() {
            return Ok(None);
        }
        let raw = match std::fs::read_to_string(dir.join(&self.key_file)) {
            Ok(s) => s,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        for line in raw.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == self.key_name {
                    let v = v.trim().trim_matches('"').trim_matches('\'');
                    if !v.is_empty() {
                        return Ok(Some(v.to_string()));
                    }
                }
            }
        }
        Ok(None)
    }
}

/// Why a request was turned away, in words a caller can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Denied {
    /// The human has not opened this seam.
    NotExposed,
    /// Opened, but no key resolves — so nothing outside may be served.
    NoKeyConfigured,
    /// A key is required and the request did not carry the right one.
    BadToken,
}

/// What the bearer established. The historical door-wide key carries no principal and can
/// reach only rungs 1-2; a registered partner credential carries the context rung 3 requires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    Door,
    Partner(PartnerContext),
}

impl Denied {
    pub fn why(&self) -> &'static str {
        match self {
            Denied::NotExposed => {
                "this familiar's MCP server is not exposed beyond its own machine"
            }
            // Deliberately the same sentence as NotExposed on the wire: a caller learns the
            // door is shut, not why, because "exposed but misconfigured" is information about
            // the inside of a system a stranger has no claim on. The distinction is kept in
            // the type so the local operator's logs and tests can tell them apart.
            Denied::NoKeyConfigured => {
                "this familiar's MCP server is not exposed beyond its own machine"
            }
            Denied::BadToken => {
                "this familiar's MCP server needs the bearer token its human issued you"
            }
        }
    }
}

/// Compare two secrets without leaking how far the match got.
/// May this request be served on the **exposed** route?
///
/// `presented` is whatever arrived in `Authorization: Bearer …`, already stripped of its scheme.
///
/// Deliberately takes no peer address. The caller cannot pass "but this one is local" because
/// there is no such argument to pass — which is the fix for the proxy bug, expressed in the
/// type rather than in a comment asking the next reader to be careful. Local callers that
/// legitimately need no token use the loopback-only route instead.
pub fn admits(dir: &Path, presented: Option<&str>) -> Result<(), Denied> {
    admit(dir, presented).map(|_| ())
}

/// Admit and retain authenticated identity for the MCP server. Exposure remains the global
/// human-owned ceiling; a registered credential is an alternative bearer for that open door,
/// never a way to open it.
pub fn admit(dir: &Path, presented: Option<&str>) -> Result<Admission, Denied> {
    let serving = load(dir);
    if !serving.expose {
        return Err(Denied::NotExposed);
    }
    if let Some(context) = presented.and_then(|token| partner::authenticate(dir, token)) {
        return Ok(Admission::Partner(context));
    }
    let Ok(Some(expected)) = serving.token(dir) else {
        // Rule 3: exposure without a key serves nobody.
        return Err(Denied::NoKeyConfigured);
    };
    match presented {
        Some(t) if partner::same_secret(t, &expected) => Ok(Admission::Door),
        _ => Err(Denied::BadToken),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("serving_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(p.join("mcp")).unwrap();
        p
    }

    fn write(dir: &Path, json: &str) {
        std::fs::write(dir.join(SERVING_FILE), json).unwrap();
    }

    fn write_key(dir: &Path, token: &str) {
        std::fs::write(
            dir.join("mcp/inbound.env"),
            format!("# a comment\nFAMILIAR_MCP_TOKEN={token}\n"),
        )
        .unwrap();
    }

    const DECL: &str =
        r#"{"expose":true,"key_file":"mcp/inbound.env","key_name":"FAMILIAR_MCP_TOKEN"}"#;

    /// The ordinary state, and the one that must hold when nobody has decided anything.
    #[test]
    fn absence_of_a_decision_is_never_permission() {
        let d = tmp("absent");
        assert_eq!(admits(&d, None), Err(Denied::NotExposed));
        assert_eq!(admits(&d, Some("anything")), Err(Denied::NotExposed));
    }

    /// A declaration nobody can parse is not a decision.
    #[test]
    fn a_malformed_declaration_closes_the_door() {
        let d = tmp("malformed");
        write(&d, "{ this is not json");
        assert_eq!(admits(&d, Some("x")), Err(Denied::NotExposed));
    }

    /// **Exposure without a key serves nobody.** The misconfiguration must close the door, not
    /// open it — this is the failure that would otherwise put an unauthenticated write surface
    /// on the public internet.
    #[test]
    fn exposed_without_a_key_refuses_everyone_outside() {
        let d = tmp("nokey");
        write(&d, r#"{"expose":true}"#);
        assert_eq!(admits(&d, Some("x")), Err(Denied::NoKeyConfigured));
        assert_eq!(admits(&d, None), Err(Denied::NoKeyConfigured));
        // Declared with a key file that does not exist is the same situation.
        write(&d, DECL);
        assert_eq!(admits(&d, Some("x")), Err(Denied::NoKeyConfigured));
    }

    #[test]
    fn the_right_token_is_admitted_and_a_wrong_one_is_not() {
        let d = tmp("token");
        write(&d, DECL);
        write_key(&d, "ucfk_secret_value");
        assert_eq!(admits(&d, Some("ucfk_secret_value")), Ok(()));
        assert_eq!(admits(&d, Some("ucfk_secret_valu")), Err(Denied::BadToken));
        assert_eq!(admits(&d, Some("")), Err(Denied::BadToken));
        assert_eq!(admits(&d, None), Err(Denied::BadToken));
    }

    /// **The proxy regression.** A reverse proxy terminating TLS forwards from `127.0.0.1`, so
    /// every request off the internet arrives looking like a neighbour. An earlier version of
    /// this function took a `loopback` flag and returned `Ok` on it before looking at anything
    /// else — which meant putting Caddy in front turned the bearer gate off for the whole
    /// world, and did so silently. Caught live on 2026-08-18 by a no-token request that
    /// answered `HTTP 200`.
    ///
    /// The fix is structural: there is no argument by which a caller can claim to be local, so
    /// no caller can be waved through. This test pins the property by asserting that the ONLY
    /// thing that admits is the right token.
    #[test]
    fn a_proxy_is_not_a_neighbour() {
        let d = tmp("proxy");
        write(&d, DECL);
        write_key(&d, "the-real-token");
        // Every shape a proxied stranger can arrive in — none of them are admitted.
        for wrong in [
            None,
            Some(""),
            Some("the-real-toke"),
            Some("Bearer the-real-token"),
        ] {
            assert!(
                admits(&d, wrong).is_err(),
                "a request without the token must never be admitted, however local it looks"
            );
        }
        assert_eq!(admits(&d, Some("the-real-token")), Ok(()));
    }

    /// A stranger learns the door is shut, not what is wrong behind it.
    #[test]
    fn a_shut_door_does_not_describe_its_own_misconfiguration() {
        assert_eq!(Denied::NotExposed.why(), Denied::NoKeyConfigured.why());
        assert_ne!(Denied::BadToken.why(), Denied::NotExposed.why());
    }

    /// Constant-time comparison, exercised at the boundaries where a naive one differs.
    #[test]
    fn secrets_compare_without_early_exit() {
        assert!(partner::same_secret("abc", "abc"));
        assert!(!partner::same_secret("abc", "abd"));
        assert!(!partner::same_secret("abc", "abcd"));
        assert!(!partner::same_secret("", "a"));
        assert!(partner::same_secret("", ""));
    }

    #[test]
    fn a_registered_partner_credential_carries_identity_but_does_not_open_the_door() {
        let d = tmp("partner");
        std::fs::write(d.join("mcp/partner.env"), "PARTNER_TOKEN=partner-secret\n").unwrap();
        let principal =
            crate::partner::register(&d, "Workshop agent", "mcp/partner.env", "PARTNER_TOKEN")
                .unwrap();
        assert_eq!(
            admit(&d, Some("partner-secret")),
            Err(Denied::NotExposed),
            "a credential is identity, never exposure authority"
        );
        write(&d, r#"{"expose":true}"#);
        match admit(&d, Some("partner-secret")).unwrap() {
            Admission::Partner(context) => assert_eq!(context.principal, principal.id),
            Admission::Door => panic!("a partner credential must not lose its identity"),
        }
        assert_eq!(admit(&d, Some("wrong")), Err(Denied::NoKeyConfigured));
    }
}
