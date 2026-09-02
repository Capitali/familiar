//! `familiar fleet` — the captain's ships, and the pilots that fly them.
//!
//! A captain in the game buys the familiar for a ship they own or lease and hands
//! it a key (today a trading key; tomorrow the co-pilot key Jeff mints with the
//! purchased automation scopes — ucf-exchange#15). PAIRING turns that hand-off into
//! a ship world of its own (ADR-0045: the ship's store holds the key, the grants and
//! the journal; the household holds only the provisioning record), leased by the
//! household's word. UNPAIRING is revocation: the pilot stops, the key is destroyed,
//! the record stays. STATUS reads every ship's own ledger on the wire and books
//! them per captain — earnings POOL within one captain's ships and never across
//! captains (Ian, 2026-09-01, the fleet money boundary). RUN keeps one pilot per
//! paired ship alive and, when told to, renews leases before they lapse.
//!
//! Ian, 2026-09-02: "The familiar needs to work in conjunction to a captain in the
//! game who purchased the familiar for their owned or leased ship … we need it all
//! working first" — this is the working half; the in-game purchase and key minting
//! are Jeff's add-in.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use familiar_mcp::{http, Url};
use familiar_world::instance::{self, Lifecycle, WorldInstance};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Who the ship flies for, written at pairing (`captain.json` in the ship store).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Captain {
    pub captain: String,
    /// The key's public id (`keyId`, the first 8 hex of the secret) — never the secret.
    pub key_id: String,
    pub server: String,
    pub automations: Vec<String>,
    pub paired_at: i64,
    /// Extra arguments for this ship's pilot (a LOCAL soak passes `--allow-paws`
    /// and a short `--interval-floor`; a PROD hull passes nothing).
    #[serde(default)]
    pub pilot_args: Vec<String>,
}

fn read_env_value(path: &Path, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .filter_map(|l| l.split_once('='))
        .find(|(k, _)| k.trim() == key)
        .map(|(_, v)| v.trim().to_string())
}

fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// GET on the exchange with the ship's own key. Bounded: one call, one timeout.
fn wire_get(server: &str, key: &str, path: &str) -> Result<Value, String> {
    let url = Url::parse(&format!("{}{}", server.trim_end_matches('/'), path))
        .map_err(|e| format!("{e:?}"))?;
    let headers = vec![
        ("Authorization".to_string(), format!("Bearer {key}")),
        ("X-UCF-App".to_string(), "familiar-fleet".to_string()),
    ];
    let resp = http::get(&url, &headers).map_err(|e| format!("{e:?}"))?;
    if !(200..300).contains(&resp.status) {
        return Err(format!("HTTP {}", resp.status));
    }
    serde_json::from_slice(&resp.body).map_err(|e| e.to_string())
}

/// Issue a fresh lease for a ship from the household's boundary and key — the same
/// act as `familiar world lease`, callable by the supervisor when a human has said
/// `--renew`.
fn issue_lease(dir: &Path, ship_dir: &Path, id: &str, ttl_hours: i64) -> Result<i64, String> {
    let root_boundary = familiar_kernel::boundary::load(dir).map_err(|e| e.to_string())?;
    let key = familiar_mesh::node::NodeKey::load_or_mint(dir, "").map_err(|e| e.to_string())?;
    let now = super::now_secs();
    let signed = familiar_world::lease::issue(
        &root_boundary,
        id,
        ttl_hours.saturating_mul(3600),
        now,
        &key,
    )
    .map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(&signed).map_err(|e| e.to_string())?;
    std::fs::write(ship_dir.join("lease.json"), bytes).map_err(|e| e.to_string())?;
    Ok(now + ttl_hours * 3600)
}

/// When the ship's current lease expires, if it can be read.
fn lease_expiry(ship_dir: &Path) -> Option<i64> {
    let raw = std::fs::read_to_string(ship_dir.join("lease.json")).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;
    let inner: Value = serde_json::from_str(v.get("lease_json")?.as_str()?).ok()?;
    inner.get("expires_at")?.as_i64()
}

