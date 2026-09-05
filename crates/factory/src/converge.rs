//! The convergence loop — the factory's brain (DF's "converge on the oracle
//! until green, iterations counted").
//!
//! Each iteration: ask the generation adapter for a candidate (handing it the
//! previous failure as feedback), validate the typed outcome against the
//! order, record the generation to the ledger, then — for a real candidate —
//! materialize it and run the **bench** oracle inside the jail. A green bench
//! records a passing `Bench` verdict and the loop stops; a red bench records a
//! failing verdict and the loop retries with the failure fed back, up to
//! `max_iterations`. A refused generation is recorded and ends the loop (the
//! reasoner declined; nothing to converge). Every step is a validated ledger
//! append — the ledger's replay remains the sole truth.
//!
//! This is the offline heart of the factory: it proves the familiar can write
//! code, submit it to an oracle, learn from the oracle's verdict, and try
//! again — all bounded, contained, and recorded. The live read/act/witness
//! rungs come after a green bench, behind the gate and the human's hand.

use std::path::{Path, PathBuf};

use familiar_workshop::ledger::{EventKind, Ledger};
use familiar_workshop::order::{validate_outcome, GenerationOutcome, OracleRung, WorkOrder};

use crate::bench::run_bench;
use crate::generate::GenerationAdapter;
use crate::materialize::materialize;

/// How convergence ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvergeOutcome {
    /// The bench went green on this 1-based iteration.
    Converged { iteration: u32 },
    /// The reasoner refused on this iteration; the loop stopped.
    Refused { iteration: u32, code: String },
    /// `max_iterations` passed without a green bench.
    Exhausted { iterations: u32 },
    /// The adapter produced an outcome that failed the order's contract; the
    /// loop stopped rather than record an invalid generation.
    InvalidOutcome { iteration: u32, reason: String },
}

#[derive(Debug, Clone)]
pub struct ConvergeReport {
    pub outcome: ConvergeOutcome,
    /// Per-iteration bench pass/fail (in order), for the human-facing summary.
    pub bench_passes: Vec<bool>,
}

/// Where a converge run does its work. `interpreter` runs the bench tests;
/// `workspace` holds per-iteration candidate + scratch dirs; `extra_hidden`
/// names the daemon's real repo/data dir to hide beyond the mandatory set.
pub struct ConvergeEnv {
    pub interpreter: PathBuf,
    pub workspace: PathBuf,
    pub extra_hidden: Vec<PathBuf>,
    pub bench_wall_secs: u64,
}

