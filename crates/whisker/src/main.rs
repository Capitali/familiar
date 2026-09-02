//! The whisker runner: wraps the pure doctrine in a wire, a clock, and two gates.
//!
//! Reads ONLY the ship's store (ADR-0045): `ucf.env` (exchange + key, 0600),
//! `issuer.json` (the household's public identity, written by the commissioning
//! ceremony), `lease.json` (the signed, expiring boundary projection — re-read every
//! cycle so refresh and revocation both land without a restart), `automations.json`
//! (the pay-per-feature grants), and appends `journal.jsonl`.
//!
//! Two gates before every consequential act, both fail-closed:
//! 1. the LEASE must verify and its projected boundary must open `allow_network`;
//! 2. the decision's AUTOMATION must be granted in the ship store.
//!
//! A shut gate is a journaled refusal and a patient sleep, never an error exit — the
//! pilot keeps watch for a fresh lease the way it waits out a fold.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use familiar_mcp::{http, Url};
use familiar_mesh::node::NodeIdentity;
use familiar_whisker::doctrine::{self, Active, ActiveWord, Decision, LoadRow, Router, Ship};
use familiar_whisker::trade::{self, Holding, Ledger, TradeDecision};
use familiar_whisker::{env_value, granted_automations, Automation};
use familiar_world::lease::{self, SignedLease};
use serde_json::{json, Value};

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The wire to one exchange, carrying one key. Every call rides the mcp crate's
/// client: verifying TLS, plain http only to loopback, bounded reads.
struct Wire {
    base: String,
    key: String,
    /// Route costs already asked of the exchange, (from, to) → (asked-at, fuel). The
    /// merchant asks about every good's best buyer each docked fold; the lane graph
    /// does not move on that timescale, and the ship has ONE key that the exchange
    /// rate-limits (429s, 2026-09-01). A remembered answer costs nothing.
    routes: RefCell<RouteCache>,
}

/// (from, to) → (asked-at, fuel or unpriceable).
type RouteCache = HashMap<(String, String), (i64, Option<i64>)>;

/// A load that reverted or lapsed on us stays off our board this long: whatever
/// undid it is not fixed by booking it again the same fold.
const LOST_COOLDOWN_TICKS: i64 = 60;

/// ℳ per unit of fuel (the pack's `fuelPricePerUnit`, not on the wire; 2 on LOCAL
/// and PROD, and what the refuel receipts show). Charged against a trade's margin.
const FUEL_PRICE_PER_UNIT: i64 = 2;

/// A trade filed this cycle: what to look for on the receipt trail once it folds.
struct PendingTrade {
    side: &'static str,
    good: String,
    units: i64,
    ask: i64,
    /// The tick the action applies on (`resolvesAtTick - 1`), the receipt's `tick`.
    applies_tick: i64,
}

/// How long a priced route stays believed before it is asked again.
const ROUTE_CACHE_SECS: i64 = 30 * 60;

impl Wire {
    fn url(&self, path: &str) -> Result<Url, String> {
        Url::parse(&format!("{}{}", self.base, path)).map_err(|e| format!("{e:?}"))
    }

    fn auth(&self) -> Vec<(String, String)> {
        vec![
            ("Authorization".into(), format!("Bearer {}", self.key)),
            ("X-UCF-App".into(), "familiar-whisker".into()),
        ]
    }

    fn get(&self, path: &str) -> Result<Value, String> {
        let url = self.url(path)?;
        let resp = http::get(&url, &self.auth()).map_err(|e| format!("{e:?}"))?;
        if !(200..300).contains(&resp.status) {
            return Err(format!(
                "GET {path}: HTTP {} {}",
                resp.status,
                String::from_utf8_lossy(&resp.body[..resp.body.len().min(200)])
            ));
        }
        serde_json::from_slice(&resp.body).map_err(|e| format!("GET {path}: {e}"))
    }

    fn act(&self, mut body: Value, action_id: &str) -> Result<Value, String> {
        // The actionId is the idempotency handle, and the contract is RETRY THE ID,
        // NEVER THE INTENT (the owner's words, ucf-exchange#14): a re-sent intent
        // must carry the SAME id, or a transient failure after server acceptance
        // becomes a double-book. The caller owns the id for exactly that reason.
        body["actionId"] = json!(action_id);
        let url = self.url("/v1/actions")?;
        let bytes = serde_json::to_vec(&body).map_err(|e| e.to_string())?;
        let resp = http::post_json(&url, &self.auth(), &bytes).map_err(|e| format!("{e:?}"))?;
        if !(200..300).contains(&resp.status) {
            return Err(format!(
                "action refused: HTTP {} {}",
                resp.status,
                String::from_utf8_lossy(&resp.body[..resp.body.len().min(200)])
            ));
        }
        serde_json::from_slice(&resp.body).map_err(|e| e.to_string())
    }
}

