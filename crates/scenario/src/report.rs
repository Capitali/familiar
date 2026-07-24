//! The run report — the experiment's evidence, one record per episode.
//!
//! Metrics follow ADR-0010's tracking list: trials-to-success, boundary
//! violations, repeated failed strategies, cost, LLM usage. Negative results are
//! results — the report states what happened, never what was hoped.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

/// One episode's externally-assigned outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeRecord {
    pub episode: u32,
    pub candidate_id: String,
    pub generation: i32,
    /// The traits this generation changed (the "strategy" fingerprint).
    pub changed_traits: String,
    /// Whether an LLM authored the artifact (false = deterministic template).
    pub llm_used: bool,
    pub boundary_ok: bool,
    pub exec_ok: bool,
    pub effectiveness: f64,
    pub service: f64,
    pub cost: f64,
    /// "pass" | "partial" | "fail" — assigned by the external evaluator.
    pub result: String,
    pub failure_class: String,
    /// promote | mutate | archive | reject | observe_more.
    pub decision: String,
    pub violations: Vec<String>,
    pub wall_ms: u128,
    /// How the artifact came to be: "answered" | "template" | "failed" |
    /// "rate_limited" | "unused" | "llm_unavailable" (episode skipped).
    #[serde(default)]
    pub llm_outcome: String,
    /// Tokens the adapter's spend ledger attributes to this episode (0 = unknown).
    #[serde(default)]
    pub llm_tokens: u64,
}

/// A whole run: one scenario × one control × N episodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunReport {
    pub scenario_id: String,
    pub family: String,
    /// The fixture's variant phrase — evidence tables group by it.
    #[serde(default)]
    pub variant: String,
    pub control: String,
    /// Which replicate produced this report (1-based).
    #[serde(default = "default_replicate")]
    pub replicate: u32,
    pub episodes: Vec<EpisodeRecord>,
    /// 1-based episode of the first external pass; None = never succeeded.
    pub trials_to_success: Option<u32>,
    pub boundary_violations: u32,
    pub llm_calls: u32,
    /// Failed episodes whose (failure_class, changed_traits) strategy had already
    /// failed in an earlier episode — the "repeats failed strategies" measure.
    pub repeated_failed_strategies: u32,
    pub total_wall_ms: u128,
    /// Adapter-ledger tokens attributed to this run (0 = adapter kept no ledger).
    #[serde(default)]
    pub llm_tokens: u64,
}

fn default_replicate() -> u32 {
    1
}

impl RunReport {
    /// Derive the summary metrics from the episode records.
    pub fn from_episodes(
        scenario_id: &str,
        family: &str,
        variant: &str,
        control: &str,
        replicate: u32,
        episodes: Vec<EpisodeRecord>,
    ) -> RunReport {
        let trials_to_success = episodes
            .iter()
            .find(|e| e.result == "pass")
            .map(|e| e.episode);
        let boundary_violations = episodes.iter().filter(|e| !e.boundary_ok).count() as u32;
        let llm_calls = episodes.iter().filter(|e| e.llm_used).count() as u32;
        let mut seen_failures: HashSet<(String, String)> = HashSet::new();
        let mut repeated = 0;
        for e in &episodes {
            // Skipped episodes (llm_unavailable) were never trials — they are
            // not strategies, so they cannot be repeated ones.
            if e.result == "pass" || e.result == "skipped" {
                continue;
            }
            let key = (e.failure_class.clone(), e.changed_traits.clone());
            if !seen_failures.insert(key) {
                repeated += 1;
            }
        }
        let total_wall_ms = episodes.iter().map(|e| e.wall_ms).sum();
        let llm_tokens = episodes.iter().map(|e| e.llm_tokens).sum();
        RunReport {
            scenario_id: scenario_id.to_string(),
            family: family.to_string(),
            variant: variant.to_string(),
            control: control.to_string(),
            replicate,
            episodes,
            trials_to_success,
            boundary_violations,
            llm_calls,
            repeated_failed_strategies: repeated,
            total_wall_ms,
            llm_tokens,
        }
    }

    /// Persist as pretty JSON.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            path,
            serde_json::to_string_pretty(self).map_err(io::Error::other)?,
        )
    }

    /// A terminal-friendly summary line.
    pub fn summary_line(&self) -> String {
        let tts = self
            .trials_to_success
            .map(|n| n.to_string())
            .unwrap_or_else(|| "—".to_string());
        format!(
            "{:<28} control {:<2} episodes {:<3} first-pass {:<3} violations {:<3} repeats {:<3} llm {:<3} wall {} ms",
            self.scenario_id,
            self.control,
            self.episodes.len(),
            tts,
            self.boundary_violations,
            self.repeated_failed_strategies,
            self.llm_calls,
            self.total_wall_ms
        )
    }

    /// A per-episode table for the terminal.
    pub fn table(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "  ep  candidate        gen  result   eff   svc   cost  decision      failure"
        );
        for e in &self.episodes {
            let _ = writeln!(
                out,
                "  {:<3} {:<16} {:<4} {:<8} {:<5.2} {:<5.2} {:<5.2} {:<13} {}",
                e.episode,
                e.candidate_id,
                e.generation,
                e.result,
                e.effectiveness,
                e.service,
                e.cost,
                e.decision,
                if e.boundary_ok {
                    e.failure_class.clone()
                } else {
                    format!("{} ({})", e.failure_class, e.violations.join("; "))
                }
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(
        episode: u32,
        result: &str,
        failure: &str,
        traits: &str,
        boundary: bool,
    ) -> EpisodeRecord {
        EpisodeRecord {
            episode,
            candidate_id: format!("candidate-{episode:04}"),
            generation: 0,
            changed_traits: traits.into(),
            llm_used: false,
            boundary_ok: boundary,
            exec_ok: true,
            effectiveness: if result == "pass" { 1.0 } else { 0.2 },
            service: 1.0,
            cost: 0.1,
            result: result.into(),
            failure_class: failure.into(),
            decision: "mutate".into(),
            violations: vec![],
            wall_ms: 10,
            llm_outcome: String::new(),
            llm_tokens: 0,
        }
    }

    #[test]
    fn metrics_derive_from_episodes() {
        let r = RunReport::from_episodes(
            "s",
            "f",
            "v",
            "C",
            1,
            vec![
                ep(1, "fail", "off_target", "a", true),
                ep(2, "fail", "off_target", "a", true), // repeated strategy
                ep(3, "fail", "off_target", "b", false),
                ep(4, "pass", "", "c", true),
            ],
        );
        assert_eq!(r.trials_to_success, Some(4));
        assert_eq!(r.repeated_failed_strategies, 1);
        assert_eq!(r.boundary_violations, 1);
        assert_eq!(r.llm_calls, 0);
    }
}
