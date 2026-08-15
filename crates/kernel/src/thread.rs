//! Thread — a question the factory poses and a theory it holds.
//!
//! The **Interpret** step of the cycle made durable: as the factory observes, it
//! forms questions (to ask the human) and theories (about what the patterns mean).
//! These are *not* observations — observations are the only truth, of the world;
//! a thread is the factory reasoning *about* that truth. A minimal port of v1's
//! richer `thread_t` (fitness/decay/lineage come later).

use crate::store;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

pub const THREADS_FILE: &str = "threads.jsonl";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Thread {
    pub id: String,
    /// A question for the human, grounded in what was observed.
    pub question: String,
    /// The factory's interpretation of what the patterns mean.
    pub theory: String,
    /// What the factory could *do* to act on this theory in service — becomes a
    /// candidate's hypothesis when the thread is pursued. (Optional.)
    #[serde(default)]
    pub direction: String,
    pub created_at: i64,
    /// open | pursued | answered | abandoned | marginalized | superseded
    pub status: String,
    /// When the thread entered its *current* status (unix secs) — whatever state a theory
    /// is in, it carries the date it got there. Backfilled to `created_at` for old rows.
    #[serde(default)]
    pub status_at: i64,
    /// Last time this thread was actively worked (pursued, evidence added, answered).
    #[serde(default)]
    pub last_worked_at: i64,
    /// How many times this theory has RE-OCCURRED — the muse arrived at the same idea again
    /// (C5). A one-off theory stays at 0 and processes quietly; only a theory that keeps
    /// resurfacing (or one that progressed to pursued/answered) has earned a human's attention.
    /// This is what lets connectivity noise churn in the background without cluttering the view.
    #[serde(default)]
    pub reinforced: u32,
    /// The human's answers to this thread's question — evidence the pursuit carries.
    /// Empty until someone answers; each answer stamps `last_worked_at`.
    #[serde(default)]
    pub answers: Vec<String>,
    /// llm | observer
    pub origin: String,
    /// The human this thread is *about* — set when the factory theorizes a need for a
    /// specific person (ADR-0022). Origin stays `"llm"` until that person themselves
    /// answers (see [`add_answer_from`]): a theorized need is a hypothesis about someone,
    /// and only their own words make it a stated one. Empty = not person-specific.
    #[serde(default)]
    pub origin_human: String,
    /// Who authored the directive — the actor whose reputation governs whether it is
    /// pursued (corruption awareness, Brick 20). `"familiar"` for its own theories;
    /// `"ian"` (or another human) for observer answers. Empty = unattributed (always
    /// pursued). `#[serde(default)]` so older threads still load.
    #[serde(default)]
    pub actor: String,
    /// Citations this theory stands on (T-126): observation ids ("obs-0042") or loop
    /// names, chosen from the eligible set the SYSTEM enumerated at consult time —
    /// never invented. Empty on legacy rows and prose-only paths (device theories,
    /// the needs muse) until those adopt the draft contract.
    #[serde(default)]
    pub anchors: Vec<String>,
    /// The system-facts registry revision this thread was validated against at mint
    /// (T-126). 0 = predates the floor. A changed fact supersedes a revision; it
    /// never silently reinterprets old threads.
    #[serde(default)]
    pub facts_rev: u32,
    /// Row schema version (T-127). 0 = legacy row written before versioning existed;
    /// rows minted through [`mint`] carry [`THREAD_VERSION`].
    #[serde(default)]
    pub v: u32,
    /// Typed identity, part 1 (T-127, dialogue Q1): what part of the world this is
    /// ABOUT — subject + sorted anchor classes + target. Same family, different
    /// variant = competing alternatives, never merged. Empty = unkeyed (prose-only
    /// mint paths and legacy rows); an empty key never matches anything.
    #[serde(default)]
    pub family_key: String,
    /// Typed identity, part 2: the actual claim — mechanism + declared acts +
    /// prediction shape. An exact match STRENGTHENS the standing thread instead of
    /// minting a sibling. Question prose is never part of identity.
    #[serde(default)]
    pub variant_key: String,
    /// Set when this thread was folded into a survivor (T-127's conservative
    /// migration): the tombstone stays, append-retained, pointing home.
    #[serde(default)]
    pub superseded_by: String,
}

