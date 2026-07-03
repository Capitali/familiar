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
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use familiar_kernel::observation::Observation;
use serde::Deserialize;

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

/// A neighbour on the local link, as seen in the ARP cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Neighbor {
    /// Resolved name if the local resolver knew one (else `None`).
    pub name: Option<String>,
    pub ip: String,
    pub mac: String,
}

/// A DHCP lease as reported by the router's dnsmasq lease file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lease {
    pub mac: String,
    pub ip: String,
    /// The client-supplied hostname, or `None` when the lease has none (`*`).
    pub name: Option<String>,
}

/// **Discover the devices sharing this network** — perception, like discovering a camera:
/// knowing a phone or watch is present is not reaching into it. Two sources, merged by MAC so
/// each physical device is reported once with its best name:
/// - the local **ARP cache** (`arp -a`) — always permitted, local, emits no packets;
/// - the router's **DHCP lease table** — the authoritative roster that names the phones and
///   watches the ARP cache misses. Reading it is **outward reach** (SSH to another host), so
///   it only happens when `allow_network` is set *and* the human has pointed a `devices.json`
///   at their router. The DHCP hostname (e.g. "iPhone") is preferred over ARP's reverse-DNS
///   FQDN ("iphone.river.io"). Modern phones/watches randomise their MAC, noted as a hint.
pub fn devices(dir: &Path, now: i64, allow_network: bool) -> Vec<Observation> {
    use std::collections::BTreeMap;
    /// A device merged across sources, keyed by MAC.
    struct Rec {
        name: Option<String>,
        ip: String,
        via: Vec<&'static str>,
        randomized: bool,
    }
    let mut map: BTreeMap<String, Rec> = BTreeMap::new();

    // 1. Local ARP cache — always-allowed perception. `-n` keeps it numeric: plain `arp -a`
    //    does a reverse-DNS lookup per entry, which crawls on a slow uplink (Starlink) and
    //    would stall the tick. Names come from the DHCP lease table below; ARP just confirms
    //    presence on the link.
    if let Some(out) = run("arp", &["-a", "-n"]) {
        for n in parse_arp(&out) {
            let e = map.entry(n.mac.to_lowercase()).or_insert_with(|| Rec {
                name: None,
                ip: n.ip.clone(),
                via: Vec::new(),
                randomized: mac_randomized(&n.mac),
            });
            e.via.push("arp");
            e.ip = n.ip;
            if e.name.is_none() {
                e.name = n.name; // reverse-DNS FQDN, a fallback name
            }
        }
    }
    // 2. Router DHCP leases — outward reach, gated + configured.
    if allow_network {
        for l in roster_leases(dir) {
            let e = map.entry(l.mac.to_lowercase()).or_insert_with(|| Rec {
                name: None,
                ip: l.ip.clone(),
                via: Vec::new(),
                randomized: mac_randomized(&l.mac),
            });
            e.via.push("dhcp");
            e.ip = l.ip;
            if l.name.is_some() {
                e.name = l.name; // DHCP hostname preferred (shorter, human-set)
            }
        }
    }

    map.into_iter()
        .map(|(mac, r)| {
            let label = r.name.clone().unwrap_or_else(|| r.ip.clone());
            let mut ctx = format!("ip={} mac={} via={}", r.ip, mac, r.via.join("+"));
            if r.randomized {
                ctx.push_str(" randomized=true");
            }
            obs("host", "sees", format!("device:{label}"), ctx, now)
        })
        .collect()
}

/// The human-configured pointer to the network's DHCP authority. Read from `devices.json` in
/// the data dir: `{"router_host": "...", "leases_path": "..."}`.
#[derive(Debug, Clone, Deserialize)]
struct RosterConfig {
    router_host: String,
    leases_path: String,
}

