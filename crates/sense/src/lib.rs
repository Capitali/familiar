//! Sense — the factory perceiving the host it lives on.
//!
//! Turns the local environment, its interfaces, and its capabilities into
//! observations (the only truth). This is the *Observe* step pointed at the host
//! itself — the precondition of serving anywhere.
//!
//! **Perception vs reach.** Perceiving the local host is always permitted — you
//! cannot serve what you cannot see — and is done here over a *fixed allowlist* of
//! read-only system commands (no arbitrary execution). *Outward* reach — the
//! connectivity probe, which touches the network — is boundary-gated by the caller
//! through the obedience guard.
//!
//! Returned observations have empty ids; the caller assigns them on record.

use std::net::{TcpStream, ToSocketAddrs};
use std::process::Command;
use std::time::Duration;

use familiar_kernel::observation::Observation;

const SENSE_CONF: f64 = 0.95;
const SOURCE: &str = "sensor";

/// A reasonable set of tools whose presence describes "what this host can do".
pub const DEFAULT_TOOLS: &[&str] = &[
    "git", "python3", "cargo", "rustc", "node", "npm", "docker", "ssh", "curl", "brew", "gh", "jq",
    "sqlite3", "make", "cc",
];

fn obs(actor: &str, action: &str, object: String, context: String, now: i64) -> Observation {
    Observation::new(actor, action, object, context, SOURCE, now, SENSE_CONF)
}

/// Run a read-only command from the allowlist; trimmed stdout if it succeeded.
fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Local self-census: OS, kernel, arch, hostname, CPU, memory. Always permitted
/// (perception). Best-effort — records what it can perceive, skips what it cannot.
pub fn census(now: i64) -> Vec<Observation> {
    let mut out = Vec::new();

    let os = run("uname", &["-s"]).unwrap_or_else(|| "unknown".into());
    let kernel = run("uname", &["-r"]).unwrap_or_default();
    let arch = run("uname", &["-m"]).unwrap_or_default();
    out.push(obs(
        "local_hardware",
        "reports",
        format!("os:{os}"),
        format!("kernel={kernel} arch={arch}"),
        now,
    ));

    if let Some(host) = run("uname", &["-n"]) {
        out.push(obs(
            "host",
            "named",
            format!("hostname:{host}"),
            String::new(),
            now,
        ));
    }

    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(0);
    let brand = run("sysctl", &["-n", "machdep.cpu.brand_string"]).unwrap_or_default();
    out.push(obs(
        "local_hardware",
        "reports",
        format!("cpu:{cores}cores"),
        format!("brand={brand}"),
        now,
    ));

    // memory: macOS hw.memsize (bytes) or Linux /proc/meminfo
    let mem_bytes = run("sysctl", &["-n", "hw.memsize"])
        .and_then(|s| s.parse::<u64>().ok())
        .or_else(|| read_linux_memtotal_kib().map(|kib| kib * 1024));
    if let Some(bytes) = mem_bytes {
        out.push(obs(
            "local_hardware",
            "reports",
            format!("memory:{}", format_gib(bytes)),
            String::new(),
            now,
        ));
    }

    out
}

/// Network interface names the host exposes (local introspection, not egress).
pub fn interfaces(now: i64) -> Vec<Observation> {
    let names = run("ifconfig", &["-l"])
        .map(|s| parse_ifconfig_l(&s))
        .or_else(read_linux_net_ifaces)
        .unwrap_or_default();
    names
        .into_iter()
        .map(|n| obs("host", "has", format!("interface:{n}"), String::new(), now))
        .collect()
}

/// Which allowlisted tools are present — the host's capabilities.
pub fn capabilities(now: i64, tools: &[&str]) -> Vec<Observation> {
    let mut out = Vec::new();
    for &tool in tools {
        if let Some(path) = run("sh", &["-c", &format!("command -v {tool}")]) {
            out.push(obs(
                "host",
                "can_run",
                format!("tool:{tool}"),
                format!("path={path}"),
                now,
            ));
        }
    }
    out
}