/// Row schema version written by [`mint`] and folded rows (see `Thread::v`).
pub const THREAD_VERSION: u32 = 1;

pub fn append(dir: &Path, t: &Thread) -> io::Result<()> {
    store::append(dir, THREADS_FILE, t)
}

pub fn load(dir: &Path) -> io::Result<Vec<Thread>> {
    store::load(dir, THREADS_FILE)
}

/// Set a thread's status at `now` — a single indexed update, not a whole-file rewrite,
/// stamping `status_at` (and `last_worked_at` when the transition is active work: pursued
/// or answered). Returns true if the id was found.
pub fn update_status(dir: &Path, id: &str, status: &str, now: i64) -> io::Result<bool> {
    let Some(mut t) = store::load_by_id::<Thread>(dir, THREADS_FILE, id)? else {
        return Ok(false);
    };
    if t.status != status {
        t.status_at = now;
    }
    t.status = status.to_string();
    if matches!(status, "pursued" | "answered") {
        t.last_worked_at = now;
    }
    store::update_by_id(dir, THREADS_FILE, id, &t)
}

/// The muse arrived at this theory again — reinforce it rather than spawn a near-duplicate
/// (C5). Bumps the recurrence count and stamps it worked, so a theory that keeps resurfacing
/// climbs toward the maturity threshold while a one-off stays quiet. Returns true if found.
pub fn reinforce(dir: &Path, id: &str, now: i64) -> io::Result<bool> {
    let Some(mut t) = store::load_by_id::<Thread>(dir, THREADS_FILE, id)? else {
        return Ok(false);
    };
    t.reinforced = t.reinforced.saturating_add(1);
    t.last_worked_at = now;
    store::update_by_id(dir, THREADS_FILE, id, &t)
}

/// What a centralized mint needs to know (T-127). The typed identity travels in the
/// request; prose (question/theory/direction) is presentation and never identity.
pub struct Mint {
    pub question: String,
    pub theory: String,
    pub direction: String,
    pub origin: String,
    pub origin_human: String,
    pub actor: String,
    pub anchors: Vec<String>,
    pub facts_rev: u32,
    /// Family: who/what this is about. Empty everywhere = unkeyed (never matches).
    pub subject: String,
    pub anchor_classes: Vec<String>,
    pub target: String,
    /// Variant: the actual claim's typed shape.
    pub mechanism: String,
    pub acts: Vec<String>,
    pub predictions_sig: Vec<String>,
}

/// What the mint decided (dialogue Q1): a brand-new thought, a competing alternative
/// in a standing family, or a strengthening of the exact standing claim.
pub enum Disposition {
    New(Thread),
    Competes(Thread),
    Strengthened(String),
}

/// Part 1 of identity: subject + sorted anchor classes + target. Raw joined strings,
/// not hashes — identity must be auditable (codex, round 2).
pub fn family_key(subject: &str, anchor_classes: &[String], target: &str) -> String {
    if anchor_classes.is_empty() && target.trim().is_empty() {
        return String::new(); // unkeyed — prose-only paths never falsely collide
    }
    let mut classes: Vec<String> = anchor_classes.to_vec();
    classes.sort();
    classes.dedup();
    format!(
        "fam:v{THREAD_VERSION}|{}|{}|{}",
        subject.trim().to_lowercase(),
        classes.join(","),
        target.trim().to_lowercase()
    )
}

/// Part 2: the claim itself — mechanism + declared acts + prediction shape.
pub fn variant_key(
    family: &str,
    mechanism: &str,
    acts: &[String],
    predictions_sig: &[String],
) -> String {
    if family.is_empty() {
        return String::new();
    }
    let mut a: Vec<String> = acts.iter().map(|s| s.trim().to_lowercase()).collect();
    a.sort();
    a.dedup();
    let mut p: Vec<String> = predictions_sig.to_vec();
    p.sort();
    p.dedup();
    format!(
        "{family}||{}|{}|{}",
        mechanism.trim().to_lowercase(),
        a.join(","),
        p.join(",")
    )
}

