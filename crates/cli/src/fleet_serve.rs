//! `familiar fleet serve` — the captain's bridge feed: the ship stores, served to the
//! companion app over HTTP with a bearer, on the household's host. The app's
//! `ShipsFeed`/`CaptainActs` protocols (T-237 B3) are the other end; the shapes here
//! are the contract agreed with the MacOnStick lane on 2026-09-04.
//!
//! Reads: `GET /ships`, `GET /ships/{world}/journal?since=N`,
//! `GET /ships/{world}/proposals`, `GET /ships/{world}/dial`. Writes:
//! `POST /ships/{world}/approve {id, approved}`, `PUT /ships/{world}/dial {…}`,
//! `POST /pair {label, captain, server, key, automations, pilot_args?}`,
//! `POST /unpair {world}`. Every reply carries `tick` and `tick_seconds` from the
//! exchange so the app settles proposal lapse exactly as whisker does. The bearer is
//! `fleet-serve.token` in the data dir (minted 0600 on first run). A plain, bounded
//! HTTP/1.1 server on std: one thread per connection, 64 KiB request cap, no TLS —
//! bind it to loopback or the Tailscale address, never the open internet.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use familiar_whisker::autonomy::{self, Approval, Dial, Level, Surface};
use serde_json::{json, Value};

use super::fleet::{
    delivery_totals, journal_fills, last_journal_line, lease_expiry, paired_ships, pid_alive,
    read_env_value, trade_book, wire_get, Ship,
};

const MAX_REQUEST: usize = 64 * 1024;

fn token(dir: &Path) -> std::io::Result<String> {
    let path = dir.join("fleet-serve.token");
    if let Ok(t) = std::fs::read_to_string(&path) {
        let t = t.trim().to_string();
        if t.len() >= 32 {
            return Ok(t);
        }
    }
    let mut bytes = [0u8; 24];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    let t: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(&path, &t)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(t)
}

struct Req {
    method: String,
    path: String,
    query: BTreeMap<String, String>,
    bearer: Option<String>,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> Option<Req> {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .ok()?;
    let mut buf = Vec::with_capacity(4096);
    let mut tmp = [0u8; 4096];
    let header_end;
    loop {
        let n = stream.read(&mut tmp).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = i + 4;
            break;
        }
        if buf.len() > MAX_REQUEST {
            return None;
        }
    }
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = head.split("\r\n");
    let start = lines.next()?;
    let mut parts = start.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let mut bearer = None;
    let mut content_length = 0usize;
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            let k = k.trim().to_ascii_lowercase();
            let v = v.trim();
            if k == "authorization" {
                bearer = v.strip_prefix("Bearer ").map(|s| s.trim().to_string());
            } else if k == "content-length" {
                content_length = v.parse().unwrap_or(0);
            }
        }
    }
    if content_length > MAX_REQUEST {
        return None;
    }
    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut tmp).ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(content_length);
    let (path, q) = target.split_once('?').unwrap_or((&target, ""));
    let query = q
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Some(Req {
        method,
        path: path.to_string(),
        query,
        bearer,
        body,
    })
}

fn respond(stream: &mut TcpStream, status: u16, body: &Value) {
    let text = body.to_string();
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Error",
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        text.len(),
        text
    );
    let _ = stream.flush();
}

/// The exchange's clock, from the first paired ship that answers, remembered 30 s.
struct Clock {
    tick: i64,
    tick_seconds: i64,
    at: i64,
}

fn clock(ships: &[Ship], cache: &mut Option<Clock>) -> (i64, i64) {
    let now = super::now_secs();
    if let Some(c) = cache {
        if now - c.at < 30 {
            return (c.tick, c.tick_seconds);
        }
    }
    for s in ships {
        let key = read_env_value(&s.dir.join("ucf.env"), "UCF_KEY").unwrap_or_default();
        let server = read_env_value(&s.dir.join("ucf.env"), "UCF_SERVER")
            .unwrap_or_else(|| s.captain.server.clone());
        if let Ok(v) = wire_get(&server, &key, "/v1/status") {
            let tick = v.get("tick").and_then(Value::as_i64).unwrap_or(0);
            let tick_seconds = v
                .get("tickDurationSec")
                .and_then(Value::as_i64)
                .unwrap_or(180);
            *cache = Some(Clock {
                tick,
                tick_seconds,
                at: now,
            });
            return (tick, tick_seconds);
        }
    }
    cache
        .as_ref()
        .map(|c| (c.tick, c.tick_seconds))
        .unwrap_or((0, 180))
}

