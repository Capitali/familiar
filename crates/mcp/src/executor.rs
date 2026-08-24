//! The surface executor seam (rungs 4-5).
//!
//! The `/mcp` door lives in `crates/mesh`; the guarded actuator executor (the shell-running,
//! revert-mapped, `allow_actuate`-gated path) lives in `crates/cycle`. Neither the door nor
//! this crate may depend on cycle — cycle depends on us. So the daemon, the one process that
//! sees both, registers a `SurfaceExecutor` here at startup, and the door reads it.
//!
//! The default is refusal: with no executor registered, every observe/invoke fails closed
//! with "execution not available at this door." This is what keeps the network door unable to
//! actuate by construction until a human-run daemon deliberately wires it — the same posture
//! every other outward capability holds. Registration is not a gate; the boundary
//! (`allow_actuate`) and a live human grant remain the gates on top of a wired executor.

use std::path::Path;
use std::sync::OnceLock;

/// The raw, already-guarded surface operations the door delegates to. An implementation is
/// expected to run through the SAME local-actuation path a human `familiar actuate` takes —
/// `allow_actuate`, the declared act, the revert map — so the door adds authority checks
/// (an active human grant, bounds) on top of a primitive that is itself fully guarded.
pub trait SurfaceExecutor: Send + Sync {
    /// Read a declared surface's current state and return its CONCRETE classified bucket
    /// (the local act label of the state it is in) — never raw device output, which could
    /// carry household specifics. The caller maps that concrete bucket to the class's
    /// abstract state before anything reaches a partner. `Err` is why it could not be read.
    fn observe(&self, dir: &Path, surface: &str) -> Result<String, String>;

    /// Run one declared act (`label`) on a declared surface, through the guarded local path
    /// (`execute_tool` enforces `allow_actuate`), attributing nothing to a human. `Ok(())`
    /// means it ran; `Err` is why it did not (shut gate, missing act, tool failure). The
    /// executor NEVER decides authority — that is done before it is called — and it returns no
    /// device detail, so nothing surface-specific can ride back to a partner through it.
    fn invoke(&self, dir: &Path, surface: &str, label: &str) -> Result<(), String>;
}

static EXECUTOR: OnceLock<&'static dyn SurfaceExecutor> = OnceLock::new();

/// Register the process-wide executor. Called once by the daemon at startup; a second call is
/// ignored (the first wiring wins). Production only — tests pass an executor directly to
/// [`crate::grant::observe`] / [`crate::grant::invoke`].
pub fn register(executor: &'static dyn SurfaceExecutor) {
    let _ = EXECUTOR.set(executor);
}

/// The registered executor, or `None` when the door is unwired (the fail-closed default).
pub fn current() -> Option<&'static dyn SurfaceExecutor> {
    EXECUTOR.get().copied()
}
