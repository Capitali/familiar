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

use crate::group::Membership;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::members::Member;
use crate::worldview::Worldview;

pub const STANDING_FILE: &str = "standing.json";

/// What a reader is allowed to see — the projection ladder (ADR-0033): each rung shows
/// strictly more, and every rung is *real* — never a fake view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// The household: the real worldview.
    Full,
    /// A federated mesh reading with its mesh key: the guest projection plus our handle
    /// and what we declare (ADR-0033 §3). Granted by the welcome tap, never automatically.
    Sibling,
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

/// A member's decision about a guest, signed and carried over the mesh (ADR-0020, ADR-0025).
///
/// Any active member may decide, so the decision has to travel rather than sit on whichever node
/// the deciding console happened to be reading. It goes to the **minting door** — the one permanent
/// fixture (ADR-0018) — which is the only place a single authoritative answer can live. Everyone
/// else converges on it at the next exchange.
///
/// **First decision wins.** A later vote on an already-decided node is refused, not applied. Two
/// people tapping different buttons on two consoles must not produce a roll that flips depending on
/// packet order, and a member who disagrees should say so out loud rather than silently overwrite
/// someone. Reversing a settled decision is a deliberate act (`mesh standing grant|revoke`), not a
/// race.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandingVote {
    pub membership: Membership,
    /// The group's public key (hex) — lets the host verify the cert without holding the secret.
    pub group_pubkey: String,
    /// The node being decided about.
    pub subject: String,
    /// "grant" (recognise) or "deny" (not now).
    pub act: String,
    pub nonce: String,
    pub ts: i64,
}

impl StandingVote {
    /// The voter signed these exact bytes with the key its membership certifies — the same proof
    /// shape as a status heartbeat or an observation batch.
    pub fn verify_sig(&self, raw: &[u8], sig_hex: &str) -> Result<()> {
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};
        let pk = crate::exactly_32(
            &crate::hex_decode(&self.membership.node_pubkey)?,
            "node pubkey",
        )?;
        let key = VerifyingKey::from_bytes(&pk)
            .map_err(|_| Error::Untrusted("bad node pubkey".into()))?;
        let sig_bytes = crate::node::exactly_64(&crate::hex_decode(sig_hex)?, "sig")?;
        key.verify(raw, &Signature::from_bytes(&sig_bytes))
            .map_err(|_| Error::Untrusted("standing: vote signature did not verify".into()))
    }
}

/// Whether this node has already been decided — recognised, or denied within its retry window.
/// Used to refuse a second vote rather than let it overwrite the first.
pub fn already_decided(dir: &Path, subject: &str, now: i64) -> bool {
    if load(dir).full.iter().any(|n| n == subject) {
        return true;
    }
    crate::enroll::denied_for(dir, subject, now) > 0
}

/// Apply a verified vote. Membership is checked by the caller; this enforces first-decision-wins
/// and does the write.
pub fn apply_vote(dir: &Path, vote: &StandingVote, now: i64) -> Result<&'static str> {
    let subject = vote.subject.trim();
    if subject.is_empty() {
        return Err(Error::Malformed("standing: empty subject".into()));
    }
    if subject == vote.membership.node_id {
        return Err(Error::Untrusted(
            "standing: a node may not decide about itself".into(),
        ));
    }
    if already_decided(dir, subject, now) {
        return Err(Error::Untrusted("standing: already decided".into()));
    }
    match vote.act.as_str() {
        "grant" => {
            grant(dir, subject, "recognised by a member over the mesh")
                .map_err(|e| Error::Malformed(format!("standing: {e}")))?;
            Ok("recognised")
        }
        "deny" => {
            crate::enroll::deny(dir, subject, now)?;
            Ok("held off")
        }
        other => Err(Error::Malformed(format!(
            "standing: act {other}? — grant | deny"
        ))),
    }
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
    // Dual-write (ADR-0026 Phase 2): full standing = established + admitted on the record.
    let minted_by = std::fs::read_to_string(dir.join(crate::node::NODE_FILE))
        .ok()
        .and_then(|s| serde_json::from_str::<crate::node::NodeIdentity>(&s).ok())
        .map(|n| n.node_id)
        .unwrap_or_else(|| "local".into());
    let _ = crate::record::record_standing_grant(
        dir,
        &minted_by,
        node_id,
        note,
        crate::transport::now_secs(),
    );
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
    let _ = crate::record::record_standing_revoke(
        dir,
        node_id.trim(),
        crate::transport::now_secs(),
    );
    Ok(true)
}