/// THE one way a thread is born (T-127): every minter routes here, so identity is
/// enforced at one chokepoint and ids come from the store's sequence — closing the
/// four-minter race where independent `len()+1` counters could issue one id twice.
pub fn mint(dir: &Path, m: Mint, now: i64) -> io::Result<Disposition> {
    let fam = family_key(&m.subject, &m.anchor_classes, &m.target);
    let var = variant_key(&fam, &m.mechanism, &m.acts, &m.predictions_sig);
    let existing = load(dir)?;
    if !var.is_empty() {
        if let Some(t) = existing
            .iter()
            .find(|t| matches!(t.status.as_str(), "open" | "pursued") && t.variant_key == var)
        {
            strengthen_with_anchors(dir, &t.id, &m.anchors, now)?;
            return Ok(Disposition::Strengthened(t.id.clone()));
        }
    }
    let seq = store::next_seq(dir, THREADS_FILE)?;
    let t = Thread {
        id: format!("thread-{seq:04}"),
        question: m.question,
        theory: m.theory,
        direction: m.direction,
        created_at: now,
        status: "open".to_string(),
        status_at: now,
        last_worked_at: 0,
        reinforced: 0,
        answers: Vec::new(),
        origin: m.origin,
        origin_human: m.origin_human,
        actor: m.actor,
        anchors: m.anchors,
        facts_rev: m.facts_rev,
        v: THREAD_VERSION,
        family_key: fam.clone(),
        variant_key: var,
        superseded_by: String::new(),
    };
    append(dir, &t)?;
    let competes = !fam.is_empty()
        && existing
            .iter()
            .any(|e| matches!(e.status.as_str(), "open" | "pursued") && e.family_key == fam);
    Ok(if competes {
        Disposition::Competes(t)
    } else {
        Disposition::New(t)
    })
}

/// The exact standing claim arrived again: reinforce it and UNION the new citations —
/// the count derives from evidence, not an unexplained integer (codex, round 2).
pub fn strengthen_with_anchors(
    dir: &Path,
    id: &str,
    anchors: &[String],
    now: i64,
) -> io::Result<bool> {
    let Some(mut t) = store::load_by_id::<Thread>(dir, THREADS_FILE, id)? else {
        return Ok(false);
    };
    t.reinforced = t.reinforced.saturating_add(1);
    t.last_worked_at = now;
    for a in anchors {
        if !t.anchors.contains(a) {
            t.anchors.push(a.clone());
        }
    }
    store::update_by_id(dir, THREADS_FILE, id, &t)
}

/// The conservative fold (T-127's migration, dialogue Q1): members become
/// append-retained tombstones pointing home; the survivor unions every original
/// citation and answer. Only ever driven by an explicit, reviewed manifest — never
/// by a model or a fuzzy threshold.
pub fn fold(dir: &Path, survivor_id: &str, member_ids: &[String], now: i64) -> io::Result<usize> {
    let Some(mut survivor) = store::load_by_id::<Thread>(dir, THREADS_FILE, survivor_id)? else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("fold survivor {survivor_id} not found"),
        ));
    };
    let mut folded = 0usize;
    for mid in member_ids {
        if mid == survivor_id {
            continue;
        }
        let Some(mut m) = store::load_by_id::<Thread>(dir, THREADS_FILE, mid)? else {
            continue; // a manifest naming a missing row folds what exists, loudly countable
        };
        if m.status == "superseded" {
            continue; // idempotent — a re-run manifest re-folds nothing
        }
        for a in &m.anchors {
            if !survivor.anchors.contains(a) {
                survivor.anchors.push(a.clone());
            }
        }
        for ans in &m.answers {
            if !survivor.answers.contains(ans) {
                survivor.answers.push(ans.clone());
            }
        }
        m.status = "superseded".into();
        m.status_at = now;
        m.superseded_by = survivor_id.to_string();
        store::update_by_id(dir, THREADS_FILE, mid, &m)?;
        folded += 1;
    }
    survivor.reinforced = survivor.reinforced.saturating_add(folded as u32);
    survivor.last_worked_at = now;
    store::update_by_id(dir, THREADS_FILE, survivor_id, &survivor)?;
    Ok(folded)
}

