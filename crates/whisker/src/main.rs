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
use familiar_whisker::adoption::{
    adopt_step, in_booking_order, resume_stale_course, AdoptOutcome, WedgeRemedy, WedgeWatch,
};
use familiar_whisker::doctrine::{
    self, Active, ActiveWord, Decision, Itinerary, LoadRow, Router, Ship,
};
use familiar_whisker::trade::{self, Holding, Ledger, TradeDecision};
use familiar_whisker::{env_value, granted_automations, ledger, Automation};
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

/// The ONE way a load leaves the runner's memory — tracked or pending alike
/// (round-2 review, finding 4). Records the cooldown so a re-listed dead id is
/// not re-booked inside 60 ticks, forgets the booking intent so a dead fold's
/// idempotency id never answers for a fresh one (LOCAL L1849, 2026-09-01),
/// clears the adoption notice so a genuinely new life gets a fresh one, and
/// journals the close exactly once.
#[allow(clippy::too_many_arguments)]
fn close_load(
    ship_dir: &Path,
    lost_at: &mut HashMap<String, i64>,
    recent: &mut HashMap<String, (i64, String)>,
    adopt_noted: &mut BTreeSet<String>,
    now: i64,
    tick: i64,
    load_id: &str,
    reason: &str,
    credits: i64,
) {
    lost_at.insert(load_id.to_string(), tick);
    recent.retain(|sig, _| !sig.contains(load_id));
    adopt_noted.remove(load_id);
    journal(
        ship_dir,
        json!({"at": now, "tick": tick, "event": "load-closed",
               "load": load_id, "why": reason, "credits": credits}),
    );
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

    // The plan's contracts, booking order (T-232): today's exchange caps this at
    // one, but nothing here assumes it. `pending_adopt` carries ledger-open load
    // ids whose board row has not yet resolved — adoption is a RECONCILIATION
    // that retries every cycle, never a startup one-shot (codex review, finding 1).
    let mut loads: Vec<Active> = Vec::new();
    let mut pending_adopt: Vec<String> = Vec::new();
    let mut adopt_noted: BTreeSet<String> = BTreeSet::new();
    let mut pending_until: i64 = -1;
    let mut recent: HashMap<String, (i64, String)> = HashMap::new();
    // Loads that left us without paying, by the tick they did: not re-booked for a while.
    let mut lost_at: HashMap<String, i64> = HashMap::new();
    let mut seq: u64 = 0;
    let mut last_refusal = String::new();
    // Wedge watch: a course filed while dry never departs on its own after refuelling
    // (ucf-exchange#16) — docked + course filed + unmoved needs a re-filed travel.
    let mut wedge = WedgeWatch::default();
    let mut last_wedge_note = String::new();
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

        // A restart — or any drift — must not forget a held contract, and it must
        // not forget ANY of them. Adoption is a per-cycle reconciliation with a
        // pending-retry list (round-1 review finding 1; the step itself is pure and
        // pinned in `adoption.rs`), and while ANY id is pending the scheduler below
        // files no new commitment (round-2 review, finding 3). A close discovered
        // here goes through the SAME close handler as a tracked load's (round-2
        // review, finding 4) — cooldown, intent purge, journal, never silent.
        if tick >= pending_until {
            let mut lookup = |lid: &str| -> Option<LoadRow> {
                for status in ["booked", "inTransit", "delivered"] {
                    if let Ok(Value::Array(rows)) =
                        wire.get(&format!("/v1/loadboard?status={status}"))
                    {
                        if let Some(row) =
                            rows.iter().filter_map(load_row).find(|l| l.load_id == lid)
                        {
                            return Some(row);
                        }
                    }
                }
                None
            };
            for outcome in adopt_step(&loads, &mut pending_adopt, &me, &mut lookup) {
                match outcome {
                    AdoptOutcome::Adopted(a) => {
                        journal(
                            &ship_dir,
                            json!({"at": now, "tick": tick,
                            "event": "adopted-held-contract", "load": a.row.load_id,
                            "word": format!("{:?}", a.word)}),
                        );
                        adopt_noted.remove(&a.row.load_id);
                        loads.push(a);
                    }
                    AdoptOutcome::Closed { load_id, reason } => {
                        close_load(
                            &ship_dir,
                            &mut lost_at,
                            &mut recent,
                            &mut adopt_noted,
                            now,
                            tick,
                            &load_id,
                            &reason,
                            ship.credits,
                        );
                    }
                    AdoptOutcome::Pending { load_id } => {
                        // Say so once per life, keep trying.
                        if adopt_noted.insert(load_id.clone()) {
                            journal(
                                &ship_dir,
                                json!({"at": now, "tick": tick,
                                "event": "adoption-pending", "load": load_id,
                                "why": "ledger shows it open but no board row resolved this fold; retrying"}),
                            );
                        }
                    }
                }
            }
            // The plan is booking order — (booked tick, load id), deterministic,
            // never lookup-resolution order (adoption::in_booking_order, pinned).
            loads = in_booking_order(loads, &ledger::open_loads(&me));
        }

        // Reconcile every contract of the plan against the ledger — the fold is the
        // truth, and each load has its own word (T-232). Not before a filed action
        // has folded, though: until then the ledger's last word is about the
        // PREVIOUS life of that load id (a re-booked contract still reads
        // "reverted" for a tick), and closing on it books the same load a third time.
        if tick >= pending_until {
            loads.retain_mut(|a| match ledger::reconcile(&me, &a.row.load_id) {
                Ok(word) => {
                    a.word = word;
                    true
                }
                Err(reason) => {
                    close_load(
                        &ship_dir,
                        &mut lost_at,
                        &mut recent,
                        &mut adopt_noted,
                        now,
                        tick,
                        &a.row.load_id,
                        &reason,
                        ship.credits,
                    );
                    false
                }
            });
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

        // The adoption gate (round-2 review, finding 3): while any ledger-open
        // contract is still unresolved, the ship's freight state is UNKNOWN — no
        // merchant buy, no carry leg, no new booking, no diversion, no movement
        // for a resolved newer load. Engaging a laid course and waiting out a
        // filed fold stayed above this line deliberately: they complete acts
        // already in motion. A quiet hold, journaled once per change.
        if !pending_adopt.is_empty() {
            let why = format!(
                "{} contract(s) unresolved; adopting first",
                pending_adopt.len()
            );
            if why != last_refusal {
                journal(
                    &ship_dir,
                    json!({"at": now, "tick": tick, "event": "holding-for-adoption",
                           "pending": pending_adopt.clone(), "why": why}),
                );
                last_refusal = why;
            }
            std::thread::sleep(Duration::from_secs((tick_secs * 3 / 5).max(floor_secs)));
            continue;
        }

        // The plan, compiled from the live ledger words — built HERE so the
        // stale-course belt below validates against the same truth the decision
        // at the bottom of the cycle uses.
        let plan = Itinerary::sequential(loads.clone(), &pumps);

        // The stale-course belt, an explicit scheduler action BELOW the adoption
        // and pending-action gates (round-4 review, finding 1). The watch names
        // exactly one remedy per firing (engage first; the re-filed travel only
        // if the same course survives another threshold), the laid destination
        // is validated against what the ship would head for NOW — the plan's
        // working stop, or the merchant's carry intent when freight is empty —
        // and a successful act sets pending_until and ends the fold: one wire
        // action, then the next fold. A mismatched course is simply not
        // resumed; the ordinary decision files the right travel itself.
        if let Some(remedy) = wedge.observe(
            ship.docked.as_deref(),
            &route_now,
            ship.fuel,
            ship.fuel_capacity,
            pending_adopt.len(),
            tick,
        ) {
            let intended: Option<String> =
                plan.current().map(|s| s.station.clone()).or_else(|| {
                    holdings
                        .iter()
                        .filter(|h| tick >= h.sellable_at && !h.sell_target.is_empty())
                        .max_by_key(|h| h.opened_tick)
                        .map(|h| h.sell_target.clone())
                });
            let laid = route_now.last().cloned();
            if resume_stale_course(laid.as_deref(), intended.as_deref()) {
                seq += 1;
                let id = format!("whisker-{}-{}", now_secs(), seq);
                let (body, verb) = match remedy {
                    WedgeRemedy::Engage => (json!({"type": "engage"}), "engage"),
                    WedgeRemedy::Refile => (
                        json!({"type": "travel", "station": laid.clone().unwrap_or_default()}),
                        "travel",
                    ),
                };
                match wire.act(body, &id) {
                    Ok(ack) => {
                        pending_until = ack
                            .get("resolvesAtTick")
                            .and_then(Value::as_i64)
                            .unwrap_or(tick)
                            + 1;
                        journal(
                            &ship_dir,
                            json!({"at": now, "tick": tick, "event": "unwedged-course",
                            "remedy": verb, "to": laid, "resolves": pending_until - 1}),
                        );
                    }
                    Err(e) => journal(
                        &ship_dir,
                        json!({"at": now, "tick": tick, "event": "unwedge-refused",
                        "remedy": verb, "why": e}),
                    ),
                }
                std::thread::sleep(Duration::from_secs((tick_secs * 3 / 5).max(floor_secs)));
                continue;
            }
            let note = format!(
                "laid course to {laid:?} no longer matches intent {intended:?}; not resumed"
            );
            if note != last_wedge_note {
                journal(
                    &ship_dir,
                    json!({"at": now, "tick": tick, "event": "stale-course-dropped", "why": note}),
                );
                last_wedge_note = note;
            }
        }

        // The board, only when the judgment could use it.
        // No booking while any open contract is still unresolved (codex review,
        // finding 1): an unseen held contract plus a fresh booking is a refused
        // double-book at best and a forgotten commitment at worst.
        let board: Vec<LoadRow> = if loads.is_empty() && pending_adopt.is_empty() && !ship.in_flight
        {
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
            // What freight is genuinely ABOARD — PickedUp only, per each load's
            // own word (doctrine::freight_aboard, pinned): a Delivered load's
            // cargo already left with the crane, and counting it here deleted a
            // real merchant lot (round-3 review, finding 2).
            let freight_aboard = doctrine::freight_aboard(&loads);
            let galaxy_for_hint = if cargo.is_empty() {
                Vec::new()
            } else {
                wire.get("/v1/galaxy/prices")
                    .map(|v| trade::parse_galaxy(&v))
                    .unwrap_or_default()
            };
            let hint = |good: &str| -> (i64, String) {
                galaxy_for_hint
                    .iter()
                    .filter(|r| r.good == good)
                    .max_by_key(|r| r.mid)
                    .map(|r| (r.mid, r.station.clone()))
                    .unwrap_or((0, String::new()))
            };
            for note in trade::reconcile_hold(
                &mut holdings,
                &cargo,
                &freight_aboard,
                &hint,
                tick,
                min_hold,
            ) {
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
                let need_hold = loads
                    .iter()
                    .any(|a| a.word == ActiveWord::Booked && a.row.units > spare_hold);
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
                // A buy is sized once more against the BUYER's shelf: the galaxy row
                // says what a berth pays, not how much it will take, and a full shelf
                // (bluefin at titania-cold-store: maxSellUnits 2) pays for nothing.
                // One extra read, only on the fold that would spend money.
                let td = match td {
                    TradeDecision::Buy {
                        good,
                        units,
                        sell_target,
                        est_margin,
                    } if loads.is_empty() => {
                        let takes = wire
                            .get(&format!("/v1/stations/{sell_target}/quotes"))
                            .map(|v| trade::parse_board(&v))
                            .unwrap_or_default()
                            .into_iter()
                            .find(|q| q.good == good)
                            .map(|q| q.max_sell)
                            .unwrap_or(0);
                        if takes <= 0 {
                            journal(
                                &ship_dir,
                                json!({"at": now, "tick": tick, "event": "merchant-idle",
                                "at_station": here, "why": format!("{sell_target} takes no {good} right now (shelf full)"),
                                "credits": ship.credits}),
                            );
                            TradeDecision::Idle {
                                why: "buyer's shelf full".into(),
                            }
                        } else {
                            let capped = units.min(takes);
                            TradeDecision::Buy {
                                good,
                                units: capped,
                                sell_target,
                                est_margin: est_margin * capped / units.max(1),
                            }
                        }
                    }
                    other => other,
                };
                let trade_body = match &td {
                    TradeDecision::Sell { good, units, .. } => Some((
                        json!({"type": "sell", "station": here, "good": good, "units": units}),
                        good.clone(),
                        *units,
                        true,
                    )),
                    TradeDecision::Buy { good, units, .. } if loads.is_empty() => Some((
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
                if loads.is_empty() && !ship.in_flight {
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
                                // Once per blocked carry, not once per fuel reading.
                                let key = format!("{} → {}", h.good, h.sell_target);
                                if key != last_carry_block {
                                    journal(
                                        &ship_dir,
                                        json!({"at": now, "tick": tick, "event": "carry-blocked", "why": why}),
                                    );
                                    last_carry_block = key;
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

        let decision = doctrine::decide_plan(&ship, &plan, &board, &pumps, &wire);

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
                                loads.push(Active {
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
