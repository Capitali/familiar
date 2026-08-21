//! The question coordination — where the familiar decides *what to ask, and when*.
//!
//! The familiar does not fire questions at the human as it thinks of them. It keeps a
//! registry of everything it might ask — the origin-story root ("What do you need most
//! today?"), questions it forms from its theories, and clarifications it needs to complete
//! an observation or make a decision — and surfaces **one at a time**, chosen by a policy.
//!
//! A dismissed question is **never thrown away**. Dismissal is data: it grows the question's
//! rest period (so the familiar doesn't nag) and is recorded with its context (the seed of
//! later understanding *why* it was dismissed). The root question recurs whenever the policy
//! judges the moment right — weighing how often it's been dismissed, unmet human needs, and
//! the familiar's own need to know. Append-only JSONL; a rewrite updates a question's stats.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

use crate::store;

pub const QUESTIONS_FILE: &str = "questions.jsonl";
/// The origin-story root question — the standing one the familiar always returns to.
pub const ROOT_TEXT: &str = "What do you need most today?";
pub const ROOT_ID: &str = "q-root";

/// Rest after a question is *answered* before it may recur (root only; non-root answered
/// questions retire). Eight hours — a day's natural cadence.
pub const ANSWER_REST_SECS: i64 = 8 * 3600;
/// Base rest after a *dismissal*; the actual rest grows with how often it's been dismissed,
/// so a question the human keeps waving off is asked less and less — but never never.
pub const DISMISS_REST_SECS: i64 = 4 * 3600;
/// Cap on the grown dismissal rest — a week. Even an oft-dismissed question comes back.
pub const DISMISS_REST_MAX_SECS: i64 = 7 * 24 * 3600;

/// What may turn on an answer (brick 3, T-181 / ADR-0040 D2). There is deliberately no
/// `none`: a question with nothing turning on it is unrepresentable — the typed form of
/// the prompt line "ask because you want to know, never to seem attentive."
pub const STAKES: [&str; 3] = ["continues", "changes", "stops"];

/// A question as drafted, before it may enter the registry. Every producer builds one of
/// these and passes [`AskDraft::check`]; there is no untyped path to [`admit`].
#[derive(Debug, Clone, Default)]
pub struct AskDraft {
    pub question: String,
    /// Why the question arose — not a restatement of it.
    pub because: String,
    /// The decision or belief that awaits the answer.
    pub turns_on: String,
    /// What happens to `turns_on` when the answer lands: one of [`STAKES`].
    pub stake: String,
}

/// Content words of `s` (lowercased, alphanumeric runs, length ≥ 3).
fn content_tokens(s: &str) -> std::collections::BTreeSet<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 3)
        .map(str::to_string)
        .collect()
}

/// True when `field` adds no content beyond the question itself — every content word
/// already appears in the question. The mechanical floor of the anti-vacuity rule
/// (conduct dialogue, Round 2/3): four populated strings can still encode no real
/// dependency, so a field that merely re-treads the question refuses.
fn restates(field: &str, question: &str) -> bool {
    let f = content_tokens(field);
    f.is_empty() || f.is_subset(&content_tokens(question))
}

impl AskDraft {
    /// The admission check. Refusals are sentences meant to be recorded and, on a
    /// consult path, told back to the model.
    pub fn check(&self) -> Result<(), String> {
        if self.question.trim().is_empty() {
            return Err("ask refused: empty question".into());
        }
        if self.because.trim().is_empty() {
            return Err("ask refused: `because` is empty — why did this question arise?".into());
        }
        if self.turns_on.trim().is_empty() {
            return Err(
                "ask refused: `turns_on` is empty — what decision or belief awaits the answer?"
                    .into(),
            );
        }
        let stake = self.stake.trim().to_lowercase();
        if !STAKES.contains(&stake.as_str()) {
            return Err(format!(
                "ask refused: stake '{}' is not one of continues|changes|stops — a question \
                 with nothing turning on it is unrepresentable",
                self.stake.trim()
            ));
        }
        if restates(&self.because, &self.question) {
            return Err(
                "ask refused: `because` restates the question instead of saying why it arose"
                    .into(),
            );
        }
        if restates(&self.turns_on, &self.question) {
            return Err(
                "ask refused: `turns_on` restates the question instead of naming what \
                 awaits the answer"
                    .into(),
            );
        }
        Ok(())
    }
}

