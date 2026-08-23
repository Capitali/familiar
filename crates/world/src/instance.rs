//! The household's `WorldInstance` provisioning record (ADR-0045 §8, §9).
//!
//! The station ceremony's HUMAN ACTS — name, commission, correct, revoke — are reused
//! here; its topology is not. A world instance is neither a household member nor a
//! sibling mesh: no group certificate, no worldview, no record sync. What the household
//! keeps is the minimal record of a real software relationship.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Error;

/// Registry path, relative to the HOUSEHOLD data dir. The ship stores themselves live
/// elsewhere (the commissioner chooses where); only this record lives with the household.
pub const INSTANCES_FILE: &str = "worlds/instances.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    /// Commissioned and live: its keys are honored at the bridge.
    Commissioned,
    /// Decommissioned: authority ended. The record stays — decommission revokes keys and
    /// grants immediately, and the STORE's fate is a separate explicit human retention
    /// act (§9). History is never silently destroyed.
    Decommissioned,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldInstance {
    /// Stable, opaque id (`world-<hex16>`) — what every bridge envelope names.
    pub id: String,
    /// Human-given label ("Purr aboard the Long Haul"). Cosmetic; never identity.
    pub label: String,
    /// The human who commissioned it — a world begins by a human's word, like every
    /// identity in this system.
    pub commissioner: String,
    /// The instance's own ed25519 public key, hex — its cryptographic principal.
    pub instance_pubkey: String,
    /// Where the ship process answers, if anywhere yet. Operational, not trusted.
    pub endpoint: String,
    pub lifecycle: Lifecycle,
    /// Grant ids currently held (ADR-0044 machinery when it arrives; empty in v1).
    pub active_grants: Vec<String>,
    /// Authority epoch: bumped on decommission (and on future revocations). A bridge
    /// envelope carrying an older epoch is refused — an epoch is authority, not identity.
    pub grant_epoch: u64,
    pub created_at: i64,
}

fn registry_path(dir: &Path) -> PathBuf {
    dir.join(INSTANCES_FILE)
}

pub fn load(dir: &Path) -> Result<Vec<WorldInstance>, Error> {
    match fs::read(registry_path(dir)) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| Error::Refused(format!("instances registry is malformed: {e}"))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

pub fn find(dir: &Path, id: &str) -> Result<Option<WorldInstance>, Error> {
    Ok(load(dir)?.into_iter().find(|w| w.id == id))
}

fn save(dir: &Path, all: &[WorldInstance]) -> Result<(), Error> {
    let path = registry_path(dir);
    let parent = path
        .parent()
        .ok_or_else(|| Error::Io("no registry parent".into()))?;
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".instances-{}.tmp", std::process::id()));
    fs::write(
        &tmp,
        serde_json::to_vec_pretty(all).map_err(|e| Error::Io(e.to_string()))?,
    )?;
    fs::rename(tmp, path)?;
    Ok(())
}

/// Commission a world instance: create its OWN store at `store_root/<id>`, mint its own
/// node key there (the household never holds the private half — it is written only into
/// the ship's store), write a fully CLOSED kernel boundary into the ship store (the ship
/// starts with nothing; authority arrives only as a lease), and append the household's
/// provisioning record.
pub fn commission(
    household_dir: &Path,
    store_root: &Path,
    label: &str,
    commissioner: &str,
    endpoint: &str,
    now: i64,
) -> Result<(WorldInstance, PathBuf), Error> {
    let label = label.trim();
    let commissioner = commissioner.trim();
    if label.is_empty() || commissioner.is_empty() {
        return Err(Error::Refused(
            "commissioning needs a label and a commissioner".into(),
        ));
    }
    let id = format!("world-{}", crate::random_hex16()?);
    let ship_dir = store_root.join(&id);
    if ship_dir.exists() {
        return Err(Error::Refused(format!("store already exists at {id}")));
    }
    fs::create_dir_all(&ship_dir)?;

    // The ship's own principal, minted in the ship's own store.
    let key = familiar_mesh::node::NodeKey::load_or_mint(&ship_dir, label)
        .map_err(|e| Error::Io(format!("mint ship key: {e}")))?;
    let pubkey = key.identity().pubkey;

    // The ship is born with every gate shut. Its working authority is only ever the
    // signed, expiring lease (crate::lease) — this file is the fail-closed floor.
    let closed = familiar_kernel::boundary::Boundary::closed();
    fs::write(
        ship_dir.join(familiar_kernel::boundary::BOUNDARY_FILE),
        serde_json::to_vec_pretty(&closed).map_err(|e| Error::Io(e.to_string()))?,
    )?;

    let record = WorldInstance {
        id,
        label: label.to_string(),
        commissioner: commissioner.to_string(),
        instance_pubkey: pubkey,
        endpoint: endpoint.trim().to_string(),
        lifecycle: Lifecycle::Commissioned,
        active_grants: Vec::new(),
        grant_epoch: 1,
        created_at: now,
    };
    let mut all = load(household_dir)?;
    all.push(record.clone());
    save(household_dir, &all)?;
    Ok((record, ship_dir))
}

/// Rename is a human correction of the cosmetic label — identity (id, pubkey) never moves.
pub fn rename(dir: &Path, id: &str, new_label: &str) -> Result<WorldInstance, Error> {
    let new_label = new_label.trim();
    if new_label.is_empty() {
        return Err(Error::Refused("a rename needs a non-empty label".into()));
    }
    let mut all = load(dir)?;
    let w = all
        .iter_mut()
        .find(|w| w.id == id)
        .ok_or_else(|| Error::Refused(format!("no world instance {id}")))?;
    w.label = new_label.to_string();
    let out = w.clone();
    save(dir, &all)?;
    Ok(out)
}

/// Decommission ends AUTHORITY, immediately: lifecycle flips, grants clear, the epoch
/// bumps so any in-flight envelope carrying the old epoch is refused at the bridge. The
/// ship's store is deliberately untouched — archive, export, or delete is an explicit
/// human retention act, not a side effect (§9).
pub fn decommission(dir: &Path, id: &str) -> Result<WorldInstance, Error> {
    let mut all = load(dir)?;
    let w = all
        .iter_mut()
        .find(|w| w.id == id)
        .ok_or_else(|| Error::Refused(format!("no world instance {id}")))?;
    w.lifecycle = Lifecycle::Decommissioned;
    w.active_grants.clear();
    w.grant_epoch += 1;
    let out = w.clone();
    save(dir, &all)?;
    Ok(out)
}
