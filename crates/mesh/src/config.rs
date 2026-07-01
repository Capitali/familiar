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
}

impl Default for MeshConfig {
    fn default() -> Self {
        MeshConfig {
            gossip_interval_secs: 30,
            gossip_port: 47_100,
            share_tools: true,
            share_knowledge: true,
            share_identities: false,
            identity_optin: Vec::new(),
            static_peers: Vec::new(),
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
        assert!(!c.identity_opted_in("betty", "other"), "opt-in is per-group");
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