fn pid_alive(ship_dir: &Path) -> Option<u32> {
    let pid: u32 = std::fs::read_to_string(ship_dir.join("whisker.pid"))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    #[cfg(unix)]
    {
        // kill(pid, 0): no signal, just "does it exist and may I signal it".
        let alive = unsafe { libc_kill(pid as i32, 0) } == 0;
        return alive.then_some(pid);
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(unix)]
extern "C" {
    #[link_name = "kill"]
    fn libc_kill(pid: i32, sig: i32) -> i32;
}

fn stop_pilot(ship_dir: &Path) -> bool {
    match pid_alive(ship_dir) {
        Some(pid) => {
            #[cfg(unix)]
            unsafe {
                libc_kill(pid as i32, 15);
            }
            let _ = std::fs::remove_file(ship_dir.join("whisker.pid"));
            true
        }
        None => false,
    }
}

/// A paired ship: the record, its store, and who it flies for.
struct Ship {
    world: WorldInstance,
    dir: PathBuf,
    captain: Captain,
}

fn paired_ships(dir: &Path, root: &Path) -> Vec<Ship> {
    let Ok(all) = instance::load(dir) else {
        return Vec::new();
    };
    all.into_iter()
        .filter(|w| w.lifecycle != Lifecycle::Decommissioned)
        .filter_map(|w| {
            let ship_dir = root.join(&w.id);
            let captain: Captain =
                serde_json::from_str(&std::fs::read_to_string(ship_dir.join("captain.json")).ok()?)
                    .ok()?;
            ship_dir.join("ucf.env").exists().then_some(Ship {
                world: w,
                dir: ship_dir,
                captain,
            })
        })
        .collect()
}

fn last_journal_line(ship_dir: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(ship_dir.join("journal.jsonl")).ok()?;
    text.lines()
        .rev()
        .find_map(|l| serde_json::from_str::<Value>(l).ok())
}

/// The merchant's book from the exchange's own receipt trail: realized profit on
/// sold lots (FIFO cost), what it cost, and what is still aboard at cost. The
/// trail covers roughly the last day of ticks, so this is a rolling window on a
/// fast world and the whole story on a slow one.
#[derive(Debug, Default, Clone, Serialize)]
struct TradeBook {
    filled: i64,
    rejected: i64,
    realized: i64,
    cost_of_sold: i64,
    inventory_cost: i64,
    inventory: BTreeMap<String, i64>,
}

fn trade_book(receipts: &Value) -> TradeBook {
    let mut book = TradeBook::default();
    let Some(rows) = receipts.as_array() else {
        return book;
    };
    let mut fills: Vec<&Value> = rows
        .iter()
        .filter(|r| {
            let filled = r.get("outcome").and_then(Value::as_str) == Some("filled");
            if !filled {
                book.rejected += 1;
            }
            filled
        })
        .collect();
    fills.sort_by_key(|r| r.get("tick").and_then(Value::as_i64).unwrap_or(0));
    // good → lots of (units, cost per unit ×1000 for integer arithmetic)
    let mut lots: BTreeMap<String, std::collections::VecDeque<(i64, i64)>> = BTreeMap::new();
    for r in fills {
        book.filled += 1;
        let good = r
            .get("good")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let units = r.get("units").and_then(Value::as_i64).unwrap_or(0);
        let total = r.get("total").and_then(Value::as_i64).unwrap_or(0);
        if units <= 0 {
            continue;
        }
        if r.get("side").and_then(Value::as_str) == Some("buy") {
            lots.entry(good)
                .or_default()
                .push_back((units, total * 1000 / units));
        } else {
            let mut left = units;
            let mut cost_milli = 0;
            if let Some(q) = lots.get_mut(&good) {
                while left > 0 {
                    let Some(front) = q.front_mut() else { break };
                    let take = front.0.min(left);
                    cost_milli += take * front.1;
                    front.0 -= take;
                    left -= take;
                    if front.0 == 0 {
                        q.pop_front();
                    }
                }
            }
            let cost = cost_milli / 1000;
            book.realized += total - cost;
            book.cost_of_sold += cost;
        }
    }
    for (good, q) in &lots {
        let units: i64 = q.iter().map(|(u, _)| *u).sum();
        if units > 0 {
            book.inventory.insert(good.clone(), units);
            book.inventory_cost += q.iter().map(|(u, c)| u * c).sum::<i64>() / 1000;
        }
    }
    book
}

/// The ship's own delivery record, summed: hauls and freight paid.
fn delivery_totals(ship_dir: &Path) -> (i64, i64) {
    let Ok(text) = std::fs::read_to_string(ship_dir.join("deliveries.jsonl")) else {
        return (0, 0);
    };
    let mut n = 0;
    let mut paid = 0;
    for l in text.lines() {
        if let Ok(v) = serde_json::from_str::<Value>(l) {
            n += 1;
            paid += v.get("paid").and_then(Value::as_i64).unwrap_or(0);
        }
    }
    (n, paid)
}

pub fn cmd_fleet(args: &[String]) -> ExitCode {
    let sub = args.first().map(String::as_str).unwrap_or("status");
    let f = super::flags(args);
    let dir = familiar_kernel::store::data_dir(f.get("data-dir").map(String::as_str));
    let root = super::world_store_root(&dir, f.get("store-root").map(String::as_str));
    let positional: Vec<&String> = {
        let mut out = Vec::new();
        let mut skip_next = false;
        for a in args.iter().skip(1) {
            if skip_next {
                skip_next = false;
                continue;
            }
            if let Some(key) = a.strip_prefix("--") {
                skip_next = !key.contains('=');
                continue;
            }
            out.push(a);
        }
        out
    };

    match sub {
        // ── pair: a captain's key becomes a ship world ─────────────────────────
        "pair" => {
            let (Some(label), Some(captain), Some(server)) =
                (f.get("label"), f.get("captain"), f.get("server"))
            else {
                eprintln!(
                    "fleet pair: --label <ship name> --captain <who> --server <exchange url> \
                     --key <ucfk_…> | --key-file <path> [--automations freight,trade,outfit] \
                     [--ttl-hours 24]"
                );
                return ExitCode::FAILURE;
            };
            let key = match (f.get("key"), f.get("key-file")) {
                (Some(k), _) => k.trim().to_string(),
                (None, Some(p)) => match std::fs::read_to_string(p) {
                    Ok(s) => s.trim().to_string(),
                    Err(e) => {
                        eprintln!("fleet pair: --key-file: {e}");
                        return ExitCode::FAILURE;
                    }
                },
                _ => {
                    eprintln!("fleet pair: the captain's key is needed — --key or --key-file");
                    return ExitCode::FAILURE;
                }
            };
            if !key.starts_with("ucfk_") || key.len() < 16 {
                eprintln!("fleet pair: that is not an exchange key (ucfk_…)");
                return ExitCode::FAILURE;
            }
            let automations: Vec<String> = f
                .get("automations")
                .map(|s| {
                    s.split(',')
                        .map(|a| a.trim().to_string())
                        .filter(|a| !a.is_empty())
                        .collect()
                })
                .unwrap_or_else(|| vec!["freight".to_string()]);
            let commissioner = f
                .get("commissioner")
                .cloned()
                .or_else(|| familiar_kernel::identity::current(&dir))
                .unwrap_or_default();
            if commissioner.is_empty() || commissioner == "observer" {
                eprintln!("fleet pair: no established commissioner — pass --commissioner <human>");
                return ExitCode::FAILURE;
            }
            // The key answers for itself before anything is written: who is this, on
            // which exchange, with what ship.
            let me = match wire_get(server, &key, "/v1/me") {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("fleet pair: the key does not answer on {server}: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let ship_name = me
                .get("shipName")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let (w, ship_dir) = match instance::commission(
                &dir,
                &root,
                label,
                &commissioner,
                server,
                super::now_secs(),
            ) {
                Ok(x) => x,
                Err(e) => {
                    eprintln!("fleet pair: {e}");
                    return ExitCode::FAILURE;
                }
            };
            // The issuer's public identity, so the ship can verify leases (as the
            // world ceremony does).
            if let Ok(k) = familiar_mesh::node::NodeKey::load_or_mint(&dir, "") {
                if let Ok(bytes) = serde_json::to_vec_pretty(&k.identity()) {
                    let _ = std::fs::write(ship_dir.join("issuer.json"), bytes);
                }
            }
            let env = format!("UCF_KEY={key}\nUCF_SERVER={server}\n");
            if let Err(e) = write_private(&ship_dir.join("ucf.env"), env.as_bytes()) {
                eprintln!("fleet pair: writing the key into the ship store: {e}");
                return ExitCode::FAILURE;
            }
            let _ = std::fs::write(
                ship_dir.join("automations.json"),
                serde_json::to_vec_pretty(&automations).unwrap_or_default(),
            );
            let key_id = key
                .trim_start_matches("ucfk_")
                .chars()
                .take(8)
                .collect::<String>();
            let record = Captain {
                captain: captain.clone(),
                key_id: key_id.clone(),
                server: server.clone(),
                automations: automations.clone(),
                paired_at: super::now_secs(),
                pilot_args: f
                    .get("pilot-args")
                    .map(|s| s.split_whitespace().map(String::from).collect())
                    .unwrap_or_default(),
            };
            let _ = std::fs::write(
                ship_dir.join("captain.json"),
                serde_json::to_vec_pretty(&record).unwrap_or_default(),
            );
            let ttl: i64 = f
                .get("ttl-hours")
                .and_then(|s| s.parse().ok())
                .unwrap_or(24);
            match issue_lease(&dir, &ship_dir, &w.id, ttl) {
                Ok(exp) => println!(
                    "paired {} — \"{}\" for captain {captain}, leased to {exp}",
                    w.id, w.label
                ),
                Err(e) => eprintln!("paired {} but the lease failed: {e}", w.id),
            }
            println!("  ship on the exchange: {ship_name} (key {key_id}, {server})");
            println!("  automations granted: {}", automations.join(", "));
            println!("  store: {}", ship_dir.display());
            println!("  next: `familiar fleet run` keeps a pilot on her; `familiar fleet unpair {}` revokes.", w.id);
            ExitCode::SUCCESS
        }

        // ── unpair: revocation ─────────────────────────────────────────────────
        "unpair" => {
            let Some(id) = positional.first() else {
                eprintln!("fleet unpair: `familiar fleet unpair <world-id>`");
                return ExitCode::FAILURE;
            };
            let ship_dir = root.join(id.as_str());
            let stopped = stop_pilot(&ship_dir);
            let key_gone = std::fs::remove_file(ship_dir.join("ucf.env")).is_ok();
            match instance::decommission(&dir, id) {
                Err(e) => {
                    eprintln!("fleet unpair: {e}");
                    ExitCode::FAILURE
                }
                Ok(w) => {
                    println!(
                        "unpaired {} — pilot {}, key {}, authority ended (epoch {}). The journal and \
                         delivery record stay for the captain.",
                        w.id,
                        if stopped { "stopped" } else { "was not running" },
                        if key_gone { "destroyed" } else { "was not held" },
                        w.grant_epoch
                    );
                    ExitCode::SUCCESS
                }
            }
        }

        // ── status: every ship, booked per captain ─────────────────────────────
        "status" => {
            let ships = paired_ships(&dir, &root);
            if ships.is_empty() {
                println!("fleet: no paired ships. `familiar fleet pair --label … --captain … --server … --key …`");
                return ExitCode::SUCCESS;
            }
            let json_out = f.contains_key("json");
            let now = super::now_secs();
            let mut rows: Vec<Value> = Vec::new();
            // credits, debt, hauls, freight paid, realized trade P&L, inventory at cost
            let mut per_captain: BTreeMap<String, (i64, i64, i64, i64, i64, i64)> = BTreeMap::new();
            for s in &ships {
                let key = read_env_value(&s.dir.join("ucf.env"), "UCF_KEY").unwrap_or_default();
                let server = read_env_value(&s.dir.join("ucf.env"), "UCF_SERVER")
                    .unwrap_or_else(|| s.captain.server.clone());
                let me = wire_get(&server, &key, "/v1/me").ok();
                let book = wire_get(&server, &key, "/v1/receipts")
                    .map(|r| trade_book(&r))
                    .unwrap_or_default();
                let g = |k: &str| {
                    me.as_ref()
                        .and_then(|m| m.get(k).cloned())
                        .unwrap_or(Value::Null)
                };
                let (hauls, paid) = delivery_totals(&s.dir);
                let credits = g("credits").as_i64().unwrap_or(0);
                let debt = g("debt").as_i64().unwrap_or(0);
                let e = per_captain.entry(s.captain.captain.clone()).or_default();
                e.0 += credits;
                e.1 += debt;
                e.2 += hauls;
                e.3 += paid;
                e.4 += book.realized;
                e.5 += book.inventory_cost;
                let last = last_journal_line(&s.dir);
                let expiry = lease_expiry(&s.dir);
                rows.push(json!({
                    "world": s.world.id, "label": s.world.label, "captain": s.captain.captain,
                    "key_id": s.captain.key_id, "server": server,
                    "automations": std::fs::read_to_string(s.dir.join("automations.json")).ok()
                        .and_then(|t| serde_json::from_str::<Value>(&t).ok()).unwrap_or(Value::Null),
                    "pilot_pid": pid_alive(&s.dir),
                    "lease_expires_in_h": expiry.map(|x| (x - now) / 3600),
                    "ship": g("shipName"), "docked": g("docked"), "credits": credits, "debt": debt,
                    "fuel": g("fuel"), "wearBps": g("wearBps"), "fittings": g("fittings"),
                    "titled": g("titled"), "hauls": hauls, "freight_paid": paid,
                    "trades": {"filled": book.filled, "rejected": book.rejected, "realized": book.realized,
                               "cost_of_sold": book.cost_of_sold,
                               "margin_pct": if book.cost_of_sold > 0 { book.realized * 100 / book.cost_of_sold } else { 0 },
                               "inventory_cost": book.inventory_cost, "inventory": book.inventory},
                    "last_event": last.as_ref().and_then(|v| v.get("event").cloned()).unwrap_or(Value::Null),
                    "last_at": last.as_ref().and_then(|v| v.get("at").cloned()).unwrap_or(Value::Null),
                    "reachable": me.is_some(),
                }));
            }
            if json_out {
                println!(
                    "{}",
                    json!({"ships": rows, "captains": per_captain.iter().map(|(c, (cr, d, h, p, rz, inv))| json!({
                        "captain": c, "pooled_credits": cr, "debt": d, "hauls": h, "freight_paid": p,
                        "trade_realized": rz, "inventory_cost": inv})).collect::<Vec<_>>()})
                );
                return ExitCode::SUCCESS;
            }
            for r in &rows {
                let pilot = match r.get("pilot_pid").and_then(Value::as_u64) {
                    Some(p) => format!("pilot {p}"),
                    None => "NO PILOT".to_string(),
                };
                let lease = match r.get("lease_expires_in_h").and_then(Value::as_i64) {
                    Some(h) if h >= 0 => format!("lease {h}h"),
                    Some(_) => "LEASE EXPIRED".to_string(),
                    None => "no lease".to_string(),
                };
                println!(
                    "{} \"{}\" — captain {} — {} — {} — {}",
                    r["world"].as_str().unwrap_or(""),
                    r["label"].as_str().unwrap_or(""),
                    r["captain"].as_str().unwrap_or(""),
                    pilot,
                    lease,
                    if r["reachable"].as_bool().unwrap_or(false) {
                        "on the wire"
                    } else {
                        "UNREACHABLE"
                    }
                );
                if r["reachable"].as_bool().unwrap_or(false) {
                    println!(
                        "    {} — {} — ℳ{} (debt {}) — fuel {} — wear {}bps — fittings {} — {} hauls, ℳ{} freight",
                        r["ship"].as_str().unwrap_or("?"),
                        r["docked"].as_str().unwrap_or("under way"),
                        r["credits"],
                        r["debt"],
                        r["fuel"],
                        r["wearBps"],
                        r["fittings"],
                        r["hauls"],
                        r["freight_paid"]
                    );
                }
                let t = &r["trades"];
                println!(
                    "    trades: {} filled — realized ℳ{} on ℳ{} sold ({}%) — aboard at cost ℳ{} {}",
                    t["filled"], t["realized"], t["cost_of_sold"], t["margin_pct"], t["inventory_cost"], t["inventory"]
                );
                println!(
                    "    last: {} — automations {}",
                    r["last_event"], r["automations"]
                );
            }
            println!("— per captain (pooled within a captain, never across) —");
            for (c, (cr, d, h, p, rz, inv)) in &per_captain {
                println!(
                    "  {c}: ℳ{cr} pooled, debt {d}, {h} hauls, ℳ{p} freight paid, trades realized ℳ{rz}, ℳ{inv} aboard at cost"
                );
            }
            ExitCode::SUCCESS
        }

        // ── run: one pilot per ship, kept alive; leases renewed on a human's word ──
        "run" => {
            let renew = f.contains_key("renew");
            let allow_paws = f.contains_key("allow-paws");
            let floor = f.get("interval-floor").cloned();
            let whisker: PathBuf = match f.get("whisker") {
                Some(p) => PathBuf::from(p),
                None => std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|d| d.join("whisker")))
                    .unwrap_or_else(|| PathBuf::from("whisker")),
            };
            if !whisker.exists() {
                eprintln!(
                    "fleet run: no whisker binary at {} — pass --whisker <path>",
                    whisker.display()
                );
                return ExitCode::FAILURE;
            }
            let once = f.contains_key("once");
            println!(
                "fleet run: pilots from {} — renew {} — {}",
                whisker.display(),
                if renew {
                    "ON (a human said so)"
                } else {
                    "off (leases are the household's word)"
                },
                if once { "one pass" } else { "supervising" }
            );
            let mut backoff: BTreeMap<String, (i64, u32)> = BTreeMap::new(); // next try, failures
            loop {
                let now = super::now_secs();
                for s in paired_ships(&dir, &root) {
                    let id = s.world.id.clone();
                    // Leases: renew inside two hours of expiry when authorized.
                    match lease_expiry(&s.dir) {
                        Some(exp) if exp - now < 2 * 3600 && renew => {
                            match issue_lease(&dir, &s.dir, &id, 24) {
                                Ok(new_exp) => println!("{id}: lease renewed, now to {}", new_exp),
                                Err(e) => eprintln!("{id}: lease renewal failed: {e}"),
                            }
                        }
                        Some(exp) if exp < now => {
                            println!("{id}: LEASE EXPIRED — the pilot holds at the gate until `familiar world lease {id}`");
                        }
                        _ => {}
                    }
                    if pid_alive(&s.dir).is_some() {
                        continue;
                    }
                    let (next, fails) = backoff.get(&id).cloned().unwrap_or((0, 0));
                    if now < next {
                        continue;
                    }
                    let out = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(s.dir.join("whisker.out"));
                    let mut cmd = std::process::Command::new(&whisker);
                    cmd.arg("--ship").arg(&s.dir);
                    for a in &s.captain.pilot_args {
                        cmd.arg(a);
                    }
                    if allow_paws {
                        cmd.arg("--allow-paws");
                    }
                    if let Some(fl) = &floor {
                        cmd.arg("--interval-floor").arg(fl);
                    }
                    if let Ok(o) = out {
                        if let Ok(e) = o.try_clone() {
                            cmd.stdout(o).stderr(e);
                        }
                    }
                    match cmd.spawn() {
                        Ok(child) => {
                            let _ =
                                std::fs::write(s.dir.join("whisker.pid"), child.id().to_string());
                            println!(
                                "{id}: pilot started (pid {}) for captain {} — \"{}\"",
                                child.id(),
                                s.captain.captain,
                                s.world.label
                            );
                            // Let it run free: the pid file is the handle, not this process.
                            std::mem::forget(child);
                            backoff.insert(id, (now + 30, fails));
                        }
                        Err(e) => {
                            let wait = (30i64 << fails.min(5)).min(600);
                            eprintln!("{id}: pilot failed to start: {e} — next try in {wait}s");
                            backoff.insert(id, (now + wait, fails + 1));
                        }
                    }
                }
                if once {
                    break;
                }
                std::thread::sleep(Duration::from_secs(60));
            }
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("fleet: unknown subcommand `{other}` — pair | unpair | status | run");
            ExitCode::FAILURE
        }
    }
}
