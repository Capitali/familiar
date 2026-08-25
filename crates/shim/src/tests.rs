use super::*;
use std::io::Cursor;

fn msg(json: &str) -> serde_json::Value {
    serde_json::from_str(json).unwrap()
}

// ---- exchange(): the stdio pump ----

#[test]
fn a_request_gets_the_response_with_its_id() {
    let mut stdin = Vec::new();
    let mut stdout =
        Cursor::new(b"{\"jsonrpc\":\"2.0\",\"id\":7,\"result\":{\"ok\":true}}\n".to_vec());
    let reply = exchange(
        &mut stdin,
        &mut stdout,
        &msg(r#"{"jsonrpc":"2.0","id":7,"method":"tools/list"}"#),
    )
    .unwrap();
    assert_eq!(
        reply.unwrap(),
        r#"{"jsonrpc":"2.0","id":7,"result":{"ok":true}}"#
    );
    // The outgoing frame is exactly one newline-terminated line.
    let written = String::from_utf8(stdin).unwrap();
    assert!(written.ends_with('\n'));
    assert_eq!(written.matches('\n').count(), 1);
}

#[test]
fn server_chatter_is_skipped_within_the_budget_never_relayed_as_the_answer() {
    let noise = "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/message\",\"params\":{}}\n";
    let mut stream = noise.repeat(3);
    stream.push_str("{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n");
    let mut stdin = Vec::new();
    let mut stdout = Cursor::new(stream.into_bytes());
    let reply = exchange(
        &mut stdin,
        &mut stdout,
        &msg(r#"{"jsonrpc":"2.0","id":1,"method":"tools/call"}"#),
    )
    .unwrap();
    assert_eq!(reply.unwrap(), r#"{"jsonrpc":"2.0","id":1,"result":{}}"#);
}

#[test]
fn a_server_chatty_past_the_budget_is_an_error_not_a_hang() {
    let noise = "{\"jsonrpc\":\"2.0\",\"method\":\"noise\"}\n".repeat(MAX_SKIPPED_LINES + 1);
    let mut stdin = Vec::new();
    let mut stdout = Cursor::new(noise.into_bytes());
    let err = exchange(
        &mut stdin,
        &mut stdout,
        &msg(r#"{"jsonrpc":"2.0","id":2,"method":"tools/call"}"#),
    )
    .unwrap_err();
    assert!(matches!(err, Error::Protocol(_)));
}

#[test]
fn a_notification_is_delivered_and_owes_nothing_back() {
    let mut stdin = Vec::new();
    let mut stdout = Cursor::new(Vec::new()); // nothing to read — and nothing is read
    let reply = exchange(
        &mut stdin,
        &mut stdout,
        &msg(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#),
    )
    .unwrap();
    assert!(reply.is_none());
    assert!(!stdin.is_empty());
}

#[test]
fn child_eof_before_the_response_is_child_gone() {
    let mut stdin = Vec::new();
    let mut stdout = Cursor::new(Vec::new());
    let err = exchange(
        &mut stdin,
        &mut stdout,
        &msg(r#"{"jsonrpc":"2.0","id":3,"method":"tools/call"}"#),
    )
    .unwrap_err();
    assert!(matches!(err, Error::ChildGone(_)));
}

#[test]
fn an_unbounded_child_line_is_refused_not_buffered() {
    // A "line" that never ends and exceeds the bound: the pump must refuse, not grow.
    let mut endless = vec![b'x'; MAX_BODY + 2];
    endless.push(b'\n');
    let mut stdin = Vec::new();
    let mut stdout = Cursor::new(endless);
    let err = exchange(
        &mut stdin,
        &mut stdout,
        &msg(r#"{"jsonrpc":"2.0","id":4,"method":"tools/call"}"#),
    )
    .unwrap_err();
    assert!(matches!(err, Error::Protocol(_)));
}

// ---- serve(): the loopback fence ----

struct CannedBridge;
impl Bridge for CannedBridge {
    fn exchange(&self, message: &serde_json::Value) -> Result<Option<String>, Error> {
        match message.get("id") {
            Some(id) if !id.is_null() => Ok(Some(format!(
                "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{{\"echoed\":true}}}}"
            ))),
            _ => Ok(None),
        }
    }
}

struct GoneBridge;
impl Bridge for GoneBridge {
    fn exchange(&self, _message: &serde_json::Value) -> Result<Option<String>, Error> {
        Err(Error::ChildGone("exited".into()))
    }
}

/// Start serve() on an ephemeral loopback port with the given bridge.
fn shim_on_loopback(bridge: &'static dyn Bridge, token: &str) -> std::net::SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let token = token.to_string();
    std::thread::spawn(move || serve(listener, &token, bridge));
    addr
}

/// One raw HTTP request, one parsed (status, body) back.
fn call(addr: std::net::SocketAddr, request: &str) -> (u16, String) {
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect(addr).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut raw = String::new();
    stream.read_to_string(&mut raw).unwrap();
    let status = raw
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let body = raw
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();
    (status, body)
}

fn post(addr: std::net::SocketAddr, path: &str, auth: Option<&str>, body: &str) -> (u16, String) {
    let auth_header = auth
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    call(
        addr,
        &format!(
            "POST {path} HTTP/1.1\r\nHost: shim\r\n{auth_header}Content-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ),
    )
}

#[test]
fn no_token_no_shim_and_the_child_is_never_touched() {
    struct MustNotRun;
    impl Bridge for MustNotRun {
        fn exchange(&self, _m: &serde_json::Value) -> Result<Option<String>, Error> {
            panic!("an unauthenticated request reached the child");
        }
    }
    static BRIDGE: MustNotRun = MustNotRun;
    let addr = shim_on_loopback(&BRIDGE, "right-token");
    let (status, _) = post(addr, "/mcp", None, r#"{"jsonrpc":"2.0","id":1}"#);
    assert_eq!(status, 401);
    let (status, _) = post(
        addr,
        "/mcp",
        Some("wrong-token"),
        r#"{"jsonrpc":"2.0","id":1}"#,
    );
    assert_eq!(status, 401);
}

#[test]
fn the_happy_path_bridges_and_the_refusals_are_typed() {
    static BRIDGE: CannedBridge = CannedBridge;
    let addr = shim_on_loopback(&BRIDGE, "tok");
    // Happy: a request comes back as the child's JSON.
    let (status, body) = post(
        addr,
        "/mcp",
        Some("tok"),
        r#"{"jsonrpc":"2.0","id":9,"method":"x"}"#,
    );
    assert_eq!(status, 200);
    assert!(body.contains("\"echoed\":true") && body.contains("\"id\":9"));
    // A notification is accepted with nothing owed back.
    let (status, body) = post(
        addr,
        "/mcp",
        Some("tok"),
        r#"{"jsonrpc":"2.0","method":"n"}"#,
    );
    assert_eq!(status, 202);
    assert!(body.is_empty());
    // Wrong path / wrong method / non-JSON / batch: typed refusals, in that order of checks.
    assert_eq!(post(addr, "/other", Some("tok"), "{}").0, 404);
    let (status, _) = call(
        addr,
        "GET /mcp HTTP/1.1\r\nHost: shim\r\nAuthorization: Bearer tok\r\n\r\n",
    );
    assert_eq!(status, 405);
    assert_eq!(post(addr, "/mcp", Some("tok"), "not json").0, 400);
    assert_eq!(
        post(addr, "/mcp", Some("tok"), r#"[{"jsonrpc":"2.0","id":1}]"#).0,
        400
    );
}

#[test]
fn a_dead_child_is_a_stated_502() {
    static BRIDGE: GoneBridge = GoneBridge;
    let addr = shim_on_loopback(&BRIDGE, "tok");
    let (status, body) = post(addr, "/mcp", Some("tok"), r#"{"jsonrpc":"2.0","id":1}"#);
    assert_eq!(status, 502);
    assert!(body.contains("child gone"));
}

// ---- the key file ----

#[test]
fn the_key_file_is_env_format_and_owner_only() {
    let dir = std::env::temp_dir().join(format!("shim_key_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("mcp").join("shim.env");
    write_key_file(&path, "abc123").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "TOKEN=abc123\n");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "the token file must be owner-only");
    }
}

#[test]
fn the_token_is_never_a_fixed_value() {
    let a = mint_token().unwrap();
    let b = mint_token().unwrap();
    assert_eq!(a.len(), 32);
    assert_ne!(a, b);
}

// ---- the real child, end to end ----

#[cfg(unix)]
#[test]
fn a_real_stdio_child_answers_through_the_bridge() {
    // A shell stand-in for a stdio MCP server: answers every line with a canned id-1 reply.
    let script = r#"while IFS= read -r line; do printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"live":true}}'; done"#;
    let (mut child, bridge) = ChildBridge::spawn("sh", &["-c".into(), script.into()]).unwrap();
    let reply = bridge
        .exchange(&msg(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#))
        .unwrap()
        .unwrap();
    assert!(reply.contains("\"live\":true"));
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn a_body_that_arrives_after_the_head_is_still_read() {
    use std::io::{Read, Write};
    static BRIDGE: CannedBridge = CannedBridge;
    let addr = shim_on_loopback(&BRIDGE, "tok");
    let body = r#"{"jsonrpc":"2.0","id":5,"method":"x"}"#;
    let mut stream = std::net::TcpStream::connect(addr).unwrap();
    // Head first, body in a separate later write — the framing every client is allowed.
    stream
        .write_all(
            format!(
                "POST /mcp HTTP/1.1\r\nHost: shim\r\nAuthorization: Bearer tok\r\n\
                 Content-Type: application/json\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n",
                body.len()
            )
            .as_bytes(),
        )
        .unwrap();
    stream.flush().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    stream.write_all(body.as_bytes()).unwrap();
    let mut raw = String::new();
    stream.read_to_string(&mut raw).unwrap();
    assert!(raw.starts_with("HTTP/1.1 200"), "got: {raw}");
    assert!(raw.contains("\"id\":5"));
}
