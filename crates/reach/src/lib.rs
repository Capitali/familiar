//! Reach — assessing what the familiar could *extend into*.
//!
//! Discovery ([`familiar_sense::device_list`]) says *what is present*. Reach says *what we could do
//! with it*: could we install a native agent (SSH), only command it through a protocol (AirPlay,
//! Roku, MQTT, RTSP…), or merely observe that it exists? That classification is the input to the
//! consent-gated expansion — the familiar asks the human "extend into these?", and for the
//! agent-capable ones it can (with the human's own credentials) install an agent that joins the
//! mesh via the covenant handshake.
//!
//! **Perception vs reach, again.** Opening connections to *other* hosts to see what they speak is
//! outward reach, so the caller gates [`scan`]/[`assess`] behind `allow_network`; actively asking
//! DNS/mDNS what a neighbour calls itself is additionally gated by `allow_network_discovery`.
//! Probing is a bounded connect to a small allowlist of well-known ports — never an exploit, never
//! a payload; it learns only what a port scan or PTR query learns, the honest floor of "what could
//! I talk to, and what does the LAN call it?"

#![forbid(unsafe_code)]

use familiar_kernel::observation::Observation;
use familiar_sense::Device;
use std::io::Read;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

const SOURCE: &str = "reach";

/// How the familiar could extend to a device — the reach ladder, strongest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReachClass {
    /// We could install a native familiar agent here (it speaks SSH; given the human's access).
    AgentCapable,
    /// We can't run our code here, but we could *command* it through a protocol it speaks.
    ProtocolControllable,
    /// We can only see that it exists.
    ObservableOnly,
}

impl ReachClass {
    pub fn label(self) -> &'static str {
        match self {
            Self::AgentCapable => "agent-capable",
            Self::ProtocolControllable => "protocol-controllable",
            Self::ObservableOnly => "observable-only",
        }
    }
    fn rank(self) -> u8 {
        match self {
            Self::AgentCapable => 2,
            Self::ProtocolControllable => 1,
            Self::ObservableOnly => 0,
        }
    }
}

/// A well-known service the familiar recognizes on a probe, and the reach it implies.
pub struct Service {
    pub port: u16,
    pub name: &'static str,
    pub class: ReachClass,
}

/// The ports we probe. Deliberately small and honest: SSH means we could install a native agent
/// (with the human's credentials); the media/home/IoT protocols mean we could command the device
/// without installing; the rest is presence with a service hint.
pub const CATALOG: &[Service] = &[
    Service {
        port: 22,
        name: "ssh",
        class: ReachClass::AgentCapable,
    },
    Service {
        port: 62078,
        name: "ios-lockdown",
        class: ReachClass::ProtocolControllable,
    },
    Service {
        port: 8060,
        name: "roku-ecp",
        class: ReachClass::ProtocolControllable,
    },
    Service {
        port: 7000,
        name: "airplay",
        class: ReachClass::ProtocolControllable,
    },
    Service {
        port: 5000,
        name: "airplay-rtsp",
        class: ReachClass::ProtocolControllable,
    },
    Service {
        port: 1883,
        name: "mqtt",
        class: ReachClass::ProtocolControllable,
    },
    Service {
        port: 8883,
        name: "mqtt-tls",
        class: ReachClass::ProtocolControllable,
    },
    Service {
        port: 554,
        name: "rtsp",
        class: ReachClass::ProtocolControllable,
    },
    Service {
        port: 9100,
        name: "printer",
        class: ReachClass::ProtocolControllable,
    },
    Service {
        port: 32400,
        name: "plex",
        class: ReachClass::ProtocolControllable,
    },
    Service {
        port: 445,
        name: "smb",
        class: ReachClass::ObservableOnly,
    },
    Service {
        port: 548,
        name: "afp",
        class: ReachClass::ObservableOnly,
    },
    Service {
        port: 80,
        name: "http",
        class: ReachClass::ObservableOnly,
    },
    Service {
        port: 443,
        name: "https",
        class: ReachClass::ObservableOnly,
    },
];

/// A device with its assessed reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceReach {
    pub label: String,
    pub ip: String,
    /// Service names found open, in catalog order.
    pub open: Vec<&'static str>,
    pub class: ReachClass,
}

pub(crate) fn port_open(ip: &str, port: u16, timeout: Duration) -> bool {
    match format!("{ip}:{port}").to_socket_addrs() {
        Ok(mut addrs) => addrs
            .next()
            .map(|a| TcpStream::connect_timeout(&a, timeout).is_ok())
            .unwrap_or(false),
        Err(_) => false,
    }
}

