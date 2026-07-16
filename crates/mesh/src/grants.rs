//! The outbound grant store — a human-facing node's record of the decisions its human made on other
//! peers' [`AuthorityRequest`](crate::brief::AuthorityRequest)s.
//!
//! When a headless peer routes an authority need (approve an enrollment, answer a question, open a
//! gate it asked for), a human at *this* node decides, and the decision is written here. Each
//! outbound brief carries these grants so the target peer can apply them. A grant is a **human act**
//! — the CLI/Glass only writes one when the person actually decides — relayed under the covenant
//! trust that binds the group.
//!
//! Grants are pruned after a TTL: the target dedups on apply, so a grant only needs to ride a few
//! rounds to be sure it lands, then it can drop.

use crate::brief::AuthorityGrant;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

pub const GRANTS_FILE: &str = "mesh/grants_out.json";

/// How long a grant keeps riding outbound briefs before it's pruned. A few gossip rounds is plenty;
/// the target applies-and-dedups, so re-sending only guards against a dropped round.
pub const GRANT_TTL_SECS: i64 = 3600;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Store {
    grants: Vec<AuthorityGrant>,
}

fn path(dir: &Path) -> std::path::PathBuf {
    dir.join(GRANTS_FILE)
}

fn read(dir: &Path) -> Store {
    std::fs::read_to_string(path(dir))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write(dir: &Path, store: &Store) -> io::Result<()> {
    if let Some(parent) = path(dir).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path(dir), serde_json::to_vec_pretty(store)?)
}

/// Record a human's decision on a peer's authority request. Idempotent on (target, kind, ref_id) —
/// a re-decision replaces the prior one (a human may change their mind before it's applied).
pub fn record(dir: &Path, grant: AuthorityGrant) -> io::Result<()> {
    let mut store = read(dir);
    store
        .grants
        .retain(|g| !(g.target == grant.target && g.kind == grant.kind && g.ref_id == grant.ref_id));
    store.grants.push(grant);
    write(dir, &store)
}

/// The live grants to attach to an outbound brief, pruned of anything past its TTL.
pub fn active(dir: &Path, now: i64) -> Vec<AuthorityGrant> {
    let mut store = read(dir);
    let before = store.grants.len();
    store.grants.retain(|g| now - g.ts <= GRANT_TTL_SECS);
    if store.grants.len() != before {
        let _ = write(dir, &store);
    }
    store.grants
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(target: &str, kind: &str, ref_id: &str, ts: i64) -> AuthorityGrant {
        AuthorityGrant {
            by: "me".into(),
            target: target.into(),
            kind: kind.into(),
            ref_id: ref_id.into(),
            approved: true,
            note: String::new(),
            ts,
        }
    }

    #[test]
    fn records_dedups_and_prunes_by_ttl() {
        let dir = std::env::temp_dir().join(format!("familiar_grants_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        record(&dir, g("nodeA", "enrollment", "x", 100)).unwrap();
        record(&dir, g("nodeA", "enrollment", "x", 150)).unwrap(); // same subject → replaces
        record(&dir, g("nodeB", "gate", "allow_execute", 150)).unwrap();
        assert_eq!(active(&dir, 160).len(), 2, "one per (target,kind,ref); both fresh");

        // After the TTL the older grant is pruned.
        let live = active(&dir, 100 + GRANT_TTL_SECS + 200);
        assert!(live.iter().all(|x| x.ts >= 150));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
