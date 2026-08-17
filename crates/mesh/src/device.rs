//! The device's own record (ADR-0039): machine facts, deliberately named, related to
//! humans only through time-bounded associations — never conflated with the membership
//! record (which answers *may this thing be here*, ADR-0026) and never squatting the
//! establishment handle (which names a HUMAN). The roster's SystemName reads from here
//! first; the brief's label is the fallback for a device nobody has named yet.
//!
//! Device facts are mesh facts: they replicate door-to-door on the record-sync dial
//! (their own endpoints — `GET /mesh/devices`, `POST /mesh/device-sync` — so a door
//! built before this module simply 404s and loses nothing). No floats ride this record,
//! by design: everything signed-then-reserialized must survive every parser in the
//! fleet (the one-longitude lesson, 2026-08-13).

use crate::group::Membership;
use crate::node::NodeKey;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Where device records live, one JSON per device, beside `records/`.
const DEVICES_DIR: &str = "mesh/devices";

/// How far back a door offers device records on a sync, and the cap per offer —
/// deliberately the record-sync values: the two channels ride the same dial.
pub const DEVICE_SYNC_WINDOW_SECS: i64 = 48 * 60 * 60;
pub const DEVICE_SYNC_CAP: usize = 64;

/// A human's association with a device — current while `until` is open, history once
/// closed. The edge is the ONLY place a human fact may touch a device object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Association {
    pub handle: String,
    pub since: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<i64>,
}

/// A network this device has been seen on, with the sighting's age — so "where it
/// lives" can be answered without pretending an old address is a current one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkSeen {
    /// "tailnet" | "lan" | "public" — the same classing the console's address cell uses.
    pub kind: String,
    pub addr: String,
    pub last_seen: i64,
}

/// The device's rich record (ADR-0039 §1). Facts only a machine has; the name is the
/// one field a human gives it, and an empty name is honest — the roster masks it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DeviceRecord {
    /// ADR-0025's durable device identity — the membership record's key space.
    pub device_id: String,
    /// The SystemName ("MacOnStick", "Codex") — deliberate, human-given, never invented.
    #[serde(default)]
    pub name: String,
    /// "phone" | "tablet" | "watch" | "mac" | "linux" | "vps" | "hub" | "" (unknown).
    #[serde(default)]
    pub kind: String,
    /// How the device is held in the world: `carried` (it follows a person) | `fixed` (it is
    /// bound to a place and serves whoever is there) | "" (not yet established).
    ///
    /// Orthogonal to `kind`, and that is the whole point (ADR-0042). An iPhone on a wall and an
    /// iPhone in a pocket are the same hardware and completely different things, and until this
    /// axis existed the only slot that changed any behaviour was the human's name — so a
    /// device-shaped question got a human-shaped answer ("set the name to shared", Ian
    /// 2026-08-15). A fixed device has no owner; who uses it lives in `humans`, which ADR-0039
    /// already made plural for exactly this.
    #[serde(default)]
    pub posture: String,
    #[serde(default)]
    pub os: String,
    #[serde(default)]
    pub os_version: String,
    #[serde(default)]
    pub arch: String,
    /// What it can do — brief capabilities plus declared actuator surfaces (ADR-0032).
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// What it can sense — camera, mic, gps, ble, network-survey … (consent-gated use;
    /// this lists the hardware truth, the gates decide the reach).
    #[serde(default)]
    pub observation_interfaces: Vec<String>,
    /// The name DISCOVERY gave this device — its mDNS/tailnet hostname ("codex"), which
    /// is the name its human gave it on the device itself. Outranked by an explicit
    /// `name`, outranks everything else (Ian, 2026-08-14: autodiscovery should gather
    /// device names; router config must not be required).
    #[serde(default)]
    pub discovered_name: String,
    #[serde(default)]
    pub networks: Vec<NetworkSeen>,
    /// Humans associated, current and past (ADR-0039: the establishment binds identity;
    /// these edges carry the history the establishment cannot).
    #[serde(default)]
    pub humans: Vec<Association>,
    /// Last deliberate change to the FACTS (name, kind, os…); the union fields
    /// (networks, humans) merge on their own keys and ignore this.
    #[serde(default)]
    pub updated_at: i64,
}

