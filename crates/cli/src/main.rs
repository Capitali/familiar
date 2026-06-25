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
        Some(cmd) => {
            eprintln!("familiar: unknown command '{cmd}'\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
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
        if interval == 0 {
            interval = 60; // a sane floor cadence so it isn't a busy loop
        }
        // Adaptive structural-fingerprint cadence: `--interval` is the *floor* (the
        // busy cadence), `--max-interval` the ceiling reached when the world goes
        // quiet (default floor×16, capped at 1h). `--fixed` opts out for a constant
        // period. The metabolism quickens when the environment or its own work moves
        // and drowses when nothing changes (see `TickReport::quiet`).
        let floor = interval;
        let fixed = f.contains_key("fixed");
        let ceil: u64 = f
            .get("max-interval")
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| floor.saturating_mul(16).min(3600))
            .max(floor);
        // Make this process visible to `daemon status/start/stop` (incl. when launched
        // by launchd), so the two control paths agree and never double-spawn.
        daemon::record_self(&dir);
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

    for n in 1..=ticks {
        match familiar_cycle::tick_gated(&dir, now_secs()) {
            Ok(r) => print_tick(n, &r),
            Err(e) => {
                eprintln!("run: {e}");
                return ExitCode::FAILURE;
            }
        }
        if interval > 0 && n < ticks {
            std::thread::sleep(std::time::Duration::from_secs(interval));
        }
    }
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