/// Read the router's dnsmasq lease table over SSH — a **fixed, read-only** command shape
/// (`ssh <router_host> cat <leases_path>`), never an arbitrary command. Empty when no
/// `devices.json` is present (a clean no-op on any host not set up for it). The caller gates
/// this behind `allow_network`; the human enables it by writing the config (their consent).
fn roster_leases(dir: &Path) -> Vec<Lease> {
    let Ok(raw) = std::fs::read_to_string(dir.join("devices.json")) else {
        return Vec::new();
    };
    let Ok(cfg) = serde_json::from_str::<RosterConfig>(&raw) else {
        return Vec::new();
    };
    if cfg.router_host.trim().is_empty() || cfg.leases_path.trim().is_empty() {
        return Vec::new();
    }
    run(
        "ssh",
        &[
            "-o",
            "ConnectTimeout=6",
            "-o",
            "BatchMode=yes",
            cfg.router_host.trim(),
            "cat",
            cfg.leases_path.trim(),
        ],
    )
    .map(|out| parse_dnsmasq_leases(&out))
    .unwrap_or_default()
}

// --- pure helpers (tested) ---

/// Parse `arp -a` output into link neighbours, dropping the noise (incomplete entries,
/// broadcast/multicast, link-local, and the subnet broadcast address).
pub fn parse_arp(output: &str) -> Vec<Neighbor> {
    let mut out = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        // shape: "name (ip) at mac on iface ..."  (name is "?" when unresolved)
        let Some(open) = line.find(" (") else { continue };
        let Some(rel_close) = line[open..].find(')') else {
            continue;
        };
        let ip = &line[open + 2..open + rel_close];
        let name_tok = line[..open].trim();
        let after = &line[open + rel_close + 1..]; // " at mac on ..."
        let Some(at) = after.find(" at ") else { continue };
        let mac = after[at + 4..].split_whitespace().next().unwrap_or("");
        if mac.is_empty() || mac.starts_with('(') {
            continue; // "(incomplete)"
        }
        if is_noise_ip(ip) || is_noise_mac(mac) {
            continue;
        }
        let name = if name_tok == "?" || name_tok.is_empty() {
            None
        } else {
            Some(name_tok.to_string())
        };
        out.push(Neighbor {
            name,
            ip: ip.to_string(),
            mac: mac.to_string(),
        });
    }
    out
}

/// Parse a dnsmasq lease file: `<expiry> <mac> <ip> <hostname> <clientid>` per line, with a
/// trailing `duid` line ignored. A hostname of `*` means the client supplied none.
pub fn parse_dnsmasq_leases(output: &str) -> Vec<Lease> {
    let mut out = Vec::new();
    for line in output.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 4 || f[0] == "duid" {
            continue;
        }
        let (mac, ip, name) = (f[1], f[2], f[3]);
        if !mac.contains(':') || !ip.contains('.') {
            continue;
        }
        out.push(Lease {
            mac: mac.to_string(),
            ip: ip.to_string(),
            name: if name == "*" {
                None
            } else {
                Some(name.to_string())
            },
        });
    }
    out
}

/// True when a MAC is locally-administered (the 0x02 bit of the first octet) — the marker of
/// a randomised/private address, as modern phones and watches use per-network.
fn mac_randomized(mac: &str) -> bool {
    mac.split(':')
        .next()
        .and_then(|b| u8::from_str_radix(b, 16).ok())
        .map(|b| b & 0x02 != 0)
        .unwrap_or(false)
}

/// Broadcast / multicast / link-local / subnet-broadcast addresses aren't devices to serve.
fn is_noise_ip(ip: &str) -> bool {
    ip.ends_with(".255")
        || ip.starts_with("224.")
        || ip.starts_with("239.")
        || ip.starts_with("169.254.")
        || ip == "255.255.255.255"
}

