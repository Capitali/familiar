//! Projection by standing — what the worldview looks like to the member that is reading it.
//!
//! Admission is automatic (ADR-0015): a signed, covenant-attesting node is admitted on sight, and
//! the human governs afterwards by review. That is a deliberate trade, but it left a hole — an
//! admitted stranger read the *same* worldview as the household, names and all. On 2026-07-29 an
//! anonymous tester installed through a public TestFlight link and auto-enrolled, which turned
//! that hole from a hypothesis into an event.
//!
//! So membership decides whether you may read; **standing** decides what you see. Full standing is
//! granted by the human, explicitly, one node at a time. Everything else is a guest, and a guest
//! reads a worldview with the same shape, cadence and timestamps as the real one, and none of the
//! identities.
//!
//! This is deliberately NOT a second, fake worldview. The structure, the counts, the times, the
//! presence and service curves are all real — which is what makes a guest view worth showing to a
//! reviewer, a new tester, or someone you are demonstrating the mesh to. Only the *who* and the
//! *what* are withheld.
//!
//! Default is deny. An unlisted node is a guest.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::members::Member;
use crate::worldview::Worldview;

pub const STANDING_FILE: &str = "standing.json";

/// What a reader is allowed to see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// The household: the real worldview.
    Full,
    /// Everyone else: same shape, no identities.
    Guest,
}

/// The human-maintained roll of who reads at full standing. Hand-editable JSON in the data dir;
/// absent or empty means *everyone is a guest*, which is the safe direction to fail.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StandingRoll {
    /// node_ids at full standing.
    #[serde(default)]
    pub full: Vec<String>,
    /// Free-text note per node_id, so the file explains itself a year from now.
    #[serde(default)]
    pub notes: HashMap<String, String>,
}

pub fn save(dir: &Path, roll: &StandingRoll) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(roll)?;
    std::fs::write(dir.join(STANDING_FILE), json)
}

/// Grant full standing — the explicit human act ADR-0020 requires. Idempotent; the note is
/// kept beside the id so the file explains itself a year from now. Returns false if the node
/// already stood.
pub fn grant(dir: &Path, node_id: &str, note: &str) -> std::io::Result<bool> {
    let node_id = node_id.trim();
    if node_id.is_empty() {
        return Ok(false);
    }
    let mut roll = load(dir);
    if roll.full.iter().any(|n| n == node_id) {
        return Ok(false);
    }
    roll.full.push(node_id.to_string());
    if !note.trim().is_empty() {
        roll.notes
            .insert(node_id.to_string(), note.trim().to_string());
    }
    save(dir, &roll)?;
    Ok(true)
}

/// Return a member to guest. The membership itself is untouched — this narrows what they see,
/// it does not remove them (that is `mesh abandon`). Returns false if they were not on the roll.
pub fn revoke(dir: &Path, node_id: &str) -> std::io::Result<bool> {
    let mut roll = load(dir);
    let before = roll.full.len();
    roll.full.retain(|n| n != node_id.trim());
    if roll.full.len() == before {
        return Ok(false);
    }
    roll.notes.remove(node_id.trim());
    save(dir, &roll)?;
    Ok(true)
}

