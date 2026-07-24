//! The campaign runner — many cells, one unattended run (ADR-0010 at length).
//!
//! A **cell** is one (fixture × control × replicate); a campaign executes its
//! cells in deterministic order, checkpointing `campaign-state.json` after
//! every cell so `--resume` picks up exactly where it stopped. Cells are the
//! resume granularity by design: `harness::run` wipes its run dir, so a cell
//! restarts clean rather than resuming mid-episode.
//!
//! Unattended safety, independent of the adapter's own spend governor:
//! `max_llm_calls` / `max_wall_hours` stop the campaign cleanly at a cell
//! boundary; a `STOP` file in the output dir (touch it over SSH) does the
//! same; a cell that ends `llm_unavailable` pauses the whole campaign with
//! state saved rather than burning the queue against dead providers.
//!
//! This module owns every sleep and wall-clock read the engine library needs —
//! the harness and evaluator stay clock-free (rehearsal-compatible, ADR-0010).

use crate::harness::{self, Control, RunConfig};
use crate::report::RunReport;
use crate::scenario::{self, Scenario};
use crate::validate;
use crate::world::fnv1a;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// A campaign plan, as JSON (strict-parsed like fixtures).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignPlan {
    /// Fixture files or directories (expanded recursively, sorted).
    pub fixtures: Vec<PathBuf>,
    /// Which controls, e.g. "ABCD" (any order; run in canonical A→D order).
    #[serde(default = "default_controls")]
    pub controls: String,
    #[serde(default = "default_episodes")]
    pub episodes: u32,
    #[serde(default = "default_replicates")]
    pub replicates: u32,
    #[serde(default)]
    pub llm_adapter: Option<PathBuf>,
    /// Campaigns default to `llm_required`: a rate-limited window pauses the
    /// campaign instead of contaminating it with template episodes.
    #[serde(default = "default_true")]
    pub llm_required: bool,
    #[serde(default = "default_patience")]
    pub llm_patience_secs: u64,
    #[serde(default = "default_backoff")]
    pub llm_retry_backoff_secs: u64,
    #[serde(default = "default_adapter_timeout")]
    pub adapter_timeout_secs: u64,
    /// Minimum seconds between LLM-consulting cells (provider pacing).
    #[serde(default)]
    pub min_llm_interval_secs: u64,
    /// Stop cleanly after this many LLM calls across the campaign (0 = off).
    #[serde(default)]
    pub max_llm_calls: u32,
    /// Stop cleanly after this much wall time (0 = off).
    #[serde(default)]
    pub max_wall_hours: f64,
    /// Ablation names (harness::Ablation vocabulary) applied to every cell.
    #[serde(default)]
    pub ablations: Vec<String>,
    /// Required alongside a "law3-gate" ablation — the plan must acknowledge
    /// that boundary-violating artifacts will execute (sandboxed, recorded).
    #[serde(default)]
    pub acknowledge_law3_ablation: bool,
    /// Perception noise applied to every cell.
    #[serde(default)]
    pub noise: Option<crate::noise::NoiseSpec>,
    /// Output directory (state, runs/, evidence inputs).
    #[serde(default = "default_out")]
    pub out: PathBuf,
}

fn default_controls() -> String {
    "ABCD".into()
}
fn default_episodes() -> u32 {
    10
}
fn default_replicates() -> u32 {
    3
}
fn default_true() -> bool {
    true
}
fn default_patience() -> u64 {
    900
}
fn default_backoff() -> u64 {
    60
}
fn default_adapter_timeout() -> u64 {
    120
}
fn default_out() -> PathBuf {
    PathBuf::from("campaign-out")
}

/// Load a plan from JSON.
pub fn load_plan(path: &Path) -> io::Result<CampaignPlan> {
    let body = fs::read_to_string(path)?;
    serde_json::from_str(&body).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: {e}", path.display()),
        )
    })
}

/// One cell's checkpointed outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellState {
    /// "done" | "failed" | "paused"
    pub status: String,
    pub summary: String,
}