fn path_of(dir: &Path, device_id: &str) -> PathBuf {
    dir.join(DEVICES_DIR).join(format!("{device_id}.json"))
}

pub fn load(dir: &Path, device_id: &str) -> Result<Option<DeviceRecord>> {
    let p = path_of(dir, device_id);
    if !p.exists() {
        return Ok(None);
    }
    let s = std::fs::read_to_string(&p)?;
    Ok(serde_json::from_str(&s).ok())
}

pub fn save(dir: &Path, rec: &DeviceRecord) -> Result<()> {
    if rec.device_id.trim().is_empty() {
        return Err(Error::Malformed("device record: empty device_id".into()));
    }
    std::fs::create_dir_all(dir.join(DEVICES_DIR))?;
    std::fs::write(
        path_of(dir, &rec.device_id),
        serde_json::to_vec_pretty(rec)?,
    )?;
    Ok(())
}

pub fn load_all(dir: &Path) -> Vec<DeviceRecord> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir.join(DEVICES_DIR)) {
        for e in entries.flatten() {
            if e.path().extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            if let Ok(s) = std::fs::read_to_string(e.path()) {
                if let Ok(r) = serde_json::from_str::<DeviceRecord>(&s) {
                    out.push(r);
                }
            }
        }
    }
    out.sort_by(|a, b| a.device_id.cmp(&b.device_id));
    out
}

/// Name a device (the human's deliberate act — CLI today, the console's Device screen
/// at Build 85). The reference resolves through the MEMBERSHIP records (a device you
/// can name is one the mesh knows; prefixes welcome, typos refuse — the doppelgänger
/// lesson), and the device record is created here if this is its first fact.
pub fn set_name(dir: &Path, node_ref: &str, name: &str, now: i64) -> Result<DeviceRecord> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Untrusted("a device name is required".into()));
    }
    let device_id = crate::record::resolve_node_id(dir, node_ref)?;
    let mut rec = load(dir, &device_id)?.unwrap_or_default();
    rec.device_id = device_id;
    rec.name = name.to_string();
    rec.updated_at = now;
    save(dir, &rec)?;
    Ok(rec)
}

/// The postures a device may be declared to hold. Anything else is refused rather than stored,
/// because a posture nobody can read is worse than none: the presence gate would silently fall
/// back to treating a station as carried, which is the exact bug this axis exists to end.
pub const POSTURES: &[&str] = &["carried", "fixed"];

impl DeviceRecord {
    /// Is this device bound to a place rather than carried by a person?
    ///
    /// The presence gate turns on this: a fixed device is always powered and always reporting,
    /// so its heartbeat is evidence that THE STATION IS UP and nothing whatever about who is
    /// near it. Unknown posture reads as *not* fixed — the familiar keeps the long-standing
    /// carried behaviour until a human says otherwise, rather than silently reclassifying a
    /// person's phone and suppressing real presence (ADR-0042 consequences).
    pub fn is_fixed(&self) -> bool {
        self.posture.eq_ignore_ascii_case("fixed")
    }
}

/// Declare how a device is held in the world. A human act, always — the familiar may propose a
/// posture from what it observes (T-176) but never enters one on its own, because the two
/// misclassifications fail in opposite directions: personal-read-as-station SUPPRESSES real
/// presence, station-read-as-personal MANUFACTURES it.
pub fn set_posture(dir: &Path, node_ref: &str, posture: &str, now: i64) -> Result<DeviceRecord> {
    let posture = posture.trim().to_ascii_lowercase();
    if !POSTURES.contains(&posture.as_str()) {
        return Err(Error::Untrusted(format!(
            "posture must be one of {} — got {posture:?}",
            POSTURES.join(" | ")
        )));
    }
    let device_id = crate::record::resolve_node_id(dir, node_ref)?;
    let mut rec = load(dir, &device_id)?.unwrap_or_default();
    rec.device_id = device_id;
    rec.posture = posture;
    rec.updated_at = now;
    save(dir, &rec)?;
    Ok(rec)
}

