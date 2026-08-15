//! Manage the metabolism as a background process — manually (pidfile) or via launchd
//! (start at login). macOS-oriented (`launchctl`, `kill`). The GUI control bar and the
//! `familiar daemon` subcommands both go through here.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const PIDFILE: &str = "daemon.pid";
const LOGFILE: &str = "daemon.log";
const LAUNCHD_LABEL: &str = "io.river.familiar";
/// A durable home for the installed binary, outside the build tree (which `cargo clean`
/// wipes) — the same stable bin dir the FamiliarMac console expects.
const STABLE_SUBDIR: &str = "Library/Application Support/Familiar/bin";

fn pidfile(dir: &Path) -> PathBuf {
    dir.join(PIDFILE)
}

fn is_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Record the *current* process as the daemon (used by `run --daemon` itself, so a
/// launchd-launched daemon is visible to `status`/`start`/`stop`).
pub fn record_self(dir: &Path) {
    let _ = fs::create_dir_all(dir);
    let _ = fs::write(pidfile(dir), std::process::id().to_string());
}

/// The running daemon's pid, if any. Clears a stale pidfile.
pub fn status(dir: &Path) -> Option<u32> {
    let pid: u32 = fs::read_to_string(pidfile(dir)).ok()?.trim().parse().ok()?;
    if is_alive(pid) {
        Some(pid)
    } else {
        let _ = fs::remove_file(pidfile(dir));
        None
    }
}

/// Start a detached daemon (no-op if one is already running). Returns its pid.
/// Output goes to `daemon.log`; the child outlives this process.
pub fn start(dir: &Path, interval: u64) -> io::Result<u32> {
    if let Some(pid) = status(dir) {
        return Ok(pid);
    }
    fs::create_dir_all(dir)?;
    let exe = std::env::current_exe()?;
    let log = fs::File::create(dir.join(LOGFILE))?;
    let log_err = log.try_clone()?;
    let child = Command::new(exe)
        .arg("run")
        .arg("--daemon")
        .arg("--interval")
        .arg(interval.to_string())
        .arg("--data-dir")
        .arg(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        .spawn()?;
    let pid = child.id();
    fs::write(pidfile(dir), pid.to_string())?;
    Ok(pid) // child is intentionally not awaited — it runs in the background
}

/// Stop the running daemon (SIGTERM). Returns whether one was running.
pub fn stop(dir: &Path) -> io::Result<bool> {
    match status(dir) {
        Some(pid) => {
            Command::new("kill").arg(pid.to_string()).status()?;
            let _ = fs::remove_file(pidfile(dir));
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Stop then start — reload the metabolism.
pub fn reload(dir: &Path, interval: u64) -> io::Result<u32> {
    stop(dir)?;
    start(dir, interval)
}

fn launchd_plist_path() -> io::Result<PathBuf> {
    let home = std::env::var("HOME")
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "HOME not set"))?;
    Ok(PathBuf::from(home)
        .join("Library/LaunchAgents")
        .join(format!("{LAUNCHD_LABEL}.plist")))
}

/// The durable bin directory the login item points at, so a `cargo clean` can't break it.
fn stable_bin_dir() -> io::Result<PathBuf> {
    let home = std::env::var("HOME")
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "HOME not set"))?;
    Ok(PathBuf::from(home).join(STABLE_SUBDIR))
}

/// Copy the running `familiar` binary into the stable bin directory and return that path,
/// so launchd points at a copy outside `target/`. Skips the copy if already in place.
/// `cargo build --release` then `target/release/familiar daemon install` installs release.
fn install_stable_binary() -> io::Result<PathBuf> {
    let src = std::env::current_exe()?;
    let bin = stable_bin_dir()?;
    fs::create_dir_all(&bin)?;
    let dst = bin.join("familiar");
    if fs::canonicalize(&src).ok() != fs::canonicalize(&dst).ok() {
        fs::copy(&src, &dst)?;
    }
    Ok(dst)
}

/// The uid launchd domain targets are scoped by (`gui/<uid>`). Shelled out like every
/// other process fact in this file — no unsafe, no new dependency.
fn current_uid() -> io::Result<u32> {
    let out = Command::new("id").arg("-u").output()?;
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "unparseable `id -u` output"))
}

