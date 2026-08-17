//! **The structural guard for T-118**: no fixture root in this workspace may be a fixed name.
//!
//! The per-process fix is easy to apply and easy to forget — the next test someone writes
//! reaches for `std::env::temp_dir().join("my_test")` because that is what the file above it
//! used to do. A collision then costs a day of reading a red bar that has nothing to do with
//! the code under test, and it only appears when two runs overlap, which is exactly when
//! nobody is looking for infrastructure faults.
//!
//! So the rule is enforced against the source itself, the same discipline as ADR-0035's
//! deck-drift test: every `temp_dir()` in every crate must reach a per-process component
//! within the same expression, or come from `familiar_kernel::testing::temp_root`.

use std::path::{Path, PathBuf};

/// How far past a `temp_dir()` occurrence to look for the pid — enough to span a wrapped
/// `format!` across several lines, short enough that the next statement cannot satisfy it.
const WINDOW: usize = 260;

fn workspace_crates() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("crates")
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn every_fixture_root_is_private_to_its_process() {
    let mut files = Vec::new();
    rs_files(&workspace_crates(), &mut files);
    assert!(files.len() > 20, "the walk found the workspace");

    let mut offenders = Vec::new();
    for f in &files {
        // Two files are allowed to say `temp_dir()` without a pid beside it: the helper that
        // owns the naming rule (its own unit test proves the property), and this guard, which
        // has to name the pattern in order to look for it.
        if f.ends_with("kernel/src/testing.rs") || f.ends_with("kernel/tests/temp_roots.rs") {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(f) else {
            continue;
        };
        let mut at = 0;
        while let Some(i) = src[at..].find("temp_dir()") {
            let start = at + i;
            let window = &src[start..src.len().min(start + WINDOW)];
            // Prose about the rule is not a use of it.
            let line_start = src[..start].rfind('\n').map(|n| n + 1).unwrap_or(0);
            let in_comment = src[line_start..start].trim_start().starts_with("//");
            if !in_comment && !window.contains("process::id()") {
                let line = src[..start].matches('\n').count() + 1;
                offenders.push(format!(
                    "{}:{line}",
                    f.strip_prefix(workspace_crates()).unwrap_or(f).display()
                ));
            }
            at = start + "temp_dir()".len();
        }
    }
    assert!(
        offenders.is_empty(),
        "fixture roots without a per-process component — two concurrent runs would share these \
         directories (T-118). Use familiar_kernel::testing::temp_root, or include \
         std::process::id() in the name:\n  {}",
        offenders.join("\n  ")
    );
}
