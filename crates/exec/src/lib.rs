//! A small, dependency-free, resource-limited script runner.
//!
//! Runs a shell script under a CPU limit (`ulimit -t`) and a wall-clock timeout
//! (enforced in-process by polling and killing), capturing capped output and
//! measuring cost. Pure `std` — no `unsafe`, no crates. Executing generated code is a
//! Law III matter; this runner is only ever invoked behind the `allow_execute`
//! boundary gate (see the obedience guard), and it bounds what a runaway can do.

use std::io;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

/// Resource limits for a run.
#[derive(Debug, Clone)]
pub struct Limits {
    /// CPU seconds (`ulimit -t`).
    pub cpu_secs: u64,
    /// Wall-clock seconds before the run is killed.
    pub wall_secs: u64,
    /// Max bytes of output retained (and used for the cost measure).
    pub output_cap: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            cpu_secs: 5,
            wall_secs: 10,
            output_cap: 8192,
        }
    }
}

impl Limits {
    /// Limits for an **unsandboxed** run (the human set `sandbox_execution=false`): no CPU
    /// cap, a large output cap, and only a generous wall-clock *liveness* timeout so a
    /// hung script can't freeze the metabolism. Capability is bound by the constitution
    /// (the pre-execution review), not by these.
    pub fn unsandboxed() -> Self {
        Limits {
            cpu_secs: 0,
            wall_secs: 300,
            output_cap: 65_536,
        }
    }

    /// Limits for running an **authored tool** to answer a request. Wider than the tick's
    /// candidate-scoring `default()` (5s/10s) because a real tool does real work — sampling CPU
    /// over a few seconds, an nmap ping sweep — that the tight budget could only ever time out
    /// (which retired the whole authored library). Still firmly bounded so a tool can't run away.
    pub fn tool_run() -> Self {
        Limits {
            cpu_secs: 30,
            wall_secs: 60,
            output_cap: 16_384,
        }
    }

    /// Limits for a **delegated agent step** — still sandboxed, but generous enough for a real
    /// task to finish (a network scan, a build probe) where the tick's tight candidate-scoring
    /// budget (5s/10s) would only ever time out. Bounded so a step still can't run away.
    pub fn agent_task() -> Self {
        Limits {
            cpu_secs: 45,
            wall_secs: 90,
            output_cap: 32_768,
        }
    }
}

/// The measured outcome of a run.
#[derive(Debug, Clone, PartialEq)]
pub struct RunResult {
    /// The script exited 0.
    pub exit_ok: bool,
    /// The wall-clock timeout was hit and the process was killed.
    pub timed_out: bool,
    /// Wall time in milliseconds.
    pub wall_ms: u128,
    /// Bytes of output produced (capped at `output_cap`).
    pub output_bytes: usize,
    /// Captured output (stdout+stderr), truncated to `output_cap`.
    pub output: String,
}

