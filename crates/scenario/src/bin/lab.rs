//! `familiar-lab` — run scenario fixtures under the experimental controls.
//!
//!   familiar-lab run <fixture.json> [--control A|B|C|D] [--episodes N]
//!                    [--replicate N] [--lab DIR] [--llm-adapter PATH]
//!                    [--llm-required] [--llm-patience S] [--llm-backoff S]
//!                    [--adapter-timeout S]
//!   familiar-lab matrix <fixture.json> [same flags, all four controls]
//!   familiar-lab validate <fixture.json|DIR>
//!   familiar-lab list [DIR]
//!
//! `run` executes one control; `matrix` executes all four (A B C D) so the
//! ADR-0010 comparison — does accumulated experience beat starting from
//! scratch? — reads off a single table. Without an `--llm-adapter`, controls
//! B/D fall back to the deterministic template and the report says so.

use familiar_scenario::harness::{self, Control, RunConfig};
use familiar_scenario::{campaign, evidence, scenario, validate};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => run(&args[1..], false),
        Some("matrix") => run(&args[1..], true),
        Some("campaign") => campaign_cmd(&args[1..]),
        Some("report") => report_cmd(&args[1..]),
        Some("validate") => validate_cmd(args.get(1).map(String::as_str)),
        Some("list") => list(args.get(1).map(String::as_str)),
        _ => {
            eprintln!(
                "usage: familiar-lab run <fixture.json> [--control A|B|C|D] [--episodes N] \
                 [--replicate N] [--lab DIR] [--llm-adapter PATH] [--llm-required] \
                 [--llm-patience S] [--llm-backoff S] [--adapter-timeout S]\n       \
                 familiar-lab matrix <fixture.json> [same flags]\n       \
                 familiar-lab campaign <plan.json> [--resume] [--force]\n       \
                 familiar-lab report <dir> [--md PATH] [--json PATH]\n       \
                 familiar-lab validate <fixture.json|DIR>\n       \
                 familiar-lab list [DIR]"
            );
            ExitCode::from(2)
        }
    }
}

fn campaign_cmd(args: &[String]) -> ExitCode {
    let Some(plan_path) = args.first().filter(|a| !a.starts_with("--")) else {
        eprintln!("familiar-lab: a campaign plan (JSON) is required");
        return ExitCode::from(2);
    };
    let plan = match campaign::load_plan(Path::new(plan_path)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("familiar-lab: {e}");
            return ExitCode::FAILURE;
        }
    };
    let resume = args.iter().any(|a| a == "--resume");
    let force = args.iter().any(|a| a == "--force");
    match campaign::run(&plan, resume, force) {
        Ok(outcome) => {
            println!(
                "campaign {:?}: {} cells run, {} skipped, {} failed (state in {})",
                outcome.halt,
                outcome.cells_run,
                outcome.cells_skipped,
                outcome.cells_failed,
                plan.out.display()
            );
            match outcome.halt {
                campaign::Halt::Complete => ExitCode::SUCCESS,
                // Interrupted-but-resumable is not a failure, but it is not done.
                _ => ExitCode::from(3),
            }
        }
        Err(e) => {
            eprintln!("familiar-lab: campaign: {e}");
            ExitCode::FAILURE
        }
    }
}

fn report_cmd(args: &[String]) -> ExitCode {
    let Some(dir) = args.first().filter(|a| !a.starts_with("--")) else {
        eprintln!("familiar-lab: a directory of reports is required");
        return ExitCode::from(2);
    };
    match evidence::Evidence::collect(Path::new(dir)) {
        Ok(ev) if ev.scenarios.is_empty() => {
            eprintln!("no report.json files under {dir}");
            ExitCode::FAILURE
        }
        Ok(ev) => {
            print!("{}", ev.table());
            if let Some(md) = flag(args, "--md") {
                if let Err(e) = std::fs::write(&md, ev.markdown()) {
                    eprintln!("familiar-lab: writing {md}: {e}");
                    return ExitCode::FAILURE;
                }
                println!("markdown evidence written to {md}");
            }
            if let Some(json) = flag(args, "--json") {
                if let Err(e) = ev.save(Path::new(&json)) {
                    eprintln!("familiar-lab: writing {json}: {e}");
                    return ExitCode::FAILURE;
                }
                println!("json evidence written to {json}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("familiar-lab: report: {e}");
            ExitCode::FAILURE
        }
    }
}

/// All fixture JSON files under `root` (or `root` itself if it is a file).
fn fixtures_under(root: &Path) -> Vec<PathBuf> {
    if root.is_file() {
        return vec![root.to_path_buf()];
    }
    let mut stack = vec![root.to_path_buf()];
    let mut fixtures = Vec::new();
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "json") {
                fixtures.push(p);
            }
        }
    }
    fixtures.sort();
    fixtures
}