/// The launchd (re)registration bracket, as argv lists in execution order around the
/// binary swap: `bootout` runs BEFORE install_stable_binary() replaces the executable,
/// `bootstrap` + `kickstart -k` after the fresh plist is written. One dialect, shared
/// with tools/new-mac-bootstrap.sh — bootout/bootstrap, never `unload -w`/`load -w`
/// (that pair leaves the agent registered-but-stalled, and predates the macOS 27
/// lesson: launchd pins a Lightweight Code Requirement to the executable it
/// bootstrapped, so a swapped binary must be re-registered or its spawn dies with
/// OS_REASON_CODESIGNING; ad-hoc re-signing does not help).
fn register_bracket(uid: u32, plist: &Path) -> [Vec<String>; 3] {
    let domain = format!("gui/{uid}");
    let service = format!("{domain}/{LAUNCHD_LABEL}");
    [
        vec!["bootout".into(), service.clone()],
        vec!["bootstrap".into(), domain, plist.display().to_string()],
        vec!["kickstart".into(), "-k".into(), service],
    ]
}

fn launchctl(argv: &[String]) -> io::Result<std::process::ExitStatus> {
    Command::new("launchctl").args(argv).status()
}

/// Run a launchctl step whose failure means the install itself failed.
fn launchctl_checked(argv: &[String]) -> io::Result<()> {
    let status = launchctl(argv)?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "launchctl {} exited {status}",
            argv.join(" ")
        )))
    }
}

/// Install (or upgrade) the launchd LaunchAgent so the daemon starts at login.
/// Registers the plist through the bootout/bootstrap bracket. macOS only.
pub fn install(dir: &Path, interval: u64) -> io::Result<PathBuf> {
    let plist = launchd_plist_path()?;
    let [bootout, bootstrap, kickstart] = register_bracket(current_uid()?, &plist);
    // Bootout FIRST, while the registered executable is still the old one —
    // install_stable_binary() is about to swap that file (see register_bracket's doc
    // for the LWCR story). Not-registered is fine (fresh install), so this exit status
    // is deliberately unread.
    let _ = launchctl(&bootout);
    let exe = install_stable_binary()?;
    // launchd runs with cwd `/`, so a relative data dir would make `--data-dir` and the
    // log path resolve under `/` — the agent then fails to open its log and exits
    // EX_CONFIG (78). Canonicalize to an absolute path so a relative arg can't break it.
    let dir = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let dir = dir.as_path();
    let log = dir.join(LOGFILE);
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{LAUNCHD_LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
    <string>run</string>
    <string>--daemon</string>
    <string>--interval</string>
    <string>{interval}</string>
    <string>--data-dir</string>
    <string>{dir}</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><false/>
  <key>StandardOutPath</key><string>{log}</string>
  <key>StandardErrorPath</key><string>{log}</string>
</dict>
</plist>
"#,
        exe = exe.display(),
        dir = dir.display(),
        log = log.display(),
    );
    if let Some(parent) = plist.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&plist, xml)?;
    // bootstrap records the NEW binary's code requirement; kickstart -k starts it
    // fresh. An unregistered agent is a failed install, so these two are checked.
    launchctl_checked(&bootstrap)?;
    launchctl_checked(&kickstart)?;
    Ok(plist)
}

/// Remove the launchd LaunchAgent.
pub fn uninstall() -> io::Result<bool> {
    let plist = launchd_plist_path()?;
    if !plist.exists() {
        return Ok(false);
    }
    // bootout stops and unregisters in one act; not-loaded is fine (a plist alone is
    // installed-but-inert), so the exit status is deliberately unread.
    let [bootout, _, _] = register_bracket(current_uid()?, &plist);
    let _ = launchctl(&bootout);
    fs::remove_file(&plist)?;
    Ok(true)
}

/// Is the launchd agent installed?
pub fn is_installed() -> bool {
    launchd_plist_path().map(|p| p.exists()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The launchd dialect is bootout → bootstrap → kickstart -k, scoped to the gui
    /// domain — and never the `unload`/`load` pair. The order is the invariant: bootout
    /// brackets the binary swap from the front, so macOS 27's pinned code requirement
    /// is re-recorded (bootstrap) for the binary that will actually spawn.
    #[test]
    fn the_bracket_reregisters_and_never_speaks_load_or_unload() {
        let plist = Path::new("/Users/who/Library/LaunchAgents/io.river.familiar.plist");
        let [bootout, bootstrap, kickstart] = register_bracket(501, plist);
        assert_eq!(bootout, ["bootout", "gui/501/io.river.familiar"]);
        assert_eq!(
            bootstrap,
            [
                "bootstrap",
                "gui/501",
                "/Users/who/Library/LaunchAgents/io.river.familiar.plist"
            ]
        );
        assert_eq!(kickstart, ["kickstart", "-k", "gui/501/io.river.familiar"]);
        for word in [&bootout, &bootstrap, &kickstart].into_iter().flatten() {
            assert!(
                !word.contains("load"),
                "forbidden launchctl dialect: {word}"
            );
        }
    }
}