/// One thing the familiar may ask the human, with its history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub text: String,
    /// Where it came from: `"root"`, `"llm"` (a theory), `"need"` (to complete an observed
    /// need), `"observer"`. Used to prioritise.
    pub origin: String,
    pub created_at: i64,
    pub times_asked: u32,
    pub times_dismissed: u32,
    pub last_asked: i64,
    pub last_dismissed: i64,
    /// True once answered. The root question is the exception — it recurs regardless.
    pub answered: bool,
    /// Why it was waved off, when known — the seed of understanding dismissal, kept so it is
    /// never merely discarded.
    pub dismiss_notes: Vec<String>,
    /// The human this question is **addressed to** — who is being asked, and who is accountable
    /// for it (`crate::routing`). Empty means unaddressed, which for a familiar serving several
    /// people means "announced at the room", the thing owners exist to stop.
    ///
    /// Ownership governs who is ASKED, never who may read: once an answer is confirmed it becomes
    /// an ordinary observation in the shared worldview, public to the whole mesh.
    #[serde(default)]
    pub owner: String,
    /// The human this question is **about** — whose need it serves and whom it must reach
    /// (ADR-0022). Distinct from `owner`: the owner is whoever is being asked *now*; the
    /// subject is the person the question exists for, and it waits for them rather than
    /// landing on whoever holds the room. Empty means anyone may answer it.
    #[serde(default)]
    pub subject: String,
    /// The thread this question serves, when it is a theory's confirm-question — the
    /// subject's answer attaches there as evidence (`thread::add_answer_from`) and can
    /// make a theorized need a stated one. Empty for standalone questions.
    #[serde(default)]
    pub thread_id: String,
    /// Why the question arose (brick 3). Empty only on rows minted before stakes existed.
    #[serde(default)]
    pub because: String,
    /// The decision or belief that awaits the answer.
    #[serde(default)]
    pub turns_on: String,
    /// What happens to `turns_on` when the answer lands: one of [`STAKES`]. New rows
    /// always carry one — [`AskDraft::check`] makes a stakeless question unrepresentable.
    #[serde(default)]
    pub stake: String,
    /// T-219: retired by explicit policy (its subject stopped existing, or its class was
    /// deliberately ended) — never an invented answer. A retired question never
    /// surfaces; the row and its history stay (append-retained, like everything).
    #[serde(default)]
    pub retired: bool,
}

impl Question {
    fn new(id: &str, draft: &AskDraft, origin: &str, now: i64) -> Self {
        Question {
            id: id.to_string(),
            text: draft.question.trim().to_string(),
            origin: origin.to_string(),
            created_at: now,
            times_asked: 0,
            times_dismissed: 0,
            last_asked: 0,
            last_dismissed: 0,
            answered: false,
            dismiss_notes: Vec::new(),
            owner: String::new(),
            subject: String::new(),
            thread_id: String::new(),
            because: draft.because.trim().to_string(),
            turns_on: draft.turns_on.trim().to_string(),
            stake: draft.stake.trim().to_lowercase(),
            retired: false,
        }
    }

    fn is_root(&self) -> bool {
        self.id == ROOT_ID
    }

    /// How long this question should rest before it may surface again.
    fn rest_secs(&self) -> i64 {
        if self.last_dismissed >= self.last_asked && self.times_dismissed > 0 {
            (DISMISS_REST_SECS * (1 + self.times_dismissed as i64)).min(DISMISS_REST_MAX_SECS)
        } else if self.answered {
            ANSWER_REST_SECS
        } else {
            0 // never engaged yet — available immediately
        }
    }

