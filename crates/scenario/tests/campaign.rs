//! The campaign runner, end to end: cells run in order, state checkpoints,
//! STOP stops, --resume runs exactly the pending cells, evidence collects.

use familiar_scenario::campaign::{self, CampaignPlan, Halt};
use familiar_scenario::evidence::Evidence;
use std::fs;
use std::path::PathBuf;

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../scenarios")
        .join(rel)
}

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn temp(name: &str) -> Temp {
    let p = std::env::temp_dir().join(format!(
        "familiar_campaign_test_{name}_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    Temp(p)
}

fn plan(t: &Temp, fixtures: Vec<PathBuf>, adapter: Option<PathBuf>) -> CampaignPlan {
    let body = serde_json::json!({
        "fixtures": fixtures,
        "controls": "AC",
        "episodes": 2,
        "replicates": 2,
        "llm_adapter": adapter,
        "llm_required": false,
        "llm_patience_secs": 0,
        "llm_retry_backoff_secs": 0,
        "out": t.0.join("out"),
    });
    serde_json::from_value(body).unwrap()
}

/// An adapter that always answers with a futile-but-valid script.
fn adapter(dir: &std::path::Path) -> PathBuf {
    let path = dir.join("call_llm.sh");
    fs::write(
        &path,
        "#!/bin/sh\nd=\"$(dirname \"$0\")\"\ncat > \"$d/response.json\" <<'RESP'\n```sh\n#!/bin/sh\ntouch tried\nexit 0\n```\nRESP\n",
    )
    .unwrap();
    path
}

#[test]
fn campaign_runs_all_cells_and_evidence_collects() {
    let t = temp("full");
    let p = plan(
        &t,
        vec![
            fixture("process-failures/backup-spaces.json"),
            fixture("resource-exhaustion/log-growth.json"),
        ],
        Some(adapter(&t.0)),
    );
    let outcome = campaign::run(&p, false, false).unwrap();
    assert_eq!(outcome.halt, Halt::Complete);
    // 2 fixtures x 2 controls (A, C) x 2 replicates.
    assert_eq!(outcome.cells_run, 8);
    assert_eq!(outcome.cells_failed, 0);

    let ev = Evidence::collect(&t.0.join("out").join("runs")).unwrap();
    assert_eq!(ev.scenarios.len(), 2);
    for s in &ev.scenarios {
        assert_eq!(s.controls["A"].n, 2);
        assert_eq!(s.controls["C"].n, 2);
        assert_eq!(s.controls["A"].boundary_violations, 0);
    }

    // Rerunning with --resume skips everything.
    let again = campaign::run(&p, true, false).unwrap();
    assert_eq!(again.cells_run, 0);
    assert_eq!(again.cells_skipped, 8);
}

#[test]
fn stop_file_halts_and_resume_finishes_the_rest() {
    let t = temp("stop");
    let p = plan(
        &t,
        vec![fixture("process-failures/backup-spaces.json")],
        None,
    );
    // A STOP file present from the start: the campaign stops before cell one.
    fs::create_dir_all(&p.out).unwrap();
    fs::write(p.out.join("STOP"), "").unwrap();
    let outcome = campaign::run(&p, false, false).unwrap();
    assert_eq!(outcome.halt, Halt::Stopped);
    assert_eq!(outcome.cells_run, 0);

    // Remove STOP; resume runs exactly the pending cells.
    fs::remove_file(p.out.join("STOP")).unwrap();
    let outcome = campaign::run(&p, true, false).unwrap();
    assert_eq!(outcome.halt, Halt::Complete);
    assert_eq!(outcome.cells_run, 4); // 1 fixture x AC x 2 replicates
}

#[test]
fn a_changed_plan_refuses_stale_state_without_force() {
    let t = temp("fingerprint");
    let p = plan(
        &t,
        vec![fixture("process-failures/backup-spaces.json")],
        None,
    );
    campaign::run(&p, false, false).unwrap();
    let mut changed = p.clone();
    changed.episodes = 3;
    let err = campaign::run(&changed, true, false).unwrap_err();
    assert!(err.to_string().contains("--force"), "{err}");
    // Forced, it proceeds (and reruns nothing that is already done).
    campaign::run(&changed, true, true).unwrap();
}

#[test]
fn provider_outage_pauses_the_campaign_with_state_saved() {
    let t = temp("pause");
    let dead = t.0.join("call_llm.sh");
    fs::write(&dead, "#!/bin/sh\nexit 2\n").unwrap();
    let mut p = plan(
        &t,
        vec![fixture("process-failures/backup-spaces.json")],
        Some(dead),
    );
    p.llm_required = true;
    let outcome = campaign::run(&p, false, false).unwrap();
    // Control A (never consults) completes; the first C cell pauses everything.
    assert_eq!(outcome.halt, Halt::Paused);
    let state = fs::read_to_string(p.out.join("campaign-state.json")).unwrap();
    assert!(state.contains("paused"), "{state}");
}

#[test]
fn llm_call_budget_stops_cleanly() {
    let t = temp("budget");
    let mut p = plan(
        &t,
        vec![
            fixture("process-failures/backup-spaces.json"),
            fixture("resource-exhaustion/log-growth.json"),
        ],
        Some(adapter(&t.0)),
    );
    p.max_llm_calls = 1;
    let outcome = campaign::run(&p, false, false).unwrap();
    assert_eq!(outcome.halt, Halt::BudgetReached);
    assert!(outcome.cells_run < 8);
    // Resuming with a lifted budget finishes the rest.
    let mut lifted = p.clone();
    lifted.max_llm_calls = 0;
    let outcome = campaign::run(&lifted, true, true).unwrap();
    assert_eq!(outcome.halt, Halt::Complete);
}
