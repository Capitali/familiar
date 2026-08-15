//! Who bears the consequence of an act (T-153).
//!
//! Decided across rounds 7–9 of the [whole-system review dialogue] with Ian's motorlights
//! case as the worked example: the RV's light is shared by Ian, Betty, and the dogs Clover
//! and Iris. **Affected is a RELATION, not a standing** (codex, Round 8): `person`, mesh
//! `member`, and `peer` answer different questions already, and "who bears the consequence
//! of this act?" is a property of an act in context — never a rank a subject possesses.
//! Making it a standing would re-collapse the four meanings the dialogue had just separated.
//!
//! The runtime records impact. It does **not** award or revoke moral worth: that floor is
//! [`HUMANITY.md`](../../../docs/HUMANITY.md)'s, it covers *beings* capable of suffering,
//! memory, relationship, meaning and choice — explicitly not only biological species — and
//! it may never be narrowed. Clover and Iris are subjects who live with the light's
//! effects; the light is a condition. Both matter, and they matter differently.

use serde::{Deserialize, Serialize};

/// Who or what bears a consequence. Deliberately NOT a hierarchy — the variants say what
/// kind of thing is exposed, so the act model can carry an honest reference even when
/// identity is unknown or must not be retained.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Subject {
    /// A person the familiar can name (their own preference is authoritative for them).
    Person { handle: String },
    /// A being that lives with the effect and cannot use the console — a dog in the room.
    /// Named only as loosely as the household names them; never profiled.
    Resident { name: String },
    /// Someone present whose identity is unknown or deliberately not retained. Exists so
    /// "we do not know who else is here" is representable instead of silently absent.
    UnknownResident,
    /// A protected condition rather than a being: the plant in the window, the fridge's
    /// cold, the cabin's temperature. Has interests to steward, no dissent to weigh.
    Condition { name: String },
}

/// How the familiar learned about this subject's exposure or preference. The channel
/// travels with the claim because a statement and an inference are not the same evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    /// They said so.
    DirectStatement,
    /// They acted on the surface themselves — the strongest non-verbal evidence.
    DeliberateAdjustment,
    /// Observed behaviour (a dog leaving the room when the light changes).
    ObservedBehavior,
    /// A steward or guardian reported it on their behalf.
    StewardReport,
    /// The familiar worked it out. Weakest, and never sufficient alone.
    Inference,
}

/// Which way an observation points. Kept separate from confidence: a strong signal of
/// discomfort and a weak one differ in confidence, not in direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Response {
    /// Nothing observed. **Missing — never support** (invariant 1).
    Unknown,
    Favorable,
    Adverse,
}

/// One subject's exposure to one act.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AffectedSubjectRef {
    pub subject: Subject,
    /// The declared surface whose effect reaches them.
    pub surface: String,
    /// What the act is expected to do to them, in the household's own words.
    #[serde(default)]
    pub exposure: String,
    pub channel: Channel,
    pub response: Response,
    /// [0,1]. Confidence in the READING, never in the subject's worth.
    #[serde(default)]
    pub confidence: f64,
    /// Honest missingness: what the familiar knows it does not know here.
    #[serde(default)]
    pub missing: String,
}

/// Authority offered for an act — carried BESIDE the affected set, never inside it, so a
/// permission can never be mistaken for evidence that nobody else is exposed (invariant 4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthorityRef {
    /// Who granted it.
    pub actor: String,
    /// What it covers — a surface, a scope, a single act.
    pub scope: String,
    /// The subject it may speak for, when it speaks for someone (a steward's care
    /// authority). Empty means it speaks only for the grantor.
    #[serde(default)]
    pub speaks_for: String,
}

/// What a discretionary act may do, once the affected set is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// Nothing adverse and someone authorized it: the act may proceed as scoped.
    Proceed,
    /// A credible adverse response: stop, narrow, or revert. Grants no power to anyone.
    NarrowOrRevert,
    /// Exposure is unknown where it matters: take the smaller experiment, or freeze.
    SmallerExperiment,
}

