//! Predictions — a theory finally says what the world will do (design dialogue
//! 2026-08-14, Q1/Q3/Q6, all DECIDED; reasoning brief B1).
//!
//! A prediction is anchored ("when I see this…"), typed ("…I expect this, within that
//! window…"), and mechanically settled — no model in the truth loop. Each anchor match
//! opens ONE pending instance with an explicit opening observation and deadline; the
//! outcome is a `PredictionResult`, retained append-only as the theory's calibration
//! evidence (the L4 fix made structural: a theory's score DERIVES from its results, it
//! never overwrites them).
//!
//! Clocks (Q6): EVENT time (`o.ts`) decides whether an arrival satisfies a window;
//! settlement waits a GRACE after the deadline so late-delivered evidence can amend a
//! provisional miss — but a written result is final and never rewritten. A confirmation
//! (or an absent-violation) is certain the moment its event arrives and finalizes
//! immediately; only the quiet outcomes wait out the grace.

use crate::obs_class::ObsMatch;
use crate::observation::Observation;
use crate::store;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

/// Standing predictions + their open instances (small, rewritten as a set).
pub const PREDICTIONS_FILE: &str = "predictions.json";
/// Settled results — append-only, the calibration record.
pub const PREDICTION_RESULTS_FILE: &str = "prediction_results.jsonl";

/// Prediction format version — rides every persisted prediction (Q1: never silently
/// change what an old prediction means).
pub const PREDICTION_VERSION: u32 = 1;

/// What opens a pending instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Anchor {
    /// An observation matching this opens the window.
    Observed { when: ObsMatch },
    /// The theory's pursuit opened it (one instance, at pursue time).
    TheoryOpened,
}

/// Expect the consequent to arrive — or to stay away.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Polarity {
    Arrives,
    Absent,
}

/// One standing claim a theory makes about the world.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prediction {
    pub id: String,
    /// The theory (thread) whose claim this is — results calibrate it.
    pub thread_id: String,
    pub v: u32,
    pub anchor: Anchor,
    /// The consequent shape (event time judged against the window below).
    pub then: ObsMatch,
    /// The window opens `min_delay_secs` after the anchor and closes `within_secs`
    /// after it. min_delay 0 = immediately.
    pub min_delay_secs: i64,
    pub within_secs: i64,
    pub polarity: Polarity,
    /// A chatty anchor must not open overlapping copies of the same claim.
    pub cooldown_secs: i64,
    /// Settlement grace for THIS prediction; 0 = inherit the co-owned default
    /// (`Parameters::prediction_grace_secs`). A prediction that knows its source's lag
    /// declares it (Q6, decided).
    #[serde(default)]
    pub grace_secs: i64,
    pub minted_at: i64,
    /// Where the claim came from ("cli", "thread:<id>", later "llm:<consult>").
    #[serde(default)]
    pub minted_from: String,
    pub enabled: bool,
}

/// An open instance: one anchor firing, awaiting its outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingPrediction {
    pub prediction_id: String,
    pub thread_id: String,
    /// The observation that opened the window ("" for TheoryOpened).
    pub opened_by: String,
    pub opened_at: i64,
    /// Window bounds in EVENT time.
    pub not_before: i64,
    pub deadline: i64,
    /// A satisfying (or violating, for Absent) event seen so far — provisional until
    /// the instance finalizes; late in-window events may set it after the deadline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_by: Option<String>,
    #[serde(default)]
    pub matched_at: i64,
}

/// How an instance ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Arrives: the consequent came inside the window.
    Confirmed,
    /// Arrives: the window (plus grace) passed with nothing.
    Missed,
    /// Absent: the window passed clean — the quiet WAS the claim.
    AbsentConfirmed,
    /// Absent: the supposedly-absent thing arrived.
    AbsentViolated,
}

impl Outcome {
    /// Did the world agree with the theory?
    pub fn favorable(self) -> bool {
        matches!(self, Outcome::Confirmed | Outcome::AbsentConfirmed)
    }
}

/// A settled instance — append-only, never rewritten (Q6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredictionResult {
    pub prediction_id: String,
    pub thread_id: String,
    pub opened_by: String,
    pub opened_at: i64,
    pub deadline: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settled_by: Option<String>,
    pub outcome: Outcome,
    /// When the result became final (write time).
    pub final_at: i64,
}

/// The rewritten half of the store: standing predictions + open instances.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PredictionsFile {
    #[serde(default)]
    pub predictions: Vec<Prediction>,
    #[serde(default)]
    pub pending: Vec<PendingPrediction>,
    /// Last anchor firing per prediction id — the cooldown memory.
    #[serde(default)]
    pub last_opened: std::collections::BTreeMap<String, i64>,
    /// Event-time high-water mark of scored observations (the scoring cursor).
    #[serde(default)]
    pub scored_through: i64,
}

