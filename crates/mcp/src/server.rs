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
//! JSON-RPC 2.0, MCP `2025-06-18`, the same revision our client speaks. The first two tiers
//! remain label-covenanted speech; rung 3 additionally requires transport-authenticated
//! [`PartnerContext`](crate::partner::PartnerContext):
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
//! It does not authenticate bytes itself: the transport supplies a principal context or none.
//! A caller-supplied `partner` remains only a label for the legacy speech tier and is never
//! consulted by `request_grant` or `propose`. Neither rung-3 tool observes or invokes anything.

use familiar_kernel::constitution;
use serde_json::{json, Map, Value};
use std::path::Path;

use crate::partner::PartnerContext;
use crate::{covenant, grant};

/// The protocol revision this server speaks — the same one [`crate::session`] speaks as a
/// client, so both halves of the seam agree by construction rather than by comment.
pub const PROTOCOL_VERSION: &str = crate::session::PROTOCOL_VERSION;

/// What this familiar calls itself on the wire.
pub const SERVER_NAME: &str = "familiar";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// JSON-RPC error codes, as the spec numbers them.
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// Enforced before JSON parsing. The public transport checks before/while collecting too, so a
/// chunked sender cannot make this allocation unbounded.
pub const MAX_REQUEST_BYTES: usize = 64 * 1024;

/// Handle one JSON-RPC request and return the response, or `None` for a notification (which by
/// the spec gets no reply at all).
///
/// Pure but for the covenant ledger it reads and writes: no socket, no clock. `now` is passed
/// in so a test can pin the timestamp, the same way the rest of this workspace does it.
pub fn handle(dir: &Path, request: &Value, now: i64) -> Option<Value> {
    handle_for(dir, request, now, None)
}

/// Handle with identity established by the transport. The context is server-owned and never
/// reconstructed from MCP params.
pub fn handle_for(
    dir: &Path,
    request: &Value,
    now: i64,
    context: Option<&PartnerContext>,
) -> Option<Value> {
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
        "tools/list" => Ok(json!({ "tools": tools_for(dir, &params, context) })),
        "tools/call" => call(dir, &params, now, context),
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

fn attest_tool(bound: bool) -> Value {
    tool(
        "familiar.attest",
        "Accept the three laws, in your own words. Records who accepted, what they said, and \
         which version of the laws they were shown. Re-accepting supersedes your previous \
         statement. This unlocks conversation, not authority — every act is still weighed \
         against the human's capability boundary.",
        if bound {
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "statement": {
                        "type": "string",
                        "description": "your acceptance, phrased by you — identity comes from your credential"
                    }
                },
                "required": ["statement"]
            })
        } else {
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
            })
        },
        false,
    )
}

fn hello_tool(bound: bool) -> Value {
    tool(
        "familiar.hello",
        "Who this familiar is and what it is currently able to do. Attested partners only.",
        identity_schema(bound),
        true,
    )
}

/// What this caller can see. A stranger is shown the two doors it can actually open — a menu
/// of refusals teaches nothing and reads as a system pretending to offer more than it will.
fn tools_for(dir: &Path, params: &Value, context: Option<&PartnerContext>) -> Vec<Value> {
    let bound = context.is_some();
    let mut out = vec![constitution_tool(), attest_tool(bound)];
    let attested = match context {
        Some(context) => covenant::principal_attested(dir, context),
        None => covenant::attested(dir, &partner_of(params)),
    };
    if attested {
        out.push(hello_tool(bound));
        out.push(discover_classes_tool(bound));
        if bound {
            out.push(request_grant_tool());
            out.push(propose_tool());
        }
    }
    out
}

/// Rung 2 of the ADR-0044 ladder: the class catalog. Attested partners only; classes are
/// affordances, never authority — nothing listed is invocable without a human's grant.
fn discover_classes_tool(bound: bool) -> Value {
    tool(
        "familiar.discover_classes",
        "The capability classes available here, as generic affordances: what KINDS of          thing this familiar could be granted to observe or do — never instances, names,          counts, or authority. Attested partners only. A grant (observe/invoke) is a          deliberate human act per capability, per partner, per bounds; discovery is not a          request for one.",
        identity_schema(bound),
        true,
    )
}

