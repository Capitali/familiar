//! Market perception — the **UCF trade feed**.
//!
//! United Cat Foods runs a market daemon (`ucfmarketd`, the `UCFTRADE_SERVER`) whose
//! world advances on a fixed tick: station quote boards, galaxy-wide mids, and the
//! Dispatch news feed. This module turns that external world into observations — the
//! only truth — so the familiar processes market data exactly as it processes any
//! other perception: loops, theories, and (via briefs) the mesh.
//!
//! **Perception vs reach.** Polling the exchange is *outward* reach — an HTTP read of
//! another process — so the caller gates it behind `allow_network` through the
//! obedience guard, the same discipline as the connectivity probe and the LAN survey.
//! The human authorizes the feed twice over: by opening the gate, and by placing
//! `market.json` (server + API key) in the data dir — the same doctrine as
//! `call_llm.sh` on the LLM seam. No config, no reach.
//!
//! **Observations are triples that dedup naturally.** A price is recorded as
//! `station:<id> quotes <good>@<mid>` — a new record only when the price *changes*.
//! A Dispatch item records each status transition (`announced` → `in-effect` →
//! `withdrawn`/`expired`) exactly once. The market clock records one `advanced
//! tick:<n>` per tick. A polite poller aligned to the exchange's 5-minute cadence
//! therefore appends a bounded trickle, not a flood.
//!
//! Everything here is best-effort: an absent exchange, a bad key, or garbage on the
//! wire yields notes and no observations — never an error that fails the caller.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

use familiar_kernel::observation::Observation;
use serde::Deserialize;

/// The human-owned feed config (in the data dir; not source, not committed):
/// `{"server": "http://127.0.0.1:7877", "key": "ucfk_..."}`. The key is optional —
/// without one only the keyless `/v1/status` endpoint is read.
pub const MARKET_CONFIG: &str = "market.json";

const SOURCE: &str = "market";
const CONF: f64 = 0.95;
const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Deserialize)]
pub struct MarketConfig {
    pub server: String,
    #[serde(default)]
    pub key: Option<String>,
}

