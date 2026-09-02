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

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use familiar_mcp::{http, Url};
use familiar_mesh::node::NodeIdentity;
use familiar_whisker::doctrine::{
    self, Active, ActiveWord, Decision, Itinerary, LoadRow, Router, Ship,
};
use familiar_whisker::{env_value, granted_automations, ledger};
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
}

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
        let v = self.get(&format!("/v1/route?from={from}&to={to}")).ok()?;
        let legs = v.get("legs")?.as_array()?;
        Some(legs.iter().filter_map(|l| l.get("fuel")?.as_i64()).sum())
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

    // The plan (T-232): every contract the ship is committed to, booking order.
    // Today's exchange enforces one at a time, so the live plan holds ≤1 stop —
    // the structure already carries more for the day UCF-Haul#43 lifts the cap.
    let mut plan = Itinerary::default();
    let mut pending_until: i64 = -1;
    let mut recent: HashMap<String, (i64, String)> = HashMap::new();
    let mut seq: u64 = 0;
    let mut last_refusal = String::new();
    // Wedge watch: a course filed while dry never departs on its own after refuelling
    // (ucf-exchange#16) — docked + course filed + unmoved needs a re-filed travel.
    let mut wedge: Option<(String, Vec<String>)> = None;
    let mut wedge_since: i64 = -1;
    let mut adopted = false;

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

        // A restart must not forget a held contract — and it must not forget ANY of
        // them. Newest-only adoption abandoned a delivered-but-uncollected payout
        // the moment a second load was open, and under a multi-load exchange
        // (UCF-Haul#43) it would shed the whole rest of the plan on every restart
        // (T-232 audit, item 3). Adopt every load the ledger still shows open, in
        // booking order — the ledger scan speaks reconcile's own closed-vocabulary,
        // `reverted`/`cancel` included, so an undone booking is never re-adopted.
        if !adopted {
            adopted = true;
            for (_, lid) in ledger::open_loads(&me) {
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
                            let word = ledger::reconcile(&me, &lid).unwrap_or(ActiveWord::Booked);
                            plan.stops.push(Active { row, word });
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

        // Reconcile every stop of the plan against the ledger — the fold is the
        // truth, and each contract has its own word (T-232 audit, item 2).
        plan.stops
            .retain_mut(|stop| match ledger::reconcile(&me, &stop.row.load_id) {
                Ok(word) => {
                    stop.word = word;
                    true
                }
                Err(reason) => {
                    journal(
                        &ship_dir,
                        json!({"at": now, "tick": tick, "event": "load-closed",
                           "load": stop.row.load_id, "why": reason, "credits": ship.credits}),
                    );
                    false
                }
            });

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

        // The board, only when the judgment could use it — an empty plan, while the
        // exchange enforces one contract. (When UCF-Haul#43 lifts the cap, this
        // widens to "plan has hold space", matching decide_plan's insertion seam.)
        let board: Vec<LoadRow> = if plan.is_empty() && !ship.in_flight {
            match wire.get("/v1/loadboard?status=open") {
                Ok(Value::Array(rows)) => rows.iter().filter_map(load_row).collect(),
                Ok(_) | Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };

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
                                plan.stops.push(Active {
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