fn validate_cmd(target: Option<&str>) -> ExitCode {
    let root = PathBuf::from(target.unwrap_or("scenarios"));
    let fixtures = fixtures_under(&root);
    if fixtures.is_empty() {
        eprintln!("no fixtures under {}", root.display());
        return ExitCode::FAILURE;
    }
    let mut errors = 0;
    for p in fixtures {
        match validate::check_file(&p) {
            Ok((_, violations)) => {
                let verdict = if validate::has_errors(&violations) {
                    errors += 1;
                    "INVALID"
                } else if violations.is_empty() {
                    "ok"
                } else {
                    "ok (warnings)"
                };
                println!("{:<44} {}", p.display(), verdict);
                for v in &violations {
                    println!("    {v}");
                }
            }
            Err(e) => {
                errors += 1;
                println!("{:<44} INVALID — {e}", p.display());
            }
        }
    }
    if errors > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn run(args: &[String], matrix: bool) -> ExitCode {
    let Some(fixture) = args.first().filter(|a| !a.starts_with("--")) else {
        eprintln!("familiar-lab: a fixture path is required");
        return ExitCode::from(2);
    };
    let scn = match scenario::load(Path::new(fixture)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("familiar-lab: {e}");
            return ExitCode::FAILURE;
        }
    };
    let episodes = flag(args, "--episodes")
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let mut cfg = RunConfig {
        lab_dir: PathBuf::from(flag(args, "--lab").unwrap_or_else(|| "lab-runs".to_string())),
        episodes,
        llm_adapter: flag(args, "--llm-adapter").map(PathBuf::from),
        replicate: flag(args, "--replicate")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1),
        llm_required: args.iter().any(|a| a == "--llm-required"),
        ..RunConfig::default()
    };
    if let Some(s) = flag(args, "--llm-patience").and_then(|v| v.parse().ok()) {
        cfg.llm_patience_secs = s;
    }
    if let Some(s) = flag(args, "--llm-backoff").and_then(|v| v.parse().ok()) {
        cfg.llm_retry_backoff_secs = s;
    }
    if let Some(s) = flag(args, "--adapter-timeout").and_then(|v| v.parse().ok()) {
        cfg.adapter_timeout_secs = s;
    }
    let controls: Vec<Control> = if matrix {
        vec![
            Control::Baseline,
            Control::LlmOnly,
            Control::NoMemory,
            Control::Full,
        ]
    } else {
        let spec = flag(args, "--control").unwrap_or_else(|| "D".to_string());
        match Control::parse(&spec) {
            Some(c) => vec![c],
            None => {
                eprintln!("familiar-lab: unknown control {spec:?} (use A, B, C, or D)");
                return ExitCode::from(2);
            }
        }
    };

    if cfg.llm_adapter.is_none() && controls.iter().any(|c| *c != Control::Baseline) {
        eprintln!(
            "note: no --llm-adapter — LLM controls will fall back to the deterministic \
             template (reported honestly as llm_used=false)"
        );
    }

    println!(
        "scenario {} · family {} · goal: {}",
        scn.id, scn.family, scn.visible_goal
    );
    let mut failed = false;
    for control in controls {
        match harness::run(&scn, control, &cfg) {
            Ok(report) => {
                println!("\n{}", report.summary_line());
                print!("{}", report.table());
            }
            Err(e) => {
                eprintln!("familiar-lab: control {} failed: {e}", control.letter());
                failed = true;
            }
        }
    }
    println!("\nreports under {}", cfg.lab_dir.display());
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn list(dir: Option<&str>) -> ExitCode {
    let root = PathBuf::from(dir.unwrap_or("scenarios"));
    let mut found = 0;
    for p in fixtures_under(&root) {
        match scenario::load(&p) {
            Ok(s) => {
                found += 1;
                let validity = match validate::check(&s) {
                    Ok(v) if validate::has_errors(&v) => "INVALID",
                    Ok(v) if !v.is_empty() => "warn",
                    Ok(_) => "ok",
                    Err(_) => "unchecked",
                };
                println!(
                    "{:<44} {:<8} {:<24} {} — {}",
                    p.display(),
                    validity,
                    s.family,
                    s.id,
                    s.visible_goal
                );
            }
            Err(e) => eprintln!("{}: INVALID — {e}", p.display()),
        }
    }
    if found == 0 {
        eprintln!("no fixtures under {}", root.display());
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
