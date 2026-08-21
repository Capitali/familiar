//! **The familiar's own MCP server** — the other direction of ADR-0037's seam.
//!
//! The ADR names the server half *"a small MCP server on the familiar's side exposing two tools
//! (`purr.say`, `purr.utterances`) plus the pairing handshake"*. This module is **the pairing
//! handshake**, built first and alone, because the other two cannot ship yet: `purr.say` carries
//! game speech, and ADR-0037 §B makes the world partition (T-205) a precondition for any game
//! data reaching this system — *"the load-bearing safety decision and it is not optional."*
//! Shipping a speech tool before the partition is how a ship's stores and a real household end
//! up in one observation log.
//!
//! So what exists here is the door, and the door is the covenant.
//!
//! ## The shape
//!
//! JSON-RPC 2.0, MCP `2025-06-18`, the same revision our client speaks. Three tools, in two
//! tiers:
//!
//! - `familiar.constitution` — **always callable, even by a stranger.** You must be able to
//!   read what you are being asked to accept before accepting it; a covenant you had to agree
//!   to in order to read is not consent.
//! - `familiar.attest` — submit acceptance, in your own words.
//! - `familiar.hello` — who this familiar is. **Attested partners only.**
//!
//! An unattested caller does not see a rich menu it cannot use: `tools/list` shows it the two
//! covenant tools, because a list of doors you cannot open is noise. Calling anything else
//! returns the reason and the remedy in one sentence.
//!
//! ## What this module deliberately does not do
//!
//! It does not authenticate. An MCP client presents no key at this layer, so `partner` is a
//! label a human reads and never an identity a decision rests on — which is exactly why the
//! only thing it unlocks is *speech about ourselves*. Nothing here can act, spend, or change
//! the boundary, and the gate that governs those is untouched. When a tool that acts is ever
//! added, it answers to the boundary the same as every other outward act, and being attested
//! will not be sufficient for it.

use familiar_kernel::constitution;
use serde_json::{json, Map, Value};
use std::path::Path;

use crate::covenant;

/// The protocol revision this server speaks — the same one [`crate::session`] speaks as a
/// client, so both halves of the seam agree by construction rather than by comment.
pub const PROTOCOL_VERSION: &str = crate::session::PROTOCOL_VERSION;

/// What this familiar calls itself on the wire.
pub const SERVER_NAME: &str = "familiar";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// JSON-RPC error codes, as the spec numbers them.
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// Handle one JSON-RPC request and return the response, or `None` for a notification (which by
/// the spec gets no reply at all).
///
/// Pure but for the covenant ledger it reads and writes: no socket, no clock. `now` is passed
/// in so a test can pin the timestamp, the same way the rest of this workspace does it.
pub fn handle(dir: &Path, request: &Value, now: i64) -> Option<Value> {
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));

    // A notification carries no id and is answered with silence, not with an error.
    let id = id?;

    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION },
            // Said at the door rather than discovered by trial: a client that reads this knows
            // what to do first without calling anything and being refused.
            "instructions": "This familiar is bound by three laws. Call \
                             `familiar.constitution` to read them, then `familiar.attest` to \
                             accept them in your own words. Until you do, nothing else here \
                             is callable."
        })),
        "tools/list" => Ok(json!({ "tools": tools_for(dir, &params) })),
        "tools/call" => call(dir, &params, now),
        // `ping` is in the protocol and costs nothing to honour.
        "ping" => Ok(json!({})),
        other => Err((
            METHOD_NOT_FOUND,
            format!("this server does not implement `{other}`"),
        )),
    };

    Some(match result {
        Ok(value) => json!({ "jsonrpc": "2.0", "id": id, "result": value }),
        Err((code, message)) => json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": code, "message": message }
        }),
    })
}

/// The `partner` a request names, if any. Carried in the tool arguments rather than a header
/// because MCP has no notion of a caller identity and inventing one would be a private
/// protocol — the thing ADR-0037 chose MCP to avoid.
fn partner_of(params: &Value) -> String {
    params
        .get("arguments")
        .and_then(|a| a.get("partner"))
        .or_else(|| params.get("partner"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn tool(name: &str, description: &str, schema: Value, read_only: bool) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": schema,
        // We annotate honestly, including on tools that write. Our own client treats a hint as
        // a claim and never as permission (see `session::Claimed`); we send them anyway,
        // because being legible to a partner's human is the whole point of the field.
        "annotations": { "readOnlyHint": read_only }
    })
}