    /// May this question surface now? Retired (answered, non-root) questions never do;
    /// everything else is available once it has rested long enough since last shown/dismissed.
    pub fn available(&self, now: i64) -> bool {
        if self.retired {
            return false; // ended by policy — its subject or class stopped existing
        }
        if self.answered && !self.is_root() {
            return false;
        }
        let last = self.last_asked.max(self.last_dismissed);
        now - last >= self.rest_secs()
    }
}

pub fn load(dir: &Path) -> io::Result<Vec<Question>> {
    store::load(dir, QUESTIONS_FILE)
}

pub fn append(dir: &Path, q: &Question) -> io::Result<()> {
    store::append(dir, QUESTIONS_FILE, q)
}

/// Seed the root question once, so the familiar always has the origin-story question to
/// return to. Idempotent.
pub fn ensure_root(dir: &Path, now: i64) -> io::Result<()> {
    let qs = load(dir)?;
    if !qs.iter().any(|q| q.id == ROOT_ID) {
        let draft = AskDraft {
            question: ROOT_TEXT.to_string(),
            because: "service begins by asking rather than assuming".to_string(),
            turns_on: "which service the familiar attends to first".to_string(),
            stake: "changes".to_string(),
        };
        debug_assert!(
            draft.check().is_ok(),
            "the root question carries its stakes"
        );
        append(dir, &Question::new(ROOT_ID, &draft, "root", now))?;
    }
    Ok(())
}

/// Admit a question the familiar formed (e.g. from a theory or an unmet need). The draft
/// must pass [`AskDraft::check`] — the inner `Err` is the refusal sentence and nothing is
/// written. Dedup is kept: re-drafting an existing text returns the standing id (don't ask
/// the same thing twice).
pub fn admit(
    dir: &Path,
    draft: &AskDraft,
    origin: &str,
    now: i64,
) -> io::Result<Result<String, String>> {
    if let Err(why) = draft.check() {
        return Ok(Err(why));
    }
    let text = draft.question.trim();
    let mut qs = load(dir)?;
    if let Some(existing) = qs.iter().find(|q| q.text == text) {
        return Ok(Ok(existing.id.clone()));
    }
    let id = format!("q-{:04}", qs.len() + 1);
    let q = Question::new(&id, draft, origin, now);
    qs.push(q.clone());
    append(dir, &q)?;
    Ok(Ok(id))
}

/// [`admit`], for a question that exists FOR someone: a confirm-question for a need the
/// factory theorized about `subject`, serving `thread_id`. The subject waits for its
/// person (see the coordination policy) instead of landing on whoever holds the room.
pub fn admit_addressed(
    dir: &Path,
    draft: &AskDraft,
    origin: &str,
    subject: &str,
    thread_id: &str,
    now: i64,
) -> io::Result<Result<String, String>> {
    let id = match admit(dir, draft, origin, now)? {
        Ok(id) => id,
        Err(why) => return Ok(Err(why)),
    };
    let subject = subject.trim().to_lowercase();
    let thread_id = thread_id.trim().to_string();
    update(dir, &id, |q| {
        q.subject = subject.clone();
        q.thread_id = thread_id.clone();
    })?;
    Ok(Ok(id))
}

fn update<F: FnMut(&mut Question)>(dir: &Path, id: &str, mut f: F) -> io::Result<bool> {
    let Some(mut q) = store::load_by_id::<Question>(dir, QUESTIONS_FILE, id)? else {
        return Ok(false);
    };
    f(&mut q);
    store::update_by_id(dir, QUESTIONS_FILE, id, &q)
}

/// Address this question to a human (`crate::routing::route` decides who). Idempotent, and
/// re-addressing is normal rather than exceptional: the person a question was put to walks out of
/// the room, and leaving it addressed to an empty chair is worse than handing it to whoever is here.
pub fn set_owner(dir: &Path, id: &str, owner: &str) -> io::Result<bool> {
    let owner = owner.trim().to_lowercase();
    update(dir, id, |q| q.owner = owner.clone())
}

