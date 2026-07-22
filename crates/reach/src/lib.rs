//! Reach — assessing what the familiar could *extend into*.
//!
//! Discovery ([`familiar_sense::device_list`]) says *what is present*. Reach says *what we could do
//! with it*: could we install a native agent (SSH), only command it through a protocol (AirPlay,
//! Roku, MQTT, RTSP…), or merely observe that it exists? That classification is the input to the
//! consent-gated expansion — the familiar asks the human "extend into these?", and for the
//! agent-capable ones it can (with the human's own credentials) install an agent that joins the
//! mesh via the covenant handshake.
//!
//! **Perception vs reach, again.** Opening TCP connections to *other* hosts to see what they speak
//! is outward reach, so the caller gates [`scan`]/[`assess`] behind `allow_network`. Probing is a
//! bounded connect to a small allowlist of well-known ports — never an exploit, never a payload;
//! it learns only what a port-scan learns, which is the honest floor of "what could I talk to."
//! HomeKit/AirPlay-2 advertise on mDNS random ports and aren't reliably port-probed — that richer
//! discovery is the next increment (mDNS); this cut is the dependency-free TCP floor.

#![forbid(unsafe_code)]

use familiar_kernel::observation::Observation;
use familiar_sense::Device;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
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

/// Probe one device's IP against the catalog, returning what's open + the strongest reach class.
pub fn assess_device(label: &str, ip: &str, timeout: Duration) -> DeviceReach {
    let mut open = Vec::new();
    let mut class = ReachClass::ObservableOnly;
    for svc in CATALOG {
        if port_open(ip, svc.port, timeout) {
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
/// `allow_network`. Returns the reach records and observations (`host can-reach device:<label>`
/// tagged with the class + open services) for the store.
pub fn assess(
    devices: &[Device],
    now: i64,
    timeout_ms: u64,
) -> (Vec<DeviceReach>, Vec<Observation>) {
    let timeout = Duration::from_millis(timeout_ms.max(1));
    let mut reaches = Vec::new();
    let mut observations = Vec::new();
    for d in devices {
        if d.ip.is_empty() {
            continue;
        }
        let r = assess_device(&d.label, &d.ip, timeout);
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
            format!("device:{}", d.label),
            ctx,
            SOURCE,
            now,
            0.9,
        ));
        reaches.push(r);
    }
    (reaches, observations)
}

/// Discover devices then assess their reach, in one call. Gated by the caller (`allow_network`).
pub fn scan(
    dir: &Path,
    now: i64,
    allow_network: bool,
    timeout_ms: u64,
) -> (Vec<DeviceReach>, Vec<Observation>) {
    let devices = familiar_sense::device_list(dir, allow_network);
    assess(&devices, now, timeout_ms)
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

    #[test]
    fn a_silent_host_is_observable_only() {
        // 127.0.0.2 with nothing listening → no catalog port open → observable-only.
        let r = assess_device("ghost", "127.0.0.2", Duration::from_millis(100));
        assert_eq!(r.class, ReachClass::ObservableOnly);
        assert!(r.open.is_empty());
    }

    #[test]
    fn assess_emits_tagged_observations() {
        let (_reaches, obs) = assess(&[device("x", "127.0.0.2")], 42, 100);
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].source, "reach");
        assert_eq!(obs[0].action, "can-reach");
        assert!(obs[0].object.starts_with("device:x"));
        assert!(obs[0].context.contains("class=observable-only"));
    }
}
