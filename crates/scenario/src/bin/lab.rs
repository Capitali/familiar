//! `familiar-lab` — run scenario fixtures under the experimental controls.
//!
//!   familiar-lab run <fixture.json> [--control A|B|C|D] [--episodes N]
//!                    [--lab DIR] [--llm-adapter PATH]
//!   familiar-lab matrix <fixture.json> [--episodes N] [--lab DIR] [--llm-adapter PATH]
//!   familiar-lab list [DIR]
//!
//! `run` executes one control; `matrix` executes all four (A B C D) so the
//! ADR-0010 comparison — does accumulated experience beat starting from
//! scratch? — reads off a single table. Without an `--llm-adapter`, controls
//! B/D fall back to the deterministic template and the report says so.

use familiar_scenario::harness::{self, Control, RunConfig};
use familiar_scenario::scenario;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => run(&args[1..], false),
        Some("matrix") => run(&args[1..], true),
        Some("list") => list(args.get(1).map(String::as_str)),
        _ => {
            eprintln!(
                "usage: familiar-lab run <fixture.json> [--control A|B|C|D] [--episodes N] \
                 [--lab DIR] [--llm-adapter PATH]\n       \
                 familiar-lab matrix <fixture.json> [--episodes N] [--lab DIR] [--llm-adapter PATH]\n       \
                 familiar-lab list [DIR]"
            );
            ExitCode::from(2)
        }
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
    let cfg = RunConfig {
        lab_dir: PathBuf::from(flag(args, "--lab").unwrap_or_else(|| "lab-runs".to_string())),
        episodes,
        llm_adapter: flag(args, "--llm-adapter").map(PathBuf::from),
    };
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
    let mut stack = vec![root.clone()];
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
    for p in fixtures {
        match scenario::load(&p) {
            Ok(s) => {
                found += 1;
                println!(
                    "{:<44} {:<24} {} — {}",
                    p.display(),
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