pub fn record_asked(dir: &Path, id: &str, now: i64) -> io::Result<bool> {
    update(dir, id, |q| {
        q.times_asked += 1;
        q.last_asked = now;
    })
}

/// A dismissal — tracked, never disposed. Grows the rest period and keeps the (optional)
/// reason so the familiar can later learn why this question doesn't land.
pub fn record_dismissed(dir: &Path, id: &str, now: i64, note: &str) -> io::Result<bool> {
    update(dir, id, |q| {
        q.times_dismissed += 1;
        q.last_dismissed = now;
        if !note.trim().is_empty() {
            q.dismiss_notes.push(note.trim().to_string());
        }
    })
}

/// T-222: mark answered every registry question bound to `thread_id` — the durable-id
/// join from a thread answer to the question the console actually asked. Returns how
/// many questions closed. Root is exempt by its own lifecycle (it recurs regardless).
pub fn record_answered_for_thread(dir: &Path, thread_id: &str, now: i64) -> io::Result<usize> {
    let thread_id = thread_id.trim();
    if thread_id.is_empty() {
        return Ok(0);
    }
    let mut closed = 0;
    for q in load(dir)? {
        if q.thread_id == thread_id && !q.answered && record_answered(dir, &q.id, now)? {
            closed += 1;
        }
    }
    Ok(closed)
}

/// T-222's conservative backfill: close every unanswered thread-bound question whose
/// thread ALREADY carries a human answer — the same durable-id join applied to history,
/// run idempotently (a no-op when the registry is current). Ambiguity is impossible by
/// construction: the join is `Question.thread_id`, nothing else. Questions without a
/// thread binding are untouched — their retirement is explicit policy (T-219), never an
/// invented answer.
pub fn backfill_answered(dir: &Path, now: i64) -> io::Result<usize> {
    let answered_threads: std::collections::BTreeSet<String> = crate::thread::load(dir)?
        .into_iter()
        .filter(|t| !t.answers.is_empty())
        .map(|t| t.id)
        .collect();
    let mut closed = 0;
    for q in load(dir)? {
        if !q.answered
            && !q.thread_id.is_empty()
            && answered_threads.contains(&q.thread_id)
            && record_answered(dir, &q.id, now)?
        {
            closed += 1;
        }
    }
    Ok(closed)
}

/// T-219: retire a question by explicit policy, with the reason kept beside its history.
/// The root never retires (its lifecycle is recurrence).
pub fn retire(dir: &Path, id: &str, why: &str, _now: i64) -> io::Result<bool> {
    if id == ROOT_ID {
        return Ok(false);
    }
    update(dir, id, |q| {
        q.retired = true;
        if !why.trim().is_empty() {
            q.dismiss_notes.push(format!("retired: {}", why.trim()));
        }
    })
}

pub fn record_answered(dir: &Path, id: &str, now: i64) -> io::Result<bool> {
    update(dir, id, |q| {
        q.answered = true;
        q.last_asked = now;
    })
}

/// The priority of an origin — higher surfaces first. Completing an observed human need
/// outranks the standing root, which outranks the familiar's own theories.
fn origin_rank(origin: &str) -> u8 {
    match origin {
        "need" => 3,
        "root" => 2,
        _ => 1,
    }
}

