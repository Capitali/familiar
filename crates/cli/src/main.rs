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
  dossier        what the familiar holds about one person, read by its subject
                 (ADR-0022): `dossier <handle>` — presence shape, standing evidence,
                 needs (stated vs theorized); `dossier withdraw <handle>` removes their
                 contributions, clears the face link, and prints an honest receipt
  actuate        drive a declared control surface by hand (ADR-0032):
                 `actuate <surface> state` reads it; `actuate <surface> <label>` acts —
                 gated by allow_actuate + allow_execute; surfaces come from actuators.json
  sense          perceive the host (environment, interfaces, capabilities)
  reach          assess what the familiar could extend into — discover devices and
                 classify each (agent-capable / protocol-controllable / observable)
  discover       periphery-invoked LAN survey: discover devices + assess reach in one
                 pass, recording the observations that seed the roster and the frontier
  tool prune     purge authored tools that reach the network (LAN scans); --dry-run to list
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
                 | `mesh abandon <node_id>` (retired hardware — hidden from the roster, history kept)
                 | `mesh share <tools|knowledge|identities> <on|off>`
                 | `mesh accept-observations <on|off>` (device agents) | `mesh qr` (enroll a device)
                 | `mesh pending`/`approve <id>`/`deny <id>` (covenant handshake) | `mesh invite`
                 | `mesh optin <handle>` (per-human, per-group consent) | `mesh status`
  outreach       speak to NON-members — the ADR-0013 seam (gated by allow_outreach;
                 every claim citation-checked, every utterance ledgered):
                 `outreach tend <host:port> [--house <host:port>]` (one honest round)
                 | `outreach status` | `outreach proposals`
                 | `outreach approve <id>` (the human's yes — the ONLY way a covenant sends)
                 | `outreach block <host>`

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
        Some("dossier") => cmd_dossier(rest),
        Some("actuate") => cmd_actuate(rest),
        Some("sense") => cmd_sense(rest),
        Some("reach") => cmd_reach(rest),
        Some("discover") => cmd_discover(rest),
        Some("tool") => cmd_tool(rest),
        Some("tick") => cmd_tick(rest),
        Some("run") => cmd_run(rest),
        Some("daemon") => cmd_daemon(rest),
        Some("boundary") => cmd_boundary(rest),
        Some("guard") => cmd_guard(rest),
        Some("consult") => cmd_consult(rest),
        Some("db") => cmd_db(rest),
        Some("agent") => cmd_agent(rest),
        Some("mesh") => cmd_mesh(rest),
        Some("outreach") => cmd_outreach(rest),
        Some("goal") => cmd_goal(rest),
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
    "goals.jsonl",
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

/// `goal …` — seed and inspect the **shared roadmap**. A goal seeded here replicates across the
/// mesh; the node whose capabilities satisfy its `needs` claims it and drives it through the agentic
/// loop, and progress/ownership travel back to every node. High-consequence goals (deploy) are
/// claimed but parked for a human. This is the human's instrument for pointing the mesh at work.
/// `familiar outreach …` — the ADR-0013 seam. Every path here is either perception,
/// an audited utterance (tend), or the human's own yes (approve).
fn cmd_outreach(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    match args.first().map(String::as_str) {
        None | Some("status") => {
            println!("{}", familiar_mesh::outreach::status(&dir));
            ExitCode::SUCCESS
        }
        Some("tend") => {
            let Some(counterparty) = args.get(1).filter(|a| !a.starts_with("--")) else {
                eprintln!("outreach: tend <host:port> [--house <host:port>]");
                return ExitCode::FAILURE;
            };
            let host = counterparty.split(':').next().unwrap_or(counterparty);
            let default_house = format!("{host}:80");
            let house = f.get("house").cloned().unwrap_or(default_house);
            match familiar_mesh::outreach::tend(&dir, counterparty, &house) {
                Ok(report) => {
                    println!("{report}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("outreach: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("proposals") => {
            let ps = familiar_mesh::outreach::load_proposals(&dir);
            if ps.is_empty() {
                println!("(no proposals — trust is earned before terms are drafted)");
            }
            for p in ps {
                println!(
                    "{} [{}] {} — {}\n  laws:  {}\n  offer: {}{}",
                    p.id,
                    p.status,
                    p.counterparty,
                    p.evidence,
                    p.laws,
                    p.offer,
                    if p.response.is_empty() {
                        String::new()
                    } else {
                        format!("\n  reply: {}", p.response)
                    }
                );
            }
            ExitCode::SUCCESS
        }
        Some("approve") => {
            let Some(id) = args.get(1) else {
                eprintln!("outreach: approve <proposal-id>");
                return ExitCode::FAILURE;
            };
            match familiar_mesh::outreach::approve(&dir, id) {
                Ok(out) => {
                    println!("✓ {out}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("outreach: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("block") => {
            let Some(host) = args.get(1) else {
                eprintln!("outreach: block <host>");
                return ExitCode::FAILURE;
            };
            let path = dir.join(familiar_mesh::outreach::BLOCKLIST_FILE);
            let _ = std::fs::create_dir_all(dir.join("outreach"));
            let mut cur = std::fs::read_to_string(&path).unwrap_or_default();
            if !cur.lines().any(|l| l.trim() == host.as_str()) {
                cur.push_str(host);
                cur.push('\n');
                if std::fs::write(&path, cur).is_err() {
                    eprintln!("outreach: could not write the blocklist");
                    return ExitCode::FAILURE;
                }
            }
            println!("✓ {host} blocked — the familiar will not contact it");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!(
                "outreach: unknown subcommand '{other}' (status|tend|proposals|approve|block)"
            );
            ExitCode::FAILURE
        }
    }
}

fn cmd_goal(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    match args.first().map(String::as_str) {
        Some("add") => {
            let Some(desc) = args.get(1).filter(|s| !s.starts_with("--")) else {
                eprintln!("goal: usage: familiar goal add \"<description>\" [--needs cap1,cap2]");
                return ExitCode::FAILURE;
            };
            let needs: Vec<String> = f
                .get("needs")
                .map(|s| {
                    s.split(',')
                        .map(|x| x.trim().to_string())
                        .filter(|x| !x.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            let seq = familiar_kernel::goal::load(&dir)
                .map(|g| g.len())
                .unwrap_or(0)
                + 1;
            let id = format!("goal-{seq:04}");
            // The originator is whoever the familiar is serving now — not a baked creator (ADR-0016).
            let who =
                familiar_kernel::identity::current(&dir).unwrap_or_else(|| "observer".to_string());
            let g = familiar_kernel::goal::Goal::seed(&id, desc, needs, &who, now_secs());
            match familiar_kernel::goal::append(&dir, &g) {
                Ok(()) => {
                    println!("✓ seeded {id} — “{}”", g.description);
                    if !g.needs.is_empty() {
                        println!("  needs: {}", g.needs.join(", "));
                    }
                    if g.is_human_gated() {
                        println!("  (high-consequence: a node will build + test it, but a human approves the deploy)");
                    }
                    println!("  it replicates to the mesh; a capable node will claim it.");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("goal: could not write — {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("list") | None => match familiar_kernel::goal::load(&dir) {
            Ok(gs) if gs.is_empty() => {
                println!("(no goals yet — seed one with `familiar goal add \"…\"`)");
                ExitCode::SUCCESS
            }
            Ok(gs) => {
                for g in gs {
                    let owner = if g.owner_node.is_empty() {
                        "—".to_string()
                    } else {
                        short_id(&g.owner_node)
                    };
                    println!(
                        "· {}  [{}]  owner {}  needs [{}]",
                        g.id,
                        g.status.as_str(),
                        owner,
                        g.needs.join(",")
                    );
                    println!("    {}", g.description);
                    if !g.notes.is_empty() {
                        println!("    ↳ {}", g.notes);
                    }
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("goal: could not read — {e}");
                ExitCode::FAILURE
            }
        },
        Some(other) => {
            eprintln!("goal: unknown subcommand '{other}' — try `add` or `list`");
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
            let label = f
                .get("label")
                .cloned()
                .unwrap_or_else(|| "familiar-group".to_string());
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
                    println!(
                        "✓ group “{label}” created · id {}",
                        short_id(&cred.group_id)
                    );
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
        Some("request-join") => {
            // `mesh request-join --host H [--port P]` — join by covenant: attest the Three Laws
            // and ask to be admitted. The group secret never comes here; we receive only our own
            // cert. Waits (polls) for the human on the other familiar to accept.
            let Some(host) = f.get("host") else {
                eprintln!("mesh: usage: familiar mesh request-join --host <addr> [--port N]");
                return ExitCode::FAILURE;
            };
            let port: u16 = f.get("port").and_then(|p| p.parse().ok()).unwrap_or(47_100);
            let node = match familiar_mesh::node::NodeKey::load_or_mint(&dir, &machine_label()) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("mesh: could not mint a node key — {e}");
                    return ExitCode::FAILURE;
                }
            };
            println!(
                "requesting to join {host}:{port} as node {} — accepting the Three Laws…",
                short_id(&node.node_id())
            );
            match familiar_mesh::enroll::request_join(
                &dir,
                host,
                port,
                &node,
                familiar_mesh::enroll::COVENANT_STATEMENT,
                now_secs(),
            ) {
                Ok(familiar_mesh::enroll::JoinOutcome::Admitted(g)) => {
                    open_mesh_gate(&dir);
                    println!(
                        "✓ admitted to “{}” by covenant — enrolled (no secret held)",
                        g.group_label
                    );
                    ExitCode::SUCCESS
                }
                Ok(familiar_mesh::enroll::JoinOutcome::Pending) => {
                    println!(
                        "… request pending — waiting for the familiar to accept (up to 5 min)"
                    );
                    let mut waited = 0;
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(3));
                        waited += 3;
                        match familiar_mesh::enroll::poll_join(&dir, host, port, &node.node_id()) {
                            Ok(Some(g)) => {
                                open_mesh_gate(&dir);
                                println!(
                                    "✓ admitted to “{}” by covenant — enrolled",
                                    g.group_label
                                );
                                return ExitCode::SUCCESS;
                            }
                            Ok(None) if waited < 300 => continue,
                            Ok(None) => {
                                eprintln!(
                                    "mesh: no decision after 5 min — run again to keep waiting"
                                );
                                return ExitCode::FAILURE;
                            }
                            Err(e) => {
                                eprintln!("mesh: {e}");
                                return ExitCode::FAILURE;
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("mesh: could not request to join — {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("join") => {
            let Some(key) = f.get("key") else {
                eprintln!("mesh: usage: familiar mesh join --key <join-key> [--label L]");
                return ExitCode::FAILURE;
            };
            let label = f
                .get("label")
                .cloned()
                .unwrap_or_else(|| "familiar-group".to_string());
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
        Some("qr") => {
            // `mesh qr [--host H] [--port P]` — the device-enrollment payload: the group secret
            // (which IS membership — show only on a trusted screen), plus where to reach this
            // familiar. Rendered as a scannable terminal QR if `qrencode` is installed (a common
            // CLI), else printed for manual entry. The payload doubles as paste-in JSON.
            let cred = match familiar_mesh::group::load(&dir) {
                Ok(Some(c)) => c,
                Ok(None) => {
                    eprintln!("mesh: not in a group — `mesh create-group` or `mesh join` first");
                    return ExitCode::FAILURE;
                }
                Err(e) => {
                    eprintln!("mesh: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let port = f
                .get("port")
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or_else(|| {
                    familiar_mesh::config::load(&dir)
                        .map(|c| c.gossip_port)
                        .unwrap_or(47_100)
                });
            // Every address the device could reach us at, most-universal first (tailnet, then
            // LAN). An explicit `--host` goes to the front. `host` stays as the single best
            // candidate so v1 clients keep working; new clients read `hosts` and fail over.
            let mut hosts = reachable_hosts();
            // Carry the rendezvous/lighthouse address too, so a device that enrolls by QR/paste
            // still learns the public failover candidate (ADR-0012).
            for h in familiar_mesh::config::load(&dir)
                .unwrap_or_default()
                .rendezvous_hosts
            {
                if !hosts.contains(&h) {
                    hosts.push(h);
                }
            }
            if let Some(h) = f.get("host") {
                hosts.retain(|x| x != h);
                hosts.insert(0, h.clone());
            }
            let host = hosts
                .first()
                .cloned()
                .unwrap_or_else(|| HOST_PLACEHOLDER.to_string());
            // Compact JSON — the phone parses this after scanning or pasting.
            let payload = serde_json::json!({
                "v": 1,
                "secret": cred.join_key(),
                "group": cred.group_id,
                "label": cred.label,
                "host": host,
                "hosts": hosts,
                "port": port,
                // TLS SPKI pin (ADR-0009): the device checks every connection against it.
                "tlspin": familiar_mesh::transport::tls_spki_pin(&dir).unwrap_or_default(),
                // The group's pin set — a device accepts any member's cert so it can fail over
                // to a sibling (the lighthouse) it can reach (ADR-0012).
                "pins": familiar_mesh::transport::advertised_pins(&dir),
            })
            .to_string();
            println!("enrollment payload (contains the group secret — trusted screen only):");
            println!("{payload}");
            if !render_qr(&payload) {
                println!(
                    "\n(install `qrencode` to show a scannable QR — `brew install qrencode`; \
                     until then paste the payload into the device app)"
                );
            }
            if host_is_placeholder(&host) {
                println!(
                    "note: could not detect a reachable address — pass `--host <tailscale-or-lan-ip>` \
                     so the device knows where to reach this familiar."
                );
            }
            ExitCode::SUCCESS
        }
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
                    println!(
                        "✓ static peer added: {addr} (gossip port {})",
                        cfg.gossip_port
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: could not write mesh/config.json — {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("abandon") => {
            // `mesh abandon <node_id>` — decommissioned hardware, a retired VM. The RECOMMENDED
            // way to clean the roster: unlike `forget`, this never deletes the record. It's
            // excluded from the active roster/worldview but the full history (first_seen,
            // total_online_secs, tools/patterns it once offered) stays. Any fresh contact from
            // that node revives it automatically — a human re-abandons if it turns out to be a
            // one-off blip, not a real departure.
            let Some(node_id) = args.get(1).filter(|a| !a.starts_with("--")) else {
                eprintln!("mesh: usage: familiar mesh abandon <node_id>");
                return ExitCode::FAILURE;
            };
            match familiar_mesh::transport::abandon_peer(&dir, node_id) {
                Ok(true) => {
                    println!("✓ {node_id} marked abandoned — hidden from the active roster, history kept");
                    ExitCode::SUCCESS
                }
                Ok(false) => {
                    eprintln!("mesh: no roster entry for {node_id}");
                    ExitCode::FAILURE
                }
                Err(e) => {
                    eprintln!("mesh: could not update the roster — {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("forget") => {
            // `mesh forget <node_id>` — hard delete, the record is gone for good (no history
            // kept). Prefer `mesh abandon` for a real departure (decommissioned hardware) —
            // this is for correcting a mistaken/test entry, not normal roster hygiene.
            let Some(node_id) = args.get(1).filter(|a| !a.starts_with("--")) else {
                eprintln!("mesh: usage: familiar mesh forget <node_id>");
                return ExitCode::FAILURE;
            };
            match familiar_mesh::transport::remove_peer(&dir, node_id) {
                Ok(true) => {
                    println!("✓ forgot {node_id} — removed from the roster");
                    ExitCode::SUCCESS
                }
                Ok(false) => {
                    eprintln!("mesh: no roster entry for {node_id}");
                    ExitCode::FAILURE
                }
                Err(e) => {
                    eprintln!("mesh: could not update the roster — {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("roster") => {
            // `mesh roster` — every member with its full metadata, one block per node.
            let now = familiar_mesh::transport::now_secs();
            let members = familiar_mesh::members::classify(&dir, now);
            if members.is_empty() {
                println!("(no mesh members — is the mesh gate open and a group enrolled?)");
                return ExitCode::SUCCESS;
            }
            let date = |ts: i64| -> String {
                if ts <= 0 {
                    "—".into()
                } else {
                    // civil date from unix secs, UTC — no chrono dependency for a roster print
                    let days = ts / 86400;
                    let (y, m, d) = civil_from_days(days);
                    format!(
                        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
                        (ts % 86400) / 3600,
                        (ts % 3600) / 60
                    )
                }
            };
            let dur = |secs: i64| -> String {
                if secs <= 0 {
                    "—".into()
                } else if secs < 3600 {
                    format!("{}m", secs / 60)
                } else if secs < 86400 {
                    format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
                } else {
                    format!("{}d {}h", secs / 86400, (secs % 86400) / 3600)
                }
            };
            for m in &members {
                let who = if m.human.is_empty() {
                    "—".into()
                } else {
                    m.human.clone()
                };
                // Present: who is actually there now, for how long, and how we know — or
                // plainly "unknown" when no evidence is fresh. Distinct from "serves".
                let present = if m.present_human.is_empty() && m.present_via.is_empty() {
                    "unknown".to_string()
                } else {
                    let who = if m.present_human.is_empty() {
                        "someone".to_string()
                    } else {
                        m.present_human.clone()
                    };
                    let how = if m.present_via.is_empty() {
                        String::new()
                    } else {
                        format!(" via {}", m.present_via)
                    };
                    let howlong = if m.present_since > 0 {
                        format!(" for {}", dur(now - m.present_since))
                    } else {
                        String::new()
                    };
                    format!("{who}{howlong}{how}")
                };
                println!(
                    "{} “{}” [{}]\n  status    {}{}\n  present   {}\n  joined    first {} · session {} · total online {}\n  platform  {} {} · familiar v{}\n  human     interactive {} · serves {}\n  offers    {} tool(s), {} pattern(s) · trust {} · addr {}",
                    match m.kind {
                        familiar_mesh::members::MemberKind::SelfNode => "self  ",
                        familiar_mesh::members::MemberKind::GossipPeer => "peer  ",
                        familiar_mesh::members::MemberKind::DevicePeer => "device",
                        familiar_mesh::members::MemberKind::DeviceAgent => "agent ",
                    },
                    m.label,
                    m.node_id.chars().take(8).collect::<String>(),
                    m.status,
                    if m.status == "online" {
                        String::new()
                    } else {
                        format!(" (last seen {} ago)", dur(now - m.last_seen))
                    },
                    present,
                    date(m.first_seen),
                    if m.session_start > 0 {
                        date(m.session_start)
                    } else {
                        "—".into()
                    },
                    dur(m.total_online_secs),
                    m.os,
                    m.os_version,
                    if m.familiar_version.is_empty() { "?" } else { &m.familiar_version },
                    if m.interactive { "yes" } else { "no" },
                    who,
                    m.tools,
                    m.patterns,
                    m.trust,
                    if m.addr.is_empty() { "—" } else { &m.addr },
                );
            }
            ExitCode::SUCCESS
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
        Some("accept-observations") => {
            // `mesh accept-observations <on|off>` — the device-ingestion switch. Separate from
            // `allow_mesh`: federation can be on while device agents (iPhone/Watch) are refused.
            let Some(setting) = args.get(1) else {
                eprintln!("mesh: usage: familiar mesh accept-observations <on|off>");
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
            cfg.accept_observations = on;
            match write_mesh_config(&dir, &cfg) {
                Ok(()) => {
                    println!("✓ accept-observations = {setting}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: could not write mesh/config.json — {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("auto-accept") => {
            // `mesh auto-accept <on|off>` — a standing invite: auto-admit any node that attests the
            // Laws and asks, without a per-device tap. Convenient on a trusted network.
            let Some(setting) = args.get(1) else {
                eprintln!("mesh: usage: familiar mesh auto-accept <on|off>");
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
            cfg.auto_accept_enrollments = on;
            match write_mesh_config(&dir, &cfg) {
                Ok(()) => {
                    println!("✓ auto-accept = {setting}");
                    if on {
                        println!("  (any device that attests the Laws and reaches this familiar is now admitted automatically)");
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: could not write mesh/config.json — {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("auto-peer") => {
            // `mesh auto-peer <on|off>` — the bootstrap side of automatic peering: with no covenant
            // yet and the gate open, reach out to the tailnet and ask to join. Pairs with a peer's
            // `auto-accept` so a fresh node self-enrolls. Never fires once we already hold a group.
            let Some(setting) = args.get(1) else {
                eprintln!("mesh: usage: familiar mesh auto-peer <on|off>");
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
            cfg.auto_peer = on;
            match write_mesh_config(&dir, &cfg) {
                Ok(()) => {
                    println!("✓ auto-peer = {setting}");
                    if on {
                        println!("  (with the gate open and no group yet, this node will seek a covenant on the tailnet)");
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: could not write mesh/config.json — {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("pending") => {
            // `mesh pending` — the covenant handshake's inbox: nodes that attested the Laws and
            // are waiting for you to extend the covenant. Approve/deny by their code or node id.
            match familiar_mesh::enroll::list_pending(&dir) {
                Ok(ps) if ps.is_empty() => {
                    println!("(no pending join requests)");
                    ExitCode::SUCCESS
                }
                Ok(ps) => {
                    let now = now_secs();
                    for p in ps {
                        println!(
                            "· {}  “{}”  node {}  · {}s ago",
                            p.code,
                            p.node.label,
                            short_id(&p.node.node_id),
                            (now - p.received_at).max(0)
                        );
                        println!(
                            "    attests (v{}): {}",
                            p.attestation.laws_version, p.attestation.statement
                        );
                        println!("    approve: familiar mesh approve {}", p.node.node_id);
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("approve") => {
            // `mesh approve <node_id>` — extend the covenant: mint this node's membership cert.
            // The join key never leaves this familiar; the node gets only a cert bound to its key.
            let Some(node_id) = args.get(1).filter(|a| !a.starts_with("--")) else {
                eprintln!("mesh: usage: familiar mesh approve <node_id>");
                return ExitCode::FAILURE;
            };
            match familiar_mesh::enroll::approve(&dir, node_id, now_secs()) {
                Ok(g) => {
                    println!(
                        "✓ admitted {} to group “{}” — its agent can now enroll and serve (revoke by \
                         node id in mesh/revoked.json)",
                        short_id(&g.membership.node_id),
                        g.group_label
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: could not approve — {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("deny") => {
            let Some(node_id) = args.get(1).filter(|a| !a.starts_with("--")) else {
                eprintln!("mesh: usage: familiar mesh deny <node_id>");
                return ExitCode::FAILURE;
            };
            match familiar_mesh::enroll::deny(&dir, node_id, now_secs()) {
                Ok(true) => {
                    println!("✓ denied {}", short_id(node_id));
                    ExitCode::SUCCESS
                }
                Ok(false) => {
                    eprintln!("mesh: no pending request for {node_id}");
                    ExitCode::FAILURE
                }
                Err(e) => {
                    eprintln!("mesh: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("grant") => {
            // `mesh grant <target_node> <enrollment|question|gate> <ref> <approve|deny> [note...]`
            // A human's decision on a peer's authority request, relayed back for that peer to apply.
            // This is a human act — only run it when you've actually decided. For a headless peer that
            // asked to open its execute gate, `mesh grant <node> gate allow_execute approve`.
            let (Some(target), Some(kind), Some(ref_id), Some(dec)) =
                (args.get(1), args.get(2), args.get(3), args.get(4))
            else {
                eprintln!("mesh: usage: familiar mesh grant <target_node> <enrollment|question|gate> <ref_id> <approve|deny> [note]");
                return ExitCode::FAILURE;
            };
            let approved = match dec.as_str() {
                "approve" | "yes" | "y" => true,
                "deny" | "no" | "n" => false,
                _ => {
                    eprintln!("mesh: decision must be approve or deny");
                    return ExitCode::FAILURE;
                }
            };
            let note = args.get(5..).map(|s| s.join(" ")).unwrap_or_default();
            let by = familiar_mesh::group::load(&dir)
                .ok()
                .flatten()
                .map(|c| c.membership.node_id)
                .unwrap_or_default();
            let grant = familiar_mesh::brief::AuthorityGrant {
                by,
                target: target.to_string(),
                kind: kind.to_string(),
                ref_id: ref_id.to_string(),
                approved,
                note,
                ts: now_secs(),
            };
            match familiar_mesh::grants::record(&dir, grant) {
                Ok(()) => {
                    println!(
                        "✓ recorded your decision ({}) on {}'s {} — it rides the next briefs to that peer",
                        if approved { "approve" } else { "deny" },
                        short_id(target),
                        kind
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: could not record grant — {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("invite") => {
            // `mesh invite [--minutes N]` — pairing mode: authorize an expansion once so devices
            // you bring in during the window enroll without a tap each (default 10 min).
            let minutes: i64 = f.get("minutes").and_then(|m| m.parse().ok()).unwrap_or(10);
            let until = now_secs() + minutes.max(1) * 60;
            match familiar_mesh::enroll::open_invite(&dir, until) {
                Ok(()) => {
                    println!(
                        "✓ inviting for {minutes} min — join requests that arrive now are auto-admitted \
                         to the covenant. Unsolicited joiners after that wait for `mesh approve`."
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: {e}");
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
                println!(
                    "already opted in: {handle} → group {}",
                    short_id(&cred.group_id)
                );
                return ExitCode::SUCCESS;
            }
            cfg.identity_optin
                .push(familiar_mesh::config::IdentityOptin {
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
                    "config  port {} · every {}s · tools {} · knowledge {} · identities {} · accept-obs {} · auto-accept {}",
                    cfg.gossip_port,
                    cfg.gossip_interval_secs,
                    cfg.share_tools,
                    cfg.share_knowledge,
                    cfg.share_identities,
                    cfg.accept_observations,
                    cfg.auto_accept_enrollments
                );
                if !cfg.static_peers.is_empty() {
                    println!("static  {}", cfg.static_peers.join(", "));
                }
                for o in &cfg.identity_optin {
                    println!("optin   {} → group {}", o.handle, short_id(&o.group));
                }
            }
            // Covenant handshake: any nodes waiting to be admitted, and the invite window.
            if let Ok(ps) = familiar_mesh::enroll::list_pending(&dir) {
                if !ps.is_empty() {
                    println!(
                        "pending {} join request(s) — `mesh pending` to review",
                        ps.len()
                    );
                }
            }
            let invite_left = familiar_mesh::enroll::invite_until(&dir) - now_secs();
            if invite_left > 0 {
                println!("invite  open — auto-admitting for {}s", invite_left);
            }
            if let Ok(s) = std::fs::read_to_string(dir.join(familiar_mesh::transport::STATUS_FILE))
            {
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
        // ---- federation (ADR-0033): meshes are peers too ---------------------------------------
        Some("federate") => {
            let sub = args.get(1).map(String::as_str);
            match sub {
                Some("invite") => {
                    // `mesh federate invite` — mint the pasteable payload one mesh's operator
                    // hands another's. Member-signed, single-use, ten minutes, no secrets.
                    let cred = match familiar_mesh::group::load(&dir) {
                        Ok(Some(c)) => c,
                        _ => {
                            eprintln!("mesh: no group here — create or join one first");
                            return ExitCode::FAILURE;
                        }
                    };
                    let node = match familiar_mesh::node::NodeKey::load_or_mint(&dir, &cred.label) {
                        Ok(n) => n,
                        Err(e) => {
                            eprintln!("mesh: no node key — {e}");
                            return ExitCode::FAILURE;
                        }
                    };
                    let hosts = familiar_mesh::transport::federation_hosts(&dir);
                    match familiar_mesh::federation::mint_mesh_invite(
                        &node,
                        &cred.membership,
                        &cred,
                        hosts,
                        now_secs(),
                    )
                    .and_then(|i| i.encode())
                    {
                        Ok(payload) => {
                            println!(
                                "mesh invite for “{}” — single use, expires in 10 minutes.\n\
                                 Hand it to the other mesh's operator; they run:\n\
                                 \n  familiar mesh federate join {payload}\n",
                                cred.label
                            );
                            ExitCode::SUCCESS
                        }
                        Err(e) => {
                            eprintln!("mesh: {e}");
                            ExitCode::FAILURE
                        }
                    }
                }
                Some("join") => {
                    // `mesh federate join <payload>` — redeem another mesh's invite: introduce
                    // ourselves at their door, adopt their answer as a standing sibling.
                    let Some(payload) = args.get(2) else {
                        eprintln!("mesh: usage: familiar mesh federate join <invite-payload>");
                        return ExitCode::FAILURE;
                    };
                    match familiar_mesh::federation::federate_with(&dir, payload) {
                        Ok(s) => {
                            println!(
                                "✓ “{}” stands as a sibling here. Their door holds us pending — \
                                 a member there still has to welcome us.",
                                s.handle
                            );
                            ExitCode::SUCCESS
                        }
                        Err(e) => {
                            eprintln!("mesh: {e}");
                            ExitCode::FAILURE
                        }
                    }
                }
                Some("welcome") => {
                    // `mesh federate welcome <group_id>` — the human's tap: a pending
                    // introduction stands as a sibling. Same trust class as /local/standing:
                    // a human at this machine's own console is the authority.
                    let Some(gid) = args.get(2) else {
                        eprintln!("mesh: usage: familiar mesh federate welcome <group_id>");
                        return ExitCode::FAILURE;
                    };
                    match familiar_mesh::federation::welcome_sibling(&dir, gid, "cli", now_secs()) {
                        Ok(s) => {
                            println!(
                                "✓ “{}” stands as a sibling — it now reads at the sibling rung",
                                s.handle
                            );
                            ExitCode::SUCCESS
                        }
                        Err(e) => {
                            eprintln!("mesh: {e}");
                            ExitCode::FAILURE
                        }
                    }
                }
                Some("sever") => {
                    // `mesh federate sever <group_id> [--reason R]` — standing withdrawal,
                    // not attack: the record stays, with its reason.
                    let Some(gid) = args.get(2) else {
                        eprintln!(
                            "mesh: usage: familiar mesh federate sever <group_id> [--reason R]"
                        );
                        return ExitCode::FAILURE;
                    };
                    let reason = f.get("reason").cloned().unwrap_or_default();
                    match familiar_mesh::federation::sever_sibling(&dir, gid, &reason, now_secs()) {
                        Ok(s) => {
                            println!("✓ “{}” severed — its reads fail closed; `federate welcome` restores", s.handle);
                            ExitCode::SUCCESS
                        }
                        Err(e) => {
                            eprintln!("mesh: {e}");
                            ExitCode::FAILURE
                        }
                    }
                }
                Some("read") => {
                    // `mesh federate read <group_id>` — read a sibling's worldview at the
                    // sibling rung (the projection drill, end to end).
                    let Some(gid) = args.get(2) else {
                        eprintln!("mesh: usage: familiar mesh federate read <group_id>");
                        return ExitCode::FAILURE;
                    };
                    match familiar_mesh::federation::read_sibling_worldview(&dir, gid) {
                        Ok(json) => {
                            match serde_json::from_str::<serde_json::Value>(&json) {
                                Ok(v) => {
                                    println!(
                                        "sibling “{}” — {} members, {} observations, declares: {}",
                                        v.get("group_label")
                                            .and_then(|s| s.as_str())
                                            .unwrap_or("?"),
                                        v.get("members")
                                            .and_then(|m| m.as_array())
                                            .map(|a| a.len())
                                            .unwrap_or(0),
                                        v.get("observation_count")
                                            .and_then(|n| n.as_u64())
                                            .unwrap_or(0),
                                        v.get("declared_areas")
                                            .and_then(|a| a.as_array())
                                            .map(|a| a
                                                .iter()
                                                .filter_map(|s| s.as_str())
                                                .collect::<Vec<_>>()
                                                .join(", "))
                                            .filter(|s| !s.is_empty())
                                            .unwrap_or_else(|| "nothing yet".into()),
                                    );
                                }
                                Err(_) => println!("{json}"),
                            }
                            ExitCode::SUCCESS
                        }
                        Err(e) => {
                            eprintln!("mesh: {e}");
                            ExitCode::FAILURE
                        }
                    }
                }
                Some("list") | None => {
                    let sibs = familiar_mesh::federation::load_siblings(&dir);
                    if sibs.is_empty() {
                        println!("federation: no siblings — `mesh federate invite` opens the door");
                    } else {
                        println!("siblings ({}):", sibs.len());
                        for s in sibs {
                            println!(
                                "  {}  “{}”  {}  areas: {}{}",
                                short_id(&s.group_id),
                                s.handle,
                                s.state,
                                if s.declared_areas.is_empty() {
                                    "—".into()
                                } else {
                                    s.declared_areas.join(", ")
                                },
                                if s.note.is_empty() {
                                    String::new()
                                } else {
                                    format!("  ({})", s.note)
                                },
                            );
                        }
                    }
                    ExitCode::SUCCESS
                }
                Some(other) => {
                    eprintln!(
                        "mesh: federate {other}? — invite | join | welcome | sever | read | list"
                    );
                    ExitCode::FAILURE
                }
            }
        }
        // ---- standing (ADR-0020): membership decides reading; standing decides seeing ---------
        Some("standing") => {
            let sub = args.get(1).map(String::as_str);
            match sub {
                Some("show") | None => {
                    let roll = familiar_mesh::standing::load(&dir);
                    if roll.full.is_empty() {
                        println!("standing: nobody on the roll — every member reads as a guest");
                    } else {
                        println!("full standing ({}):", roll.full.len());
                        for id in &roll.full {
                            let note = roll.notes.get(id).map(String::as_str).unwrap_or("");
                            println!("  {id}  {note}");
                        }
                    }
                    ExitCode::SUCCESS
                }
                Some("grant") => {
                    let Some(node_id) = args.get(2) else {
                        eprintln!("mesh: usage: familiar mesh standing grant <node_id> [--note N]");
                        return ExitCode::FAILURE;
                    };
                    let note = f.get("note").cloned().unwrap_or_default();
                    match familiar_mesh::standing::grant(&dir, node_id, &note) {
                        Ok(true) => {
                            println!("✓ {node_id} now reads at full standing");
                            ExitCode::SUCCESS
                        }
                        Ok(false) => {
                            println!("standing: {node_id} already stood (or empty id) — no change");
                            ExitCode::SUCCESS
                        }
                        Err(e) => {
                            eprintln!("mesh: {e}");
                            ExitCode::FAILURE
                        }
                    }
                }
                Some("revoke") => {
                    let Some(node_id) = args.get(2) else {
                        eprintln!("mesh: usage: familiar mesh standing revoke <node_id>");
                        return ExitCode::FAILURE;
                    };
                    match familiar_mesh::standing::revoke(&dir, node_id) {
                        Ok(true) => {
                            // Narrows what they SEE; the membership itself is untouched.
                            println!("✓ {node_id} returns to guest (still a member — `mesh abandon` removes)");
                            ExitCode::SUCCESS
                        }
                        Ok(false) => {
                            println!("standing: {node_id} was not on the roll — no change");
                            ExitCode::SUCCESS
                        }
                        Err(e) => {
                            eprintln!("mesh: {e}");
                            ExitCode::FAILURE
                        }
                    }
                }
                Some(other) => {
                    eprintln!("mesh: standing {other}? — show | grant <node_id> [--note N] | revoke <node_id>");
                    ExitCode::FAILURE
                }
            }
        }
        // ---- corrections (ADR-0026 §5): the deliberate reversals -----------------------------
        // Low-ceremony by design (ADR-0022): trust extended automatically must be cheap to
        // withdraw deliberately. These live here and on the roster card — never on the welcome.
        Some(act @ ("sever" | "hold" | "disestablish" | "restore")) => {
            let Some(node_id) = args.get(1).filter(|a| !a.starts_with("--")) else {
                eprintln!("mesh: usage: familiar mesh {act} <node_id> [--reason R]");
                return ExitCode::FAILURE;
            };
            let reason = f.get("reason").cloned().unwrap_or_else(|| match act {
                "sever" => "removed by the operator".into(),
                "hold" => "not now".into(),
                "disestablish" => "identity was wrong".into(),
                _ => "restored".into(),
            });
            let act_kind = match act {
                "sever" => familiar_mesh::record::CorrectionAct::Sever,
                "hold" => familiar_mesh::record::CorrectionAct::Hold,
                "disestablish" => familiar_mesh::record::CorrectionAct::Disestablish,
                _ => familiar_mesh::record::CorrectionAct::Restore,
            };
            let corrected_by = familiar_mesh::node::NodeKey::load_or_mint(&dir, "familiar")
                .map(|n| n.node_id())
                .unwrap_or_else(|_| "local".into());
            let now = now_secs();
            let c = familiar_mesh::record::Correction {
                act: act_kind,
                subject_device: node_id.clone(),
                corrected_by,
                reason,
                ts: now,
                nonce: format!("cli-{act}-{node_id}-{now}"),
                sig: String::new(), // the local human IS the authority; signatures gate the wire
            };
            match familiar_mesh::record::apply_correction(&dir, &c, now) {
                Ok(r) => {
                    println!(
                        "✓ {act} {node_id} — record now: {:?}",
                        familiar_mesh::record::derive_state(&r)
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("name") => {
            // `mesh name <node_id> <handle>` — the human at this door naming an established
            // device whose establishment carries no handle (the roll migration deliberately
            // wrote "" rather than invent names). The name is what the E2/E4 guardrails and
            // the voucher path key on, so an unnamed fleet cannot protect or vouch for anyone.
            let (Some(node_id), Some(handle)) = (
                args.get(1).filter(|a| !a.starts_with("--")),
                args.get(2).filter(|a| !a.starts_with("--")),
            ) else {
                eprintln!("mesh: usage: familiar mesh name <node_id> <handle>");
                return ExitCode::FAILURE;
            };
            match familiar_mesh::record::name_established(&dir, node_id, handle, now_secs()) {
                Ok(_) => {
                    println!("✓ {node_id} established as “{handle}”");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("invite-token") => {
            // `mesh invite-token [--handle H]` — a member's deliberate act, displaced in time:
            // single-use, ten minutes, carries NO secret. Naming a handle lets the newcomer
            // attach to that existing identity; unnamed establishes a new one.
            let handle = f.get("handle").cloned().unwrap_or_default();
            let Ok(Some(cred)) = familiar_mesh::group::load(&dir) else {
                eprintln!("mesh: no group enrolled");
                return ExitCode::FAILURE;
            };
            let Ok(node) = familiar_mesh::node::NodeKey::load_or_mint(&dir, "familiar") else {
                eprintln!("mesh: no node key");
                return ExitCode::FAILURE;
            };
            match familiar_mesh::record::mint_invite_token(
                &node,
                &cred.membership,
                &handle,
                now_secs(),
            ) {
                Ok(t) => {
                    eprintln!(
                        "single use, expires in {} min{}",
                        familiar_mesh::record::INVITE_TOKEN_TTL_SECS / 60,
                        if handle.is_empty() {
                            String::new()
                        } else {
                            format!(", names “{handle}”")
                        }
                    );
                    println!("{}", serde_json::to_string(&t).unwrap_or_default());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        // ---- the unified record (ADR-0026, Phase 2) ------------------------------------------
        Some("migrate-records") => {
            // `mesh migrate-records` — the ONE migration: fold granted/ + pending/ + peers +
            // standing.json + revoked.json + denied/ into mesh/records/. Idempotent; the legacy
            // stores are left untouched and stay authoritative until `read_records` flips.
            match familiar_mesh::record::migrate(&dir, now_secs()) {
                Ok(r) => {
                    println!(
                        "✓ folded the legacy stores into mesh/records/ ({} records)",
                        r.records
                    );
                    println!(
                        "  from grants: {}   from pending: {}   from peers: {}",
                        r.from_granted, r.from_pending, r.from_peers
                    );
                    println!("  established from the roll: {}   severed from revoked.json: {}   holds: {}", r.established_from_roll, r.severed_from_revoked, r.held_from_denials);
                    println!(
                        "  next: `familiar mesh doctor` — flip read_records only on a clean report"
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: migration failed — {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("doctor") => {
            // `mesh doctor` — the Phase 2 deploy gate: compare every membership answer the
            // legacy stores give against the record's answer. Flip read_records on a clean
            // report only; a divergent row means the fold missed something real.
            let report = familiar_mesh::record::doctor(&dir, now_secs());
            if report.rows.is_empty() {
                println!("doctor: nothing to compare — no grants, peers, roll or records here");
                return ExitCode::SUCCESS;
            }
            println!(
                "{:<18} {:>7} {:>7} {:>8} {:>8}  ok",
                "node", "roll", "record", "revoked", "severed"
            );
            for row in &report.rows {
                println!(
                    "{:<18} {:>7} {:>7} {:>8} {:>8}  {}",
                    row.node_id.chars().take(16).collect::<String>(),
                    row.legacy_standing,
                    row.record_standing,
                    if row.legacy_revoked { "yes" } else { "-" },
                    if row.record_severed { "yes" } else { "-" },
                    if row.ok { "✓" } else { "✗ DIVERGENT" },
                );
            }
            for (label, ids) in &report.ghost_suspects {
                println!(
                    "ghost suspect: “{label}” answers under {} keys ({}) — reinstalls? `mesh abandon` the dead ones",
                    ids.len(),
                    ids.join(", ")
                );
            }
            if report.candidates_pending > 0 {
                println!(
                    "candidates ripening (not folded, transient): {}",
                    report.candidates_pending
                );
            }
            if report.divergent == 0 {
                println!(
                    "✓ every answer agrees — safe to set read_records true in mesh/config.json"
                );
                ExitCode::SUCCESS
            } else {
                eprintln!(
                    "✗ {} divergent — do NOT flip read_records; fix and re-run",
                    report.divergent
                );
                ExitCode::FAILURE
            }
        }
        // ---- minting warrants (ADR-0026 §6): any warranted member is a door ------------------
        Some("warrant") => {
            // `mesh warrant <node_id> <pubkey_hex>` — the group key's deliberate act turning a
            // member node into a door. Run on a secret-holding node; find the target's id and
            // pubkey in its mesh/node.json. Prints the warrant JSON; install it on the target
            // with `mesh warrant-install '<json>'`.
            let (Some(node_id), Some(pubkey)) = (args.get(1), args.get(2)) else {
                eprintln!("mesh: usage: familiar mesh warrant <node_id> <pubkey_hex>  (both from the target's mesh/node.json)");
                return ExitCode::FAILURE;
            };
            let Ok(Some(cred)) = familiar_mesh::group::load(&dir) else {
                eprintln!("mesh: no group enrolled");
                return ExitCode::FAILURE;
            };
            match familiar_mesh::group::issue_warrant(
                &cred,
                node_id,
                pubkey,
                now_secs(),
                familiar_mesh::group::DEFAULT_WARRANT_TTL_SECS,
            ) {
                Ok(w) => {
                    eprintln!(
                        "⚠ this empowers {node_id} to ADMIT MEMBERS for {} days — hand it over a trusted channel",
                        familiar_mesh::group::DEFAULT_WARRANT_TTL_SECS / 86_400
                    );
                    println!("{}", serde_json::to_string(&w).unwrap_or_default());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("warrant-install") => {
            // `mesh warrant-install '<json>'` — install a warrant issued to THIS node. Verified
            // against the group key and this node's identity before it is written.
            let Some(json) = args.get(1) else {
                eprintln!("mesh: usage: familiar mesh warrant-install '<warrant json>'");
                return ExitCode::FAILURE;
            };
            let w: familiar_mesh::group::MintWarrant = match serde_json::from_str(json) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("mesh: bad warrant json — {e}");
                    return ExitCode::FAILURE;
                }
            };
            match familiar_mesh::group::install_warrant(&dir, &w, now_secs()) {
                Ok(()) => {
                    println!(
                        "✓ warrant installed — this node can now admit knocks (expires {})",
                        w.expiry
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        // ---- group-secret escrow (ADR-0018) --------------------------------------------------
        // The lighthouse is the only minting door, which makes the group secret a single point of
        // extinction unless it is escrowed. These three commands are the whole procedure; the human
        // side of it is security/group-secret-escrow.md.
        Some("escrow-export") => match familiar_mesh::group::export_escrow(&dir, now_secs()) {
            Ok(escrow) => match serde_json::to_string_pretty(&escrow) {
                Ok(json) => {
                    // Straight to stdout so it can be piped into an encryptor and never touch disk
                    // in the clear. The warning goes to stderr so it does not corrupt the pipe.
                    eprintln!(
                        "⚠ this is the group's MINTING AUTHORITY, not a backup. Anyone holding it \
                         can admit anyone. Encrypt it and keep it offline."
                    );
                    println!("{json}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: could not serialize escrow — {e}");
                    ExitCode::FAILURE
                }
            },
            Err(e) => {
                eprintln!("mesh: {e}");
                ExitCode::FAILURE
            }
        },
        Some("escrow-restore") => {
            use std::io::Read;
            let mut buf = String::new();
            if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
                eprintln!("mesh: could not read escrow from stdin — {e}");
                return ExitCode::FAILURE;
            }
            let escrow: familiar_mesh::group::GroupEscrow = match serde_json::from_str(&buf) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("mesh: that is not an escrow document — {e}");
                    return ExitCode::FAILURE;
                }
            };
            match familiar_mesh::group::restore_from_escrow(&dir, &escrow) {
                Ok(cred) => {
                    println!(
                        "✓ minting authority restored for “{}” · id {}",
                        cred.label,
                        short_id(&cred.group_id)
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: restore refused — {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("reduce-to-covenant") => {
            // Irreversible without the escrow, so it refuses unless the human says the word. The
            // confirmation is not ceremony: until an escrow exists, a second node holding the
            // secret IS the group's redundancy, and stripping it makes things worse.
            if !f.contains_key("yes") {
                eprintln!(
                    "mesh: this strips the group secret from this node. It CANNOT be undone \
                     without an escrow (security/group-secret-escrow.md).\n\
                     Export and verify an escrow first, then re-run with --yes."
                );
                return ExitCode::FAILURE;
            }
            match familiar_mesh::group::reduce_to_covenant(&dir) {
                Ok(cred) => {
                    println!(
                        "✓ this node now holds a covenant credential for “{}” — it can prove \
                         membership and verify peers, and can no longer mint members.",
                        cred.label
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("mesh: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!(
                "mesh: usage: familiar mesh <create-group [--label L] | join --key K [--label L] \
                 | request-join --host H | key | qr | peer <ip[:port]> \
                 | abandon <node_id> | forget <node_id> \
                 | share <tools|knowledge|identities> <on|off> | accept-observations <on|off> \
                 | auto-accept <on|off> | pending | approve <node_id> | deny <node_id> \
                 | invite [--minutes N] | optin <handle> | status \
                 | escrow-export | escrow-restore | reduce-to-covenant --yes \
                 | standing [show | grant <node_id> [--note N] | revoke <node_id>] \
                 | sever|hold|disestablish|restore <node_id> [--reason R] \
                 | name <node_id> <handle> | invite-token [--handle H] | warrant <node_id> <pubkey> | warrant-install <json> \
                 | migrate-records | doctor>"
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

/// Civil (year, month, day) from days since the unix epoch — Howard Hinnant's algorithm,
/// so the roster prints dates without pulling in a chrono dependency.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
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

/// The placeholder used when no reachable address can be detected — a signal (not a real host)
/// so the caller can nudge the human to pass `--host`.
const HOST_PLACEHOLDER: &str = "<this-familiar>";

fn host_is_placeholder(host: &str) -> bool {
    host == HOST_PLACEHOLDER
}

/// Every address a device could reach this familiar at, most-universal first — see
/// `transport::reachable_hosts` (tailnet, then LAN).
fn reachable_hosts() -> Vec<String> {
    familiar_mesh::transport::reachable_hosts()
}

/// Render `payload` as a scannable terminal QR via `qrencode` if it's installed. Returns whether
/// a QR was drawn — dependency-free (optional external tool), matching how the mesh shells out to
/// `tailscale` rather than pulling in a crate.
fn render_qr(payload: &str) -> bool {
    use std::io::Write;
    let Ok(mut child) = std::process::Command::new("qrencode")
        .args(["-t", "ANSIUTF8", "-m", "1"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::inherit())
        .spawn()
    else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(payload.as_bytes());
    }
    child.wait().map(|s| s.success()).unwrap_or(false)
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

/// The human's hand on a declared surface — same wrapper tools, same review, same gates
/// as the familiar's own loop (cmd_discover pattern: boundary check up front).
fn cmd_actuate(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    let mut positional: Vec<&String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i].starts_with("--") {
            if !args[i].contains('=') && args.get(i + 1).is_some_and(|v| !v.starts_with("--")) {
                i += 1;
            }
        } else {
            positional.push(&args[i]);
        }
        i += 1;
    }
    let [surface, label] = positional.as_slice() else {
        eprintln!("usage: familiar actuate <surface> <state|label>  (surfaces: actuators.json)");
        return ExitCode::FAILURE;
    };
    let b = boundary::load(&dir).unwrap_or_else(|_| boundary::Boundary::closed());
    let action = guard::Action::new(guard::ActionKind::Actuate, surface.as_str());
    let v = guard::evaluate(&action, &b);
    if v.decision != Decision::Allow {
        eprintln!("{}", v.rationale);
        return ExitCode::FAILURE;
    }
    match familiar_cycle::actuate_by_hand(&dir, surface, label, now_secs()) {
        Ok(Ok(out)) => {
            println!("{out}");
            ExitCode::SUCCESS
        }
        Ok(Err(msg)) => {
            eprintln!("actuate: {msg}");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("actuate: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The subject-facing view (ADR-0022 constraint 3): a record kept about someone that
/// they cannot see is surveillance, whatever its purpose. Deliberately CLI-only — the
/// worldview federates to every member device, and a person's shape is theirs to read,
/// not the room's to browse.
fn cmd_dossier(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    let now = now_secs();
    // Positionals: skip flags AND their values (mirrors how `flags()` consumes them).
    let mut positional: Vec<&String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i].starts_with("--") {
            if !args[i].contains('=') && args.get(i + 1).is_some_and(|v| !v.starts_with("--")) {
                i += 1; // the flag's value
            }
        } else {
            positional.push(&args[i]);
        }
        i += 1;
    }
    let half_life = familiar_kernel::parameters::Parameters::load_or_default(&dir)
        .sane()
        .dossier_half_life_days
        * 86_400;
    match positional.as_slice() {
        [w, handle] if w.as_str() == "withdraw" => {
            match familiar_kernel::dossier::withdraw(&dir, handle, now) {
                Ok(r) => {
                    println!(
                        "{} contributions removed; face link cleared: {}.",
                        r.contributions_removed, r.face_unlinked
                    );
                    println!("Your weight no longer feeds any pattern, and no later fold will rebuild one about you.");
                    println!("The honest boundary: aggregate structure that no longer identifies you survives; nothing that points at you does.");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("dossier withdraw: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        [handle] => {
            let d = match familiar_kernel::dossier::read(&dir, handle, now, half_life) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("dossier: {e}");
                    return ExitCode::FAILURE;
                }
            };
            match &d.identity {
                Some(i) => println!(
                    "{} ({}) — {} · first seen {} · {} interactions",
                    i.name, i.handle, i.relation, i.first_seen, i.interactions
                ),
                None => println!("({handle} — no identity on record)"),
            }
            if d.withdrawn {
                println!("WITHDRAWN — this person removed themselves; no pattern is kept and none will be rebuilt.");
                return ExitCode::SUCCESS;
            }
            println!("\npresence by hour (UTC) — share of decayed weight, · none:");
            let max = d
                .presence_hours
                .iter()
                .map(|s| s.share)
                .fold(0.0_f64, f64::max);
            for s in &d.presence_hours {
                let bar_len = if max > 0.0 {
                    ((s.share / max) * 24.0).round() as usize
                } else {
                    0
                };
                println!(
                    "  {} {:24} {}",
                    s.slot,
                    "#".repeat(bar_len),
                    if s.count > 0 {
                        format!(
                            "share {:.2} · confidence {:.2} · {} sightings",
                            s.share, s.confidence, s.count
                        )
                    } else {
                        "·".to_string()
                    }
                );
            }
            if !d.standing.is_empty() {
                println!("\nusually identified by:");
                for s in &d.standing {
                    println!(
                        "  {:9} share {:.2} · confidence {:.2} · {} sightings",
                        s.slot, s.share, s.confidence, s.count
                    );
                }
            }
            if d.needs.is_empty() {
                println!("\nneeds: none on record");
            } else {
                println!("\nneeds:");
                for n in &d.needs {
                    println!(
                        "  [{}] {} — {}",
                        if n.stated { "stated" } else { "theorized" },
                        n.status,
                        n.text
                    );
                }
            }
            println!(
                "\nin a sentence: {}",
                familiar_kernel::dossier::coarse_summary(&d)
            );
            println!(
                "this record is yours: `familiar dossier withdraw {handle}` removes you from it."
            );
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: familiar dossier <handle> | familiar dossier withdraw <handle>");
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

/// `reach` — assess what the familiar could extend into (default), or `reach install <ip>` to
/// extend into an agent-capable host: install/enroll an agent that joins by covenant.
fn cmd_reach(args: &[String]) -> ExitCode {
    if let Some("install") = args.first().map(String::as_str) {
        return cmd_reach_install(&args[1..]);
    }
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    let now = now_secs();
    let timeout: u64 = f
        .get("timeout-ms")
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);

    let b = match boundary::load(&dir) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("reach: boundary policy error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let verdict = guard::evaluate(&Action::new(ActionKind::Network, "reach-scan"), &b);
    if verdict.decision != Decision::Allow {
        eprintln!(
            "reach: the network is outside the boundary — open `allow_network` to let the familiar \
             assess what it could extend into.\n  {}",
            verdict.rationale
        );
        return ExitCode::FAILURE;
    }

    println!("assessing reach (probing discovered devices)…");
    let (reaches, observations) = familiar_reach::scan(&dir, now, b.allow_network, timeout);
    if reaches.is_empty() {
        println!(
            "reach: no devices discovered — are you on the LAN? (A `devices.json` pointed at your \
             router names more of them.)"
        );
        return ExitCode::SUCCESS;
    }

    use familiar_reach::ReachClass;
    for class in [
        ReachClass::AgentCapable,
        ReachClass::ProtocolControllable,
        ReachClass::ObservableOnly,
    ] {
        let group: Vec<_> = reaches.iter().filter(|r| r.class == class).collect();
        if group.is_empty() {
            continue;
        }
        println!("\n{} ({}):", class.label(), group.len());
        for r in group {
            let svc = if r.open.is_empty() {
                "—".to_string()
            } else {
                r.open.join(", ")
            };
            println!("  · {:<22} {:<15} {}", r.label, r.ip, svc);
        }
    }

    let mut recorded = 0;
    for o in observations {
        if observation::record(&dir, o).is_ok() {
            recorded += 1;
        }
    }
    println!(
        "\nrecorded {recorded} reach observation(s). Agent-capable devices are the candidates for \
         a consent-gated agent install (Brick 3)."
    );
    ExitCode::SUCCESS
}

/// `reach install <ip> --user U --familiar-host H --authorize` — the consent-gated act of
/// extending into an agent-capable host: over SSH (the human's OWN access — never an exploit), have
/// the target's familiar agent request to join this familiar by covenant. This familiar opens a
/// brief invite window so the authorized device is admitted without a per-node tap; the target
/// holds only its own cert. Law III: nothing happens without `--authorize` and an open boundary.
fn cmd_reach_install(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    let Some(ip) = args.first().filter(|a| !a.starts_with("--")) else {
        eprintln!(
            "reach: usage: familiar mesh reach install <ip> --user U --familiar-host H --authorize"
        );
        return ExitCode::FAILURE;
    };
    if !f.contains_key("authorize") {
        eprintln!(
            "reach install: this extends the familiar into {ip} — installing/enrolling an agent \
             there over your SSH access. Re-run with --authorize to consent (Law III)."
        );
        return ExitCode::FAILURE;
    }
    // Outward reach — gated like any network act.
    match boundary::load(&dir) {
        Ok(b) => {
            let v = guard::evaluate(&Action::new(ActionKind::Network, "reach-install"), &b);
            if v.decision != Decision::Allow {
                eprintln!(
                    "reach install: network is outside the boundary — open `allow_network`.\n  {}",
                    v.rationale
                );
                return ExitCode::FAILURE;
            }
        }
        Err(e) => {
            eprintln!("reach install: boundary policy error: {e}");
            return ExitCode::FAILURE;
        }
    }

    let user = f
        .get("user")
        .cloned()
        .unwrap_or_else(|| "familiar".to_string());
    let ssh_port = f
        .get("ssh-port")
        .cloned()
        .unwrap_or_else(|| "22".to_string());
    let fam_port = f
        .get("familiar-port")
        .cloned()
        .unwrap_or_else(|| "47100".to_string());
    let Some(fam_host) = f.get("familiar-host") else {
        eprintln!("reach install: --familiar-host <addr> is required (how the target reaches THIS familiar)");
        return ExitCode::FAILURE;
    };
    let remote_bin = f
        .get("remote-bin")
        .cloned()
        .unwrap_or_else(|| "familiar".to_string());
    let remote_data = f
        .get("remote-data")
        .cloned()
        .unwrap_or_else(|| "familiar_data".to_string());

    // 1. Consent recorded here IS the authorization to admit this device — open a brief invite
    //    window on THIS familiar so the target's covenant request is auto-accepted.
    if let Err(e) = familiar_mesh::enroll::open_invite(&dir, now_secs() + 180) {
        eprintln!("reach install: could not open the invite window — {e}");
        return ExitCode::FAILURE;
    }
    println!("· opened a 3-min invite window — the authorized device will be admitted");

    // 2. Over SSH (the human's access), have the target's agent request to join by covenant.
    let remote_cmd = format!(
        "{remote_bin} mesh request-join --host {fam_host} --port {fam_port} --data-dir {remote_data}"
    );
    println!("· {user}@{ip}: {remote_cmd}");
    let status = std::process::Command::new("ssh")
        .args([
            "-o",
            "ConnectTimeout=8",
            "-o",
            "BatchMode=yes",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-p",
            &ssh_port,
            &format!("{user}@{ip}"),
            &remote_cmd,
        ])
        .status();
    match status {
        Ok(s) if s.success() => {
            // Record the expansion as an observation (auditability).
            let _ = observation::record(
                &dir,
                familiar_kernel::observation::Observation::new(
                    "familiar",
                    "extended-into",
                    format!("device:{ip}"),
                    format!("covenant agent via {user}@{ip}"),
                    "reach",
                    now_secs(),
                    0.95,
                ),
            );
            println!("✓ {ip} joined by covenant — a new agent in the mesh (revoke by node id if ever needed)");
            ExitCode::SUCCESS
        }
        Ok(s) => {
            eprintln!("reach install: the remote request-join failed (exit {:?}). Is `{remote_bin}` on the target and can it reach {fam_host}:{fam_port}?", s.code());
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("reach install: could not ssh to {user}@{ip} — {e}");
            ExitCode::FAILURE
        }
    }
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

/// How often (in ticks) the daemon sweeps the LAN for reachable devices — the frontier the mesh map
/// draws as faded branches. Network probing is heavier than a tick, so it runs sparsely.
/// `familiar discover` — the periphery's one-shot network survey: discover the devices sharing this
/// LAN (ARP + optional DHCP leases) and assess their reach (bounded port probe), recording the
/// `device:*` and `can-reach device:*` observations that populate the roster and the map's frontier.
///
/// This is the seam that replaces the core's old autonomous sweep: the shell (a launchd timer, the
/// GUI app, a native survey) decides *when* to look, invokes this, and the findings flow back in as
/// observations — so discovery is a peripheral, consent-gated act, not a metabolic reflex flooding
/// the theory pipeline. Gated on `allow_network`: nothing reaches outward without the human's gate.
fn cmd_discover(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    let now = now_secs();
    let timeout: u64 = f
        .get("timeout-ms")
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    let b = match boundary::load(&dir) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("discover: boundary policy error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let verdict = guard::evaluate(&Action::new(ActionKind::Network, "discover"), &b);
    if verdict.decision != Decision::Allow {
        eprintln!(
            "discover: the network is outside the boundary — open `allow_network` to let the \
             periphery survey the LAN.\n  {}",
            verdict.rationale
        );
        return ExitCode::FAILURE;
    }
    let mut recorded = 0;
    // Device survey (who is present) …
    for o in familiar_sense::devices(&dir, now, b.allow_network) {
        if observation::record(&dir, o).is_ok() {
            recorded += 1;
        }
    }
    // … then reach assessment (what we could do with them) — seeds the frontier.
    let (reaches, obs) = familiar_reach::scan(&dir, now, b.allow_network, timeout);
    for o in obs {
        if observation::record(&dir, o).is_ok() {
            recorded += 1;
        }
    }
    println!(
        "discover: {} device(s) assessed, {recorded} observation(s) recorded.",
        reaches.len()
    );
    ExitCode::SUCCESS
}

/// `familiar tool prune [--dry-run]` — purge every authored tool whose script reaches the network
/// (LAN scans/probes that should never have been core-authored or federated). `--dry-run` lists
/// what would be removed without touching anything. The purge deletes each tool's script file and
/// rewrites the store with the survivors.
fn cmd_tool(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    match args.first().map(String::as_str) {
        Some("prune") => {
            if f.contains_key("dry-run") {
                let tools = familiar_kernel::tool::load(&dir).unwrap_or_default();
                let mut n = 0;
                for t in &tools {
                    let reaches = std::fs::read_to_string(&t.script_path)
                        .map(|s| familiar_kernel::review::reaches_network(&s))
                        .unwrap_or(false);
                    if reaches {
                        println!("  would remove {} ({})", t.id, t.name);
                        n += 1;
                    }
                }
                println!("tool prune --dry-run: {n} network-reaching tool(s) would be removed.");
                return ExitCode::SUCCESS;
            }
            match familiar_kernel::tool::prune_network(&dir) {
                Ok(removed) => {
                    for (id, name) in &removed {
                        println!("  removed {id} ({name})");
                    }
                    println!(
                        "tool prune: {} network-reaching tool(s) removed.",
                        removed.len()
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("tool prune: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("usage: familiar tool prune [--dry-run]");
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
            // The LAN reach-sweep that seeds the frontier is no longer driven from the core's own
            // metabolism — it's a peripheral capability now, invoked on the shell's cadence via
            // `familiar discover` (macOS launchd timer / GUI app) or a native survey POSTing to the
            // observe seam. The core no longer goes out and scans the network on every Nth tick.
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
        Ok(familiar_llm::Outcome::RateLimited(why)) => {
            println!("RATE-LIMITED: {why}");
            println!("  try again later — every configured provider is cooling down");
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
