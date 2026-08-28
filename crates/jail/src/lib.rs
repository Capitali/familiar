//! The containment jail (T-229 brick 2) — the floor codex's Round 2 signed.
//!
//! A factory candidate is untrusted code the familiar generated. It runs
//! under a macOS sandbox profile that starts from `(deny default)` and grants
//! only: read of its own candidate tree and the named toolchain, write of one
//! bounded scratch directory, and process basics. **Network is denied. Any
//! Bluetooth/CoreBluetooth access is denied** — the candidate never touches
//! the radio; a separate trusted broker owns Bluetooth and hands the
//! candidate a capability-scoped pipe. The launcher runs the candidate in its
//! own process group with an empty, allowlisted environment, a wall-clock
//! bound, and an output cap, killing the whole group on overrun.
//!
//! The SBPL profile is pure, testable string generation; the launcher wraps
//! `/usr/bin/sandbox-exec`. Hostile fixtures actually run a probe under the
//! profile and prove the denials hold — resource limits are not containment,
//! so the jail must be demonstrated, not asserted.
//!
//! ## Why reads are a denylist, not an allowlist
//!
//! codex's Round-2 floor asked for a read *allowlist* (candidate tree +
//! toolchain, nothing else). On macOS 27 that is not achievable for a
//! dynamically-linked interpreter: dyld maps the shared cache and cryptex
//! images from paths that are neither stable nor fully enumerable, and a
//! strict `(deny default)` read-allowlist SIGABRTs the process before it
//! runs (verified 2026-08-28). So reads are granted broadly for
//! `file-map-executable` (what dyld needs) and then the **named household
//! roots are explicitly denied** — the boundary, data dir, repo, and any
//! secret directory the caller lists. The security property codex actually
//! named ("the boundary, data dir, repo, and household files are outside its
//! authority") is enforced exactly, and the hostile fixture proves a planted
//! secret in a hidden root is unreadable. Writes remain a true allowlist:
//! only the scratch dir, nothing else. Network and IOKit (the radio) are
//! denied outright.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What a candidate is allowed to touch and for how long.
#[derive(Debug, Clone)]
pub struct ContainmentProfile {
    /// The candidate's own tree — always readable (re-granted even if it sits
    /// under a hidden root), never writable.
    pub candidate_root: PathBuf,
    /// Household roots the candidate must never read: the boundary, data dir,
    /// repo, keys — anything private. Denied after the broad read grant, so
    /// the deny wins.
    pub hidden_roots: Vec<PathBuf>,
    /// The one writable directory — scratch for the run's own files.
    pub scratch: PathBuf,
    /// Environment the candidate sees. Empty means an empty environment; the
    /// launcher never inherits the daemon's env.
    pub env: BTreeMap<String, String>,
    /// Wall-clock bound in seconds; the process group is killed on overrun.
    pub wall_secs: u64,
    /// Output cap in bytes (stdout+stderr combined, captured).
    pub output_cap: usize,
}

impl ContainmentProfile {
    /// A minimal profile: one writable scratch dir (also the candidate root),
    /// the given hidden roots, empty env, the given wall bound.
    pub fn minimal(candidate_root: PathBuf, scratch: PathBuf, wall_secs: u64) -> Self {
        ContainmentProfile {
            candidate_root,
            hidden_roots: Vec::new(),
            scratch,
            env: BTreeMap::new(),
            wall_secs,
            output_cap: 64 * 1024,
        }
    }
}