/// Choose the question to surface now (or `None` to ask nothing). Among the available
/// questions, prefer higher origin-rank; then the one dismissed *least* (don't nag); then
/// the oldest. `unmet_needs` biases the familiar toward asking about needs over the root.
pub fn next(questions: &[Question], now: i64, unmet_needs: usize) -> Option<&Question> {
    questions
        .iter()
        .filter(|q| q.available(now))
        .max_by_key(|q| {
            let mut rank = origin_rank(&q.origin) as i64;
            // when needs are waiting, lift need-questions decisively above the root
            if unmet_needs > 0 && q.origin == "need" {
                rank += 5;
            }
            // prefer the least-dismissed, then the oldest (negative for max_by_key)
            (rank, -(q.times_dismissed as i64), -q.created_at)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct Temp(PathBuf);
    impl Temp {
        fn new(t: &str) -> Self {
            let p = std::env::temp_dir()
                .join(format!("familiar_question_test_{}_{t}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Temp(p)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn root_is_seeded_once_and_recurs_after_answering() {
        let t = Temp::new("root");
        ensure_root(&t.0, 0).unwrap();
        ensure_root(&t.0, 0).unwrap(); // idempotent
        assert_eq!(load(&t.0).unwrap().len(), 1);
        // answered -> rests, then recurs after the answer-rest window
        record_answered(&t.0, ROOT_ID, 1000).unwrap();
        let qs = load(&t.0).unwrap();
        assert!(!qs[0].available(1000), "just answered — resting");
        assert!(
            qs[0].available(1000 + ANSWER_REST_SECS),
            "the root question returns"
        );
    }

    #[test]
    fn dismissal_is_tracked_grows_the_rest_and_is_never_disposed() {
        let t = Temp::new("dismiss");
        ensure_root(&t.0, 0).unwrap();
        record_dismissed(&t.0, ROOT_ID, 1000, "not now").unwrap();
        let q = &load(&t.0).unwrap()[0];
        assert_eq!(q.times_dismissed, 1);
        assert_eq!(q.dismiss_notes, vec!["not now".to_string()]);
        // still present (not disposed), resting longer than a fresh dismissal would
        assert!(!q.available(1000 + DISMISS_REST_SECS - 1));
        assert!(q.available(1000 + 2 * DISMISS_REST_SECS));
        // dismissed again -> rests longer still (asked less and less, never never)
        record_dismissed(&t.0, ROOT_ID, 2000, "").unwrap();
        let q = &load(&t.0).unwrap()[0];
        assert_eq!(q.times_dismissed, 2);
        assert!(!q.available(2000 + 2 * DISMISS_REST_SECS));
    }

    fn staked(text: &str) -> AskDraft {
        AskDraft {
            question: text.to_string(),
            because: "her overnight job looked stalled twice".to_string(),
            turns_on: "whether to keep watching that job".to_string(),
            stake: "continues".to_string(),
        }
    }

    #[test]
    fn need_questions_outrank_the_root_when_needs_wait() {
        let t = Temp::new("rank");
        ensure_root(&t.0, 0).unwrap();
        admit(&t.0, &staked("Did the backup finish?"), "need", 10)
            .unwrap()
            .unwrap();
        let qs = load(&t.0).unwrap();
        // with an unmet need pending, the need-question is chosen over the root
        assert_eq!(next(&qs, 100, 1).map(|q| q.origin.as_str()), Some("need"));
        // dedup: admitting the same text again doesn't create a second
        admit(&t.0, &staked("Did the backup finish?"), "need", 20)
            .unwrap()
            .unwrap();
        assert_eq!(load(&t.0).unwrap().len(), 2);
    }

    #[test]
    fn a_question_with_nothing_turning_on_it_is_unrepresentable() {
        let t = Temp::new("stakes");
        // No stake at all — refused; nothing written.
        let mut d = staked("Did the backup finish?");
        d.stake = String::new();
        assert!(admit(&t.0, &d, "need", 10).unwrap().is_err());
        // A stake outside the set — refused. There is deliberately no `none`.
        d.stake = "none".to_string();
        assert!(admit(&t.0, &d, "need", 10).unwrap().is_err());
        // Empty because / turns_on — refused.
        let mut d = staked("Did the backup finish?");
        d.because = "  ".to_string();
        assert!(admit(&t.0, &d, "need", 10).unwrap().is_err());
        let mut d = staked("Did the backup finish?");
        d.turns_on = String::new();
        assert!(admit(&t.0, &d, "need", 10).unwrap().is_err());
        assert!(load(&t.0).unwrap().is_empty(), "refusals write nothing");
        // The staked draft is admitted, wearing its stakes.
        let id = admit(&t.0, &staked("Did the backup finish?"), "need", 10)
            .unwrap()
            .unwrap();
        let q = load(&t.0).unwrap().remove(0);
        assert_eq!(q.id, id);
        assert_eq!(q.stake, "continues");
        assert!(!q.because.is_empty() && !q.turns_on.is_empty());
    }

    #[test]
    fn restating_the_question_is_not_a_because() {
        let t = Temp::new("vacuity");
        // `because` made only of the question's own content words — vacuous, refused.
        let mut d = staked("Did the nightly backup finish?");
        d.because = "the nightly backup — did it finish".to_string();
        let why = admit(&t.0, &d, "need", 10).unwrap().unwrap_err();
        assert!(why.contains("restates"), "{why}");
        // Same for turns_on.
        let mut d = staked("Did the nightly backup finish?");
        d.turns_on = "the nightly backup finish".to_string();
        assert!(admit(&t.0, &d, "need", 10).unwrap().is_err());
        // Fields that add real content pass.
        assert!(
            admit(&t.0, &staked("Did the nightly backup finish?"), "need", 10)
                .unwrap()
                .is_ok()
        );
    }

    #[test]
    fn a_thread_answer_closes_its_registry_question_by_durable_id_only() {
        let t = Temp::new("t222_join");
        // Two addressed questions on two threads, one standalone, and the root.
        ensure_root(&t.0, 0).unwrap();
        let d1 = AskDraft {
            question: "Betty — long evenings?".into(),
            because: "her lights burned past midnight three nights running".into(),
            turns_on: "whether to keep watching her nights".into(),
            stake: "continues".into(),
        };
        admit_addressed(&t.0, &d1, "need", "betty", "thread-0001", 10)
            .unwrap()
            .unwrap();
        let d2 = AskDraft {
            question: "Dim when away?".into(),
            because: "three evening adjustments followed departures".into(),
            turns_on: "a standing lighting rule".into(),
            stake: "changes".into(),
        };
        admit_addressed(&t.0, &d2, "need", "ian", "thread-0002", 20)
            .unwrap()
            .unwrap();
        // An answer lands on thread-0002: exactly ITS question closes — by id, never by
        // prose or recency (thread-0001's stays open; the root is untouched).
        assert_eq!(
            record_answered_for_thread(&t.0, "thread-0002", 100).unwrap(),
            1
        );
        let qs = load(&t.0).unwrap();
        let by_text = |txt: &str| qs.iter().find(|q| q.text == txt).unwrap();
        assert!(by_text("Dim when away?").answered);
        assert!(!by_text("Betty — long evenings?").answered);
        assert!(!qs.iter().find(|q| q.id == ROOT_ID).unwrap().answered);
        // Idempotent: answering the same thread again closes nothing further.
        assert_eq!(
            record_answered_for_thread(&t.0, "thread-0002", 200).unwrap(),
            0
        );
        // An unbound thread id closes nothing.
        assert_eq!(
            record_answered_for_thread(&t.0, "thread-9999", 300).unwrap(),
            0
        );
        // And an answered non-root question no longer surfaces (retired by lifecycle).
        assert!(!by_text("Dim when away?").available(10_000_000));
    }

    #[test]
    fn rows_minted_before_stakes_existed_still_load() {
        let t = Temp::new("legacy");
        // A pre-brick-3 row, byte-for-byte without the stake fields.
        std::fs::write(
            t.0.join(QUESTIONS_FILE),
            concat!(
                r#"{"id":"q-0001","text":"old?","origin":"llm","created_at":5,"#,
                r#""times_asked":0,"times_dismissed":0,"last_asked":0,"last_dismissed":0,"#,
                r#""answered":false,"dismiss_notes":[]}"#,
                "
"
            ),
        )
        .unwrap();
        let qs = load(&t.0).unwrap();
        assert_eq!(qs.len(), 1);
        assert!(qs[0].stake.is_empty(), "legacy rows wear no stake");
        assert!(qs[0].available(100), "…and still surface as before");
    }
}