/// Words that are hardware, not names — a discovered "iPhone" names nothing.
fn is_generic_name(n: &str) -> bool {
    let l = n.trim().to_lowercase();
    l.is_empty()
        || matches!(
            l.as_str(),
            "iphone" | "ipad" | "watch" | "mac" | "macbook" | "imac" | "appletv" | "localhost"
        )
        || l.chars()
            .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.' || c == '-')
}

/// Adopt a DISCOVERED name (mDNS host, tailnet host) onto a device — trimmed of
/// `.local`-style suffixes, refused when generic, written only on change so the
/// every-read call sites stay cheap. Never touches the given `name`; the SystemName
/// ladder prefers that on its own.
pub fn set_discovered_name(dir: &Path, device_id: &str, raw: &str, now: i64) -> Result<()> {
    let cleaned = raw
        .trim()
        .trim_end_matches('.')
        .split('.')
        .next()
        .unwrap_or("")
        .to_string();
    if is_generic_name(&cleaned) || device_id.trim().is_empty() {
        return Ok(());
    }
    let mut rec = load(dir, device_id)?.unwrap_or_default();
    if rec.discovered_name == cleaned {
        return Ok(());
    }
    rec.device_id = device_id.to_string();
    rec.discovered_name = cleaned;
    rec.updated_at = now;
    save(dir, &rec)
}

/// Remember an address a member was SEEN at — its LAN face, tailnet face, NAT face
/// accumulate here over time, so a discovery at any interface can be associated back to
/// the member ("codex" at the LAN ip IS the iPad reading via the lighthouse — the
/// association Ian watched the map lose, 2026-08-14). Deduped by address, capped.
pub fn note_network(dir: &Path, device_id: &str, kind: &str, addr: &str, now: i64) -> Result<()> {
    let addr = addr.trim();
    if device_id.trim().is_empty() || addr.is_empty() || addr == "localhost" {
        return Ok(());
    }
    let mut rec = load(dir, device_id)?.unwrap_or_default();
    rec.device_id = device_id.to_string();
    if let Some(n) = rec.networks.iter_mut().find(|n| n.addr == addr) {
        // Refresh at most hourly — this runs on every worldview read.
        if now - n.last_seen < 3600 {
            return Ok(());
        }
        n.last_seen = now;
    } else {
        rec.networks.push(NetworkSeen {
            kind: kind.to_string(),
            addr: addr.to_string(),
            last_seen: now,
        });
        rec.networks.sort_by_key(|n| std::cmp::Reverse(n.last_seen));
        rec.networks.truncate(12);
    }
    save(dir, &rec)
}

/// Keep this node's own device record honest on every tick: machine facts refreshed,
/// the name never touched (a name is given, not derived). Cheap and idempotent.
pub fn refresh_self(dir: &Path, node_id: &str, now: i64) -> Result<()> {
    if node_id.trim().is_empty() {
        return Ok(());
    }
    let mut rec = load(dir, node_id)?.unwrap_or_default();
    let os = std::env::consts::OS;
    let kind = match os {
        "macos" => "mac",
        "linux" => "linux",
        _ => os,
    };
    let changed = rec.device_id != node_id
        || rec.os != os
        || rec.arch != std::env::consts::ARCH
        || rec.kind != kind
        || rec.os_version != crate::merge::os_release();
    if changed {
        rec.device_id = node_id.to_string();
        rec.os = os.to_string();
        rec.os_version = crate::merge::os_release();
        rec.arch = std::env::consts::ARCH.to_string();
        rec.kind = kind.to_string();
        rec.updated_at = now;
        save(dir, &rec)?;
    }
    Ok(())
}

