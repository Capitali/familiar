//! The LLM seam — a boundary-gated consult. *The model is not the factory.*
//!
//! Every consult is an `Llm` action weighed by the obedience guard against the
//! human-owned boundary. Under the default-closed boundary it is **refused** with no
//! side effects (no prompt written, no network, no key read). Only when a human has
//! opened `allow_llm` does it shell out to the human-installed adapter
//! (`<data-dir>/llm/call_llm.sh`), which the factory does not author.

use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use familiar_kernel::boundary;
use familiar_kernel::guard::{self, Action, ActionKind, Decision};

/// Default adapter deadline — comfortably above the adapter's own 90s
/// per-request network timeout, so the adapter times out first when it can.
pub const DEFAULT_ADAPTER_TIMEOUT: Duration = Duration::from_secs(120);

/// The result of a consult attempt.
pub enum Outcome {
    /// The guard refused (boundary closed, or adapter missing/failed). No reach occurred.
    Refused(String),
    /// Every provider is rate-limited right now (adapter exit code 2). Distinct
    /// from `Refused` so callers can wait-and-retry instead of degrading.
    RateLimited(String),
    /// The adapter's raw response (JSON text, per call_llm.sh).
    Response(String),
}

/// Consult the LLM with `prompt`, gated by the boundary on disk.
///
/// Returns `Refused` (with a rationale) when the boundary forbids it or the adapter
/// is absent — never reaching outward in those cases. Returns `Response` with the
/// adapter's raw output otherwise.
pub fn consult(dir: &Path, prompt: &str) -> io::Result<Outcome> {
    consult_with(dir, prompt, DEFAULT_ADAPTER_TIMEOUT)
}

/// [`consult`] with an explicit adapter deadline. A hung adapter must never hang
/// the caller: at the deadline the adapter is killed and the consult is
/// `Refused`. Exit code 2 is the adapter contract for "every provider
/// rate-limited" and maps to [`Outcome::RateLimited`].
pub fn consult_with(dir: &Path, prompt: &str, timeout: Duration) -> io::Result<Outcome> {
    let b = boundary::load(dir)?;
    let verdict = guard::evaluate(&Action::new(ActionKind::Llm, "llm-provider"), &b);
    if verdict.decision != Decision::Allow {
        return Ok(Outcome::Refused(verdict.rationale));
    }

    let llm_dir = dir.join("llm");
    let script = llm_dir.join("call_llm.sh");
    if !script.exists() {
        return Ok(Outcome::Refused(format!(
            "{} not found — install the adapter (see llm/README.md)",
            script.display()
        )));
    }
    fs::create_dir_all(&llm_dir)?;
    fs::write(llm_dir.join("prompt.txt"), prompt)?;
    let mut child = Command::new("sh").arg(&script).spawn()?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(Outcome::Refused(format!(
                "adapter exceeded its {}s deadline and was killed",
                timeout.as_secs()
            )));
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    if status.code() == Some(2) {
        return Ok(Outcome::RateLimited(
            "every provider rate-limited (adapter exit 2)".to_string(),
        ));
    }
    if !status.success() {
        return Ok(Outcome::Refused(format!(
            "adapter exited with status {status}"
        )));
    }
    let resp = fs::read_to_string(llm_dir.join("response.json"))?;
    Ok(Outcome::Response(resp))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn refused_with_no_side_effects_under_closed_boundary() {
        let p =
            std::env::temp_dir().join(format!("familiar_llm_test_closed_{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        let t = Temp(p.clone());
        match consult(&t.0, "hello").unwrap() {
            Outcome::Refused(_) => {}
            Outcome::Response(_) | Outcome::RateLimited(_) => {
                panic!("closed boundary must refuse")
            }
        }
        // no prompt written, no llm dir created beyond what we made
        assert!(!p.join("llm").join("prompt.txt").exists());
    }

    /// Set up a data dir whose boundary allows LLM and whose adapter is `body`.
    fn open_dir_with_adapter(tag: &str, body: &str) -> Temp {
        use familiar_kernel::boundary::{Boundary, BOUNDARY_FILE};
        let p =
            std::env::temp_dir().join(format!("familiar_llm_test_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(p.join("llm")).unwrap();
        let mut b = Boundary::closed();
        b.allow_llm = true;
        fs::write(p.join(BOUNDARY_FILE), serde_json::to_string(&b).unwrap()).unwrap();
        fs::write(p.join("llm").join("call_llm.sh"), body).unwrap();
        Temp(p)
    }

    #[test]
    fn adapter_exit_two_is_rate_limited_not_refused() {
        let t = open_dir_with_adapter("ratelimit", "#!/bin/sh\nexit 2\n");
        match consult(&t.0, "hello").unwrap() {
            Outcome::RateLimited(why) => assert!(why.contains("rate-limited")),
            Outcome::Refused(why) => panic!("exit 2 must be RateLimited, got Refused({why})"),
            Outcome::Response(_) => panic!("exit 2 must not be a response"),
        }
    }

    #[test]
    fn hung_adapter_is_killed_at_the_deadline() {
        let t = open_dir_with_adapter("hang", "#!/bin/sh\nsleep 300\n");
        let started = std::time::Instant::now();
        match consult_with(&t.0, "hello", Duration::from_secs(1)).unwrap() {
            Outcome::Refused(why) => assert!(why.contains("deadline"), "unexpected: {why}"),
            _ => panic!("a hung adapter must end in a timed-out refusal"),
        }
        // Killed near the deadline — not after the adapter's five minutes.
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}