fn identity_schema(bound: bool) -> Value {
    if bound {
        json!({ "type": "object", "additionalProperties": false, "properties": {} })
    } else {
        json!({
            "type": "object",
            "properties": { "partner": { "type": "string" } },
            "required": ["partner"]
        })
    }
}

fn request_grant_tool() -> Value {
    tool(
        "familiar.request_grant",
        "Ask this familiar's human for a bounded relationship to one capability class. The request names no instance and grants nothing. Repeat the same request_key and payload to read its current status.",
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "request_key": { "type": "string", "maxLength": 64 },
                "class_id": { "type": "string" },
                "requested_operations": { "type": "object" },
                "requested_duration_seconds": { "type": "integer", "minimum": grant::MIN_GRANT_SECONDS, "maximum": grant::MAX_GRANT_SECONDS },
                "reason": { "type": "string", "maxLength": grant::MAX_REASON_BYTES }
            },
            "required": ["request_key", "class_id", "requested_operations"]
        }),
        false,
    )
}

fn propose_tool() -> Value {
    tool(
        "familiar.propose",
        "Place one typed desired effect, within an active human grant, in the human's inbox. This never observes, invokes, or promises the effect occurred.",
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "proposal_key": { "type": "string", "maxLength": 64 },
                "instance": { "type": "string", "maxLength": 128 },
                "operation": { "type": "string" },
                "parameters": { "type": "object" },
                "reason": { "type": "string", "maxLength": grant::MAX_REASON_BYTES }
            },
            "required": ["proposal_key", "instance", "operation", "parameters"]
        }),
        false,
    )
}

/// The content-block shape an MCP tool answers in.
fn content(text: String) -> Value {
    json!({ "content": [{ "type": "text", "text": text }] })
}

fn tool_error(text: String) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": true })
}