pub fn load(dir: &Path) -> PredictionsFile {
    store::load_one(dir, PREDICTIONS_FILE)
        .ok()
        .flatten()
        .unwrap_or_default()
}

pub fn save(dir: &Path, f: &PredictionsFile) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let s = serde_json::to_string_pretty(f)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    std::fs::write(dir.join(PREDICTIONS_FILE), s)
}

/// The calibration record, oldest first.
pub fn results(dir: &Path) -> io::Result<Vec<PredictionResult>> {
    store::load(dir, PREDICTION_RESULTS_FILE)
}

/// Mint a standing prediction. The consent/authorship story rides `minted_from`;
/// this function only guards shape (a window must exist; an Absent claim without a
/// bounded window would be unfalsifiable and is refused).
#[allow(clippy::too_many_arguments)]
pub fn mint(
    dir: &Path,
    thread_id: &str,
    anchor: Anchor,
    then: ObsMatch,
    min_delay_secs: i64,
    within_secs: i64,
    polarity: Polarity,
    cooldown_secs: i64,
    grace_secs: i64,
    minted_from: &str,
    now: i64,
) -> io::Result<Prediction> {
    if thread_id.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a prediction belongs to a theory",
        ));
    }
    if within_secs <= 0 || within_secs <= min_delay_secs {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a prediction needs a real window (within > min_delay ≥ 0)",
        ));
    }
    let mut f = load(dir);
    let p = Prediction {
        id: format!(
            "pred-{:08x}",
            (now as u64)
                .wrapping_mul(0x9e37_79b9)
                .wrapping_add(f.predictions.len() as u64)
                & 0xffff_ffff
        ),
        thread_id: thread_id.to_string(),
        v: PREDICTION_VERSION,
        anchor,
        then,
        min_delay_secs,
        within_secs,
        polarity,
        cooldown_secs,
        grace_secs,
        minted_at: now,
        minted_from: minted_from.to_string(),
        enabled: true,
    };
    // TheoryOpened anchors fire exactly once, at mint (the pursuit IS the opening).
    if p.anchor == Anchor::TheoryOpened {
        f.pending.push(PendingPrediction {
            prediction_id: p.id.clone(),
            thread_id: p.thread_id.clone(),
            opened_by: String::new(),
            opened_at: now,
            not_before: now + p.min_delay_secs,
            deadline: now + p.within_secs,
            matched_by: None,
            matched_at: 0,
        });
        f.last_opened.insert(p.id.clone(), now);
    }
    f.predictions.push(p.clone());
    save(dir, &f)?;
    Ok(p)
}

/// The per-theory calibration summary a score can derive from (never the reverse).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Calibration {
    pub confirmed: usize,
    pub missed: usize,
    pub absent_confirmed: usize,
    pub absent_violated: usize,
}

impl Calibration {
    pub fn favorable(&self) -> usize {
        self.confirmed + self.absent_confirmed
    }
    pub fn unfavorable(&self) -> usize {
        self.missed + self.absent_violated
    }
}

pub fn calibration(dir: &Path, thread_id: &str) -> Calibration {
    let mut c = Calibration::default();
    for r in results(dir).unwrap_or_default() {
        if r.thread_id != thread_id {
            continue;
        }
        match r.outcome {
            Outcome::Confirmed => c.confirmed += 1,
            Outcome::Missed => c.missed += 1,
            Outcome::AbsentConfirmed => c.absent_confirmed += 1,
            Outcome::AbsentViolated => c.absent_violated += 1,
        }
    }
    c
}

