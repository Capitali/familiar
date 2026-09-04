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
use familiar_whisker::autonomy::{self, Dial, Gate, Surface};
use familiar_whisker::doctrine::{self, Active, ActiveWord, Decision, LoadRow, Router, Ship};
use familiar_whisker::outfit::{self, DeliveryStat, OutfitDecision, Purse};
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

/// One priced route: fuel at the reference drive, and each leg's separation in km.
#[derive(Clone)]
struct PricedRoute {
    fuel: i64,
    leg_km: Vec<i64>,
}

/// (from, to) → (asked-at, the route or unpriceable).
type RouteCache = HashMap<(String, String), (i64, Option<PricedRoute>)>;

/// A load that reverted or lapsed on us stays off our board this long: whatever
/// undid it is not fixed by booking it again the same fold.
const LOST_COOLDOWN_TICKS: i64 = 60;

/// Lease service per day, as observed on KK II's lease (leaseServicePaid 778 over
/// ~1.5 days); the pack's `leaseServiceChargeBps` is not on the wire.
const LEASE_SERVICE_PER_DAY_EST: i64 = 520;

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

impl Wire {
    /// One priced route, remembered for a while.
    fn route(&self, from: &str, to: &str) -> Option<PricedRoute> {
        if from == to {
            return Some(PricedRoute {
                fuel: 0,
                leg_km: Vec::new(),
            });
        }
        let key = (from.to_string(), to.to_string());
        let now = now_secs();
        if let Some((at, r)) = self.routes.borrow().get(&key) {
            if now - at < ROUTE_CACHE_SECS {
                return r.clone();
            }
        }
        let r = (|| {
            let v = self.get(&format!("/v1/route?from={from}&to={to}")).ok()?;
            let legs = v.get("legs")?.as_array()?;
            let fuel = legs.iter().filter_map(|l| l.get("fuel")?.as_i64()).sum();
            let leg_km = legs
                .iter()
                .map(|l| l.get("distanceKm").and_then(Value::as_i64).unwrap_or(0))
                .collect();
            Some(PricedRoute { fuel, leg_km })
        })();
        self.routes.borrow_mut().insert(key, (now, r.clone()));
        r
    }
}

