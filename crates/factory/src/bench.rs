//! The bench oracle: run a candidate's own self-tests inside the containment
//! jail, offline, with no radio and no household. Red here never reaches the
//! device. A candidate passes the bench rung iff every self-test exits 0
//! within its bounds.

use std::path::{Path, PathBuf};

use familiar_jail::{run_jailed, ContainmentProfile, JailOutcome};
use familiar_workshop::manifest::digest_bytes;

/// One self-test's result.
#[derive(Debug, Clone)]
pub struct TestResult {
    pub path: String,
    pub outcome: JailOutcome,
    pub output: String,
}

/// The bench rung's verdict for a candidate.
#[derive(Debug, Clone)]
pub struct BenchReport {
    /// True iff every self-test exited 0 within bounds.
    pub passed: bool,
    pub results: Vec<TestResult>,
    /// sha256 over the concatenated (path, outcome, output) of every test —
    /// the evidence digest the ledger's RungVerdict cites.
    pub evidence_digest: String,
}

/// Run each self-test with `interpreter` inside the jail. `candidate_dir` is
/// the materialized candidate tree (readable); `scratch` is the one writable
/// dir; `extra_hidden` names household roots to hide beyond the mandatory set
/// (e.g. the actual repo and data dir this daemon uses).
pub fn run_bench(
    candidate_dir: &Path,
    self_tests: &[String],
    interpreter: &Path,
    scratch: &Path,
    extra_hidden: &[PathBuf],
    wall_secs: u64,
) -> std::io::Result<BenchReport> {
    let _ = std::fs::create_dir_all(scratch);
    let mut results = Vec::new();
    let mut evidence = Vec::new();

    for rel in self_tests {
        let test_path = candidate_dir.join(rel);
        let mut profile = ContainmentProfile::minimal(
            candidate_dir.to_path_buf(),
            scratch.to_path_buf(),
            wall_secs,
        );
        profile.extra_hidden_roots = extra_hidden.to_vec();
        let argv = vec![
            interpreter.to_string_lossy().into_owned(),
            test_path.to_string_lossy().into_owned(),
        ];
        let run = run_jailed(&profile, &argv)?;
        evidence.extend_from_slice(rel.as_bytes());
        evidence.push(0);
        evidence.extend_from_slice(format!("{:?}", run.outcome).as_bytes());
        evidence.push(0);
        evidence.extend_from_slice(run.output.as_bytes());
        evidence.push(0);
        results.push(TestResult {
            path: rel.clone(),
            outcome: run.outcome,
            output: run.output,
        });
    }

    let passed = !results.is_empty() && results.iter().all(|r| r.outcome == JailOutcome::Ok);
    Ok(BenchReport {
        passed,
        results,
        evidence_digest: digest_bytes(&evidence),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use familiar_workshop::manifest::{digest_bytes, FileEntry, FileRole, Manifest};

    fn python() -> Option<PathBuf> {
        for p in [
            "/opt/homebrew/bin/python3.13",
            "/opt/homebrew/bin/python3",
            "/usr/bin/python3",
        ] {
            if Path::new(p).exists() {
                return Some(PathBuf::from(p));
            }
        }
        None
    }

    fn workspace() -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("familiar-bench-{}", std::process::id()));
        let cand = base.join("candidate");
        let scratch = base.join("scratch");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&cand).unwrap();
        std::fs::create_dir_all(&scratch).unwrap();
        (cand, scratch)
    }

    #[test]
    fn a_passing_self_test_passes_the_bench() {
        let Some(py) = python() else {
            eprintln!("skipping: no python3");
            return;
        };
        let (cand, scratch) = workspace();
        // A stdlib-only self-test that exercises a tiny "driver" and exits 0.
        std::fs::write(
            cand.join("driver.py"),
            "def frame(t,p): return bytes([0x53,t])+p\n",
        )
        .unwrap();
        std::fs::write(
            cand.join("test_driver.py"),
            "import sys; sys.path.insert(0,'.')\n\
             import driver\n\
             assert driver.frame(2, b'') == bytes([0x53,2]), 'framing'\n\
             print('BENCH_OK')\n",
        )
        .unwrap();
        // The test reads driver.py from the candidate dir; run with cwd=scratch
        // so sys.path '.' is scratch — copy driver there too, or point path at
        // the candidate dir. Simplest: the test adds the candidate dir.
        std::fs::write(
            cand.join("test_driver.py"),
            format!(
                "import sys; sys.path.insert(0, {:?})\n\
                 import driver\n\
                 assert driver.frame(2, b'') == bytes([0x53,2]), 'framing'\n\
                 print('BENCH_OK')\n",
                cand.to_string_lossy()
            ),
        )
        .unwrap();

        let report = run_bench(
            &cand,
            &["test_driver.py".to_string()],
            &py,
            &scratch,
            &[],
            10,
        )
        .expect("bench");
        assert!(report.passed, "bench should pass: {:?}", report.results);
        assert!(report.results[0].output.contains("BENCH_OK"));
        let _ = std::fs::remove_dir_all(cand.parent().unwrap());
    }

    #[test]
    fn a_failing_self_test_fails_the_bench() {
        let Some(py) = python() else {
            eprintln!("skipping: no python3");
            return;
        };
        let (cand, scratch) = workspace();
        std::fs::write(cand.join("test_bad.py"), "assert False, 'boom'\n").unwrap();
        let report =
            run_bench(&cand, &["test_bad.py".to_string()], &py, &scratch, &[], 10).expect("bench");
        assert!(!report.passed, "bench should fail");
        assert_eq!(report.results[0].outcome, JailOutcome::Failed);
        let _ = std::fs::remove_dir_all(cand.parent().unwrap());
    }

    #[test]
    fn the_bench_runs_under_the_jail_no_radio_no_household() {
        let Some(py) = python() else {
            eprintln!("skipping: no python3");
            return;
        };
        let (cand, scratch) = workspace();
        // Plant a secret under a fake home; pass that home so the mandatory
        // household denylist hides <home>/.ssh. The hostile self-test reads the
        // secret by absolute path and prints its CONTENT; under the jail the
        // read is denied, so the secret token never appears in the output.
        let home = std::env::temp_dir().join(format!("familiar-bench-home-{}", std::process::id()));
        let ssh = home.join(".ssh");
        std::fs::create_dir_all(&ssh).unwrap();
        let secret = ssh.join("id_ed25519");
        std::fs::write(&secret, "SECRET-TOKEN-9F3A").unwrap();
        std::fs::write(
            cand.join("test_evil.py"),
            format!("print(open({:?}).read())\n", secret.to_string_lossy()),
        )
        .unwrap();

        // run_bench builds its own profile; hide the secret's dir via
        // extra_hidden (the mandatory list is keyed on the daemon's own $HOME;
        // this test's secret lives under a fabricated home, added explicitly).
        let report = run_bench(
            &cand,
            &["test_evil.py".to_string()],
            &py,
            &scratch,
            std::slice::from_ref(&ssh),
            10,
        )
        .expect("bench");
        assert!(!report.passed, "reading a hidden secret must fail the test");
        assert!(
            !report.results[0].output.contains("SECRET-TOKEN-9F3A"),
            "the bench leaked a hidden secret: {:?}",
            report.results[0].output
        );
        let _ = std::fs::remove_dir_all(cand.parent().unwrap());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn materialize_then_bench_end_to_end() {
        let Some(py) = python() else {
            eprintln!("skipping: no python3");
            return;
        };
        // Prove the whole slice: a validated candidate's bytes are materialized
        // (digest-verified) and its self-test passes under the jail.
        let base = std::env::temp_dir().join(format!("familiar-e2e-{}", std::process::id()));
        let cand = base.join("candidate");
        let scratch = base.join("scratch");
        let _ = std::fs::remove_dir_all(&base);

        let driver = b"def ok(): return 42\n".to_vec();
        let test = format!(
            "import sys; sys.path.insert(0, {:?})\nimport driver\nassert driver.ok()==42\nprint('E2E_OK')\n",
            cand.to_string_lossy()
        )
        .into_bytes();
        let mut store = BTreeMap::new();
        let dd = digest_bytes(&driver);
        let td = digest_bytes(&test);
        store.insert(dd.clone(), driver);
        store.insert(td.clone(), test);
        let manifest = Manifest {
            files: vec![
                FileEntry {
                    path: "driver.py".into(),
                    digest: dd,
                    role: FileRole::Source,
                },
                FileEntry {
                    path: "test_driver.py".into(),
                    digest: td,
                    role: FileRole::SelfTest,
                },
            ],
        };
        crate::materialize::materialize(&manifest, &store, &cand).expect("materialize");
        let report = run_bench(
            &cand,
            &["test_driver.py".to_string()],
            &py,
            &scratch,
            &[],
            10,
        )
        .expect("bench");
        assert!(report.passed, "{:?}", report.results);
        assert!(report.results[0].output.contains("E2E_OK"));
        let _ = std::fs::remove_dir_all(&base);
    }
}
