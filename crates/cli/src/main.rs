//! The Substrate CLI shell — a thin wrapper over the kernel.
//!
//! Argument parsing is hand-rolled and dependency-free on purpose: a small,
//! legible trust surface is part of the Law III commitment.

use std::collections::HashMap;
use std::fs;
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use substrate_kernel::boundary;
use substrate_kernel::guard::{self, Action, ActionKind, Decision};
use substrate_kernel::observation::{self, Observation};
use substrate_kernel::presence;
use substrate_kernel::service;
use substrate_kernel::store;

const USAGE: &str = "\
substrate — telos-first factory (genesis)

usage:
  substrate <command> [options]

commands:
  observe        record an observation (the only truth)
  observations   list recorded observations
  service        report the service signal (Law I)
  presence       report the presence signal (Law II)
  sense          perceive the host (environment, interfaces, capabilities)
  tick           run one cycle of the metabolism (sense → detect → generate → measure)
  run            run the metabolism: --ticks N (bounded) or --daemon/--ticks 0
                 (unbounded, every --interval S, default 60; Ctrl-C to stop)
  boundary       show the current capability boundary (the human's lever)
  guard          weigh a proposed action against the boundary (Law III)
  consult        consult the LLM (refused unless a human has opened the boundary)

options:
  --data-dir <dir>   data directory (default: substrate_data)

observe options:
  --actor <a> --action <act> --object <o>   (required)
  --context <c> --source <s> --confidence <0..1>   (optional)

guard options:
  --kind <observe|emit_artifact|read_file|write_file|network|llm|install_tool>
  --target <t>   --affects-person   --irreversible

see docs/SOUL.md for the Three Laws this factory is built to serve.";

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
        Some("sense") => cmd_sense(rest),
        Some("tick") => cmd_tick(rest),
        Some("run") => cmd_run(rest),
        Some("boundary") => cmd_boundary(rest),
        Some("guard") => cmd_guard(rest),
        Some("consult") => cmd_consult(rest),
        Some(cmd) => {
            eprintln!("substrate: unknown command '{cmd}'\n\n{USAGE}");
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

fn cmd_sense(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    let now = now_secs();

    // Perception of the local host is always permitted (you can't serve what you
    // can't see). Outward reach — the connectivity probe — is boundary-gated.
    let mut perceived = Vec::new();
    perceived.extend(substrate_sense::census(now));
    perceived.extend(substrate_sense::interfaces(now));
    perceived.extend(substrate_sense::capabilities(
        now,
        substrate_sense::DEFAULT_TOOLS,
    ));

    let mut connectivity_note = "skipped (network outside the boundary)".to_string();
    match boundary::load(&dir) {
        Ok(b) => {
            let verdict =
                guard::evaluate(&Action::new(ActionKind::Network, "connectivity-probe"), &b);
            if verdict.decision == Decision::Allow {
                let o = substrate_sense::connectivity(now);
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
    println!("  (open the Observatory to see the environment the factory discovered)");
    ExitCode::SUCCESS
}

fn print_tick(n: usize, r: &substrate_cycle::TickReport) {
    println!(
        "tick {n}: +{} sensed, {} loops, +{} candidates | service {:.2}, presence {:.2}{}",
        r.sensed,
        r.loops,
        r.new_candidates,
        r.service,
        r.presence,
        if r.presence_withdrawn {
            " (withdrawn)"
        } else {
            ""
        }
    );
}

fn cmd_tick(args: &[String]) -> ExitCode {
    let f = flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    match substrate_cycle::tick_gated(&dir, now_secs()) {
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
            interval = 60; // a sane default cadence so it isn't a busy loop
        }
        println!("metabolism running every {interval}s — Ctrl-C to stop");
        let mut n = 0usize;
        loop {
            n += 1;
            match substrate_cycle::tick_gated(&dir, now_secs()) {
                Ok(r) => print_tick(n, &r),
                Err(e) => {
                    eprintln!("run: {e}");
                    return ExitCode::FAILURE;
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(interval));
        }
    }

    for n in 1..=ticks {
        match substrate_cycle::tick_gated(&dir, now_secs()) {
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
        _ => {
            eprintln!("guard: --kind must be one of observe|emit_artifact|read_file|write_file|network|llm|install_tool");
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

    // Law III: the LLM seam is an outward action, gated by the human-owned boundary.
    let b = match boundary::load(&dir) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("consult: boundary policy error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let verdict = guard::evaluate(&Action::new(ActionKind::Llm, "llm-provider"), &b);
    match verdict.decision {
        Decision::Refuse => {
            println!("REFUSE: {}", verdict.rationale);
            println!(
                "  the LLM seam is closed; a human opens it via boundary.json (docs/boundaries.md)"
            );
            return ExitCode::SUCCESS;
        }
        Decision::SeekConsent => {
            println!("SEEK CONSENT: {}", verdict.rationale);
            return ExitCode::SUCCESS;
        }
        Decision::Allow => {}
    }

    // Allowed by the boundary: shell out to the human-installed adapter.
    let llm_dir = dir.join("llm");
    let script = llm_dir.join("call_llm.sh");
    if !script.exists() {
        eprintln!(
            "consult: {} not found — copy llm/call_llm.sh into your data dir and add key.env (see llm/README.md)",
            script.display()
        );
        return ExitCode::FAILURE;
    }
    if let Err(e) =
        fs::create_dir_all(&llm_dir).and_then(|_| fs::write(llm_dir.join("prompt.txt"), prompt))
    {
        eprintln!("consult: {e}");
        return ExitCode::FAILURE;
    }
    match Command::new("sh").arg(&script).status() {
        Ok(s) if s.success() => match fs::read_to_string(llm_dir.join("response.json")) {
            Ok(r) => {
                println!("{r}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("consult: response unreadable: {e}");
                ExitCode::FAILURE
            }
        },
        Ok(s) => {
            eprintln!("consult: adapter exited with status {s}");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("consult: could not run adapter: {e}");
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
