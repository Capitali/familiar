//! The dossier — remembering a human well enough to serve them (ADR-0022).
//!
//! This is the familiar's interior life, not a feature: to serve someone you have to
//! know where they tend to be, when, and what they keep needing — otherwise the
//! familiar can only ever react to whoever is standing in front of it. Described
//! neutrally, that is a surveillance dossier, and the ADR uses the word deliberately;
//! the constraints here are the design, not follow-up:
//!
//! - **Patterns, not tape — structurally.** Every observation feeds a pattern as a
//!   *contribution*: a weight accumulated into a slot ([`Contribution`]), never a
//!   query over retained history. The raw record can age out; the shape survives.
//! - **Node-local, never federated.** No mesh path reads this table. A brief never
//!   carries a dossier; a mesh-wide picture of a person is exactly what must not exist.
//! - **The subject can read it** (`familiar dossier <handle>`), and
//! - **the subject can remove themselves** ([`withdraw`]): weights zeroed (rows
//!   dropped), the face link cleared, and a tombstone so no later fold re-materialises
//!   them. The receipt reports what actually went.
//! - **Confidence travels.** A pattern from three sightings is presented as three
//!   sightings' worth of belief, never as fact (Laplace prior, the score.rs idiom).
//! - **Consent-gated at the source.** The fold introduces no new collection; it can
//!   only be as rich as the sensing the human already opened.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

use crate::identity;
use crate::observation::{self, Observation};
use crate::question;
use crate::routing;
use crate::store;
use crate::thread;

pub const DOSSIER_FILE: &str = "dossier.jsonl";
/// Last observation `seq` the fold has consumed — the resumable cursor
/// (`store::load_since_seq`). Plain text beside the other tiny pointers.
const CURSOR_FILE: &str = "dossier_cursor.txt";
/// Observations folded per call — bounded so a tick never stalls on a long backlog;
/// the cursor resumes next tick.
const FOLD_BATCH: usize = 512;
/// Laplace phantom sightings: `confidence = count / (count + PRIOR)`. Three real
/// sightings ≈ 0.43 — a thin history stays humble (the score.rs idiom).
const PRIOR_STRENGTH: f64 = 4.0;

/// One contribution-scored slot of a per-human pattern. The id is
/// `ctb|<handle>|<kind>|<slot>` — most-significant-part first, so `load_prefix`
/// range-scans one person, or one person's one pattern, off the existing id index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contribution {
    pub id: String,
    pub subject: String,
    /// `"presence"` (when they tend to be here) | `"standing"` (how they are usually
    /// identified). `"habit"` is the anticipated next kind — additive, no schema change.
    pub kind: String,
    /// presence: `"h00"`..`"h23"` (UTC hour). standing: the evidence tier from
    /// `routing::subject_and_strength` (`"face"|"dialogue"|"motion"|"activity"`).
    pub slot: String,
    /// Decayed accumulation of evidence strengths. Decay is lazy-exponential — applied
    /// when the slot is written and when it is read — so dismissal-by-time needs no sweep.
    pub weight: f64,
    /// Undecayed contribution count — the confidence carrier (ADR-0022 constraint 6).
    pub count: u64,
    pub first_at: i64,
    pub last_at: i64,
}

/// A withdrawal tombstone (`wdr|<handle>`): the person removed themselves, and no later
/// fold — including one from a rewound cursor or a future rebuild — may re-materialise
/// them from raw observations still in the log. Without this, withdrawal would fail
/// quietly in exactly the way ADR-0022 warns contribution scoring can.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Withdrawal {
    id: String,
    subject: String,
    at: i64,
}

fn ctb_id(handle: &str, kind: &str, slot: &str) -> String {
    format!("ctb|{handle}|{kind}|{slot}")
}

fn withdrawn(dir: &Path, handle: &str) -> bool {
    store::load_by_id::<Withdrawal>(dir, DOSSIER_FILE, &format!("wdr|{handle}"))
        .ok()
        .flatten()
        .is_some()
}