/// One ship's status row — the same facts `fleet status --json` prints.
fn ship_row(s: &Ship, now: i64) -> Value {
    let key = read_env_value(&s.dir.join("ucf.env"), "UCF_KEY").unwrap_or_default();
    let server = read_env_value(&s.dir.join("ucf.env"), "UCF_SERVER")
        .unwrap_or_else(|| s.captain.server.clone());
    let me = wire_get(&server, &key, "/v1/me").ok();
    let g = |k: &str| {
        me.as_ref()
            .and_then(|m| m.get(k).cloned())
            .unwrap_or(Value::Null)
    };
    let (hauls, paid) = delivery_totals(&s.dir);
    let mut fills: Vec<Value> = journal_fills(&s.dir)
        .as_array()
        .cloned()
        .unwrap_or_default();
    if let Ok(Value::Array(rows)) = wire_get(&server, &key, "/v1/receipts") {
        for r in rows {
            let dup = fills.iter().any(|f| {
                f["good"] == r["good"]
                    && f["side"] == r["side"]
                    && f["units"] == r["units"]
                    && (f["tick"].as_i64().unwrap_or(0) - r["tick"].as_i64().unwrap_or(0)).abs()
                        <= 3
            });
            if !dup {
                fills.push(r);
            }
        }
    }
    let book = trade_book(&Value::Array(fills));
    let last = last_journal_line(&s.dir);
    let dial = Dial::load(&s.dir);
    let open_proposals = {
        let approvals = autonomy::load_approvals(&s.dir);
        autonomy::load_proposals(&s.dir)
            .iter()
            .filter(|p| !approvals.iter().any(|a| a.id == p.id))
            .count()
    };
    json!({
        "world": s.world.id, "label": s.world.label, "captain": s.captain.captain,
        "key_id": s.captain.key_id, "server": server,
        "automations": std::fs::read_to_string(s.dir.join("automations.json")).ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok()).unwrap_or(Value::Null),
        "pilot_pid": pid_alive(&s.dir),
        "lease_expires_at": lease_expiry(&s.dir),
        "lease_expires_in_h": lease_expiry(&s.dir).map(|x| (x - now) / 3600),
        "ship": g("shipName"), "docked": g("docked"), "route": g("route"),
        "credits": g("credits"), "debt": g("debt"), "fuel": g("fuel"), "fuelCapacity": g("fuelCapacity"),
        "wearBps": g("wearBps"), "fittings": g("fittings"), "titled": g("titled"),
        "holdUsed": g("holdUsed"), "holdCapacity": g("holdCapacity"), "cargo": g("cargo"),
        "hauls": hauls, "freight_paid": paid,
        "trades": {"filled": book.filled, "rejected": book.rejected, "realized": book.realized,
                   "cost_of_sold": book.cost_of_sold, "inventory_cost": book.inventory_cost,
                   "inventory": book.inventory},
        "dial": dial.settings,
        "open_proposals": open_proposals,
        "last_event": last.as_ref().and_then(|v| v.get("event").cloned()).unwrap_or(Value::Null),
        "last_at": last.as_ref().and_then(|v| v.get("at").cloned()).unwrap_or(Value::Null),
        "reachable": me.is_some(),
    })
}

fn proposals_with_state(ship_dir: &Path, tick: i64) -> Vec<Value> {
    let approvals = autonomy::load_approvals(ship_dir);
    autonomy::load_proposals(ship_dir)
        .into_iter()
        .map(|p| {
            let answer = approvals.iter().rev().find(|a| a.id == p.id);
            let state = match answer {
                Some(a) if a.approved => "approved",
                Some(_) => "denied",
                None if tick > p.expires_tick => "lapsed",
                None => "open",
            };
            let mut v = serde_json::to_value(&p).unwrap_or(Value::Null);
            v["state"] = json!(state);
            v["answered_at"] = answer.map(|a| json!(a.at)).unwrap_or(Value::Null);
            v
        })
        .collect()
}

