//! Loopback client to the familiar daemon — the Glass reads and speaks through the same
//! localhost-only seam the SwiftUI console used (`GET /local/worldview`, `POST /local/answer`,
//! `POST /local/gate`), so the daemon stays the single writer of the data dir.
//!
//! A background thread polls the worldview and executes writes; the render loop only ever
//! locks a small shared struct — no I/O on the frame path.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use familiar_mesh::worldview::Worldview;

const POLL_EVERY: Duration = Duration::from_secs(3);
const IO_TIMEOUT: Duration = Duration::from_secs(4);

/// What the poller learned, for the render loop to draw.
#[derive(Default)]
pub struct Shared {
    pub view: Option<Worldview>,
    /// Human-readable reason the last poll failed (daemon down, no group…); None when healthy.
    pub error: Option<String>,
    /// Bumped on every successful poll — the UI shows staleness honestly (Law III).
    pub fetched_at: Option<Instant>,
}

/// A human act the UI queues for the writer thread.
pub enum Act {
    Answer(String),
    Gate { gate: String, open: bool },
}

pub struct Client {
    pub shared: Arc<Mutex<Shared>>,
    acts: mpsc::Sender<Act>,
}

impl Client {
    /// Spawn the poll/write thread against the daemon on `port` (loopback only).
    pub fn start(port: u16) -> Self {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let (tx, rx) = mpsc::channel::<Act>();
        let s = shared.clone();
        std::thread::spawn(move || {
            let mut last_poll = Instant::now() - POLL_EVERY;
            loop {
                // Drain queued acts first so an answer lands before the next read.
                while let Ok(act) = rx.try_recv() {
                    let res = match act {
                        Act::Answer(text) => post(
                            port,
                            "/local/answer",
                            &serde_json::json!({ "text": text }).to_string(),
                        ),
                        Act::Gate { gate, open } => post(
                            port,
                            "/local/gate",
                            &serde_json::json!({ "gate": gate, "open": open }).to_string(),
                        ),
                    };
                    if let Err(e) = res {
                        let mut g = s.lock().unwrap_or_else(|p| p.into_inner());
                        g.error = Some(e);
                    }
                    last_poll = Instant::now() - POLL_EVERY; // re-read right away
                }
                if last_poll.elapsed() >= POLL_EVERY {
                    last_poll = Instant::now();
                    match fetch(port) {
                        Ok(view) => {
                            let mut g = s.lock().unwrap_or_else(|p| p.into_inner());
                            g.view = Some(view);
                            g.error = None;
                            g.fetched_at = Some(Instant::now());
                        }
                        Err(e) => {
                            let mut g = s.lock().unwrap_or_else(|p| p.into_inner());
                            g.error = Some(e);
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        });
        Self { shared, acts: tx }
    }

    pub fn answer(&self, text: &str) {
        let _ = self.acts.send(Act::Answer(text.to_string()));
    }

    pub fn set_gate(&self, gate: &str, open: bool) {
        let _ = self.acts.send(Act::Gate {
            gate: gate.to_string(),
            open,
        });
    }
}

fn fetch(port: u16) -> Result<Worldview, String> {
    let body = request(
        port,
        "GET /local/worldview HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
    )?;
    serde_json::from_slice(&body).map_err(|e| format!("decode: {e}"))
}

fn post(port: u16, path: &str, json: &str) -> Result<(), String> {
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
        json.len()
    );
    request(port, &req).map(|_| ())
}

/// One HTTP/1.1 exchange over loopback. `Connection: close` keeps parsing trivial: status
/// line + headers, then read to EOF for the body (the daemon answers with Content-Length,
/// never chunked, but read-to-EOF is correct for both).
fn request(port: u16, raw: &str) -> Result<Vec<u8>, String> {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&addr, IO_TIMEOUT)
        .map_err(|e| format!("daemon not reachable on :{port} — is it running? ({e})"))?;
    stream.set_read_timeout(Some(IO_TIMEOUT)).ok();
    stream.set_write_timeout(Some(IO_TIMEOUT)).ok();
    stream
        .write_all(raw.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let header_end = buf
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("malformed response")?;
    let head = String::from_utf8_lossy(&buf[..header_end]);
    let status: u16 = head
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or("malformed status line")?;
    let body = buf[header_end + 4..].to_vec();
    if status != 200 {
        return Err(format!(
            "{status}: {}",
            String::from_utf8_lossy(&body).trim()
        ));
    }
    Ok(body)
}