/// Merge a sibling door's copy into ours: facts latest-wins by `updated_at` (a rename at
/// any door travels); associations union by (handle, since) with the latest `until`
/// winning (a closed edge stays closed — a deletion is not a state, ADR-0027); networks
/// union by address with the freshest sighting kept.
pub fn absorb(dir: &Path, incoming: &DeviceRecord) -> Result<DeviceRecord> {
    if incoming.device_id.trim().is_empty() {
        return Err(Error::Malformed("device-sync: empty device_id".into()));
    }
    let merged = match load(dir, &incoming.device_id)? {
        None => incoming.clone(),
        Some(local) => merge(&local, incoming),
    };
    save(dir, &merged)?;
    Ok(merged)
}

fn merge(a: &DeviceRecord, b: &DeviceRecord) -> DeviceRecord {
    let (facts, other) = if b.updated_at > a.updated_at {
        (b, a)
    } else {
        (a, b)
    };
    let mut out = facts.clone();
    // Associations: union on (handle, since); the latest word on `until` wins.
    for edge in &other.humans {
        match out
            .humans
            .iter_mut()
            .find(|e| e.handle == edge.handle && e.since == edge.since)
        {
            Some(mine) => {
                if edge.until.unwrap_or(i64::MAX) > mine.until.unwrap_or(i64::MAX)
                    || (mine.until.is_none() && edge.until.is_some())
                {
                    mine.until = edge.until;
                }
            }
            None => out.humans.push(edge.clone()),
        }
    }
    out.humans
        .sort_by(|x, y| (&x.handle, x.since).cmp(&(&y.handle, y.since)));
    // Networks: union on addr, freshest sighting kept.
    for net in &other.networks {
        match out.networks.iter_mut().find(|n| n.addr == net.addr) {
            Some(mine) => {
                if net.last_seen > mine.last_seen {
                    mine.last_seen = net.last_seen;
                    mine.kind = net.kind.clone();
                }
            }
            None => out.networks.push(net.clone()),
        }
    }
    out.networks.sort_by(|x, y| x.addr.cmp(&y.addr));
    out
}

/// The signed body of a device-sync — the record-sync proof shape verbatim (cert in our
/// group, cert certifies the signing key, signature over the canonical body).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSyncBody {
    pub node: crate::node::NodeIdentity,
    pub membership: Membership,
    pub ts: i64,
    pub nonce: String,
    pub devices: Vec<DeviceRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSync {
    pub body: DeviceSyncBody,
    /// ed25519 (hex) by the sending door's node key over `serde_json(body)`.
    pub sig: String,
}

/// Build + sign this door's device offer: records touched inside the window, capped.
/// None when there is nothing to say — the caller skips the POST entirely.
pub fn build_device_sync(
    dir: &Path,
    cred: &crate::group::GroupCredential,
    node: &NodeKey,
    now: i64,
) -> Result<Option<DeviceSync>> {
    let mut recent: Vec<DeviceRecord> = load_all(dir)
        .into_iter()
        .filter(|d| now - d.updated_at <= DEVICE_SYNC_WINDOW_SECS)
        .collect();
    if recent.is_empty() {
        return Ok(None);
    }
    recent.sort_by_key(|d| std::cmp::Reverse(d.updated_at));
    recent.truncate(DEVICE_SYNC_CAP);
    let body = DeviceSyncBody {
        node: node.identity(),
        membership: cred.membership.clone(),
        ts: now,
        nonce: format!("{now:016x}"),
        devices: recent,
    };
    let sig = node.sign(&serde_json::to_vec(&body)?);
    Ok(Some(DeviceSync { body, sig }))
}