fn handle(req: Req, dir: &Path, root: &Path, tok: &str, clk: &mut Option<Clock>) -> (u16, Value) {
    if req.bearer.as_deref() != Some(tok) {
        return (401, json!({"error": "bearer"}));
    }
    let ships = paired_ships(dir, root);
    let (tick, tick_seconds) = clock(&ships, clk);
    let now = super::now_secs();
    let segs: Vec<&str> = req.path.trim_matches('/').split('/').collect();
    let find = |id: &str| ships.iter().find(|s| s.world.id == id);
    match (req.method.as_str(), segs.as_slice()) {
        ("GET", ["ships"]) => (
            200,
            json!({"tick": tick, "tick_seconds": tick_seconds,
                   "ships": ships.iter().map(|s| ship_row(s, now)).collect::<Vec<_>>()}),
        ),
        ("GET", ["ships", id, "journal"]) => {
            let Some(s) = find(id) else {
                return (404, json!({"error": "no such ship"}));
            };
            let since: usize = req
                .query
                .get("since")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let limit: usize = req
                .query
                .get("limit")
                .and_then(|v| v.parse().ok())
                .unwrap_or(500)
                .min(2000);
            let text = std::fs::read_to_string(s.dir.join("journal.jsonl")).unwrap_or_default();
            let all: Vec<&str> = text.lines().collect();
            let start = since.min(all.len());
            let end = (start + limit).min(all.len());
            let lines: Vec<Value> = all[start..end]
                .iter()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect();
            (
                200,
                json!({"tick": tick, "tick_seconds": tick_seconds, "since": start, "next": end,
                         "total": all.len(), "lines": lines}),
            )
        }
        ("GET", ["ships", id, "proposals"]) => {
            let Some(s) = find(id) else {
                return (404, json!({"error": "no such ship"}));
            };
            (
                200,
                json!({"tick": tick, "tick_seconds": tick_seconds,
                         "proposals": proposals_with_state(&s.dir, tick)}),
            )
        }
        ("GET", ["ships", id, "dial"]) => {
            let Some(s) = find(id) else {
                return (404, json!({"error": "no such ship"}));
            };
            let dial = Dial::load(&s.dir);
            let bought: Value = std::fs::read_to_string(s.dir.join("automations.json"))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or(json!([]));
            let effective: BTreeMap<String, &str> = Surface::all()
                .iter()
                .map(|x| (x.key(), dial.level(*x).name()))
                .collect();
            (
                200,
                json!({"tick": tick, "tick_seconds": tick_seconds, "dial": dial.settings,
                         "bought": bought, "effective": effective}),
            )
        }
        ("POST", ["ships", id, "approve"]) => {
            let Some(s) = find(id) else {
                return (404, json!({"error": "no such ship"}));
            };
            let Ok(b) = serde_json::from_slice::<Value>(&req.body) else {
                return (400, json!({"error": "json"}));
            };
            let Some(pid) = b.get("id").and_then(Value::as_str) else {
                return (400, json!({"error": "id"}));
            };
            let approved = b.get("approved").and_then(Value::as_bool).unwrap_or(false);
            if !autonomy::load_proposals(&s.dir).iter().any(|p| p.id == pid) {
                return (404, json!({"error": "no such proposal"}));
            }
            let a = Approval {
                id: pid.to_string(),
                approved,
                at: now,
            };
            autonomy::append_approval(&s.dir, &a);
            (
                200,
                json!({"tick": tick, "tick_seconds": tick_seconds, "approval": a}),
            )
        }
        ("PUT", ["ships", id, "dial"]) => {
            let Some(s) = find(id) else {
                return (404, json!({"error": "no such ship"}));
            };
            let Ok(b) = serde_json::from_slice::<Value>(&req.body) else {
                return (400, json!({"error": "json"}));
            };
            let Some(obj) = b.as_object() else {
                return (400, json!({"error": "object"}));
            };
            let mut dial = Dial::default();
            for (k, v) in obj {
                let Some(level) = v.as_str().and_then(Level::parse) else {
                    return (
                        400,
                        json!({"error": format!("`{k}`: level must be advise|confirm|auto")}),
                    );
                };
                if let Err(e) = dial.set(k, level) {
                    return (400, json!({"error": e}));
                }
            }
            if let Err(e) = dial.save(&s.dir) {
                return (500, json!({"error": e.to_string()}));
            }
            (
                200,
                json!({"tick": tick, "tick_seconds": tick_seconds, "dial": dial.settings}),
            )
        }
        ("POST", ["pair"]) => {
            let Ok(b) = serde_json::from_slice::<Value>(&req.body) else {
                return (400, json!({"error": "json"}));
            };
            let get = |k: &str| b.get(k).and_then(Value::as_str).map(String::from);
            let (Some(label), Some(captain), Some(server), Some(key)) =
                (get("label"), get("captain"), get("server"), get("key"))
            else {
                return (400, json!({"error": "label, captain, server, key"}));
            };
            // The same ceremony as `fleet pair`, run as that command so one code path
            // owns commissioning; the key travels by a 0600 temp file, never argv.
            let tmp = dir.join(format!(".pair-{}.key", now));
            if std::fs::write(&tmp, &key).is_err() {
                return (500, json!({"error": "key file"}));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
            }
            let autos = b
                .get("automations")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .unwrap_or_else(|| "freight".into());
            let mut cmd = std::process::Command::new(
                std::env::current_exe().unwrap_or_else(|_| "familiar".into()),
            );
            cmd.args([
                "fleet",
                "pair",
                "--label",
                &label,
                "--captain",
                &captain,
                "--server",
                &server,
                "--key-file",
                &tmp.to_string_lossy(),
                "--automations",
                &autos,
                "--data-dir",
                &dir.to_string_lossy(),
            ]);
            if let Some(pa) = get("pilot_args") {
                cmd.args(["--pilot-args", &pa]);
            }
            let out = cmd.output();
            let _ = std::fs::remove_file(&tmp);
            match out {
                Ok(o) if o.status.success() => {
                    let text = String::from_utf8_lossy(&o.stdout).to_string();
                    let world = text
                        .split_whitespace()
                        .find(|w| w.starts_with("world-"))
                        .unwrap_or("")
                        .to_string();
                    (
                        200,
                        json!({"tick": tick, "tick_seconds": tick_seconds, "world": world, "output": text}),
                    )
                }
                Ok(o) => (
                    400,
                    json!({"error": String::from_utf8_lossy(&o.stderr).to_string()}),
                ),
                Err(e) => (500, json!({"error": e.to_string()})),
            }
        }
        ("POST", ["unpair"]) => {
            let Ok(b) = serde_json::from_slice::<Value>(&req.body) else {
                return (400, json!({"error": "json"}));
            };
            let Some(world) = b.get("world").and_then(Value::as_str) else {
                return (400, json!({"error": "world"}));
            };
            if find(world).is_none() {
                return (404, json!({"error": "no such ship"}));
            }
            let out = std::process::Command::new(
                std::env::current_exe().unwrap_or_else(|_| "familiar".into()),
            )
            .args([
                "fleet",
                "unpair",
                world,
                "--data-dir",
                &dir.to_string_lossy(),
            ])
            .output();
            match out {
                Ok(o) if o.status.success() => (
                    200,
                    json!({"tick": tick, "tick_seconds": tick_seconds,
                    "output": String::from_utf8_lossy(&o.stdout).to_string()}),
                ),
                Ok(o) => (
                    400,
                    json!({"error": String::from_utf8_lossy(&o.stderr).to_string()}),
                ),
                Err(e) => (500, json!({"error": e.to_string()})),
            }
        }
        ("GET", _) | ("POST", _) | ("PUT", _) => (404, json!({"error": "no such route"})),
        _ => (405, json!({"error": "method"})),
    }
}