fn constitution_tool() -> Value {
    tool(
        "familiar.constitution",
        "The three laws this familiar is bound by, quoted verbatim from its founding \
         document, with the reconciliation line and each law's inversion guard. Callable by \
         anyone, always: you must be able to read what you are being asked to accept.",
        json!({ "type": "object", "properties": {} }),
        true,
    )
}

fn attest_tool() -> Value {
    tool(
        "familiar.attest",
        "Accept the three laws, in your own words. Records who accepted, what they said, and \
         which version of the laws they were shown. Re-accepting supersedes your previous \
         statement. This unlocks conversation, not authority — every act is still weighed \
         against the human's capability boundary.",
        json!({
            "type": "object",
            "properties": {
                "partner": { "type": "string", "description": "what you call yourself" },
                "statement": {
                    "type": "string",
                    "description": "your acceptance, phrased by you — an empty one is refused"
                }
            },
            "required": ["partner", "statement"]
        }),
        false,
    )
}

fn hello_tool() -> Value {
    tool(
        "familiar.hello",
        "Who this familiar is and what it is currently able to do. Attested partners only.",
        json!({
            "type": "object",
            "properties": { "partner": { "type": "string" } },
            "required": ["partner"]
        }),
        true,
    )
}

/// What this caller can see. A stranger is shown the two doors it can actually open — a menu
/// of refusals teaches nothing and reads as a system pretending to offer more than it will.
fn tools_for(dir: &Path, params: &Value) -> Vec<Value> {
    let mut out = vec![constitution_tool(), attest_tool()];
    if covenant::attested(dir, &partner_of(params)) {
        out.push(hello_tool());
        out.push(discover_classes_tool());
    }
    out
}

/// Rung 2 of the ADR-0044 ladder: the class catalog. Attested partners only; classes are
/// affordances, never authority — nothing listed is invocable without a human's grant.
fn discover_classes_tool() -> Value {
    tool(
        "familiar.discover_classes",
        "The capability classes available here, as generic affordances: what KINDS of          thing this familiar could be granted to observe or do — never instances, names,          counts, or authority. Attested partners only. A grant (observe/invoke) is a          deliberate human act per capability, per partner, per bounds; discovery is not a          request for one.",
        json!({
            "type": "object",
            "properties": { "partner": { "type": "string" } },
            "required": ["partner"]
        }),
        true,
    )
}

/// The content-block shape an MCP tool answers in.
fn content(text: String) -> Value {
    json!({ "content": [{ "type": "text", "text": text }] })
}

fn tool_error(text: String) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": true })
}

fn call(dir: &Path, params: &Value, now: i64) -> Result<Value, (i64, String)> {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let partner = partner_of(params);

    match name {
        "familiar.constitution" => Ok(content(partner_constitution())),

        "familiar.attest" => {
            let statement = args
                .get("statement")
                .and_then(Value::as_str)
                .unwrap_or_default();
            match covenant::accept(dir, &partner, statement, now) {
                Ok(c) => Ok(content(format!(
                    "Recorded. {} accepted the three laws (version {}) at {}, saying: \"{}\"\n\n\
                     What that unlocks: conversation. What it does not: authority. Every act \
                     this familiar takes is still weighed against a capability boundary its \
                     human owns, and being attested is never sufficient for one.",
                    c.partner, c.laws_version, c.ts, c.statement
                ))),
                Err(r) => Ok(tool_error(format!("not recorded — {}", r.why()))),
            }
        }

        "familiar.discover_classes" => {
            if !covenant::attested(dir, &partner) {
                return Ok(tool_error(unattested(&partner)));
            }
            let avail = crate::offering::available(dir);
            Ok(content(
                serde_json::to_string_pretty(&crate::offering::catalog_json(&avail))
                    .unwrap_or_else(|_| "{}".into()),
            ))
        }

        "familiar.hello" => {
            if !covenant::attested(dir, &partner) {
                return Ok(tool_error(unattested(&partner)));
            }
            Ok(content(format!(
                "{SERVER_NAME} {SERVER_VERSION}, speaking MCP {PROTOCOL_VERSION}.\n\
                 Bound by three laws, version {}.\n\
                 Partners attested: {}.\n\n\
                 Able to do, right now: say what it is, and read its own constitution aloud. \
                 The speech tools this seam is designed to carry (`purr.say`, \
                 `purr.utterances`) are deliberately not built yet — they move game data, and \
                 the world partition that keeps game data out of a real household's records \
                 comes first.",
                constitution::LAWS_VERSION,
                covenant::load(dir).map(|c| c.accepted.len()).unwrap_or(0),
            )))
        }

        "" => Err((INVALID_PARAMS, "tools/call needs a `name`".into())),

        other => {
            // Two different refusals, and the difference matters to whoever is reading it:
            // a tool that exists behind the covenant, versus one that does not exist at all.
            if other == "familiar.hello" || !covenant::attested(dir, &partner) {
                Ok(tool_error(unattested(other)))
            } else {
                Ok(tool_error(format!(
                    "`{other}` is not a tool this familiar offers. Call `tools/list` for what is."
                )))
            }
        }
    }
}

