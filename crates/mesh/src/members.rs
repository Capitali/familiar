//! Mesh membership, classified into one accurate, non-overlapping picture.
//!
//! Every participant in the mesh sits at exactly one layer (the OSI-style hierarchy the counts must
//! honor): the local familiar itself (**self**), full **gossip peers** that exchange briefs, member
//! devices that read the worldview (**device peers**), and member devices that only push
//! observations (**device agents**). This module is the single source of that classification — both
//! the macOS Glass and the `/mesh/worldview` snapshot render from it, so they never disagree.

use crate::group;
use crate::transport::{self, PeerRecord};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// A participant's layer in the mesh. Mutually exclusive — a node is counted once, as itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberKind {
    /// This familiar — the node you're looking through.
    SelfNode,
    /// A full node that exchanges signed briefs both ways.
    GossipPeer,
    /// A member device that reads the worldview (a console, e.g. an iPad) — a full participant.
    DevicePeer,
    /// A member device that only pushes observations (a sensor, e.g. a phone or watch).
    DeviceAgent,
}

/// One classified mesh participant, with enough to render a roster row or a node in a graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Member {
    pub node_id: String,
    pub label: String,
    pub kind: MemberKind,
    /// OS family — from the peer's brief ("linux"/"macos") or derived from a device's actor
    /// namespace ("iOS"/"iPadOS"/"watchOS"). Empty when unknown.
    pub os: String,
    /// OS release detail ("iPadOS 26.1", "Ubuntu 24.04") when the node reports it. Empty otherwise.
    #[serde(default)]
    pub os_version: String,
    /// The device's actor namespace where applicable (`phone:ian`, `ipad:ian`, `watch:ian`).
    pub actor: String,
    /// Latest thing this member did/reported — a one-line status.
    pub detail: String,
    /// When first seen (unix secs) — the "date joined". 0 if unknown.
    pub first_seen: i64,
    /// When last seen (unix secs).
    pub last_seen: i64,
    /// Present if seen within the freshness window.
    pub online: bool,
    /// The familiar build the node runs (gossip peers). Empty for devices.
    #[serde(default)]
    pub familiar_version: String,
    /// Tools this peer offers to the mesh.
    #[serde(default)]
    pub tools: usize,
    /// Distilled patterns this peer offers.
    #[serde(default)]
    pub patterns: usize,
    /// Where it connected from / its address (display only).
    #[serde(default)]
    pub addr: String,
    /// The relationship — how this node participates: "self", "gossip", "reads worldview",
    /// "sensor (direct)", "sensor (via phone)". Human-readable for the roster.
    #[serde(default)]
    pub relationship: String,
    /// This node has direct local / context-specific AI access (can reason locally): the self node
    /// with `allow_llm`, or a device that has reasoned (posted a `theorizes` observation — e.g. the
    /// iPad's on-device Apple Intelligence). Badged in the roster + mesh map.
    #[serde(default)]
    pub ai: bool,
    /// The graduated trust the familiar holds this member at — "trusted" (normal), "throttled"
    /// (directives paused), "marginalized" (content ignored), or "severed" (briefs dropped, revoke
    /// recommended). Reversible; derived from the corruption-awareness score. See `corruption::Trust`.
    #[serde(default)]
    pub trust: String,
    /// Liveness as a word — "online" (inside its kind's freshness window), "away" (missed
    /// its cadence but recent), "offline" (gone). Derived at classify time from `last_seen`,
    /// so a phone in airplane mode decays on the mesh's real cadence, not a 10-minute grace.
    #[serde(default)]
    pub status: String,
    /// When the current continuous-online run began (unix secs). 0 when offline/unknown.
    #[serde(default)]
    pub session_start: i64,
    /// Cumulative seconds this member has spent online in the mesh, live session included.
    #[serde(default)]
    pub total_online_secs: i64,
    /// A human can interact at this node's console (false = headless / sensor-only).
    #[serde(default)]
    pub interactive: bool,
    /// The human that node serves, when shared/derivable ("ian"). Empty when none/unknown.
    #[serde(default)]
    pub human: String,
    /// Where the node is (decimal degrees) — self from `transport::self_geo`, peers from their
    /// briefs, devices from the GPS they report on worldview reads. 0/0 = unknown, and the map
    /// says so rather than inventing a place.
    #[serde(default)]
    pub lat: f64,
    #[serde(default)]
    pub lon: f64,
}