/// Connectivity probe — **outward reach**; the caller must guard-gate this (Network).
/// Connects to a well-known address:port with a short timeout; no DNS, no payload.
pub fn connectivity(now: i64) -> Observation {
    let online = ("1.1.1.1:443")
        .to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
        .map(|addr| TcpStream::connect_timeout(&addr, Duration::from_secs(3)).is_ok())
        .unwrap_or(false);
    obs(
        "host",
        "reports",
        format!("connectivity:{}", if online { "online" } else { "offline" }),
        String::new(),
        now,
    )
}

/// A closer look at the host's network configuration — default gateway, DNS resolvers,
/// and how many TCP ports are listening. Read-only and best-effort (records what it can
/// perceive on this OS, skips the rest), so an analysis of "network config" can rest on
/// verified facts rather than guesses. macOS (`route`) and Linux (`ip route`) variants.
pub fn network_detail(now: i64) -> Vec<Observation> {
    let mut out = Vec::new();

    let gw = run("route", &["-n", "get", "default"]) // macOS
        .and_then(|s| {
            s.lines().find_map(|l| {
                l.trim()
                    .strip_prefix("gateway:")
                    .map(|g| g.trim().to_string())
            })
        })
        .or_else(|| {
            run(
                "sh",
                &[
                    "-c",
                    "ip route show default 2>/dev/null | awk '{print $3; exit}'",
                ],
            )
        })
        .filter(|g| !g.is_empty());
    if let Some(gw) = gw {
        out.push(obs(
            "network",
            "reports",
            format!("default_gateway:{gw}"),
            String::new(),
            now,
        ));
    }

    if let Ok(resolv) = std::fs::read_to_string("/etc/resolv.conf") {
        let servers: Vec<&str> = resolv
            .lines()
            .filter_map(|l| l.trim().strip_prefix("nameserver ").map(str::trim))
            .collect();
        if !servers.is_empty() {
            out.push(obs(
                "network",
                "reports",
                format!("dns_servers:{}", servers.join(",")),
                String::new(),
                now,
            ));
        }
    }

    if let Some(n) = run("sh", &["-c", "netstat -an 2>/dev/null | grep -c LISTEN"])
        .and_then(|s| s.trim().parse::<u32>().ok())
    {
        out.push(obs(
            "network",
            "reports",
            format!("listening_ports:{n}"),
            String::new(),
            now,
        ));
    }

    out
}

// --- pure helpers (tested) ---

/// Parse macOS `ifconfig -l` output (space-separated interface names).
pub fn parse_ifconfig_l(s: &str) -> Vec<String> {
    s.split_whitespace().map(|t| t.to_string()).collect()
}

/// Format bytes as a rounded GiB string, e.g. "16GB".
pub fn format_gib(bytes: u64) -> String {
    let gib = (bytes as f64) / (1024.0 * 1024.0 * 1024.0);
    format!("{}GB", gib.round() as u64)
}

fn read_linux_memtotal_kib() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            return rest.split_whitespace().next()?.parse::<u64>().ok();
        }
    }
    None
}

fn read_linux_net_ifaces() -> Option<Vec<String>> {
    let mut names = Vec::new();
    for e in std::fs::read_dir("/sys/class/net").ok()?.flatten() {
        names.push(e.file_name().to_string_lossy().to_string());
    }
    if names.is_empty() {
        None
    } else {
        Some(names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ifconfig_l() {
        assert_eq!(
            parse_ifconfig_l("lo0 en0 en1  awdl0\n"),
            vec!["lo0", "en0", "en1", "awdl0"]
        );
        assert!(parse_ifconfig_l("   ").is_empty());
    }

    #[test]
    fn formats_memory() {
        assert_eq!(format_gib(16 * 1024 * 1024 * 1024), "16GB");
        assert_eq!(format_gib(0), "0GB");
    }

    #[test]
    fn census_perceives_something() {
        // census is best-effort but should always yield at least the OS line
        let o = census(1000);
        assert!(!o.is_empty());
        assert!(o.iter().any(|x| x.object.starts_with("os:")));
        assert!(o.iter().all(|x| x.source == "sensor" && x.ts == 1000));
    }

    #[test]
    fn connectivity_yields_a_reading() {
        // no network assertion (offline CI is fine) — just that it produces a record
        let o = connectivity(1000);
        assert!(o.object.starts_with("connectivity:"));
    }
}