/// Load `market.json` from the data dir. `None` when absent or unreadable — the
/// human simply has not pointed the familiar at an exchange.
pub fn load_config(dir: &Path) -> Option<MarketConfig> {
    let bytes = std::fs::read(dir.join(MARKET_CONFIG)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// What one poll produced: observations to record, and human-readable notes about
/// anything skipped or unreachable (best-effort perception explains itself).
#[derive(Debug, Default)]
pub struct MarketPull {
    pub observations: Vec<Observation>,
    pub notes: Vec<String>,
}

fn obs(actor: &str, action: &str, object: String, context: String, now: i64) -> Observation {
    Observation::new(actor, action, object, context, SOURCE, now, CONF)
}

/// Poll the exchange once. Reads `/v1/status` (keyless), and with a key also
/// `/v1/galaxy/prices` and `/v1/news`. The caller has already cleared the
/// `allow_network` gate; this function only ever *reads*.
pub fn poll(cfg: &MarketConfig, now: i64) -> MarketPull {
    let mut pull = MarketPull::default();

    let Some(host) = host_of(&cfg.server) else {
        pull.notes.push(format!(
            "unsupported server URL '{}' (http:// only)",
            cfg.server
        ));
        return pull;
    };

    match get(&host, "/v1/status", None) {
        Some((200, body)) => pull.observations.extend(parse_status(&body, now)),
        Some((code, _)) => pull.notes.push(format!("status: HTTP {code}")),
        None => {
            pull.notes.push(format!(
                "exchange unreachable at {} — is it open?",
                cfg.server
            ));
            return pull; // no point walking the keyed endpoints
        }
    }

    let Some(key) = cfg.key.as_deref() else {
        pull.notes
            .push("no key configured — only /v1/status read (prices and news need one)".into());
        return pull;
    };

    match get(&host, "/v1/galaxy/prices", Some(key)) {
        Some((200, body)) => pull.observations.extend(parse_prices(&body, now)),
        Some((code, _)) => pull.notes.push(format!("prices: HTTP {code}")),
        None => pull.notes.push("prices: connection lost".into()),
    }
    match get(&host, "/v1/news", Some(key)) {
        Some((200, body)) => pull.observations.extend(parse_news(&body, now)),
        Some((code, _)) => pull.notes.push(format!("news: HTTP {code}")),
        None => pull.notes.push("news: connection lost".into()),
    }
    pull
}

/// `http://host:port[/]` → `host:port`. Anything else (https, garbage) is refused —
/// the exchange is a local/LAN daemon, and pretending to speak TLS would be worse
/// than saying no.
fn host_of(server: &str) -> Option<String> {
    let rest = server.strip_prefix("http://")?;
    let host = rest.trim_end_matches('/');
    if host.is_empty() || host.contains('/') {
        return None;
    }
    Some(host.to_string())
}

/// One bounded HTTP/1.0 GET. `None` on any transport failure; `Some((code, body))`
/// otherwise. Reads to Content-Length when declared, EOF otherwise.
fn get(host: &str, path: &str, key: Option<&str>) -> Option<(u16, String)> {
    let addr = host.to_socket_addrs().ok()?.next()?;
    let mut stream = TcpStream::connect_timeout(&addr, TIMEOUT).ok()?;
    stream.set_read_timeout(Some(TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(TIMEOUT)).ok()?;

    let auth = key
        .map(|k| format!("Authorization: Bearer {k}\r\n"))
        .unwrap_or_default();
    let req = format!("GET {path} HTTP/1.0\r\nHost: {host}\r\n{auth}Connection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).ok()?;

    let mut raw = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                raw.extend_from_slice(&buf[..n]);
                if response_complete(&raw) {
                    break;
                }
            }
            Err(_) => break, // timeout with a partial response: parse what we have
        }
    }
    parse_http_response(&raw)
}

/// Do we hold the full response (headers plus a declared Content-Length of body)?
fn response_complete(raw: &[u8]) -> bool {
    let Some(split) = find_header_end(raw) else {
        return false;
    };
    match content_length(&raw[..split]) {
        Some(len) => raw.len() >= split + 4 + len,
        None => false, // no declared length: only EOF ends it
    }
}

fn find_header_end(raw: &[u8]) -> Option<usize> {
    raw.windows(4).position(|w| w == b"\r\n\r\n")
}

fn content_length(head: &[u8]) -> Option<usize> {
    String::from_utf8_lossy(head).lines().find_map(|l| {
        l.to_ascii_lowercase()
            .strip_prefix("content-length:")
            .map(str::trim)
            .and_then(|v| v.parse().ok())
    })
}

/// Split a raw HTTP response into (status code, body). Pure — unit-tested.
fn parse_http_response(raw: &[u8]) -> Option<(u16, String)> {
    let split = find_header_end(raw)?;
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let code: u16 = head
        .lines()
        .next()?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    let body = String::from_utf8_lossy(&raw[split + 4..]).into_owned();
    Some((code, body))
}

// ---------------------------------------------------------------------------------
// Parsers — pure functions from response body to observations. Garbage in, empty
// out: the feed never trusts its own far end.
// ---------------------------------------------------------------------------------

/// `/v1/status` → one `ucf-market advanced tick:<n>` observation. The state hash and
/// content version ride the context, so a hash change at the same tick still reads
/// as the same market moment (the tick is the market's own clock).
fn parse_status(body: &str, now: i64) -> Vec<Observation> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Status {
        tick: u64,
        #[serde(default)]
        state_hash: String,
        #[serde(default)]
        content_version: u64,
    }
    let Ok(s) = serde_json::from_str::<Status>(body) else {
        return Vec::new();
    };
    vec![obs(
        "ucf-market",
        "advanced",
        format!("tick:{}", s.tick),
        format!("state={} content=v{}", s.state_hash, s.content_version),
        now,
    )]
}

/// `/v1/galaxy/prices` → one `station:<id> quotes <good>@<mid>` per row. The triple
/// carries the price, so the caller's structural dedup records only *changes*.
fn parse_prices(body: &str, now: i64) -> Vec<Observation> {
    #[derive(Deserialize)]
    struct Row {
        good: String,
        station: String,
        mid: i64,
        #[serde(default)]
        stock: i64,
    }
    let Ok(rows) = serde_json::from_str::<Vec<Row>>(body) else {
        return Vec::new();
    };
    rows.into_iter()
        .map(|r| {
            obs(
                &format!("station:{}", r.station),
                "quotes",
                format!("{}@{}", r.good, r.mid),
                format!("stock={}", r.stock),
                now,
            )
        })
        .collect()
}

