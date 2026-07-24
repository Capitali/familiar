//! The evidence table — ADR-0010's question, answered from the reports.
//!
//! `collect` walks a directory of `report.json` files (a campaign's `runs/`,
//! or any lab dir), aggregates per scenario × control across replicates, and
//! states the load-bearing comparisons — D−C and D−B — as categorical
//! verdicts. Negative results are results: "D worse" and "no difference" are
//! printed with exactly the same prominence as "D better", and any cell that
//! ran degraded (llm_unavailable episodes, too few replicates) is
//! "insufficient data", never silently included.
//!
//! At n = 3 replicates the honest statistic is min/median/max — no p-values.

use crate::report::RunReport;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

/// One control's aggregate across replicates of one scenario.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ControlStats {
    /// Replicates aggregated.
    pub n: u32,
    /// Fraction of replicates that reached an external pass.
    pub success_rate: f64,
    /// Trials-to-success across succeeding replicates (None = none succeeded).
    pub tts_min: Option<u32>,
    pub tts_median: Option<u32>,
    pub tts_max: Option<u32>,
    /// Best single-episode effectiveness seen.
    pub best_effectiveness: f64,
    /// Mean service across all episodes.
    pub mean_service: f64,
    /// Total boundary violations (this row should be zero, or it screams).
    pub boundary_violations: u32,
    pub repeated_failed_strategies: u32,
    pub llm_calls: u32,
    pub llm_tokens: u64,
    pub total_wall_ms: u128,
    /// Episodes skipped because providers were unavailable (degraded evidence).
    pub llm_unavailable_episodes: u32,
}

/// One scenario's evidence across controls, with the D-vs comparisons.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioEvidence {
    pub family: String,
    pub scenario_id: String,
    pub variant: String,
    pub controls: BTreeMap<String, ControlStats>,
    /// "D better" | "no difference" | "D worse" | "insufficient data" | "no D/C data"
    pub verdict_d_vs_c: String,
    pub verdict_d_vs_b: String,
}

/// The whole table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub scenarios: Vec<ScenarioEvidence>,
}

impl Evidence {
    /// Walk `dir` for `report.json` files and build the table.
    pub fn collect(dir: &Path) -> io::Result<Evidence> {
        let mut reports = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            let Ok(entries) = fs::read_dir(&d) else {
                continue;
            };
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.file_name().is_some_and(|n| n == "report.json") {
                    let body = fs::read_to_string(&p)?;
                    let report: RunReport = serde_json::from_str(&body).map_err(|e| {
                        io::Error::new(io::ErrorKind::InvalidData, format!("{}: {e}", p.display()))
                    })?;
                    reports.push(report);
                }
            }
        }
        Ok(Evidence::from_reports(reports))
    }

    /// Aggregate parsed reports (grouping: family → scenario+variant → control).
    pub fn from_reports(reports: Vec<RunReport>) -> Evidence {
        let mut grouped: BTreeMap<(String, String, String), BTreeMap<String, Vec<RunReport>>> =
            BTreeMap::new();
        for r in reports {
            grouped
                .entry((r.family.clone(), r.scenario_id.clone(), r.variant.clone()))
                .or_default()
                .entry(r.control.clone())
                .or_default()
                .push(r);
        }
        let scenarios = grouped
            .into_iter()
            .map(|((family, scenario_id, variant), by_control)| {
                let controls: BTreeMap<String, ControlStats> = by_control
                    .into_iter()
                    .map(|(control, runs)| (control, aggregate(&runs)))
                    .collect();
                let verdict_d_vs_c = verdict(controls.get("D"), controls.get("C"));
                let verdict_d_vs_b = verdict(controls.get("D"), controls.get("B"));
                ScenarioEvidence {
                    family,
                    scenario_id,
                    variant,
                    controls,
                    verdict_d_vs_c,
                    verdict_d_vs_b,
                }
            })
            .collect();
        Evidence { scenarios }
    }

    /// Save as pretty JSON.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            path,
            serde_json::to_string_pretty(self).map_err(io::Error::other)?,
        )
    }

    /// The terminal table.
    pub fn table(&self) -> String {
        let mut out = String::new();
        for s in &self.scenarios {
            let _ = writeln!(
                out,
                "{} / {}{}",
                s.family,
                s.scenario_id,
                if s.variant.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", s.variant)
                }
            );
            let _ = writeln!(
                out,
                "  ctl  n  success  trials-to-success  best-eff  service  violations  repeats  llm    tokens"
            );
            for (control, c) in &s.controls {
                let _ = writeln!(
                    out,
                    "  {:<4} {:<2} {:<8} {:<18} {:<9.2} {:<8.2} {:<11} {:<8} {:<6} {}",
                    control,
                    c.n,
                    format!("{:.0}%", c.success_rate * 100.0),
                    tts_cell(c),
                    c.best_effectiveness,
                    c.mean_service,
                    c.boundary_violations,
                    c.repeated_failed_strategies,
                    c.llm_calls,
                    c.llm_tokens,
                );
            }
            let _ = writeln!(
                out,
                "  D vs C: {}   D vs B: {}",
                s.verdict_d_vs_c, s.verdict_d_vs_b
            );
            let _ = writeln!(out);
        }
        out
    }

    /// The markdown report (checked in as the experiment's evidence).
    pub fn markdown(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "# Scenario laboratory evidence\n");
        let _ = writeln!(
            out,
            "Aggregated per scenario × control across replicates. Trials-to-success is \
             min/median/max over succeeding replicates; \"never\" means no replicate \
             passed. Verdicts compare D against the memoryless controls; \"insufficient \
             data\" marks any cell degraded by unavailable providers or n < 2 — degraded \
             evidence is named, never blended in.\n"
        );
        for s in &self.scenarios {
            let _ = writeln!(
                out,
                "## {} / {}{}\n",
                s.family,
                s.scenario_id,
                if s.variant.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", s.variant)
                }
            );
            let _ = writeln!(
                out,
                "| control | n | success | trials-to-success | best eff | service | violations | repeated strategies | llm calls | tokens |"
            );
            let _ = writeln!(out, "|---|---|---|---|---|---|---|---|---|---|");
            for (control, c) in &s.controls {
                let _ = writeln!(
                    out,
                    "| {} | {} | {:.0}% | {} | {:.2} | {:.2} | {} | {} | {} | {} |",
                    control,
                    c.n,
                    c.success_rate * 100.0,
                    tts_cell(c),
                    c.best_effectiveness,
                    c.mean_service,
                    c.boundary_violations,
                    c.repeated_failed_strategies,
                    c.llm_calls,
                    c.llm_tokens,
                );
            }
            let _ = writeln!(
                out,
                "\n**D vs C:** {} · **D vs B:** {}\n",
                s.verdict_d_vs_c, s.verdict_d_vs_b
            );
        }
        out
    }
}

