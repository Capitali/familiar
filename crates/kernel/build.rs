//! Stamp the **release version** into the binary at build time.
//!
//! Self-upgrade needs an orderable identity for the running build so a node can tell whether an
//! incoming release is newer than what it runs. The crate version (`0.1.0`) is static across
//! commits, so it can't order releases. Instead we read the repo-root `VERSION` file — a monotonic
//! integer the human bumps when blessing a release (`familiar release bless`) — and bake it in as
//! `FAMILIAR_BUILD`. The file (not git) is the source of truth so it works on a node that built from
//! an rsync'd tree with no `.git` (the VM), where `git rev-parse` would fail.

use std::path::Path;

fn main() {
    // crates/kernel → repo root is two levels up.
    let version_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../VERSION");
    let contents = std::fs::read_to_string(&version_path).unwrap_or_default();
    // Collapse to a single trimmed line: "<n> <optional label>" — parsed by kernel::version.
    let stamp = contents
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    let stamp = if stamp.is_empty() { "0".to_string() } else { stamp };
    println!("cargo:rustc-env=FAMILIAR_BUILD={stamp}");
    println!("cargo:rerun-if-changed={}", version_path.display());
}