/// Liveness thresholds, per member kind: a gossip peer beacons every ~30s, so two missed
/// rounds means it's off; device consoles/sensors report on change and get a longer leash.
/// Beyond `away`, it's offline. (secs)
fn status_of(kind: MemberKind, age: i64) -> &'static str {
    let (online, away) = match kind {
        MemberKind::SelfNode => return "online",
        MemberKind::GossipPeer => (transport::GOSSIP_FRESH_SECS, 3600),
        MemberKind::DevicePeer | MemberKind::DeviceAgent => (DEVICE_FRESH_SECS, 3600),
    };
    if age <= online {
        "online"
    } else if age <= away {
        "away"
    } else {
        "offline"
    }
}

/// A device counts as present if it was seen within this window. Generous, because device agents
/// report on change (they can be quiet for a while yet still be "here").
pub const ONLINE_WINDOW_SECS: i64 = 600;
/// The window for a device to still read **"online"** in the roster — tighter than
/// [`ONLINE_WINDOW_SECS`] (which still governs session continuity), so a phone that drops
/// off the network (airplane mode) visibly decays within minutes, not ten.
pub const DEVICE_FRESH_SECS: i64 = 180;
/// A device agent older than this has departed — dropped from the roster.
const AGENT_FRESH_SECS: i64 = 6 * 3600;

/// Known device actor namespaces — the prefix before the ':' in a device's actor (`ipad:ian`).
/// Only observations under one of these count as *device* reports; a peer's own cycle actors
/// (`familiar`), human actors (`ian`), or gossip presence (`mesh:…`) do NOT make a node a device.
/// This is what keeps a headless gossip peer (whose replicated `familiar` observations arrive tagged
/// `mesh:<node>`) from being misread as a device peer.
const DEVICE_NAMESPACES: &[&str] = &[
    "phone", "iphone", "ipad", "watch", "mac", "tv", "appletv", "roku", "android", "tablet",
    "tizen", "wearable", "windows", "linux",
];

fn is_device_actor(actor: &str) -> bool {
    match actor.split_once(':') {
        Some((ns, _)) => DEVICE_NAMESPACES.contains(&ns),
        None => false,
    }
}

/// The latest device report per node id: `node -> (actor, object, ts)`, over **device-namespace**
/// actors only (`phone:`/`ipad:`/`watch:`/`tv:`…). Non-device actors (a peer's `familiar` cycle,
/// human `ian`, gossip `mesh:*`) are ignored so a gossip peer isn't misclassified as a device.
fn device_reports(
    obs: &[familiar_kernel::observation::Observation],
) -> HashMap<String, (String, String, i64)> {
    let mut latest: HashMap<String, (String, String, i64)> = HashMap::new();
    for o in obs {
        let Some(node) = o.source.strip_prefix("mesh:") else {
            continue;
        };
        if !is_device_actor(&o.actor) {
            continue; // not a device-sensor report (peer cycle / human / gossip presence)
        }
        let e = latest
            .entry(node.to_string())
            .or_insert((String::new(), String::new(), 0));
        if o.ts >= e.2 {
            *e = (o.actor.clone(), o.object.clone(), o.ts);
        }
    }
    latest
}

/// OS family from a device actor namespace (`ipad:ian` → "iPadOS"). Empty if not a known device.
pub fn os_from_actor(actor: &str) -> String {
    let ns = actor.split(':').next().unwrap_or("");
    match ns {
        "ipad" => "iPadOS",
        "phone" | "iphone" => "iOS",
        "watch" => "watchOS",
        "mac" => "macOS",
        _ => "",
    }
    .to_string()
}

