//! `familiar autonomy` — the captain's dial over the ship's computer, per control
//! surface, and the message window's other half: read the advice, answer the
//! proposals (T-237 §3.5, Ian's ruling 2026-09-03).

use std::path::PathBuf;
use std::process::ExitCode;

use familiar_whisker::autonomy::{self, Approval, Level, Surface};
use serde_json::Value;

fn ship_dir_for(dir: &std::path::Path, root: &std::path::Path, id: &str) -> Option<PathBuf> {
    let d = root.join(id);
    if d.is_dir() {
        return Some(d);
    }
    // A label or a prefix is fine too.
    familiar_world::instance::load(dir)
        .ok()?
        .into_iter()
        .find(|w| w.id.starts_with(id) || w.label.eq_ignore_ascii_case(id))
        .map(|w| root.join(w.id))
}

pub fn cmd_autonomy(args: &[String]) -> ExitCode {
    let sub = args.first().map(String::as_str).unwrap_or("show");
    let f = super::flags(args);
    let dir = familiar_kernel::store::data_dir(f.get("data-dir").map(String::as_str));
    let root = super::world_store_root(&dir, f.get("store-root").map(String::as_str));
    let positional: Vec<&String> = {
        let mut out = Vec::new();
        let mut skip_next = false;
        for a in args.iter().skip(1) {
            if skip_next {
                skip_next = false;
                continue;
            }
            if let Some(key) = a.strip_prefix("--") {
                skip_next = !key.contains('=');
                continue;
            }
            out.push(a);
        }
        out
    };
    let Some(ship) = positional.first() else {
        eprintln!(
            "autonomy: `familiar autonomy show <ship>` | `set <ship> <surface>=<advise|confirm|auto> …` | \
             `advice <ship> [--all]` | `approve <ship> <proposal-id>` | `deny <ship> <proposal-id>`\n\
             surfaces: * | navigation[.course|.fuel|.rescue] | freight[.book|.collect|.cancel] | \
             market[.buy|.sell|.carry] | ship[.repair|.refit|.crew|.frame|.lease] | racing[.plot|.line|.refusal]"
        );
        return ExitCode::FAILURE;
    };
    let Some(ship_dir) = ship_dir_for(&dir, &root, ship) else {
        eprintln!("autonomy: no ship store for `{ship}`");
        return ExitCode::FAILURE;
    };
    let mut dial = familiar_whisker::store::load_dial(&ship_dir);
    match sub {
        "show" => {
            println!(
                "{} — the dial ({}):",
                ship_dir.file_name().unwrap_or_default().to_string_lossy(),
                autonomy::DIAL_FILE
            );
            if dial.settings.is_empty() {
                println!("  (no settings: everything bought is AUTO; rescue is ADVISE)");
            }
            for (k, v) in &dial.settings {
                println!("  {k} = {}", v.name());
            }
            println!("  effective:");
            let mut fam = "";
            for s in Surface::all() {
                if s.family() != fam {
                    fam = s.family();
                    print!("    {fam}:");
                }
                print!(" {}={}", s.category(), dial.level(*s).name());
                if Surface::all()
                    .iter()
                    .skip_while(|x| **x != *s)
                    .nth(1)
                    .map(|n| n.family() != fam)
                    .unwrap_or(true)
                {
                    println!();
                }
            }
            ExitCode::SUCCESS
        }
        "set" => {
            let mut changed = 0;
            for kv in positional.iter().skip(1) {
                let Some((k, v)) = kv.split_once('=') else {
                    eprintln!("autonomy set: `{kv}` is not <surface>=<level>");
                    return ExitCode::FAILURE;
                };
                let Some(level) = Level::parse(v) else {
                    eprintln!("autonomy set: `{v}` is not advise | confirm | auto");
                    return ExitCode::FAILURE;
                };
                if let Err(e) = dial.set(k, level) {
                    eprintln!("autonomy set: {e}");
                    return ExitCode::FAILURE;
                }
                changed += 1;
            }
            if changed == 0 {
                eprintln!("autonomy set: nothing to set");
                return ExitCode::FAILURE;
            }
            match familiar_whisker::store::save_dial(&ship_dir, &dial) {
                Ok(()) => {
                    println!("set {changed} — the pilot reads the dial every fold");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("autonomy set: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        "advice" => {
            let all = f.contains_key("all");
            let text = std::fs::read_to_string(ship_dir.join("journal.jsonl")).unwrap_or_default();
            let approvals = familiar_whisker::store::load_approvals(&ship_dir);
            let mut shown = 0;
            let lines: Vec<Value> = text
                .lines()
                .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                .filter(|v| {
                    matches!(
                        v.get("event").and_then(Value::as_str),
                        Some("advice") | Some("proposed") | Some("proposal-lapsed")
                    )
                })
                .collect();
            let take = if all {
                lines.len()
            } else {
                lines.len().min(20)
            };
            for v in lines.iter().rev().take(take).rev() {
                let ev = v["event"].as_str().unwrap_or("");
                let id = v.get("id").and_then(Value::as_str).unwrap_or("");
                let answered = approvals
                    .iter()
                    .rev()
                    .find(|a| a.id == id)
                    .map(|a| {
                        if a.approved {
                            " ✔ approved"
                        } else {
                            " ✘ denied"
                        }
                    })
                    .unwrap_or("");
                println!(
                    "t{} {:<16} {}{}{}",
                    v["tick"],
                    ev,
                    v["would"].as_str().unwrap_or(""),
                    if ev == "proposed" {
                        format!("  [{id}, until t{}]", v["expires"])
                    } else {
                        String::new()
                    },
                    answered
                );
                if let Some(w) = v.get("why").and_then(Value::as_str) {
                    if !w.is_empty() {
                        println!("      because {w}");
                    }
                }
                shown += 1;
            }
            if shown == 0 {
                println!("no advice or proposals on the journal — the dial is auto everywhere it matters");
            }
            ExitCode::SUCCESS
        }
        "approve" | "deny" => {
            let Some(id) = positional.get(1) else {
                eprintln!("autonomy {sub}: `familiar autonomy {sub} <ship> <proposal-id>`");
                return ExitCode::FAILURE;
            };
            let proposals = familiar_whisker::store::load_proposals(&ship_dir);
            let Some(p) = proposals.iter().rev().find(|p| p.id == **id) else {
                eprintln!("autonomy {sub}: no proposal {id} on file");
                return ExitCode::FAILURE;
            };
            familiar_whisker::store::append_approval(
                &ship_dir,
                &Approval {
                    id: id.to_string(),
                    approved: sub == "approve",
                    at: super::now_secs(),
                },
            );
            println!(
                "{} {} — {} (the pilot acts on its next fold if the proposal has not lapsed at t{})",
                if sub == "approve" { "approved" } else { "denied" },
                id,
                p.describe,
                p.expires_tick
            );
            ExitCode::SUCCESS
        }
        other => {
            eprintln!(
                "autonomy: unknown subcommand `{other}` — show | set | advice | approve | deny"
            );
            ExitCode::FAILURE
        }
    }
}