fn read_cursor(dir: &Path) -> i64 {
    fs::read_to_string(dir.join(CURSOR_FILE))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

/// What remains of a weight after `elapsed` seconds of silence. Out-of-order commits can
/// put an observation's ts before the slot's `last_at`; time never runs backwards here.
fn decayed(weight: f64, elapsed: i64, half_life_secs: i64) -> f64 {
    let elapsed = elapsed.max(0) as f64;
    weight * 0.5_f64.powf(elapsed / half_life_secs.max(1) as f64)
}

/// Fold observations newer than the cursor into per-human contributions. Resumable and
/// bounded: the cursor advances only after a batch is applied, so a crash replays a
/// bounded batch (upserts keyed on slot make the replay idempotent in shape, and the
/// double-count window is one batch at worst). Returns how many observations were read.
///
/// Exclusions are part of the design, not tuning:
/// - `actor == "familiar"` — the factory is not a human, and its self-reports pattern
///   like a person's motion tier (the phantom-human hazard).
/// - `mesh:*` actors and sources — this node's dossier is built from this node's own
///   sensing; a merged mesh-wide picture of a person must not exist.
/// - withdrawn subjects — the tombstone outranks the log.
///
/// Everything else rides `routing::subject_and_strength`, the one evidence ladder.
pub fn fold(dir: &Path, half_life_secs: i64) -> io::Result<usize> {
    let mut cursor = read_cursor(dir);
    let mut read = 0usize;
    loop {
        let batch: Vec<(i64, Observation)> =
            store::load_since_seq(dir, observation::OBSERVATIONS_FILE, cursor, FOLD_BATCH)?;
        if batch.is_empty() {
            break;
        }
        for (_, o) in &batch {
            if o.actor == "familiar" || o.actor.starts_with("mesh") || o.source.starts_with("mesh")
            {
                continue;
            }
            let Some((who, strength, via)) = routing::subject_and_strength(o) else {
                continue;
            };
            if withdrawn(dir, &who) {
                continue;
            }
            let hour = (o.ts.rem_euclid(86_400)) / 3_600;
            contribute(dir, &who, "presence", &format!("h{hour:02}"), strength, o.ts, half_life_secs)?;
            contribute(dir, &who, "standing", via, strength, o.ts, half_life_secs)?;
        }
        read += batch.len();
        cursor = batch.last().map(|(seq, _)| *seq).unwrap_or(cursor);
        fs::write(dir.join(CURSOR_FILE), cursor.to_string())?;
        if batch.len() < FOLD_BATCH {
            break;
        }
    }
    Ok(read)
}

/// Accumulate one weighted contribution into a slot — decay what was there, add what
/// was seen, upsert. The first contribution creates the slot.
fn contribute(
    dir: &Path,
    handle: &str,
    kind: &str,
    slot: &str,
    strength: f64,
    ts: i64,
    half_life_secs: i64,
) -> io::Result<()> {
    let id = ctb_id(handle, kind, slot);
    let mut c = store::load_by_id::<Contribution>(dir, DOSSIER_FILE, &id)?.unwrap_or(Contribution {
        id: id.clone(),
        subject: handle.to_string(),
        kind: kind.to_string(),
        slot: slot.to_string(),
        weight: 0.0,
        count: 0,
        first_at: ts,
        last_at: ts,
    });
    c.weight = decayed(c.weight, ts - c.last_at, half_life_secs) + strength;
    c.count += 1;
    c.last_at = c.last_at.max(ts);
    store::upsert_by_id(dir, DOSSIER_FILE, &id, &c)?;
    Ok(())
}

/// One slot of a pattern as presented: its share of the pattern's decayed weight, and
/// how much belief the sighting count actually supports.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SlotView {
    pub slot: String,
    /// This slot's fraction of the pattern's total decayed weight, in [0, 1].
    pub share: f64,
    /// `count / (count + PRIOR_STRENGTH)` — three sightings ≈ 0.43, never a fact.
    pub confidence: f64,
    pub count: u64,
}

/// A need as the dossier presents it: `stated` distinguishes what the person said
/// (origin `"observer"`) from what the familiar merely theorizes about them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NeedView {
    pub id: String,
    pub text: String,
    pub status: String,
    pub stated: bool,
}

