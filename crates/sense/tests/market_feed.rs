//! The market feed against a **fake exchange**.
//!
//! `market::poll` reaches outward over real HTTP, so it is tested against a real
//! loopback socket speaking scripted responses — the familiar's own client, header
//! handling, and parsers all run unmodified; only the far end is canned. The
//! properties: what one poll records, that the key travels as a bearer header, that
//! keyless polls never touch the keyed endpoints, and that an absent or lying
//! exchange yields notes rather than panics.

use familiar_sense::market::{poll, MarketConfig};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

/// One request the fake saw: (path, authorization-header-value-if-any).
type Seen = Arc<Mutex<Vec<(String, Option<String>)>>>;

/// A minimal scripted exchange: serves canned bodies per path, records every
/// request. Runs until the test process exits.
fn fake_exchange(routes: Vec<(&'static str, u16, String)>) -> (String, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let record = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut raw = Vec::new();
            let mut buf = [0u8; 2048];
            while !raw.windows(4).any(|w| w == b"\r\n\r\n") {
                match stream.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => raw.extend_from_slice(&buf[..n]),
                }
            }
            let head = String::from_utf8_lossy(&raw).into_owned();
            let path = head
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .unwrap_or_default()
                .to_string();
            let auth = head
                .lines()
                .find_map(|l| l.strip_prefix("Authorization: ").map(str::to_string));
            record.lock().unwrap().push((path.clone(), auth));
            let (code, body) = routes
                .iter()
                .find(|(p, _, _)| *p == path)
                .map(|(_, c, b)| (*c, b.clone()))
                .unwrap_or((404, "no such route".to_string()));
            let resp = format!(
                "HTTP/1.1 {code} X\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    (format!("http://{addr}"), seen)
}

const STATUS: &str = r#"{"tick":88,"tickDurationSec":300,"nextTickAt":"2026-08-03T12:00:00Z","stateHash":"h1","contentVersion":3}"#;
const PRICES: &str = r#"[{"good":"tuna-supreme","station":"meowmart-prime","mid":42,"stock":900}]"#;
const NEWS: &str = r#"[{"headline":"Tuna futures soar","tier":"major","announcedAtTick":80,"effectiveAtTick":85,"expiresAtTick":95,"status":"in-effect"}]"#;

/// A keyed poll walks all three endpoints, carries the bearer key to the keyed ones,
/// and turns the exchange's world into the expected triples.
#[test]
fn a_keyed_poll_records_the_clock_the_prices_and_the_dispatch() {
    let (server, seen) = fake_exchange(vec![
        ("/v1/status", 200, STATUS.into()),
        ("/v1/galaxy/prices", 200, PRICES.into()),
        ("/v1/news", 200, NEWS.into()),
    ]);
    let cfg = MarketConfig {
        server,
        key: Some("ucfk_test".into()),
    };

    let pull = poll(&cfg, 1000);

    let triples: Vec<(String, String, String)> = pull
        .observations
        .iter()
        .map(|o| (o.actor.clone(), o.action.clone(), o.object.clone()))
        .collect();
    assert!(triples.contains(&("ucf-market".into(), "advanced".into(), "tick:88".into())));
    assert!(triples.contains(&(
        "station:meowmart-prime".into(),
        "quotes".into(),
        "tuna-supreme@42".into()
    )));
    assert!(triples.contains(&(
        "ucf-dispatch".into(),
        "in-effect".into(),
        "Tuna futures soar".into()
    )));
    assert!(
        pull.observations.iter().all(|o| o.source == "market"),
        "market data is never laundered into local sensing"
    );

    let seen = seen.lock().unwrap();
    let auth_of = |p: &str| {
        seen.iter()
            .find(|(path, _)| path == p)
            .map(|(_, a)| a.clone())
            .expect("endpoint was contacted")
    };
    assert_eq!(
        auth_of("/v1/galaxy/prices"),
        Some("Bearer ucfk_test".into())
    );
    assert_eq!(auth_of("/v1/news"), Some("Bearer ucfk_test".into()));
}

/// Without a key only the keyless status endpoint is read — the poller does not
/// knock on doors it has no key for, and it says so.
#[test]
fn a_keyless_poll_reads_status_only_and_says_so() {
    let (server, seen) = fake_exchange(vec![("/v1/status", 200, STATUS.into())]);
    let cfg = MarketConfig { server, key: None };

    let pull = poll(&cfg, 1000);

    assert_eq!(pull.observations.len(), 1);
    assert_eq!(pull.observations[0].object, "tick:88");
    assert!(pull.notes.iter().any(|n| n.contains("no key")));
    let paths: Vec<String> = seen
        .lock()
        .unwrap()
        .iter()
        .map(|(p, _)| p.clone())
        .collect();
    assert_eq!(
        paths,
        vec!["/v1/status"],
        "keyed endpoints were never contacted"
    );
}

/// An exchange that is simply not there is a note, not an error and not a hang.
#[test]
fn an_absent_exchange_yields_a_note_and_no_observations() {
    // Bind-then-drop reserves an address nobody is listening on.
    let addr = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap()
    };
    let cfg = MarketConfig {
        server: format!("http://{addr}"),
        key: Some("ucfk_test".into()),
    };

    let pull = poll(&cfg, 1000);

    assert!(pull.observations.is_empty());
    assert!(pull.notes.iter().any(|n| n.contains("unreachable")));
}

/// An exchange that answers garbage (or refuses the key) contributes nothing —
/// the feed never trusts its own far end.
#[test]
fn garbage_and_refusals_are_survived() {
    let (server, _) = fake_exchange(vec![
        ("/v1/status", 200, "<<not json>>".into()),
        ("/v1/galaxy/prices", 401, "who are you".into()),
        ("/v1/news", 200, r#"{"wrong":"shape"}"#.into()),
    ]);
    let cfg = MarketConfig {
        server,
        key: Some("ucfk_bad".into()),
    };

    let pull = poll(&cfg, 1000);

    assert!(pull.observations.is_empty(), "{:?}", pull.observations);
    assert!(
        pull.notes.iter().any(|n| n.contains("401")),
        "the refusal is reported: {:?}",
        pull.notes
    );
}