/// Classify every mesh participant at `now`. Reads the peer roster + the observation log; needs no
/// network. The self node comes first, then peers, then agents — each exactly once.
pub fn classify(dir: &Path, now: i64) -> Vec<Member> {
    let mut out = Vec::new();

    // Self — this instance of the familiar. Peers are equals: it is named by its host (like every
    // other peer's brief label), NOT by the group or a privileged "familiar" name. `SelfNode` marks
    // "you are here"; it confers no special standing.
    if let Ok(Some(cred)) = group::load(dir) {
        let label = crate::node::NodeKey::load_or_mint(dir, "")
            .ok()
            .map(|n| n.identity().label)
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| cred.membership.node_id.chars().take(8).collect());
        let obs = familiar_kernel::observation::load(dir).unwrap_or_default();
        let first = obs.iter().map(|o| o.ts).min().unwrap_or(now);
        let last = obs.iter().map(|o| o.ts).max().unwrap_or(now);
        let tools = familiar_kernel::tool::load(dir)
            .map(|t| t.len())
            .unwrap_or(0);
        let patterns = familiar_kernel::pattern_memory::load(dir)
            .map(|p| p.len())
            .unwrap_or(0);
        let self_ai = familiar_kernel::boundary::load(dir)
            .map(|b| b.allow_llm)
            .unwrap_or(false);
        let cfg = crate::config::load(dir).unwrap_or_default();
        let (self_lat, self_lon) = transport::self_geo(dir).unwrap_or((0.0, 0.0));
        out.push(Member {
            node_id: cred.membership.node_id.clone(),
            label,
            kind: MemberKind::SelfNode,
            os: os_pretty(std::env::consts::OS),
            os_version: crate::merge::os_release(),
            actor: String::new(),
            detail: format!("this node · v{}", env!("CARGO_PKG_VERSION")),
            first_seen: first,
            last_seen: last,
            online: true,
            familiar_version: env!("CARGO_PKG_VERSION").to_string(),
            tools,
            patterns,
            addr: "localhost".into(),
            relationship: "self".into(),
            ai: self_ai,
            trust: "trusted".into(),
            status: "online".into(),
            session_start: 0,
            total_online_secs: 0,
            interactive: !cfg.headless,
            human: familiar_kernel::identity::current(dir).unwrap_or_default(),
            lat: self_lat,
            lon: self_lon,
        });
    }

    let obs = familiar_kernel::observation::load(dir).unwrap_or_default();
    let reports = device_reports(&obs);
    // The graduated trust tier per actor (monitor → throttle → marginalize → sever), from the shared
    // refusal log. Surfaced so the roster/map can badge a peer whose standing has slipped.
    let refusals = familiar_kernel::corruption::load(dir).unwrap_or_default();
    // Nodes with direct local / context AI: anyone who has posted a `theorizes` observation reasoned
    // locally (the iPad's on-device Apple Intelligence, a headless peer with an LLM adapter, …).
    let ai_nodes: std::collections::HashSet<String> = obs
        .iter()
        .filter(|o| o.action == "theorizes")
        .map(|o| o.actor.clone())
        .collect();
    let ai_node =
        |node_id: &str, actor: &str| ai_nodes.contains(actor) || ai_nodes.contains(node_id);
    let peers: Vec<PeerRecord> = transport::load_peers(dir);
    let peer_ids: std::collections::HashSet<&str> =
        peers.iter().map(|p| p.node_id.as_str()).collect();
    // Best-probable names for peers on the tailnet: their Tailscale hostname, keyed by IP.
    let tailnet: std::collections::HashMap<String, String> = transport::enumerate_peers()
        .into_iter()
        .map(|t| (t.ip, t.host))
        .collect();

    for p in &peers {
        let ip = p.addr.split(':').next().unwrap_or("").to_string();
        let is_device = reports.contains_key(&p.node_id);
        let (kind, os, actor, relationship) = if let Some((actor, _, _)) = reports.get(&p.node_id) {
            (
                MemberKind::DevicePeer,
                os_from_actor(actor),
                actor.clone(),
                "reads worldview".to_string(),
            )
        } else {
            (
                MemberKind::GossipPeer,
                os_pretty(&p.os),
                String::new(),
                "gossip peer".to_string(),
            )
        };
        // Prefer a resolved tailnet hostname for gossip peers; keep the device's own label otherwise.
        let label = if !is_device {
            tailnet
                .get(&ip)
                .cloned()
                .filter(|h| !h.is_empty())
                .unwrap_or_else(|| p.label.clone())
        } else {
            p.label.clone()
        };
        let detail = if is_device {
            reports
                .get(&p.node_id)
                .map(|(_, o, _)| o.clone())
                .unwrap_or_default()
        } else {
            let v = if p.familiar_version.is_empty() {
                String::new()
            } else {
                format!("v{} · ", p.familiar_version)
            };
            format!(
                "{v}{} tool(s), {} pattern(s)",
                p.tools_offered, p.patterns_offered
            )
        };
        let has_ai = ai_node(&p.node_id, &actor);
        let trust =
            familiar_kernel::corruption::trust(&refusals, &format!("mesh:{}", p.node_id), now)
                .label()
                .to_string();
        let status = status_of(kind, now - p.last_seen);
        // The human at that node: what its brief shared, else the device's actor namespace
        // (`ipad:ian` → "ian" — the device is inherently a human's console).
        let human = if !p.human.is_empty() {
            p.human.clone()
        } else {
            actor.split_once(':').map(|(_, h)| h.to_string()).unwrap_or_default()
        };
        // Cumulative online time, live session included while it's still fresh.
        let live = if status == "online" && p.session_start > 0 {
            (now - p.session_start).max(0)
        } else if p.session_start > 0 {
            (p.last_seen - p.session_start).max(0)
        } else {
            0
        };
        out.push(Member {
            node_id: p.node_id.clone(),
            label,
            kind,
            os,
            os_version: p.os_version.clone(),
            actor,
            detail,
            first_seen: p.first_seen,
            last_seen: p.last_seen,
            online: status == "online",
            familiar_version: p.familiar_version.clone(),
            tools: p.tools_offered,
            patterns: p.patterns_offered,
            addr: ip,
            relationship,
            ai: has_ai,
            trust,
            status: status.into(),
            session_start: if status == "online" { p.session_start } else { 0 },
            total_online_secs: p.total_online_secs + live,
            interactive: p.interactive || is_device,
            human,
            lat: p.lat,
            lon: p.lon,
        });
    }

    // Device agents — devices that push observations but aren't in the peer roster (never read the
    // worldview). One row per node, dropped once stale. A watch is called out with its relationship.
    for (node, (actor, object, ts)) in &reports {
        if peer_ids.contains(node.as_str()) {
            continue; // already listed as a (device) peer
        }
        if now - *ts > AGENT_FRESH_SECS {
            continue; // departed
        }
        let ns = actor.split(':').next().unwrap_or("");
        let relationship = match ns {
            "watch" => "watch · sensor",
            "ipad" | "phone" | "iphone" => "sensor",
            _ => "observed device",
        }
        .to_string();
        out.push(Member {
            node_id: node.clone(),
            label: actor.clone(),
            kind: MemberKind::DeviceAgent,
            os: os_from_actor(actor),
            os_version: String::new(),
            actor: actor.clone(),
            detail: object.clone(),
            first_seen: *ts,
            last_seen: *ts,
            online: now - *ts <= ONLINE_WINDOW_SECS,
            familiar_version: String::new(),
            tools: 0,
            patterns: 0,
            addr: String::new(),
            relationship,
            ai: ai_node(node, actor),
            trust: familiar_kernel::corruption::trust(&refusals, actor, now)
                .label()
                .to_string(),
            status: status_of(MemberKind::DeviceAgent, now - *ts).into(),
            session_start: 0,
            total_online_secs: 0,
            interactive: false,
            human: actor.split_once(':').map(|(_, h)| h.to_string()).unwrap_or_default(),
            lat: 0.0,
            lon: 0.0,
        });
    }

    out
}

