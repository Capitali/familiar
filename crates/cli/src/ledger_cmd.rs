//! `familiar ledger` — the refusal ledger, read, verified and exported.
//!
//!   familiar ledger                      # verify the chain and say how long it is
//!   familiar ledger verify
//!   familiar ledger show [--redacted] [--last N]
//!   familiar ledger export --out <file> [--redacted]
//!
//! `--redacted` replaces the three free-text fields with their hashes; the copy still
//! verifies (Layer 4 of The Service Charter). The public copy is always the redacted one.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use familiar_kernel::ledger::{self, Record};
use familiar_kernel::store;

pub fn cmd_ledger(args: &[String], flags: &HashMap<String, String>) -> ExitCode {
    let sub = args.first().map(String::as_str).unwrap_or("verify");
    let dir = store::data_dir(flags.get("data-dir").map(String::as_str));
    let redacted = flags.contains_key("redacted");
    let records = match ledger::load(&dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("ledger: {e}");
            return ExitCode::FAILURE;
        }
    };
    match sub {
        "verify" => verify(&records),
        "show" => {
            let last = flags
                .get("last")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(20);
            show(&records, redacted, last)
        }
        "export" => {
            let Some(out) = flags.get("out") else {
                eprintln!("ledger export: --out <file> is required");
                return ExitCode::FAILURE;
            };
            export(&records, redacted, Path::new(out))
        }
        other => {
            eprintln!("ledger: unknown subcommand '{other}' (verify | show | export)");
            ExitCode::FAILURE
        }
    }
}

fn verify(records: &[Record]) -> ExitCode {
    match ledger::verify(records) {
        Ok(0) => {
            println!("ledger: empty — nothing has been refused, escalated or recorded yet");
            ExitCode::SUCCESS
        }
        Ok(n) => {
            let last = &records[n - 1];
            println!(
                "ledger: {n} record(s) verified from genesis; tail {} at {}",
                &last.hash[..12],
                last.timestamp
            );
            ExitCode::SUCCESS
        }
        Err(b) => {
            eprintln!("ledger: BROKEN at {b}");
            ExitCode::FAILURE
        }
    }
}

fn show(records: &[Record], redacted: bool, last: usize) -> ExitCode {
    let start = records.len().saturating_sub(last);
    for (i, rec) in records.iter().enumerate().skip(start) {
        let r = if redacted {
            ledger::redacted(rec)
        } else {
            rec.clone()
        };
        println!(
            "{i:>5}  {}  {:<10}  {:<5}  {}  {}",
            r.timestamp,
            format!("{:?}", r.action_type).to_lowercase(),
            r.trigger,
            &r.hash[..12],
            r.command_summary
        );
        if !redacted {
            println!("       {} — {}", r.affected_population, r.reasoning);
        }
    }
    match ledger::verify(records) {
        Ok(n) => {
            println!("{n} record(s); chain verifies");
            ExitCode::SUCCESS
        }
        Err(b) => {
            eprintln!("chain BROKEN at {b}");
            ExitCode::FAILURE
        }
    }
}

fn export(records: &[Record], redacted: bool, out: &Path) -> ExitCode {
    let mut f = match fs::File::create(out) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("ledger export: {}: {e}", out.display());
            return ExitCode::FAILURE;
        }
    };
    let mut copy: Vec<Record> = Vec::with_capacity(records.len());
    for rec in records {
        let r = if redacted {
            ledger::redacted(rec)
        } else {
            rec.clone()
        };
        if writeln!(f, "{}", serde_json::to_string(&r).unwrap_or_default()).is_err() {
            eprintln!("ledger export: write failed");
            return ExitCode::FAILURE;
        }
        copy.push(r);
    }
    match ledger::verify(&copy) {
        Ok(n) => {
            println!(
                "ledger: {n} record(s) written to {}{}; the copy verifies",
                out.display(),
                if redacted { " (redacted)" } else { "" }
            );
            ExitCode::SUCCESS
        }
        Err(b) => {
            eprintln!("ledger export: the copy does not verify — {b}");
            ExitCode::FAILURE
        }
    }
}
