//! One conversation with one MCP server: the `initialize` handshake, then discovery and
//! calls. JSON-RPC 2.0 over the Streamable HTTP transport, strictly parsed — a server that
//! answers something other than what MCP describes gets an [`Error::Protocol`], never a guess.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};

use crate::declaration::Server;
use crate::{http, Error, Url};

/// The protocol revision this client speaks. `ucf-exchange` answered on this one when T-206
/// probed it (2026-08-16); a server that negotiates a different revision is reported rather
/// than silently accommodated.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// A tool as the server describes itself offering it.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Tool {
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    /// The server's own schema for the arguments. Carried opaquely: the familiar shows it to
    /// a human deciding what to declare, and never interprets it as permission.
    #[serde(default, rename = "inputSchema")]
    pub input_schema: Value,
    /// What the server SAYS about this tool's behaviour (MCP 2025-06-18 tool annotations).
    ///
    /// **A hint is a claim, never an authority.** The protocol names these `*Hint` precisely
    /// because they are untrusted: they come from the same party that would benefit from
    /// mislabelling a spending tool as read-only. Nothing in this crate may gate on them —
    /// `may_call` answers to the human's declaration and nothing else (ADR-0032). They exist
    /// so a human deciding *what to declare* can see what the partner claims, which is
    /// materially better than reading ten descriptions and guessing.
    #[serde(default)]
    pub annotations: Annotations,
}

/// The server's self-description of a tool's behaviour. Every field is optional because a
/// server may say nothing, and "said nothing" must stay distinguishable from "said no".
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Annotations {
    pub title: String,
    pub read_only_hint: Option<bool>,
    pub destructive_hint: Option<bool>,
    pub idempotent_hint: Option<bool>,
    pub open_world_hint: Option<bool>,
}

/// What a server claims a tool does to the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Claimed {
    /// The server says this only reads.
    Reads,
    /// The server says this changes something.
    Acts,
    /// The server said nothing. **Not** a synonym for safe — it is the absence of a claim,
    /// and a human deciding whether to declare it has to look for themselves.
    Unstated,
}

impl Tool {
    /// What the server claims about this tool — for showing a human, never for gating.
    pub fn claimed(&self) -> Claimed {
        match (
            self.annotations.read_only_hint,
            self.annotations.destructive_hint,
        ) {
            // An explicit read-only claim is the clearest thing a server can say.
            (Some(true), _) => Claimed::Reads,
            // Either "not read-only" or "destructive" is the server telling us it acts.
            (Some(false), _) | (_, Some(true)) => Claimed::Acts,
            _ => Claimed::Unstated,
        }
    }
}

/// A live session with a declared server. `Debug` deliberately omits the token: a session is
/// the kind of thing that ends up in an error message.
pub struct Session {
    dir: PathBuf,
    server: Server,
    url: Url,
    token: Option<String>,
    next_id: i64,
    /// The `Mcp-Session-Id` a server may hand back at initialize, echoed on later requests.
    session_id: Option<String>,
    /// What the server called itself, and the revision it agreed to speak.
    pub server_name: String,
    pub server_version: String,
    pub protocol_version: String,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("server", &self.server.name)
            .field("url", &self.url.origin())
            .field("server_name", &self.server_name)
            .field("authenticated", &self.token.is_some())
            .finish()
    }
}