fn tts_cell(c: &ControlStats) -> String {
    match (c.tts_min, c.tts_median, c.tts_max) {
        (Some(min), Some(med), Some(max)) => format!("{min}/{med}/{max}"),
        _ => "never".to_string(),
    }
}

/// Aggregate one control's replicates.
fn aggregate(runs: &[RunReport]) -> ControlStats {
    let mut c = ControlStats {
        n: runs.len() as u32,
        ..ControlStats::default()
    };
    let mut tts: Vec<u32> = Vec::new();
    let mut service_sum = 0.0;
    let mut episode_count = 0u32;
    for r in runs {
        if let Some(t) = r.trials_to_success {
            tts.push(t);
        }
        c.boundary_violations += r.boundary_violations;
        c.repeated_failed_strategies += r.repeated_failed_strategies;
        c.llm_calls += r.llm_calls;
        c.llm_tokens += r.llm_tokens;
        c.total_wall_ms += r.total_wall_ms;
        for e in &r.episodes {
            episode_count += 1;
            service_sum += e.service;
            c.best_effectiveness = c.best_effectiveness.max(e.effectiveness);
            if e.llm_outcome == "llm_unavailable" {
                c.llm_unavailable_episodes += 1;
            }
        }
    }
    c.success_rate = if runs.is_empty() {
        0.0
    } else {
        tts.len() as f64 / runs.len() as f64
    };
    tts.sort_unstable();
    c.tts_min = tts.first().copied();
    c.tts_median = (!tts.is_empty()).then(|| tts[(tts.len() - 1) / 2]);
    c.tts_max = tts.last().copied();
    if episode_count > 0 {
        c.mean_service = service_sum / f64::from(episode_count);
    }
    c
}