/// Run the convergence loop for `order`, recording to `ledger`. The ledger
/// must already carry the order's `Opened` event.
pub fn converge(
    ledger: &Ledger,
    order: &WorkOrder,
    adapter: &dyn GenerationAdapter,
    env: &ConvergeEnv,
    max_iterations: u32,
    now: u64,
) -> std::io::Result<ConvergeReport> {
    // Resume from the ledger, never from a counter of our own: the replayed
    // state says which iteration was last counted, so a runner restarted on an
    // existing order continues at the next one instead of re-numbering from 1
    // (codex whole-factory review, blocker 1). A terminal order is refused. The
    // previous failure's feedback is not replayed; the next candidate starts
    // from the order alone.
    let state = ledger.state().map_err(std::io::Error::other)?;
    if state.order.id != order.id {
        return Err(std::io::Error::other(format!(
            "ledger holds {}, not {}",
            state.order.id, order.id
        )));
    }
    if state.terminal() {
        return Err(std::io::Error::other(
            "order is terminal; a new run needs a new order id, not this ledger",
        ));
    }
    let mut bench_passes = Vec::new();
    let mut feedback: Option<String> = None;

    for iteration in (state.iteration + 1)..=max_iterations {
        let result = adapter.generate(order, feedback.as_deref())?;

        // Validate the typed outcome before it can be recorded or run.
        if let Err(e) = validate_outcome(order, &result.outcome) {
            return Ok(ConvergeReport {
                outcome: ConvergeOutcome::InvalidOutcome {
                    iteration,
                    reason: e.to_string(),
                },
                bench_passes,
            });
        }

        // Record the generation (the door re-validates the outcome).
        ledger
            .append_generation(now, &order.id, iteration, &result.outcome)
            .map_err(std::io::Error::other)?;

        let candidate = match &result.outcome {
            GenerationOutcome::Refused(r) => {
                return Ok(ConvergeReport {
                    outcome: ConvergeOutcome::Refused {
                        iteration,
                        code: r.code.clone(),
                    },
                    bench_passes,
                });
            }
            GenerationOutcome::Candidate {
                manifest,
                self_tests,
                ..
            } => (manifest, self_tests),
        };
        let (manifest, self_tests) = candidate;

        // Materialize into this iteration's candidate dir.
        let iter_dir = env.workspace.join(format!("iter-{iteration}"));
        let cand_dir = iter_dir.join("candidate");
        let scratch = iter_dir.join("scratch");
        let _ = std::fs::remove_dir_all(&iter_dir);
        materialize(manifest, &result.artifacts, &cand_dir).map_err(std::io::Error::other)?;

        // Run the bench oracle in the jail.
        let report = run_bench(
            &cand_dir,
            self_tests,
            &env.interpreter,
            &scratch,
            &env.extra_hidden,
            env.bench_wall_secs,
        )?;
        bench_passes.push(report.passed);

        // Record the bench verdict.
        ledger
            .append(
                now,
                &order.id,
                EventKind::RungVerdict {
                    iteration,
                    rung: OracleRung::Bench,
                    pass: report.passed,
                    evidence_digest: report.evidence_digest.clone(),
                },
            )
            .map_err(std::io::Error::other)?;

        if report.passed {
            return Ok(ConvergeReport {
                outcome: ConvergeOutcome::Converged { iteration },
                bench_passes,
            });
        }

        // Feed the failure back to the next iteration.
        feedback = Some(summarize_failure(&report));
    }

    Ok(ConvergeReport {
        outcome: ConvergeOutcome::Exhausted {
            iterations: max_iterations,
        },
        bench_passes,
    })
}

fn summarize_failure(report: &crate::bench::BenchReport) -> String {
    let mut s = String::from("bench failed:\n");
    for r in &report.results {
        if r.outcome != familiar_jail::JailOutcome::Ok {
            s.push_str(&format!("- {} [{:?}]\n{}\n", r.path, r.outcome, r.output));
        }
    }
    s
}