/// Everything the familiar holds about one person — the record the subject reads
/// (ADR-0022 constraint 3: a record kept about someone that they cannot see is
/// surveillance, whatever its purpose).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Dossier {
    pub identity: Option<identity::Identity>,
    /// 24 slots, `h00`..`h23`, decayed to `now`; shares sum to ~1 when any weight remains.
    pub presence_hours: Vec<SlotView>,
    /// Evidence tiers, strongest first.
    pub standing: Vec<SlotView>,
    /// Open threads about or from this person, plus unanswered questions addressed to them.
    pub needs: Vec<NeedView>,
    pub withdrawn: bool,
}

/// Read one person's dossier, decayed to `now`. The needs list is composed from
/// threads and questions at read time (they are already derived, bounded records —
/// only presence and standing are contribution-scored).
pub fn read(dir: &Path, handle: &str, now: i64, half_life_secs: i64) -> io::Result<Dossier> {
    let handle = handle.trim().to_lowercase();
    let contributions: Vec<Contribution> =
        store::load_prefix(dir, DOSSIER_FILE, &format!("ctb|{handle}|"))?;
    let view = |kind: &str| -> Vec<SlotView> {
        let of_kind: Vec<&Contribution> =
            contributions.iter().filter(|c| c.kind == kind).collect();
        let total: f64 = of_kind
            .iter()
            .map(|c| decayed(c.weight, now - c.last_at, half_life_secs))
            .sum();
        let mut out: Vec<SlotView> = of_kind
            .iter()
            .map(|c| SlotView {
                slot: c.slot.clone(),
                share: if total > 0.0 {
                    decayed(c.weight, now - c.last_at, half_life_secs) / total
                } else {
                    0.0
                },
                confidence: c.count as f64 / (c.count as f64 + PRIOR_STRENGTH),
                count: c.count,
            })
            .collect();
        out.sort_by(|a, b| b.share.partial_cmp(&a.share).unwrap_or(std::cmp::Ordering::Equal));
        out
    };
    let mut presence_hours = view("presence");
    // Present all 24 hours, absent ones at zero — a gap is information too.
    for h in 0..24 {
        let slot = format!("h{h:02}");
        if !presence_hours.iter().any(|s| s.slot == slot) {
            presence_hours.push(SlotView { slot, share: 0.0, confidence: 0.0, count: 0 });
        }
    }
    presence_hours.sort_by(|a, b| a.slot.cmp(&b.slot));

    let threads = thread::load(dir).unwrap_or_default();
    let questions = question::load(dir).unwrap_or_default();
    let mut needs: Vec<NeedView> = threads
        .iter()
        .filter(|t| {
            (t.origin_human == handle || (t.origin == "observer" && t.actor == handle))
                && matches!(t.status.as_str(), "open" | "pursued")
        })
        .map(|t| NeedView {
            id: t.id.clone(),
            text: t.theory.clone(),
            status: t.status.clone(),
            stated: t.origin == "observer",
        })
        .collect();
    needs.extend(
        questions
            .iter()
            .filter(|q| q.subject == handle && !q.answered)
            .map(|q| NeedView {
                id: q.id.clone(),
                text: q.text.clone(),
                status: "asked".into(),
                stated: false,
            }),
    );

    Ok(Dossier {
        identity: identity::load(dir)?.into_iter().find(|p| p.handle == handle),
        presence_hours,
        standing: view("standing"),
        needs,
        withdrawn: withdrawn(dir, &handle),
    })
}

/// A coarse, spoken-sized summary — what a muse prompt or a console line may carry.
/// Deliberately this and no more: the raw distribution is as sensitive as the readings
/// it came from and stays on the node; a sentence at this coarseness is grounding.
pub fn coarse_summary(d: &Dossier) -> String {
    let day_part = |slot: &str| -> &'static str {
        match slot.trim_start_matches('h').parse::<i64>().unwrap_or(0) {
            5..=11 => "in the morning",
            12..=17 => "in the afternoon",
            18..=22 => "in the evening",
            _ => "at night",
        }
    };
    let mut parts: Vec<String> = Vec::new();
    if let Some(best) = d
        .presence_hours
        .iter()
        .filter(|s| s.count > 0)
        .max_by(|a, b| a.share.partial_cmp(&b.share).unwrap_or(std::cmp::Ordering::Equal))
    {
        parts.push(format!(
            "usually here {} (confidence {:.1})",
            day_part(&best.slot),
            best.confidence
        ));
    }
    if let Some(via) = d.standing.first() {
        parts.push(format!("identified mostly by {}", via.slot));
    }
    if parts.is_empty() {
        "not yet known well enough to say".to_string()
    } else {
        parts.join("; ")
    }
}

