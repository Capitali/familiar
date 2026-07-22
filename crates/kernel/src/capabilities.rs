//! Capabilities — what a node can actually *do*, so the mesh can route work to the right peer.
//!
//! A goal declares the capabilities it `needs`; a node advertises the capabilities it *has* in its
//! brief; the node whose capabilities satisfy a goal claims it. A capability is the intersection of
//! two things: **discovered tools** (is `cargo` / `xcodebuild` / `swift` on this host?) and **boundary
//! gates** (may this node execute / run an agent at all?). Discovery is perception — always allowed;
//! it never *reaches*, it only reads what is installed. Whether the node may then act on that is the
//! boundary's call, so a capability like `execute` only appears when `allow_execute` is open.
//!
//! Names are short, stable, lowercase slugs so they read the same across the mesh. High-consequence
//! *doing* (installing to a device) is namespaced `deploy-*` and stays human-gated at the goal layer
//! ([`crate::goal::GATED_CAPABILITY_PREFIX`]) — advertising it means "I *can*", not "I may unattended".

use crate::boundary::Boundary;
use std::path::Path;
use std::sync::OnceLock;

/// Is `bin` an executable on `PATH`? A read-only perception (spawns nothing) — walks `$PATH` and
/// checks for a file. Cached per process: the toolchain doesn't change under a running daemon.
fn has_bin(bin: &str) -> bool {
    let Ok(path) = std::env::var("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        let p = dir.join(bin);
        p.is_file() || std::fs::metadata(&p).map(|m| m.is_file()).unwrap_or(false)
    })
}

/// The host toolchain, discovered once. Kept separate from the gate-derived capabilities because it
/// is stable for the process lifetime, whereas the boundary can change tick to tick.
fn discovered() -> &'static Vec<String> {
    static CACHE: OnceLock<Vec<String>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut caps = Vec::new();
        // Rust toolchain — any gossip peer with this can build + test the core.
        if has_bin("cargo") {
            caps.push("build-rust".to_string());
            caps.push("run-tests".to_string());
        }
        // Apple toolchain — only a macOS node with Xcode can build the Apple family, and only it can
        // sign + install to a device. `build-apple` runs autonomously; `deploy-apple` is human-gated.
        if has_bin("xcodebuild") {
            caps.push("build-apple".to_string());
            caps.push("deploy-apple".to_string());
        }
        if has_bin("swift") && !caps.iter().any(|c| c == "build-apple") {
            // Swift without full Xcode: can compile SwiftPM, but not build/sign a device app.
            caps.push("build-swift".to_string());
        }
        // A local model server (Ollama and the like) — a peripheral AI provider on this host.
        if has_bin("ollama") {
            caps.push("has-local-llm".to_string());
        }
        caps
    })
}

/// The capabilities this node advertises right now: discovered toolchain ∩ what the boundary permits.
/// A node that can't execute advertises none of the *doing* capabilities even if the tools are
/// present (availability is not authorization). `dir` is unused today but kept so a future capability
/// can read node-local config without a signature change.
pub fn detect(_dir: &Path, b: &Boundary) -> Vec<String> {
    let mut caps: Vec<String> = Vec::new();
    // Gate-derived: the reach the human has opened.
    if b.allow_execute {
        caps.push("execute".to_string());
    }
    if b.allow_agent {
        caps.push("agent".to_string());
    }
    if b.allow_llm {
        caps.push("llm".to_string());
    }
    // Toolchain-derived, but only when the node may actually run things — a build capability is
    // meaningless on a node whose execute gate is shut.
    if b.allow_execute {
        for c in discovered() {
            caps.push(c.clone());
        }
    }
    caps.sort();
    caps.dedup();
    caps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_closed_boundary_advertises_nothing() {
        let dir = std::env::temp_dir();
        let caps = detect(&dir, &Boundary::closed());
        assert!(
            caps.is_empty(),
            "no reach opened ⇒ no capabilities, whatever tools exist"
        );
    }

    #[test]
    fn gates_surface_as_capabilities() {
        let dir = std::env::temp_dir();
        let mut b = Boundary::closed();
        b.allow_execute = true;
        b.allow_agent = true;
        b.allow_llm = true;
        let caps = detect(&dir, &b);
        assert!(caps.contains(&"execute".to_string()));
        assert!(caps.contains(&"agent".to_string()));
        assert!(caps.contains(&"llm".to_string()));
        // sorted + deduped for a stable, mesh-comparable advertisement
        let mut sorted = caps.clone();
        sorted.sort();
        assert_eq!(caps, sorted);
    }
}
