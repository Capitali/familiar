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
  reach          assess what the familiar could extend into — discover devices and
                 classify each (agent-capable / protocol-controllable / observable)
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
                 | `mesh accept-observations <on|off>` (device agents) | `mesh qr` (enroll a device)
                 | `mesh pending`/`approve <id>`/`deny <id>` (covenant handshake) | `mesh invite`
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
        Some("reach") => cmd_reach(rest),
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
                    println!("✓ admitted to “{}” by covenant — enrolled (no secret held)", g.group_label);
                    ExitCode::SUCCESS
                }
                Ok(familiar_mesh::enroll::JoinOutcome::Pending) => {
                    println!("… request pending — waiting for the familiar to accept (up to 5 min)");
                    let mut waited = 0;
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(3));
                        waited += 3;
                        match familiar_mesh::enroll::poll_join(&dir, host, port, &node.node_id()) {
                            Ok(Some(g)) => {
                                open_mesh_gate(&dir);
                                println!("✓ admitted to “{}” by covenant — enrolled", g.group_label);
                                return ExitCode::SUCCESS;
                            }
                            Ok(None) if waited < 300 => continue,
                            Ok(None) => {
                                eprintln!("mesh: no decision after 5 min — run again to keep waiting");
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
                .unwrap_or_else(|| familiar_mesh::config::load(&dir).map(|c| c.gossip_port).unwrap_or(47_100));
            let host = f.get("host").cloned().unwrap_or_else(tailnet_ip_or_hint);
            // Compact JSON — the phone parses this after scanning or pasting.
            let payload = serde_json::json!({
                "v": 1,
                "secret": cred.join_key(),
                "group": cred.group_id,
                "label": cred.label,
                "host": host,
                "port": port,
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
                        println!("    attests (v{}): {}", p.attestation.laws_version, p.attestation.statement);
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
            match familiar_mesh::enroll::deny(&dir, node_id) {
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
                 | request-join --host H | key | qr | peer <ip[:port]> \
                 | share <tools|knowledge|identities> <on|off> | accept-observations <on|off> \
                 | auto-accept <on|off> | pending | approve <node_id> | deny <node_id> \
                 | invite [--minutes N] | optin <handle> | status>"
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

/// The placeholder used when no reachable address can be detected — a signal (not a real host)
/// so the caller can nudge the human to pass `--host`.
const HOST_PLACEHOLDER: &str = "<this-familiar>";

fn host_is_placeholder(host: &str) -> bool {
    host == HOST_PLACEHOLDER
}

/// This familiar's tailnet IPv4 (via `tailscale ip -4`), so a device can reach it off-LAN. Falls
/// back to a placeholder the caller flags — the mesh already shells out to tailscale for peers.
fn tailnet_ip_or_hint() -> String {
    std::process::Command::new("tailscale")
        .args(["ip", "-4"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
        })
        .unwrap_or_else(|| HOST_PLACEHOLDER.to_string())
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
    let timeout: u64 = f.get("timeout-ms").and_then(|s| s.parse().ok()).unwrap_or(300);

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
            let svc = if r.open.is_empty() { "—".to_string() } else { r.open.join(", ") };
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
        eprintln!("reach: usage: familiar mesh reach install <ip> --user U --familiar-host H --authorize");
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
                eprintln!("reach install: network is outside the boundary — open `allow_network`.\n  {}", v.rationale);
                return ExitCode::FAILURE;
            }
        }
        Err(e) => {
            eprintln!("reach install: boundary policy error: {e}");
            return ExitCode::FAILURE;
        }
    }

    let user = f.get("user").cloned().unwrap_or_else(|| "familiar".to_string());
    let ssh_port = f.get("ssh-port").cloned().unwrap_or_else(|| "22".to_string());
    let fam_port = f.get("familiar-port").cloned().unwrap_or_else(|| "47100".to_string());
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
            "-o", "ConnectTimeout=8",
            "-o", "BatchMode=yes",
            "-o", "StrictHostKeyChecking=accept-new",
            "-p", &ssh_port,
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
const REACH_EVERY: usize = 15;

/// Sweep the LAN for reachable devices and record the `can-reach` frontier observations, but only if
/// the network gate is open. Returns how many devices were assessed (0 if the gate is shut or nothing
/// answered). Short per-port timeout so it doesn't stall the tick loop.
fn reach_sweep(dir: &std::path::Path) -> usize {
    let Ok(b) = boundary::load(dir) else { return 0 };
    if !b.allow_network {
        return 0;
    }
    let (reaches, obs) = familiar_reach::scan(dir, now_secs(), true, 250);
    for o in obs {
        let _ = observation::record(dir, o);
    }
    reaches.len()
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
            // Every REACH_EVERY ticks (and on the first), if the network gate is open, sweep the
            // LAN for reachable devices. These `can-reach` observations are the mesh's *frontier* —
            // interfaces the familiar can see but hasn't enrolled — drawn as faded branches on the map.
            if n == 1 || n % REACH_EVERY == 0 {
                let seeded = reach_sweep(&dir);
                if seeded > 0 {
                    println!("  reach: swept the frontier, {seeded} device(s) assessed");
                }
            }
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
