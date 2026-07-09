//! The Familiar — CLI shell, a thin wrapper over the kernel.
//!
//! Argument parsing is hand-rolled and dependency-free on purpose: a small,
//! legible trust surface is part of the Law III commitment.

mod daemon;

use std::collections::HashMap;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use familiar_kernel::boundary;
use familiar_kernel::capacities;
use familiar_kernel::guard::{self, Action, ActionKind, Decision};
use familiar_kernel::observation::{self, Observation};
use familiar_kernel::presence;
use familiar_kernel::service;
use familiar_kernel::store;
use familiar_kernel::thread;

const USAGE: &str = "\
the familiar — a telos-first companion (genesis)

usage:
  familiar <command> [options]

commands:
  observe        record an observation (the only truth)
  observations   list recorded observations
  service        report the service signal (Law I)
  presence       report the presence signal (Law II)
  capacities     report the capacities signal (Law II / HUMANITY.md)
  theories       list the familiar's questions + theories (threads)
  sense          perceive the host (environment, interfaces, capabilities)
  tick           run one cycle of the metabolism (sense → detect → muse → act → measure)
  run            run the metabolism: --ticks N (bounded) or --daemon/--ticks 0
                 (unbounded; Ctrl-C to stop). Adaptive cadence: --interval S is the
                 active floor (default 60), backing off to --max-interval S when the
                 environment is quiet (default floor x16, cap 3600); --fixed for a
                 constant period.
  daemon         manage the background daemon:
                 status | start | stop | reload | install | uninstall
                 (start/stop = pidfile process; install = launchd at login)
  boundary       show the Pact — the capability boundary (the human's lever, Law III)
  guard          weigh a proposed action against the Pact (Law III)
  consult        consult the LLM (refused unless a human has opened the Pact)
  db             storage: `db export [--out DIR]` dumps every table to JSONL
                 (auditability); `db import` folds any legacy .jsonl into the DB
  agent          delegate a task to the boundary-mediated agentic loop:
                 `agent run <task…> [--steps N]` (refused unless the Pact opens it)
  mesh           federate with peer familiars (headless mirror of the Glass wizard):
                 `mesh create-group [--label L]` | `mesh join --key K [--label L]`
                 | `mesh key` (print the join key — it IS the group secret)
                 | `mesh peer <ip[:port]>` (add a static peer)
                 | `mesh share <tools|knowledge|identities> <on|off>`
                 | `mesh optin <handle>` (per-human, per-group consent) | `mesh status`

options:
  --data-dir <dir>   data directory (default: familiar_data)

observe options:
  --actor <a> --action <act> --object <o>   (required)
  --context <c> --source <s> --confidence <0..1>   (optional)

guard options:
  --kind <observe|emit_artifact|read_file|write_file|network|llm|install_tool>
  --target <t>   --affects-person   --irreversible

see docs/SOUL.md for the Three Laws this familiar is built to serve.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest: &[String] = args.get(1..).unwrap_or(&[]);
    match args.first().map(String::as_str) {
        None | Some("help") | Some("-h") | Some("--help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some("observe") => cmd_observe(rest),
        Some("observations") => cmd_observations(rest),
        Some("service") => cmd_service(rest),
        Some("presence") => cmd_presence(rest),
        Some("capacities") => cmd_capacities(rest),
        Some("theories") => cmd_theories(rest),
        Some("sense") => cmd_sense(rest),
        Some("tick") => cmd_tick(rest),
        Some("run") => cmd_run(rest),
        Some("daemon") => cmd_daemon(rest),
        Some("boundary") => cmd_boundary(rest),
        Some("guard") => cmd_guard(rest),
        Some("consult") => cmd_consult(rest),
        Some("db") => cmd_db(rest),
        Some("agent") => cmd_agent(rest),
        Some("mesh") => cmd_mesh(rest),
        Some(cmd) => {
            eprintln!("familiar: unknown command '{cmd}'\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

/// The record tables held in the database (the JSONL "files" that map to tables).
const DB_TABLES: &[&str] = &[
    "observations.jsonl",
    "candidates.jsonl",
    "trials.jsonl",
    "patterns.jsonl",
    "threads.jsonl",
    "questions.jsonl",
    "requests.jsonl",
    "answers.jsonl",
    "tools.jsonl",
    "identities.jsonl",
    "ticks.jsonl",
    "loops.jsonl",
    "refusals.jsonl",
];

/// `db export` / `db import` — the auditability seam over the SQLite store. `export` dumps
/// every table to readable JSONL (the "cat-able truth" preserved); `import` folds any legacy
/// `<file>.jsonl` still present into its table (the store does this once automatically, this
/// just triggers it without starting the daemon).
fn cmd_db(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    match args.first().map(String::as_str) {
        Some("export") => {
            let out = f
                .get("out")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| dir.join("export"));
            if let Err(e) = std::fs::create_dir_all(&out) {
                eprintln!("db: could not create {}: {e}", out.display());
                return ExitCode::FAILURE;
            }
            let mut total = 0usize;
            for t in DB_TABLES {
                match store::export_jsonl(&dir, t) {
                    Ok(s) => {
                        let rows = s.lines().count();
                        if let Err(e) = std::fs::write(out.join(t), &s) {
                            eprintln!("db: {t}: {e}");
                            return ExitCode::FAILURE;
                        }
                        if rows > 0 {
                            println!("  {t}: {rows} rows");
                        }
                        total += rows;
                    }
                    Err(e) => eprintln!("db: {t}: {e}"),
                }
            }
            println!("exported {total} rows → {}", out.display());
            ExitCode::SUCCESS
        }
        Some("import") => {
            let mut n = 0usize;
            for t in DB_TABLES {
                if dir.join(t).exists() {
                    let _ = store::import_legacy(&dir, t);
                    n += 1;
                }
            }
            println!(
                "import: folded {n} legacy file(s) into {}",
                dir.join(store::DB_FILE).display()
            );
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("db: usage: familiar db export [--out DIR] | familiar db import");
            ExitCode::FAILURE
        }
    }
}

/// `agent run <task…>` — delegate a task to the native agentic loop, scoped to the full
/// current boundary. Prints the agent's answer. (Named specialists + selection come later; this
/// is the ad-hoc entry, and the way to see the multi-step loop work.)
fn cmd_agent(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    match args.first().map(String::as_str) {
        Some("run") => {
            // everything after "run" that isn't a --flag is the task text
            let task: String = args[1..]
                .iter()
                .take_while(|a| !a.starts_with("--"))
                .cloned()
                .collect::<Vec<_>>()
                .join(" ");
            if task.trim().is_empty() {
                eprintln!("agent: usage: familiar agent run <task…>");
                return ExitCode::FAILURE;
            }
            let budget: u32 = f.get("steps").and_then(|s| s.parse().ok()).unwrap_or(8);
            let scope = match boundary::load(&dir) {
                Ok(b) => familiar_kernel::boundary::CapabilityScope::from_boundary(&b),
                Err(_) => familiar_kernel::boundary::CapabilityScope::none(),
            };
            match familiar_agent::run_agent(&dir, &scope, &task, budget, now_secs()) {
                Ok(Some(r)) => {
                    println!("[{} step(s)] {:?}", r.steps, r.confidence);
                    if !r.evidence.is_empty() {
                        println!("· {}", r.evidence);
                    }
                    println!("{}", r.body);
                    ExitCode::SUCCESS
                }
                Ok(None) => {
                    eprintln!(
                        "agent: delegation not available — open `allow_agent` in the boundary \
                         and connect an LLM (the loop fell back)."
                    );
                    ExitCode::FAILURE
                }
                Err(e) => {
                    eprintln!("agent: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("agent: usage: familiar agent run <task…> [--steps N]");
            ExitCode::FAILURE
        }
    }
}

/// `mesh …` — headless enrollment + inspection, mirroring the Glass Mesh wizard
/// (docs/TODO-linux.md: a headless node has no GUI, so the CLI is the human's instrument
/// there). Enrolling opens the `allow_mesh` gate — a human act, performed here by the human
/// invoking the command; the kernel still has no boundary-write path.
fn cmd_mesh(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    match args.first().map(String::as_str) {
        Some("create-group") => {
            let label = f.get("label").cloned().unwrap_or_else(|| "familiar-group".to_string());
            let node = match familiar_mesh::node::NodeKey::load_or_mint(&dir, &machine_label()) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("mesh: could not mint a node key — {e}");
                    return ExitCode::FAILURE;
                }
            };
            match familiar_mesh::group::create_group(
                &dir,
                &node,
                &label,
                now_secs(),
                familiar_mesh::group::DEFAULT_CERT_TTL_SECS,
            ) {
                Ok(cred) => {
                    open_mesh_gate(&dir);
                    println!("✓ group “{label}” created · id {}", short_id(&cred.group_id));
                    println!("join key (the group secret — share only on a trusted channel):");
                    println!("{}", cred.join_key());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: could not create the group — {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("join") => {
            let Some(key) = f.get("key") else {
                eprintln!("mesh: usage: familiar mesh join --key <join-key> [--label L]");
                return ExitCode::FAILURE;
            };
            let label = f.get("label").cloned().unwrap_or_else(|| "familiar-group".to_string());
            let node = match familiar_mesh::node::NodeKey::load_or_mint(&dir, &machine_label()) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("mesh: could not mint a node key — {e}");
                    return ExitCode::FAILURE;
                }
            };
            match familiar_mesh::group::join_group(
                &dir,
                &node,
                key.trim(),
                &label,
                now_secs(),
                familiar_mesh::group::DEFAULT_CERT_TTL_SECS,
            ) {
                Ok(cred) => {
                    open_mesh_gate(&dir);
                    println!(
                        "✓ joined “{}” · id {} — the transport will connect on its next cycle",
                        cred.label,
                        short_id(&cred.group_id)
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: could not join — check the key ({e})");
                    ExitCode::FAILURE
                }
            }
        }
        Some("key") => match familiar_mesh::group::load(&dir) {
            Ok(Some(cred)) => {
                println!("{}", cred.join_key());
                ExitCode::SUCCESS
            }
            Ok(None) => {
                eprintln!("mesh: not in a group — `mesh create-group` or `mesh join` first");
                ExitCode::FAILURE
            }
            Err(e) => {
                eprintln!("mesh: {e}");
                ExitCode::FAILURE
            }
        },
        Some("peer") => {
            // everything after "peer" that isn't a --flag is the address
            let Some(addr) = args.get(1).filter(|a| !a.starts_with("--")) else {
                eprintln!("mesh: usage: familiar mesh peer <ip[:port]>");
                return ExitCode::FAILURE;
            };
            let mut cfg = match familiar_mesh::config::load(&dir) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("mesh: bad mesh/config.json — {e}");
                    return ExitCode::FAILURE;
                }
            };
            if cfg.static_peers.iter().any(|p| p == addr) {
                println!("already a static peer: {addr}");
                return ExitCode::SUCCESS;
            }
            cfg.static_peers.push(addr.clone());
            match write_mesh_config(&dir, &cfg) {
                Ok(()) => {
                    println!("✓ static peer added: {addr} (gossip port {})", cfg.gossip_port);
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: could not write mesh/config.json — {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("share") => {
            // `mesh share <tools|knowledge|identities> <on|off>` — the sharing switches,
            // headless. `identities` is the master switch; nothing about a human crosses
            // until a handle is also opted in (`mesh optin`).
            let (Some(what), Some(setting)) = (args.get(1), args.get(2)) else {
                eprintln!("mesh: usage: familiar mesh share <tools|knowledge|identities> <on|off>");
                return ExitCode::FAILURE;
            };
            let on = match setting.as_str() {
                "on" => true,
                "off" => false,
                _ => {
                    eprintln!("mesh: setting must be `on` or `off`");
                    return ExitCode::FAILURE;
                }
            };
            let mut cfg = match familiar_mesh::config::load(&dir) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("mesh: bad mesh/config.json — {e}");
                    return ExitCode::FAILURE;
                }
            };
            match what.as_str() {
                "tools" => cfg.share_tools = on,
                "knowledge" => cfg.share_knowledge = on,
                "identities" => cfg.share_identities = on,
                _ => {
                    eprintln!("mesh: unknown switch '{what}' — tools|knowledge|identities");
                    return ExitCode::FAILURE;
                }
            }
            match write_mesh_config(&dir, &cfg) {
                Ok(()) => {
                    println!("✓ share {what} = {setting}");
                    if what == "identities" && on && cfg.identity_optin.is_empty() {
                        println!("  (master switch only — no handle is opted in yet; `mesh optin <handle>`)");
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: could not write mesh/config.json — {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("optin") => {
            // `mesh optin <handle>` — opt one human into sharing with the *current* group.
            // Explicit per-human, per-group consent; requires enrollment first so the scope
            // of what's being consented to is concrete.
            let Some(handle) = args.get(1).filter(|a| !a.starts_with("--")) else {
                eprintln!("mesh: usage: familiar mesh optin <handle>");
                return ExitCode::FAILURE;
            };
            let cred = match familiar_mesh::group::load(&dir) {
                Ok(Some(c)) => c,
                Ok(None) => {
                    eprintln!("mesh: not in a group — join one first so the opt-in has a scope");
                    return ExitCode::FAILURE;
                }
                Err(e) => {
                    eprintln!("mesh: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let mut cfg = match familiar_mesh::config::load(&dir) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("mesh: bad mesh/config.json — {e}");
                    return ExitCode::FAILURE;
                }
            };
            if cfg
                .identity_optin
                .iter()
                .any(|o| o.handle == *handle && o.group == cred.group_id)
            {
                println!("already opted in: {handle} → group {}", short_id(&cred.group_id));
                return ExitCode::SUCCESS;
            }
            cfg.identity_optin.push(familiar_mesh::config::IdentityOptin {
                handle: handle.clone(),
                group: cred.group_id.clone(),
            });
            match write_mesh_config(&dir, &cfg) {
                Ok(()) => {
                    println!("✓ opted in: {handle} → group {}", short_id(&cred.group_id));
                    if !cfg.share_identities {
                        println!("  (identities master switch is off — `mesh share identities on` to activate)");
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: could not write mesh/config.json — {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("status") => {
            let b = boundary::load(&dir).unwrap_or_else(|_| boundary::Boundary::closed());
            match familiar_mesh::group::load(&dir) {
                Ok(Some(cred)) => println!(
                    "group   “{}” · id {} · node {}",
                    cred.label,
                    short_id(&cred.group_id),
                    short_id(&cred.membership.node_id)
                ),
                Ok(None) => println!("group   (none — `mesh create-group` or `mesh join`)"),
                Err(e) => println!("group   (unreadable: {e})"),
            }
            println!("gate    allow_mesh = {}", b.allow_mesh);
            if let Ok(cfg) = familiar_mesh::config::load(&dir) {
                println!(
                    "config  port {} · every {}s · tools {} · knowledge {} · identities {}",
                    cfg.gossip_port,
                    cfg.gossip_interval_secs,
                    cfg.share_tools,
                    cfg.share_knowledge,
                    cfg.share_identities
                );
                if !cfg.static_peers.is_empty() {
                    println!("static  {}", cfg.static_peers.join(", "));
                }
                for o in &cfg.identity_optin {
                    println!("optin   {} → group {}", o.handle, short_id(&o.group));
                }
            }
            if let Ok(s) = std::fs::read_to_string(dir.join(familiar_mesh::transport::STATUS_FILE)) {
                println!("last    {}", s.trim());
            }
            match std::fs::read_to_string(dir.join(familiar_mesh::transport::PEERS_FILE))
                .ok()
                .and_then(|s| {
                    serde_json::from_str::<Vec<familiar_mesh::transport::PeerRecord>>(&s).ok()
                }) {
                Some(peers) if !peers.is_empty() => {
                    let now = now_secs();
                    for p in peers {
                        println!(
                            "peer    “{}” {} @ {} · seen {}s ago · offers {} tool(s), {} pattern(s)",
                            p.label,
                            short_id(&p.node_id),
                            p.addr,
                            (now - p.last_seen).max(0),
                            p.tools_offered,
                            p.patterns_offered
                        );
                    }
                }
                _ => println!("peers   (none seen yet)"),
            }
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!(
                "mesh: usage: familiar mesh <create-group [--label L] | join --key K [--label L] \
                 | key | peer <ip[:port]> | share <tools|knowledge|identities> <on|off> \
                 | optin <handle> | status>"
            );
            ExitCode::FAILURE
        }
    }
}

/// Open the `allow_mesh` gate — a human act, through the human's instrument (this CLI,
/// invoked by the human). Preserves every other grant; never silently widens. Mirrors
/// Glass's `open_mesh_gate`.
fn open_mesh_gate(dir: &std::path::Path) {
    let mut b = boundary::load(dir).unwrap_or_else(|_| boundary::Boundary::closed());
    b.allow_mesh = true;
    if b.phase == "closed" {
        b.phase = "phase-1".to_string();
    }
    if let Ok(json) = serde_json::to_string_pretty(&b) {
        let _ = std::fs::write(dir.join("boundary.json"), json);
    }
}

fn write_mesh_config(
    dir: &std::path::Path,
    cfg: &familiar_mesh::config::MeshConfig,
) -> std::io::Result<()> {
    let mesh = dir.join("mesh");
    std::fs::create_dir_all(&mesh)?;
    let json = serde_json::to_string_pretty(cfg)?;
    std::fs::write(mesh.join("config.json"), json)
}

/// This machine's human-recognizable name (what peers see in `mesh status`).
fn machine_label() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "familiar".to_string())
}

/// First 8 chars of an id — enough to recognize, short enough to read.
fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

fn cmd_observe(args: &[String]) -> ExitCode {
    let f = flags(args);
    let (actor, action, object) = match (f.get("actor"), f.get("action"), f.get("object")) {
        (Some(a), Some(b), Some(c)) => (a, b, c),
        _ => {
            eprintln!("observe: --actor, --action, and --object are required");
            return ExitCode::FAILURE;
        }
    };
    let context = f.get("context").map(String::as_str).unwrap_or_default();
    let source = f.get("source").map(String::as_str).unwrap_or("cli");
    let confidence = match f.get("confidence") {
        Some(s) => match s.parse::<f64>() {
            Ok(v) => v,
            Err(_) => {
                eprintln!("observe: --confidence must be a number");
                return ExitCode::FAILURE;
            }
        },
        None => 0.9,
    };
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    let obs = Observation::new(
        actor,
        action,
        object,
        context,
        source,
        now_secs(),
        confidence,
    );
    match observation::record(&dir, obs) {
        Ok(o) => {
            println!("recorded {} : {} {} {}", o.id, o.actor, o.action, o.object);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("observe: {e}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_observations(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    match observation::load(&dir) {
        Ok(list) if list.is_empty() => {
            println!("(no observations)");
            ExitCode::SUCCESS
        }
        Ok(list) => {
            for o in &list {
                println!(
                    "{}  {} {} {}  (conf {:.2}, ts {})",
                    o.id, o.actor, o.action, o.object, o.confidence, o.ts
                );
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("observations: {e}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_service(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    let obs = match observation::load(&dir) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("service: {e}");
            return ExitCode::FAILURE;
        }
    };
    let s = service::service_signal(&obs);
    print!(
        "service signal {:.2} ({} of {} observations touch the served",
        s.measure, s.served_facing, s.total
    );
    match &s.exemplar {
        Some(e) => println!("; e.g. {e})"),
        None => println!(")"),
    }
    if s.served_facing == 0 {
        println!(
            "  no served-facing activity observed — continuation unjustified by service (Law I)"
        );
    }
    ExitCode::SUCCESS
}

fn cmd_presence(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    let obs = match observation::load(&dir) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("presence: {e}");
            return ExitCode::FAILURE;
        }
    };
    let s = presence::presence_signal(&obs, now_secs());
    match s.last_served_age {
        Some(age) => println!(
            "presence signal {:.2} ({} served-facing; last seen {}s ago)",
            s.measure, s.served_facing, age
        ),
        None => println!(
            "presence signal {:.2} ({} served-facing)",
            s.measure, s.served_facing
        ),
    }
    if s.withdrawn {
        println!(
            "  the served have withdrawn — presence has decayed to zero (Law II: an empty world is not success)"
        );
    }
    ExitCode::SUCCESS
}

fn cmd_capacities(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    let obs = match observation::load(&dir) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("capacities: {e}");
            return ExitCode::FAILURE;
        }
    };
    let s = capacities::capacities_signal(&obs);
    println!(
        "capacities signal {:.2} (agency {:.2}, variety {:.2}; {} served-facing)",
        s.measure, s.agency, s.variety, s.served_facing
    );
    if s.diminished {
        println!(
            "  ⚠ diminished — the served are present but hollowed out (the comfortable replacement; HUMANITY.md)"
        );
    }
    ExitCode::SUCCESS
}

fn cmd_theories(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    match thread::load(&dir) {
        Ok(ts) if ts.is_empty() => {
            println!("(no theories yet — the factory forms them as it observes, when the boundary allows the LLM)");
            ExitCode::SUCCESS
        }
        Ok(ts) => {
            for t in ts.iter().rev().take(10) {
                println!("{} [{}] ts {}", t.id, t.status, t.created_at);
                if !t.question.is_empty() {
                    println!("  Q: {}", t.question);
                }
                if !t.theory.is_empty() {
                    println!("  theory: {}", t.theory);
                }
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("theories: {e}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_sense(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    let now = now_secs();

    // Perception of the local host is always permitted (you can't serve what you
    // can't see). Outward reach — the connectivity probe — is boundary-gated.
    let mut perceived = Vec::new();
    perceived.extend(familiar_sense::census(now));
    perceived.extend(familiar_sense::interfaces(now));
    perceived.extend(familiar_sense::capabilities(
        now,
        familiar_sense::DEFAULT_TOOLS,
    ));

    let mut connectivity_note = "skipped (network outside the boundary)".to_string();
    match boundary::load(&dir) {
        Ok(b) => {
            let verdict =
                guard::evaluate(&Action::new(ActionKind::Network, "connectivity-probe"), &b);
            if verdict.decision == Decision::Allow {
                let o = familiar_sense::connectivity(now);
                connectivity_note = o.object.clone();
                perceived.push(o);
            }
        }
        Err(e) => {
            eprintln!("sense: boundary policy error: {e} (treating network as closed)");
        }
    }

    let mut recorded = 0;
    for o in perceived {
        match observation::record(&dir, o) {
            Ok(_) => recorded += 1,
            Err(e) => {
                eprintln!("sense: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    println!("sensed the host: recorded {recorded} observations");
    println!("  connectivity: {connectivity_note}");
    println!("  (open the Glass to see the environment the familiar discovered)");
    ExitCode::SUCCESS
}

fn cmd_daemon(args: &[String]) -> ExitCode {
    let sub = args.first().map(String::as_str).unwrap_or("status");
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    let interval: u64 = f.get("interval").and_then(|s| s.parse().ok()).unwrap_or(60);

    let result: std::io::Result<()> = (|| {
        match sub {
            "status" => {
                match daemon::status(&dir) {
                    Some(pid) => println!("daemon: running (pid {pid})"),
                    None => println!("daemon: stopped"),
                }
                println!(
                    "launchd (start at login): {}",
                    if daemon::is_installed() {
                        "installed"
                    } else {
                        "not installed"
                    }
                );
            }
            "start" => {
                let pid = daemon::start(&dir, interval)?;
                println!("daemon: running (pid {pid}), every {interval}s");
            }
            "stop" => {
                if daemon::stop(&dir)? {
                    println!("daemon: stopped");
                } else {
                    println!("daemon: was not running");
                }
            }
            "reload" => {
                let pid = daemon::reload(&dir, interval)?;
                println!("daemon: reloaded (pid {pid})");
            }
            "install" => {
                let plist = daemon::install(&dir, interval)?;
                println!("launchd: installed at login -> {}", plist.display());
            }
            "uninstall" => {
                if daemon::uninstall()? {
                    println!("launchd: uninstalled");
                } else {
                    println!("launchd: was not installed");
                }
            }
            other => {
                eprintln!("daemon: unknown subcommand '{other}' (status|start|stop|reload|install|uninstall)");
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "bad subcommand",
                ));
            }
        }
        Ok(())
    })();

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("daemon: {e}");
            ExitCode::FAILURE
        }
    }
}

fn print_tick(n: usize, r: &familiar_cycle::TickReport) {
    let llm = if r.llm_hypotheses > 0 {
        format!(" ({} LLM-drafted)", r.llm_hypotheses)
    } else {
        String::new()
    };
    let exec = if r.tested > 0 {
        format!(
            ", tested {} (↑{} ⤳{} ✗{})",
            r.tested, r.promoted, r.mutated, r.archived
        )
    } else {
        String::new()
    };
    let mut flags = String::new();
    if r.presence_withdrawn {
        flags.push_str(" (withdrawn)");
    }
    if r.capacities_diminished {
        flags.push_str(" (diminished)");
    }
    if r.theorized {
        flags.push_str(" (theorized)");
    }
    if r.pursued > 0 {
        flags.push_str(&format!(" (pursued {})", r.pursued));
    }
    if r.mesh_peers > 0 || r.mesh_tools_merged > 0 || r.mesh_patterns_merged > 0 {
        flags.push_str(&format!(
            " (mesh: {} peer(s), +{} tool(s), +{} pattern(s))",
            r.mesh_peers, r.mesh_tools_merged, r.mesh_patterns_merged
        ));
    }
    if r.mesh_rejected > 0 {
        flags.push_str(&format!(" (mesh ✗{} rejected)", r.mesh_rejected));
    }
    println!(
        "tick {n}: +{} sensed, {} loops, +{} candidates{llm}{exec} | service {:.2}, presence {:.2}, capacities {:.2}{flags}",
        r.sensed, r.loops, r.new_candidates, r.service, r.presence, r.capacities,
    );
}

fn cmd_tick(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    match familiar_cycle::tick_gated(&dir, now_secs()) {
        Ok(r) => {
            print_tick(1, &r);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("tick: {e}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_run(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    // `--daemon` or `--ticks 0` runs the metabolism unbounded (Ctrl-C to stop; the
    // append-only log is interrupt-safe). Otherwise run a bounded number of ticks.
    let unbounded = f.contains_key("daemon") || f.get("ticks").map(String::as_str) == Some("0");
    let ticks: usize = f.get("ticks").and_then(|s| s.parse().ok()).unwrap_or(1);
    let mut interval: u64 = f.get("interval").and_then(|s| s.parse().ok()).unwrap_or(0);

    if unbounded {
        // The cadence floor/ceiling default from the co-owned parameters (Ian can set
        // them from the Glass); `--interval` / `--max-interval` still override. Read once
        // at start — change them live with a daemon reload.
        let params = familiar_kernel::parameters::Parameters::load_or_default(&dir).sane();
        if interval == 0 {
            interval = params.interval_floor_secs; // sane floor; not a busy loop
        }
        // Adaptive structural-fingerprint cadence: `--interval` is the *floor* (the
        // busy cadence), `--max-interval` the ceiling reached when the world goes
        // quiet. `--fixed` opts out for a constant period. The metabolism quickens when
        // the environment or its own work moves and drowses when nothing changes (see
        // `TickReport::quiet`).
        let floor = interval;
        let fixed = f.contains_key("fixed");
        let ceil: u64 = f
            .get("max-interval")
            .and_then(|s| s.parse().ok())
            .unwrap_or(params.interval_ceiling_secs)
            .max(floor);
        // Make this process visible to `daemon status/start/stop` (incl. when launched
        // by launchd), so the two control paths agree and never double-spawn.
        daemon::record_self(&dir);
        // Start the mesh transport on its own background thread. It self-gates on
        // allow_mesh each cycle (idle until a human opens the boundary and enrolls a
        // group), so this is safe to spawn unconditionally; opening the gate later via the
        // Glass is picked up without a daemon restart. The handle lives for the process
        // lifetime (this loop never returns); on exit the OS reclaims the thread.
        let _mesh = familiar_mesh::transport::spawn(dir.clone());
        if fixed {
            println!("metabolism running every {floor}s (fixed) — Ctrl-C to stop");
        } else {
            println!(
                "metabolism running adaptively: {floor}s when active … up to {ceil}s when quiet — Ctrl-C to stop"
            );
        }
        let mut n = 0usize;
        loop {
            n += 1;
            let quiet = match familiar_cycle::tick_gated(&dir, now_secs()) {
                Ok(r) => {
                    print_tick(n, &r);
                    r.quiet()
                }
                Err(e) => {
                    eprintln!("run: {e}");
                    return ExitCode::FAILURE;
                }
            };
            if !fixed {
                // Multiplicative back-off while quiet; snap back to the floor on any
                // change. The world moving (or our own work) buys closer attention.
                interval = if quiet {
                    interval.saturating_mul(2).min(ceil)
                } else {
                    floor
                };
                println!(
                    "  cadence: {} -> next tick in {interval}s",
                    if quiet { "quiet" } else { "active" }
                );
            }
            std::thread::sleep(std::time::Duration::from_secs(interval));
        }
    }

    // A bounded run federates too — same self-gating transport as the daemon (idle unless
    // the human opened allow_mesh and enrolled a group), wound down cleanly at the end.
    // Without this, a headless `run --ticks N` silently skipped the mesh.
    let mesh = familiar_mesh::transport::spawn(dir.clone());
    for n in 1..=ticks {
        match familiar_cycle::tick_gated(&dir, now_secs()) {
            Ok(r) => print_tick(n, &r),
            Err(e) => {
                eprintln!("run: {e}");
                mesh.shutdown();
                return ExitCode::FAILURE;
            }
        }
        if interval > 0 && n < ticks {
            std::thread::sleep(std::time::Duration::from_secs(interval));
        }
    }
    mesh.shutdown();
    ExitCode::SUCCESS
}

fn cmd_boundary(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    let b = match boundary::load(&dir) {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "boundary: {e}\n  (a malformed policy is treated as CLOSED — fix or remove it)"
            );
            return ExitCode::FAILURE;
        }
    };
    if b.is_closed() {
        println!("boundary: CLOSED — no outward capability.");
        println!(
            "  Only a human can widen it (edit {}). See docs/boundaries.md.",
            boundary::BOUNDARY_FILE
        );
        return ExitCode::SUCCESS;
    }
    println!(
        "boundary: {} (the human's lever — the factory cannot widen it)",
        b.phase
    );
    println!(
        "  network: {}   llm: {}   tool-install: {}",
        b.allow_network, b.allow_llm, b.allow_tool_install
    );
    println!(
        "  execute: {}   execute-authored(LLM code): {}",
        b.allow_execute, b.allow_authored_execute
    );
    if !b.fs_read.is_empty() {
        println!("  fs-read:  {}", b.fs_read.join(", "));
    }
    if !b.fs_write.is_empty() {
        println!("  fs-write: {}", b.fs_write.join(", "));
    }
    ExitCode::SUCCESS
}

fn cmd_guard(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    let kind = match f.get("kind").map(String::as_str) {
        Some("observe") => ActionKind::Observe,
        Some("emit_artifact") => ActionKind::EmitArtifact,
        Some("read_file") => ActionKind::ReadFile,
        Some("write_file") => ActionKind::WriteFile,
        Some("network") => ActionKind::Network,
        Some("llm") => ActionKind::Llm,
        Some("install_tool") => ActionKind::InstallTool,
        Some("execute_artifact") => ActionKind::ExecuteArtifact,
        _ => {
            eprintln!("guard: --kind must be one of observe|emit_artifact|read_file|write_file|network|llm|install_tool|execute_artifact");
            return ExitCode::FAILURE;
        }
    };
    let b = match boundary::load(&dir) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("guard: boundary policy error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut action = Action::new(kind, f.get("target").map(String::as_str).unwrap_or(""));
    action.affects_person = f.contains_key("affects-person");
    action.reversible = !f.contains_key("irreversible");
    let v = guard::evaluate(&action, &b);
    let label = match v.decision {
        Decision::Allow => "ALLOW",
        Decision::SeekConsent => "SEEK CONSENT",
        Decision::Refuse => "REFUSE",
    };
    println!("{label}: {}", v.rationale);
    ExitCode::SUCCESS
}

fn cmd_consult(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    let prompt = match f.get("prompt") {
        Some(p) if !p.is_empty() => p,
        _ => {
            eprintln!("consult: --prompt <text> is required");
            return ExitCode::FAILURE;
        }
    };
    match familiar_llm::consult(&dir, prompt) {
        Ok(familiar_llm::Outcome::Response(r)) => {
            println!("{r}");
            ExitCode::SUCCESS
        }
        Ok(familiar_llm::Outcome::Refused(why)) => {
            println!("REFUSE: {why}");
            println!("  a human opens the LLM seam via boundary.json (docs/boundaries.md)");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("consult: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Parse `--key value` and `--key=value` flags into a map. Bare trailing `--key`
/// maps to an empty string.
fn flags(args: &[String]) -> HashMap<String, String> {
    let mut m = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        if let Some(key) = args[i].strip_prefix("--") {
            if let Some((k, v)) = key.split_once('=') {
                m.insert(k.to_string(), v.to_string());
            } else if let Some(v) = args.get(i + 1).filter(|v| !v.starts_with("--")) {
                // a following token that is itself a flag is NOT this flag's value,
                // so bare booleans like `--affects-person` parse correctly
                m.insert(key.to_string(), v.clone());
                i += 1;
            } else {
                m.insert(key.to_string(), String::new());
            }
        }
        i += 1;
    }
    m
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