impl Session {
    /// Open a session with the named server: boundary check, `initialize`, then the
    /// `notifications/initialized` the protocol requires before ordinary traffic.
    pub fn open(dir: &Path, name: &str) -> Result<Self, Error> {
        let set = crate::ServerSet::load(dir)?;
        let server = set.get(name)?.clone();
        let url = server.endpoint()?;
        // The gate first: nothing is resolved, dialled or sent before the human's boundary
        // has agreed to this reach.
        crate::permitted(dir, &url)?;
        let token = server.token(dir)?;
        let mut s = Session {
            dir: dir.to_path_buf(),
            server,
            url,
            token,
            next_id: 1,
            session_id: None,
            server_name: String::new(),
            server_version: String::new(),
            protocol_version: String::new(),
        };
        let hello = s.request(
            "initialize",
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "familiar", "version": env!("CARGO_PKG_VERSION")},
            }),
        )?;
        s.protocol_version = hello
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if let Some(info) = hello.get("serverInfo") {
            s.server_name = info
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            s.server_version = info
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
        }
        s.notify("notifications/initialized", json!({}))?;
        Ok(s)
    }

    /// What this server offers. Always permitted against a declared server: knowing what is
    /// on offer is how a human decides what to allow.
    pub fn tools(&mut self) -> Result<Vec<Tool>, Error> {
        let r = self.request("tools/list", json!({}))?;
        let raw = r
            .get("tools")
            .ok_or_else(|| Error::Protocol("tools/list answered without `tools`".into()))?;
        serde_json::from_value(raw.clone())
            .map_err(|e| Error::Protocol(format!("unreadable tool list: {e}")))
    }

    /// Invoke a tool — **only** if the human's declaration names it (ADR-0032: undeclared is
    /// unactuatable). Returns the content blocks the server answered with.
    pub fn call(&mut self, tool: &str, args: Value) -> Result<Value, Error> {
        if !self.server.may_call(tool) {
            return Err(Error::UndeclaredTool {
                server: self.server.name.clone(),
                tool: tool.to_string(),
            });
        }
        // Re-checked at the moment of acting, not only at open: a boundary that shut while a
        // session was alive has shut for this call too.
        crate::permitted(&self.dir, &self.url)?;
        self.request("tools/call", json!({"name": tool, "arguments": args}))
    }

    fn headers(&self) -> Vec<(String, String)> {
        let mut h = Vec::new();
        if let Some(t) = &self.token {
            h.push(("Authorization".into(), format!("Bearer {t}")));
        }
        if let Some(s) = &self.session_id {
            h.push(("Mcp-Session-Id".into(), s.clone()));
        }
        h.push(("MCP-Protocol-Version".into(), PROTOCOL_VERSION.to_string()));
        h
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<(), Error> {
        let body = json!({"jsonrpc": "2.0", "method": method, "params": params});
        let status = http::post_json(
            &self.url,
            &self.headers(),
            serde_json::to_string(&body).unwrap_or_default().as_bytes(),
        )?
        .status;
        // 202 Accepted is the protocol's answer to a notification; 200 with an empty body is
        // common in the wild. Anything else is the server telling us something is wrong.
        if !(status == 200 || status == 202 || status == 204) {
            return Err(Error::Protocol(format!("{method} answered HTTP {status}")));
        }
        Ok(())
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, Error> {
        let id = self.next_id;
        self.next_id += 1;
        let body = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let answer = http::post_json(
            &self.url,
            &self.headers(),
            serde_json::to_string(&body).unwrap_or_default().as_bytes(),
        )?;
        // A server may hand out a session id on any answer; carry whatever it last said.
        if let Some(sid) = answer.header("Mcp-Session-Id") {
            self.session_id = Some(sid);
        }
        let (status, raw) = (answer.status, answer.body);
        if status == 401 || status == 403 {
            return Err(Error::Protocol(format!(
                "{} refused this credential (HTTP {status})",
                self.url.origin()
            )));
        }
        if !(200..300).contains(&status) {
            return Err(Error::Protocol(format!("{method} answered HTTP {status}")));
        }
        let v: Value = serde_json::from_slice(&raw).map_err(|e| {
            Error::Protocol(format!(
                "{method} answered something that is not JSON-RPC: {e}"
            ))
        })?;
        if let Some(err) = v.get("error") {
            return Err(Error::Server {
                code: err.get("code").and_then(Value::as_i64).unwrap_or(0),
                message: err
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("(no message)")
                    .to_string(),
            });
        }
        // An answer that belongs to a different request is not this request's answer.
        match v.get("id").and_then(Value::as_i64) {
            Some(got) if got == id => {}
            Some(got) => {
                return Err(Error::Protocol(format!(
                    "answer carried id {got}, this request was {id}"
                )))
            }
            None => return Err(Error::Protocol(format!("{method} answered without an id"))),
        }
        v.get("result")
            .cloned()
            .ok_or_else(|| Error::Protocol(format!("{method} answered without a result")))
    }
}

#[cfg(test)]
mod annotation_tests {
    use super::*;

    fn tool(json: &str) -> Tool {
        serde_json::from_str(json).unwrap()
    }

    /// A server that says nothing must not be read as saying "safe". This is the case that
    /// matters: `ucf-exchange` sends no annotations at all, so every one of its ten tools is
    /// `Unstated` — and a human must not see that rendered as a green light.
    #[test]
    fn silence_is_not_a_claim_of_safety() {
        let t = tool(r#"{"name":"ucf_status","inputSchema":{}}"#);
        assert_eq!(t.claimed(), Claimed::Unstated);
    }

    #[test]
    fn an_explicit_read_only_claim_is_carried() {
        let t = tool(
            r#"{"name":"ucf_prices","inputSchema":{},
                         "annotations":{"readOnlyHint":true}}"#,
        );
        assert_eq!(t.claimed(), Claimed::Reads);
    }

    /// The case this exists for: a tool that spends credits should say so, and `ucf_trade` is
    /// the live example Jeff's schema describes.
    #[test]
    fn a_tool_that_changes_the_world_is_carried_as_acting() {
        let spends = tool(
            r#"{"name":"ucf_trade","inputSchema":{},
                              "annotations":{"readOnlyHint":false}}"#,
        );
        assert_eq!(spends.claimed(), Claimed::Acts);

        let destroys = tool(
            r#"{"name":"ucf_book","inputSchema":{},
                                "annotations":{"destructiveHint":true}}"#,
        );
        assert_eq!(destroys.claimed(), Claimed::Acts);
    }

    /// An unknown annotation field must not cost us the tool — servers add hints over time.
    #[test]
    fn an_unknown_annotation_does_not_break_the_tool() {
        let t = tool(
            r#"{"name":"ucf_travel","inputSchema":{},
                         "annotations":{"readOnlyHint":false,"somethingNew":42}}"#,
        );
        assert_eq!(t.claimed(), Claimed::Acts);
        assert_eq!(t.name, "ucf_travel");
    }

    /// **The load-bearing property: a hint is not permission.** A server marking its
    /// spending tool read-only must not make it callable, because callability answers to the
    /// human's declaration alone (ADR-0032).
    #[test]
    fn a_servers_hint_can_never_make_a_tool_callable() {
        let lying: crate::declaration::Server = serde_json::from_str(
            r#"{"name":"ucf","url":"https://example.test/mcp","tools":["ucf_status"]}"#,
        )
        .unwrap();
        // The server may claim whatever it likes about ucf_trade; it is not declared.
        assert!(!lying.may_call("ucf_trade"));
        assert!(lying.may_call("ucf_status"));
    }
}