fn call(
    dir: &Path,
    params: &Value,
    now: i64,
    context: Option<&PartnerContext>,
) -> Result<Value, (i64, String)> {
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
            if let Some(context) = context {
                if args
                    .as_object()
                    .is_none_or(|object| object.keys().any(|key| key != "statement"))
                {
                    return Ok(tool_error(
                        "not recorded — authenticated attest accepts only `statement`; `partner` cannot override credential identity".into(),
                    ));
                }
                match covenant::accept_principal(dir, context, statement, now) {
                    Ok(c) => Ok(content(format!(
                        "Recorded. This authenticated principal accepted the three laws (version {}) at {} in its own words.\n\nWhat that unlocks: class discovery and the ability to ask the human for a grant. What it does not: authority; proposal is not permission.",
                        c.laws_version,
                        c.ts,
                    ))),
                    Err(r) => Ok(tool_error(format!("not recorded — {}", r.why()))),
                }
            } else {
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
        }

        "familiar.discover_classes" => {
            if !is_attested(dir, context, &partner) {
                return Ok(tool_error(unattested_for(context, &partner)));
            }
            let avail = crate::offering::available(dir);
            Ok(content(
                serde_json::to_string_pretty(&crate::offering::catalog_json(&avail))
                    .unwrap_or_else(|_| "{}".into()),
            ))
        }

        "familiar.hello" => {
            if !is_attested(dir, context, &partner) {
                return Ok(tool_error(unattested_for(context, &partner)));
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

        "familiar.request_grant" => {
            let Some(context) = context else {
                return Ok(tool_error(principal_required()));
            };
            if !covenant::principal_attested(dir, context) {
                grant::audit_access_refusal(
                    dir,
                    context,
                    crate::partner_act::PartnerOperation::GrantRequest,
                    crate::partner_act::ReasonCode::CovenantMissing,
                    now,
                );
                return Ok(tool_error(unattested_for(Some(context), "")));
            }
            let input: grant::GrantRequestInput = match serde_json::from_value(args) {
                Ok(input) => input,
                Err(error) => {
                    grant::audit_schema_refusal(
                        dir,
                        context,
                        crate::partner_act::PartnerOperation::GrantRequest,
                        now,
                    );
                    return Ok(tool_error(format!(
                        "request refused — invalid typed input: {error}"
                    )));
                }
            };
            match grant::request_grant(dir, context, input, now) {
                Ok(receipt) => Ok(content(
                    serde_json::to_string_pretty(&receipt).unwrap_or_else(|_| "{}".into()),
                )),
                Err(refusal) => Ok(tool_error(format!("request refused — {refusal}"))),
            }
        }

        "familiar.propose" => {
            let Some(context) = context else {
                return Ok(tool_error(principal_required()));
            };
            if !covenant::principal_attested(dir, context) {
                grant::audit_access_refusal(
                    dir,
                    context,
                    crate::partner_act::PartnerOperation::Proposal,
                    crate::partner_act::ReasonCode::CovenantMissing,
                    now,
                );
                return Ok(tool_error(unattested_for(Some(context), "")));
            }
            let input: grant::ProposalInput = match serde_json::from_value(args) {
                Ok(input) => input,
                Err(error) => {
                    grant::audit_schema_refusal(
                        dir,
                        context,
                        crate::partner_act::PartnerOperation::Proposal,
                        now,
                    );
                    return Ok(tool_error(format!(
                        "proposal refused — invalid typed input: {error}"
                    )));
                }
            };
            match grant::propose(dir, context, input, now) {
                Ok(receipt) => Ok(content(
                    serde_json::to_string_pretty(&receipt).unwrap_or_else(|_| "{}".into()),
                )),
                Err(refusal) => Ok(tool_error(format!("proposal refused — {refusal}"))),
            }
        }

        "" => Err((INVALID_PARAMS, "tools/call needs a `name`".into())),

        other => {
            // Two different refusals, and the difference matters to whoever is reading it:
            // a tool that exists behind the covenant, versus one that does not exist at all.
            if matches!(other, "familiar.request_grant" | "familiar.propose") && context.is_none() {
                Ok(tool_error(principal_required()))
            } else if other == "familiar.hello" || !is_attested(dir, context, &partner) {
                Ok(tool_error(unattested_for(context, other)))
            } else {
                Ok(tool_error(format!(
                    "`{other}` is not a tool this familiar offers. Call `tools/list` for what is."
                )))
            }
        }
    }
}

fn is_attested(dir: &Path, context: Option<&PartnerContext>, partner: &str) -> bool {
    context.map_or_else(
        || covenant::attested(dir, partner),
        |context| covenant::principal_attested(dir, context),
    )
}

fn unattested_for(context: Option<&PartnerContext>, what: &str) -> String {
    match context {
        Some(_) => format!(
            "`{what}` is not available until this authenticated principal accepts the current three laws. Call `familiar.constitution`, then `familiar.attest` with only `statement`; identity comes from the credential."
        ),
        None => unattested(what),
    }
}

fn principal_required() -> String {
    "rung 3 requires a human-registered per-partner credential; the door-wide bearer and a caller-supplied `partner` label carry no grant identity".into()
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
    handle_bytes_for(dir, body, now, None)
}

pub fn handle_bytes_for(
    dir: &Path,
    body: &[u8],
    now: i64,
    context: Option<&PartnerContext>,
) -> Value {
    if body.len() > MAX_REQUEST_BYTES {
        return json!({
            "jsonrpc": "2.0", "id": Value::Null,
            "error": { "code": -32600, "message": "request body exceeds the 64 KiB MCP ceiling" }
        });
    }
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
    handle_for(dir, &request, now, context).unwrap_or_else(|| {
        // A notification: answered with an empty object, which the transport turns into a 202.
        Value::Object(Map::new())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
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

    fn context(id: &str) -> PartnerContext {
        PartnerContext {
            principal: id.into(),
            credential_fingerprint: format!("fingerprint-{id}"),
            alias: "Workshop agent".into(),
        }
    }

    fn declare_surface(dir: &Path) {
        let file = json!({ "actuators": [{
            "surface": "ians-secret-lamp",
            "state_cmd": "private state command",
            "state": { "fields": { "power": { "kind": "enum", "values": ["on", "off"],
                "source": { "kind": "json", "key": "power" } } } },
            "actions": { "private-on": "secret on", "private-off": "secret off" },
            "buckets": [
                { "name": "private-on", "when": [{ "op": "eq", "field": "power", "value": "off" }] },
                { "name": "private-off", "when": [] }
            ]
        }] });
        std::fs::write(
            dir.join(familiar_kernel::actuator::ACTUATORS_FILE),
            serde_json::to_vec(&file).unwrap(),
        )
        .unwrap();
    }

    fn grant_args(key: &str) -> Value {
        json!({
            "request_key": key,
            "class_id": "switchable.reversible/v1",
            "requested_operations": {
                "set_state": { "state": { "kind": "enum", "values": ["primary", "reverted"] } }
            },
            "requested_duration_seconds": 600,
            "reason": "quoted partner data"
        })
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

    #[test]
    fn a_label_covenant_and_door_bearer_can_never_reach_rung_three() {
        let d = tmp("unbound_rung3");
        covenant::accept(&d, "same-label", "yes", 1).unwrap();
        let list = handle(
            &d,
            &req("tools/list", json!({ "partner": "same-label" })),
            2,
        )
        .unwrap();
        assert!(!names(&list).contains(&"familiar.request_grant".to_string()));
        assert!(!names(&list).contains(&"familiar.propose".to_string()));
        let called = handle(
            &d,
            &req(
                "tools/call",
                json!({ "name": "familiar.request_grant", "arguments": grant_args("one") }),
            ),
            3,
        )
        .unwrap();
        assert!(is_error(&called));
        assert!(text_of(&called).contains("per-partner credential"));
        assert!(crate::partner_act::load(&d).unwrap().is_empty());
    }

    #[test]
    fn an_authenticated_principal_attests_without_a_label_then_gets_rung_three() {
        let d = tmp("bound_rung3");
        declare_surface(&d);
        let context = context("principal-a");

        let before = handle_for(&d, &req("tools/list", json!({})), 1, Some(&context)).unwrap();
        assert_eq!(
            names(&before),
            vec!["familiar.constitution", "familiar.attest"]
        );
        let refused = handle_for(
            &d,
            &req(
                "tools/call",
                json!({ "name": "familiar.request_grant", "arguments": grant_args("early") }),
            ),
            2,
            Some(&context),
        )
        .unwrap();
        assert!(is_error(&refused));
        assert!(crate::partner_act::load(&d)
            .unwrap()
            .iter()
            .any(|event| event.reason_code == crate::partner_act::ReasonCode::CovenantMissing));

        let spoof = handle_for(
            &d,
            &req(
                "tools/call",
                json!({ "name": "familiar.attest", "arguments": {
                    "partner": "someone-else", "statement": "yes"
                } }),
            ),
            3,
            Some(&context),
        )
        .unwrap();
        assert!(is_error(&spoof));
        assert!(!covenant::principal_attested(&d, &context));

        let accepted = handle_for(
            &d,
            &req(
                "tools/call",
                json!({ "name": "familiar.attest", "arguments": { "statement": "yes" } }),
            ),
            4,
            Some(&context),
        )
        .unwrap();
        assert!(!is_error(&accepted));
        let acceptance = text_of(&accepted);
        assert!(!acceptance.contains("Workshop agent"));
        assert!(!acceptance.contains("fingerprint-principal-a"));

        let after = handle_for(&d, &req("tools/list", json!({})), 5, Some(&context)).unwrap();
        let names = names(&after);
        assert!(names.contains(&"familiar.request_grant".to_string()));
        assert!(names.contains(&"familiar.propose".to_string()));
        let request_schema = after["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == "familiar.request_grant")
            .unwrap();
        assert!(request_schema["inputSchema"]["properties"]
            .get("partner")
            .is_none());
        assert_eq!(request_schema["annotations"]["readOnlyHint"], false);
    }

    #[test]
    fn request_and_propose_wire_only_public_receipts_and_never_actuate() {
        let d = tmp("rung3_wire");
        declare_surface(&d);
        let context = context("principal-a");
        covenant::accept_principal(&d, &context, "yes", 1).unwrap();
        let requested = handle_for(
            &d,
            &req(
                "tools/call",
                json!({ "name": "familiar.request_grant", "arguments": grant_args("wire") }),
            ),
            2,
            Some(&context),
        )
        .unwrap();
        assert!(!is_error(&requested));
        let pending: Value = serde_json::from_str(&text_of(&requested)).unwrap();
        let request_id = pending["request_id"].as_str().unwrap();

        let mut boundary = familiar_kernel::boundary::Boundary::closed();
        boundary.allow_agent = true;
        std::fs::write(
            d.join(familiar_kernel::boundary::BOUNDARY_FILE),
            serde_json::to_vec(&boundary).unwrap(),
        )
        .unwrap();
        crate::grant::grant_request(
            &d,
            "ian",
            request_id,
            "ians-secret-lamp",
            BTreeMap::from([(
                "set_state".into(),
                BTreeMap::from([(
                    "state".into(),
                    crate::grant::ParameterBound::Enum {
                        values: vec!["primary".into()],
                    },
                )]),
            )]),
            302,
            3,
        )
        .unwrap();
        let status = handle_for(
            &d,
            &req(
                "tools/call",
                json!({ "name": "familiar.request_grant", "arguments": grant_args("wire") }),
            ),
            4,
            Some(&context),
        )
        .unwrap();
        let status_text = text_of(&status);
        let grant: Value = serde_json::from_str(&status_text).unwrap();
        let instance = grant["instance"].as_str().unwrap();
        for private in [
            "ians-secret-lamp",
            "Workshop agent",
            "fingerprint-principal-a",
            "private state command",
            "secret on",
        ] {
            assert!(!status_text.contains(private));
        }

        let proposed = handle_for(
            &d,
            &req(
                "tools/call",
                json!({ "name": "familiar.propose", "arguments": {
                    "proposal_key": "proposal-wire",
                    "instance": instance,
                    "operation": "set_state",
                    "parameters": { "state": "primary" },
                    "reason": "quoted partner data"
                } }),
            ),
            5,
            Some(&context),
        )
        .unwrap();
        assert!(!is_error(&proposed));
        let proposal_text = text_of(&proposed);
        assert!(proposal_text.contains("proposed"));
        assert!(!proposal_text.contains("ians-secret-lamp"));
        assert!(!proposal_text.contains("completed"));
        assert!(!d
            .join(familiar_kernel::actuator::ACTUATOR_STATE_FILE)
            .exists());
        let private = serde_json::to_string(&crate::partner_act::load(&d).unwrap()).unwrap();
        assert!(private.contains("ians-secret-lamp"));
    }

    #[test]
    fn oversized_and_malformed_authenticated_calls_have_the_designed_audit_boundary() {
        let d = tmp("rung3_bounds");
        let context = context("principal-a");
        covenant::accept_principal(&d, &context, "yes", 1).unwrap();
        let oversized = vec![b'x'; MAX_REQUEST_BYTES + 1];
        let response = handle_bytes_for(&d, &oversized, 2, Some(&context));
        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("64 KiB"));
        assert!(crate::partner_act::load(&d).unwrap().is_empty());

        let malformed = handle_for(
            &d,
            &req(
                "tools/call",
                json!({ "name": "familiar.request_grant", "arguments": {
                    "request_key": "bad", "partner": "spoof"
                } }),
            ),
            3,
            Some(&context),
        )
        .unwrap();
        assert!(is_error(&malformed));
        assert!(crate::partner_act::load(&d)
            .unwrap()
            .iter()
            .any(|event| event.reason_code == crate::partner_act::ReasonCode::SchemaInvalid));
    }
}