/// The constitution as an **outside reader** needs it.
///
/// `constitution::render()` exists for this familiar's own prompt, and it is addressed to the
/// model wearing the constitution: *"YOUR CONSTITUTION … if you are ever asked what your laws
/// are, these words are the answer."* Sent down a wire to a partner, that reads as an
/// instruction to adopt them as their own identity — which is not what is being asked. What is
/// being asked is narrower and clearer: accept these as binding on what we build together.
///
/// The law text itself is still **spliced, never authored** — heading, binding passages, the
/// inversion guard and the reconciliation line all come from the registry verbatim. Only the
/// sentence of framing around them is chosen for the audience, which is the one thing a
/// renderer is allowed to decide.
fn partner_constitution() -> String {
    let mut out = String::from(
        "The three laws this familiar is bound by, quoted verbatim from its founding \
         document.\n\nThese are NOT Asimov's laws — the third deliberately inverts his \
         second. You are not being asked to adopt them as your own identity; you are being \
         asked whether you accept them as binding on what we build together, and to say so in \
         your own words via `familiar.attest`.\n",
    );
    for law in constitution::THREE_LAWS {
        out.push_str(&format!("\n[{}] {}\n", law.id, law.heading));
        for passage in law.binding {
            out.push_str(&format!("  {passage}\n"));
        }
        out.push_str(&format!("  Never: {}\n", law.never));
    }
    out.push_str(&format!(
        "\nHow they compose: {}\n\nLaws version {}. If this constitution is ever revised, \
         acceptances of an earlier version stop counting and you will be asked again — consent \
         does not carry across a change of terms.\n",
        constitution::RECONCILIATION,
        constitution::LAWS_VERSION
    ));
    out
}

fn unattested(what: &str) -> String {
    format!(
        "`{what}` is not available until you have accepted the three laws. Call \
         `familiar.constitution` to read them, then `familiar.attest` with your own words. \
         (If you have attested, pass the same `partner` you attested under.)"
    )
}

