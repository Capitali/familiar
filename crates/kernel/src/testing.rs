//! **Test fixture roots that cannot collide** (T-118).
//!
//! Observed 2026-08-14 while barring T-103: a full green-bar run overlapped another session's
//! run and `cycle`'s parameter-revert test watched its fixture revert a second time. The test
//! passed alone and a clean rerun passed once the other job finished — so the failure was
//! never in the code under test. Two processes were writing the same directory, because a
//! fixture root named `substrate_obs_test_x` is the same path in every process on the machine
//! and this repo runs several worktrees at once by design (coordination README rule 7).
//!
//! A test that fails for a reason outside its own subject is worse than no test: it spends the
//! one thing a suite has, which is the belief that a red bar means something. So the root
//! carries the process id, and [`temp_root`] is the single place that decides how.
//!
//! Pinned two ways: [`tests::two_processes_never_share_a_fixture_root`] proves the property
//! against a *real* other process's id, and `tests/temp_roots.rs` walks every source file in
//! the workspace and fails on any fixture root that lacks a per-process component — so the
//! next fixed name is caught when it is written rather than the next time two runs overlap.

use std::path::PathBuf;

/// A fixture directory private to this process. Created if absent, emptied if a previous run
/// of *this* pid left something behind.
///
/// Tags name the fixture, not the process — `temp_root("caps_closed")`, not
/// `temp_root("caps_closed_1234")`.
pub fn temp_root(tag: &str) -> PathBuf {
    let p = root_for(tag, std::process::id());
    let _ = std::fs::remove_dir_all(&p);
    let _ = std::fs::create_dir_all(&p);
    p
}

/// The naming rule, as a pure function of its inputs so the isolation property can be tested
/// without spawning anything.
fn root_for(tag: &str, pid: u32) -> PathBuf {
    std::env::temp_dir().join(format!("familiar_test_{tag}_{pid}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// The property, against a real second process rather than an invented number: whatever
    /// pid another process has, its fixture root for the same tag is a different directory.
    #[test]
    fn two_processes_never_share_a_fixture_root() {
        let out = Command::new("/bin/sh")
            .args(["-c", "echo $$"])
            .output()
            .expect("a shell to borrow a pid from");
        let other: u32 = String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse()
            .expect("a pid");
        assert_ne!(other, std::process::id(), "a live child has its own pid");
        assert_ne!(
            root_for("same_tag", other),
            root_for("same_tag", std::process::id()),
            "the same fixture name in two processes must be two directories"
        );
        // And the tag still names the fixture, so a failure is legible in `ls /tmp`.
        assert!(root_for("same_tag", other)
            .to_string_lossy()
            .contains("same_tag"));
    }

    /// The root exists, is empty, and is reusable within one process — a test that calls it
    /// twice gets a clean directory both times rather than yesterday's leftovers.
    #[test]
    fn a_fixture_root_arrives_clean() {
        let d = temp_root("clean_check");
        std::fs::write(d.join("stale"), b"x").unwrap();
        let again = temp_root("clean_check");
        assert_eq!(d, again);
        assert!(!again.join("stale").exists(), "the root arrives empty");
        let _ = std::fs::remove_dir_all(&again);
    }
}