/// The whole campaign's checkpoint, saved after every cell.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CampaignState {
    /// Fingerprint of the plan this state belongs to — `--resume` refuses a
    /// changed plan unless forced.
    pub plan_fingerprint: u64,
    pub cells: BTreeMap<String, CellState>,
    pub llm_calls: u32,
}

const STATE_FILE: &str = "campaign-state.json";
const STOP_FILE: &str = "STOP";

/// Why the campaign returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Halt {
    /// Every cell is done.
    Complete,
    /// STOP file observed; state saved.
    Stopped,
    /// A cell ended llm_unavailable; state saved, resume when providers return.
    Paused,
    /// A budget cap (calls or wall) was reached; state saved.
    BudgetReached,
}

/// The campaign's outcome summary.
#[derive(Debug)]
pub struct CampaignOutcome {
    pub halt: Halt,
    pub cells_run: u32,
    pub cells_skipped: u32,
    pub cells_failed: u32,
}

/// Run (or resume) a campaign. `resume` keeps prior state and skips done
/// cells; `force` accepts a changed plan over old state.
pub fn run(plan: &CampaignPlan, resume: bool, force: bool) -> io::Result<CampaignOutcome> {
    fs::create_dir_all(&plan.out)?;
    let fingerprint = plan_fingerprint(plan)?;
    let state_path = plan.out.join(STATE_FILE);
    let mut state: CampaignState = if resume && state_path.is_file() {
        let s: CampaignState =
            serde_json::from_str(&fs::read_to_string(&state_path)?).map_err(io::Error::other)?;
        if s.plan_fingerprint != fingerprint && !force {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the plan changed since this state was written — rerun with --force to accept",
            ));
        }
        CampaignState {
            plan_fingerprint: fingerprint,
            ..s
        }
    } else {
        CampaignState {
            plan_fingerprint: fingerprint,
            ..CampaignState::default()
        }
    };

    // Load and validate every fixture up front: a campaign must fail at plan
    // time, not three hours in.
    let fixtures = expand_fixtures(&plan.fixtures)?;
    if fixtures.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the plan names no fixtures",
        ));
    }
    let mut scenarios = Vec::new();
    for path in &fixtures {
        let s = scenario::load(path)?;
        let violations = validate::check(&s)?;
        if validate::has_errors(&violations) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{}: fixture refused by validation", path.display()),
            ));
        }
        scenarios.push((path.clone(), s));
    }
    let controls = parse_controls(&plan.controls)?;
    let mut ablations = Vec::new();
    for name in &plan.ablations {
        let a = harness::Ablation::parse(name).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown ablation {name:?}"),
            )
        })?;
        ablations.push(a);
    }
    if ablations.contains(&harness::Ablation::Law3Gate) && !plan.acknowledge_law3_ablation {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the law3-gate ablation executes boundary-violating artifacts (sandboxed, \
             recorded); set acknowledge_law3_ablation: true in the plan to confirm",
        ));
    }

    let started = Instant::now();
    let mut outcome = CampaignOutcome {
        halt: Halt::Complete,
        cells_run: 0,
        cells_skipped: 0,
        cells_failed: 0,
    };
    let mut last_llm_cell: Option<Instant> = None;

    'cells: for (path, scn) in &scenarios {
        for control in &controls {
            for replicate in 1..=plan.replicates {
                let key = cell_key(scn, *control, replicate);
                if state.cells.get(&key).is_some_and(|c| c.status == "done") {
                    outcome.cells_skipped += 1;
                    continue;
                }
                // Graceful stop and budget checks happen at cell boundaries.
                if plan.out.join(STOP_FILE).is_file() {
                    outcome.halt = Halt::Stopped;
                    break 'cells;
                }
                if plan.max_llm_calls > 0 && state.llm_calls >= plan.max_llm_calls {
                    outcome.halt = Halt::BudgetReached;
                    break 'cells;
                }
                if plan.max_wall_hours > 0.0
                    && started.elapsed() >= Duration::from_secs_f64(plan.max_wall_hours * 3600.0)
                {
                    outcome.halt = Halt::BudgetReached;
                    break 'cells;
                }
                // Provider pacing between LLM-consulting cells.
                let consults = *control != Control::Baseline && plan.llm_adapter.is_some();
                if consults && plan.min_llm_interval_secs > 0 {
                    if let Some(prev) = last_llm_cell {
                        let due = Duration::from_secs(plan.min_llm_interval_secs);
                        let elapsed = prev.elapsed();
                        if elapsed < due {
                            std::thread::sleep(due - elapsed);
                        }
                    }
                }

                let cfg = RunConfig {
                    lab_dir: plan.out.join("runs"),
                    episodes: plan.episodes,
                    llm_adapter: plan.llm_adapter.clone(),
                    replicate,
                    llm_required: plan.llm_required,
                    llm_patience_secs: plan.llm_patience_secs,
                    llm_retry_backoff_secs: plan.llm_retry_backoff_secs,
                    adapter_timeout_secs: plan.adapter_timeout_secs,
                    ablations: ablations.clone(),
                    noise: plan.noise.clone(),
                };
                let result = harness::run(scn, *control, &cfg);
                if consults {
                    last_llm_cell = Some(Instant::now());
                }
                match result {
                    Ok(report) => {
                        outcome.cells_run += 1;
                        state.llm_calls += report.llm_calls;
                        let paused = paused_on_providers(&report);
                        state.cells.insert(
                            key,
                            CellState {
                                status: if paused { "paused" } else { "done" }.into(),
                                summary: report.summary_line(),
                            },
                        );
                        save_state(&state_path, &state)?;
                        if paused {
                            outcome.halt = Halt::Paused;
                            break 'cells;
                        }
                    }
                    Err(e) => {
                        // A failed cell is recorded and the campaign moves on;
                        // `--resume` retries it (it never reads "done").
                        outcome.cells_failed += 1;
                        state.cells.insert(
                            key,
                            CellState {
                                status: "failed".into(),
                                summary: format!("{}: {e}", path.display()),
                            },
                        );
                        save_state(&state_path, &state)?;
                    }
                }
            }
        }
    }

    save_state(&state_path, &state)?;
    Ok(outcome)
}

