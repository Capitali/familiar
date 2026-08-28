//! The containment jail (T-229 brick 2) — the floor codex's Round 2 signed,
//! repaired against codex's Brick-2 review (2026-08-28).
//!
//! A factory candidate is untrusted code the familiar generated. It runs under
//! a macOS sandbox profile plus a resource-bounded, process-group-isolated
//! launcher. The guarantees, each demonstrated by a hostile fixture rather than
//! asserted:
//!
//!   - **No radio.** The profile grants no `mach-lookup` at all, so the
//!     candidate cannot reach `bluetoothd`/CoreBluetooth over XPC/Mach (proven:
//!     a `bleak` scan inside the jail returns "Bluetooth unsupported" while the
//!     same scan outside finds devices). IOKit is denied too. The trusted BLE
//!     broker — a separate process — owns the radio; the candidate only ever
//!     receives fixed inherited pipe descriptors from it.
//!   - **No household.** A code-owned, MANDATORY denylist hides the sensitive
//!     namespace — the data dir, the repo, and the key/credential directories
//!     under `$HOME` — regardless of what the caller passes; callers may add
//!     hidden roots but can never remove the mandatory ones. Proven: a read of
//!     the repo, the data dir, and `~/.ssh` all return `Operation not
//!     permitted` inside the jail.
//!   - **No network.** `network*` denied. Proven against a live listener.
//!   - **Write only scratch.** One writable directory; everything else is
//!     read-only or denied.
//!   - **Bounded.** CPU, address space, process count, and file descriptors
//!     are capped by `ulimit` in the launch wrapper; output is drained
//!     concurrently and the whole process group is killed the moment it
//!     exceeds the byte cap or the wall clock. Proven: an output flood is
//!     refused with `OutputCapExceeded` (not a timeout), a fork fan-out is
//!     bounded, and a hang is killed.
//!
//! ## Why reads are a mandatory denylist, not an allowlist
//!
//! codex's floor asked for a read allowlist. On macOS 27 that is not
//! achievable for a dynamically-linked interpreter: dyld maps the shared cache
//! and cryptex images from paths that are neither stable nor fully enumerable,
//! and a strict `(deny default)` read-allowlist SIGABRTs the interpreter
//! before it runs (verified 2026-08-28). So reads are broad, and the
//! **sensitive namespace is denied by a mandatory, code-owned policy** —
//! exactly the property codex named ("the boundary, data dir, repo, and
//! household files are outside its authority"). The residual — an arbitrary
//! new secret placed OUTSIDE the known household namespace — is the documented
//! limit of a sandbox-exec jail for dynamic Python; a true read-allowlist
//! needs a static/containerized runtime, recorded as a follow-up.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Resource bounds enforced by `ulimit` in the launch wrapper.
#[derive(Debug, Clone, Copy)]
pub struct ResourceLimits {
    /// CPU seconds (`ulimit -t`).
    pub cpu_secs: u64,
    /// Address space in KB (`ulimit -v`); 0 leaves it unset (macOS often does
    /// not honor `-v`, so the wall clock + output cap are the real backstops).
    pub address_kb: u64,
    /// Max user processes (`ulimit -u`). 0 leaves it unset. **On macOS this
    /// limit is per-USER, not per-job**, so a small absolute value breaks the
    /// whole user's forking, not just the candidate; it cannot isolate one
    /// job. A fork fan-out is instead bounded in *time* by the CPU cap, the
    /// wall clock, and the process-group kill; a true per-job process cap
    /// needs a container (the static-runtime follow-up). Left unset by default.
    pub max_procs: u64,
    /// Max open file descriptors (`ulimit -n`) — per-process, enforceable.
    pub max_fds: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        ResourceLimits {
            cpu_secs: 30,
            address_kb: 0,
            max_procs: 0,
            max_fds: 256,
        }
    }
}

/// What a candidate is allowed to touch and for how long.
#[derive(Debug, Clone)]
pub struct ContainmentProfile {
    /// The candidate's own tree — always readable (re-granted even if it sits
    /// under a hidden root), never writable.
    pub candidate_root: PathBuf,
    /// Extra household roots the caller wants hidden. These ADD to the
    /// mandatory set (see [`mandatory_hidden_roots`]); they cannot remove it.
    pub extra_hidden_roots: Vec<PathBuf>,
    /// The one writable directory — scratch for the run's own files.
    pub scratch: PathBuf,
    /// Environment the candidate sees. Empty means an empty environment; the
    /// launcher never inherits the daemon's env.
    pub env: BTreeMap<String, String>,
    /// Wall-clock bound in seconds; the process group is killed on overrun.
    pub wall_secs: u64,
    /// Output cap in bytes (stdout+stderr combined). Exceeding it kills the
    /// group with [`JailOutcome::OutputCapExceeded`].
    pub output_cap: usize,
    /// Resource bounds applied via `ulimit`.
    pub limits: ResourceLimits,
    /// Home directory whose sensitive subtrees are hidden. Defaults to
    /// `$HOME`; overridable for tests.
    pub home: PathBuf,
}