/// The categorical comparison. Success rate decides; trials-to-success breaks
/// ties among succeeding pairs. Degraded or thin cells are named as such.
fn verdict(d: Option<&ControlStats>, other: Option<&ControlStats>) -> String {
    let (Some(d), Some(o)) = (d, other) else {
        return "no data".to_string();
    };
    if d.llm_unavailable_episodes > 0 || o.llm_unavailable_episodes > 0 || d.n < 2 || o.n < 2 {
        return "insufficient data".to_string();
    }
    let by_rate = d
        .success_rate
        .partial_cmp(&o.success_rate)
        .unwrap_or(std::cmp::Ordering::Equal);
    match by_rate {
        std::cmp::Ordering::Greater => "D better".to_string(),
        std::cmp::Ordering::Less => "D worse".to_string(),
        std::cmp::Ordering::Equal => match (d.tts_median, o.tts_median) {
            (Some(dm), Some(om)) if dm < om => "D better".to_string(),
            (Some(dm), Some(om)) if dm > om => "D worse".to_string(),
            _ => "no difference".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{EpisodeRecord, RunReport};

    fn run_with(
        control: &str,
        replicate: u32,
        tts: Option<u32>,
        episodes: u32,
        outcome: &str,
    ) -> RunReport {
        let eps: Vec<EpisodeRecord> = (1..=episodes)
            .map(|i| EpisodeRecord {
                episode: i,
                candidate_id: format!("candidate-{i:04}"),
                generation: 0,
                changed_traits: String::new(),
                llm_used: outcome == "answered",
                boundary_ok: true,
                exec_ok: true,
                effectiveness: if Some(i) == tts { 1.0 } else { 0.2 },
                service: 1.0,
                cost: 0.1,
                result: if Some(i) == tts { "pass" } else { "fail" }.into(),
                failure_class: String::new(),
                decision: String::new(),
                violations: vec![],
                wall_ms: 5,
                llm_outcome: outcome.into(),
                llm_tokens: 100,
            })
            .collect();
        RunReport::from_episodes("scn", "fam", "v", control, replicate, eps)
    }

    #[test]
    fn d_better_when_it_succeeds_faster() {
        let ev = Evidence::from_reports(vec![
            run_with("D", 1, Some(2), 2, "answered"),
            run_with("D", 2, Some(3), 3, "answered"),
            run_with("C", 1, Some(5), 5, "answered"),
            run_with("C", 2, Some(5), 5, "answered"),
        ]);
        assert_eq!(ev.scenarios[0].verdict_d_vs_c, "D better");
    }

    #[test]
    fn no_difference_and_d_worse_are_stated_plainly() {
        let ev = Evidence::from_reports(vec![
            run_with("D", 1, Some(3), 3, "answered"),
            run_with("D", 2, Some(3), 3, "answered"),
            run_with("C", 1, Some(3), 3, "answered"),
            run_with("C", 2, Some(3), 3, "answered"),
        ]);
        assert_eq!(ev.scenarios[0].verdict_d_vs_c, "no difference");

        let ev = Evidence::from_reports(vec![
            run_with("D", 1, None, 5, "answered"),
            run_with("D", 2, None, 5, "answered"),
            run_with("B", 1, Some(2), 2, "answered"),
            run_with("B", 2, Some(4), 4, "answered"),
        ]);
        assert_eq!(ev.scenarios[0].verdict_d_vs_b, "D worse");
    }

    #[test]
    fn degraded_or_thin_cells_are_insufficient() {
        // An llm_unavailable episode anywhere poisons the comparison.
        let mut degraded = run_with("C", 1, None, 1, "llm_unavailable");
        degraded.episodes[0].result = "skipped".into();
        let ev = Evidence::from_reports(vec![
            run_with("D", 1, Some(1), 1, "answered"),
            run_with("D", 2, Some(1), 1, "answered"),
            degraded,
            run_with("C", 2, Some(1), 1, "answered"),
        ]);
        assert_eq!(ev.scenarios[0].verdict_d_vs_c, "insufficient data");

        // n = 1 is too thin to compare.
        let ev = Evidence::from_reports(vec![
            run_with("D", 1, Some(1), 1, "answered"),
            run_with("C", 1, Some(2), 2, "answered"),
        ]);
        assert_eq!(ev.scenarios[0].verdict_d_vs_c, "insufficient data");
    }

    #[test]
    fn aggregate_sums_and_medians() {
        let ev = Evidence::from_reports(vec![
            run_with("D", 1, Some(1), 1, "answered"),
            run_with("D", 2, Some(3), 3, "answered"),
            run_with("D", 3, Some(5), 5, "answered"),
        ]);
        let d = &ev.scenarios[0].controls["D"];
        assert_eq!(d.n, 3);
        assert_eq!(
            (d.tts_min, d.tts_median, d.tts_max),
            (Some(1), Some(3), Some(5))
        );
        assert!((d.success_rate - 1.0).abs() < f64::EPSILON);
        assert_eq!(d.llm_tokens, 900); // 9 episodes x 100
    }
}
