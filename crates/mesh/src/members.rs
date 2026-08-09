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
    /// The human **currently present** at this device — distinct from `human` (whom it serves):
    /// who the evidence says is actually there now. Empty = unknown (no recent presence evidence).
    #[serde(default)]
    pub present_human: String,
    /// When that presence was first established in the current unbroken run (unix secs). 0 = unknown.
    #[serde(default)]
    pub present_since: i64,
    /// How sure we are the named human is here (ADR-0019). Carried rather than flattened, so a
    /// device's 0.7 binding is never read as a human's own 1.0 answer.
    #[serde(default)]
    pub present_confidence: f64,
    /// How presence was established — "face" (recognized), "motion" (carried device biometrics),
    /// "dialogue" (the human spoke to the familiar), "activity" (the device is reporting at all).
    /// Empty = unknown. Strongest evidence wins when several are fresh.
    #[serde(default)]
    pub present_via: String,
    /// Where the node is (decimal degrees) — self from `transport::self_geo`, peers from their
    /// briefs, devices from the GPS they report on worldview reads. 0/0 = unknown, and the map
    /// says so rather than inventing a place.
    #[serde(default)]
    pub lat: f64,
    #[serde(default)]
    pub lon: f64,
    /// The fix above is this device's OWN GPS, reported by a full member — the only kind of
    /// fix a console may treat as the mesh's ground truth (its clustering anchor). False for
    /// brief-carried positions (which may themselves be inherited) and for guests: a visitor's
    /// self-reported origin is welcome-card evidence, never a place to hang the household.
    #[serde(default)]
    pub geo_device: bool,
    /// Sub-devices attached to this one through the same human — e.g. a phone/iPad that has a
    /// paired watch reporting to the mesh carries "⌚ watch" here. Lets the roster badge a device
    /// with its companions without walking the edge graph. Empty for devices with nothing attached.
    #[serde(default)]
    pub attached: Vec<String>,
    /// How this member is reaching the mesh — "local" | "lighthouse" | "tailscale" — from its status
    /// heartbeat (ADR-0017). Empty when unknown. Rendered as a connectivity badge on the roster.
    #[serde(default)]
    pub connectivity: String,
    /// The node this member runs ON — a Mac console shell carries the node_id of the daemon on
    /// the same machine. They stay two real members (separate keys; see PlatformDevice.name's
    /// load-bearing " console" suffix), but the roster may nest this one under its host instead
    /// of showing an unexplained sibling. Empty for members that stand on their own machine.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub attached_to: String,
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
// 7 minutes: a phone stops polling the moment its screen locks, and normal handling gaps
// (pocket, table, conversation) run minutes — 180s churned the roster online↔away on every
// lock (watched live, four flips in four minutes). Devices decay honestly, just not twitchily.
pub const DEVICE_FRESH_SECS: i64 = 420;
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
    now: i64,
) -> HashMap<String, (String, String, i64)> {
    let mut latest: HashMap<String, (String, String, i64)> = HashMap::new();
    for o in obs {
        let Some(node) = o.source.strip_prefix("mesh:") else {
            continue;
        };
        if !is_device_actor(&o.actor) {
            continue; // not a device-sensor report (peer cycle / human / gossip presence)
        }
        // Stale reports don't classify: a weeks-old row from a since-retired relay path once
        // tagged a DOOR as "watch:ian" forever, merging it into the watch's dedup lineage and
        // flapping the lighthouse roster every time their last_seens leapfrogged. What a node
        // IS deserves fresh evidence; the sticky peer actor carries identity across quiet
        // spells, not this map.
        if now - o.ts > AGENT_FRESH_SECS {
            continue;
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

/// A human is present only if the evidence is fresher than this. Past it, presence is
/// **unknown** — the roster says so rather than showing a stale name (a phone left on a
/// counter is not its owner standing there).
const PRESENCE_WINDOW_SECS: i64 = 30 * 60;

/// The presence a single observation attests, if any: `(strength, via, human)`. Higher
/// strength wins when several are fresh — a recognized face outranks a mere device beacon.
/// `human` is empty when the evidence shows *someone* is there but not *who* (a bare
/// dialogue turn, an unnamespaced report).
fn presence_evidence(
    o: &familiar_kernel::observation::Observation,
) -> Option<(u8, &'static str, String)> {
    let ns_human = || {
        o.actor
            .split_once(':')
            .map(|(_, h)| h.to_string())
            .unwrap_or_default()
    };
    // 1. A recognized face names the human directly — the strongest evidence.
    if o.action == "recognized" {
        if let Some(name) = o.object.strip_prefix("face:") {
            return Some((4, "face", name.to_string()));
        }
    }
    // 2. The human spoke to the familiar (a dialogue turn / an answered thread).
    if o.source == "observer" || o.action == "answered" || o.action == "told the familiar" {
        // A human actor names them; the observer channel may not, and that's honest.
        let who = if o.actor.contains(':') {
            ns_human()
        } else {
            o.actor.clone()
        };
        let who = if who == "observer" {
            String::new()
        } else {
            who
        };
        return Some((3, "dialogue", who));
    }
    // 3. A carried personal device sensing its owner (motion, heartbeat, sleep) — but the
    //    bare `presence` beacon is only that the device is *active*, the weakest tier.
    if familiar_kernel::service::is_personal_device_report(o) {
        if o.action == "reports" && o.object == "presence" {
            return Some((1, "activity", ns_human()));
        }
        return Some((2, "motion", ns_human()));
    }
    None
}

/// Who is present at a node now, since when, and how — from the node's own observation
/// stream (`belongs` selects them). Returns empty/0 when no evidence is inside the window.
/// `since` walks back through an unbroken run (gaps ≤ the window) of same-human evidence.
fn derive_presence(
    obs: &[familiar_kernel::observation::Observation],
    now: i64,
    belongs: impl Fn(&familiar_kernel::observation::Observation) -> bool,
) -> (String, i64, String) {
    // Freshest presence evidence for this node, strongest tier breaking ties.
    let mut best: Option<(i64, u8, &'static str, String)> = None;
    for o in obs.iter().filter(|o| belongs(o)) {
        if now - o.ts > PRESENCE_WINDOW_SECS {
            continue;
        }
        if let Some((strength, via, human)) = presence_evidence(o) {
            // Strongest establishment wins, freshness breaks ties: a face recognized 20
            // minutes ago says more about *who is here* than a beacon ping one minute ago.
            let better = match &best {
                None => true,
                Some((ts, s, _, _)) => strength > *s || (strength == *s && o.ts > *ts),
            };
            if better {
                best = Some((o.ts, strength, via, human));
            }
        }
    }
    let Some((_, _, via, human)) = best else {
        return (String::new(), 0, String::new()); // unknown
    };
    // How long: the earliest fresh evidence of the same human — since the freshness filter
    // already bounds everything to one window of now, that is the start of the present run.
    // Same human is matched case-insensitively (a recognized `face:Ian` and a `phone:ian`
    // report are the same person); an anonymous turn (empty human) joins any run.
    let since = obs
        .iter()
        .filter(|o| belongs(o) && now - o.ts <= PRESENCE_WINDOW_SECS)
        .filter_map(|o| {
            presence_evidence(o).and_then(|(_, _, h)| {
                (human.is_empty() || h.is_empty() || h.eq_ignore_ascii_case(&human)).then_some(o.ts)
            })
        })
        .min()
        .unwrap_or(now);
    (human, since, via.to_string())
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
    // A device's OWN presence claim (ADR-0019), if it is heartbeating one. The device is closer to
    // the evidence than we are — it holds the binding, ran the 1:1 face check, and heard the human
    // answer — so where it makes a live claim about itself, that claim wins over what we can infer
    // from its observation stream. `status::record` already enforces that a member may only place
    // its OWN status, so a device can only ever speak for itself.
    let claims = crate::status::directory(dir, now);
    let claim_of = |node_id: &str| -> Option<(String, i64, String, f64)> {
        claims
            .iter()
            .find(|s| s.node_id == node_id && !s.present_human.trim().is_empty())
            .map(|s| {
                (
                    s.present_human.clone(),
                    s.present_since,
                    s.present_via.clone(),
                    s.present_confidence,
                )
            })
    };

    // Self — this instance of the familiar. Peers are equals: it is named by its host (like every
    // other peer's brief label), NOT by the group or a privileged "familiar" name. `SelfNode` marks
    // "you are here"; it confers no special standing.
    if let Ok(Some(cred)) = group::load(dir) {
        let label = crate::node::NodeKey::load_or_mint(dir, "")
            .ok()
            .map(|n| n.identity().label)
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| cred.membership.node_id.chars().take(8).collect());
        let obs = familiar_kernel::observation::load_recent(dir, 4000).unwrap_or_default();
        let first = familiar_kernel::observation::first_ts(dir)
            .ok()
            .flatten()
            .unwrap_or(now);
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
        // Presence at the host itself: face recognitions and dialogue land here as
        // non-mesh observations (host/observer sources), so select the local stream.
        let (sp_human, sp_since, sp_via) = derive_presence(&obs, now, |o| {
            !o.source.starts_with("mesh:") && !is_device_actor(&o.actor)
        });
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
            present_confidence: match sp_via.as_str() {
                "face" => 0.9,
                "dialogue" => 0.85,
                "motion" => 0.6,
                "activity" => 0.4,
                _ => 0.0,
            },
            present_human: sp_human,
            present_since: sp_since,
            present_via: sp_via,
            lat: self_lat,
            lon: self_lon,
            // The daemon is not a GPS device; consoles anchor on this row by its kind
            // (self_node / the "· host" relationship), not by fix provenance.
            geo_device: false,
            attached: Vec::new(),
            attached_to: String::new(),
            connectivity: "local".into(),
        });
    }

    let obs = familiar_kernel::observation::load_recent(dir, 4000).unwrap_or_default();
    let reports = device_reports(&obs, now);
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
    // Abandoned peers (familiar mesh abandon <node_id>) are excluded from the active
    // roster/worldview entirely — their full record stays in peers.jsonl (never deleted), they
    // just stop being carried around in every gossip round and device read. Self-healing: any
    // fresh contact revives one automatically (see transport::upsert_peer/register_device_peer).
    let peers: Vec<PeerRecord> = transport::load_peers(dir)
        .into_iter()
        .filter(|p| p.status != "abandoned")
        .collect();
    let peer_ids: std::collections::HashSet<&str> =
        peers.iter().map(|p| p.node_id.as_str()).collect();
    // Best-probable names for peers on the tailnet: their Tailscale hostname, keyed by IP.
    let tailnet: std::collections::HashMap<String, String> = transport::enumerate_peers()
        .into_iter()
        .map(|t| (t.ip, t.host))
        .collect();

    for p in &peers {
        let ip = p.addr.split(':').next().unwrap_or("").to_string();
        let is_device = reports.contains_key(&p.node_id) || !p.actor.is_empty();
        let (kind, os, actor, relationship) = if let Some((actor, _, _)) = reports.get(&p.node_id) {
            (
                MemberKind::DevicePeer,
                os_from_actor(actor),
                actor.clone(),
                "reads worldview".to_string(),
            )
        } else if !p.actor.is_empty() {
            // The sticky actor: identity survives report-quiet spells (the watch-fold and the
            // hardware label both key on it).
            (
                MemberKind::DevicePeer,
                os_from_actor(&p.actor),
                p.actor.clone(),
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
        // Whose device this IS: the record's ESTABLISHED handle is the identity truth and
        // outranks every cached claim — after a release/disestablish the cached brief kept
        // whispering the old name ("iPhone ian" while bob was present). A record that exists
        // with NO establishment names nobody, honestly; only devices without records at all
        // fall back to what they said about themselves.
        let record = crate::record::find_by_key(dir, &p.node_id);
        // Full membership gates fix provenance below: a guest's roster row may still carry the
        // position it reported (shown honestly on its own pin), but only a member's own GPS is
        // marked geo_device — the mark that lets a console anchor the unlocated on it.
        let full_member = record
            .as_ref()
            .map(|r| crate::record::derive_state(r) == crate::record::RecordState::Member)
            .unwrap_or(false);
        // The established handle is the truth when it exists — but an un-established record must
        // still fall back to what the device said about itself (B8): a guest iPhone with a
        // record but no establishment was rendering NAMELESS forever, one ghost row per node.
        let human = match record.as_ref().and_then(|r| r.identity.established.as_ref()) {
            Some(e) if !e.handle.is_empty() => e.handle.clone(),
            _ if !p.human.is_empty() => p.human.clone(),
            _ => actor
                .split_once(':')
                .map(|(_, h)| h.to_string())
                .unwrap_or_default(),
        };
        // Cumulative online time, live session included while it's still fresh.
        let live = if status == "online" && p.session_start > 0 {
            (now - p.session_start).max(0)
        } else if p.session_start > 0 {
            (p.last_seen - p.session_start).max(0)
        } else {
            0
        };
        // Presence, only for devices (a gossip peer reports its own). Evidence from this
        // device arrives tagged `mesh:<node_id>` in our store.
        let dev_presence = if is_device {
            let src = format!("mesh:{}", p.node_id);
            derive_presence(&obs, now, |o| o.source == src)
        } else {
            (String::new(), 0, String::new())
        };
        // The device's own claim wins where it makes one; our derivation is the fallback for
        // clients too old to report a claim, and for gossip peers that are not devices at all.
        // Derived presence carries no stated confidence, so it is scored by its own evidence tier.
        let claimed = claim_of(&p.node_id).unwrap_or_else(|| {
            let conf = match dev_presence.2.as_str() {
                "face" => 0.9,
                "dialogue" => 0.85,
                "motion" => 0.6,
                "activity" => 0.4,
                _ => 0.0,
            };
            (
                dev_presence.0.clone(),
                dev_presence.1,
                dev_presence.2.clone(),
                conf,
            )
        });
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
            session_start: if status == "online" {
                p.session_start
            } else {
                0
            },
            total_online_secs: p.total_online_secs + live,
            interactive: p.interactive || is_device,
            human,
            // A gossip peer's presence is its own to know and report; here we derive it only for
            // devices whose observations flow through this node's store (source `mesh:<node>`).
            present_human: claimed.0,
            present_since: claimed.1,
            present_via: claimed.2,
            present_confidence: claimed.3,
            lat: p.lat,
            lon: p.lon,
            geo_device: p.geo_device && full_member,
            attached: Vec::new(),
            attached_to: String::new(),
            connectivity: p.connectivity.clone(),
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
        let src = format!("mesh:{node}");
        let agent_presence = derive_presence(&obs, now, |o| o.source == src);
        out.push(Member {
            node_id: node.clone(),
            // A friendly label, not the raw actor — "watch:ian" beside the prominent human
            // name read as a stutter ("watch:ian Ian"). The namespace names the hardware;
            // the human column names the human.
            label: match ns {
                "watch" => "Apple Watch".to_string(),
                "phone" | "iphone" => "iPhone".to_string(),
                "ipad" => "iPad".to_string(),
                "mac" => "Mac".to_string(),
                _ => actor.clone(),
            },
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
            human: actor
                .split_once(':')
                .map(|(_, h)| h.to_string())
                .unwrap_or_default(),
            present_confidence: match agent_presence.2.as_str() {
                "face" => 0.9,
                "dialogue" => 0.85,
                "motion" => 0.6,
                "activity" => 0.4,
                _ => 0.0,
            },
            present_human: agent_presence.0,
            present_since: agent_presence.1,
            present_via: agent_presence.2,
            lat: 0.0,
            lon: 0.0,
            geo_device: false,
            attached: Vec::new(),
            attached_to: String::new(),
            connectivity: String::new(),
        });
    }

    // Remote members (ADR-0033 federation, and any member admitted through a door this one
    // can't reach — under CGNAT, both directions, that is most of them). They have a member
    // record but no local peer row, so the peer loop above never seats them and they vanish
    // from the roster on every door their device can't reach (B14). A member is never trapped
    // on one machine — except, until now, on the roster. Build a seat from the record itself,
    // marked "remote" rather than a fabricated online/offline, since this door genuinely cannot
    // see their liveness (that can later ride the door-to-door record sync the records use).
    let self_id = crate::node::NodeKey::load_or_mint(dir, "")
        .map(|n| n.node_id())
        .unwrap_or_default();
    let emitted: std::collections::HashSet<String> =
        out.iter().map(|m| m.node_id.clone()).collect();
    for r in crate::record::snapshot(dir).iter() {
        if crate::record::derive_state(r) != crate::record::RecordState::Member {
            continue;
        }
        if r.device_id == self_id {
            continue; // never re-emit self as a remote member
        }
        if emitted.contains(&r.device_id) || r.keys.iter().any(|k| emitted.contains(k)) {
            continue; // it already got a real peer row on this door
        }
        let handle = r
            .identity
            .established
            .as_ref()
            .map(|e| e.handle.clone())
            .unwrap_or_default();
        let origin = r.origin.as_ref();
        let label = origin
            .map(|o| o.label.clone())
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| r.device_id.chars().take(8).collect());
        let (lat, lon) = origin.map(|o| (o.lat, o.lon)).unwrap_or((0.0, 0.0));
        let joined = r
            .admitted
            .as_ref()
            .map(|a| a.at)
            .filter(|&t| t > 0)
            .unwrap_or(r.first_seen);
        let trust =
            familiar_kernel::corruption::trust(&refusals, &format!("mesh:{}", r.device_id), now)
                .label()
                .to_string();
        out.push(Member {
            node_id: r.device_id.clone(),
            label,
            kind: MemberKind::DevicePeer,
            os: String::new(),
            os_version: String::new(),
            actor: String::new(),
            detail: r.note.clone(),
            first_seen: joined,
            last_seen: r.last_seen,
            online: false,
            familiar_version: String::new(),
            tools: 0,
            patterns: 0,
            addr: origin.map(|o| o.addr.clone()).unwrap_or_default(),
            relationship: "remote member — via the lighthouse".into(),
            ai: false,
            trust,
            // A word the console tolerates (statusColor/isStale key on != "online"); it reads as
            // present-in-the-mesh without claiming a liveness this door can't witness.
            status: "remote".into(),
            session_start: 0,
            total_online_secs: 0,
            interactive: true,
            human: handle,
            present_human: String::new(),
            present_since: 0,
            present_via: String::new(),
            present_confidence: 0.0,
            lat,
            lon,
            geo_device: false,
            attached: Vec::new(),
            attached_to: String::new(),
            connectivity: "lighthouse".into(),
        });
    }

    out = dedup_devices(out);
    attach_companions(&mut out, &obs);
    attach_consoles(&mut out, &transport::reachable_hosts());
    attach_watches(&mut out);
    out
}

/// A watch rides its paired phone (the design of record): TOGETHER, it is the ⌚ badge on the
/// phone's row — its own row folds into the host card like a console's does. Apart — its own
/// address, different from the phone's (cellular, foreign wifi) — it stands alone in the
/// mesh. "Together" is judged the same honest way consoles nest: no address of its own
/// (presence relayed through the pairing) or the same address as the phone.
fn attach_watches(out: &mut [Member]) {
    let ip_of = |addr: &str| addr.split(':').next().unwrap_or("").to_string();
    let mut links: Vec<(usize, usize)> = Vec::new();
    for (wi, w) in out.iter().enumerate() {
        if w.actor.split(':').next() != Some("watch") {
            continue;
        }
        let human = w.actor.split(':').nth(1).unwrap_or("");
        if human.is_empty() {
            continue;
        }
        let wip = ip_of(&w.addr);
        // Together = same place, judged by address: exact match (a shared NAT seen from the
        // lighthouse), or the same private /24 (the home LAN, where every device holds its
        // own address). A watch on cellular carries an address matching neither.
        let together = |a: &str, b: &str| -> bool {
            if a == b {
                return true;
            }
            let p = |s: &str| s.rsplitn(2, '.').nth(1).map(|x| x.to_string());
            let private = |s: &str| {
                s.starts_with("192.168.") || s.starts_with("10.")
                    || s.starts_with("172.16.") || s.starts_with("172.17.")
                    || s.starts_with("172.18.") || s.starts_with("172.19.")
                    || s.starts_with("172.2") || s.starts_with("172.30.") || s.starts_with("172.31.")
            };
            private(a) && private(b) && p(a).is_some() && p(a) == p(b)
        };
        let host = out.iter().enumerate().find(|(hi, h)| {
            *hi != wi
                && matches!(h.actor.split(':').next(), Some("phone") | Some("iphone"))
                && h.actor.split(':').nth(1) == Some(human)
                && (wip.is_empty() || together(&wip, &ip_of(&h.addr)))
        });
        if let Some((hi, _)) = host {
            links.push((wi, hi));
        }
    }
    for (wi, hi) in links {
        out[wi].attached_to = out[hi].node_id.clone();
        if !out[hi].attached.iter().any(|a| a == "⌚ watch") {
            out[hi].attached.push("⌚ watch".to_string());
        }
    }
}

/// A Mac console shell (`mac:*`) and the daemon on the same machine are two real members —
/// separate keys, and the console can quit while the daemon runs on (the " console" label
/// suffix is load-bearing; see the Swift side's PlatformDevice.name). What the roster was
/// missing is the RELATIONSHIP: mark the console `attached_to` its host node and badge the
/// host, so the UI can nest one under the other instead of showing an unexplained sibling.
///
/// The link is structural first — the console's address is the host machine's address (a full
/// peer's addr, or one of this host's own addresses for the self node) — with the label
/// convention ("<machine> console" vs "<machine>") as the fallback when addresses are absent
/// or stale. Pure over the member list + this host's own addresses, so it's directly testable.
fn attach_consoles(out: &mut [Member], self_hosts: &[String]) {
    let ip_of = |addr: &str| addr.split(':').next().unwrap_or("").to_string();
    // A console that has pushed observations recently classifies as a DevicePeer with a `mac:*`
    // actor; one that has merely been gossiping sits as a GossipPeer with an EMPTY actor, and
    // only its label ("<machine> console" — PlatformDevice.name) says what it is. Accept both,
    // and never let a console be another console's host.
    let is_console = |m: &Member| {
        m.kind != MemberKind::SelfNode
            && (m.actor.split(':').next() == Some("mac")
                || m.label.to_lowercase().ends_with(" console"))
    };
    let mut links: Vec<(usize, usize)> = Vec::new();
    for (ci, c) in out.iter().enumerate() {
        if !is_console(c) {
            continue;
        }
        let cip = ip_of(&c.addr);
        let on_self_host =
            !cip.is_empty() && (cip == "127.0.0.1" || self_hosts.contains(&cip));
        let stem = c.label.to_lowercase();
        let stem = stem.strip_suffix(" console").unwrap_or("").trim().to_string();
        // The label pass accepts ANY non-console member as the machine — from a sibling door's
        // view the wildhorse daemon classifies as a DevicePeer (it reports observations there),
        // and nesting must hold from EVERY door or the roster flaps whenever a console fails
        // over. The label convention is machine-specific, so it decides first regardless of
        // kind; the structural (address) pass stays restricted to full nodes, because behind
        // one NAT many machines share a recorded public address.
        let full_node = |h: &Member| {
            matches!(h.kind, MemberKind::SelfNode | MemberKind::GossipPeer) && !is_console(h)
        };
        let host = out
            .iter()
            .enumerate()
            .find(|(hi, h)| {
                *hi != ci && !is_console(h) && !stem.is_empty() && h.label.to_lowercase() == stem
            })
            .or_else(|| {
                out.iter().enumerate().find(|(hi, h)| {
                    *hi != ci
                        && full_node(h)
                        && ((h.kind == MemberKind::SelfNode && on_self_host)
                            || (!cip.is_empty() && ip_of(&h.addr) == cip))
                })
            });
        if let Some((hi, _)) = host {
            links.push((ci, hi));
        }
    }
    for (ci, hi) in links {
        out[ci].attached_to = out[hi].node_id.clone();
        if !out[hi].attached.iter().any(|a| a == "🖥 console") {
            out[hi].attached.push("🖥 console".to_string());
        }
    }
}

/// Collapse repeated identities of the same physical device into one roster row. A device that is
/// deleted and reinstalled mints a fresh node key (a new `node_id`), and with auto-admit each one
/// joins — so the same phone/iPad/watch can appear several times in different states, polluting the
/// roster. They share a device **actor** (`phone:ian`, `ipad:ian`, `watch:ian`), so we group by that
/// and keep the freshest identity (max `last_seen`), carrying the cumulative online time across the
/// lineage and preferring a human-friendly label over a raw `namespace:human` one. Non-device members
/// (the self node, gossip peers — empty or non-device actor) are keyed by `node_id` and never merged.
fn dedup_devices(members: Vec<Member>) -> Vec<Member> {
    use std::collections::HashMap;
    let is_device_ns = |a: &str| {
        matches!(
            a.split(':').next().unwrap_or(""),
            "phone" | "iphone" | "ipad" | "mac" | "watch" | "tv"
        )
    };
    // One node, one row: a watch (or phone) can appear both as an actor-carrying agent row
    // and an actorless peer row for the SAME node_id — adopt the actor onto the actorless
    // twin first, so the grouping below folds them into one member.
    let node_actor: HashMap<String, String> = members
        .iter()
        .filter(|m| !m.actor.is_empty() && is_device_ns(&m.actor))
        .map(|m| (m.node_id.clone(), m.actor.clone()))
        .collect();
    let members: Vec<Member> = members
        .into_iter()
        .map(|mut m| {
            if m.actor.is_empty() {
                if let Some(a) = node_actor.get(&m.node_id) {
                    m.actor = a.clone();
                }
            }
            m
        })
        .collect();
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<Member>> = HashMap::new();
    for m in members {
        let key = if !m.actor.is_empty() && is_device_ns(&m.actor) {
            format!("actor:{}", m.actor)
        } else {
            format!("id:{}", m.node_id)
        };
        if !groups.contains_key(&key) {
            order.push(key.clone());
        }
        groups.entry(key).or_default().push(m);
    }
    let mut out = Vec::new();
    for key in order {
        let mut g = groups.remove(&key).unwrap();
        if g.len() == 1 {
            out.push(g.pop().unwrap());
            continue;
        }
        let total: i64 = g.iter().map(|m| m.total_online_secs).sum();
        // A friendly label from any identity in the lineage (one that isn't a raw `ns:human`).
        let nice = g
            .iter()
            .map(|m| m.label.clone())
            .find(|l| !l.is_empty() && !l.contains(':'));
        // Freshest identity represents the device now.
        g.sort_by_key(|m| std::cmp::Reverse(m.last_seen));
        let mut rep = g.remove(0);
        rep.total_online_secs = total;
        if rep.label.contains(':') || rep.label.is_empty() {
            if let Some(l) = nice {
                rep.label = l;
            }
        }
        out.push(rep);
    }
    out
}

/// A watch reports `watch:<human> reports <signal>:<value>`. Roll the freshest of each signal
/// (heart / motion / gyro / gps) into one line for the watch's own roster row, and badge the same
/// human's phone/iPad/Mac with "⌚ watch" — so the roster shows the pairing (and the watch's live
/// telemetry) without the reader having to walk the mesh edge graph. Pure over the member list +
/// this node's observation store, so it's directly testable.
fn attach_companions(out: &mut [Member], obs: &[familiar_kernel::observation::Observation]) {
    let mut watch_details: Vec<(usize, String)> = Vec::new();
    let mut humans_with_watch: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (i, m) in out.iter().enumerate() {
        if m.actor.split(':').next() != Some("watch") {
            continue;
        }
        let human = m.actor.split(':').nth(1).unwrap_or("").to_string();
        if !human.is_empty() {
            humans_with_watch.insert(human);
        }
        // Freshest value per signal (newest observation first).
        let src = format!("mesh:{}", m.node_id);
        let mut recent: Vec<&familiar_kernel::observation::Observation> = obs
            .iter()
            .filter(|o| (o.source == src || o.actor == m.actor) && o.action == "reports")
            .collect();
        recent.sort_by_key(|o| std::cmp::Reverse(o.ts));
        let (mut heart, mut motion, mut gyro, mut gps) = (None, None, None, None);
        for o in recent {
            let obj = o.object.as_str();
            if heart.is_none() {
                if let Some(v) = obj.strip_prefix("heart_rate:") {
                    heart = Some(v.to_string());
                    continue;
                }
            }
            if motion.is_none() {
                if let Some(v) = obj.strip_prefix("motion:") {
                    motion = Some(v.to_string());
                    continue;
                }
            }
            if gyro.is_none() {
                if let Some(v) = obj.strip_prefix("gyro:") {
                    gyro = Some(v.to_string());
                    continue;
                }
            }
            if gps.is_none() {
                if let Some(v) = obj.strip_prefix("location:") {
                    gps = Some(v.to_string());
                }
            }
        }
        let mut parts: Vec<String> = Vec::new();
        if let Some(h) = heart {
            parts.push(format!("♥ {h}"));
        }
        if let Some(mo) = motion {
            parts.push(mo);
        }
        if let Some(gy) = gyro {
            parts.push(format!("gyro {gy}"));
        }
        if let Some(g) = gps {
            parts.push(format!("gps {g}"));
        }
        if !parts.is_empty() {
            watch_details.push((i, parts.join(" · ")));
        }
    }
    for (i, detail) in watch_details {
        out[i].detail = detail;
    }
    if !humans_with_watch.is_empty() {
        for m in out.iter_mut() {
            let ns = m.actor.split(':').next().unwrap_or("");
            let human = m.actor.split(':').nth(1).unwrap_or("");
            // An Apple Watch pairs with the iPhone only — never an iPad or Mac. Badge just the phone.
            if matches!(ns, "phone" | "iphone")
                && humans_with_watch.contains(human)
                && !m.attached.iter().any(|a| a == "⌚ watch")
            {
                m.attached.push("⌚ watch".to_string());
            }
        }
    }
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
    fn presence_is_derived_by_strength_freshness_and_run_length() {
        let src = "mesh:dev1";
        let mk = |actor: &str, action: &str, object: &str, ts: i64| {
            Observation::new(actor, action, object, "", src, ts, 0.9)
        };
        // A run of motion reports, then a fresher recognized face: face wins (stronger),
        // names the human, and `since` reaches back through the unbroken run.
        let obs = vec![
            mk("phone:ian", "reports", "motion:walking", NOW - 20 * 60),
            mk("phone:ian", "reports", "motion:still", NOW - 12 * 60),
            mk("phone:ian", "reports", "presence", NOW - 5 * 60),
            mk("host", "recognized", "face:Ian", NOW - 60),
        ];
        let (who, since, via) = derive_presence(&obs, NOW, |o| o.source == src);
        assert_eq!(who, "Ian");
        assert_eq!(via, "face");
        assert_eq!(
            since,
            NOW - 20 * 60,
            "run reaches back to the first fresh turn"
        );

        // Nothing fresh → unknown, honestly.
        let stale = vec![mk("phone:ian", "reports", "motion:walking", NOW - 60 * 60)];
        let (who, since, via) = derive_presence(&stale, NOW, |o| o.source == src);
        assert!(who.is_empty() && via.is_empty() && since == 0);

        // Evidence older than the window is not counted toward the run — `since` is the
        // earliest evidence still inside it.
        let mixed = vec![
            mk("phone:ian", "reports", "motion:walking", NOW - 50 * 60), // stale, ignored
            mk("phone:ian", "reports", "motion:still", NOW - 10 * 60),
            mk("phone:ian", "reports", "presence", NOW - 3 * 60),
        ];
        let (_, since, via) = derive_presence(&mixed, NOW, |o| o.source == src);
        assert_eq!(via, "motion");
        assert_eq!(
            since,
            NOW - 10 * 60,
            "the stale turn outside the window does not count"
        );

        // Dialogue names the speaker; the observer channel may not, and that's honest.
        let named = vec![mk("ian", "answered", "thread:1", NOW - 60)];
        assert_eq!(derive_presence(&named, NOW, |o| o.source == src).0, "ian");
        let anon = vec![Observation::new(
            "observer",
            "answered",
            "thread:1",
            "",
            "observer",
            NOW - 60,
            0.9,
        )];
        let (who, _, via) = derive_presence(&anon, NOW, |_| true);
        assert!(who.is_empty() && via == "dialogue");
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

    #[test]
    fn an_abandoned_peer_is_hidden_but_revives_on_fresh_contact() {
        let dir = fresh("abandon_revive");
        let host = NodeKey::load_or_mint(&dir, "host").unwrap();
        group::create_group(&dir, &host, "river", NOW, DEFAULT_CERT_TTL_SECS).unwrap();

        transport::register_device_peer(
            &dir,
            "retired1",
            "Old iPad",
            "192.168.1.9",
            "v1",
            "iPadOS 18",
            0.0,
            0.0,
        )
        .unwrap();
        assert!(
            classify(&dir, NOW).iter().any(|m| m.node_id == "retired1"),
            "present before being abandoned"
        );

        assert!(transport::abandon_peer(&dir, "retired1").unwrap());
        assert!(
            !classify(&dir, NOW).iter().any(|m| m.node_id == "retired1"),
            "abandoned peers are excluded from the active roster"
        );
        // The record itself is never deleted, only hidden.
        assert!(transport::load_peers(&dir)
            .iter()
            .any(|p| p.node_id == "retired1" && p.status == "abandoned"));

        // Fresh contact (this device reads the worldview again) revives it automatically —
        // a human re-abandons if it turns out to be a one-off blip, not a real departure.
        transport::register_device_peer(
            &dir,
            "retired1",
            "Old iPad",
            "192.168.1.9",
            "v1",
            "iPadOS 18",
            0.0,
            0.0,
        )
        .unwrap();
        assert!(
            classify(&dir, NOW).iter().any(|m| m.node_id == "retired1"),
            "renewed contact revives an abandoned peer"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn watch_attaches_to_phone_and_summarizes_its_signals() {
        let mk = |node: &str, actor: &str, kind: &str, detail: &str| -> Member {
            serde_json::from_value(serde_json::json!({
                "node_id": node, "label": actor, "kind": kind, "os": "",
                "actor": actor, "detail": detail, "first_seen": 0, "last_seen": 0, "online": true
            }))
            .unwrap()
        };
        let mut out = vec![
            mk("phone1", "phone:ian", "device_peer", ""),
            mk("watch1", "watch:ian", "device_agent", "location:1,2"),
            mk("ipad9", "ipad:kai", "device_peer", ""), // different human — must NOT be badged
            mk("ipadi", "ipad:ian", "device_peer", ""), // SAME human, but a watch pairs with the phone
        ];
        let src = "mesh:watch1";
        let obs = vec![
            Observation::new(
                "watch:ian",
                "reports",
                "heart_rate:elevated",
                "",
                src,
                100,
                0.9,
            ),
            Observation::new("watch:ian", "reports", "motion:walking", "", src, 101, 0.9),
            Observation::new("watch:ian", "reports", "gyro:turning", "", src, 102, 0.9),
            Observation::new(
                "watch:ian",
                "reports",
                "location:48.6,-93.4",
                "",
                src,
                103,
                0.9,
            ),
            // A newer heart reading must win over the older one.
            Observation::new(
                "watch:ian",
                "reports",
                "heart_rate:normal",
                "",
                src,
                104,
                0.9,
            ),
        ];
        attach_companions(&mut out, &obs);
        assert!(
            out[0].attached.iter().any(|a| a == "⌚ watch"),
            "the same human's phone is badged with its watch"
        );
        assert!(
            out[1].attached.is_empty(),
            "the watch itself carries no badge"
        );
        assert!(
            out[2].attached.is_empty(),
            "a different human's iPad is not badged"
        );
        assert!(
            out[3].attached.is_empty(),
            "even the same human's iPad is not badged — a watch pairs with the iPhone, not the iPad"
        );
        let d = &out[1].detail;
        assert!(d.contains("♥ normal"), "freshest heart wins: {d}");
        assert!(d.contains("walking"), "motion in summary: {d}");
        assert!(d.contains("gyro turning"), "gyro in summary: {d}");
        assert!(d.contains("gps 48.6,-93.4"), "gps in summary: {d}");
    }

    #[test]
    fn a_mac_console_nests_under_the_machine_it_runs_on() {
        let mk = |node: &str, label: &str, actor: &str, kind: &str, addr: &str| -> Member {
            serde_json::from_value(serde_json::json!({
                "node_id": node, "label": label, "kind": kind, "actor": actor, "addr": addr,
                "os": "", "detail": "", "first_seen": 0, "last_seen": 0, "online": true,
            }))
            .unwrap()
        };
        // Seen from a third node (the lighthouse): wildhorse's daemon is a gossip peer and its
        // console a device peer, both reporting the same machine address.
        let mut out = vec![
            mk("selflh", "lighthouse", "", "self_node", "localhost"),
            mk("1c99", "wildhorse", "", "gossip_peer", "192.168.108.10"),
            mk("a24d", "Wildhorse console", "mac:ian", "device_peer", "192.168.108.10"),
            mk("d5c3", "iPhone", "phone:ian", "device_peer", "192.168.108.188"),
        ];
        attach_consoles(&mut out, &[]);
        assert_eq!(
            out[2].attached_to, "1c99",
            "the console is attached to the daemon on its machine (label stem match)"
        );
        assert!(
            out[1].attached.iter().any(|a| a == "🖥 console"),
            "the machine's daemon is badged with its console"
        );
        assert!(
            out[3].attached_to.is_empty(),
            "a phone stands on its own — no attachment"
        );

        // Seen from the Mac itself: the daemon is the self node ("localhost"), the console's
        // address is one of this host's own. No label dependence — structural match.
        let mut own = vec![
            mk("1c99", "TheRiver", "", "self_node", "localhost"),
            mk("a24d", "Renamed Thing", "mac:ian", "device_peer", "192.168.108.10"),
        ];
        attach_consoles(&mut own, &["192.168.108.10".to_string()]);
        assert_eq!(
            own[1].attached_to, "1c99",
            "on the host itself the console attaches to the self node by address"
        );

        // A console whose machine is nowhere on the roster stays a sibling — no false nesting.
        let mut alone = vec![
            mk("selflh", "lighthouse", "", "self_node", "localhost"),
            mk("a24d", "Codex console", "mac:ian", "device_peer", "10.0.0.7"),
        ];
        attach_consoles(&mut alone, &[]);
        assert!(
            alone[1].attached_to.is_empty(),
            "no host on the roster — the console stands alone rather than guessing"
        );

        // The live shape on wildhorse: a console that has only been gossiping (no recent
        // observation reports) classifies as a GossipPeer with an EMPTY actor — only the
        // "<machine> console" label says what it is. It must still nest, and must never be
        // taken as another console's host.
        let mut quiet = vec![
            mk("1c99", "TheRiver", "", "self_node", "localhost"),
            mk("a24d", "Wildhorse console", "", "gossip_peer", "192.168.108.10"),
            mk("aaaa", "Codex console", "", "gossip_peer", "192.168.108.10"),
        ];
        attach_consoles(&mut quiet, &["192.168.108.10".to_string()]);
        assert_eq!(
            quiet[1].attached_to, "1c99",
            "a quiet console (gossip peer, empty actor) still nests by its label + address"
        );
        assert_eq!(
            quiet[2].attached_to, "1c99",
            "the second console attaches to the machine, never to the first console"
        );

        // From a SIBLING door's view the machine's daemon classifies as a DevicePeer (it
        // reports observations there) — nesting must hold from every door, or the roster
        // flaps whenever a console reads through its fallback.
        let mut sibling = vec![
            mk("selflh", "lighthouse", "", "self_node", "localhost"),
            mk("1c99", "Wildhorse", "familiar:wildhorse", "device_peer", "129.224.211.111"),
            mk("a24d", "Wildhorse console", "mac:ian", "device_peer", "129.224.211.111"),
        ];
        attach_consoles(&mut sibling, &[]);
        assert_eq!(
            sibling[2].attached_to, "1c99",
            "the console nests under its machine even when the machine reads as a device peer"
        );
    }

    #[test]
    fn dedup_collapses_reinstalled_device_identities() {
        let mk =
            |node: &str, label: &str, actor: &str, kind: &str, last: i64, total: i64| -> Member {
                serde_json::from_value(serde_json::json!({
                    "node_id": node, "label": label, "kind": kind, "os": "", "actor": actor,
                    "detail": "", "first_seen": 0, "last_seen": last, "online": false,
                    "total_online_secs": total
                }))
                .unwrap()
            };
        let members = vec![
            mk("self1", "Wildhorse", "", "self_node", 500, 0), // no device actor — untouched
            mk("d5c3", "iPhone", "phone:ian", "device_peer", 100, 60), // oldest, nice label
            mk("7361", "phone:ian", "phone:ian", "device_agent", 200, 30),
            mk("8e51", "phone:ian", "phone:ian", "device_agent", 400, 10), // freshest
            mk("ipad1", "iPad", "ipad:ian", "device_peer", 300, 90),       // sole iPad — kept as-is
        ];
        let out = dedup_devices(members);
        // Wildhorse + one iPhone + one iPad == 3 rows (the three phone:ian identities collapsed).
        assert_eq!(out.len(), 3, "three phone identities collapse to one");
        let phone = out.iter().find(|m| m.actor == "phone:ian").unwrap();
        assert_eq!(
            phone.node_id, "8e51",
            "the freshest identity represents the device"
        );
        assert_eq!(
            phone.label, "iPhone",
            "a friendly label is carried from the lineage"
        );
        assert_eq!(
            phone.total_online_secs, 100,
            "cumulative online time summed across the lineage"
        );
        assert!(
            out.iter().any(|m| m.node_id == "self1"),
            "the self node is never merged"
        );
        assert!(
            out.iter().any(|m| m.node_id == "ipad1"),
            "a sole device is left intact"
        );
    }
}