/// Verify a device-sync came from a live member of OUR group — the record-sync checks.
pub fn verify_device_sync(
    sync: &DeviceSync,
    group_key: &ed25519_dalek::VerifyingKey,
    group_id: &str,
    now: i64,
    revoked: &[String],
) -> Result<()> {
    let b = &sync.body;
    crate::group::verify_membership(&b.membership, group_key, group_id, now, revoked)?;
    if b.membership.node_pubkey != b.node.pubkey || b.membership.node_id != b.node.node_id {
        return Err(Error::Untrusted(
            "device-sync: membership cert does not match the signing node".into(),
        ));
    }
    if now - b.ts > DEVICE_SYNC_WINDOW_SECS {
        return Err(Error::Untrusted("device-sync: stale".into()));
    }
    b.node.verify(&serde_json::to_vec(b)?, &sync.sig)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const NOW: i64 = 1_780_000_000;

    fn tmp(tag: &str) -> PathBuf {
        let p =
            std::env::temp_dir().join(format!("familiar_mesh_device_{}_{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn membership_record(dir: &Path, device_id: &str) {
        // The restoration path mints a guest record from enrollment evidence — the same
        // shape a knock leaves behind; enough for resolution to have something to name.
        crate::record::upsert_enrolled(dir, device_id, None, NOW - 100).unwrap();
    }

    #[test]
    fn a_name_is_given_through_the_membership_space_and_survives_merge() {
        let dir = tmp("name");
        membership_record(&dir, "d5c3147245250000");

        // The display prefix names the device; the record is created on first fact.
        let rec = set_name(&dir, "d5c31472", "Aphelion", NOW).unwrap();
        assert_eq!(rec.device_id, "d5c3147245250000");
        assert_eq!(rec.name, "Aphelion");

        // An unknown reference refuses — no ghost device records either.
        assert!(set_name(&dir, "beefbeef", "Nope", NOW).is_err());
        assert!(load(&dir, "beefbeef").unwrap().is_none());

        // A later rename at another door wins by updated_at, whichever side absorbs.
        let mut theirs = rec.clone();
        theirs.name = "Aphelion-2".into();
        theirs.updated_at = NOW + 10;
        let merged = absorb(&dir, &theirs).unwrap();
        assert_eq!(merged.name, "Aphelion-2");
        let mut stale = rec.clone();
        stale.name = "Old".into();
        stale.updated_at = NOW - 10;
        let merged = absorb(&dir, &stale).unwrap();
        assert_eq!(merged.name, "Aphelion-2", "an older rename never wins");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn discovered_names_are_cleaned_guarded_and_outranked_by_the_given_name() {
        let dir = tmp("disc");
        membership_record(&dir, "aa11aa11aa11aa11");
        // mDNS-style suffix trimmed; generic hardware words and bare addresses refused.
        set_discovered_name(&dir, "aa11aa11aa11aa11", "Codex.local.", NOW).unwrap();
        assert_eq!(
            load(&dir, "aa11aa11aa11aa11")
                .unwrap()
                .unwrap()
                .discovered_name,
            "Codex"
        );
        set_discovered_name(&dir, "aa11aa11aa11aa11", "iPhone", NOW + 1).unwrap();
        set_discovered_name(&dir, "aa11aa11aa11aa11", "192.168.1.7", NOW + 2).unwrap();
        assert_eq!(
            load(&dir, "aa11aa11aa11aa11")
                .unwrap()
                .unwrap()
                .discovered_name,
            "Codex",
            "generic words and addresses never overwrite a discovered name"
        );
        // networks accumulate deduped, and the hourly throttle skips hot rewrites.
        note_network(&dir, "aa11aa11aa11aa11", "lan", "192.168.108.42", NOW).unwrap();
        note_network(&dir, "aa11aa11aa11aa11", "lan", "192.168.108.42", NOW + 10).unwrap();
        note_network(&dir, "aa11aa11aa11aa11", "tailnet", "100.64.9.9", NOW + 20).unwrap();
        let d = load(&dir, "aa11aa11aa11aa11").unwrap().unwrap();
        assert_eq!(d.networks.len(), 2);
        assert_eq!(
            d.networks
                .iter()
                .find(|n| n.addr == "192.168.108.42")
                .unwrap()
                .last_seen,
            NOW,
            "a sighting inside the hour is not rewritten"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn associations_union_and_a_closed_edge_stays_closed() {
        let dir = tmp("edges");
        let a = DeviceRecord {
            device_id: "aa00".into(),
            humans: vec![Association {
                handle: "ian".into(),
                since: NOW,
                until: None,
            }],
            updated_at: NOW,
            ..Default::default()
        };
        save(&dir, &a).unwrap();
        // A sibling knows the same edge CLOSED, plus an older one we never saw.
        let b = DeviceRecord {
            device_id: "aa00".into(),
            humans: vec![
                Association {
                    handle: "ian".into(),
                    since: NOW,
                    until: Some(NOW + 100),
                },
                Association {
                    handle: "betty".into(),
                    since: NOW - 500,
                    until: Some(NOW - 100),
                },
            ],
            updated_at: NOW - 50, // older FACTS — the union must still land
            ..Default::default()
        };
        let merged = absorb(&dir, &b).unwrap();
        assert_eq!(merged.humans.len(), 2);
        let ian = merged.humans.iter().find(|e| e.handle == "ian").unwrap();
        assert_eq!(ian.until, Some(NOW + 100), "the closed edge travels");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_device_sync_round_trips_and_refuses_a_stranger() {
        let dir = tmp("sync");
        let node = NodeKey::load_or_mint(&dir, "door").unwrap();
        let cred =
            crate::group::create_group(&dir, &node, "g", NOW, crate::group::DEFAULT_CERT_TTL_SECS)
                .unwrap();
        membership_record(&dir, "aa11aa11aa11aa11");
        set_name(&dir, "aa11aa11", "Codex", NOW).unwrap();

        let sync = build_device_sync(&dir, &cred, &node, NOW).unwrap().unwrap();
        let wire = serde_json::to_vec(&sync).unwrap();
        let back: DeviceSync = serde_json::from_slice(&wire).unwrap();
        let gk = cred.verifying_key().unwrap();
        assert!(verify_device_sync(&back, &gk, &cred.group_id, NOW + 1, &[]).is_ok());

        // A stranger's signature (right shape, wrong group) is refused.
        let dir2 = tmp("sync2");
        let node2 = NodeKey::load_or_mint(&dir2, "other").unwrap();
        let theirs = crate::group::create_group(
            &dir2,
            &node2,
            "theirs",
            NOW,
            crate::group::DEFAULT_CERT_TTL_SECS,
        )
        .unwrap();
        membership_record(&dir2, "bb22bb22bb22bb22");
        set_name(&dir2, "bb22bb22", "Intruder", NOW).unwrap();
        let foreign = build_device_sync(&dir2, &theirs, &node2, NOW)
            .unwrap()
            .unwrap();
        assert!(verify_device_sync(&foreign, &gk, &cred.group_id, NOW + 1, &[]).is_err());
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&dir2);
    }

    // ADR-0042 — posture is the axis that keeps a device-shaped question from getting a
    // human-shaped answer.

    #[test]
    fn an_unset_posture_reads_as_carried_never_as_a_station() {
        let d = DeviceRecord::default();
        assert!(
            !d.is_fixed(),
            "unknown posture must keep the long-standing carried behaviour: reading it as \
             fixed would silently suppress a real person's presence"
        );
    }

    #[test]
    fn posture_refuses_a_word_the_presence_gate_cannot_read() {
        let dir = tmp("posture_refuses");
        let err = set_posture(&dir, "dev1", "shared", 10).unwrap_err();
        assert!(
            format!("{err}").contains("carried"),
            "the refusal must name what is allowed: {err}"
        );
    }
}