/// Run `script_path` under `limits`. Combined stdout/stderr go to a sibling `.out`
/// file, which is read back (capped). Wall timeout is enforced by polling `try_wait`
/// and killing on overrun.
pub fn run_script(script_path: &Path, limits: &Limits, workdir: &Path) -> io::Result<RunResult> {
    let out_path = script_path.with_extension("out");
    // Run with `workdir` as the working directory — the familiar's designated workspace —
    // so scripts that "write files under the current directory" default there, not into
    // the repo or wherever the daemon happened to start.
    let _ = std::fs::create_dir_all(workdir);
    // `cpu_secs == 0` means *no* CPU cap — the unsandboxed path. The wall-clock timeout
    // below still applies as a liveness bound so a hung script can't freeze the metabolism
    // (Law I: the familiar must keep serving), even when resource confinement is off.
    let ulimit = if limits.cpu_secs > 0 {
        format!("ulimit -t {}; ", limits.cpu_secs)
    } else {
        String::new()
    };
    let cmd = format!(
        "{ulimit}cd '{wd}' && sh '{script}' > '{out}' 2>&1",
        wd = workdir.display(),
        script = script_path.display(),
        out = out_path.display(),
    );

    let start = Instant::now();
    let mut child = Command::new("sh").arg("-c").arg(&cmd).spawn()?;

    let mut timed_out = false;
    let mut exit_ok = false;
    loop {
        match child.try_wait()? {
            Some(status) => {
                exit_ok = status.success();
                break;
            }
            None => {
                if start.elapsed() >= Duration::from_secs(limits.wall_secs) {
                    let _ = child.kill();
                    let _ = child.wait();
                    timed_out = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
    let wall_ms = start.elapsed().as_millis();

    let raw = std::fs::read_to_string(&out_path).unwrap_or_default();
    let _ = std::fs::remove_file(&out_path);
    let output: String = raw.chars().take(limits.output_cap).collect();
    let output_bytes = output.len();

    Ok(RunResult {
        exit_ok,
        timed_out,
        wall_ms,
        output_bytes,
        output,
    })
}

/// Measured cost in [0,1]: a blend of wall time and output size against the limits;
/// a run that hit the wall is maximally costly. This is what folds into the trial's
/// complexity (Soul Rule 9 → Law I: a cheaper artifact outranks an expensive equal).
pub fn cost(r: &RunResult, limits: &Limits) -> f64 {
    if r.timed_out {
        return 1.0;
    }
    let wall = r.wall_ms as f64 / (limits.wall_secs.max(1) as f64 * 1000.0);
    let out = r.output_bytes as f64 / limits.output_cap.max(1) as f64;
    (0.5 * wall + 0.5 * out).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("familiar_exec_test_{tag}"));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn runs_a_clean_script_cheaply() {
        let d = tmp("clean");
        let s = d.join("a.sh");
        fs::write(&s, "echo hello from the familiar\n").unwrap();
        let r = run_script(&s, &Limits::default(), &d).unwrap();
        assert!(r.exit_ok && !r.timed_out);
        assert!(r.output.contains("hello from the familiar"));
        assert!(cost(&r, &Limits::default()) < 0.5);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn nonzero_exit_is_recorded() {
        let d = tmp("fail");
        let s = d.join("b.sh");
        fs::write(&s, "exit 3\n").unwrap();
        let r = run_script(&s, &Limits::default(), &d).unwrap();
        assert!(!r.exit_ok && !r.timed_out);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn unsandboxed_runs_without_a_cpu_cap() {
        // no ulimit prefix (cpu_secs == 0), but still captured + measured + liveness-bounded
        let d = tmp("unsandboxed");
        let s = d.join("u.sh");
        fs::write(&s, "echo ran unsandboxed\n").unwrap();
        let r = run_script(&s, &Limits::unsandboxed(), &d).unwrap();
        assert!(r.exit_ok && !r.timed_out);
        assert!(r.output.contains("ran unsandboxed"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn tool_run_budget_lets_a_multi_second_tool_finish() {
        // A real authored tool (CPU sampling, a bounded scan) runs for several seconds — the
        // tick's default() 5s-CPU/10s-wall budget could only ever time it out (which retired
        // the whole authored library). tool_run() is wide enough that such work completes.
        let d = tmp("toolrun");
        let s = d.join("t.sh");
        fs::write(&s, "sleep 2; echo sampled\n").unwrap();
        let r = run_script(&s, &Limits::tool_run(), &d).unwrap();
        assert!(
            r.exit_ok && !r.timed_out,
            "a 2s tool must finish under tool_run"
        );
        assert!(r.output.contains("sampled"));
        // …and the budget is genuinely wider than the tick's candidate-scoring default.
        assert!(Limits::tool_run().wall_secs > Limits::default().wall_secs);
        assert!(Limits::tool_run().cpu_secs > Limits::default().cpu_secs);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn wall_timeout_is_enforced_and_maximally_costly() {
        let d = tmp("timeout");
        let s = d.join("c.sh");
        fs::write(&s, "sleep 3\n").unwrap();
        let limits = Limits {
            cpu_secs: 5,
            wall_secs: 1,
            output_cap: 8192,
        };
        let r = run_script(&s, &limits, &d).unwrap();
        assert!(r.timed_out && !r.exit_ok);
        assert_eq!(cost(&r, &limits), 1.0);
        let _ = fs::remove_dir_all(&d);
    }
}