/// `/v1/news` → one `ucf-dispatch <status> <headline>` per item. The status is the
/// action, so each transition of an event's life (`announced` → `in-effect` →
/// `withdrawn`/`expired`) records exactly once.
fn parse_news(body: &str, now: i64) -> Vec<Observation> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Item {
        headline: String,
        #[serde(default)]
        tier: String,
        #[serde(default)]
        status: String,
        #[serde(default)]
        effective_at_tick: u64,
        #[serde(default)]
        expires_at_tick: u64,
    }
    let Ok(items) = serde_json::from_str::<Vec<Item>>(body) else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter(|i| !i.headline.is_empty() && !i.status.is_empty())
        .map(|i| {
            obs(
                "ucf-dispatch",
                &i.status,
                i.headline,
                format!(
                    "tier={} effective={} expires={}",
                    i.tier, i.effective_at_tick, i.expires_at_tick
                ),
                now,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_becomes_one_market_clock_observation() {
        let body = r#"{"tick":1234,"tickDurationSec":300,"nextTickAt":"2026-08-03T12:00:00Z",
                       "stateHash":"abc123","contentVersion":7}"#;
        let o = parse_status(body, 99);
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].actor, "ucf-market");
        assert_eq!(o[0].action, "advanced");
        assert_eq!(o[0].object, "tick:1234");
        assert!(o[0].context.contains("state=abc123"));
        assert_eq!(o[0].source, "market");
    }

    #[test]
    fn prices_become_price_carrying_triples_that_dedup_on_change() {
        let body = r#"[{"good":"tuna-supreme","station":"meowmart-prime","mid":42,"stock":900},
                       {"good":"salmon-loaf","station":"whisker-station","mid":17,"stock":40}]"#;
        let o = parse_prices(body, 99);
        assert_eq!(o.len(), 2);
        assert_eq!(o[0].actor, "station:meowmart-prime");
        assert_eq!(o[0].action, "quotes");
        assert_eq!(o[0].object, "tuna-supreme@42", "the price is IN the triple");
        assert!(o[0].context.contains("stock=900"));
    }

    #[test]
    fn news_records_each_status_transition_as_its_own_action() {
        let body = r#"[{"headline":"Tuna futures soar","tier":"major","announcedAtTick":10,
                        "effectiveAtTick":12,"expiresAtTick":20,"status":"in-effect"},
                       {"headline":"","tier":"minor","announcedAtTick":1,"effectiveAtTick":2,
                        "expiresAtTick":3,"status":"expired"}]"#;
        let o = parse_news(body, 99);
        assert_eq!(o.len(), 1, "a headline-less item is dropped");
        assert_eq!(o[0].actor, "ucf-dispatch");
        assert_eq!(o[0].action, "in-effect");
        assert_eq!(o[0].object, "Tuna futures soar");
        assert!(o[0].context.contains("tier=major"));
    }

    #[test]
    fn garbage_bodies_yield_nothing_never_a_panic() {
        assert!(parse_status("not json", 0).is_empty());
        assert!(parse_prices(r#"{"wrong":"shape"}"#, 0).is_empty());
        assert!(parse_news("[[[[", 0).is_empty());
    }

    #[test]
    fn server_urls_are_parsed_strictly() {
        assert_eq!(
            host_of("http://127.0.0.1:7877").as_deref(),
            Some("127.0.0.1:7877")
        );
        assert_eq!(
            host_of("http://127.0.0.1:7877/").as_deref(),
            Some("127.0.0.1:7877")
        );
        assert!(
            host_of("https://exchange.example:7877").is_none(),
            "no TLS pretence"
        );
        assert!(host_of("127.0.0.1:7877").is_none(), "scheme required");
        assert!(host_of("http://host:1/path").is_none(), "no path smuggling");
    }

    #[test]
    fn http_responses_are_split_into_code_and_body() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nhi";
        assert_eq!(parse_http_response(raw), Some((200, "hi".into())));
        let raw = b"HTTP/1.0 401 Unauthorized\r\n\r\n";
        assert_eq!(parse_http_response(raw), Some((401, String::new())));
        assert_eq!(parse_http_response(b"junk with no header end"), None);
    }

    #[test]
    fn a_response_is_complete_at_its_declared_length() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nbody";
        assert!(response_complete(raw));
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\nbody";
        assert!(!response_complete(raw));
        let raw = b"HTTP/1.1 200 OK\r\n\r\nbody";
        assert!(!response_complete(raw), "no length: only EOF ends it");
    }
}