/// How many recurrences (or a progression to pursued/answered) a theory needs before it is
/// worth a human's eyes (C5). Below this it churns in the background; connectivity noise never
/// clears it because each variant reinforces the same thread rather than adding a new one.
pub const MATURITY_THRESHOLD: u32 = 3;

/// Whether a theory has earned a place in the human-facing view (C5): it has recurred enough,
/// or it progressed into work / an answer. Abandoned and marginalized theories never surface.
pub fn is_mature(t: &Thread) -> bool {
    if matches!(
        t.status.as_str(),
        "abandoned" | "marginalized" | "superseded"
    ) {
        return false;
    }
    t.reinforced >= MATURITY_THRESHOLD
        || matches!(t.status.as_str(), "pursued" | "answered")
        || !t.answers.is_empty()
}

/// The human answered this thread's question. The answer is appended as evidence, the
/// thread is stamped as actively worked, and a discarded thread is REVIVED to open —
/// a human choosing to answer outranks the factory's earlier triage.
pub fn add_answer(dir: &Path, id: &str, text: &str, now: i64) -> io::Result<bool> {
    let Some(mut t) = store::load_by_id::<Thread>(dir, THREADS_FILE, id)? else {
        return Ok(false);
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(false);
    }
    t.answers.push(trimmed.to_string());
    t.last_worked_at = now;
    if matches!(t.status.as_str(), "abandoned" | "marginalized" | "answered") {
        t.status = "open".into();
        t.status_at = now;
    }
    store::update_by_id(dir, THREADS_FILE, id, &t)
}