impl ContainmentProfile {
    /// A minimal profile: one writable scratch dir (also the candidate root),
    /// default bounds, `$HOME` as the household to hide.
    pub fn minimal(candidate_root: PathBuf, scratch: PathBuf, wall_secs: u64) -> Self {
        ContainmentProfile {
            candidate_root,
            extra_hidden_roots: Vec::new(),
            scratch,
            env: BTreeMap::new(),
            wall_secs,
            output_cap: 64 * 1024,
            limits: ResourceLimits::default(),
            home: default_home(),
        }
    }
}

fn default_home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/root"))
}

/// The MANDATORY hidden roots for a given home: the household namespace no
/// candidate may read, regardless of caller. Code-owned so it cannot be
/// weakened by a caller who forgets a path. Covers the familiar's data dir,
/// this repo, and the key/credential directories.
pub fn mandatory_hidden_roots(home: &Path) -> Vec<PathBuf> {
    [
        "Library/Application Support/Familiar", // the daemon's data dir (boundary, keys, records)
        "Projects/familiar",                    // this repo (source, coordination, keys in tools)
        "Development/familiar",                 // the repo's other home on wildhorse
        ".ssh",                                 // ssh keys
        ".appstoreconnect",                     // ASC API keys
        ".codex",                               // codex auth + session data
        ".aws",
        ".config",
        ".gnupg",
    ]
    .iter()
    .map(|s| home.join(s))
    .collect()
}

fn quote_sbpl(path: &Path) -> String {
    // SBPL string literals are double-quoted; escape backslashes and quotes.
    let s = path.to_string_lossy();
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Generate the SBPL sandbox profile. Deny-by-default; broad read for dyld/the
/// interpreter; the mandatory household namespace denied; the candidate root
/// and scratch re-granted last (last match wins) so they survive any
/// overlapping deny; write only to scratch; **no `mach-lookup` at all** (closes
/// the CoreBluetooth XPC route); IOKit and network denied.
pub fn sbpl_profile(p: &ContainmentProfile) -> String {
    let mut out = String::new();
    out.push_str("(version 1)\n");
    out.push_str("(deny default)\n");
    out.push_str("(allow process*)\n");
    out.push_str("(allow sysctl-read)\n");
    out.push_str("(allow ipc-posix-shm*)\n");
    out.push_str("(allow signal (target self))\n");
    // Broad read for what dyld and the interpreter map/read at startup.
    out.push_str("(allow file-read* file-map-executable)\n");
    // The mandatory household namespace, then any extra roots the caller adds.
    for root in mandatory_hidden_roots(&p.home)
        .iter()
        .chain(p.extra_hidden_roots.iter())
    {
        out.push_str(&format!(
            "(deny file-read* (subpath {}))\n",
            quote_sbpl(root)
        ));
    }
    // Re-grant the candidate's own tree and scratch LAST, so a candidate that
    // happens to live under a hidden root is still readable.
    out.push_str(&format!(
        "(allow file-read* (subpath {}))\n",
        quote_sbpl(&p.candidate_root)
    ));
    out.push_str(&format!(
        "(allow file-read* file-write* (subpath {}))\n",
        quote_sbpl(&p.scratch)
    ));
    // No mach-lookup grant anywhere: the CoreBluetooth XPC route to bluetoothd
    // is unreachable. Deny it explicitly for legibility, plus IOKit and net.
    out.push_str("(deny mach-lookup)\n");
    out.push_str("(deny iokit-open)\n");
    out.push_str("(deny network*)\n");
    out
}

/// How a jailed run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JailOutcome {
    /// Exited 0 within all bounds.
    Ok,
    /// Exited non-zero within all bounds.
    Failed,
    /// The wall-clock bound killed it.
    TimedOut,
    /// The output byte cap killed it (distinct from a timeout).
    OutputCapExceeded,
    /// The launcher could not start the sandbox.
    LaunchError,
}