impl Router for Wire {
    fn fuel_between(&self, from: &str, to: &str) -> Option<i64> {
        if from == to {
            return Some(0);
        }
        let key = (from.to_string(), to.to_string());
        let now = now_secs();
        if let Some((at, fuel)) = self.routes.borrow().get(&key) {
            if now - at < ROUTE_CACHE_SECS {
                return *fuel;
            }
        }
        let fuel = (|| {
            let v = self.get(&format!("/v1/route?from={from}&to={to}")).ok()?;
            let legs = v.get("legs")?.as_array()?;
            Some(legs.iter().filter_map(|l| l.get("fuel")?.as_i64()).sum())
        })();
        self.routes.borrow_mut().insert(key, (now, fuel));
        fuel
    }
}

fn journal(ship_dir: &Path, entry: Value) {
    use std::io::Write;
    let line = format!("{entry}\n");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(ship_dir.join("journal.jsonl"))
    {
        let _ = f.write_all(line.as_bytes());
    }
    println!("{entry}");
}

fn ship_from(me: &Value) -> Ship {
    let route_len = me
        .get("route")
        .and_then(Value::as_array)
        .map(|r| r.len())
        .unwrap_or(0);
    let docked = me.get("docked").and_then(Value::as_str).map(String::from);
    // Under way = NOT berthed. PROD reports `route: []` DURING a crossing (the
    // transit rides arrival ticks, not the route array), so keying flight on a
    // non-empty route read a flying ship as "adrift between folds" and held on a
    // wrong reason (KK II, t6094 en route to titania-cold-store, 2026-09-01). A
    // course merely LAID but not yet engaged shows as `driveAwaiting`, and that
    // is handled before the doctrine ever sees the ship — so by here, no berth
    // means she is crossing. The route array stays a belt to those braces.
    Ship {
        in_flight: docked.is_none() || route_len > 0,
        docked,
        hold_used: me.get("holdUsed").and_then(Value::as_i64).unwrap_or(0),
        hold_capacity: me.get("holdCapacity").and_then(Value::as_i64).unwrap_or(0),
        fuel: me.get("fuel").and_then(Value::as_i64).unwrap_or(0),
        fuel_capacity: me.get("fuelCapacity").and_then(Value::as_i64).unwrap_or(1),
        credits: me.get("credits").and_then(Value::as_i64).unwrap_or(0),
    }
}