/// One scoring pass (called each tick): open new instances on anchor matches, match
/// consequents by EVENT time, finalize what can no longer change. `grace_default` is
/// the co-owned parameter; a prediction's own `grace_secs` overrides. Returns the
/// results written this pass (already appended) — the caller narrates transitions,
/// never this function (D1 is T-114's).
pub fn score(
    dir: &Path,
    obs: &[Observation],
    now: i64,
    grace_default: i64,
) -> io::Result<Vec<PredictionResult>> {
    let mut f = load(dir);
    if f.predictions.is_empty() {
        return Ok(Vec::new());
    }
    // New evidence only — event-time cursor with grace overlap (a late event must
    // still be seen while any window it could satisfy remains amendable).
    let overlap = f
        .predictions
        .iter()
        .map(|p| {
            if p.grace_secs > 0 {
                p.grace_secs
            } else {
                grace_default
            }
        })
        .max()
        .unwrap_or(grace_default);
    let fresh: Vec<&Observation> = obs
        .iter()
        .filter(|o| o.ts > f.scored_through - overlap && o.ts <= now)
        .collect();

    // 1. Anchors open instances (cooldown-guarded), on fresh evidence.
    let mut opened: Vec<PendingPrediction> = Vec::new();
    for p in f.predictions.iter().filter(|p| p.enabled) {
        let Anchor::Observed { when } = &p.anchor else {
            continue;
        };
        for o in &fresh {
            if !when.matches(o) {
                continue;
            }
            // Never-opened saturates to "infinitely long ago" — a plain subtraction
            // against i64::MIN overflows in debug.
            let last = f.last_opened.get(&p.id).copied().unwrap_or(i64::MIN);
            if o.ts.saturating_sub(last) < p.cooldown_secs {
                continue;
            }
            // One live instance per prediction per opening — an anchor refiring inside
            // an open window is the cooldown's business, not a second window's.
            if f.pending
                .iter()
                .chain(opened.iter())
                .any(|pd| pd.prediction_id == p.id && pd.opened_by == o.id)
            {
                continue;
            }
            f.last_opened.insert(p.id.clone(), o.ts);
            opened.push(PendingPrediction {
                prediction_id: p.id.clone(),
                thread_id: p.thread_id.clone(),
                opened_by: o.id.clone(),
                opened_at: o.ts,
                not_before: o.ts + p.min_delay_secs,
                deadline: o.ts + p.within_secs,
                matched_by: None,
                matched_at: 0,
            });
        }
    }
    f.pending.append(&mut opened);

    // 2. Consequents match open instances — event time inside [not_before, deadline].
    for pd in f.pending.iter_mut() {
        if pd.matched_by.is_some() {
            continue;
        }
        let Some(p) = f.predictions.iter().find(|p| p.id == pd.prediction_id) else {
            continue;
        };
        for o in &fresh {
            if o.id == pd.opened_by {
                continue; // the opener never satisfies its own claim
            }
            if o.ts >= pd.not_before && o.ts <= pd.deadline && p.then.matches(o) {
                pd.matched_by = Some(o.id.clone());
                pd.matched_at = o.ts;
                break;
            }
        }
    }

    // 3. Finalize: certainty finalizes immediately; quiet waits out the grace.
    let mut written = Vec::new();
    let mut still_pending = Vec::new();
    for pd in f.pending.drain(..) {
        let Some(p) = f.predictions.iter().find(|p| p.id == pd.prediction_id) else {
            continue; // its prediction was removed — the instance dies with it
        };
        let grace = if p.grace_secs > 0 {
            p.grace_secs
        } else {
            grace_default
        };
        let outcome = match (p.polarity, &pd.matched_by) {
            // A satisfied Arrives is certain now.
            (Polarity::Arrives, Some(_)) => Some(Outcome::Confirmed),
            // A violated Absent is certain now.
            (Polarity::Absent, Some(_)) => Some(Outcome::AbsentViolated),
            // Quiet: only final once late evidence can no longer amend it.
            (Polarity::Arrives, None) if now > pd.deadline + grace => Some(Outcome::Missed),
            (Polarity::Absent, None) if now > pd.deadline + grace => Some(Outcome::AbsentConfirmed),
            _ => None,
        };
        match outcome {
            Some(outcome) => {
                let r = PredictionResult {
                    prediction_id: pd.prediction_id,
                    thread_id: pd.thread_id,
                    opened_by: pd.opened_by,
                    opened_at: pd.opened_at,
                    deadline: pd.deadline,
                    settled_by: pd.matched_by,
                    outcome,
                    final_at: now,
                };
                store::append(dir, PREDICTION_RESULTS_FILE, &r)?;
                written.push(r);
            }
            None => still_pending.push(pd),
        }
    }
    f.pending = still_pending;
    f.scored_through = f
        .scored_through
        .max(fresh.iter().map(|o| o.ts).max().unwrap_or(f.scored_through));
    save(dir, &f)?;
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::obs_class::{FieldMatch, ObsMatch, MATCH_VERSION};

    fn dir(tag: &str) -> std::path::PathBuf {
        // Unique per process AND tag — the T-118 lesson: fixed names collide across
        // concurrent worktrees.
        let p = std::env::temp_dir().join(format!("familiar_pred_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn o(id: &str, actor: &str, action: &str, object: &str, ts: i64) -> Observation {
        let mut o = Observation::new(actor, action, object, "", "test", ts, 1.0);
        o.id = id.to_string();
        o
    }

    fn m(actor: &str, action: &str, obj_prefix: &str) -> ObsMatch {
        ObsMatch {
            v: MATCH_VERSION,
            actor: FieldMatch::Exact(actor.into()),
            action: FieldMatch::Exact(action.into()),
            object: FieldMatch::Prefix(obj_prefix.into()),
        }
    }

    const NOW: i64 = 2_000_000;

    #[test]
    fn an_anchored_claim_confirms_on_arrival_and_misses_after_grace() {
        let d = dir("confirm_miss");
        // "When ian goes away, the lights dim within 5 minutes."
        mint(
            &d,
            "th-light",
            Anchor::Observed {
                when: m("ian", "went", "away"),
            },
            m("wildhorse", "reported", "lighting:"),
            0,
            300,
            Polarity::Arrives,
            600,
            60,
            "test",
            NOW - 10,
        )
        .unwrap();

        // Anchor fires; consequent arrives inside the window → Confirmed immediately.
        let obs = vec![
            o("a1", "ian", "went", "away", NOW),
            o("c1", "wildhorse", "reported", "lighting:dim", NOW + 120),
        ];
        let written = score(&d, &obs, NOW + 130, 60).unwrap();
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].outcome, Outcome::Confirmed);
        assert_eq!(written[0].settled_by.as_deref(), Some("c1"));

        // Cooldown blocks an immediate re-open; a later anchor opens a fresh window
        // that stays QUIET — pending through deadline+grace, then Missed.
        let obs2 = vec![o("a2", "ian", "went", "away", NOW + 700)];
        assert!(score(&d, &obs2, NOW + 710, 60).unwrap().is_empty());
        assert!(
            score(&d, &obs2, NOW + 700 + 300 + 30, 60)
                .unwrap()
                .is_empty(),
            "inside grace: still amendable, not yet a miss"
        );
        let final_pass = score(&d, &obs2, NOW + 700 + 300 + 61, 60).unwrap();
        assert_eq!(final_pass.len(), 1);
        assert_eq!(final_pass[0].outcome, Outcome::Missed);

        let c = calibration(&d, "th-light");
        assert_eq!((c.confirmed, c.missed), (1, 1));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn late_evidence_amends_a_provisional_miss_but_never_a_final() {
        let d = dir("late");
        mint(
            &d,
            "th",
            Anchor::Observed {
                when: m("ian", "went", "away"),
            },
            m("sensor", "reported", "door:"),
            0,
            100,
            Polarity::Arrives,
            0,
            120,
            "test",
            NOW - 10,
        )
        .unwrap();
        let anchor = vec![o("a1", "ian", "went", "away", NOW)];
        assert!(score(&d, &anchor, NOW + 1, 120).unwrap().is_empty());
        // Deadline passes… but grace holds it open; a LATE-DELIVERED event whose EVENT
        // time sits inside the window amends the pending to a confirmation.
        let late = vec![o("l1", "sensor", "reported", "door:closed", NOW + 80)];
        let written = score(&d, &late, NOW + 150, 120).unwrap();
        assert_eq!(written.len(), 1);
        assert_eq!(
            written[0].outcome,
            Outcome::Confirmed,
            "late but in-window amends"
        );
        // And a written result is immutable: nothing about it changes on later passes.
        let again = score(&d, &late, NOW + 400, 120).unwrap();
        assert!(again.is_empty());
        assert_eq!(results(&d).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn absent_claims_are_bounded_and_settle_both_ways() {
        let d = dir("absent");
        // "After the door locks, NOTHING moves inside for 10 minutes."
        mint(
            &d,
            "th-quiet",
            Anchor::Observed {
                when: m("sensor", "reported", "door:locked"),
            },
            m("sensor", "reported", "motion:"),
            0,
            600,
            Polarity::Absent,
            0,
            30,
            "test",
            NOW - 10,
        )
        .unwrap();
        // Window 1: motion arrives → AbsentViolated, certain immediately.
        let obs = vec![
            o("k1", "sensor", "reported", "door:locked", NOW),
            o("mv", "sensor", "reported", "motion:hall", NOW + 200),
        ];
        let w = score(&d, &obs, NOW + 210, 30).unwrap();
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].outcome, Outcome::AbsentViolated);
        // Window 2: quiet through deadline+grace → AbsentConfirmed.
        let obs2 = vec![o("k2", "sensor", "reported", "door:locked", NOW + 1000)];
        assert!(score(&d, &obs2, NOW + 1010, 30).unwrap().is_empty());
        let w2 = score(&d, &obs2, NOW + 1000 + 600 + 31, 30).unwrap();
        assert_eq!(w2.len(), 1);
        assert_eq!(w2[0].outcome, Outcome::AbsentConfirmed);
        // An unfalsifiable Absent (no window) is refused at mint.
        assert!(mint(
            &d,
            "th-quiet",
            Anchor::TheoryOpened,
            m("x", "y", "z"),
            0,
            0,
            Polarity::Absent,
            0,
            0,
            "test",
            NOW
        )
        .is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
