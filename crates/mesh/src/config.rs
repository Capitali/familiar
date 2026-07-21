//! Mesh **tunables** — `mesh/config.json`. Non-constitutional knobs only.
//!
//! The authorization to federate lives in `boundary.json` (`allow_mesh`), never here. This
//! file holds *how* to gossip once permitted (interval, port) and *what kinds* of thing to
//! share (tools/knowledge on by default; identities off, opt-in per handle+group). A
//! missing file is the safe default: share tools + knowledge, share **no** identities.

use crate::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const CONFIG_FILE: &str = "mesh/config.json";

/// One human explicitly opted into cross-node sharing, scoped to one group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityOptin {
    pub handle: String,
    pub group: String,
}

/// Mesh tunables. `#[serde(default)]` so a partial/old file fills safe defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MeshConfig {
    /// Seconds between gossip rounds.
    pub gossip_interval_secs: u64,
    /// TCP port the mesh server binds on the tailnet IP.
    pub gossip_port: u16,
    /// Offer authored tools to peers (bodies still fetched on demand).
    pub share_tools: bool,
    /// Offer distilled patterns/knowledge to peers.
    pub share_knowledge: bool,
    /// Replicate observations to peers so every node holds the shared record — a true mesh where
    /// any peer can vanish and the rest still know what it knew. Derived data only, mesh-tagged and
    /// deduped by origin; a separate switch from `share_knowledge` since it moves more data.
    pub share_observations: bool,
    /// Master switch for identity sharing. Even when true, only handles listed in
    /// `identity_optin` for the relevant group are shared — this just gates the whole path.
    pub share_identities: bool,
    /// The explicit per-human, per-group opt-ins. Nothing about a human crosses unless it
    /// appears here (and `share_identities` is on).
    pub identity_optin: Vec<IdentityOptin>,
    /// Extra peer addresses to gossip with beyond tailnet enumeration (`ip` or `ip:port`).
    /// Useful for a two-instance test on one host, or explicit peering off-tailnet. Still
    /// fully signature/group-gated — a static peer earns no trust it can't prove.
    pub static_peers: Vec<String>,
    /// Accept signed observation batches from device agents (iPhone/Watch) at `/mesh/observe`.
    /// A separate human switch from `allow_mesh`: mesh federation can be on while device
    /// ingestion is off. Every batch is still cert-verified and signature-checked regardless.
    pub accept_observations: bool,
    /// This node has **no local human** to perform the human-gated acts (approve enrollments,
    /// answer questions, open gates). When true it routes those authority needs to human-facing
    /// peers instead of waiting on a human who will never arrive. A full peer otherwise — it still
    /// theorizes and (gates permitting) builds + tests code. Off by default (most nodes have a human).
    #[serde(default)]
    pub headless: bool,
    /// **Auto-admit any well-formed covenant request** — a standing invite. When true, a node that
    /// attests the Laws and reaches `/mesh/enroll-request` is admitted immediately, without a
    /// per-device tap. Convenient on a trusted network; leave off to review each joiner (`mesh
    /// pending`/`approve`). Off by default — admitting a member is a human act. This is the *admit*
    /// side of automatic peering; [`auto_peer`](Self::auto_peer) is the *seek* side.
    pub auto_accept_enrollments: bool,
    /// **Seek a covenant automatically** — the bootstrap side of automatic peering. When this node
    /// has *no group yet* and the mesh gate is open, it reaches out to each online tailnet peer and
    /// asks to join (attesting the Laws). Paired with a peer running `auto_accept_enrollments`, a
    /// fresh node self-enrolls without a manual `mesh request-join`. Never fires once we hold a
    /// covenant (it would replace it) and never switches an existing group. Off by default — the
    /// human opens the gate first; this only removes the last manual tap.
    ///
    /// When *no* group exists anywhere in reach, `auto_peer` also covers **auto-formation**: two
    /// (or more) ungrouped auto_peer nodes that discover each other form a group without a human
    /// tap — the lowest node id creates it and opens a bounded invite window so its peers enroll
    /// by covenant on their next round. The mesh thus needs any two nodes, not a designated
    /// founder host. See `docs/mesh.md`.
    #[serde(default)]
    pub auto_peer: bool,
    /// Discover peers on the local network by UDP broadcast beacons, alongside (not instead of)
    /// tailnet enumeration. Discovery only — a LAN-discovered peer earns zero trust it can't
    /// prove with a membership cert, exactly like a tailnet or static peer. On by default so
    /// nodes on one LAN mesh even when Tailscale is absent or down.
    pub lan_discovery: bool,
    /// UDP port the discovery beacons use (distinct from the TCP `gossip_port`).
    pub lan_port: u16,
}

impl Default for MeshConfig {
    fn default() -> Self {
        MeshConfig {
            gossip_interval_secs: 30,
            gossip_port: 47_100,
            share_tools: true,
            share_knowledge: true,
            share_observations: true,
            share_identities: false,
            identity_optin: Vec::new(),
            static_peers: Vec::new(),
            accept_observations: true,
            headless: false,
            auto_accept_enrollments: false,
            auto_peer: false,
            lan_discovery: true,
            lan_port: 47_101,
        }
    }
}

impl MeshConfig {
    /// Is this handle opted into sharing for this group?
    pub fn identity_opted_in(&self, handle: &str, group: &str) -> bool {
        self.share_identities
            && self
                .identity_optin
                .iter()
                .any(|o| o.handle == handle && o.group == group)
    }
}

/// Load `mesh/config.json`, or the safe defaults if absent.
pub fn load(dir: &Path) -> Result<MeshConfig> {
    match fs::read_to_string(dir.join(CONFIG_FILE)) {
        Ok(s) => Ok(serde_json::from_str(&s)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(MeshConfig::default()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("familiar_mesh_config_{tag}"));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn missing_file_is_safe_default() {
        let dir = tmp("missing");
        let c = load(&dir).unwrap();
        assert!(c.share_tools && c.share_knowledge);
        assert!(!c.share_identities, "identities never shared by default");
        assert!(!c.identity_opted_in("betty", "river"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn partial_file_fills_defaults_and_optin_is_scoped() {
        let dir = tmp("partial");
        fs::create_dir_all(dir.join("mesh")).unwrap();
        fs::write(
            dir.join(CONFIG_FILE),
            r#"{"share_identities":true,"identity_optin":[{"handle":"betty","group":"river"}]}"#,
        )
        .unwrap();
        let c = load(&dir).unwrap();
        assert_eq!(c.gossip_interval_secs, 30); // default filled
        assert!(c.identity_opted_in("betty", "river"));
        assert!(
            !c.identity_opted_in("betty", "other"),
            "opt-in is per-group"
        );
        assert!(!c.identity_opted_in("ian", "river"), "opt-in is per-handle");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn optin_requires_master_switch() {
        let mut c = MeshConfig::default();
        c.identity_optin.push(IdentityOptin {
            handle: "betty".into(),
            group: "river".into(),
        });
        // Listed, but the master switch is still off → not shared.
        assert!(!c.identity_opted_in("betty", "river"));
        c.share_identities = true;
        assert!(c.identity_opted_in("betty", "river"));
    }
}