/// Broadcast and multicast MACs aren't a device either.
fn is_noise_mac(mac: &str) -> bool {
    let m = mac.to_lowercase();
    m == "ff:ff:ff:ff:ff:ff" || m.starts_with("01:00:5e") || m.starts_with("33:33")
}

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
    fn parses_arp_and_drops_noise() {
        // Real macOS `arp -a` output from the RV network (names resolved by the router DNS).
        let fixture = "\
giiweo.river.io (192.168.108.1) at a0:ad:9f:ec:c1:b0 on en0 ifscope [ethernet]
wildhorse (192.168.108.10) at f8:ff:c2:49:a7:93 on en0 ifscope permanent [ethernet]
? (192.168.108.36) at (incomplete) on en0 ifscope [ethernet]
ipad.river.io (192.168.108.145) at d2:f6:f1:cd:52:5b on en0 ifscope [ethernet]
? (192.168.108.42) at 96:b3:58:2e:74:b1 on en0 ifscope [ethernet]
? (192.168.108.255) at ff:ff:ff:ff:ff:ff on en0 ifscope [ethernet]
mdns.mcast.net (224.0.0.251) at 1:0:5e:0:0:fb on en0 ifscope permanent [ethernet]";
        let n = parse_arp(fixture);
        // router, mac, ipad, and the unnamed .42 survive; incomplete/broadcast/multicast drop.
        assert_eq!(n.len(), 4);
        let ipad = n.iter().find(|x| x.ip == "192.168.108.145").unwrap();
        assert_eq!(ipad.name.as_deref(), Some("ipad.river.io"));
        let unnamed = n.iter().find(|x| x.ip == "192.168.108.42").unwrap();
        assert_eq!(unnamed.name, None);
        assert!(mac_randomized("96:b3:58:2e:74:b1"), "phone MAC is randomised");
        assert!(!mac_randomized("f8:ff:c2:49:a7:93"), "the Mac's burned-in MAC is not");
    }

    #[test]
    fn parses_dnsmasq_leases_naming_watch_and_phone() {
        // Real dnsmasq lease table from the RV router — it names the Watch and iPhone the
        // ARP cache never showed, and reports `*` for a client with no hostname.
        let fixture = "\
78178 7c:e9:13:9a:a1:b5 192.168.108.38 giiweoprime 01:7c:e9:13:9a:a1:b5
86400 d2:f6:f1:cd:52:5b 192.168.108.145 iPad 01:d2:f6:f1:cd:52:5b
74473 3e:03:ce:2c:d7:1b 192.168.108.41 Watch 01:3e:03:ce:2c:d7:1b
74473 96:b3:58:2e:74:b1 192.168.108.42 iPhone 01:96:b3:58:2e:74:b1
50298 d2:b9:67:6a:7a:b8 192.168.108.188 * 00:41:70:68:65:6c:69:6f:6e
duid 00:03:00:01:1a:69:a5:9e:66:6f";
        let leases = parse_dnsmasq_leases(fixture);
        assert_eq!(leases.len(), 5); // the duid line is skipped
        assert!(leases
            .iter()
            .any(|l| l.name.as_deref() == Some("Watch") && l.ip == "192.168.108.41"));
        assert!(leases
            .iter()
            .any(|l| l.name.as_deref() == Some("iPhone")));
        let anon = leases.iter().find(|l| l.ip == "192.168.108.188").unwrap();
        assert_eq!(anon.name, None); // "*" → None
    }

    #[test]
    fn devices_makes_no_outward_reach_without_config() {
        // No devices.json + network off → the DHCP source never runs, so no device is
        // reported via dhcp (only whatever the local ARP cache already held, if anything).
        let dir = std::env::temp_dir().join(format!("sense_roster_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let obs = devices(&dir, 1, false);
        assert!(
            obs.iter().all(|o| !o.context.contains("via=dhcp")),
            "no DHCP reach without config / with network closed"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn connectivity_yields_a_reading() {
        // no network assertion (offline CI is fine) — just that it produces a record
        let o = connectivity(1000);
        assert!(o.object.starts_with("connectivity:"));
    }
}