pub fn load(dir: &Path) -> StandingRoll {
    std::fs::read_to_string(dir.join(STANDING_FILE))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// A reader's standing. Default deny: unlisted is a guest.
///
/// With `read_records` on (ADR-0026 Phase 2, after a clean `mesh doctor`), the answer comes
/// from the unified record instead of the roll: a member is a device whose two filters both
/// hold. A missing record is a guest — the safe direction to fail, same as a missing roll.
/// Flipping the flag back is the rollback; the roll keeps being dual-written either way.
pub fn standing_of(dir: &Path, node_id: &str) -> Standing {
    if node_id.is_empty() {
        return Standing::Guest;
    }
    let read_records = crate::config::load(dir)
        .map(|c| c.read_records)
        .unwrap_or(false);
    if read_records {
        return match crate::record::find_by_key(dir, node_id) {
            Some(r) if crate::record::derive_state(&r) == crate::record::RecordState::Member => {
                Standing::Full
            }
            _ => Standing::Guest,
        };
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
    // How many others are awaiting a decision is the household's business, not a guest's.
    view.guests_waiting = 0;
    view.standing_full.clear();
    // Whose device is claiming whom is entirely the household's business — a guest never
    // learns that another guest is knocking as "ian", let alone gets a key to vouch for.
    view.claims_waiting.clear();
    // The fire is inside the house: a guest sees no game, no players, no story.
    view.game = None;
    // A guest sees the arrivals too — the mesh greets, that is shape — but not who: labels
    // pseudonymize and handles fall to "someone", same rule as the roster.
    for a in view.arrivals.iter_mut() {
        a.label = pseudonym(&a.node_id);
        if !a.handle.is_empty() {
            a.handle = "someone".into();
        }
        // Origin is the household's verification evidence, never a fellow visitor's to see.
        a.lat = 0.0;
        a.lon = 0.0;
        a.addr.clear();
        a.build.clear();
        // Same for activity: when another arrival was last heard from is household evidence.
        a.last_seen = 0;
    }

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

    // That this mesh federates is shape; WITH WHOM is the household's business. Handles
    // pseudonymize (stable per reader-visible id), declarations and welcomers are withheld.
    for s in view.siblings.iter_mut() {
        s.handle = pseudonym(&s.group_id);
        s.group_id = pseudonym(&s.group_id);
        s.declared_areas.clear();
        s.offered_tools.clear();
        s.welcomed_by = String::new();
        s.lat = 0.0;
        s.lon = 0.0;
    }
    // What we declare is a promise to siblings, not a guest's to read.
    view.declared_areas.clear();

    // Addresses and pins are how a device reaches the mesh — a guest is a member and still needs
    // them, so they are NOT scrubbed here. They are not personal data; they are the door.
}

/// Rewrite a worldview into its **sibling projection** (ADR-0033 §3): the guest projection
/// plus our handle and what we declare. What a sibling never sees, at any trust level:
/// names, humans, faces, addresses, per-node positions, free text. `reader_group_id` is the
/// sibling mesh's id — it sees its own entry as itself; other siblings stay pseudonymized
/// (who else we federate with is the household's business).
pub fn to_sibling_view(view: &mut Worldview, reader_group_id: &str, our_handle: &str) {
    // Keep the reader's own sibling entry aside — it knows itself, and hiding it would make
    // the console look broken (same rule as a guest seeing its own node).
    let own = view
        .siblings
        .iter()
        .find(|s| s.group_id == reader_group_id)
        .cloned();
    let declared = view.declared_areas.clone();
    to_guest_view(view, reader_group_id);
    // The sibling rung adds back, deliberately and only: our handle, our declaration, and
    // the reader's own standing here.
    if !our_handle.trim().is_empty() {
        view.group_label = our_handle.to_string();
    }
    view.declared_areas = declared;
    if let Some(own) = own {
        if let Some(slot) = view
            .siblings
            .iter_mut()
            .find(|s| s.group_id == pseudonym(&own.group_id))
        {
            *slot = own;
        }
    }
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

    /// First decision wins — the property that keeps two consoles from producing a roll that
    /// flips with packet order.
    #[test]
    fn a_second_vote_on_a_decided_node_is_refused() {
        let dir = std::env::temp_dir().join(format!("standing-vote-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        const NOW: i64 = 1_785_000_000;

        assert!(!already_decided(&dir, "guest1", NOW));
        grant(&dir, "guest1", "recognised").unwrap();
        assert!(
            already_decided(&dir, "guest1", NOW),
            "a recognised node is decided"
        );

        // A denial also counts as decided, for as long as its retry window holds — and stops
        // counting once the window lapses, so "not now" really is not-now rather than never.
        assert!(!already_decided(&dir, "guest2", NOW));
        crate::enroll::deny(&dir, "guest2", NOW).unwrap();
        assert!(already_decided(&dir, "guest2", NOW));
        assert!(
            !already_decided(&dir, "guest2", NOW + crate::enroll::DENY_RETRY_SECS + 1),
            "a lapsed denial reopens the decision"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

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