/// Read `dest` back as the candidate's on-disk tree helper for callers that
/// want to keep the winning iteration.
pub fn iteration_candidate_dir(workspace: &Path, iteration: u32) -> PathBuf {
    workspace
        .join(format!("iter-{iteration}"))
        .join("candidate")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use familiar_workshop::ledger::{EventKind, Ledger};
    use familiar_workshop::manifest::{digest_bytes, FileEntry, FileRole, Manifest};
    use familiar_workshop::order::{ResearchEntry, Toolchain};

    use crate::generate::scripted::Scripted;
    use crate::generate::GenerationResult;

    fn python() -> Option<PathBuf> {
        for p in [
            "/opt/homebrew/bin/python3.13",
            "/opt/homebrew/bin/python3",
            "/usr/bin/python3",
        ] {
            if Path::new(p).exists() {
                return Some(PathBuf::from(p));
            }
        }
        None
    }

    fn order() -> WorkOrder {
        WorkOrder {
            id: "order-conv".into(),
            requester: "ian".into(),
            goal: "manufacture a tiny driver".into(),
            wording: "build it".into(),
            subject: "ble:mfr=0x5053,wifi_mac=ba:16:b5:fe:19:82".into(),
            capability_surface: vec!["state".into(), "off".into()],
            research: vec![ResearchEntry {
                title: "notes".into(),
                source: "record".into(),
                digest: digest_bytes(b"n"),
            }],
            required_gates: vec!["allow_execute".into()],
            oracle_plan: vec![OracleRung::Bench, OracleRung::Read, OracleRung::Act],
            toolchain: Toolchain {
                interpreter: "python3.13".into(),
                lock_digest: String::new(),
            },
            containment: "jail-v1".into(),
        }
    }

    /// A candidate whose single self-test either passes or fails.
    fn candidate(test_body: &str) -> GenerationResult {
        let driver = b"def frame(op): return bytes([0x53, op])\n".to_vec();
        let test = test_body.as_bytes().to_vec();
        let mut artifacts = BTreeMap::new();
        let dd = digest_bytes(&driver);
        let td = digest_bytes(&test);
        artifacts.insert(dd.clone(), driver);
        artifacts.insert(td.clone(), test);
        let outcome = GenerationOutcome::Candidate {
            manifest: Manifest {
                files: vec![
                    FileEntry {
                        path: "driver.py".into(),
                        digest: dd,
                        role: FileRole::Source,
                    },
                    FileEntry {
                        path: "test_driver.py".into(),
                        digest: td,
                        role: FileRole::SelfTest,
                    },
                ],
            },
            entrypoints: vec!["driver.py".into()],
            self_tests: vec!["test_driver.py".into()],
            declared_effects: vec!["off".into()],
            toolchain_lock: String::new(),
            capability_surface: vec!["state".into(), "off".into()],
        };
        GenerationResult { outcome, artifacts }
    }

    fn env(tag: &str) -> (ConvergeEnv, PathBuf) {
        let py = python().unwrap();
        let ws = std::env::temp_dir().join(format!("familiar-conv-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&ws);
        (
            ConvergeEnv {
                interpreter: py,
                workspace: ws.clone(),
                extra_hidden: Vec::new(),
                bench_wall_secs: 10,
            },
            ws,
        )
    }

    fn open_ledger(ws: &Path, order: &WorkOrder) -> Ledger {
        let l = Ledger::at(ws.join("ledger.jsonl"));
        std::fs::create_dir_all(ws).unwrap();
        l.append(
            1,
            &order.id,
            EventKind::Opened {
                order: Box::new(order.clone()),
            },
        )
        .expect("open");
        l
    }

    #[test]
    fn it_converges_on_the_second_iteration_and_records_the_history() {
        if python().is_none() {
            eprintln!("skipping: no python3");
            return;
        }
        // This one benches the candidate, and the bench runs only inside the
        // macOS sandbox-exec jail — skip loudly where the platform lacks it.
        if !std::path::Path::new("/usr/bin/sandbox-exec").exists() {
            eprintln!("skipping: no /usr/bin/sandbox-exec");
            return;
        }
        let (e, ws) = env("two");
        let o = order();
        let ledger = open_ledger(&ws, &o);

        // Iteration 1: a self-test that fails. Iteration 2: one that passes.
        let bad = candidate(&format!(
            "import sys; sys.path.insert(0,{:?})\nimport driver\nassert driver.frame(0)==b'WRONG'\n",
            ws.join("iter-1/candidate").to_string_lossy()
        ));
        let good = candidate(&format!(
            "import sys; sys.path.insert(0,{:?})\nimport driver\nassert driver.frame(0)==bytes([0x53,0])\nprint('OK')\n",
            ws.join("iter-2/candidate").to_string_lossy()
        ));
        let adapter = Scripted::new(vec![bad, good]);

        let report = converge(&ledger, &o, &adapter, &e, 5, 1000).expect("converge");
        assert_eq!(report.outcome, ConvergeOutcome::Converged { iteration: 2 });
        assert_eq!(report.bench_passes, vec![false, true]);

        // The second generation was handed the first's failure as feedback.
        let seen = adapter.feedback_seen.borrow();
        assert!(seen[0].is_none(), "first attempt has no feedback");
        assert!(
            seen[1].as_deref().unwrap_or("").contains("bench failed"),
            "second attempt gets the failure fed back"
        );

        // The ledger's replay proves the history: 2 generations, a failing then
        // a passing Bench verdict, iteration counter at 2.
        let state = ledger.state().expect("replay");
        assert_eq!(state.iteration, 2);
        assert!(state.rungs_passed.contains(&OracleRung::Bench));
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn a_restarted_runner_resumes_at_the_next_iteration_and_refuses_a_terminal_order() {
        if python().is_none() {
            eprintln!("skipping: no python3");
            return;
        }
        if !std::path::Path::new("/usr/bin/sandbox-exec").exists() {
            eprintln!("skipping: no /usr/bin/sandbox-exec");
            return;
        }
        let (e, ws) = env("resume");
        let o = order();
        let ledger = open_ledger(&ws, &o);

        // First run: one iteration, it fails, the run ends exhausted.
        let bad = candidate(&format!(
            "import sys; sys.path.insert(0,{:?})\nimport driver\nassert driver.frame(0)==b'WRONG'\n",
            ws.join("iter-1/candidate").to_string_lossy()
        ));
        let first = converge(&ledger, &o, &Scripted::new(vec![bad]), &e, 1, 1000).expect("run 1");
        assert_eq!(first.outcome, ConvergeOutcome::Exhausted { iterations: 1 });
        let events_before = ledger.read().expect("read").len();

        // Second run on the SAME ledger (never deleted, never re-opened): it
        // continues at iteration 2 — the ledger says where we were.
        let good = candidate(&format!(
            "import sys; sys.path.insert(0,{:?})\nimport driver\nassert driver.frame(0)==bytes([0x53,0])\n",
            ws.join("iter-2/candidate").to_string_lossy()
        ));
        let second = converge(&ledger, &o, &Scripted::new(vec![good]), &e, 5, 2000).expect("run 2");
        assert_eq!(second.outcome, ConvergeOutcome::Converged { iteration: 2 });
        let events = ledger.read().expect("read");
        assert!(
            events.len() > events_before,
            "the second run appended to the first's truth"
        );
        for (i, ev) in events.iter().enumerate() {
            assert_eq!(ev.seq, i as u64 + 1, "one consecutive history");
        }
        let state = ledger.state().expect("replay");
        assert_eq!(state.iteration, 2);

        // A terminal order is refused outright: no generation, no spawn.
        ledger
            .append(
                3000,
                &o.id,
                EventKind::Closed {
                    reason: "withdrawn".into(),
                },
            )
            .expect("close");
        let adapter = Scripted::new(vec![candidate("assert True\n")]);
        let err = converge(&ledger, &o, &adapter, &e, 5, 4000).expect_err("terminal refused");
        assert!(err.to_string().contains("terminal"), "{err}");
        assert!(
            adapter.feedback_seen.borrow().is_empty(),
            "nothing was generated"
        );
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn a_refusal_ends_the_loop_and_is_recorded() {
        if python().is_none() {
            eprintln!("skipping: no python3");
            return;
        }
        let (e, ws) = env("refuse");
        let o = order();
        let ledger = open_ledger(&ws, &o);

        use familiar_workshop::order::Refusal;
        let refusal = GenerationResult {
            outcome: GenerationOutcome::Refused(Refusal {
                code: "no-spec".into(),
                rationale: "insufficient research".into(),
                unmet_requirements: vec!["framing".into()],
                evidence: None,
            }),
            artifacts: BTreeMap::new(),
        };
        let adapter = Scripted::new(vec![refusal]);
        let report = converge(&ledger, &o, &adapter, &e, 5, 1000).expect("converge");
        assert_eq!(
            report.outcome,
            ConvergeOutcome::Refused {
                iteration: 1,
                code: "no-spec".into()
            }
        );
        let state = ledger.state().expect("replay");
        assert!(state.refused);
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn an_invalid_outcome_stops_before_recording_generation() {
        if python().is_none() {
            eprintln!("skipping: no python3");
            return;
        }
        let (e, ws) = env("invalid");
        let o = order();
        let ledger = open_ledger(&ws, &o);

        // A candidate claiming an effect beyond the order.
        let mut bad = candidate("assert True\n");
        if let GenerationOutcome::Candidate {
            declared_effects, ..
        } = &mut bad.outcome
        {
            declared_effects.push("unlock-door".into());
        }
        let adapter = Scripted::new(vec![bad]);
        let report = converge(&ledger, &o, &adapter, &e, 5, 1000).expect("converge");
        assert!(matches!(
            report.outcome,
            ConvergeOutcome::InvalidOutcome { iteration: 1, .. }
        ));
        // No generation was recorded — the order is still at iteration 0.
        let state = ledger.state().expect("replay");
        assert_eq!(state.iteration, 0);
        let _ = std::fs::remove_dir_all(&ws);
    }
}