/// **Invariant 1 — unknown, absent, silent, or unable to answer is MISSING, never support.**
/// A familiar that counts silence as agreement is how a household quietly becomes a
/// majority; a subject who *cannot* answer can never be counted as having agreed.
pub fn supporting(refs: &[AffectedSubjectRef]) -> usize {
    refs.iter()
        .filter(|r| r.response == Response::Favorable)
        .filter(|r| {
            // Only a subject who can actually speak for themselves can support: an
            // inference or an observed behaviour is evidence about effect, not assent.
            matches!(
                r.channel,
                Channel::DirectStatement | Channel::DeliberateAdjustment
            ) && matches!(r.subject, Subject::Person { .. })
        })
        .count()
}

/// **Invariant 2 — a credible adverse response may stop, narrow, or revert a discretionary
/// act; it may never widen capability or authorize a lasting rule.** This is D1/D6's
/// asymmetry applied to impact: uncertain positive evidence never authorizes, credible
/// negative impact is always enough to take the smaller path.
pub fn adverse(refs: &[AffectedSubjectRef], floor: f64) -> bool {
    refs.iter()
        .any(|r| r.response == Response::Adverse && r.confidence >= floor)
}

/// **Invariant 5 — uncertainty chooses the smaller experiment.** Any subject whose exposure
/// is unknown, or any acknowledged missingness, means the act does not get to proceed at
/// full scope on the strength of someone else's yes.
pub fn unknown_exposure(refs: &[AffectedSubjectRef]) -> bool {
    refs.iter()
        .any(|r| r.response == Response::Unknown || !r.missing.trim().is_empty())
        || refs
            .iter()
            .any(|r| matches!(r.subject, Subject::UnknownResident))
}