/// Run one bounded `dig` lookup. DNS is an outward query and may stall when the LAN's
/// resolver is sick, so the same per-host timeout as the reach probes is a hard wall.
fn dig(ip: &str, mdns: bool, timeout: Duration) -> Option<String> {
    let mut command = Command::new("dig");
    command.args(["+short", "+time=1", "+tries=1"]);
    if mdns {
        command.args(["@224.0.0.251", "-p", "5353"]);
    }
    let mut child = command
        .args(["-x", ip])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let mut output = String::new();
                child.stdout.take()?.read_to_string(&mut output).ok()?;
                return clean_resolved_name(&output, ip);
            }
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// Ask the LAN's normal reverse resolver, then the mDNS multicast resolver directly.
/// Best-effort: an absent `dig`, a timeout, or no PTR leaves the numeric label unchanged.
fn reverse_name(ip: &str, timeout: Duration) -> Option<String> {
    dig(ip, false, timeout).or_else(|| dig(ip, true, timeout))
}

/// `dig +short` returns one PTR per line, normally with a trailing dot. The SystemName
/// ladder wants the host label, not its search domain; Bonjour escapes spaces as `\032`.
fn clean_resolved_name(output: &str, ip: &str) -> Option<String> {
    let raw = output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    let name = raw
        .trim_end_matches('.')
        .split('.')
        .next()
        .unwrap_or("")
        .replace("\\032", " ");
    let name = name.trim();
    if name.is_empty()
        || name.eq_ignore_ascii_case("localhost")
        || name == ip
        || name.len() > 63
        || name
            .chars()
            .all(|c| c.is_ascii_hexdigit() || matches!(c, ':' | '.' | '-'))
    {
        None
    } else {
        Some(name.to_string())
    }
}

/// Probe one device's IP against the catalog, returning what's open + the strongest reach class.
pub fn assess_device(label: &str, ip: &str, timeout: Duration) -> DeviceReach {
    assess_device_with(port_open, label, ip, timeout)
}

/// The probe is a parameter so tests can pin what the network says: on a shared CI runner
/// anything bound to 0.0.0.0 (the runner's own sshd) answers for every loopback address,
/// so probing a "silent" 127.0.0.2 there finds a listener and a ghost ranks agent-capable.
fn assess_device_with(
    probe: fn(&str, u16, Duration) -> bool,
    label: &str,
    ip: &str,
    timeout: Duration,
) -> DeviceReach {
    let mut open = Vec::new();
    let mut class = ReachClass::ObservableOnly;
    for svc in CATALOG {
        if probe(ip, svc.port, timeout) {
            open.push(svc.name);
            if svc.class.rank() > class.rank() {
                class = svc.class;
            }
        }
    }
    DeviceReach {
        label: label.to_string(),
        ip: ip.to_string(),
        open,
        class,
    }
}

/// Assess reach across a set of devices — **outward reach**, so the caller gates this behind
/// `allow_network`. When the separate discovery gate is open, a still-numeric label gets one
/// bounded local-DNS/mDNS reverse lookup. Returns the reach records and observations
/// (`host can-reach device:<label>` tagged with the class + open services) for the store.
pub fn assess(
    devices: &[Device],
    now: i64,
    timeout_ms: u64,
    resolve_names: bool,
) -> (Vec<DeviceReach>, Vec<Observation>) {
    assess_with(
        port_open,
        reverse_name,
        devices,
        now,
        timeout_ms,
        resolve_names,
    )
}

fn assess_with(
    probe: fn(&str, u16, Duration) -> bool,
    resolver: fn(&str, Duration) -> Option<String>,
    devices: &[Device],
    now: i64,
    timeout_ms: u64,
    resolve_names: bool,
) -> (Vec<DeviceReach>, Vec<Observation>) {
    let timeout = Duration::from_millis(timeout_ms.max(1));
    let mut reaches = Vec::new();
    let mut observations = Vec::new();
    for d in devices {
        if d.ip.is_empty() {
            continue;
        }
        // DHCP / an earlier discovery already gave the stronger name. Ask only for a
        // numeric label, and only under the separately human-opened discovery gate.
        let label = if resolve_names && d.label == d.ip {
            resolver(&d.ip, timeout).unwrap_or_else(|| d.label.clone())
        } else {
            d.label.clone()
        };
        let r = assess_device_with(probe, &label, &d.ip, timeout);
        let ctx = format!(
            "class={} open={} ip={}",
            r.class.label(),
            if r.open.is_empty() {
                "-".to_string()
            } else {
                r.open.join(",")
            },
            r.ip
        );
        observations.push(Observation::new(
            "host",
            "can-reach",
            format!("device:{}", r.label),
            ctx,
            SOURCE,
            now,
            0.9,
        ));
        reaches.push(r);
    }
    (reaches, observations)
}