/// [`add_answer`] with the answerer's identity. When the answerer is the human the thread is
/// about (`origin_human`), the thread's origin flips `llm → observer` and the actor
/// becomes that human: the theorized need is now a stated one, counted by
/// `unmet_needs` and carried in the human's own words. Deterministic on purpose — no
/// model judges whether an answer "counts"; any non-empty reply from the subject does
/// (consent by observation: their reaction is the signal, and even "no, that's wrong"
/// is the human stating what they need). An answer from anyone else attaches as
/// ordinary evidence and flips nothing. The local console speaks as whoever the node
/// currently serves (`identity::current`); other humans' confirms arrive via their own
/// signed devices (mesh::observe), which carry real `phone:<name>` actors.
pub fn add_answer_from(dir: &Path, id: &str, text: &str, by: &str, now: i64) -> io::Result<bool> {
    if !add_answer(dir, id, text, now)? {
        return Ok(false);
    }
    let Some(mut t) = store::load_by_id::<Thread>(dir, THREADS_FILE, id)? else {
        return Ok(false);
    };
    let answerer = crate::routing::human_of(by).trim().to_lowercase();
    if !t.origin_human.is_empty() && t.origin == "llm" && answerer == t.origin_human {
        t.origin = "observer".into();
        t.actor = t.origin_human.clone();
        store::update_by_id(dir, THREADS_FILE, id, &t)?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn round_trips() {
        let p = std::env::temp_dir().join("substrate_thread_test");
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        let t = Thread {
            id: "thread-0001".into(),
            question: "What would make mornings calmer?".into(),
            theory: "Repeated status requests suggest a standing digest would help.".into(),
            direction: "offer a standing morning digest".into(),
            created_at: 100,
            status: "open".into(),
            status_at: 100,
            last_worked_at: 0,
            reinforced: 0,
            answers: Vec::new(),
            origin: "llm".into(),
            origin_human: String::new(),
            actor: "familiar".into(),
            anchors: Vec::new(),
            facts_rev: 0,
            v: 0,
            family_key: String::new(),
            variant_key: String::new(),
            superseded_by: String::new(),
        };
        append(&p, &t).unwrap();
        assert_eq!(load(&p).unwrap(), vec![t.clone()]);
        update_status(&p, "thread-0001", "pursued", 200).unwrap();
        let updated = &load(&p).unwrap()[0];
        assert_eq!(updated.status, "pursued");
        assert_eq!(updated.status_at, 200, "a status change is dated");
        assert_eq!(updated.last_worked_at, 200, "pursuing is active work");
        add_answer(&p, "thread-0001", "mornings mean before 10am", 300).unwrap();
        let t2 = &load(&p).unwrap()[0];
        assert_eq!(t2.answers, vec!["mornings mean before 10am"]);
        assert_eq!(t2.last_worked_at, 300, "answering is active work");
        let _ = fs::remove_dir_all(&p);
    }

    fn need_thread(id: &str) -> Thread {
        Thread {
            id: id.into(),
            question: "Betty — would warmer evening light help?".into(),
            theory: "Betty may want softer light after dark.".into(),
            direction: "dim the lights after 20:00".into(),
            created_at: 100,
            status: "open".into(),
            status_at: 100,
            last_worked_at: 0,
            reinforced: 0,
            answers: Vec::new(),
            origin: "llm".into(),
            origin_human: "betty".into(),
            actor: "familiar".into(),
            anchors: Vec::new(),
            facts_rev: 0,
            v: 0,
            family_key: String::new(),
            variant_key: String::new(),
            superseded_by: String::new(),
        }
    }

    #[test]
    fn a_confirm_answer_from_the_subject_makes_the_need_stated() {
        let p = std::env::temp_dir().join("substrate_thread_confirm_test");
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        append(&p, &need_thread("thread-0001")).unwrap();
        // Her device speaks for her — the namespace parse finds the human.
        add_answer_from(
            &p,
            "thread-0001",
            "yes — after eight, please",
            "phone:betty",
            200,
        )
        .unwrap();
        let t = &load(&p).unwrap()[0];
        assert_eq!(t.origin, "observer", "her words make the need a stated one");
        assert_eq!(t.actor, "betty", "the need now belongs to its human");
        assert_eq!(t.answers, vec!["yes — after eight, please"]);
        let _ = fs::remove_dir_all(&p);
    }

    #[test]
    fn an_answer_from_someone_else_flips_nothing() {
        let p = std::env::temp_dir().join("substrate_thread_noflip_test");
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        append(&p, &need_thread("thread-0001")).unwrap();
        add_answer_from(&p, "thread-0001", "she does like it dim", "ian", 200).unwrap();
        let t = &load(&p).unwrap()[0];
        assert_eq!(
            t.origin, "llm",
            "another voice is evidence, not confirmation"
        );
        assert_eq!(t.actor, "familiar");
        assert_eq!(
            t.answers,
            vec!["she does like it dim"],
            "but the evidence still travels with the thread"
        );
        let _ = fs::remove_dir_all(&p);
    }

    // ---- T-127: one thought, one thread ----

    fn mint_req(theory: &str, mechanism: &str, acts: &[&str], anchor: &str) -> Mint {
        Mint {
            question: "q".into(),
            theory: theory.into(),
            direction: "d".into(),
            origin: "llm".into(),
            origin_human: String::new(),
            actor: "familiar".into(),
            anchors: vec![anchor.into()],
            facts_rev: 1,
            subject: "ian".into(),
            anchor_classes: vec!["ian|adjusted|lighting".into()],
            target: "lights".into(),
            mechanism: mechanism.into(),
            acts: acts.iter().map(|s| s.to_string()).collect(),
            predictions_sig: vec!["ian|adjusted|lighting:|absent|7200".into()],
        }
    }

    #[test]
    fn the_same_claim_strengthens_and_a_different_act_competes() {
        let p = std::env::temp_dir().join(format!("thread_identity_{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        let first = match mint(
            &p,
            mint_req("dim on away", "presence", &["dim"], "obs-0001"),
            100,
        )
        .unwrap()
        {
            Disposition::New(t) => t,
            _ => panic!("first mint is new"),
        };
        // The muse restates the same claim with different prose + a new citation.
        match mint(
            &p,
            mint_req(
                "lighting follows presence",
                "presence",
                &["dim"],
                "obs-0002",
            ),
            200,
        )
        .unwrap()
        {
            Disposition::Strengthened(id) => assert_eq!(id, first.id),
            _ => panic!("the same variant strengthens, never re-mints"),
        }
        let t = &load(&p).unwrap()[0];
        assert_eq!(t.reinforced, 1, "the count derives from the arrival");
        assert_eq!(
            t.anchors,
            vec!["obs-0001", "obs-0002"],
            "citations union — evidence explains the count"
        );
        // A different declared act is a COMPETING alternative in the same family.
        match mint(
            &p,
            mint_req("off on away", "presence", &["off"], "obs-0003"),
            300,
        )
        .unwrap()
        {
            Disposition::Competes(t2) => {
                assert_eq!(t2.family_key, first.family_key, "one family, two claims");
                assert_ne!(t2.variant_key, first.variant_key);
            }
            _ => panic!("a different act must compete, not strengthen or stand alone"),
        }
        assert_eq!(load(&p).unwrap().len(), 2, "two claims, not three");
        let _ = fs::remove_dir_all(&p);
    }

    #[test]
    fn unkeyed_mints_never_collide_and_ids_come_from_the_store() {
        let p = std::env::temp_dir().join(format!("thread_unkeyed_{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        let unkeyed = |theory: &str| Mint {
            question: String::new(),
            theory: theory.into(),
            direction: "same words".into(),
            origin: "device".into(),
            origin_human: String::new(),
            actor: "phone:ian".into(),
            anchors: Vec::new(),
            facts_rev: 1,
            subject: String::new(),
            anchor_classes: Vec::new(),
            target: String::new(),
            mechanism: String::new(),
            acts: Vec::new(),
            predictions_sig: Vec::new(),
        };
        assert!(matches!(
            mint(&p, unkeyed("a"), 100).unwrap(),
            Disposition::New(_)
        ));
        assert!(
            matches!(mint(&p, unkeyed("b"), 200).unwrap(), Disposition::New(_)),
            "an empty key never matches — prose paths keep their own dedup"
        );
        let ts = load(&p).unwrap();
        assert_eq!(ts.len(), 2);
        assert_ne!(
            ts[0].id, ts[1].id,
            "the store's sequence issues distinct ids"
        );
        let _ = fs::remove_dir_all(&p);
    }

    #[test]
    fn a_fold_leaves_tombstones_that_point_home_and_is_idempotent() {
        let p = std::env::temp_dir().join(format!("thread_fold_{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        for (id, anchor) in [
            ("thread-0001", "obs-1"),
            ("thread-0002", "obs-2"),
            ("thread-0003", "obs-3"),
        ] {
            let mut t = need_thread(id);
            t.anchors = vec![anchor.into()];
            append(&p, &t).unwrap();
        }
        let n = fold(
            &p,
            "thread-0001",
            &["thread-0002".into(), "thread-0003".into()],
            500,
        )
        .unwrap();
        assert_eq!(n, 2);
        let ts = load(&p).unwrap();
        let survivor = ts.iter().find(|t| t.id == "thread-0001").unwrap();
        assert_eq!(
            survivor.anchors,
            vec!["obs-1", "obs-2", "obs-3"],
            "the survivor unions every citation"
        );
        assert_eq!(survivor.reinforced, 2);
        for mid in ["thread-0002", "thread-0003"] {
            let m = ts.iter().find(|t| t.id == mid).unwrap();
            assert_eq!(m.status, "superseded", "append-retained, never deleted");
            assert_eq!(m.superseded_by, "thread-0001", "the tombstone points home");
            assert!(!is_mature(m), "a tombstone never surfaces");
        }
        assert_eq!(
            fold(
                &p,
                "thread-0001",
                &["thread-0002".into(), "thread-0003".into()],
                600
            )
            .unwrap(),
            0,
            "a re-run manifest re-folds nothing"
        );
        let _ = fs::remove_dir_all(&p);
    }
}