/// A friendlier OS name for the roster ("linux" → "Linux", "macos" → "macOS").
fn os_pretty(os: &str) -> String {
    match os {
        "linux" => "Linux".into(),
        "macos" => "macOS".into(),
        "windows" => "Windows".into(),
        other if !other.is_empty() => other.into(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::{self, DEFAULT_CERT_TTL_SECS};
    use crate::node::NodeKey;
    use familiar_kernel::observation::{self, Observation};

    const NOW: i64 = 1_000_000;

    fn fresh(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("familiar_members_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn os_family_from_actor() {
        assert_eq!(os_from_actor("ipad:ian"), "iPadOS");
        assert_eq!(os_from_actor("phone:ian"), "iOS");
        assert_eq!(os_from_actor("watch:ian"), "watchOS");
        assert_eq!(os_from_actor("client"), "");
    }

    #[test]
    fn classifies_self_gossip_peer_and_device_agent_without_overlap() {
        let dir = fresh("classify");
        let host = NodeKey::load_or_mint(&dir, "host").unwrap();
        group::create_group(&dir, &host, "river", NOW, DEFAULT_CERT_TTL_SECS).unwrap();

        // A device agent: an observation tagged mesh:<node> under a phone actor.
        observation::record(
            &dir,
            Observation::new(
                "phone:ian",
                "reports",
                "location:home",
                "",
                "mesh:phonenode1",
                NOW,
                0.9,
            ),
        )
        .unwrap();

        let members = classify(&dir, NOW + 10);
        let self_n = members
            .iter()
            .filter(|m| m.kind == MemberKind::SelfNode)
            .count();
        let agents: Vec<_> = members
            .iter()
            .filter(|m| m.kind == MemberKind::DeviceAgent)
            .collect();
        assert_eq!(self_n, 1, "exactly one self node");
        assert_eq!(agents.len(), 1, "the phone is a device agent");
        assert_eq!(agents[0].os, "iOS");
        assert_eq!(agents[0].node_id, "phonenode1");
        assert!(agents[0].online);
        // No node appears in two layers.
        let mut ids: Vec<&str> = members.iter().map(|m| m.node_id.as_str()).collect();
        ids.sort();
        let uniq = ids.len();
        ids.dedup();
        assert_eq!(uniq, ids.len(), "no node double-counted across layers");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
