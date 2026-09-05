//! Run production order #1 live: the familiar manufactures its own SP548E
//! driver and converges it against the bench oracle.
//!
//! Usage: familiar-factory-run <data-dir> [max-iterations]
//!
//! This wires the factory's generation seam to the real reasoner via
//! `familiar_llm::consult`, which participates in the daemon's consult lane —
//! so running this alongside the live daemon is safe. It stops at the bench
//! rung: generation → validate → converge on the offline bench oracle in the
//! jail. The live read/act/witness rungs and the declaration are later, gated
//! steps.

use std::path::PathBuf;

use familiar_factory::converge::{converge, ConvergeEnv, ConvergeOutcome};
use familiar_factory::generate::ReasonerAdapter;
use familiar_workshop::ledger::{EventKind, Ledger};
use familiar_workshop::manifest::digest_bytes;
use familiar_workshop::order::{OracleRung, ResearchEntry, Toolchain, WorkOrder};

fn order_one() -> WorkOrder {
    WorkOrder {
        id: "order-0001-motorlights".into(),
        requester: "ian".into(),
        // Scoped small for the first live run: the available reasoner provider
        // caps at 2048 tokens, so order #1 begins with just the state-query
        // frame builder + its decode, proven by one stdlib self-test. The rest
        // of the surface (power, brightness) follows as provider budget allows.
        // KEEP THE REPLY UNDER ~1200 TOKENS: one small driver file and one
        // small self-test, nothing more.
        goal: "a tiny Python module `driver.py` with build_state_query() returning the \
               SP548E 0x02 state-query frame as bytes (framing 0x53|type|key|total_frags|\
               frag_idx|payload_len|payload, key=0x00, single fragment, empty payload), and \
               decode_state(reply: bytes) returning a dict with 'mode' (byte[30]) and \
               'brightness' (byte[33]); plus one stdlib self-test. Keep the whole JSON reply \
               under ~1200 tokens."
            .into(),
        wording: "autonomously discover, write, execute, and automate".into(),
        subject: "ble:mfr=0x5053,wifi_mac=ba:16:b5:fe:19:82".into(),
        capability_surface: vec!["state".into()],
        research: vec![ResearchEntry {
            title: "SP548E BLE protocol: framing 53|type|key|total_frags|frag_idx|len|payload; \
                    0x02=state query (18 frags/245B), 0x50=power, 0x51=brightness [which,level], \
                    0x52=static RGB [r,g,b,level]; state offsets [30]=mode [33]=brightness; \
                    key=0x00 unencrypted; colour is never echoed back"
                .into(),
            source: "household record (CLAUDE.md river.io network), verified 2026-07-28; \
                     docs/research/sp548e-protocol.md"
                .into(),
            digest: digest_bytes(b"sp548e-protocol-notes"),
        }],
        required_gates: vec!["allow_execute".into(), "allow_actuate".into()],
        oracle_plan: vec![
            OracleRung::Bench,
            OracleRung::Read,
            OracleRung::Act,
            OracleRung::Witness,
        ],
        toolchain: Toolchain {
            interpreter: "python3.13".into(),
            lock_digest: String::new(),
        },
        containment: "jail-v1".into(),
    }
}

