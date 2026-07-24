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

use familiar_kernel::boundary;
use familiar_kernel::guard::{self, Action, ActionKind, Decision};

/// The result of a consult attempt.
pub enum Outcome {
    /// The guard refused (boundary closed, or adapter missing/failed). No reach occurred.
    Refused(String),
    /// The adapter's raw response (JSON text, per call_llm.sh).
    Response(String),
}

/// Consult the LLM with `prompt`, gated by the boundary on disk.
///
/// Returns `Refused` (with a rationale) when the boundary forbids it or the adapter
/// is absent — never reaching outward in those cases. Returns `Response` with the
/// adapter's raw output otherwise.
pub fn consult(dir: &Path, prompt: &str) -> io::Result<Outcome> {
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
    let status = Command::new("sh").arg(&script).status()?;
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
            Outcome::Response(_) => panic!("closed boundary must refuse"),
        }
        // no prompt written, no llm dir created beyond what we made
        assert!(!p.join("llm").join("prompt.txt").exists());
    }
}