/// Discover devices then assess their reach, in one call. The caller supplies the independent
/// network and active-discovery gates; name lookup occurs only when both are open.
pub fn scan(
    dir: &Path,
    now: i64,
    allow_network: bool,
    allow_network_discovery: bool,
    timeout_ms: u64,
) -> (Vec<DeviceReach>, Vec<Observation>) {
    let devices = familiar_sense::device_list(dir, allow_network);
    assess(
        &devices,
        now,
        timeout_ms,
        allow_network && allow_network_discovery,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn device(label: &str, ip: &str) -> Device {
        Device {
            label: label.into(),
            ip: ip.into(),
            mac: "aa:bb:cc:dd:ee:ff".into(),
            via: vec!["arp".into()],
            randomized: false,
        }
    }

    #[test]
    fn class_ranking_takes_the_strongest_reach() {
        assert!(ReachClass::AgentCapable.rank() > ReachClass::ProtocolControllable.rank());
        assert!(ReachClass::ProtocolControllable.rank() > ReachClass::ObservableOnly.rank());
    }

    #[test]
    fn port_open_detects_a_listener_and_not_a_closed_port() {
        // Bind an ephemeral loopback port; probing it must succeed. A port with nothing on it
        // (the ephemeral +1, extremely unlikely to be bound) must not. Host-independent, unlike
        // probing the real catalog against 127.0.0.1 (this host has its own services open).
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
        let port = listener.local_addr().unwrap().port();
        assert!(
            port_open("127.0.0.1", port, Duration::from_millis(300)),
            "listener is reachable"
        );
        drop(listener);
        // 127.0.0.2 has nothing listening on loopback → closed.
        assert!(!port_open("127.0.0.2", port, Duration::from_millis(100)));
    }

    // A probe that hears nothing anywhere — what the real network answers is some
    // other machine's fact, not this test's.
    fn deaf(_ip: &str, _port: u16, _t: Duration) -> bool {
        false
    }

    #[test]
    fn a_silent_host_is_observable_only() {
        let r = assess_device_with(deaf, "ghost", "127.0.0.2", Duration::from_millis(100));
        assert_eq!(r.class, ReachClass::ObservableOnly);
        assert!(r.open.is_empty());
    }

    #[test]
    fn an_ssh_speaker_is_agent_capable() {
        fn ssh_only(_ip: &str, port: u16, _t: Duration) -> bool {
            port == 22
        }
        let r = assess_device_with(ssh_only, "box", "192.0.2.9", Duration::from_millis(100));
        assert_eq!(r.class, ReachClass::AgentCapable);
        assert_eq!(r.open, vec!["ssh"]);
    }

    #[test]
    fn assess_emits_tagged_observations() {
        let (_reaches, obs) =
            assess_with(deaf, no_name, &[device("x", "127.0.0.2")], 42, 100, true);
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].source, "reach");
        assert_eq!(obs[0].action, "can-reach");
        assert!(obs[0].object.starts_with("device:x"));
        assert!(obs[0].context.contains("class=observable-only"));
    }

    fn no_name(_ip: &str, _timeout: Duration) -> Option<String> {
        None
    }

    fn codex_name(ip: &str, _timeout: Duration) -> Option<String> {
        (ip == "192.168.108.42").then(|| "codex".to_string())
    }

    #[test]
    fn a_gated_reverse_lookup_names_a_numeric_neighbor_before_it_reaches_the_frontier() {
        let numeric = device("192.168.108.42", "192.168.108.42");
        let (reaches, obs) = assess_with(deaf, codex_name, &[numeric.clone()], 42, 100, true);
        assert_eq!(reaches[0].label, "codex");
        assert_eq!(obs[0].object, "device:codex");
        assert!(obs[0].context.contains("ip=192.168.108.42"));

        let (closed_reaches, closed_obs) =
            assess_with(deaf, codex_name, &[numeric], 42, 100, false);
        assert_eq!(closed_reaches[0].label, "192.168.108.42");
        assert_eq!(closed_obs[0].object, "device:192.168.108.42");

        let already_named = device("dhcp-name", "192.168.108.42");
        let (named_reaches, _) = assess_with(deaf, codex_name, &[already_named], 42, 100, true);
        assert_eq!(
            named_reaches[0].label, "dhcp-name",
            "an authoritative name is never replaced by a reverse answer"
        );
    }

    #[test]
    fn ptr_output_becomes_a_host_label_and_numeric_or_local_answers_name_nothing() {
        assert_eq!(
            clean_resolved_name("Codex\\032iPad.local.\n", "192.168.108.42"),
            Some("Codex iPad".to_string())
        );
        assert_eq!(
            clean_resolved_name("mac.river.io.\n", "192.168.108.3"),
            Some("mac".to_string())
        );
        assert_eq!(
            clean_resolved_name("192.168.108.42.\n", "192.168.108.42"),
            None
        );
        assert_eq!(clean_resolved_name("localhost.\n", "127.0.0.1"), None);
    }
}
