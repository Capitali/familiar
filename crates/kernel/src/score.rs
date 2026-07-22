//! Scoring and the adaptive promotion bar. Faithful port of v1's `score.c`.
//!
//! The promotion threshold is **self-regulating**: base 0.70, raised toward 0.95 as
//! the rigor drive rises — so when the factory promotes too easily it makes its own
//! bar harder. This is the selection-pressure invariant (do not change the constants
//! without an ADR; the threshold's *meaning* is constitutional).

use crate::candidate::Candidate;
use crate::selection::{self, Decision};
use crate::thread::Thread;
use crate::trial::{self, Trial};

/// The cost-weighted fitness selection reads. Cost is folded exactly once upstream
/// (into `trial.overall`); do not penalize cost again here.
pub fn overall(trial: &Trial) -> f64 {
    trial.overall
}

/// Adaptive promotion bar: `0.70 + 0.25 * rigor`, with `rigor` clamped to [0,1].
pub fn promote_threshold(rigor: f64) -> f64 {
    0.70 + 0.25 * rigor.clamp(0.0, 1.0)
}

/// The fixed mutation floor.
pub fn mutate_threshold() -> f64 {
    0.35
}

// --- Theory quality -----------------------------------------------------------------------------
//
// The factory doesn't only score candidate *artifacts*; it must also learn whether its own
// *theories* — the interpretations it forms and then acts on — tend to pay off. A theory (a pursued
// thread) becomes a candidate (`loop_id == thread.id`); that candidate is trialed and then decided.
// So a theory's outcome IS the selection decision its candidate earned. Reading those outcomes back
// gives the factory a track record as a theorist, and lets it score a *new* theory against the fate
// of the ones before it — so it stops re-pursuing what its own testing already threw away.

/// How a theory turned out, read from the candidate it spawned and that candidate's trial.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TheoryOutcome {
    /// Its candidate cleared the promotion bar — the theory paid off.
    Promoted,
    /// Mutated — partly right, worth another generation.
    Refined,
    /// Archived/rejected — kept as negative evidence; the theory did not pan out.
    Discarded,
    /// Acted on but not yet decided, or never acted on.
    Pending,
}

/// Resolve one theory's outcome: thread → its candidate (`loop_id == thread.id`) → that candidate's
/// most recent trial → the selection decision at the current rigor. Pure over the loaded slices.
pub fn theory_outcome(
    thread: &Thread,
    candidates: &[Candidate],
    trials: &[Trial],
    rigor: f64,
) -> TheoryOutcome {
    let Some(c) = candidates.iter().find(|c| c.loop_id == thread.id) else {
        return TheoryOutcome::Pending;
    };
    let Some(t) = trial::find_by_candidate(trials, &c.id) else {
        return TheoryOutcome::Pending;
    };
    match selection::decide(t, rigor) {
        Decision::Promote => TheoryOutcome::Promoted,
        Decision::Mutate => TheoryOutcome::Refined,
        Decision::Archive | Decision::Reject => TheoryOutcome::Discarded,
        Decision::Hold | Decision::ObserveMore => TheoryOutcome::Pending,
    }
}

/// The factory's track record as a theorist — the resolved outcomes of the theories it acted on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TheoryRecord {
    pub acted_on: usize,
    pub promoted: usize,
    pub refined: usize,
    pub discarded: usize,
    pub pending: usize,
    /// Smoothed share that paid off (promoted = 1.0, refined = 0.5), in [0,1]. Smoothed toward 0.5
    /// so a thin history stays humble — the factory doesn't get cocky (or despairing) off one result.
    pub quality: f64,
}

/// Laplace-style smoothing: this many phantom neutral (0.5) outcomes pull `quality` toward 0.5 until
/// real evidence accrues.
const THEORY_PRIOR_STRENGTH: f64 = 2.0;

/// Aggregate the outcomes of the theories the factory has acted on into its theorist track record.
/// Only threads it actually pursued (a real direction, no longer `open`) count.
pub fn theory_record(
    threads: &[Thread],
    candidates: &[Candidate],
    trials: &[Trial],
    rigor: f64,
) -> TheoryRecord {
    let (mut promoted, mut refined, mut discarded, mut pending) = (0, 0, 0, 0);
    for t in threads {
        if t.status == "open" || t.direction.trim().is_empty() {
            continue; // not yet acted on — no outcome to learn from
        }
        match theory_outcome(t, candidates, trials, rigor) {
            TheoryOutcome::Promoted => promoted += 1,
            TheoryOutcome::Refined => refined += 1,
            TheoryOutcome::Discarded => discarded += 1,
            TheoryOutcome::Pending => pending += 1,
        }
    }
    let decided = (promoted + refined + discarded) as f64;
    let paid = promoted as f64 + 0.5 * refined as f64;
    let quality = (paid + 0.5 * THEORY_PRIOR_STRENGTH) / (decided + THEORY_PRIOR_STRENGTH);
    TheoryRecord {
        acted_on: promoted + refined + discarded + pending,
        promoted,
        refined,
        discarded,
        pending,
        quality: quality.clamp(0.0, 1.0),
    }
}