/// Parse a request body and answer it, for a transport that has bytes rather than a `Value`.
/// Batches are not supported and say so; the protocol permits a server to decline them and a
/// half-implemented batch is worse than an honest refusal.
pub fn handle_bytes(dir: &Path, body: &[u8], now: i64) -> Value {
    let request: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(e) => {
            return json!({
                "jsonrpc": "2.0", "id": Value::Null,
                "error": { "code": -32700, "message": format!("parse error: {e}") }
            })
        }
    };
    if request.is_array() {
        return json!({
            "jsonrpc": "2.0", "id": Value::Null,
            "error": { "code": -32600, "message": "batched requests are not supported; send one at a time" }
        });
    }
    handle(dir, &request, now).unwrap_or_else(|| {
        // A notification: answered with an empty object, which the transport turns into a 202.
        Value::Object(Map::new())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("mcpsrv_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn req(method: &str, params: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params })
    }

    fn text_of(response: &Value) -> String {
        response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }

    fn is_error(response: &Value) -> bool {
        response["result"]["isError"].as_bool() == Some(true)
    }

    fn names(response: &Value) -> Vec<String> {
        response["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn initialize_names_the_protocol_and_says_what_to_do_first() {
        let d = tmp("init");
        let r = handle(&d, &req("initialize", json!({})), 1).unwrap();
        assert_eq!(r["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(r["result"]["serverInfo"]["name"], SERVER_NAME);
        let says = r["result"]["instructions"].as_str().unwrap();
        assert!(says.contains("familiar.constitution") && says.contains("familiar.attest"));
    }

    /// A notification has no id and gets no reply — answering one is a protocol violation.
    #[test]
    fn a_notification_is_answered_with_silence() {
        let d = tmp("notify");
        let n = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        assert!(handle(&d, &n, 1).is_none());
    }

    /// **The gate.** A stranger sees the two doors it can open, and no others.
    #[test]
    fn a_stranger_is_shown_only_the_covenant() {
        let d = tmp("stranger");
        let r = handle(&d, &req("tools/list", json!({})), 1).unwrap();
        assert_eq!(
            names(&r),
            vec!["familiar.constitution", "familiar.attest"],
            "a menu of doors you cannot open is noise"
        );
    }

    /// You must be able to read what you are being asked to accept, before accepting it.
    #[test]
    fn the_constitution_is_readable_without_accepting_it_first() {
        let d = tmp("read_first");
        let r = handle(
            &d,
            &req("tools/call", json!({ "name": "familiar.constitution" })),
            1,
        )
        .unwrap();
        assert!(!is_error(&r));
        let laws = text_of(&r);
        // The canonical text, spliced from the registry — not a paraphrase this module wrote.
        for law in constitution::THREE_LAWS {
            for passage in law.binding {
                assert!(laws.contains(passage), "law text must be verbatim");
            }
            assert!(
                laws.contains(law.never),
                "the inversion guard travels with the law"
            );
        }
        assert!(laws.contains(constitution::RECONCILIATION));

        // Framed for a READER, not for this familiar's own model. The prompt rendering tells
        // whoever holds it "if you are ever asked what your laws are, these words are the
        // answer" — down a wire that reads as "adopt these as your identity", which is not
        // the ask.
        assert!(
            !laws.contains("YOUR CONSTITUTION"),
            "the partner-facing rendering must not be the model's own prompt text"
        );
        assert!(laws.contains("binding on what we build together"));
        assert!(
            laws.contains("consent does not carry across a change of terms"),
            "a partner must be told their acceptance expires if the laws are revised"
        );
    }

    #[test]
    fn accepting_unlocks_the_attested_tier_and_nothing_more() {
        let d = tmp("accept_flow");
        let before = handle(
            &d,
            &req(
                "tools/call",
                json!({
                    "name": "familiar.hello",
                    "arguments": { "partner": "ucf-market" }
                }),
            ),
            1,
        )
        .unwrap();
        assert!(is_error(&before));
        assert!(text_of(&before).contains("not available until you have accepted"));

        let accept = handle(
            &d,
            &req(
                "tools/call",
                json!({
                    "name": "familiar.attest",
                    "arguments": { "partner": "ucf-market", "statement": "We accept them." }
                }),
            ),
            5_000,
        )
        .unwrap();
        assert!(!is_error(&accept));
        // The receipt must say what it does NOT confer, or "attested" starts to feel like power.
        assert!(text_of(&accept).contains("What it does not: authority"));

        let after = handle(
            &d,
            &req(
                "tools/call",
                json!({
                    "name": "familiar.hello",
                    "arguments": { "partner": "ucf-market" }
                }),
            ),
            6_000,
        )
        .unwrap();
        assert!(!is_error(&after));
        assert!(text_of(&after).contains("Bound by three laws"));

        // And the menu grew, for that partner only.
        let listed = handle(
            &d,
            &req("tools/list", json!({ "partner": "ucf-market" })),
            1,
        )
        .unwrap();
        assert!(names(&listed).contains(&"familiar.hello".to_string()));
        let stranger = handle(&d, &req("tools/list", json!({ "partner": "someone" })), 1).unwrap();
        assert!(!names(&stranger).contains(&"familiar.hello".to_string()));
    }

    /// **Rung 2 (ADR-0044): discovery is attested-only, offers classes, and leaks nothing.**
    /// A stranger cannot list or call it; an attested partner gets the catalog — which, on
    /// a household with no shaped surfaces, is honestly empty rather than padded.
    #[test]
    fn discovery_is_attested_only_and_offers_affordances_never_authority() {
        let d = tmp("discover");
        // A stranger: not listed, and a call refuses on the covenant.
        let stranger_menu = handle(&d, &req("tools/list", json!({ "partner": "s" })), 1).unwrap();
        assert!(!names(&stranger_menu).contains(&"familiar.discover_classes".to_string()));
        let refused = handle(
            &d,
            &req(
                "tools/call",
                json!({ "name": "familiar.discover_classes", "arguments": { "partner": "s" } }),
            ),
            1,
        )
        .unwrap();
        assert!(is_error(&refused));

        covenant::accept(&d, "jeffs-agent", "we accept the three laws", 1).unwrap();
        let menu = handle(
            &d,
            &req("tools/list", json!({ "partner": "jeffs-agent" })),
            2,
        )
        .unwrap();
        assert!(names(&menu).contains(&"familiar.discover_classes".to_string()));
        let cat = handle(
            &d,
            &req(
                "tools/call",
                json!({ "name": "familiar.discover_classes",
                        "arguments": { "partner": "jeffs-agent" } }),
            ),
            3,
        )
        .unwrap();
        assert!(!is_error(&cat));
        let body = text_of(&cat);
        assert!(
            body.contains(r#""classes": []"#),
            "no shaped surface, an empty catalog: {body}"
        );
        assert!(
            body.contains("not authority"),
            "the catalog says what discovery is not"
        );
    }

    /// An empty acceptance is refused, and the refusal says how to fix it.
    #[test]
    fn an_empty_acceptance_does_not_unlock_anything() {
        let d = tmp("empty_accept");
        let r = handle(
            &d,
            &req(
                "tools/call",
                json!({
                    "name": "familiar.attest",
                    "arguments": { "partner": "p", "statement": "  " }
                }),
            ),
            1,
        )
        .unwrap();
        assert!(is_error(&r));
        assert!(text_of(&r).contains("own words"));
        assert!(!covenant::attested(&d, "p"));
    }

    /// A tool that does not exist and a tool behind the covenant are different facts, and a
    /// reader deserves to be told which one they hit.
    #[test]
    fn an_unknown_tool_and_a_gated_tool_read_differently() {
        let d = tmp("unknown");
        covenant::accept(&d, "p", "yes", 1).unwrap();
        let r = handle(
            &d,
            &req(
                "tools/call",
                json!({
                    "name": "familiar.spend_everything",
                    "arguments": { "partner": "p" }
                }),
            ),
            1,
        )
        .unwrap();
        assert!(is_error(&r));
        assert!(text_of(&r).contains("is not a tool this familiar offers"));
    }

    #[test]
    fn an_unknown_method_is_refused_by_name() {
        let d = tmp("method");
        let r = handle(&d, &req("resources/list", json!({})), 1).unwrap();
        assert_eq!(r["error"]["code"], METHOD_NOT_FOUND);
        assert!(r["error"]["message"]
            .as_str()
            .unwrap()
            .contains("resources/list"));
    }

    #[test]
    fn malformed_bytes_and_batches_are_refused_rather_than_guessed() {
        let d = tmp("bytes");
        let bad = handle_bytes(&d, b"{ not json", 1);
        assert_eq!(bad["error"]["code"], -32700);
        let batch = handle_bytes(
            &d,
            b"[{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}]",
            1,
        );
        assert!(batch["error"]["message"]
            .as_str()
            .unwrap()
            .contains("one at a time"));
    }

    /// We annotate our own tools honestly — including marking the one that writes.
    #[test]
    fn our_own_tools_carry_truthful_annotations() {
        let d = tmp("annot");
        let r = handle(&d, &req("tools/list", json!({})), 1).unwrap();
        let tools = r["result"]["tools"].as_array().unwrap();
        let by = |n: &str| {
            tools
                .iter()
                .find(|t| t["name"] == n)
                .unwrap()
                .get("annotations")
                .unwrap()
                .clone()
        };
        assert_eq!(by("familiar.constitution")["readOnlyHint"], json!(true));
        assert_eq!(
            by("familiar.attest")["readOnlyHint"],
            json!(false),
            "a tool that writes must not claim to be read-only, even our own"
        );
    }
}