/// What actually went, reported rather than "succeeded" (the store's own doctrine).
#[derive(Debug, Clone, PartialEq)]
pub struct WithdrawalReceipt {
    pub contributions_removed: usize,
    pub face_unlinked: bool,
}

/// ADR-0022 constraint 4 — the subject removes themselves: their contribution rows are
/// dropped (weight set to zero, structurally), the face link is cleared, and a tombstone
/// keeps every later fold from re-materialising them. The honest boundary, stated so it
/// is not discovered later: this does not unpick arithmetic — aggregate structure that no
/// longer identifies them survives; nothing that points at them does.
pub fn withdraw(dir: &Path, handle: &str, now: i64) -> io::Result<WithdrawalReceipt> {
    let handle = handle.trim().to_lowercase();
    let removed = store::delete_prefix(dir, DOSSIER_FILE, &format!("ctb|{handle}|"))?;
    let face_unlinked = identity::link_face(dir, &handle, None)?.is_some();
    let id = format!("wdr|{handle}");
    store::upsert_by_id(
        dir,
        DOSSIER_FILE,
        &id,
        &Withdrawal { id: id.clone(), subject: handle.clone(), at: now },
    )?;
    observation::record(
        dir,
        Observation::new(
            "familiar",
            "withdrew",
            format!("dossier:{handle}"),
            format!("{removed} contributions removed; face link cleared: {face_unlinked}"),
            "familiar",
            now,
            1.0,
        ),
    )?;
    Ok(WithdrawalReceipt { contributions_removed: removed, face_unlinked })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const HL: i64 = 30 * 24 * 3600; // 30-day half-life, in seconds

    fn dir(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn obs(actor: &str, action: &str, object: &str, ts: i64) -> Observation {
        Observation::new(actor, action, object, "", "test", ts, 1.0)
    }

    #[test]
    fn fold_accumulates_by_hour_with_the_evidence_ladders_weights() {
        let p = dir("substrate_dossier_fold");
        // 19:00 UTC dialogue (0.9) and a same-hour watch presence beacon (0.4).
        let t19 = 19 * 3600;
        observation::record(&p, obs("phone:betty", "told the familiar", "hello", t19)).unwrap();
        observation::record(&p, obs("watch:betty", "reports", "presence", t19 + 60)).unwrap();
        fold(&p, HL).unwrap();
        let c: Contribution =
            store::load_by_id(&p, DOSSIER_FILE, "ctb|betty|presence|h19").unwrap().unwrap();
        assert_eq!(c.count, 2);
        assert!((c.weight - 1.3).abs() < 0.01, "0.9 dialogue + 0.4 beacon, barely decayed");
        let s: Contribution =
            store::load_by_id(&p, DOSSIER_FILE, "ctb|betty|standing|dialogue").unwrap().unwrap();
        assert_eq!(s.count, 1, "the evidence tier is remembered too");
        let _ = fs::remove_dir_all(&p);
    }

    #[test]
    fn decay_halves_a_slot_after_one_half_life_and_fades_on_read() {
        let p = dir("substrate_dossier_decay");
        let t = 8 * 3600;
        observation::record(&p, obs("phone:betty", "told the familiar", "hi", t)).unwrap();
        fold(&p, HL).unwrap();
        let d = read(&p, "betty", t + HL, HL).unwrap();
        let h8 = d.presence_hours.iter().find(|s| s.slot == "h08").unwrap();
        // Share is normalized (only slot → 1.0), but the raw weight has halved:
        let c: Contribution =
            store::load_by_id(&p, DOSSIER_FILE, "ctb|betty|presence|h08").unwrap().unwrap();
        assert!((decayed(c.weight, HL, HL) - 0.45).abs() < 0.01, "0.9 → 0.45 after one half-life");
        assert_eq!(h8.count, 1, "the count does not decay — it carries confidence");
        let _ = fs::remove_dir_all(&p);
    }

    #[test]
    fn the_cursor_resumes_without_double_counting() {
        let p = dir("substrate_dossier_cursor");
        observation::record(&p, obs("phone:betty", "told the familiar", "hi", 100)).unwrap();
        fold(&p, HL).unwrap();
        fold(&p, HL).unwrap(); // nothing new — must not re-fold
        let c: Contribution =
            store::load_by_id(&p, DOSSIER_FILE, "ctb|betty|presence|h00").unwrap().unwrap();
        assert_eq!(c.count, 1, "a second fold over the same log adds nothing");
        let _ = fs::remove_dir_all(&p);
    }

    #[test]
    fn observer_familiar_and_mesh_actors_feed_no_dossier() {
        let p = dir("substrate_dossier_excluded");
        observation::record(&p, obs("observer", "told the familiar", "hi", 100)).unwrap();
        observation::record(&p, obs("familiar", "reports", "theory_quality:0.5", 100)).unwrap();
        observation::record(&p, obs("mesh:node-a", "reports", "presence", 100)).unwrap();
        let mut peer = obs("phone:betty", "told the familiar", "hi", 100);
        peer.source = "mesh:node-a".into();
        observation::record(&p, peer).unwrap();
        fold(&p, HL).unwrap();
        let all: Vec<Contribution> = store::load_prefix(&p, DOSSIER_FILE, "ctb|").unwrap();
        assert!(all.is_empty(), "no human here — and no mesh-merged picture of one: {all:?}");
        let _ = fs::remove_dir_all(&p);
    }

    #[test]
    fn thin_history_presents_humbly() {
        let p = dir("substrate_dossier_humble");
        for i in 0..3 {
            observation::record(&p, obs("phone:betty", "told the familiar", "hi", 100 + i)).unwrap();
        }
        fold(&p, HL).unwrap();
        let d = read(&p, "betty", 200, HL).unwrap();
        let h0 = d.presence_hours.iter().find(|s| s.slot == "h00").unwrap();
        assert!(h0.confidence < 0.5, "three sightings are not a fact: {}", h0.confidence);
        let _ = fs::remove_dir_all(&p);
    }

    #[test]
    fn withdrawal_reports_what_went_and_survives_a_refold() {
        let p = dir("substrate_dossier_withdraw");
        observation::record(&p, obs("phone:betty", "told the familiar", "hi", 100)).unwrap();
        observation::record(&p, obs("watch:betty", "reports", "presence", 200)).unwrap();
        fold(&p, HL).unwrap();
        let receipt = withdraw(&p, "betty", 300).unwrap();
        assert!(receipt.contributions_removed >= 2, "the receipt reports real rows");
        // The load-bearing half: rewind the cursor (a rebuild) — she must not come back.
        fs::write(p.join(CURSOR_FILE), "0").unwrap();
        fold(&p, HL).unwrap();
        let after: Vec<Contribution> = store::load_prefix(&p, DOSSIER_FILE, "ctb|betty|").unwrap();
        assert!(after.is_empty(), "the tombstone outranks the log: {after:?}");
        assert!(read(&p, "betty", 400, HL).unwrap().withdrawn);
        let _ = fs::remove_dir_all(&p);
    }

    #[test]
    fn a_coarse_summary_speaks_in_day_parts_not_distributions() {
        let p = dir("substrate_dossier_summary");
        let evening = 19 * 3600;
        for i in 0..6 {
            observation::record(
                &p,
                obs("phone:betty", "told the familiar", "hi", evening + i * 86_400),
            )
            .unwrap();
        }
        fold(&p, HL).unwrap();
        let d = read(&p, "betty", evening + 6 * 86_400, HL).unwrap();
        let s = coarse_summary(&d);
        assert!(s.contains("in the evening"), "{s}");
        assert!(s.contains("dialogue"), "{s}");
        assert!(!s.contains("h19"), "no raw slots in a spoken summary: {s}");
        let _ = fs::remove_dir_all(&p);
    }
}