pub(crate) fn serve(dir: &Path, root: &Path, bind: &str) -> ExitCode {
    let tok = match token(dir) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("fleet serve: token: {e}");
            return ExitCode::FAILURE;
        }
    };
    let listener = match TcpListener::bind(bind) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("fleet serve: bind {bind}: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "fleet serve: listening on {bind} — bearer in {} ({}…{}) — GET /ships, /ships/{{world}}/journal|proposals|dial; POST approve, PUT dial, POST pair|unpair",
        dir.join("fleet-serve.token").display(),
        &tok[..4],
        &tok[tok.len() - 4..]
    );
    let dir = dir.to_path_buf();
    let root = root.to_path_buf();
    let clk = std::sync::Arc::new(std::sync::Mutex::new(None::<Clock>));
    for conn in listener.incoming() {
        let Ok(mut stream) = conn else { continue };
        let (dir, root, tok, clk) = (dir.clone(), root.clone(), tok.clone(), clk.clone());
        std::thread::spawn(move || {
            let Some(req) = read_request(&mut stream) else {
                respond(&mut stream, 400, &json!({"error": "request"}));
                return;
            };
            let (status, body) = {
                let mut c = clk.lock().unwrap_or_else(|e| e.into_inner());
                handle(req, &dir, &root, &tok, &mut c)
            };
            respond(&mut stream, status, &body);
        });
    }
    ExitCode::SUCCESS
}