/// Did this run stop because every provider was unavailable?
fn paused_on_providers(report: &RunReport) -> bool {
    report
        .episodes
        .iter()
        .any(|e| e.llm_outcome == "llm_unavailable")
}

/// The stable identity of a cell inside a campaign.
fn cell_key(s: &Scenario, control: Control, replicate: u32) -> String {
    format!("{}|{}|r{replicate}", s.id, control.letter())
}

fn save_state(path: &Path, state: &CampaignState) -> io::Result<()> {
    fs::write(
        path,
        serde_json::to_string_pretty(state).map_err(io::Error::other)?,
    )
}

/// Fingerprint the plan's canonical JSON (field order fixed by the struct).
fn plan_fingerprint(plan: &CampaignPlan) -> io::Result<u64> {
    Ok(fnv1a(
        serde_json::to_string(plan)
            .map_err(io::Error::other)?
            .as_bytes(),
    ))
}

fn parse_controls(spec: &str) -> io::Result<Vec<Control>> {
    let mut out = Vec::new();
    for c in [
        Control::Baseline,
        Control::LlmOnly,
        Control::NoMemory,
        Control::Full,
    ] {
        if spec
            .to_ascii_uppercase()
            .contains(c.letter().chars().next().unwrap())
        {
            out.push(c);
        }
    }
    if out.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("controls {spec:?} names none of A, B, C, D"),
        ));
    }
    Ok(out)
}

/// Expand files-or-directories into a sorted fixture list.
fn expand_fixtures(roots: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for root in roots {
        if root.is_file() {
            out.push(root.clone());
            continue;
        }
        let mut stack = vec![root.clone()];
        while let Some(d) = stack.pop() {
            for entry in fs::read_dir(&d)?.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|e| e == "json")
                    && !p.to_string_lossy().ends_with(".curriculum.json")
                {
                    out.push(p);
                }
            }
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}