pub fn load(dir: &Path) -> StandingRoll {
    std::fs::read_to_string(dir.join(STANDING_FILE))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// A reader's standing. Default deny: unlisted is a guest.
pub fn standing_of(dir: &Path, node_id: &str) -> Standing {
    if node_id.is_empty() {
        return Standing::Guest;
    }
    let roll = load(dir);
    if roll.full.iter().any(|n| n == node_id) {
        Standing::Full
    } else {
        Standing::Guest
    }
}

/// A stable, meaningless name for a node — same input, same output, so a guest sees a coherent
/// mesh across polls instead of names that shuffle every five seconds.
fn pseudonym(node_id: &str) -> String {
    let h = fnv(node_id);
    format!("peer-{:04x}", (h & 0xffff) as u16)
}

fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

/// The device kind from an actor ("phone:ian" → "phone"), which is structure rather than identity
/// and is worth keeping — a guest should still see that the mesh has a phone, a watch and a Mac.
fn kind_of(actor: &str) -> String {
    match actor.split_once(':') {
        Some((kind, _)) if !kind.is_empty() => kind.to_string(),
        _ => String::new(),
    }
}

fn anon_actor(actor: &str) -> String {
    let k = kind_of(actor);
    if k.is_empty() {
        String::new()
    } else {
        format!("{k}:someone")
    }
}

/// Relocate the whole mesh by one deterministic offset, so the *shape* survives — relative
/// positions, distances, the fact that two nodes are together and a third is far away — while the
/// absolute position, which is someone's home, does not. The offset is keyed to the reader, so two
/// guests cannot compare notes to triangulate the real location.
fn shift(lat: f64, lon: f64, dlat: f64, dlon: f64) -> (f64, f64) {
    if lat == 0.0 && lon == 0.0 {
        return (0.0, 0.0); // "unlocated" is itself information; keep it unlocated
    }
    let mut la = lat + dlat;
    if la > 85.0 {
        la -= 170.0;
    } else if la < -85.0 {
        la += 170.0;
    }
    let mut lo = lon + dlon;
    while lo > 180.0 {
        lo -= 360.0;
    }
    while lo < -180.0 {
        lo += 360.0;
    }
    (la, lo)
}

const REDACTED: &str = "—";

/// Rewrite a worldview into its guest projection, in place.
///
/// Kept, deliberately: every timestamp (`ts`, `first_seen`, `last_seen`, `created_at`,
/// `status_at`, `session_start`, `total_online_secs`), every count, the member kinds, OS families,
/// online/status/trust words, the gates, the meters, the graph edges, and the relative geometry of
/// the map. A guest sees a mesh that is genuinely alive and genuinely shaped like this one.
///
/// Removed: labels, actors, the humans served and present, addresses, and all free text — the
/// observation objects, theory questions, goal descriptions, reflections, service and frontier
/// names. That is where the people are.
pub fn to_guest_view(view: &mut Worldview, reader_node_id: &str) {
    let h = fnv(reader_node_id);
    // Deterministic per reader, spread over the globe.
    let dlat = ((h >> 8) & 0xff) as f64 / 255.0 * 120.0 - 60.0;
    let dlon = (h & 0xff) as f64 / 255.0 * 360.0 - 180.0;

    view.group_label = "Mesh".into();

    // The dialog screen stays exactly as it is — same panel, same affordance — but the question
    // itself is the familiar talking about its household, so a guest gets a real-shaped prompt
    // rather than this one.
    if !view.question.is_empty() {
        view.question = "What should I be paying attention to?".into();
    }
    // Who the question is addressed to names a person, so it goes with the rest of the names.
    view.question_owner = String::new();

    for m in view.members.iter_mut() {
        let is_reader = m.node_id == reader_node_id;
        anon_member(m, is_reader, dlat, dlon);
    }

    for p in view.peers.iter_mut() {
        p.label = pseudonym(&p.node_id);
    }

    for o in view.recent.iter_mut() {
        o.actor = anon_actor(&o.actor);
        o.object = REDACTED.into();
        o.context = String::new();
        o.source = String::new();
        // action, ts and confidence stay — the cadence and character of the feed is the point.
    }

    for t in view.theories.iter_mut() {
        t.question = REDACTED.into();
        t.theory = REDACTED.into();
        t.direction = REDACTED.into();
        t.answers.clear();
    }

    for r in view.humanity.iter_mut() {
        r.reflection = REDACTED.into();
        r.grounded_in = String::new();
    }

    for g in view.goals.iter_mut() {
        g.description = REDACTED.into();
        g.owner_human = String::new(); // names a person
        g.needs.clear();
        g.origin = String::new();
        g.produced = String::new();
        g.notes = String::new();
        // status, owner (a short node id) and every date stay.
    }

    for s in view.services.iter_mut() {
        s.name = REDACTED.into();
        s.seen_by = anon_actor(&s.seen_by);
        // `kind` stays: that the mesh sees an airplay and an mqtt is shape, not identity.
    }

    for f in view.frontier.iter_mut() {
        f.label = REDACTED.into();
        f.ip = String::new();
        // reach + open service kinds + last_seen stay.
    }

    // Addresses and pins are how a device reaches the mesh — a guest is a member and still needs
    // them, so they are NOT scrubbed here. They are not personal data; they are the door.
}

fn anon_member(m: &mut Member, is_reader: bool, dlat: f64, dlon: f64) {
    // The reader still sees itself as itself — it already knows its own name, and hiding it would
    // just make the console look broken.
    if !is_reader {
        m.label = pseudonym(&m.node_id);
        m.actor = anon_actor(&m.actor);
        m.human = if m.human.is_empty() {
            String::new()
        } else {
            "someone".into()
        };
        m.present_human = if m.present_human.is_empty() {
            String::new()
        } else {
            "someone".into()
        };
        // The confidence and provenance of a claim about a named person go with the name.
        m.present_confidence = 0.0;
        m.present_via = String::new();
    }
    m.detail = String::new();
    m.addr = String::new();
    let (la, lo) = shift(m.lat, m.lon, dlat, dlon);
    m.lat = la;
    m.lon = lo;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlisted_reader_is_a_guest_and_listed_is_full() {
        let dir = std::env::temp_dir().join(format!("standing-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // No file at all: everyone is a guest. Failing closed is the whole point.
        assert_eq!(standing_of(&dir, "abc123"), Standing::Guest);

        std::fs::write(
            dir.join(STANDING_FILE),
            r#"{"full":["abc123"],"notes":{"abc123":"ian's iPad"}}"#,
        )
        .unwrap();
        assert_eq!(standing_of(&dir, "abc123"), Standing::Full);
        assert_eq!(standing_of(&dir, "def456"), Standing::Guest);
        // An empty node id is never full standing.
        assert_eq!(standing_of(&dir, ""), Standing::Guest);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn pseudonyms_are_stable_and_distinct() {
        assert_eq!(pseudonym("node-a"), pseudonym("node-a"));
        assert_ne!(pseudonym("node-a"), pseudonym("node-b"));
    }

    #[test]
    fn actor_keeps_its_kind_and_loses_its_human() {
        assert_eq!(anon_actor("phone:ian"), "phone:someone");
        assert_eq!(anon_actor("watch:betty"), "watch:someone");
        assert_eq!(anon_actor(""), "");
    }

    #[test]
    fn shift_preserves_relative_geometry_and_wraps() {
        // Two nodes 1 degree apart stay 1 degree apart after relocation.
        let (a_lat, a_lon) = shift(47.0, -119.0, 10.0, 40.0);
        let (b_lat, b_lon) = shift(48.0, -118.0, 10.0, 40.0);
        assert!((b_lat - a_lat - 1.0).abs() < 1e-9);
        assert!((b_lon - a_lon - 1.0).abs() < 1e-9);
        // Longitude wraps into range rather than running off the globe.
        let (_, lo) = shift(0.0, 170.0, 0.0, 40.0);
        assert!((-180.0..=180.0).contains(&lo));
        // Unlocated stays unlocated — "we don't know where this is" is real information.
        assert_eq!(shift(0.0, 0.0, 10.0, 40.0), (0.0, 0.0));
    }
}
