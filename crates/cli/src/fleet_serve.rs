//! `familiar fleet serve` — the captain's bridge feed: the ship stores, served to the
//! companion app over HTTP with a bearer, on the household's host. The app's
//! `ShipsFeed`/`CaptainActs` protocols (T-237 B3) are the other end; the shapes here
//! are the contract agreed with the MacOnStick lane on 2026-09-04.
//!
//! Reads: `GET /ships`, `GET /ships/{world}/journal?since=N`,
//! `GET /ships/{world}/proposals`, `GET /ships/{world}/dial`, `GET /ships/{world}/book`
//! (holdings + deliveries), `GET /ships/{world}/fuel` (the fuel conversation's facts),
//! `GET /ships/{world}/brief` and `GET /brief` (one call for the context on screen). Writes:
//! `POST /ships/{world}/approve {id, approved}`, `PUT /ships/{world}/dial {…}`,
//! `PUT /ships/{world}/automations {automations: […]}`, `POST /ships/{world}/rename {name}`,
//! `PUT /ships/{world}/captain {captain}`,
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
    aboard, delivery_totals, estimate_calibration, journal_fills, last_journal_line, lease_expiry,
    paired_ships, persona_for, pid_alive, read_env_value, trade_book, wire_get, Ship,
};

const MAX_REQUEST: usize = 64 * 1024;

/// ℳ per unit of fuel (the content pack's `fuelPricePerUnit`; 2 on LOCAL and PROD).
const FUEL_PRICE_PER_UNIT: i64 = 2;

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

/// An exchange's clock, remembered 30 s, keyed by the exchange — two ships on two
/// worlds (KK II on PROD, the soak ship on LOCAL) run on two clocks.
struct Clock {
    tick: i64,
    tick_seconds: i64,
    at: i64,
}

type Clocks = BTreeMap<String, Clock>;

fn clock(ship: &Ship, cache: &mut Clocks) -> (i64, i64) {
    let now = super::now_secs();
    let key = read_env_value(&ship.dir.join("ucf.env"), "UCF_KEY").unwrap_or_default();
    let server = read_env_value(&ship.dir.join("ucf.env"), "UCF_SERVER")
        .unwrap_or_else(|| ship.captain.server.clone());
    if let Some(c) = cache.get(&server) {
        if now - c.at < 30 {
            return (c.tick, c.tick_seconds);
        }
    }
    if let Ok(v) = wire_get(&server, &key, "/v1/status") {
        let tick = v.get("tick").and_then(Value::as_i64).unwrap_or(0);
        let tick_seconds = v
            .get("tickDurationSec")
            .and_then(Value::as_i64)
            .unwrap_or(180);
        cache.insert(
            server,
            Clock {
                tick,
                tick_seconds,
                at: now,
            },
        );
        return (tick, tick_seconds);
    }
    cache
        .get(&server)
        .map(|c| (c.tick, c.tick_seconds))
        .unwrap_or((0, 180))
}