/// Score a *new* theory (by its direction) against the outcomes of the ones before it. The base is
/// the factory's track record (a prior on whether a fresh theory pays off); a direction that merely
/// repeats one already **discarded** is heavily discounted — the factory should not spend selection
/// pressure re-testing what it already threw away. Returns a quality in [0,1] (~0.5 with no history).
pub fn score_theory(
    new_direction: &str,
    threads: &[Thread],
    candidates: &[Candidate],
    trials: &[Trial],
    rigor: f64,
) -> f64 {
    let record = theory_record(threads, candidates, trials, rigor);
    let mut score = record.quality;
    let nd = new_direction.trim().to_lowercase();
    if !nd.is_empty() {
        let repeats_discarded = threads.iter().any(|t| {
            t.direction.trim().to_lowercase() == nd
                && theory_outcome(t, candidates, trials, rigor) == TheoryOutcome::Discarded
        });
        if repeats_discarded {
            score *= 0.4;
        }
    }
    score.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promote_threshold_self_regulates() {
        assert!((promote_threshold(0.0) - 0.70).abs() < 1e-12);
        assert!((promote_threshold(1.0) - 0.95).abs() < 1e-12);
        assert!((promote_threshold(0.5) - 0.825).abs() < 1e-12);
        // clamped
        assert!((promote_threshold(-1.0) - 0.70).abs() < 1e-12);
        assert!((promote_threshold(2.0) - 0.95).abs() < 1e-12);
    }

    #[test]
    fn mutate_floor_is_fixed() {
        assert!((mutate_threshold() - 0.35).abs() < 1e-12);
    }

    // --- theory quality ---

    fn theory(id: &str, direction: &str, status: &str) -> Thread {
        Thread {
            id: id.into(),
            question: String::new(),
            theory: String::new(),
            direction: direction.into(),
            created_at: 0,
            status: status.into(),
            status_at: 0,
            last_worked_at: 0,
            origin: "llm".into(),
            actor: "familiar".into(),
        }
    }

    /// A candidate spawned by a theory (`loop_id == thread.id`), plus its trial.
    fn spawned(thread_id: &str, cand_id: &str, result: &str, overall: f64) -> (Candidate, Trial) {
        let mut c = Candidate::from_loop(
            &crate::loops::Loop {
                id: thread_id.into(),
                name: format!("thread:{thread_id}"),
                description: String::new(),
                loop_type: "thread".into(),
                observation_ids: String::new(),
                observation_count: 0,
                first_seen: 0,
                last_seen: 0,
                recurrence_score: 0.0,
                friction_score: 0.5,
                opportunity_score: 0.5,
                confidence: 0.5,
            },
            cand_id,
        );
        c.status = "generated".into();
        let mut t = Trial::new(format!("trial-{cand_id}"), cand_id);
        t.result = result.into();
        t.overall = overall;
        if result == "fail" {
            t.failure_class = "too_complex".into();
        }
        (c, t)
    }

    #[test]
    fn theory_outcome_reads_the_candidates_decision() {
        let th = theory("thread-1", "offer a morning digest", "pursued");
        let (c, tr) = spawned("thread-1", "candidate-1", "pass", 0.90);
        // pass at 0.90, low rigor → the candidate promotes → the theory paid off.
        assert_eq!(
            theory_outcome(&th, &[c.clone()], &[tr.clone()], 0.0),
            TheoryOutcome::Promoted
        );
        // A theory never acted on (no candidate) is pending.
        assert_eq!(theory_outcome(&th, &[], &[], 0.0), TheoryOutcome::Pending);
    }

    #[test]
    fn track_record_is_smoothed_toward_half() {
        // No history → quality is exactly the 0.5 prior.
        let empty = theory_record(&[], &[], &[], 0.0);
        assert_eq!(empty.acted_on, 0);
        assert!((empty.quality - 0.5).abs() < 1e-9);

        // One promoted theory lifts quality above 0.5 but not all the way to 1 (humble on thin data).
        let th = theory("t1", "d1", "pursued");
        let (c, tr) = spawned("t1", "c1", "pass", 0.9);
        let rec = theory_record(&[th], &[c], &[tr], 0.0);
        assert_eq!((rec.promoted, rec.discarded), (1, 0));
        assert!(rec.quality > 0.5 && rec.quality < 1.0);
    }

    #[test]
    fn a_new_theory_repeating_a_discarded_direction_scores_low() {
        // A prior theory with this exact direction was discarded (failed, below the floor).
        let past = theory("t1", "poll the battery every second", "pursued");
        let (c, tr) = spawned("t1", "c1", "fail", 0.10); // archived → discarded
        let threads = vec![past];
        let cands = vec![c];
        let trials = vec![tr];
        let repeat = score_theory(
            "poll the battery every second",
            &threads,
            &cands,
            &trials,
            0.0,
        );
        let fresh = score_theory(
            "offer a gentle morning digest",
            &threads,
            &cands,
            &trials,
            0.0,
        );
        assert!(
            repeat < fresh,
            "repeating a discarded direction must score below a fresh one"
        );
        assert!(repeat < 0.5);
    }
}
