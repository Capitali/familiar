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
}

/// A device counts as present if it was seen within this window. Generous, because device agents
/// report on change (they can be quiet for a while yet still be "here").
pub const ONLINE_WINDOW_SECS: i64 = 600;
/// A device agent older than this has departed — dropped from the roster.
const AGENT_FRESH_SECS: i64 = 6 * 3600;

/// The latest device report per node id: `node -> (actor, object, ts)`, over non-`mesh:` actors
/// (a device is anything reporting under `phone:`/`ipad:`/`watch:`… — the layer above the network).
fn device_reports(obs: &[familiar_kernel::observation::Observation]) -> HashMap<String, (String, String, i64)> {
    let mut latest: HashMap<String, (String, String, i64)> = HashMap::new();
    for o in obs {
        let Some(node) = o.source.strip_prefix("mesh:") else { continue };
        if o.actor.starts_with("mesh:") {
            continue; // gossip-peer presence, not a device
        }
        let e = latest.entry(node.to_string()).or_insert((String::new(), String::new(), 0));
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
        let tools = familiar_kernel::tool::load(dir).map(|t| t.len()).unwrap_or(0);
        let patterns = familiar_kernel::pattern_memory::load(dir).map(|p| p.len()).unwrap_or(0);
        out.push(Member {
            node_id: cred.membership.node_id.clone(),
            label,
            kind: MemberKind::SelfNode,
            os: os_pretty(std::env::consts::OS),
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
        });
    }

    let obs = familiar_kernel::observation::load(dir).unwrap_or_default();
    let reports = device_reports(&obs);
    let peers: Vec<PeerRecord> = transport::load_peers(dir);
    let peer_ids: std::collections::HashSet<&str> = peers.iter().map(|p| p.node_id.as_str()).collect();
    // Best-probable names for peers on the tailnet: their Tailscale hostname, keyed by IP.
    let tailnet: std::collections::HashMap<String, String> = transport::enumerate_peers()
        .into_iter()
        .map(|t| (t.ip, t.host))
        .collect();

    for p in &peers {
        let ip = p.addr.split(':').next().unwrap_or("").to_string();
        let is_device = reports.contains_key(&p.node_id);
        let (kind, os, actor, relationship) = if let Some((actor, _, _)) = reports.get(&p.node_id) {
            (MemberKind::DevicePeer, os_from_actor(actor), actor.clone(), "reads worldview".to_string())
        } else {
            (MemberKind::GossipPeer, os_pretty(&p.os), String::new(), "gossip peer".to_string())
        };
        // Prefer a resolved tailnet hostname for gossip peers; keep the device's own label otherwise.
        let label = if !is_device {
            tailnet.get(&ip).cloned().filter(|h| !h.is_empty()).unwrap_or_else(|| p.label.clone())
        } else {
            p.label.clone()
        };
        let detail = if is_device {
            reports.get(&p.node_id).map(|(_, o, _)| o.clone()).unwrap_or_default()
        } else {
            let v = if p.familiar_version.is_empty() { String::new() } else { format!("v{} · ", p.familiar_version) };
            format!("{v}{} tool(s), {} pattern(s)", p.tools_offered, p.patterns_offered)
        };
        out.push(Member {
            node_id: p.node_id.clone(),
            label,
            kind,
            os,
            actor,
            detail,
            first_seen: p.first_seen,
            last_seen: p.last_seen,
            online: now - p.last_seen <= ONLINE_WINDOW_SECS,
            familiar_version: p.familiar_version.clone(),
            tools: p.tools_offered,
            patterns: p.patterns_offered,
            addr: ip,
            relationship,
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
            Observation::new("phone:ian", "reports", "location:home", "", "mesh:phonenode1", NOW, 0.9),
        )
        .unwrap();

        let members = classify(&dir, NOW + 10);
        let self_n = members.iter().filter(|m| m.kind == MemberKind::SelfNode).count();
        let agents: Vec<_> = members.iter().filter(|m| m.kind == MemberKind::DeviceAgent).collect();
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
