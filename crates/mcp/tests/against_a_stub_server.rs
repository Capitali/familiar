//! The client against a server that actually speaks: a stub MCP endpoint on loopback, so the
//! handshake, discovery, the declaration gate and the boundary gate are all exercised over a
//! real socket rather than mocked away.
//!
//! Loopback plain HTTP is the one place the transport permits an unencrypted wire, and this is
//! why that exemption exists — the alternative is a test that proves nothing about framing.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use familiar_kernel::boundary::{Boundary, BOUNDARY_FILE};
use familiar_mcp::{Error, Session};
use serde_json::{json, Value};

/// A stub `ucf-exchange`: initialize, initialized, tools/list, tools/call. Counts what it was
/// asked, so a test can assert that a refused call never reached the wire at all.
struct Stub {
    port: u16,
    seen: Arc<AtomicUsize>,
    tool_calls: Arc<AtomicUsize>,
}

fn spawn_stub() -> Stub {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(AtomicUsize::new(0));
    let tool_calls = Arc::new(AtomicUsize::new(0));
    let (s, t) = (seen.clone(), tool_calls.clone());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let (s, t) = (s.clone(), t.clone());
            std::thread::spawn(move || serve_one(stream, s, t));
        }
    });
    Stub {
        port,
        seen,
        tool_calls,
    }
}

fn serve_one(mut stream: TcpStream, seen: Arc<AtomicUsize>, tool_calls: Arc<AtomicUsize>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut len = 0usize;
    let mut authorized = false;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            len = v.trim().parse().unwrap_or(0);
        }
        if lower.starts_with("authorization: bearer ucfk_") {
            authorized = true;
        }
        if line == "\r\n" {
            break;
        }
    }
    let mut body = vec![0u8; len];
    let _ = reader.read_exact(&mut body);
    seen.fetch_add(1, Ordering::SeqCst);
    let req: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let method = req.get("method").and_then(Value::as_str).unwrap_or("");
    let id = req.get("id").cloned().unwrap_or(Value::Null);

    if !authorized && !method.is_empty() {
        let _ = stream.write_all(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n");
        return;
    }

    let (status, payload) = match method {
        "initialize" => (
            200,
            Some(json!({"jsonrpc":"2.0","id":id,"result":{
                "protocolVersion":"2025-06-18",
                "capabilities":{"tools":{}},
                "serverInfo":{"name":"ucf-exchange","version":"1.0.0"}}})),
        ),
        "notifications/initialized" => (202, None),
        "tools/list" => (
            200,
            Some(json!({"jsonrpc":"2.0","id":id,"result":{"tools":[
                {"name":"list_products","description":"Catalogue of United Cat Foods lines",
                 "inputSchema":{"type":"object","properties":{}}},
                {"name":"place_order","description":"Not for the familiar to touch",
                 "inputSchema":{"type":"object","properties":{}}}]}})),
        ),
        "tools/call" => {
            tool_calls.fetch_add(1, Ordering::SeqCst);
            (
                200,
                Some(json!({"jsonrpc":"2.0","id":id,"result":{
                    "content":[{"type":"text","text":"Salmon Pate; Tuna Flakes"}],
                    "isError":false}})),
            )
        }
        _ => (
            200,
            Some(
                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"no such method"}}),
            ),
        ),
    };

    match payload {
        // Answered as SSE on purpose: it is the framing the real server uses, and the client
        // has to undo it.
        Some(v) => {
            let data = format!("event: message\r\ndata: {v}\r\n\r\n");
            let head = format!(
                "HTTP/1.1 {status} OK\r\nContent-Type: text/event-stream\r\n\
                 Mcp-Session-Id: stub-session\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                data.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(data.as_bytes());
        }
        None => {
            let _ = stream.write_all(
                format!(
                    "HTTP/1.1 {status} Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .as_bytes(),
            );
        }
    }
}

fn setup(dir: &std::path::Path, port: u16, tools: &str, open_network: bool) {
    std::fs::create_dir_all(dir.join("mcp")).unwrap();
    std::fs::write(
        dir.join("mcp/servers.json"),
        format!(
            r#"{{"servers":[{{"name":"ucf","url":"http://127.0.0.1:{port}/mcp",
               "key_file":"mcp/ucf.env","key_name":"UCF_TOKEN","tools":[{tools}],
               "note":"United Cat Foods — Jeff's exchange"}}]}}"#
        ),
    )
    .unwrap();
    std::fs::write(dir.join("mcp/ucf.env"), "UCF_TOKEN=ucfk_stub_secret\n").unwrap();
    let mut b = Boundary::closed();
    b.allow_network = open_network;
    std::fs::write(dir.join(BOUNDARY_FILE), serde_json::to_string(&b).unwrap()).unwrap();
}