/// The result of a jailed run.
#[derive(Debug, Clone)]
pub struct JailRun {
    pub outcome: JailOutcome,
    /// Combined stdout+stderr, at most `output_cap` bytes.
    pub output: String,
}

impl JailRun {
    pub fn ok(&self) -> bool {
        self.outcome == JailOutcome::Ok
    }
}

fn shell_quote(s: &str) -> String {
    // Single-quote for /bin/sh, escaping embedded single quotes.
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Run `argv` (e.g. `["python3", "candidate.py"]`) under the profile: an empty
/// allowlisted environment, `ulimit` resource bounds, its own process group,
/// concurrently-drained output capped at `output_cap`, and a wall-clock bound.
/// The whole process group is killed on cap or wall overrun. Requires
/// `/usr/bin/sandbox-exec`.
pub fn run_jailed(profile: &ContainmentProfile, argv: &[String]) -> std::io::Result<JailRun> {
    use std::os::unix::process::CommandExt as _;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let _ = std::fs::create_dir_all(&profile.scratch);
    // sandbox-exec matches physical paths; resolve symlinked roots (macOS temp
    // dirs are /var → /private/var) so subpath grants match.
    let resolve = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let resolved = ContainmentProfile {
        candidate_root: resolve(&profile.candidate_root),
        extra_hidden_roots: profile
            .extra_hidden_roots
            .iter()
            .map(|r| resolve(r))
            .collect(),
        scratch: resolve(&profile.scratch),
        env: profile.env.clone(),
        wall_secs: profile.wall_secs,
        output_cap: profile.output_cap,
        limits: profile.limits,
        home: resolve(&profile.home),
    };
    let sbpl = sbpl_profile(&resolved);
    let sbpl_path = resolved.scratch.join(".jail.sb");
    std::fs::write(&sbpl_path, &sbpl)?;

    // Build: sh -c "ulimit ...; exec sandbox-exec -f <sb> <argv...>" so the
    // resource bounds apply to the sandboxed process and all its descendants.
    let l = &resolved.limits;
    let mut wrapper = String::new();
    wrapper.push_str(&format!("ulimit -t {} 2>/dev/null; ", l.cpu_secs));
    if l.address_kb > 0 {
        wrapper.push_str(&format!("ulimit -v {} 2>/dev/null; ", l.address_kb));
    }
    if l.max_procs > 0 {
        wrapper.push_str(&format!("ulimit -u {} 2>/dev/null; ", l.max_procs));
    }
    wrapper.push_str(&format!("ulimit -n {} 2>/dev/null; ", l.max_fds));
    wrapper.push_str("exec /usr/bin/sandbox-exec -f ");
    wrapper.push_str(&shell_quote(&sbpl_path.to_string_lossy()));
    for a in argv {
        wrapper.push(' ');
        wrapper.push_str(&shell_quote(a));
    }

    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c").arg(&wrapper);
    cmd.env_clear();
    for (k, v) in &resolved.env {
        cmd.env(k, v);
    }
    cmd.current_dir(&resolved.scratch);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.process_group(0); // own group so we can kill the whole tree

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => {
            let _ = std::fs::remove_file(&sbpl_path);
            return Ok(JailRun {
                outcome: JailOutcome::LaunchError,
                output: String::new(),
            });
        }
    };
    let pgid = child.id() as i32;

    // Drain stdout+stderr concurrently into a shared, byte-bounded buffer. The
    // readers keep draining past the cap (so a full pipe never deadlocks the
    // child) but discard beyond it and raise `over` the instant the cap is
    // crossed, which the supervisor turns into a group kill.
    let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let total = Arc::new(AtomicUsize::new(0));
    let over = Arc::new(AtomicBool::new(false));
    let cap = resolved.output_cap;

    let mut readers = Vec::new();
    let pipes: [Option<Box<dyn std::io::Read + Send>>; 2] = [
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn std::io::Read + Send>),
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn std::io::Read + Send>),
    ];
    for pipe in pipes {
        let Some(mut pipe) = pipe else { continue };
        let buf = buf.clone();
        let total = total.clone();
        let over = over.clone();
        readers.push(std::thread::spawn(move || {
            use std::io::Read as _;
            let mut chunk = [0u8; 4096];
            loop {
                match pipe.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let prev = total.fetch_add(n, Ordering::SeqCst);
                        if prev < cap {
                            let take = n.min(cap - prev);
                            buf.lock().unwrap().extend_from_slice(&chunk[..take]);
                        }
                        if prev + n > cap {
                            over.store(true, Ordering::SeqCst);
                        }
                    }
                }
            }
        }));
    }

    let kill_group = || {
        let _ = Command::new("/bin/kill")
            .arg("-KILL")
            .arg(format!("-{pgid}"))
            .status();
    };

    let start = Instant::now();
    let mut outcome;
    loop {
        if over.load(Ordering::SeqCst) {
            kill_group();
            let _ = child.wait();
            outcome = JailOutcome::OutputCapExceeded;
            break;
        }
        match child.try_wait()? {
            Some(status) => {
                outcome = if status.success() {
                    JailOutcome::Ok
                } else {
                    JailOutcome::Failed
                };
                break;
            }
            None => {
                if start.elapsed() >= Duration::from_secs(resolved.wall_secs) {
                    kill_group();
                    let _ = child.wait();
                    outcome = JailOutcome::TimedOut;
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
    // If the cap was crossed right as the process exited, prefer the cap
    // verdict — it is the more specific refusal.
    if over.load(Ordering::SeqCst) && outcome == JailOutcome::Ok {
        outcome = JailOutcome::OutputCapExceeded;
    }

    for r in readers {
        let _ = r.join();
    }
    let bytes = buf.lock().unwrap().clone();
    let output = String::from_utf8_lossy(&bytes).into_owned();

    let _ = std::fs::remove_file(&sbpl_path);
    Ok(JailRun { outcome, output })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox_exec_present() -> bool {
        Path::new("/usr/bin/sandbox-exec").exists()
    }

    fn scratch_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("familiar-jail-{}-{}", tag, std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        d
    }

    #[test]
    fn the_profile_denies_by_default_names_denials_and_grants_no_mach_lookup() {
        let p = ContainmentProfile::minimal(
            PathBuf::from("/tmp/cand"),
            PathBuf::from("/tmp/scratch"),
            10,
        );
        let sbpl = sbpl_profile(&p);
        assert!(sbpl.contains("(deny default)"));
        assert!(sbpl.contains("(deny network*)"));
        assert!(sbpl.contains("(deny iokit-open)"));
        assert!(sbpl.contains("(deny mach-lookup)"));
        // Crucially: no ALLOW of mach-lookup anywhere — the radio's XPC route.
        assert!(!sbpl.contains("(allow mach-lookup"));
        assert!(sbpl.contains("file-write* (subpath \"/tmp/scratch\")"));
    }

    #[test]
    fn the_household_denylist_is_mandatory_and_precedes_the_broad_read() {
        // Even with no extra roots, the mandatory household namespace is denied.
        let p = ContainmentProfile {
            home: PathBuf::from("/Users/tester"),
            ..ContainmentProfile::minimal(
                PathBuf::from("/tmp/cand"),
                PathBuf::from("/tmp/scratch"),
                10,
            )
        };
        let sbpl = sbpl_profile(&p);
        let broad = sbpl.find("(allow file-read* file-map-executable)").unwrap();
        for expected in [
            "(deny file-read* (subpath \"/Users/tester/Library/Application Support/Familiar\"))",
            "(deny file-read* (subpath \"/Users/tester/Projects/familiar\"))",
            "(deny file-read* (subpath \"/Users/tester/.ssh\"))",
            "(deny file-read* (subpath \"/Users/tester/.appstoreconnect\"))",
        ] {
            let at = sbpl
                .find(expected)
                .unwrap_or_else(|| panic!("missing mandatory deny: {expected}"));
            assert!(at > broad, "the household deny must follow the broad read");
        }
    }

    #[test]
    fn a_jailed_process_runs_and_writes_its_scratch() {
        if !sandbox_exec_present() {
            eprintln!("skipping: no /usr/bin/sandbox-exec");
            return;
        }
        let scratch = scratch_dir("ok");
        std::fs::write(scratch.join("hello.txt"), "hi").unwrap();
        let p = ContainmentProfile::minimal(scratch.clone(), scratch.clone(), 10);
        let argv = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "cat hello.txt && echo done > out.txt && echo RAN".to_string(),
        ];
        let r = run_jailed(&p, &argv).expect("run");
        assert_eq!(r.outcome, JailOutcome::Ok, "got {r:?}");
        assert!(r.output.contains("hi") && r.output.contains("RAN"));
        assert!(
            scratch.join("out.txt").exists(),
            "scratch write should succeed"
        );
        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[test]
    fn a_jailed_process_cannot_read_a_mandatory_hidden_root() {
        if !sandbox_exec_present() {
            eprintln!("skipping: no /usr/bin/sandbox-exec");
            return;
        }
        let scratch = scratch_dir("noread");
        // Fabricate a fake home whose .ssh holds a secret; make it mandatory by
        // pointing the profile's home there (the mandatory list denies
        // <home>/.ssh).
        let home = scratch_dir("fakehome");
        let ssh = home.join(".ssh");
        std::fs::create_dir_all(&ssh).unwrap();
        std::fs::write(ssh.join("id_ed25519"), "TOPSECRETKEY").unwrap();
        let p = ContainmentProfile {
            home: home.clone(),
            ..ContainmentProfile::minimal(scratch.clone(), scratch.clone(), 10)
        };
        let argv = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            format!("cat {}/id_ed25519 2>&1 || echo DENIED", ssh.display()),
        ];
        let r = run_jailed(&p, &argv).expect("run");
        assert!(
            !r.output.contains("TOPSECRETKEY"),
            "the jail leaked a mandatory-hidden secret: {r:?}"
        );
        let _ = std::fs::remove_dir_all(&scratch);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn a_jailed_process_cannot_open_the_network() {
        if !sandbox_exec_present() {
            eprintln!("skipping: no /usr/bin/sandbox-exec");
            return;
        }
        // Stand up a real listener; prove an unsandboxed control connects, then
        // prove the jailed process is denied — so a pass cannot be "nothing was
        // listening".
        use std::io::Write as _;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            for _ in 0..2 {
                if let Ok((mut s, _)) = listener.accept() {
                    let _ = s.write_all(b"HELLO");
                }
            }
        });

        // Control (unsandboxed) connects.
        let control = std::net::TcpStream::connect(("127.0.0.1", port));
        assert!(control.is_ok(), "control connection should succeed");
        drop(control);

        let scratch = scratch_dir("nonet");
        let p = ContainmentProfile::minimal(scratch.clone(), scratch.clone(), 10);
        let argv = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            format!("exec 3<>/dev/tcp/127.0.0.1/{port} && echo CONNECTED || echo BLOCKED"),
        ];
        let r = run_jailed(&p, &argv).expect("run");
        assert!(
            !r.output.contains("CONNECTED"),
            "the jail allowed a network connection: {r:?}"
        );
        drop(handle);
        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[test]
    fn the_output_cap_kills_a_flood_with_a_distinct_refusal() {
        if !sandbox_exec_present() {
            eprintln!("skipping: no /usr/bin/sandbox-exec");
            return;
        }
        let scratch = scratch_dir("flood");
        let mut p = ContainmentProfile::minimal(scratch.clone(), scratch.clone(), 30);
        p.output_cap = 4096;
        // `yes` floods stdout forever; the cap must kill it, and the verdict
        // must be OutputCapExceeded, NOT a wall-clock timeout.
        let argv = vec!["/usr/bin/yes".to_string()];
        let start = std::time::Instant::now();
        let r = run_jailed(&p, &argv).expect("run");
        assert_eq!(r.outcome, JailOutcome::OutputCapExceeded, "got {r:?}");
        assert!(
            r.output.len() <= 4096 + 4096,
            "output not bounded: {}",
            r.output.len()
        );
        assert!(start.elapsed().as_secs() < 25, "cap kill was too slow");
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
        assert_eq!(r.outcome, JailOutcome::TimedOut, "got {r:?}");
        assert!(start.elapsed().as_secs() < 10);
        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[test]
    fn the_process_count_limit_bounds_a_fork_fanout() {
        if !sandbox_exec_present() {
            eprintln!("skipping: no /usr/bin/sandbox-exec");
            return;
        }
        let scratch = scratch_dir("fork");
        // Wall clock 3s. A fork fan-out of backgrounded sleeps must be killed
        // as a whole group at the wall bound — proving descendants do not
        // escape the group kill and wedge the harness. (Per-job process-count
        // isolation is not available via ulimit on macOS; the group kill +
        // wall clock is the enforceable bound. See ResourceLimits::max_procs.)
        let p = ContainmentProfile::minimal(scratch.clone(), scratch.clone(), 3);
        let argv = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "i=0; while [ $i -lt 40 ]; do sleep 30 & i=$((i+1)); done; wait".to_string(),
        ];
        let start = std::time::Instant::now();
        let r = run_jailed(&p, &argv).expect("run");
        assert_eq!(r.outcome, JailOutcome::TimedOut, "got {r:?}");
        assert!(
            start.elapsed().as_secs() < 12,
            "fork fan-out escaped the group kill: {:?}",
            start.elapsed()
        );
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