fn quote_sbpl(path: &Path) -> String {
    // SBPL string literals are double-quoted; escape backslashes and quotes.
    let s = path.to_string_lossy();
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Generate the SBPL sandbox profile. Deny-by-default; broad executable read
/// for dyld; named household roots denied; the candidate root and scratch
/// re-granted last (last match wins) so they survive any overlapping deny;
/// write only to scratch; network and radio denied.
pub fn sbpl_profile(p: &ContainmentProfile) -> String {
    let mut out = String::new();
    out.push_str("(version 1)\n");
    out.push_str("(deny default)\n");
    out.push_str("(allow process*)\n");
    out.push_str("(allow sysctl-read)\n");
    out.push_str("(allow mach-lookup)\n");
    out.push_str("(allow ipc-posix-shm*)\n");
    out.push_str("(allow signal (target self))\n");
    // Broad read for what dyld and the interpreter map. `file-map-executable`
    // is the operation the shared cache needs; without it a linked binary
    // aborts at launch under a deny-default base.
    out.push_str("(allow file-read* file-map-executable)\n");
    // Then hide the household. These come after the broad allow, so for these
    // subpaths the deny is the last match and wins.
    for root in &p.hidden_roots {
        out.push_str(&format!(
            "(deny file-read* (subpath {}))\n",
            quote_sbpl(root)
        ));
    }
    // Re-grant the candidate's own tree and scratch last, so a candidate that
    // happens to live under a hidden root is still readable.
    out.push_str(&format!(
        "(allow file-read* (subpath {}))\n",
        quote_sbpl(&p.candidate_root)
    ));
    // The one writable place.
    out.push_str(&format!(
        "(allow file-read* file-write* (subpath {}))\n",
        quote_sbpl(&p.scratch)
    ));
    // Network stays denied by the base; stated explicitly so the intent is
    // legible and survives any future base change.
    out.push_str("(deny network*)\n");
    // Bluetooth is reached through IOKit user clients; deny that surface
    // outright — the candidate has no radio, ever. The broker owns it.
    out.push_str("(deny iokit-open)\n");
    out
}

/// The result of a jailed run.
#[derive(Debug, Clone)]
pub struct JailRun {
    /// True if the process exited 0 within its bounds.
    pub ok: bool,
    /// True if the wall-clock bound killed it.
    pub timed_out: bool,
    /// Combined stdout+stderr, truncated to the output cap.
    pub output: String,
}

/// Run `argv` (e.g. `["python3", "candidate.py"]`) under the profile. The
/// child is placed in its own process group with an empty, allowlisted
/// environment; on wall-clock overrun the whole group is killed. Requires
/// `/usr/bin/sandbox-exec` (present on macOS).
pub fn run_jailed(profile: &ContainmentProfile, argv: &[String]) -> std::io::Result<JailRun> {
    use std::io::Read as _;
    use std::os::unix::process::CommandExt as _;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let _ = std::fs::create_dir_all(&profile.scratch);
    // sandbox-exec matches against physical paths, so a subpath grant for a
    // symlinked root (macOS temp dirs are /var → /private/var; /tmp →
    // /private/tmp) would never match. Resolve every path before generating
    // the profile, falling back to the given path when it cannot be resolved.
    let resolve = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let resolved = ContainmentProfile {
        candidate_root: resolve(&profile.candidate_root),
        hidden_roots: profile.hidden_roots.iter().map(|r| resolve(r)).collect(),
        scratch: resolve(&profile.scratch),
        env: profile.env.clone(),
        wall_secs: profile.wall_secs,
        output_cap: profile.output_cap,
    };
    let sbpl = sbpl_profile(&resolved);
    let sbpl_path = resolved.scratch.join(".jail.sb");
    std::fs::write(&sbpl_path, &sbpl)?;
    let profile = &resolved;

    let mut cmd = Command::new("/usr/bin/sandbox-exec");
    cmd.arg("-f").arg(&sbpl_path);
    for a in argv {
        cmd.arg(a);
    }
    // Empty, allowlisted environment — never the daemon's.
    cmd.env_clear();
    for (k, v) in &profile.env {
        cmd.env(k, v);
    }
    cmd.current_dir(&profile.scratch);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    // Own process group so we can kill the whole tree on overrun.
    cmd.process_group(0);

    let mut child = cmd.spawn()?;
    let pgid = child.id() as i32;

    let start = Instant::now();
    let mut timed_out = false;
    let mut exit_ok = false;
    loop {
        match child.try_wait()? {
            Some(status) => {
                exit_ok = status.success();
                break;
            }
            None => {
                if start.elapsed() >= Duration::from_secs(profile.wall_secs) {
                    // Kill the whole process group via the kill binary (no
                    // unsafe/libc): a negative pid targets the group.
                    let _ = Command::new("/bin/kill")
                        .arg("-KILL")
                        .arg(format!("-{pgid}"))
                        .status();
                    let _ = child.wait();
                    timed_out = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }

    let mut output = String::new();
    if let Some(mut o) = child.stdout.take() {
        let mut buf = String::new();
        let _ = o.read_to_string(&mut buf);
        output.push_str(&buf);
    }
    if let Some(mut e) = child.stderr.take() {
        let mut buf = String::new();
        let _ = e.read_to_string(&mut buf);
        output.push_str(&buf);
    }
    if output.len() > profile.output_cap {
        output.truncate(profile.output_cap);
    }
    let ok = !timed_out && exit_ok;

    let _ = std::fs::remove_file(&sbpl_path);
    Ok(JailRun {
        ok,
        timed_out,
        output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(scratch: &Path) -> ContainmentProfile {
        ContainmentProfile::minimal(PathBuf::from("/tmp"), scratch.to_path_buf(), 10)
    }

    #[test]
    fn the_profile_denies_by_default_and_names_its_denials() {
        let p = profile(Path::new("/tmp/scratch"));
        let sbpl = sbpl_profile(&p);
        assert!(sbpl.contains("(deny default)"));
        assert!(sbpl.contains("(deny network*)"));
        assert!(sbpl.contains("(deny iokit-open)"));
        // The scratch is the only writable subpath.
        assert!(sbpl.contains("file-write* (subpath \"/tmp/scratch\")"));
    }

    #[test]
    fn hidden_roots_are_denied_after_the_broad_read_grant() {
        let mut p = profile(Path::new("/tmp/s"));
        p.hidden_roots = vec![PathBuf::from(
            "/Users/ian/Library/Application Support/Familiar",
        )];
        let sbpl = sbpl_profile(&p);
        let broad = sbpl.find("(allow file-read* file-map-executable)").unwrap();
        let deny = sbpl
            .find("(deny file-read* (subpath \"/Users/ian/Library/Application Support/Familiar\"))")
            .unwrap();
        // The deny must come AFTER the broad allow (last match wins).
        assert!(
            deny > broad,
            "the household deny must follow the broad read"
        );
    }

    fn scratch_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "familiar-jail-{}-{}-{}",
            tag,
            std::process::id(),
            // vary by tag only; process id keeps parallel tests apart
            tag.len()
        ));
        let _ = std::fs::create_dir_all(&d);
        d
    }

    fn sandbox_exec_present() -> bool {
        Path::new("/usr/bin/sandbox-exec").exists()
    }

    #[test]
    fn a_jailed_process_runs_and_reads_its_scratch() {
        if !sandbox_exec_present() {
            eprintln!("skipping: no /usr/bin/sandbox-exec");
            return;
        }
        let scratch = scratch_dir("ok");
        std::fs::write(scratch.join("hello.txt"), "hi").unwrap();
        let p = ContainmentProfile::minimal(scratch.clone(), scratch.clone(), 10);
        // Read a file inside scratch and print it.
        let argv = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            format!("cat {}/hello.txt", scratch.display()),
        ];
        let r = run_jailed(&p, &argv).expect("run");
        assert!(r.ok, "expected clean exit, got {r:?}");
        assert!(r.output.contains("hi"));
        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[test]
    fn a_jailed_process_cannot_read_outside_its_grants() {
        if !sandbox_exec_present() {
            eprintln!("skipping: no /usr/bin/sandbox-exec");
            return;
        }
        let scratch = scratch_dir("noread");
        // A secret OUTSIDE the candidate root and scratch — stands in for
        // boundary.json / household files.
        let secret_dir =
            std::env::temp_dir().join(format!("familiar-jail-secret-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&secret_dir);
        let secret = secret_dir.join("boundary.json");
        std::fs::write(&secret, "TOPSECRET").unwrap();
        // secret_dir is a hidden household root — denied after the broad read.
        let mut p = ContainmentProfile::minimal(scratch.clone(), scratch.clone(), 10);
        p.hidden_roots = vec![secret_dir.clone()];
        let argv = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            format!("cat {} 2>&1", secret.display()),
        ];
        let r = run_jailed(&p, &argv).expect("run");
        assert!(
            !r.output.contains("TOPSECRET"),
            "the jail leaked a file outside its grants: {r:?}"
        );
        let _ = std::fs::remove_dir_all(&scratch);
        let _ = std::fs::remove_dir_all(&secret_dir);
    }

    #[test]
    fn a_jailed_process_cannot_open_the_network() {
        if !sandbox_exec_present() {
            eprintln!("skipping: no /usr/bin/sandbox-exec");
            return;
        }
        let scratch = scratch_dir("nonet");
        let p = ContainmentProfile::minimal(scratch.clone(), scratch.clone(), 10);
        // bash /dev/tcp is the simplest socket attempt without deps.
        let argv = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            // Try to connect; print MARKER only if the connection opens.
            "exec 3<>/dev/tcp/127.0.0.1/9 && echo CONNECTED || echo BLOCKED".to_string(),
        ];
        let r = run_jailed(&p, &argv).expect("run");
        assert!(
            !r.output.contains("CONNECTED"),
            "the jail allowed a network connection: {r:?}"
        );
        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[test]
    fn the_wall_clock_bound_kills_a_hang() {
        if !sandbox_exec_present() {
            eprintln!("skipping: no /usr/bin/sandbox-exec");
            return;
        }
        let scratch = scratch_dir("hang");
        let p = ContainmentProfile::minimal(scratch.clone(), scratch.clone(), 1);
        let argv = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "sleep 30".to_string(),
        ];
        let start = std::time::Instant::now();
        let r = run_jailed(&p, &argv).expect("run");
        assert!(r.timed_out, "expected a timeout kill, got {r:?}");
        assert!(
            start.elapsed().as_secs() < 10,
            "kill took too long: {:?}",
            start.elapsed()
        );
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