#[test]
fn the_familiar_shakes_hands_discovers_and_calls_only_what_was_declared() {
    let stub = spawn_stub();
    let dir = familiar_kernel::testing::temp_root("mcp_stub");
    setup(&dir, stub.port, "\"list_products\"", true);

    // Handshake: the server names itself and agrees a protocol revision.
    let mut s = Session::open(&dir, "ucf").expect("the stub answers initialize");
    assert_eq!(s.server_name, "ucf-exchange");
    assert_eq!(s.server_version, "1.0.0");
    assert_eq!(s.protocol_version, familiar_mcp::session::PROTOCOL_VERSION);

    // Discovery is always permitted against a declared server — it is how a human decides
    // what to allow. Both tools are visible; only one is callable.
    let tools = s.tools().expect("tools/list");
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["list_products", "place_order"]);

    let out = s.call("list_products", json!({})).expect("a declared call");
    assert!(out.to_string().contains("Salmon Pate"));
    assert_eq!(stub.tool_calls.load(Ordering::SeqCst), 1);

    // Undeclared stays unactuatable (ADR-0032) — and the refusal happens HERE, so the wire
    // never carries it. The server's own offer is not permission.
    let before = stub.seen.load(Ordering::SeqCst);
    match s.call("place_order", json!({"qty": 1})) {
        Err(Error::UndeclaredTool { server, tool }) => {
            assert_eq!((server.as_str(), tool.as_str()), ("ucf", "place_order"));
        }
        other => panic!("an undeclared tool must refuse locally: {other:?}"),
    }
    assert_eq!(
        stub.seen.load(Ordering::SeqCst),
        before,
        "a refused call never reached the server"
    );
    assert_eq!(stub.tool_calls.load(Ordering::SeqCst), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_shut_boundary_refuses_before_anything_is_sent() {
    let stub = spawn_stub();
    let dir = familiar_kernel::testing::temp_root("mcp_stub_shut");
    setup(&dir, stub.port, "\"list_products\"", false);

    match Session::open(&dir, "ucf") {
        Err(Error::Refused(why)) => assert!(!why.is_empty()),
        other => panic!("a closed network gate must refuse: {other:?}"),
    }
    assert_eq!(
        stub.seen.load(Ordering::SeqCst),
        0,
        "nothing was sent — the gate is checked before the socket"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unreachable_server_degrades_instead_of_erroring_out() {
    // The no-oracle floor (ADR-0035): a partner that is not there is a system that is not
    // reachable, not a crash and not an invented answer.
    let dead = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = dead.local_addr().unwrap().port();
    drop(dead);
    let dir = familiar_kernel::testing::temp_root("mcp_stub_dead");
    setup(&dir, port, "", true);
    match Session::open(&dir, "ucf") {
        Err(Error::Io(_)) => {}
        other => panic!("an absent server is an Io error, nothing louder: {other:?}"),
    }
    // And a server nobody declared does not exist to the familiar at all.
    assert!(matches!(
        Session::open(&dir, "not-declared"),
        Err(Error::Undeclared(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}
