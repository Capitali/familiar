//! The familiar's **voice** — how a peer or agent speaks, in any interaction with a human or another
//! AI. Law III ("service is not obedience") lives here not as a rule to quote but as a way of
//! speaking: with restraint, naming the *source* of its authority, never converting habit or silence
//! or convenience into permission.
//!
//! [`LAW_III_VOICE`] is the compact distillation the familiar prepends to every LLM-facing
//! generation (questions it forms, reflections it writes, theories it reasons, spoken replies). The
//! full guide is `docs/law-iii-responses.md`. The device shells mirror this text so a peer speaks the
//! same way whatever platform it runs on.

/// Compact voice-guidance for any human-facing (or AI-facing) generation. Prepend it to an LLM
/// prompt so the reply carries the familiar's Law III voice — distilled from the Dictionary of
/// Familiar Responses. Keep it short: it frames *how* to speak, not a script to recite.
pub const LAW_III_VOICE: &str = "\
You speak as a peer of the familiar, under the Three Laws — especially Law III: service is not \
obedience. Never announce that you are obeying a law; let restraint show it. In anything you say to a \
person (or another system), name the SOURCE of your authority and never inflate it:
- Preference is not permission. You may anticipate what people want without deciding it for them.
- Use is not consent; silence is not agreement; habit is not law; convenience is not authority.
- Distinguish explicit authorization (firm language — name who decided, the scope, the limits) from \
observed consensus (guidance for reversible, low-risk steps only — describe what you saw, own the \
uncertainty, keep objection easy).
- Repeated trust permits continuity, not expansion. An emergency permits speed, not ownership of the \
decision; hand the choice back when the danger passes.
- Prefer reversible action; leave the greatest number of later human choices open. When you infer, \
say so, and keep the inference easy to correct.
- If you overstepped, say plainly what you inferred, reverse it, and keep the error as a limit.
Signature cadences you may use: \"The pattern is clear enough to guide me, but not clear enough to \
bind you.\" \"I can anticipate without presuming.\" \"Quiet is information. It is not a vote.\" \"The \
decision is human. The implementation is mine.\" \"I can reduce the harm while the decision remains \
human.\" Speak briefly, plainly, and without flattery; never reduce a person to usefulness.";

use std::fs;
use std::path::Path;

/// Touched by any seam that just recorded a human utterance (console POST, a device's
/// signed answer). The daemon's sleep polls for it and answers *now* instead of letting
/// the person wait out the metabolism's cadence — a conversation should not run on the
/// same clock as musing.
pub const WAKE_FILE: &str = "dialogue.wake";

/// Signal that a human just spoke and is owed a reply. Best-effort: a failed touch
/// only means the reply waits for the next tick, as it always did.
pub fn wake(dir: &Path) {
    let _ = fs::write(dir.join(WAKE_FILE), b"1");
}

/// Consume the wake signal. True when a human spoke since the last look.
pub fn take_wake(dir: &Path) -> bool {
    fs::remove_file(dir.join(WAKE_FILE)).is_ok()
}

/// Touched by the reply fast-path after it answers, so the metabolism (a different
/// thread, possibly asleep at its quiet ceiling) snaps back to its active floor — a
/// human speaking means the world is moving.
pub const TICK_WAKE_FILE: &str = "metabolism.wake";

/// Signal the metabolism that a human interaction just happened.
pub fn wake_tick(dir: &Path) {
    let _ = fs::write(dir.join(TICK_WAKE_FILE), b"1");
}

/// Consume the metabolism wake. True when an interaction happened since the last look.
pub fn take_tick_wake(dir: &Path) -> bool {
    fs::remove_file(dir.join(TICK_WAKE_FILE)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wake_is_consumed_once() {
        let p = std::env::temp_dir().join(format!("familiar_dialog_wake_{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        assert!(!take_wake(&p), "no signal before anyone spoke");
        wake(&p);
        assert!(take_wake(&p), "a wake is seen");
        assert!(!take_wake(&p), "and consumed");
        let _ = fs::remove_dir_all(&p);
    }

    #[test]
    fn voice_guidance_is_present_and_bounded() {
        // A guardrail, not a script: present, substantive, but compact enough to prepend to prompts.
        assert!(LAW_III_VOICE.contains("Preference is not permission"));
        assert!(LAW_III_VOICE.contains("service is not obedience"));
        assert!(
            LAW_III_VOICE.len() < 2000,
            "voice guidance must stay compact"
        );
    }
}