fn interpreter() -> PathBuf {
    // A real interpreter, resolved outside the jail — /usr/bin/python3 is the
    // xcrun shim and cannot run inside it (see `bench::find_python`).
    familiar_factory::bench::find_python().unwrap_or_else(|| {
        eprintln!("factory: no real python3 (Homebrew or `xcrun --find python3`) — the jail cannot run the /usr/bin shim");
        std::process::exit(2);
    })
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let data_dir = PathBuf::from(args.next().unwrap_or_else(|| {
        eprintln!("usage: familiar-factory-run <data-dir> [max-iterations]");
        std::process::exit(2);
    }));
    let max_iters: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(4);

    let order = order_one();
    let ws = data_dir.join("factory").join(&order.id);
    let _ = std::fs::create_dir_all(&ws);

    // Resume, never delete (codex whole-factory review, blocker 1): the ledger
    // is the order's only truth and its lock may be another writer's. An
    // existing order continues at its next iteration; a terminal one is
    // refused; only an absent ledger is opened here.
    let ledger_path = ws.join("ledger.jsonl");
    let ledger = Ledger::at(&ledger_path);
    if ledger_path.exists() {
        match ledger.state() {
            Ok(state) if state.order.id != order.id => {
                eprintln!(
                    "factory: {} holds {}, not {}",
                    ledger_path.display(),
                    state.order.id,
                    order.id
                );
                std::process::exit(2);
            }
            Ok(state) if state.terminal() => {
                eprintln!(
                    "factory: {} is already terminal (commissioned={}, closed={:?}); a new \
                     run needs a new order id, not this ledger's path",
                    order.id, state.commissioned, state.closed
                );
                std::process::exit(2);
            }
            Ok(state) => eprintln!(
                "factory: resuming {} at iteration {} from {}",
                order.id,
                state.iteration + 1,
                ledger_path.display()
            ),
            Err(e) => {
                eprintln!("factory: the existing ledger will not replay: {e}");
                std::process::exit(2);
            }
        }
    } else {
        ledger
            .append(
                now_secs(),
                &order.id,
                EventKind::Opened {
                    order: Box::new(order.clone()),
                },
            )
            .expect("open order");
    }

    // The reasoner closure: hand the prompt to the familiar's real mind via the
    // consult seam (which respects the daemon's lane). A refusal/rate-limit
    // surfaces as an error the convergence loop reports honestly.
    let dd = data_dir.clone();
    let ask = move |prompt: &str| -> std::io::Result<String> {
        match familiar_llm::consult(&dd, prompt) {
            Ok(familiar_llm::Outcome::Response(text)) => Ok(text),
            Ok(familiar_llm::Outcome::Refused(r)) => {
                Err(std::io::Error::other(format!("reasoner refused: {r}")))
            }
            Ok(familiar_llm::Outcome::RateLimited(r)) => {
                Err(std::io::Error::other(format!("reasoner rate-limited: {r}")))
            }
            Ok(familiar_llm::Outcome::Yielded(r)) => {
                Err(std::io::Error::other(format!("reasoner yielded: {r}")))
            }
            Err(e) => Err(e),
        }
    };
    let adapter = ReasonerAdapter::new(ask);

    let env = ConvergeEnv {
        interpreter: interpreter(),
        workspace: ws.join("work"),
        // Hide the repo and the data dir from the jailed candidate beyond the
        // mandatory set.
        extra_hidden: vec![
            data_dir.clone(),
            PathBuf::from("/Users/ian/Projects/familiar"),
        ],
        bench_wall_secs: 20,
    };

    eprintln!(
        "factory: opening {} — the familiar will write its own driver and converge it \
         (max {} iterations)…",
        order.id, max_iters
    );
    match converge(&ledger, &order, &adapter, &env, max_iters, now_secs()) {
        Ok(report) => {
            eprintln!(
                "factory: bench passes per iteration: {:?}",
                report.bench_passes
            );
            match report.outcome {
                ConvergeOutcome::Converged { iteration } => {
                    let dir = familiar_factory::converge::iteration_candidate_dir(
                        &ws.join("work"),
                        iteration,
                    );
                    println!(
                        "CONVERGED on iteration {iteration}. The familiar's own driver passed \
                         the bench oracle in the jail.\nWinning candidate: {}",
                        dir.display()
                    );
                    println!("Ledger: {}", ledger_path.display());
                }
                other => {
                    println!("NOT CONVERGED: {other:?}");
                    println!("Ledger: {}", ledger_path.display());
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("factory: run error: {e}");
            std::process::exit(1);
        }
    }
}