impl Router for Wire {
    fn fuel_between(&self, from: &str, to: &str) -> Option<i64> {
        self.route(from, to).map(|r| r.fuel)
    }
    fn leg_distances_km(&self, from: &str, to: &str) -> Option<Vec<i64>> {
        self.route(from, to).map(|r| r.leg_km)
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
        accel_milli_g: me
            .get("effectiveAccelMilliG")
            .and_then(Value::as_i64)
            .unwrap_or(doctrine::REFERENCE_ACCEL_MILLI_G),
        wear_bps: me.get("wearBps").and_then(Value::as_i64).unwrap_or(0),
        leased: !me.get("titled").and_then(Value::as_bool).unwrap_or(true)
            && me
                .get("leasePrincipal")
                .and_then(Value::as_i64)
                .unwrap_or(0)
                > 0,
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
        class_bps: match v
            .get("serviceClass")
            .and_then(Value::as_str)
            .unwrap_or("standard")
        {
            "economy" => 5_000,
            "express" => 20_000,
            "priority" => 30_000,
            _ => 10_000,
        },
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
    familiar_whisker::doctrine::ledger_word(&events)
}

/// The captain's dial at the action door. `allow` answers whether THIS act may be
/// filed now; when it may not, the reason is on the journal — advice the message
/// window shows, a proposal waiting for a yes, or a lapse.
struct DialGate {
    dial: Dial,
    /// (surface|body) → the tick advice was last journaled, so a standing advice
    /// is said once per window, not every fold.
    last_advice: HashMap<String, i64>,
}

impl DialGate {
    #[allow(clippy::too_many_arguments)]
    fn allow(
        &mut self,
        ship_dir: &Path,
        surface: Surface,
        tick: i64,
        now: i64,
        body: &Value,
        describe: &str,
        why: &str,
    ) -> bool {
        let proposals = autonomy::load_proposals(ship_dir);
        let approvals = autonomy::load_approvals(ship_dir);
        let mut fresh = None;
        let g = autonomy::gate(
            &self.dial, surface, tick, body, describe, why, &proposals, &approvals, &mut fresh,
        );
        if let Some(p) = &fresh {
            autonomy::append_proposal(ship_dir, p);
            journal(
                ship_dir,
                json!({"at": now, "tick": tick, "event": "proposed",
                "id": p.id, "surface": p.surface, "would": describe, "why": why,
                "expires": p.expires_tick}),
            );
        }
        match g {
            Gate::Act => true,
            Gate::Proposed => false,
            Gate::Lapsed => {
                let id = autonomy::proposal_id(surface, body);
                let key = format!("lapsed|{id}");
                if self
                    .last_advice
                    .get(&key)
                    .map(|t| tick - t > 20)
                    .unwrap_or(true)
                {
                    journal(
                        ship_dir,
                        json!({"at": now, "tick": tick, "event": "proposal-lapsed",
                        "id": id, "surface": surface.key(), "would": describe}),
                    );
                    self.last_advice.insert(key, tick);
                }
                false
            }
            Gate::Advise => {
                let key = format!("advice|{}|{}", surface.key(), body);
                if self
                    .last_advice
                    .get(&key)
                    .map(|t| tick - t > 20)
                    .unwrap_or(true)
                {
                    journal(
                        ship_dir,
                        json!({"at": now, "tick": tick, "event": "advice",
                        "surface": surface.key(), "would": describe, "why": why, "body": body}),
                    );
                    self.last_advice.insert(key, tick);
                }
                false
            }
        }
    }
}

fn surface_of(d: &Decision) -> Surface {
    match d {
        Decision::Refuel | Decision::DivertToPump { .. } => Surface::NavigationFuel,
        Decision::CallPaws => Surface::NavigationRescue,
        Decision::Travel { .. } | Decision::Hold { .. } => Surface::NavigationCourse,
        Decision::Repair => Surface::ShipRepair,
        Decision::Book { .. } => Surface::FreightBook,
        Decision::Collect { .. } => Surface::FreightCollect,
    }
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

    // The pid, for `familiar fleet` to know the pilot is aboard.
    let _ = std::fs::write(ship_dir.join("whisker.pid"), std::process::id().to_string());
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
    // Ids the exchange has acknowledged: a re-send of one is a no-op it can skip.
    let mut acked_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    // Loads that left us without paying, by the tick they did: not re-booked for a while.
    let mut lost_at: HashMap<String, i64> = HashMap::new();
    let mut seq: u64 = 0;
    let mut last_refusal = String::new();
    // Wedge watch: a course filed while dry never departs on its own after refuelling
    // (ucf-exchange#16) — docked + course filed + unmoved needs a re-filed travel.
    let mut wedge: Option<(String, Vec<String>)> = None;
    let mut wedge_since: i64 = -1;
    let mut adopted = false;
    // A restart inside a fold we filed: the journal's last "acted" line names the
    // tick it resolves at. Until then the ledger and the load board do not show our
    // own order, and deciding again re-files it (PROD L2831, 2026-09-02: booked,
    // then "rejected: load is not open" for the restart's duplicate, same tick).
    let mut pending_from_journal: i64 = std::fs::read_to_string(ship_dir.join("journal.jsonl"))
        .ok()
        .and_then(|j| {
            j.lines()
                .rev()
                .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                // Any filed action journals its resolve tick — freight ("acted"), the
                // merchant ("traded", "carry-to-market"), the yard ("outfitted"), the
                // drive ("engaged-drive"). A restart that only honoured "acted" bought a
                // 9,000 ℳ fitting on the same tick as a 3,800 ℳ position (2026-09-03).
                .find(|v| v.get("resolves").and_then(Value::as_i64).is_some())
                .and_then(|v| v.get("resolves").and_then(Value::as_i64))
        })
        .unwrap_or(-1);
    // The merchant's speculative book (ADR-0045: lives in the ship's own store).
    let trades = granted.contains(&Automation::Trade);
    let mut last_carry_block = String::new();
    let mut last_merchant_idle = String::new();
    // A filed trade whose fold has not been read back from the receipt trail yet.
    let mut pending_trade: Option<PendingTrade> = None;
    // The world's day, in ticks: the exchange's minimum hold on bought goods is a
    // day (`minHoldTicks` in the pack, not exposed on the wire — LOCAL and PROD both
    // 288). The refusal text corrects us if a world says otherwise.
    let reference = wire.get("/v1/reference").ok();
    let min_hold: i64 = reference
        .as_ref()
        .and_then(|v| v.get("ticksPerDay").and_then(Value::as_i64))
        .unwrap_or(288);
    // The pack's goods that rot in transit (decayBps > 0): what refrigeration is for.
    let perishable: BTreeSet<String> = reference
        .as_ref()
        .and_then(|v| v.get("goods").and_then(Value::as_array))
        .map(|goods| {
            goods
                .iter()
                .filter(|g| g.get("decayBps").and_then(Value::as_i64).unwrap_or(0) > 0)
                .filter_map(|g| g.get("id").and_then(Value::as_str).map(String::from))
                .collect()
        })
        .unwrap_or_default();
    // The ship's fixed daily charges: the mortgage payment the pack names, plus the
    // lease service (not on the wire; ~520/day observed on KK II's lease).
    let mortgage_per_day = reference
        .as_ref()
        .and_then(|v| v.get("params")?.get("mortgagePaymentPerDay")?.as_i64())
        .unwrap_or(600);
    let outfits = granted.contains(&Automation::Outfit);
    let mut deliveries: Vec<DeliveryStat> =
        std::fs::read_to_string(ship_dir.join("deliveries.jsonl"))
            .map(|j| {
                j.lines()
                    .filter_map(|l| serde_json::from_str(l).ok())
                    .collect()
            })
            .unwrap_or_default();
    let mut last_outfit_idle = String::new();
    let mut last_pending_note = String::new();
    let mut last_distress = String::new();
    let mut dial_gate = DialGate {
        dial: Dial::load(&ship_dir),
        last_advice: HashMap::new(),
    };
    let mut holdings: Vec<Holding> = if trades {
        trade::load_holdings(&ship_dir)
    } else {
        Vec::new()
    };

    loop {
        let now = now_secs();
        // The captain may turn the dial at any time.
        dial_gate.dial = Dial::load(&ship_dir);

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
        // The exchange's own word on what is in flight for this key (UCF-Haul#65's
        // pending overlay, 2026-09-02): every accepted, unfolded action with the tick
        // it resolves at — ours from before a restart, or a captain's filed from the
        // desk app on the same key. Wait them out, and look for a booked load again
        // once a `book` among them has folded.
        if let Some(pending) = me.get("pendingActions").and_then(Value::as_array) {
            let latest = pending
                .iter()
                .filter_map(|p| p.get("resolvesAtTick").and_then(Value::as_i64))
                .max();
            if let Some(r) = latest {
                if r + 1 > pending_until {
                    pending_until = r + 1;
                }
            }
            if pending
                .iter()
                .any(|p| p.get("verb").and_then(Value::as_str) == Some("book"))
            {
                adopted = false;
            }
            if !pending.is_empty() && tick <= latest.unwrap_or(-1) {
                let verbs: Vec<&str> = pending
                    .iter()
                    .filter_map(|p| p.get("verb").and_then(Value::as_str))
                    .collect();
                let line = format!("pending {verbs:?}");
                if line != last_pending_note {
                    journal(
                        &ship_dir,
                        json!({"at": now, "tick": tick, "event": "awaiting-pending-actions",
                        "verbs": verbs, "resolves": latest}),
                    );
                    last_pending_note = line;
                }
            } else {
                last_pending_note.clear();
            }
        }
        if pending_from_journal >= 0 {
            if tick <= pending_from_journal {
                journal(
                    &ship_dir,
                    json!({"at": now, "tick": tick, "event": "awaiting-our-own-fold",
                           "resolves": pending_from_journal}),
                );
                std::thread::sleep(Duration::from_secs((tick_secs * 3 / 5).max(floor_secs)));
                continue;
            }
            pending_from_journal = -1;
        }
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
                    let wedge_ok = dial_gate.allow(
                        &ship_dir,
                        Surface::NavigationCourse,
                        tick,
                        now,
                        &json!({"type": "engage", "wedge": route_now.last()}),
                        "engage and re-file the wedged course",
                        "docked with a course on file and nothing moving for 30 ticks",
                    );
                    seq += 1;
                    let engage_id = format!("whisker-{}-{}", now_secs(), seq);
                    let engaged =
                        wedge_ok && wire.act(json!({"type": "engage"}), &engage_id).is_ok();
                    // Never a travel to the berth we are at: a stale route can still
                    // list it after arrival, and the exchange returns the filing
                    // ("no lane route remains to cannery-row", PROD 2026-09-02).
                    if let Some(dest) = route_now
                        .last()
                        .filter(|d| wedge_ok && Some(d.as_str()) != ship.docked.as_deref())
                    {
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
                    // A settled delivery goes on the ship's own record (what the
                    // desk booked, what the fold paid): the outfitting doctrine
                    // reads decay out of it.
                    if reason.starts_with("settled") {
                        let events: Vec<&Value> = me
                            .get("freight")
                            .and_then(Value::as_array)
                            .map(|arr| {
                                arr.iter()
                                    .filter(|f| {
                                        f.get("loadId").and_then(Value::as_str)
                                            == Some(a.row.load_id.as_str())
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        let amount = |word: &str| -> i64 {
                            events
                                .iter()
                                .find(|f| {
                                    f.get("event")
                                        .and_then(Value::as_str)
                                        .map(|e| e.contains(word))
                                        .unwrap_or(false)
                                })
                                .and_then(|f| f.get("freightPaid").and_then(Value::as_i64))
                                .unwrap_or(0)
                        };
                        let stat = DeliveryStat {
                            load_id: a.row.load_id.clone(),
                            good: a.row.good.clone(),
                            perishable: perishable.contains(&a.row.good),
                            booked: amount("booked"),
                            paid: amount("payment taken"),
                        };
                        if let Ok(line) = serde_json::to_string(&stat) {
                            use std::io::Write;
                            if let Ok(mut f) = std::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(ship_dir.join("deliveries.jsonl"))
                            {
                                let _ = writeln!(f, "{line}");
                            }
                        }
                        deliveries.push(stat);
                    }
                    // A load that left us is off the board for a while, and the intent
                    // that booked it is forgotten: the idempotency id of a dead booking
                    // must never answer for a fresh one (LOCAL L1849, 2026-09-01: the
                    // replayed id acked the old fold and the pilot closed and re-booked
                    // the same lapsed contract every 15 ticks).
                    lost_at.insert(a.row.load_id.clone(), tick);
                    let lid = a.row.load_id.clone();
                    recent.retain(|sig, _| !sig.contains(&lid));
                    // Whatever else the ledger says we hold — a second contract the
                    // exchange let us book, a delivery parked uncollected — gets looked
                    // for again now that this one is done (KK II held L2831 AND L2835
                    // on the same lane, 2026-09-02; until T-232's itinerary lands, one
                    // is flown at a time and the other picked up when it closes).
                    adopted = false;
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
        // Only a BERTHED hull with no route engages: the marker stays set while a
        // multi-leg voyage is already flying (PROD, 2026-09-02: fourteen engages in
        // two hours, every one "rejected: already under way"), and a crane at work
        // refuses it too ("the crew is in the housing until t6866") — that refusal
        // names the tick to try again, so it is honoured.
        let crane_until = me
            .get("freight")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter(|f| f.get("outcome").and_then(Value::as_str) == Some("refused"))
                    .filter_map(|f| f.get("event").and_then(Value::as_str))
                    .filter_map(|e| {
                        let i = e.find("until t")?;
                        e[i + 7..]
                            .chars()
                            .take_while(|c| c.is_ascii_digit())
                            .collect::<String>()
                            .parse::<i64>()
                            .ok()
                    })
                    .max()
                    .unwrap_or(-1)
            })
            .unwrap_or(-1);
        let berthed_and_still = ship.docked.is_some() && route_now.is_empty();
        if let Some(dest) = awaiting.filter(|_| berthed_and_still && tick >= crane_until) {
            if tick >= pending_until
                && dial_gate.allow(
                    &ship_dir,
                    Surface::NavigationCourse,
                    tick,
                    now,
                    &json!({"type": "engage"}),
                    &format!("engage the drive for {dest}"),
                    "a course is laid at the drive and the berth is still",
                )
            {
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

        // ── The outfitting phase (Automation::Outfit) ──────────────────────────
        // Berthed, freight idle: buy the next fitting the purse can bear above its
        // reserve — BEFORE the merchant may spend the same cash on a position. A
        // fitting is permanent capacity; a position is one trade (PROD 2026-09-03: the
        // merchant took 3,800 for bluefin on the fold that could have bought the
        // drive-tune). One of each ever, so this fires rarely.
        if outfits
            && tick >= pending_until
            && pending_trade.is_none()
            && active.is_none()
            && !ship.in_flight
        {
            if let Some(here) = ship.docked.clone() {
                let purse = Purse {
                    credits: ship.credits,
                    daily_fixed_cost: mortgage_per_day
                        + if ship.leased {
                            LEASE_SERVICE_PER_DAY_EST
                        } else {
                            0
                        },
                    tank_price: ship.fuel_capacity * FUEL_PRICE_PER_UNIT,
                    titled: !ship.leased,
                    fittings: me
                        .get("fittings")
                        .and_then(Value::as_array)
                        .map(|a| {
                            a.iter()
                                .filter_map(|f| f.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                };
                match outfit::decide_outfit(&purse, &deliveries) {
                    OutfitDecision::Refit { fitting, price }
                        if dial_gate.allow(
                            &ship_dir,
                            Surface::ShipRefit,
                            tick,
                            now,
                            &json!({"type": "refit", "fitting": fitting.wire()}),
                            &format!("buy {} for ℳ{price} at {here}", fitting.wire()),
                            "next fitting the purse can bear above its reserve",
                        ) =>
                    {
                        seq += 1;
                        let id = format!("whisker-{}-{}", now_secs(), seq);
                        match wire.act(json!({"type": "refit", "fitting": fitting.wire()}), &id) {
                            Ok(ack) => {
                                pending_until = ack
                                    .get("resolvesAtTick")
                                    .and_then(Value::as_i64)
                                    .unwrap_or(tick)
                                    + 1;
                                journal(
                                    &ship_dir,
                                    json!({"at": now, "tick": tick, "event": "outfitted",
                                    "fitting": fitting.wire(), "price": price, "at_station": here,
                                    "credits": ship.credits, "reserve": outfit::reserve(&purse), "resolves": pending_until - 1}),
                                );
                                std::thread::sleep(Duration::from_secs(
                                    (tick_secs * 3 / 5).max(floor_secs),
                                ));
                                continue;
                            }
                            Err(e) => journal(
                                &ship_dir,
                                json!({"at": now, "tick": tick,
                                "event": "refit-refused", "fitting": fitting.wire(), "why": e}),
                            ),
                        }
                    }
                    OutfitDecision::Refit { .. } => {} // advised or proposed
                    OutfitDecision::Idle { why } => {
                        if why != last_outfit_idle {
                            journal(
                                &ship_dir,
                                json!({"at": now, "tick": tick, "event": "outfit-idle",
                                "why": why, "credits": ship.credits, "reserve": outfit::reserve(&purse)}),
                            );
                            last_outfit_idle = why;
                        }
                    }
                }
            }
        }

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
            // (Contract freight never appears in the cargo map: nothing to exclude.)
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
            for note in trade::reconcile_hold(&mut holdings, &cargo, &hint, tick) {
                journal(
                    &ship_dir,
                    json!({"at": now, "tick": tick, "event": "book-corrected", "why": note}),
                );
            }
            trade::save_holdings(&ship_dir, &holdings);

            if let Some(here) = ship.docked.clone() {
                // `holdUsed` counts the merchant's goods only (freight never enters the
                // cargo map), so a contract aboard is subtracted here by hand.
                let freight_aboard_units = active
                    .as_ref()
                    .filter(|a| a.word != ActiveWord::Booked)
                    .map(|a| a.row.units)
                    .unwrap_or(0);
                let spare_hold =
                    (ship.hold_capacity - ship.hold_used - freight_aboard_units).max(0);
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
                // Arrived at a position's market and it did not pay: re-aim it now, so
                // the next idle fold does not ferry the goods straight back here.
                if matches!(td, TradeDecision::Idle { .. }) {
                    let mut notes = Vec::new();
                    for h in holdings
                        .iter_mut()
                        .filter(|h| h.sell_target == here && tick >= h.sellable_at)
                    {
                        if let Some(n) = trade::retarget(h, &here, &galaxy) {
                            notes.push(n);
                        }
                    }
                    if !notes.is_empty() {
                        trade::save_holdings(&ship_dir, &holdings);
                        for n in notes {
                            journal(
                                &ship_dir,
                                json!({"at": now, "tick": tick, "event": "retargeted", "why": n}),
                            );
                        }
                    }
                }
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
                //
                // A position may be opened with freight idle OR with the contract's
                // cargo already aboard and room to spare: it rides under the haul
                // either way (the hold clock makes every position a rider). Never
                // while a booking still needs its space.
                let freight_allows_buy = active
                    .as_ref()
                    .map(|a| a.word == ActiveWord::PickedUp)
                    .unwrap_or(true);
                let td = match td {
                    TradeDecision::Buy {
                        good,
                        units,
                        sell_target,
                        est_margin,
                    } if freight_allows_buy => {
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
                    TradeDecision::Buy { good, units, .. } if freight_allows_buy => Some((
                        json!({"type": "buy", "station": here, "good": good, "units": units}),
                        good.clone(),
                        *units,
                        false,
                    )),
                    _ => None,
                };
                if let Some((body, good, units, is_sell)) = trade_body {
                    let surface = if is_sell {
                        Surface::MarketSell
                    } else {
                        Surface::MarketBuy
                    };
                    let describe = format!(
                        "{} {units} {good} at {here}",
                        if is_sell { "sell" } else { "buy" }
                    );
                    let why_text = match &td {
                        TradeDecision::Sell { why, .. } => why.clone(),
                        TradeDecision::Buy {
                            sell_target,
                            est_margin,
                            ..
                        } => {
                            format!("for {sell_target}, est. margin ℳ{est_margin}")
                        }
                        _ => String::new(),
                    };
                    if !dial_gate.allow(&ship_dir, surface, tick, now, &body, &describe, &why_text)
                    {
                        std::thread::sleep(Duration::from_secs(
                            (tick_secs * 3 / 5).max(floor_secs),
                        ));
                        continue;
                    }
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
                            // The carry plus the leg from the market to a pump — the
                            // same PAWS lesson as the freight plan.
                            let cost = wire.fuel_between(&here, &h.sell_target).map(|c| {
                                doctrine::fuel_at_drive(
                                    c + doctrine::onward_to_pump(&h.sell_target, &pumps, &wire),
                                    ship.accel_milli_g,
                                )
                            });
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
                            } else if !dial_gate.allow(
                                &ship_dir,
                                Surface::MarketCarry,
                                tick,
                                now,
                                &json!({"type": "travel", "station": h.sell_target}),
                                &format!("carry {} {} to {}", h.units, h.good, h.sell_target),
                                "the position's clock has passed and no bid here clears it",
                            ) {
                                // advised or proposed; the freight doctrine may still act
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
            // Its OWN marker: `last_refusal` is the lease gate's, and that gate clears
            // it every fold it passes, so sharing it re-journalled the distress on
            // every loop — KK's journal carried the same hold four times across two
            // ticks (2026-09-04). Re-said only when the berth or the tank changes.
            let distress = format!("{:?}|{}", ship.docked, ship.fuel);
            if last_distress != distress {
                journal(
                    &ship_dir,
                    json!({"at": now, "tick": tick, "event": "distress-hold",
                           "docked": ship.docked, "fuel": ship.fuel,
                           "why": "low fuel, no affordable pump; PAWS refused (would strand for days on this world) — holding for a fuelable load or a human"}),
                );
                last_distress = distress;
            }
            std::thread::sleep(Duration::from_secs((tick_secs * 3 / 5).max(floor_secs)));
            continue;
        }
        last_distress.clear();

        let body = match &decision {
            Decision::Hold { .. } => None,
            Decision::Refuel => Some(json!({"type": "refuel"})),
            Decision::Repair => Some(json!({"type": "repair"})),
            Decision::CallPaws => Some(json!({"type": "paws"})),
            Decision::DivertToPump { pump } => Some(json!({"type": "travel", "station": pump})),
            Decision::Travel { station } if Some(station.as_str()) == ship.docked.as_deref() => {
                None
            }
            Decision::Travel { station } => Some(json!({"type": "travel", "station": station})),
            Decision::Book { load_id } => Some(json!({"type": "book", "loadId": load_id})),
            Decision::Collect { load_id } => Some(json!({"type": "collect", "loadId": load_id})),
        };

        let body = body.filter(|b| {
            dial_gate.allow(
                &ship_dir,
                surface_of(&decision),
                tick,
                now,
                b,
                &format!("{decision:?}"),
                "the freight doctrine's decision this fold",
            )
        });
        if let Some(body) = body {
            // One id per INTENT, where an intent is this body filed within the last
            // window of ticks: a re-send inside the window carries the SAME id (a retry
            // after a transient failure is idempotent at the exchange, never a second
            // order — retry the id, never the intent), and the same body after the
            // window is a NEW intent with a NEW id. The earlier revision reused the
            // old id forever, so every later refuel at the same pump, repair, or
            // travel to the same berth was answered with the old fold's ack and
            // nothing happened (LOCAL, 2026-09-02: a Repair "acted" with a resolve
            // tick 366 ticks in the past).
            let sig = body.to_string();
            let (action_id, retry) = match recent.get(&sig) {
                Some((t, id)) if tick - t < 15 => (id.clone(), true),
                _ => {
                    seq += 1;
                    (format!("whisker-{}-{}", now_secs(), seq), false)
                }
            };
            // Inside the window, a decision the exchange already ACKED is not re-sent
            // (the zigzag-to-empty lesson); only one it refused at the door is retried.
            let acked = acked_ids.contains(&action_id);
            if !(retry && acked) {
                match wire.act(body.clone(), &action_id) {
                    Ok(ack) => {
                        recent.insert(sig, (tick, action_id.clone()));
                        acked_ids.insert(action_id);
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
                        // Keep the id: the exchange may have taken the order before the
                        // wire broke, and the retry next fold must carry the same id.
                        recent.insert(sig, (tick, action_id));
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