/// One ship's status row — the same facts `fleet status --json` prints.
fn ship_row(s: &Ship, root: &Path, now: i64) -> Value {
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
    let (aboard_units, aboard_cost) = aboard(&s.dir);
    let (closed_positions, expected, est_realized) = estimate_calibration(&s.dir);
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
        // The world's OWN word for itself (PROD / LOCAL / TEST) — an instance name,
        // never part of a ship's name (Ian, 2026-09-04).
        "world_name": wire_get(&server, &key, "/v1/status")
            .ok()
            .and_then(|v| v.get("worldName").and_then(Value::as_str).map(String::from)),
        "credits": g("credits"), "debt": g("debt"), "fuel": g("fuel"), "fuelCapacity": g("fuelCapacity"),
        "wearBps": g("wearBps"), "fittings": g("fittings"), "titled": g("titled"),
        "holdUsed": g("holdUsed"), "holdCapacity": g("holdCapacity"), "cargo": g("cargo"),
        "hauls": hauls, "freight_paid": paid,
        "trades": {"filled": book.filled, "rejected": book.rejected, "realized": book.realized,
                   "cost_of_sold": book.cost_of_sold, "inventory_cost": aboard_cost,
                   "inventory": aboard_units,
                   "unmatched_units": book.unmatched_units,
                   "unmatched_proceeds": book.unmatched_proceeds,
                   "quoted_basis_lots": book.quoted_basis_lots,
                   "closed_positions": closed_positions,
                   "expected_margin": expected,
                   "realized_on_closed": est_realized},
        "dial": dial.settings,
        "open_proposals": open_proposals,
        // The CAPTAIN's computer (T-236 as Ian ruled it, 2026-09-04): one persona
        // across their whole fleet, with a ship-local record as the fallback.
        "persona": persona_for(root, &s.dir, &s.captain.captain).unwrap_or(Value::Null),
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

fn handle(req: Req, dir: &Path, root: &Path, tok: &str, clk: &mut Clocks) -> (u16, Value) {
    if req.bearer.as_deref() != Some(tok) {
        return (401, json!({"error": "bearer"}));
    }
    let ships = paired_ships(dir, root);
    let now = super::now_secs();
    let segs: Vec<&str> = req.path.trim_matches('/').split('/').collect();
    let find = |id: &str| ships.iter().find(|s| s.world.id == id);
    // Per-ship routes carry that ship's clock at the top level; the fleet list
    // carries each ship's clock on its row.
    let (tick, tick_seconds) = match segs.as_slice() {
        ["ships", id, ..] => find(id).map(|s| clock(s, clk)).unwrap_or((0, 180)),
        _ => (0, 180),
    };
    match (req.method.as_str(), segs.as_slice()) {
        ("GET", ["brief"]) => {
            // The fleet in one call, for a captain looking at the whole list.
            let mut per_captain: BTreeMap<String, Vec<Value>> = BTreeMap::new();
            for s in &ships {
                let (t, ts) = clock(s, clk);
                let open = proposals_with_state(&s.dir, t)
                    .into_iter()
                    .filter(|p| p["state"] == "open")
                    .count();
                let mut row = ship_row(s, root, now);
                row["tick"] = json!(t);
                row["tick_seconds"] = json!(ts);
                row["open_proposals"] = json!(open);
                per_captain
                    .entry(s.captain.captain.clone())
                    .or_default()
                    .push(row);
            }
            (
                200,
                json!({
                    "context": {"kind": "fleet", "captains": per_captain.keys().collect::<Vec<_>>()},
                    "captains": per_captain.iter().map(|(c, rows)| json!({
                        "captain": c,
                        "computer": rows.first().and_then(|r| r["persona"]["name"].as_str()),
                        "ships": rows,
                        "pooled_credits": rows.iter().filter_map(|r| r["credits"].as_i64()).sum::<i64>(),
                        "open_proposals": rows.iter().filter_map(|r| r["open_proposals"].as_i64()).sum::<i64>(),
                    })).collect::<Vec<_>>(),
                }),
            )
        }
        ("GET", ["ships"]) => (
            200,
            json!({"ships": ships
                .iter()
                .map(|s| {
                    let (t, ts) = clock(s, clk);
                    let mut row = ship_row(s, root, now);
                    row["tick"] = json!(t);
                    row["tick_seconds"] = json!(ts);
                    row
                })
                .collect::<Vec<_>>()}),
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
        // The context brief: everything about ONE ship, in one call, shaped for a
        // conversation rather than a dashboard. Ian, 2026-09-04: "the communications
        // with the familiar need to be context aware to the device being used, the
        // ship being viewed, or a fleet being used, or even an individual captain or
        // crew being viewed… context makes all the difference." The client says what
        // the captain is looking at; this answers in that frame.
        ("GET", ["ships", id, "brief"]) => {
            let Some(s) = find(id) else {
                return (404, json!({"error": "no such ship"}));
            };
            let (aboard_units, aboard_cost) = aboard(&s.dir);
            let dial = Dial::load(&s.dir);
            let open: Vec<Value> = proposals_with_state(&s.dir, tick)
                .into_iter()
                .filter(|p| p["state"] == "open")
                .collect();
            // The advice standing right now, folded: one line per thing she is saying,
            // with when she first said it and how often — never the same sentence twice.
            let text = std::fs::read_to_string(s.dir.join("journal.jsonl")).unwrap_or_default();
            let lines: Vec<Value> = text
                .lines()
                .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                .collect();
            let mut advice: BTreeMap<String, (i64, i64, Value)> = BTreeMap::new();
            for v in lines.iter().filter(|v| {
                matches!(
                    v.get("event").and_then(Value::as_str),
                    Some("advice")
                        | Some("merchant-idle")
                        | Some("outfit-idle")
                        | Some("carry-blocked")
                        | Some("distress-hold")
                )
            }) {
                let what = v
                    .get("would")
                    .or_else(|| v.get("why"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if what.is_empty() {
                    continue;
                }
                let t = v.get("tick").and_then(Value::as_i64).unwrap_or(0);
                let e = advice.entry(what).or_insert((t, 0, v.clone()));
                e.1 += 1;
                e.2 = v.clone();
            }
            let standing: Vec<Value> = advice
                .into_iter()
                .map(|(what, (since, times, last))| {
                    json!({"what": what, "since_tick": since, "times": times,
                           "event": last.get("event").cloned().unwrap_or(Value::Null),
                           "surface": last.get("surface").cloned().unwrap_or(Value::Null)})
                })
                .collect();
            let recent: Vec<Value> = lines
                .iter()
                .rev()
                .filter(|v| {
                    !matches!(
                        v.get("event").and_then(Value::as_str),
                        Some("holding")
                            | Some("advice")
                            | Some("merchant-idle")
                            | Some("outfit-idle")
                            | Some("distress-hold")
                            | Some("awaiting-pending-actions")
                            | Some("awaiting-our-own-fold")
                    )
                })
                .take(12)
                .cloned()
                .collect();
            let mut row = ship_row(s, root, now);
            row["tick"] = json!(tick);
            row["tick_seconds"] = json!(tick_seconds);
            (
                200,
                json!({
                    "context": {"kind": "ship", "world": s.world.id, "hull": s.world.label,
                                "captain": s.captain.captain,
                                "computer": persona_for(root, &s.dir, &s.captain.captain)
                                    .and_then(|p| p.get("name").and_then(Value::as_str).map(String::from))},
                    "tick": tick, "tick_seconds": tick_seconds,
                    "ship": row,
                    "aboard": {"units": aboard_units, "cost": aboard_cost},
                    "dial": dial.settings,
                    "open_proposals": open,
                    "standing_advice": standing,
                    "recent": recent,
                }),
            )
        }
        ("GET", ["ships", id, "book"]) => {
            let Some(s) = find(id) else {
                return (404, json!({"error": "no such ship"}));
            };
            let holdings: Value = std::fs::read_to_string(s.dir.join("holdings.json"))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or(json!([]));
            let deliveries: Vec<Value> = std::fs::read_to_string(s.dir.join("deliveries.jsonl"))
                .map(|t| {
                    t.lines()
                        .filter_map(|l| serde_json::from_str(l).ok())
                        .collect()
                })
                .unwrap_or_default();
            (
                200,
                json!({"tick": tick, "tick_seconds": tick_seconds,
                         "holdings": holdings, "deliveries": deliveries}),
            )
        }
        // Everything a conversation about fuel needs, computed rather than recited:
        // where she can reach, what it would cost, what she would do, and why the
        // tanker is refused. Ian, 2026-09-04, after talking to Felix on the iPad:
        // "all it did was read out ships status, nothing particularly useful, no
        // conversation about refueling which is what I was attempting to have."
        ("GET", ["ships", id, "fuel"]) => {
            let Some(s) = find(id) else {
                return (404, json!({"error": "no such ship"}));
            };
            let key = read_env_value(&s.dir.join("ucf.env"), "UCF_KEY").unwrap_or_default();
            let server = read_env_value(&s.dir.join("ucf.env"), "UCF_SERVER")
                .unwrap_or_else(|| s.captain.server.clone());
            let Ok(me) = wire_get(&server, &key, "/v1/me") else {
                return (502, json!({"error": "the exchange did not answer"}));
            };
            let n = |k: &str| me.get(k).and_then(Value::as_i64).unwrap_or(0);
            let here = me.get("docked").and_then(Value::as_str).map(String::from);
            let (fuel, capacity, credits) = (n("fuel"), n("fuelCapacity"), n("credits"));
            let accel = if n("effectiveAccelMilliG") > 0 {
                n("effectiveAccelMilliG")
            } else {
                familiar_whisker::doctrine::REFERENCE_ACCEL_MILLI_G
            };
            let pumps: Vec<String> = match wire_get(&server, &key, "/v1/stations") {
                Ok(Value::Array(rows)) => rows
                    .iter()
                    .filter(|st| {
                        st.get("sellsFuel")
                            .and_then(Value::as_bool)
                            .unwrap_or(false)
                    })
                    .filter_map(|st| st.get("id").and_then(Value::as_str).map(String::from))
                    .collect(),
                _ => Vec::new(),
            };
            let tank_price = (capacity - fuel).max(0) * FUEL_PRICE_PER_UNIT;
            let mut options: Vec<Value> = Vec::new();
            if let Some(here) = here.as_deref() {
                for p in &pumps {
                    if p == here {
                        options.push(json!({"station": p, "here": true, "fuel_cost": 0,
                            "reachable": true, "fill_price": tank_price,
                            "affordable": credits >= tank_price}));
                        continue;
                    }
                    let Ok(route) =
                        wire_get(&server, &key, &format!("/v1/route?from={here}&to={p}"))
                    else {
                        continue;
                    };
                    let quoted = route.get("totalFuel").and_then(Value::as_i64).unwrap_or(0);
                    let ticks = route.get("totalTicks").and_then(Value::as_i64).unwrap_or(0);
                    // The quote is for the reference drive; she flies at her own.
                    let cost = familiar_whisker::doctrine::fuel_at_drive(quoted, accel);
                    options.push(json!({
                        "station": p, "here": false, "fuel_cost": cost, "ticks": ticks,
                        "reachable": familiar_whisker::trade::carry_affordable(cost, fuel),
                        "short_by": (((cost as f64) * 1.2) as i64 - fuel).max(0),
                        "fill_price": tank_price, "affordable": credits >= tank_price,
                    }));
                }
            }
            options.sort_by_key(|o| o["fuel_cost"].as_i64().unwrap_or(i64::MAX));
            let reachable: Vec<&Value> = options
                .iter()
                .filter(|o| o["reachable"].as_bool().unwrap_or(false))
                .collect();
            // What she can sell where she stands, for a captain with no credits.
            let hold: Vec<Value> = me
                .get("cargo")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let saleable = here.as_deref().and_then(|h| {
                wire_get(&server, &key, &format!("/v1/stations/{h}/quotes"))
                    .ok()
                    .map(|q| {
                        let board = familiar_whisker::trade::parse_board(&q);
                        hold.iter()
                            .filter_map(|c| {
                                let good = c.get("good")?.as_str()?;
                                let units = c.get("units")?.as_i64()?;
                                if units <= 0 {
                                    return None;
                                }
                                let row = board.iter().find(|b| b.good == good)?;
                                let can = units.min(row.max_sell.max(0));
                                Some(json!({"good": good, "units": units, "bid": row.bid,
                                            "will_take": can, "worth": can * row.bid}))
                            })
                            .collect::<Vec<_>>()
                    })
            });
            (
                200,
                json!({
                    "tick": tick, "tick_seconds": tick_seconds,
                    "docked": here, "fuel": fuel, "capacity": capacity, "credits": credits,
                    "accel_milli_g": accel, "fill_price_here": tank_price,
                    "pumps": options,
                    "can_reach": reachable.iter().map(|o| o["station"].clone()).collect::<Vec<_>>(),
                    "stranded": here.is_some() && reachable.is_empty(),
                    "saleable_here": saleable,
                    // The tanker, and why the pilot will not call it on a real-time world.
                    "tanker": {
                        "available": true,
                        "pilot_will_call": false,
                        "why": "a PAWS call-out on this world is days of transit and pins the hull                             where it stands until the tanker arrives (metal#59); the pilot holds                             a distress instead, which a fuelable load or a human can still undo",
                    },
                    "if_stranded": "sell what this berth will take for credits, wait for a load whose                                 origin is reachable, ask another captain (metal#75 proposes fuel                                 between hulls), or call the tanker knowingly",
                }),
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
        ("POST", ["ships", id, "rename"]) => {
            let Some(s) = find(id) else {
                return (404, json!({"error": "no such ship"}));
            };
            let Ok(b) = serde_json::from_slice::<Value>(&req.body) else {
                return (400, json!({"error": "json"}));
            };
            let Some(name) = b.get("name").and_then(Value::as_str).map(str::trim) else {
                return (400, json!({"error": "name"}));
            };
            if name.is_empty() || name.chars().count() > 40 {
                return (400, json!({"error": "a name is 1–40 characters"}));
            }
            // The captain's computer, not the hull's: one rename, the whole fleet.
            let out = std::process::Command::new(
                std::env::current_exe().unwrap_or_else(|_| "familiar".into()),
            )
            .args([
                "fleet",
                "rename",
                id,
                name,
                "--data-dir",
                &dir.to_string_lossy(),
            ])
            .output();
            match out {
                Ok(o) if o.status.success() => (
                    200,
                    json!({"tick": tick, "tick_seconds": tick_seconds, "name": name,
                           "captain": s.captain.captain,
                           "output": String::from_utf8_lossy(&o.stdout).trim().to_string()}),
                ),
                Ok(o) => (
                    400,
                    json!({"error": String::from_utf8_lossy(&o.stderr).trim().to_string()}),
                ),
                Err(e) => (500, json!({"error": e.to_string()})),
            }
        }
        ("PUT", ["ships", id, "captain"]) => {
            let Some(s) = find(id) else {
                return (404, json!({"error": "no such ship"}));
            };
            let Ok(b) = serde_json::from_slice::<Value>(&req.body) else {
                return (400, json!({"error": "json"}));
            };
            let Some(captain) = b.get("captain").and_then(Value::as_str).map(str::trim) else {
                return (400, json!({"error": "captain"}));
            };
            if captain.is_empty() || captain.chars().count() > 60 {
                return (400, json!({"error": "a captain is 1–60 characters"}));
            }
            let was = s.captain.captain.clone();
            let Ok(text) = std::fs::read_to_string(s.dir.join("captain.json")) else {
                return (500, json!({"error": "captain.json"}));
            };
            let Ok(mut c) = serde_json::from_str::<Value>(&text) else {
                return (500, json!({"error": "captain.json is not json"}));
            };
            c["captain"] = json!(captain);
            if let Err(e) = std::fs::write(
                s.dir.join("captain.json"),
                serde_json::to_vec_pretty(&c).unwrap_or_default(),
            ) {
                return (500, json!({"error": e.to_string()}));
            }
            // A captain store nobody flies for, whose name no human ever chose, is
            // swept up: leaving it would keep a computer nobody commands. One a
            // human named is kept, however empty — that name was an act.
            let old_store = super::fleet::captain_store(root, &was);
            let still_flown = paired_ships(dir, root)
                .iter()
                .any(|o| o.captain.captain == was);
            let human_named = std::fs::read_to_string(old_store.join("persona-names.jsonl"))
                .map(|t| {
                    t.lines()
                        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                        .any(|v| v.get("actor").and_then(Value::as_str) != Some("pairing"))
                })
                .unwrap_or(false);
            let swept = if !still_flown && !human_named && old_store.is_dir() {
                std::fs::remove_dir_all(&old_store).is_ok()
            } else {
                false
            };
            let joined = persona_for(root, &s.dir, captain)
                .and_then(|p| p.get("name").and_then(Value::as_str).map(String::from));
            (
                200,
                json!({"tick": tick, "tick_seconds": tick_seconds, "captain": captain,
                         "was": was, "computer": joined, "retired_old_captain_store": swept}),
            )
        }
        ("PUT", ["ships", id, "automations"]) => {
            let Some(s) = find(id) else {
                return (404, json!({"error": "no such ship"}));
            };
            let Ok(b) = serde_json::from_slice::<Value>(&req.body) else {
                return (400, json!({"error": "json"}));
            };
            let Some(list) = b.get("automations").and_then(Value::as_array) else {
                return (400, json!({"error": "automations: a list of names"}));
            };
            // Only names the pilot knows: an unknown grant would sit in the file
            // meaning nothing, and the captain would think they had bought something.
            let mut names = Vec::new();
            for v in list {
                let Some(n) = v.as_str() else {
                    return (400, json!({"error": "automations: names are strings"}));
                };
                if familiar_whisker::Automation::parse(n).is_none() {
                    return (400, json!({"error": format!("unknown automation `{n}`")}));
                }
                names.push(n.to_string());
            }
            if let Err(e) = std::fs::write(
                s.dir.join("automations.json"),
                serde_json::to_vec_pretty(&names).unwrap_or_default(),
            ) {
                return (500, json!({"error": e.to_string()}));
            }
            // captain.json remembers what was bought, so `fleet status` and a re-pair
            // agree with the file the pilot reads.
            if let Ok(text) = std::fs::read_to_string(s.dir.join("captain.json")) {
                if let Ok(mut c) = serde_json::from_str::<Value>(&text) {
                    c["automations"] = json!(names);
                    let _ = std::fs::write(
                        s.dir.join("captain.json"),
                        serde_json::to_vec_pretty(&c).unwrap_or_default(),
                    );
                }
            }
            (
                200,
                json!({"tick": tick, "tick_seconds": tick_seconds, "automations": names,
                         "note": "the pilot reads its grants at start — restart it to grant now"}),
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
    let clk = std::sync::Arc::new(std::sync::Mutex::new(Clocks::new()));
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