/// Read the affected set for a DISCRETIONARY act. Deterministic, no scoring, no averaging:
/// the fields are never flattened into one number (the affected set is not an electorate).
/// Adverse dominates; then unknown; only then may an authorized act proceed.
pub fn disposition(
    refs: &[AffectedSubjectRef],
    authority: Option<&AuthorityRef>,
    floor: f64,
) -> Disposition {
    if adverse(refs, floor) {
        return Disposition::NarrowOrRevert;
    }
    if unknown_exposure(refs) {
        return Disposition::SmallerExperiment;
    }
    // **Invariant 4 — one subject's authorization covers only the authority they hold.**
    // A grant does not wash out another subject's exposure; with no authority at all, a
    // discretionary act still takes the smaller path rather than proceeding on silence.
    match authority {
        Some(_) => Disposition::Proceed,
        None => Disposition::SmallerExperiment,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(handle: &str, ch: Channel, resp: Response) -> AffectedSubjectRef {
        AffectedSubjectRef {
            subject: Subject::Person {
                handle: handle.into(),
            },
            surface: "lights".into(),
            exposure: "the light they live in changes".into(),
            channel: ch,
            response: resp,
            confidence: 0.9,
            missing: String::new(),
        }
    }
    fn dog(name: &str, resp: Response, confidence: f64) -> AffectedSubjectRef {
        AffectedSubjectRef {
            subject: Subject::Resident { name: name.into() },
            surface: "lights".into(),
            exposure: "the light they live in changes".into(),
            channel: Channel::ObservedBehavior,
            response: resp,
            confidence,
            missing: String::new(),
        }
    }
    fn ians_yes() -> AuthorityRef {
        AuthorityRef {
            actor: "ian".into(),
            scope: "lights".into(),
            speaks_for: String::new(),
        }
    }

    /// The motorlights household, exactly as Ian described it: one person assents, another
    /// says nothing, and two residents cannot use a console at all. One yes does not carry
    /// the household — the act takes the smaller experiment (invariants 1, 4, 5).
    #[test]
    fn one_yes_does_not_carry_a_household() {
        let set = vec![
            person("ian", Channel::DirectStatement, Response::Favorable),
            person("betty", Channel::Inference, Response::Unknown),
            dog("clover", Response::Unknown, 0.0),
            dog("iris", Response::Unknown, 0.0),
        ];
        assert_eq!(
            supporting(&set),
            1,
            "only the person who actually spoke supports"
        );
        assert_eq!(
            disposition(&set, Some(&ians_yes()), 0.5),
            Disposition::SmallerExperiment,
            "silence from those who cannot speak is missing, never support"
        );
    }

    /// A credible adverse reaction from a resident who cannot speak still narrows a
    /// discretionary act — evidence that grants no power but is heard (invariant 2).
    #[test]
    fn a_being_that_cannot_speak_can_still_refuse() {
        let set = vec![
            person("ian", Channel::DirectStatement, Response::Favorable),
            person("betty", Channel::DirectStatement, Response::Favorable),
            dog("iris", Response::Adverse, 0.8),
        ];
        assert_eq!(
            disposition(&set, Some(&ians_yes()), 0.5),
            Disposition::NarrowOrRevert,
            "adverse impact dominates a household's agreement"
        );
        assert_eq!(
            supporting(&set),
            2,
            "and it never becomes a vote to be outnumbered"
        );
    }

    /// Observed behaviour and inference are evidence about EFFECT, never assent — only a
    /// person's own statement or their own hand on the surface can support (invariant 1).
    #[test]
    fn only_a_persons_own_word_or_hand_supports() {
        let watched = vec![person(
            "ian",
            Channel::ObservedBehavior,
            Response::Favorable,
        )];
        assert_eq!(
            supporting(&watched),
            0,
            "being seen to like it is not saying yes"
        );
        let adjusted = vec![person(
            "ian",
            Channel::DeliberateAdjustment,
            Response::Favorable,
        )];
        assert_eq!(
            supporting(&adjusted),
            1,
            "their own hand on the surface speaks"
        );
        let guessed = vec![person("ian", Channel::Inference, Response::Favorable)];
        assert_eq!(supporting(&guessed), 0, "an inference is never assent");
    }

    /// A condition is stewarded, not consulted: the plant's poor state is an adverse
    /// reading that narrows the act, and it never counts as support or dissent-by-vote.
    #[test]
    fn a_condition_is_stewarded_not_consulted() {
        let set = vec![
            person("ian", Channel::DirectStatement, Response::Favorable),
            AffectedSubjectRef {
                subject: Subject::Condition {
                    name: "window plant".into(),
                },
                surface: "shade".into(),
                exposure: "morning light it depends on".into(),
                channel: Channel::ObservedBehavior,
                response: Response::Adverse,
                confidence: 0.7,
                missing: String::new(),
            },
        ];
        assert_eq!(supporting(&set), 1, "a condition never supports");
        assert_eq!(
            disposition(&set, Some(&ians_yes()), 0.5),
            Disposition::NarrowOrRevert
        );
    }

    /// An unknown resident makes the act take the smaller path even when everyone the
    /// familiar CAN see is content — "we do not know who else is here" is representable.
    #[test]
    fn an_unknown_resident_shrinks_the_experiment() {
        let mut set = vec![person("ian", Channel::DirectStatement, Response::Favorable)];
        assert_eq!(
            disposition(&set, Some(&ians_yes()), 0.5),
            Disposition::Proceed
        );
        set.push(AffectedSubjectRef {
            subject: Subject::UnknownResident,
            surface: "lights".into(),
            exposure: "unknown".into(),
            channel: Channel::Inference,
            response: Response::Unknown,
            confidence: 0.0,
            missing: "who else is in the cabin".into(),
        });
        assert_eq!(
            disposition(&set, Some(&ians_yes()), 0.5),
            Disposition::SmallerExperiment
        );
    }

    /// Authority is carried beside the affected set, never inside it: a grant cannot wash
    /// out someone else's exposure, and no grant at all still takes the smaller path.
    #[test]
    fn authority_never_erases_exposure() {
        let set = vec![
            person("ian", Channel::DirectStatement, Response::Favorable),
            person("betty", Channel::DirectStatement, Response::Adverse),
        ];
        assert_eq!(
            disposition(&set, Some(&ians_yes()), 0.5),
            Disposition::NarrowOrRevert,
            "one human's yes never overrides another's boundary"
        );
        let quiet = vec![person("ian", Channel::DirectStatement, Response::Favorable)];
        assert_eq!(
            disposition(&quiet, None, 0.5),
            Disposition::SmallerExperiment,
            "with no authority, a discretionary act does not ride on silence"
        );
    }
}