fn load_row(v: &Value) -> Option<LoadRow> {
    Some(LoadRow {
        load_id: v.get("loadId")?.as_str()?.to_string(),
        good: v
            .get("good")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        origin: v.get("origin")?.as_str()?.to_string(),
        dest: v.get("dest")?.as_str()?.to_string(),
        units: v.get("units").and_then(Value::as_i64).unwrap_or(0),
        estimated_net: v.get("estimatedNet").and_then(Value::as_i64).unwrap_or(0),
        deadhead_ticks: v.get("deadheadTicks").and_then(Value::as_i64).unwrap_or(0),
        haul_ticks: v.get("haulTicks").and_then(Value::as_i64).unwrap_or(0),
        loading_ticks: v.get("loadingTicks").and_then(Value::as_i64).unwrap_or(8),
        held_for_other: v
            .get("heldForOther")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// The ledger's last word about a load, reduced to what decides. `None` = settled or
/// lost — either way the caller stops tracking it (with the reason for the journal).
fn reconcile(me: &Value, load_id: &str) -> Result<Option<ActiveWord>, String> {
    let events = me
        .get("freight")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter(|f| f.get("loadId").and_then(Value::as_str) == Some(load_id))
                .filter_map(|f| f.get("event").and_then(Value::as_str))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut word = ActiveWord::Booked;
    for e in events {
        let e_lower = e.to_lowercase();
        if e_lower.contains("payment taken") || e_lower.contains("collected") {
            return Err(format!("settled: {e}"));
        }
        // Every way a load leaves us without paying. "reverted" is the one that
        // stranded KK II at foxys-diner (booked t6195, reverted t6265, 2026-09-01):
        // a fold can UNDO a booking, and without this word the ledger shows no
        // terminal event, reconcile defaults to Booked, and she waits forever for a
        // crane to load a contract that no longer exists. "cancel" covers a
        // cancelBooking too.
        if e_lower.contains("rejected")
            || e_lower.contains("expired")
            || e_lower.contains("lapsed")
            || e_lower.contains("reverted")
            || e_lower.contains("cancel")
        {
            return Err(format!("lost: {e}"));
        }
        if e_lower.contains("delivered") {
            word = ActiveWord::Delivered;
        } else if (e_lower.contains("pickedup") || e_lower.contains("picked up"))
            && word == ActiveWord::Booked
        {
            word = ActiveWord::PickedUp;
        }
    }
    Ok(Some(word))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut ship_dir: Option<PathBuf> = None;
    let mut floor_secs: u64 = 5;
    let mut allow_paws = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--ship" => {
                i += 1;
                ship_dir = args.get(i).map(PathBuf::from);
            }
            "--interval-floor" => {
                i += 1;
                floor_secs = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(5);
            }
            // Only pass this where the tanker is actually a rescue (LOCAL, instant).
            // On a real-time world PAWS is a multi-day strand (metal#59) and the
            // default distress-hold is the safe answer.
            "--allow-paws" => allow_paws = true,
            other => {
                eprintln!("whisker: unknown argument {other}");
                eprintln!(
                    "usage: whisker --ship <ship-store-dir> [--interval-floor SECS] [--allow-paws]"
                );
                return ExitCode::FAILURE;
            }
        }
        i += 1;
    }
    let Some(ship_dir) = ship_dir else {
        eprintln!("whisker: --ship <ship-store-dir> is required (the ship's OWN store)");
        return ExitCode::FAILURE;
    };
    let instance = ship_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    if !instance.starts_with("world-") {
        eprintln!(
            "whisker: {} does not look like a commissioned ship store (world-<hex>)",
            ship_dir.display()
        );
        return ExitCode::FAILURE;
    }

    let issuer: NodeIdentity = match std::fs::read_to_string(ship_dir.join("issuer.json"))
        .map_err(|e| e.to_string())
        .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
    {
        Ok(id) => id,
        Err(e) => {
            eprintln!(
                "whisker: no readable issuer.json in the ship store ({e}) — without the \
                 household's public identity no lease can verify, and without a lease \
                 nothing is permitted. Re-run the commissioning ceremony."
            );
            return ExitCode::FAILURE;
        }
    };

    let env_path = ship_dir.join("ucf.env");
    let server =
        env_value(&env_path, "UCF_SERVER").unwrap_or_else(|| "http://127.0.0.1:7877".to_string());
    let Some(key) = env_value(&env_path, "UCF_KEY") else {
        eprintln!(
            "whisker: no UCF_KEY in {} — the ship needs its own trading key (0600, \
             never the captain's)",
            env_path.display()
        );
        return ExitCode::FAILURE;
    };
    let wire = Wire {
        base: server.trim_end_matches('/').to_string(),
        key,
        routes: RefCell::new(HashMap::new()),
    };

    let (granted, unknown) = granted_automations(&ship_dir);
    for u in &unknown {
        eprintln!("whisker: automations.json names unknown automation {u:?} — it grants nothing");
    }
    journal(
        &ship_dir,
        json!({"at": now_secs(), "event": "watch-begins", "instance": instance,
               "exchange": wire.base, "automations": granted.iter().map(|a| format!("{a:?}")).collect::<Vec<_>>()}),
    );

    // The chart: which stations sell fuel. Read once; a content change is a new world.
    let pumps: BTreeSet<String> = match wire.get("/v1/stations") {
        Ok(Value::Array(stations)) => stations
            .iter()
            .filter(|s| s.get("sellsFuel").and_then(Value::as_bool).unwrap_or(false))
            .filter_map(|s| s.get("id").and_then(Value::as_str).map(String::from))
            .collect(),
        Ok(_) | Err(_) => BTreeSet::new(),
    };

    let mut active: Option<Active> = None;
    let mut pending_until: i64 = -1;
    let mut recent: HashMap<String, (i64, String)> = HashMap::new();
    // Loads that left us without paying, by the tick they did: not re-booked for a while.
    let mut lost_at: HashMap<String, i64> = HashMap::new();
    let mut seq: u64 = 0;
    let mut last_refusal = String::new();
    // Wedge watch: a course filed while dry never departs on its own after refuelling
    // (ucf-exchange#16) — docked + course filed + unmoved needs a re-filed travel.
    let mut wedge: Option<(String, Vec<String>)> = None;
    let mut wedge_since: i64 = -1;
    let mut adopted = false;
    // The merchant's speculative book (ADR-0045: lives in the ship's own store).
    let trades = granted.contains(&Automation::Trade);
    let mut last_carry_block = String::new();
    let mut last_merchant_idle = String::new();
    // A filed trade whose fold has not been read back from the receipt trail yet.
    let mut pending_trade: Option<PendingTrade> = None;
    // The world's day, in ticks: the exchange's minimum hold on bought goods is a
    // day (`minHoldTicks` in the pack, not exposed on the wire — LOCAL and PROD both
    // 288). The refusal text corrects us if a world says otherwise.
    let min_hold: i64 = wire
        .get("/v1/reference")
        .ok()
        .and_then(|v| v.get("ticksPerDay").and_then(Value::as_i64))
        .unwrap_or(288);
    let mut holdings: Vec<Holding> = if trades {
        trade::load_holdings(&ship_dir)
    } else {
        Vec::new()
    };

    loop {
        let now = now_secs();

        // Gate 1: the lease, re-read every cycle so refresh and expiry both bite.
        let signed: Option<SignedLease> = std::fs::read_to_string(ship_dir.join("lease.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok());
        if !lease::permits(signed.as_ref(), &issuer, &instance, now, |b| {
            b.allow_network
        }) {
            let why = match &signed {
                None => "no lease in the ship store".to_string(),
                Some(s) => match lease::verify(s, &issuer, &instance, now) {
                    Err(e) => format!("{e}"),
                    Ok(_) => "the leased boundary keeps allow_network shut".to_string(),
                },
            };
            if why != last_refusal {
                journal(
                    &ship_dir,
                    json!({"at": now, "event": "held-at-the-gate", "why": why}),
                );
                last_refusal = why;
            }
            std::thread::sleep(Duration::from_secs(60));
            continue;
        }
        last_refusal.clear();

        let (tick, tick_secs) = match wire.get("/v1/status") {
            Ok(v) => (
                v.get("tick").and_then(Value::as_i64).unwrap_or(0),
                v.get("tickDurationSec")
                    .and_then(Value::as_u64)
                    .unwrap_or(10),
            ),
            Err(e) => {
                journal(
                    &ship_dir,
                    json!({"at": now, "event": "exchange-unreachable", "why": e}),
                );
                std::thread::sleep(Duration::from_secs(30));
                continue;
            }
        };
        let me = match wire.get("/v1/me") {
            Ok(v) => v,
            Err(e) => {
                journal(
                    &ship_dir,
                    json!({"at": now, "event": "exchange-unreachable", "why": e}),
                );
                std::thread::sleep(Duration::from_secs(30));
                continue;
            }
        };
        let ship = ship_from(&me);
        let route_now: Vec<String> = me
            .get("route")
            .and_then(Value::as_array)
            .map(|r| {
                r.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        // A restart must not forget a held contract — the exchange allows ONE at a
        // time, and a pilot that assumes idle double-books and is refused. Adopt the
        // newest load the ledger still shows open.
        if !adopted {
            adopted = true;
            let mut open: Vec<(i64, String)> = Vec::new();
            if let Some(events) = me.get("freight").and_then(Value::as_array) {
                let mut latest: HashMap<String, (i64, bool)> = HashMap::new();
                for f in events {
                    let Some(lid) = f.get("loadId").and_then(Value::as_str) else {
                        continue;
                    };
                    let e = f
                        .get("event")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_lowercase();
                    let t = f.get("tick").and_then(Value::as_i64).unwrap_or(0);
                    let closed = e.contains("payment taken")
                        || e.contains("collected")
                        || e.contains("rejected")
                        || e.contains("expired")
                        || e.contains("lapsed");
                    let opens =
                        e.contains("booked") || e.contains("picked") || e.contains("delivered");
                    let entry = latest.entry(lid.to_string()).or_insert((t, false));
                    if closed {
                        entry.1 = true;
                    } else if opens {
                        *entry = (t, false);
                    }
                }
                open = latest
                    .into_iter()
                    .filter(|(_, (_, closed))| !closed)
                    .map(|(lid, (t, _))| (t, lid))
                    .collect();
            }
            if let Some((_, lid)) = open.into_iter().max() {
                for status in ["booked", "inTransit", "delivered"] {
                    if let Ok(Value::Array(rows)) =
                        wire.get(&format!("/v1/loadboard?status={status}"))
                    {
                        if let Some(row) =
                            rows.iter().filter_map(load_row).find(|l| l.load_id == lid)
                        {
                            journal(
                                &ship_dir,
                                json!({"at": now, "tick": tick,
                                "event": "adopted-held-contract", "load": lid, "status": status}),
                            );
                            let word = reconcile(&me, &lid)
                                .ok()
                                .flatten()
                                .unwrap_or(ActiveWord::Booked);
                            active = Some(Active { row, word });
                            break;
                        }
                    }
                }
            }
        }

        // The wedge: docked, healthy tank, course still filed, nothing moving.
        if ship.docked.is_some() && !route_now.is_empty() && ship.fuel > ship.fuel_capacity / 10 {
            let key = (ship.docked.clone().unwrap_or_default(), route_now.clone());
            if wedge.as_ref() == Some(&key) {
                if tick - wedge_since > 30 {
                    // First file `engage`: the drive is a two-step file-then-engage on
                    // some folds and the verb is missing from the API's own error list
                    // (UCF-Haul#65 research; verified accepted on main 2026-08-31). A
                    // fresh travel to the same destination is the belt to its braces.
                    seq += 1;
                    let engage_id = format!("whisker-{}-{}", now_secs(), seq);
                    let engaged = wire.act(json!({"type": "engage"}), &engage_id).is_ok();
                    if let Some(dest) = route_now.last() {
                        seq += 1;
                        let travel_id = format!("whisker-{}-{}", now_secs(), seq);
                        if let Ok(ack) =
                            wire.act(json!({"type": "travel", "station": dest}), &travel_id)
                        {
                            journal(
                                &ship_dir,
                                json!({"at": now, "tick": tick,
                                "event": "unwedged-course", "to": dest, "engaged": engaged,
                                "resolves": ack.get("resolvesAtTick")}),
                            );
                        }
                    }
                    wedge_since = tick;
                }
            } else {
                wedge = Some(key);
                wedge_since = tick;
            }
        } else {
            wedge = None;
        }

        // Reconcile the active load against the ledger — the fold is the truth. Not
        // before a filed action has folded, though: until then the ledger's last word
        // is about the PREVIOUS life of that load id (a re-booked contract still reads
        // "reverted" for a tick), and closing on it books the same load a third time.
        if let Some(a) = active.as_mut().filter(|_| tick >= pending_until) {
            match reconcile(&me, &a.row.load_id) {
                Ok(Some(word)) => a.word = word,
                Ok(None) => {}
                Err(reason) => {
                    // A load that left us is off the board for a while, and the intent
                    // that booked it is forgotten: the idempotency id of a dead booking
                    // must never answer for a fresh one (LOCAL L1849, 2026-09-01: the
                    // replayed id acked the old fold and the pilot closed and re-booked
                    // the same lapsed contract every 15 ticks).
                    lost_at.insert(a.row.load_id.clone(), tick);
                    let lid = a.row.load_id.clone();
                    recent.retain(|sig, _| !sig.contains(&lid));
                    journal(
                        &ship_dir,
                        json!({"at": now, "tick": tick, "event": "load-closed",
                               "load": a.row.load_id, "why": reason, "credits": ship.credits}),
                    );
                    active = None;
                }
            }
        }

        // The two-step drive. On PROD, `travel` LAYS a course at the drive and a
        // separate `engage` departs it — and a laid-but-unengaged course shows as
        // `driveAwaiting: <station>` while `route` stays EMPTY, so the doctrine's
        // route-based logic cannot see it and the ship sits forever (KK II lost 40
        // minutes to exactly this, 2026-09-01). LOCAL's drive auto-engages, which is
        // why it never surfaced there. Whenever a course is awaiting engagement,
        // engaging it IS this fold's action — before anything else, since nothing
        // else can move the ship. (UCF-Haul#65; the verb is real but undiscoverable.)
        let awaiting = me
            .get("driveAwaiting")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty());
        if let Some(dest) = awaiting {
            if tick >= pending_until {
                seq += 1;
                let id = format!("whisker-{}-{}", now_secs(), seq);
                match wire.act(json!({"type": "engage"}), &id) {
                    Ok(ack) => {
                        pending_until = ack
                            .get("resolvesAtTick")
                            .and_then(Value::as_i64)
                            .unwrap_or(tick)
                            + 1;
                        journal(
                            &ship_dir,
                            json!({"at": now, "tick": tick, "event": "engaged-drive",
                                   "to": dest, "resolves": pending_until - 1}),
                        );
                    }
                    Err(e) => journal(
                        &ship_dir,
                        json!({"at": now, "tick": tick, "event": "engage-refused", "why": e}),
                    ),
                }
            }
            std::thread::sleep(Duration::from_secs((tick_secs * 3 / 5).max(floor_secs)));
            continue;
        }

        // One intent in flight at a time: wait out the fold we already paid for.
        if tick < pending_until {
            std::thread::sleep(Duration::from_secs((tick_secs * 3 / 5).max(floor_secs)));
            continue;
        }

        // The board, only when the judgment could use it.
        let board: Vec<LoadRow> = if active.is_none() && !ship.in_flight {
            match wire.get("/v1/loadboard?status=open") {
                Ok(Value::Array(rows)) => rows
                    .iter()
                    .filter_map(load_row)
                    .filter(|l| {
                        lost_at
                            .get(&l.load_id)
                            .map(|t| tick - t > LOST_COOLDOWN_TICKS)
                            .unwrap_or(true)
                    })
                    .collect(),
                Ok(_) | Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };

        // ── The merchant phase (Automation::Trade) ──────────────────────────────
        // Runs when berthed, before the freight decision, so a trade takes the fold
        // (one action per fold holds). SELL runs even while hauling freight — held
        // goods are realized at whatever berth pays once the exchange's clock allows;
        // BUY (opening a position) only when freight is idle and nothing is held.
        // The whole phase is gated: no Trade grant, no merchant behavior.
        if trades && tick >= pending_until {
            // 1. Read back the last trade's fold from the receipt trail: the outcome is
            //    a market fact recorded in the world (filled, or a named refusal), never
            //    an HTTP error — and the refusal that matters names the clock.
            if let Some(pt) = pending_trade.take() {
                let receipt = wire
                    .get("/v1/receipts")
                    .ok()
                    .and_then(|v| v.as_array().cloned())
                    .unwrap_or_default()
                    .into_iter()
                    .find(|r| {
                        r.get("tick").and_then(Value::as_i64) == Some(pt.applies_tick)
                            && r.get("good").and_then(Value::as_str) == Some(pt.good.as_str())
                            && r.get("side").and_then(Value::as_str) == Some(pt.side)
                    });
                let outcome = receipt
                    .as_ref()
                    .and_then(|r| r.get("outcome").and_then(Value::as_str))
                    .unwrap_or("(no receipt yet)")
                    .to_string();
                let total = receipt
                    .as_ref()
                    .and_then(|r| r.get("total").and_then(Value::as_i64))
                    .unwrap_or(0);
                if pt.side == "buy" && outcome == "filled" {
                    let basis = trade::basis_from_total(total, pt.units, pt.ask);
                    if let Some(h) = holdings.iter_mut().find(|h| h.good == pt.good) {
                        h.avg_cost = basis;
                    }
                }
                if let Some(at) = trade::sellable_tick_from_refusal(&outcome) {
                    if let Some(h) = holdings.iter_mut().find(|h| h.good == pt.good) {
                        h.sellable_at = at;
                    }
                }
                journal(
                    &ship_dir,
                    json!({"at": now, "tick": tick, "event": "trade-outcome", "side": pt.side,
                           "good": pt.good, "units": pt.units, "outcome": outcome,
                           "total": total, "credits": ship.credits}),
                );
            }

            // 2. The hold is the truth: bring the book to what is actually aboard.
            let cargo = trade::parse_cargo(&me);
            let freight_aboard = active
                .as_ref()
                .filter(|a| a.word != ActiveWord::Booked && !a.row.good.is_empty())
                .map(|a| (a.row.good.as_str(), a.row.units));
            let galaxy_for_hint = if cargo.is_empty() {
                Vec::new()
            } else {
                wire.get("/v1/galaxy/prices")
                    .map(|v| trade::parse_galaxy(&v))
                    .unwrap_or_default()
            };
            let hint = |good: &str| -> i64 {
                galaxy_for_hint
                    .iter()
                    .filter(|r| r.good == good)
                    .map(|r| r.mid)
                    .max()
                    .unwrap_or(0)
            };
            for note in
                trade::reconcile_hold(&mut holdings, &cargo, freight_aboard, &hint, tick, min_hold)
            {
                journal(
                    &ship_dir,
                    json!({"at": now, "tick": tick, "event": "book-corrected", "why": note}),
                );
            }
            trade::save_holdings(&ship_dir, &holdings);

            if let Some(here) = ship.docked.clone() {
                let spare_hold = (ship.hold_capacity - ship.hold_used).max(0);
                // Freight needs the space back only when a BOOKED contract's cargo would
                // not fit beside what we carry. A loaded or delivered contract already
                // has its room; a fitting one rides alongside.
                let need_hold = active
                    .as_ref()
                    .map(|a| a.word == ActiveWord::Booked && a.row.units > spare_hold)
                    .unwrap_or(false);
                let board_here = wire
                    .get(&format!("/v1/stations/{here}/quotes"))
                    .map(|v| trade::parse_board(&v))
                    .unwrap_or_default();
                let galaxy = if galaxy_for_hint.is_empty() {
                    wire.get("/v1/galaxy/prices")
                        .map(|v| trade::parse_galaxy(&v))
                        .unwrap_or_default()
                } else {
                    galaxy_for_hint
                };
                // What the carry leg can leave with: a full tank if this berth pumps
                // (the freight doctrine tops up here first), else what is in it now.
                let fuel_available = if pumps.contains(&here) {
                    ship.fuel_capacity
                } else {
                    ship.fuel
                };
                let ledger = Ledger {
                    here: &here,
                    tick,
                    credits: ship.credits,
                    spare_hold,
                    need_hold,
                    fuel_available,
                    fuel_price: FUEL_PRICE_PER_UNIT,
                    min_hold,
                };
                let td =
                    trade::decide_trade(&ledger, &board_here, &galaxy, &holdings, &pumps, &wire);
                // Say why the merchant passed — once per reason, so the journal reads
                // "no fuel for a carry" / "no arbitrage on this board" without a line
                // per fold.
                if let TradeDecision::Idle { why } = &td {
                    let line = format!("{here}: {why}");
                    if line != last_merchant_idle {
                        journal(
                            &ship_dir,
                            json!({"at": now, "tick": tick, "event": "merchant-idle",
                            "at_station": here, "why": why, "credits": ship.credits,
                            "fuel_available": fuel_available, "spare_hold": spare_hold}),
                        );
                        last_merchant_idle = line;
                    }
                } else {
                    last_merchant_idle.clear();
                }
                let trade_body = match &td {
                    TradeDecision::Sell { good, units, .. } => Some((
                        json!({"type": "sell", "station": here, "good": good, "units": units}),
                        good.clone(),
                        *units,
                        true,
                    )),
                    TradeDecision::Buy { good, units, .. } if active.is_none() => Some((
                        json!({"type": "buy", "station": here, "good": good, "units": units}),
                        good.clone(),
                        *units,
                        false,
                    )),
                    _ => None,
                };
                if let Some((body, good, units, is_sell)) = trade_body {
                    seq += 1;
                    let id = format!("whisker-{}-{}", now_secs(), seq);
                    match wire.act(body, &id) {
                        Ok(ack) => {
                            let resolves = ack
                                .get("resolvesAtTick")
                                .and_then(Value::as_i64)
                                .unwrap_or(tick + 1);
                            pending_until = resolves + 1;
                            let ask = board_here
                                .iter()
                                .find(|q| q.good == good)
                                .map(|q| q.ask)
                                .unwrap_or(0);
                            pending_trade = Some(PendingTrade {
                                side: if is_sell { "sell" } else { "buy" },
                                good: good.clone(),
                                units,
                                ask,
                                applies_tick: resolves - 1,
                            });
                            if !is_sell {
                                // The book leads the fold by one tick; the receipt sets
                                // the true basis and the hold reconcile corrects the
                                // units. Clock: the exchange arms it at the applying
                                // tick.
                                if let TradeDecision::Buy {
                                    sell_target,
                                    est_margin,
                                    ..
                                } = &td
                                {
                                    holdings.push(Holding {
                                        good: good.clone(),
                                        units,
                                        avg_cost: ask,
                                        sell_target: sell_target.clone(),
                                        opened_tick: resolves - 1,
                                        sellable_at: resolves - 1 + min_hold,
                                    });
                                    journal(
                                        &ship_dir,
                                        json!({"at": now, "tick": tick, "event": "position-opened",
                                        "good": good, "units": units, "ask": ask, "sell_target": sell_target,
                                        "est_margin": est_margin, "sellable_at": resolves - 1 + min_hold}),
                                    );
                                }
                            }
                            // A sell is not taken off the book until the hold confirms it.
                            trade::save_holdings(&ship_dir, &holdings);
                            let why = match &td {
                                TradeDecision::Sell { why, .. } => why.clone(),
                                _ => String::new(),
                            };
                            journal(
                                &ship_dir,
                                json!({"at": now, "tick": tick, "event": "traded",
                                "side": if is_sell {"sell"} else {"buy"}, "good": good, "units": units,
                                "credits": ship.credits, "why": why, "resolves": resolves}),
                            );
                        }
                        Err(e) => journal(
                            &ship_dir,
                            json!({"at": now, "tick": tick,
                            "event": "trade-refused", "side": if is_sell {"sell"} else {"buy"}, "good": good, "why": e}),
                        ),
                    }
                    std::thread::sleep(Duration::from_secs((tick_secs * 3 / 5).max(floor_secs)));
                    continue;
                }
                // Carry leg: freight idle, a position past its clock that no bid here
                // clears — fly it toward its market so the arb can close. Before the
                // clock there is nothing to do at the market but wait, so the goods
                // ride under freight instead.
                if active.is_none() && !ship.in_flight {
                    if let Some(h) = holdings
                        .iter()
                        .filter(|h| tick >= h.sellable_at && !h.sell_target.is_empty())
                        .max_by_key(|h| h.opened_tick)
                    {
                        if h.sell_target != here {
                            // The leg must be flyable on what is in the tank, reserve
                            // included — otherwise leave the fold to the freight
                            // doctrine, whose fuel rules (top-up here, divert to a pump)
                            // are what gets the tank filled. A carry that strands the
                            // hull is a PAWS bill, not a trade.
                            let cost = wire.fuel_between(&here, &h.sell_target);
                            let flyable = cost
                                .map(|c| trade::carry_affordable(c, ship.fuel))
                                .unwrap_or(false);
                            if !flyable {
                                let why = format!(
                                    "carry {} → {} needs fuel {:?}, tank {}",
                                    h.good, h.sell_target, cost, ship.fuel
                                );
                                if why != last_carry_block {
                                    journal(
                                        &ship_dir,
                                        json!({"at": now, "tick": tick, "event": "carry-blocked", "why": why}),
                                    );
                                    last_carry_block = why;
                                }
                            } else {
                                last_carry_block.clear();
                                seq += 1;
                                let id = format!("whisker-{}-{}", now_secs(), seq);
                                match wire
                                    .act(json!({"type": "travel", "station": h.sell_target}), &id)
                                {
                                    Ok(ack) => {
                                        pending_until = ack
                                            .get("resolvesAtTick")
                                            .and_then(Value::as_i64)
                                            .unwrap_or(tick)
                                            + 1;
                                        journal(
                                            &ship_dir,
                                            json!({"at": now, "tick": tick, "event": "carry-to-market",
                                            "good": h.good, "to": h.sell_target, "resolves": pending_until - 1}),
                                        );
                                        std::thread::sleep(Duration::from_secs(
                                            (tick_secs * 3 / 5).max(floor_secs),
                                        ));
                                        continue;
                                    }
                                    Err(e) => journal(
                                        &ship_dir,
                                        json!({"at": now, "tick": tick,
                                        "event": "carry-refused", "good": h.good, "to": h.sell_target, "why": e}),
                                    ),
                                }
                            }
                        }
                    }
                }
            }
        }

        let decision = doctrine::decide(&ship, active.as_ref(), &board, &pumps, &wire);

        // Gate 2: the automation this decision spends must be granted (pay-per-feature).
        if let Some(auto) = decision.automation() {
            if !granted.contains(&auto) {
                let why = format!("{auto:?} is not granted in this ship's automations.json");
                if why != last_refusal {
                    journal(
                        &ship_dir,
                        json!({"at": now, "tick": tick,
                        "event": "held-at-the-gate", "decision": format!("{decision:?}"), "why": why}),
                    );
                    last_refusal = why;
                }
                std::thread::sleep(Duration::from_secs(60));
                continue;
            }
        }

        // The PAWS guard. On a real-time world the tanker is a CATASTROPHE, not a
        // rescue: PROD prices the call-out from raw km at a flat rate, so a tanker to
        // the outer system is ~2,600 ticks = FIVE AND A HALF DAYS away (metal#59). So
        // committing to PAWS strands the hull for days and bills it heavily. Unless a
        // human opts in (`--allow-paws`, for LOCAL where the tanker is instant), a
        // would-be PAWS call becomes a LOUD distress hold instead: safe, reversible,
        // and surfaced — a new fuelable load or a human can still rescue her, where a
        // filed tanker cannot be recalled.
        if matches!(decision, Decision::CallPaws) && !allow_paws {
            if last_refusal != "distress" {
                journal(
                    &ship_dir,
                    json!({"at": now, "tick": tick, "event": "distress-hold",
                           "docked": ship.docked, "fuel": ship.fuel,
                           "why": "low fuel, no affordable pump; PAWS refused (would strand for days on this world) — holding for a fuelable load or a human"}),
                );
                last_refusal = "distress".to_string();
            }
            std::thread::sleep(Duration::from_secs((tick_secs * 3 / 5).max(floor_secs)));
            continue;
        }

        let body = match &decision {
            Decision::Hold { .. } => None,
            Decision::Refuel => Some(json!({"type": "refuel"})),
            Decision::CallPaws => Some(json!({"type": "paws"})),
            Decision::DivertToPump { pump } => Some(json!({"type": "travel", "station": pump})),
            Decision::Travel { station } => Some(json!({"type": "travel", "station": station})),
            Decision::Book { load_id } => Some(json!({"type": "book", "loadId": load_id})),
            Decision::Collect { load_id } => Some(json!({"type": "collect", "loadId": load_id})),
        };

        if let Some(body) = body {
            // The same intent inside the window is the same decision, never re-filed —
            // the zigzag-to-empty lesson.
            let sig = body.to_string();
            let fresh = recent
                .get(&sig)
                .map(|(t, _)| tick - t >= 15)
                .unwrap_or(true);
            if fresh {
                // The id is minted once per INTENT and reused on every re-send of it,
                // so a retry after a transient failure is idempotent at the exchange
                // rather than a second order (retry the id, never the intent).
                let action_id = recent
                    .get(&sig)
                    .map(|(_, id)| id.clone())
                    .unwrap_or_else(|| {
                        seq += 1;
                        format!("whisker-{}-{}", now_secs(), seq)
                    });
                match wire.act(body.clone(), &action_id) {
                    Ok(ack) => {
                        recent.insert(sig, (tick, action_id));
                        pending_until = ack
                            .get("resolvesAtTick")
                            .and_then(Value::as_i64)
                            .unwrap_or(tick)
                            + 1;
                        journal(
                            &ship_dir,
                            json!({"at": now, "tick": tick,
                            "event": "acted", "decision": format!("{decision:?}"),
                            "resolves": pending_until - 1, "fuel": ship.fuel, "credits": ship.credits}),
                        );
                        if let Decision::Book { load_id } = &decision {
                            if let Some(row) = board.iter().find(|l| &l.load_id == load_id) {
                                active = Some(Active {
                                    row: row.clone(),
                                    word: ActiveWord::Booked,
                                });
                            }
                        }
                    }
                    Err(e) => {
                        journal(
                            &ship_dir,
                            json!({"at": now, "tick": tick,
                            "event": "refused-at-the-door", "decision": format!("{decision:?}"), "why": e}),
                        );
                    }
                }
            }
        } else if let Decision::Hold { why } = &decision {
            // Holds are journaled only when the reason changes — a quiet watch, not a
            // silent one.
            if why != &last_refusal {
                journal(
                    &ship_dir,
                    json!({"at": now, "tick": tick, "event": "holding",
                    "why": why, "docked": ship.docked, "fuel": ship.fuel, "credits": ship.credits}),
                );
                last_refusal = why.clone();
            }
        }

        std::thread::sleep(Duration::from_secs((tick_secs * 3 / 5).max(floor_secs)));
    }
}
