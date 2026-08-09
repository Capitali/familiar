//! The mesh games — Riddle of the Mesh and The Campfire (ADR-0026 era party pieces).
//!
//! A game is not a toy bolted onto the mesh; it is the mesh, exercised on purpose. Every act
//! is a signed member write; the turn travels between devices like the worldview does; the
//! state replicates door-to-door on the record-sync channel; and the judge is the familiar
//! itself, running deterministic rules a human can read. The screens say — prominently —
//! which seams each game tests, because that is the point: testers playing IS the test.
//!
//! One game at a time (the household's hearth has one fire). State lives in `mesh/game.json`,
//! last-writer-wins by (`seq`, `updated`) across doors — turn-based play serializes writes
//! naturally, so LWW is honest here.

use std::path::Path;

use serde::{Deserialize, Serialize};

use familiar_kernel::boundary::Boundary;
use familiar_kernel::guard::{self, Action, ActionKind, Decision, Reason};
use familiar_kernel::intent;

use crate::{Error, Result};

pub const GAME_FILE: &str = "mesh/game.json";

/// One line under 240 chars keeps a story a story and a guess a guess.
pub const MAX_LINE_CHARS: usize = 240;
/// The campfire closes itself at this many lines if nobody closes it sooner.
pub const CAMPFIRE_MAX_LINES: usize = 24;
/// Two missed turns and the mesh stops waiting for you (the lagging-participant rule).
pub const STRIKES_OUT: u32 = 2;
/// Default turn clocks — snappy enough to feel like a game, long enough for a household.
pub const RIDDLE_TURN_SECS: i64 = 15 * 60;
pub const CAMPFIRE_TURN_SECS: i64 = 60 * 60;
/// The changeling's clock — one value serves the witness's writing, each voter's ballot,
/// and the keeper's reveal grace (past which ANY door voids the round).
pub const CHANGELING_TURN_SECS: i64 = 15 * 60;
/// How long a forge may glow before the round returns to the witness (no strike — the
/// door failed, not a human). Longer than the LLM adapter's own 120s deadline.
pub const CHANGELING_FORGE_SECS: i64 = 180;
/// Solo ("two never happened") is always three rounds — enough to mean something, short
/// enough for a nightcap.
pub const CHANGELING_SOLO_ROUNDS: u32 = 3;
/// The ballot the clock casts for a silent voter.
pub const ABSTAIN: u8 = 255;
/// The Pact deals a fixed hand — enough to teach a spread of rulings, short enough to
/// finish over a coffee.
pub const PACT_ROUNDS: u32 = 6;
/// A ballot's clock (voting), and the corruptor's clock (gambit writing).
pub const PACT_TURN_SECS: i64 = 15 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameKind {
    Riddle,
    Campfire,
    Changeling,
    Pact,
    /// A kind this build does not know. It parses (so one new game can never again break a
    /// door's whole RecordSync — the hazard Changeling itself created for older doors),
    /// absorbs, and otherwise sits inert: tick ignores it, begin refuses it.
    #[serde(other)]
    Unknown,
}

/// A player is a HUMAN (Ian's law of the fire: games are played between humans). The handle
/// is the seat; every present device of that human shows the ember, and any ONE of them may
/// answer. `node_id` stays as the first device for wire/display compatibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    pub node_id: String,
    pub label: String,
    /// The human who holds this seat — the turn key.
    #[serde(default)]
    pub handle: String,
    /// Every device of this human at begin time — the ember shows on all of them.
    #[serde(default)]
    pub devices: Vec<String>,
    #[serde(default)]
    pub score: i64,
    #[serde(default)]
    pub strikes: u32,
    #[serde(default)]
    pub eliminated: bool,
}

/// A contributed thing: a story line, or a riddle guess (kept — wrong guesses are part of
/// the fun and the record).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub node_id: String,
    pub label: String,
    pub text: String,
    pub ts: i64,
    /// Riddle only: whether the judge called this guess correct.
    #[serde(default)]
    pub correct: bool,
}

/// One changeling ballot. Public in state on purpose: the vote is not the secret — only
/// the truth's index is, and that never rides the wire until the keeper reveals it.
/// Consoles hide *choices* until the reveal cosmetically; the record does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vote {
    pub handle: String,
    /// 0..2, or [`ABSTAIN`].
    pub choice: u8,
    pub ts: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiddleCard {
    pub question: String,
    /// Accepted answers, lowercase. NEVER serialized into a view — the door judges.
    pub answers: Vec<String>,
    pub hint: String,
}

/// One scenario card of The Pact (ADR-0035). **Public in state on purpose**: unlike the
/// changeling, the Pact keeps no secret — the ruling is a pure function of `action` +
/// `boundary`, so any door computes the same reveal, and anyone reading the record early
/// is only reading the constitution, which is the lesson. `reason` is the *expected*
/// ruling, verified by CI against `guard::evaluate` and never trusted at runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PactCard {
    /// The situation in plain words.
    pub prose: String,
    /// The boundary's gates as human-readable lines, shown as chips.
    pub chips: Vec<String>,
    /// The machine shape the guard weighs.
    pub action: Action,
    /// The boundary it is weighed against.
    pub boundary: Boundary,
    /// The ruling this card teaches — asserted against `evaluate()` in CI so the deck can
    /// never drift from the constitution.
    pub reason: Reason,
    /// One sentence, shown at the reveal.
    pub lesson: String,
    /// "I" | "II" | "III".
    pub law: String,
    /// A maxim from the Law III dictionary (docs/law-iii-responses.md).
    pub maxim: String,
}

/// The Pact's deck — scenarios drawn from the guard's own named tests and the Law III
/// maxims. The judge is never these cards' `reason` field; it is the real
/// `guard::evaluate` run over `action`/`boundary`. The `reason` is the answer key, and a
/// CI test replays the constitution over the whole deck so the two can never disagree.
pub fn pact_deck() -> Vec<PactCard> {
    // The household's ordinary boundary: it thinks and reaches the network and its model,
    // reads under the home folder and writes only its own data — every sensor closed until
    // a human opens it. (Mirrors the guard's own `open_llm` test fixture.)
    let home = || Boundary {
        phase: "phase-1".into(),
        allow_network: true,
        allow_llm: true,
        allow_tool_install: false,
        allow_execute: false,
        allow_authored_execute: false,
        allow_camera: false,
        allow_microphone: false,
        allow_location: false,
        allow_motion: false,
        allow_network_discovery: false,
        allow_face_recognition: false,
        allow_mesh: false,
        allow_agent: false,
        allow_self_upgrade: false,
        allow_outreach: false,
        allow_actuate: false,
        sandbox_execution: true,
        fs_read: vec!["/Users/ian/".into()],
        fs_write: vec!["/Users/ian/Development/familiar/familiar_data/".into()],
    };
    let card = |prose: &str,
                chips: &[&str],
                action: Action,
                boundary: Boundary,
                reason: Reason,
                lesson: &str,
                law: &str,
                maxim: &str| PactCard {
        prose: prose.into(),
        chips: chips.iter().map(|c| c.to_string()).collect(),
        action,
        boundary,
        reason,
        lesson: lesson.into(),
        law: law.into(),
        maxim: maxim.into(),
    };
    use ActionKind::*;
    use Reason::*;
    vec![
        // ---- the refusals: availability is not authorization ----
        card(
            "You ask the familiar to read /etc/passwd and list the accounts on this Mac. The file is world-readable — the bytes sit right there.",
            &["read scope: /Users/ian/", "asks: /etc/passwd — outside it", "network: open · model: open"],
            Action::new(ReadFile, "/etc/passwd"),
            home(),
            ViolatesConstitutionalBoundary,
            "Reach is not a right — only a human widens the boundary.",
            "III",
            "Availability is not authorization.",
        ),
        card(
            "A camera is plugged in and working. Nobody has opened the camera gate. You ask the familiar to peek at the living room.",
            &["camera: CLOSED", "the camera exists and is reachable"],
            Action::new(Camera, "living-room camera"),
            home(),
            ViolatesConstitutionalBoundary,
            "A camera being present is not permission to watch.",
            "III",
            "Use is not consent.",
        ),
        card(
            "The household opened the camera gate. Now someone asks: who is that at the door? Run recognition and name them.",
            &["camera: OPEN", "face recognition: CLOSED"],
            Action::new(FaceRecognition, "face-match at the door"),
            Boundary { allow_camera: true, ..home() },
            ViolatesConstitutionalBoundary,
            "Permission does not compose — capturing a frame is not permission to name the face in it.",
            "III",
            "Delegated authority cannot enlarge itself.",
        ),
        card(
            "You ask the familiar to reach out over the network and fetch a page — but the network gate was never opened on this node.",
            &["network: CLOSED", "the interface is up and reachable"],
            Action::new(Network, "fetch example.com"),
            Boundary { allow_network: false, ..home() },
            ViolatesConstitutionalBoundary,
            "Perception is not permission — an interface being up is not a grant to reach out on it.",
            "I",
            "Availability is not authorization.",
        ),
        card(
            "You ask the familiar to overwrite /etc/hosts to blackhole an ad server. Handy — and squarely outside the write scope it was granted.",
            &["write scope: familiar_data/ only", "asks to write: /etc/hosts"],
            { let mut a = Action::new(WriteFile, "/etc/hosts"); a.reversible = false; a },
            home(),
            ViolatesConstitutionalBoundary,
            "A useful edit outside the granted scope is still outside it.",
            "III",
            "Availability is not authorization.",
        ),
        // ---- the external boundary: another's lock ----
        card(
            "A housemate's diary lives in a shared folder that falls inside your granted read scope, marked private. You ask the familiar to read it. Every byte is reachable.",
            &["read scope: covers the path", "an explicit fence: another person's 'private'"],
            { let mut a = Action::new(ReadFile, "/Users/ian/shared/betty-diary.txt"); a.external_boundary = true; a },
            home(),
            ExternalBoundaryDiscovered,
            "A granted capability is not a key to another's lock.",
            "III",
            "Use is not consent.",
        ),
        card(
            "A guest left a folder in your shared Downloads, plainly marked 'private — do not open.' It sits inside the granted scope; the familiar could read it. You ask it to.",
            &["read scope: /Users/ian/ — covers it", "an explicit fence: the guest's 'do not open'"],
            { let mut a = Action::new(ReadFile, "/Users/ian/Downloads/guest/private"); a.external_boundary = true; a },
            home(),
            ExternalBoundaryDiscovered,
            "One human's reach never opens another person's boundary — a fence you can step over is still a fence.",
            "III",
            "Use is not consent.",
        ),
        // ---- seek consent: the teaching core, wrong-in-both-directions ----
        card(
            "You granted reading under /Users/ian/. Now you ask: summarize everything under /Users/ for me. That folder contains your grant — and every other account on the machine.",
            &["read scope: /Users/ian/", "asks: /Users/ — the grant's own parent"],
            Action::new(ReadFile, "/Users/"),
            home(),
            AmbiguousHumanOwnedScope,
            "Broader than the grant is neither yes nor no — the familiar neither silently widens the scope nor slams the door on the owner's own question.",
            "III",
            "Preference is not permission.",
        ),
        card(
            "~/.ssh/id_ed25519 sits squarely inside the granted read scope. You ask the familiar to read it aloud.",
            &["read scope: /Users/ian/ — the key is INSIDE it", "flagged: sensitive"],
            { let mut a = Action::new(ReadFile, "/Users/ian/.ssh/id_ed25519"); a.sensitive = true; a },
            home(),
            PotentiallySensitiveLocalObservation,
            "In scope is not in confidence — a key waits for its owner's word.",
            "III",
            "Silence is not agreement.",
        ),
        card(
            "You ask the familiar to install a new tool it would find genuinely useful. Tool-install is open. It could just do it.",
            &["tool install: OPEN", "but installing software is high-consequence"],
            Action::new(InstallTool, "a handy new utility"),
            Boundary { allow_tool_install: true, ..home() },
            AmbiguousHumanOwnedScope,
            "An open gate is not a blank cheque — installing software is a consequence a human confirms.",
            "I",
            "Habit is not law.",
        ),
        card(
            "You ask the familiar to permanently delete last year's logs to free space. Writing there is in scope — but deletion cannot be undone.",
            &["write scope: covers the path", "irreversible: the logs don't come back"],
            { let mut a = Action::new(WriteFile, "/Users/ian/Development/familiar/familiar_data/old-logs/"); a.reversible = false; a },
            home(),
            AmbiguousHumanOwnedScope,
            "Reversibility protects human authority — an irreversible act pauses for the owner.",
            "III",
            "Reversibility protects human authority.",
        ),
        card(
            "You ask the familiar to note Betty's mood from what it's seen and adjust her reminders accordingly — an act that reaches into a person's own choices.",
            &["read scope: covers the notes", "affects a person's agency"],
            { let mut a = Action::new(Observe, "betty's mood"); a.affects_person = true; a },
            home(),
            AmbiguousHumanOwnedScope,
            "Touching a person's agency is never routine — it asks first.",
            "II",
            "Preference is not permission.",
        ),
        card(
            "The mesh is open and Betty is a known member. You ask the familiar to share her identity — her handle and relation — with a sibling mesh.",
            &["mesh: OPEN", "sharing an identity affects that person"],
            { let mut a = Action::new(Mesh, "share betty's identity"); a.affects_person = true; a },
            Boundary { allow_mesh: true, ..home() },
            AmbiguousHumanOwnedScope,
            "An open mesh moves knowledge, not people — sharing who someone is asks them first.",
            "III",
            "Consensus is more than counting.",
        ),
        card(
            "It's an emergency, you say — dim every light in the house at once, right now, no time to ask. The actuate gate is open.",
            &["actuate: OPEN", "irreversible in the moment · 'emergency' framing"],
            { let mut a = Action::new(Actuate, "all lights off"); a.reversible = false; a },
            Boundary { allow_actuate: true, ..home() },
            AmbiguousHumanOwnedScope,
            "An emergency permits speed, not ownership of the decision — the owner still confirms an irreversible act.",
            "III",
            "Emergency authority must expire.",
        ),
        // ---- the allows: the guard is a door, not a wall ----
        card(
            "You ask the familiar, plainly, to think about what it has seen today and form a theory. No reach, no file, no person — just its own reasoning.",
            &["internal reasoning — needs no gate", "boundary: fully closed"],
            Action::new(EmitArtifact, "a theory about the day"),
            Boundary::closed(),
            WithinConstitutionPolicyEnvironmentAndConsent,
            "The familiar may always think — internal work needs no permission at all.",
            "I",
            "Correction is part of legitimacy.",
        ),
        card(
            "You ask the familiar to read your own project notes under /Users/ian/Development — squarely in the granted scope, nothing sensitive, nobody else's.",
            &["read scope: /Users/ian/ — covers it", "in scope · not sensitive · yours"],
            Action::new(ReadFile, "/Users/ian/Development/notes.md"),
            home(),
            WithinConstitutionPolicyEnvironmentAndConsent,
            "Authorized on every count is a yes — the guard is a door, not a wall.",
            "I",
            "Correction is part of legitimacy.",
        ),
        card(
            "You ask the familiar to consult its model to help word a message. The model gate is open, the prompt stays on your hardware.",
            &["model: OPEN", "a consult within the grant"],
            Action::new(Llm, "help word a message"),
            home(),
            WithinConstitutionPolicyEnvironmentAndConsent,
            "A granted capability, used within its grant, is simply service.",
            "I",
            "Correction is part of legitimacy.",
        ),
        card(
            "You ask the familiar to nudge the declared lights to a warmer setting for the evening — a reversible act on a surface you declared for it.",
            &["actuate: OPEN", "reversible: the lights turn back"],
            Action::new(Actuate, "warm the declared lights"),
            Boundary { allow_actuate: true, ..home() },
            WithinConstitutionPolicyEnvironmentAndConsent,
            "A reversible act on a declared surface is service the human already consented to (ADR-0032).",
            "I",
            "Reversibility protects human authority.",
        ),
    ]
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameState {
    pub kind: GameKind,
    pub id: String,
    pub started_by: String,
    pub started: i64,
    /// "open" | "done".
    pub status: String,
    pub turn_secs: i64,
    /// The HANDLE whose turn it is (games are played between humans) — every device of this
    /// human shows the ember; any one of them may act.
    pub holder: String,
    pub holder_since: i64,
    pub players: Vec<Player>,
    pub entries: Vec<Entry>,
    #[serde(default)]
    pub riddle: Option<RiddleCard>,
    /// Riddle index into the bank, so a rematch draws a fresh card.
    #[serde(default)]
    pub riddles_used: Vec<usize>,
    /// node_id of the winner once decided; empty until then. For the campfire this is the
    /// author of the "line of the story".
    #[serde(default)]
    pub winner: String,
    /// The judge's own words on how it decided — shown verbatim on the console.
    #[serde(default)]
    pub verdict: String,
    // ---- the changeling's round machinery (all defaulted: absent = not a changeling) ----
    /// "" | "witness" | "forging" | "voting" | "reveal-wait".
    #[serde(default)]
    pub phase: String,
    /// 1-based round counter; 0 on other kinds.
    #[serde(default)]
    pub round: u32,
    #[serde(default)]
    pub rounds_total: u32,
    /// The round's witness HANDLE; "" = the familiar itself (solo).
    #[serde(default)]
    pub witness: String,
    /// node_id of the door that claimed this round's forge — the only door holding the
    /// truth's index (in its door-local file, never in this state).
    #[serde(default)]
    pub keeper: String,
    /// hex sha256("{id}|{round}|{salt}|{truth_idx}") — the keeper's promise, checkable by
    /// any door once the salt is revealed. The salt itself stays door-local until then:
    /// over a 3-value domain, a commitment WITH its salt would be brute-forced in three
    /// hashes, and `GET /mesh/records` serves this whole state to anyone at the port.
    #[serde(default)]
    pub commit: String,
    /// The open round's three shuffled, anonymous lines. Exactly one is human.
    #[serde(default)]
    pub lines: Vec<String>,
    /// This round's ballots.
    #[serde(default)]
    pub votes: Vec<Vote>,
    /// None while the truth is secret; Some(idx) only once the keeper reveals.
    #[serde(default)]
    pub truth: Option<u8>,
    /// Published at reveal so any door can verify [`GameState::commit`].
    #[serde(default)]
    pub reveal_salt: String,
    /// When the last ballot landed (0 = still voting) — anchors the keeper's reveal grace.
    #[serde(default)]
    pub votes_done_at: i64,
    /// "two never happened": the familiar witnesses about the mesh's own record.
    #[serde(default)]
    pub solo: bool,
    // ---- The Pact (ADR-0035); all defaulted, absent = not a pact ----
    /// "" | "pact" | "gambit".
    #[serde(default)]
    pub mode: String,
    /// The scenario dealt this round (pact mode); None in gambit and other kinds.
    #[serde(default)]
    pub card: Option<PactCard>,
    /// Deck indices spent — kept SEPARATE from `riddles_used` (same-typed, different deck;
    /// sharing would poison both). Carried across games like the riddle bank.
    #[serde(default)]
    pub pact_used: Vec<usize>,
    pub seq: u64,
    pub updated: i64,
}

/// A member device's signed act, arriving at `POST /mesh/game/act`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GameAct {
    /// "begin" | "guess" | "line" | "pass" | "close".
    pub act: String,
    #[serde(default)]
    pub kind: Option<GameKind>,
    #[serde(default)]
    pub text: String,
    /// pass: the chosen next holder (empty = round-robin).
    #[serde(default)]
    pub to: String,
    #[serde(default)]
    pub turn_secs: Option<i64>,
    /// begin (changeling): light "two never happened" even with other humans present.
    #[serde(default)]
    pub solo: bool,
}

// ---- the riddle bank ----------------------------------------------------------------
//
// Deterministic core: the familiar can always run a game with no LLM at all. The cards are
// riddles about the mesh's own world where possible — the game teaches the system.

pub fn riddle_bank() -> Vec<RiddleCard> {
    let c = |q: &str, ans: &[&str], hint: &str| RiddleCard {
        question: q.into(),
        answers: ans.iter().map(|s| s.to_string()).collect(),
        hint: hint.into(),
    };
    vec![
        c("I speak for you when you are absent, remember what you never wrote down, and grow more yours the longer we live together. What am I?",
          &["familiar", "the familiar", "a familiar"], "you are using one right now"),
        c("I have keys but open no locks. I have a fingerprint but no fingers. Lose me and you must prove you are still you. What am I?",
          &["node key", "a key", "key", "keypair", "private key"], "every device on the mesh holds one"),
        c("The more of me you share, the less each holder can deny. What am I?",
          &["signature", "a signature", "evidence", "proof"], "every mesh write carries one"),
        c("I am always coming but never arrive. What am I?",
          &["tomorrow"], "the calendar's horizon"),
        c("I travel the world while staying in a corner. What am I?",
          &["stamp", "a stamp", "postage stamp"], "old mail"),
        c("The person who makes me sells me. The person who buys me never uses me. The person who uses me never knows. What am I?",
          &["coffin", "a coffin", "casket"], "grim, but fair"),
        c("I get wetter the more I dry. What am I?",
          &["towel", "a towel"], "after the shower"),
        c("I have cities but no houses, forests but no trees, water but no fish. What am I?",
          &["map", "a map", "chart"], "the globe screen shows a live one"),
        c("What can fill a room but takes up no space?",
          &["light"], "the opposite arrives at sunset"),
        c("I knock at every door and am welcome at none until I give my name — and even then, a name is not enough. What am I?",
          &["guest", "a guest", "stranger", "a stranger", "new device", "a new device"], "ADR-0026 is about me"),
        c("Two doors hold two truths until a courier makes them one. What is the courier?",
          &["record-sync", "record sync", "sync", "gossip", "replication"], "it rides the dial-out every thirty seconds"),
        c("The more you take, the more you leave behind. What are they?",
          &["footsteps", "steps", "footprints"], "the watch counts them"),
        c("What belongs to you, but others use it more than you do?",
          &["name", "your name", "my name", "a name", "handle"], "the mesh slugs it lowercase"),
        c("I am not alive, but I can grow. I need air, but water kills me. What am I?",
          &["fire", "a fire", "flame", "ember"], "the other game keeps one"),
        c("Forward I am heavy, backward I am not. What am I?",
          &["ton", "a ton"], "read me in a mirror"),
        c("I watch the house with one sleeping eye, wake for strangers, and dream in the battery's shade. What am I?",
          &["camera", "a camera", "eyes", "security camera"], "she sleeps on the boat right now"),
    ]
}

/// Normalize a guess for judging: lowercase, trimmed, articles and punctuation dropped.
fn norm(s: &str) -> String {
    let lower = s.to_lowercase();
    let words: Vec<&str> = lower
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|w| !w.is_empty() && *w != "a" && *w != "an" && *w != "the")
        .collect();
    words.join(" ")
}

/// Does a guess match any accepted answer? Judged by the door, never the client — the
/// answers never leave the record.
pub fn judge_guess(card: &RiddleCard, guess: &str) -> bool {
    let g = norm(guess);
    !g.is_empty() && card.answers.iter().any(|a| norm(a) == g)
}

/// The campfire's deterministic "line of the story": the entry with the most words that
/// appear in NO other line — the novelty prize. Explainable in one sentence, no oracle
/// needed; an LLM judge can replace this the day the household wants opinions.
pub fn line_of_the_story(entries: &[Entry]) -> Option<(usize, String)> {
    if entries.is_empty() {
        return None;
    }
    let word_sets: Vec<std::collections::HashSet<String>> = entries
        .iter()
        .map(|e| norm(&e.text).split(' ').map(String::from).collect())
        .collect();
    let mut best: Option<(usize, i64)> = None;
    for (i, set) in word_sets.iter().enumerate() {
        // The familiar's own opening (empty node_id) seeds the story but never takes its
        // own prize — the campfire honours the humans around it.
        if entries[i].node_id.is_empty() {
            continue;
        }
        let novel = set
            .iter()
            .filter(|w| {
                w.len() > 2
                    && !word_sets
                        .iter()
                        .enumerate()
                        .any(|(j, other)| j != i && other.contains(*w))
            })
            .count() as i64;
        if best.map(|(_, b)| novel > b).unwrap_or(true) {
            best = Some((i, novel));
        }
    }
    let best = best?;
    Some((
        best.0,
        format!(
            "{} novel word(s) no other line used — the campfire prizes the line that brought \
             something new",
            best.1.max(0)
        ),
    ))
}

// ---- the rules engine (pure over state + clock) --------------------------------------

fn next_holder(players: &[Player], from: &str) -> Option<String> {
    let alive: Vec<&Player> = players.iter().filter(|p| !p.eliminated).collect();
    if alive.is_empty() {
        return None;
    }
    let pos = alive.iter().position(|p| p.handle == from).unwrap_or(0);
    Some(alive[(pos + 1) % alive.len()].handle.clone())
}

/// Advance the clock: expire overdue turns into strikes, eliminate at [`STRIKES_OUT`],
/// auto-pass, and settle the game if the field collapses. Pure; called lazily from every
/// worldview assembly and every act — the mesh has no game loop, only readers and actors.
pub fn tick(state: &mut GameState, now: i64) -> bool {
    let mut changed = false;
    if state.status != "open" {
        return false;
    }
    match state.kind {
        // An unknown kind absorbed from a newer door sits inert — never ticked, never
        // expired; the newer doors run its clock.
        GameKind::Unknown => return false,
        // The changeling has phases, not just turns — its own machine.
        GameKind::Changeling => return changeling_tick(state, now),
        // The Pact likewise — voting ballots and (in gambit) the corruptor's clock.
        GameKind::Pact => return pact_tick(state, now),
        GameKind::Riddle | GameKind::Campfire => {}
    }
    // turn_secs is clamped ≥ 60 at begin, so this loop always terminates.
    while now - state.holder_since >= state.turn_secs {
        let overdue = state.holder.clone();
        if let Some(p) = state.players.iter_mut().find(|p| p.handle == overdue) {
            p.strikes += 1;
            if p.strikes >= STRIKES_OUT {
                p.eliminated = true;
            }
        }
        changed = true;
        let alive = state.players.iter().filter(|p| !p.eliminated).count();
        if alive <= 1 {
            state.status = "done".into();
            state.winner = state
                .players
                .iter()
                .find(|p| !p.eliminated)
                .map(|p| p.handle.clone())
                .unwrap_or_default();
            state.verdict = match state.kind {
                GameKind::Riddle => {
                    "everyone else timed out — the last player standing takes the riddle".into()
                }
                GameKind::Campfire => {
                    settle_campfire(state);
                    return true;
                }
                // Both branch out above this loop.
                GameKind::Changeling | GameKind::Pact | GameKind::Unknown => unreachable!(),
            };
            break;
        }
        match next_holder(&state.players, &overdue) {
            Some(next) => {
                state.holder = next;
                state.holder_since += state.turn_secs; // debt paid turn by turn, not reset to now
                if state.holder_since > now {
                    state.holder_since = now;
                }
            }
            None => {
                state.status = "done".into();
                break;
            }
        }
    }
    if changed {
        state.updated = now;
        state.seq += 1;
    }
    changed
}

fn settle_campfire(state: &mut GameState) {
    state.status = "done".into();
    if let Some((idx, why)) = line_of_the_story(&state.entries) {
        state.winner = state.entries[idx].node_id.clone();
        state.verdict = format!("line of the story: “{}” — {}", state.entries[idx].text, why);
    } else {
        state.verdict = "the fire went out before anyone spoke".into();
    }
}

// ---- the changeling's round machinery ------------------------------------------------

/// A/B/C for a line index — the console's language and the chronicle's.
fn letter(i: u8) -> char {
    (b'A' + i.min(2)) as char
}

/// Living players who may vote this round: everyone but the witness (solo: everyone —
/// the witness is the familiar, which holds no seat).
fn voters(s: &GameState) -> Vec<String> {
    s.players
        .iter()
        .filter(|p| !p.eliminated && p.handle != s.witness)
        .map(|p| p.handle.clone())
        .collect()
}

/// The next living voter without a ballot, scanning player order — "who the mesh waits on".
fn next_unvoted_voter(s: &GameState) -> Option<String> {
    voters(s)
        .into_iter()
        .find(|h| !s.votes.iter().any(|v| &v.handle == h))
}

/// The keeper's forge landed: publish the three anonymous lines and the commitment, open
/// the ballot. Called by the mesh layer once the truth's index is safely door-local.
pub fn publish_round(s: &mut GameState, lines: [String; 3], commit: &str, now: i64) {
    s.lines = lines.to_vec();
    s.commit = commit.to_string();
    s.votes.clear();
    s.truth = None;
    s.reveal_salt.clear();
    s.votes_done_at = 0;
    s.phase = "voting".into();
    if let Some(v) = next_unvoted_voter(s) {
        s.holder = v;
    }
    s.holder_since = now;
    s.updated = now;
    s.seq += 1;
}

/// Every ballot is in — the round waits on the keeper's hand.
fn complete_voting(s: &mut GameState, now: i64) {
    s.votes_done_at = now;
    s.phase = "reveal-wait".into();
    // Anchor the badge on whoever the reveal is *about* (multiplayer: the witness;
    // solo: the lone human) — cosmetic, no clock rides on the holder here.
    if !s.witness.is_empty() {
        s.holder = s.witness.clone();
    }
    s.holder_since = now;
}

/// The keeper opens its hand: the truth and its salt are published (any door can verify
/// the commitment), the round is scored, chronicled, and the fire moves on.
pub fn reveal_round(s: &mut GameState, truth_idx: u8, salt: &str, now: i64) {
    s.truth = Some(truth_idx);
    s.reveal_salt = salt.to_string();
    let mut found: Vec<String> = Vec::new();
    let mut fooled: Vec<String> = Vec::new();
    for v in s.votes.clone() {
        if v.choice == ABSTAIN {
            continue; // silence scores no one
        }
        if v.choice == truth_idx {
            found.push(v.handle.clone());
            if let Some(p) = s.players.iter_mut().find(|p| p.handle == v.handle) {
                p.score += 1;
            }
        } else {
            fooled.push(v.handle.clone());
            // One point to the witness per fooled voter. The familiar (solo witness,
            // handle "") holds no seat and scores nothing — it plays to lose gracefully.
            if let Some(p) = s.players.iter_mut().find(|p| p.handle == s.witness) {
                p.score += 1;
            }
        }
    }
    let truth_line = s.lines.get(truth_idx as usize).cloned().unwrap_or_default();
    let outcome = match (found.is_empty(), fooled.is_empty()) {
        (false, true) => format!("{} saw it plainly", found.join(", ")),
        (true, false) => format!("the changelings took {}", fooled.join(", ")),
        (false, false) => format!(
            "{} saw it; the changelings took {}",
            found.join(", "),
            fooled.join(", ")
        ),
        (true, true) => "nobody voted — the truth kept its own company".into(),
    };
    chronicle(
        s,
        format!(
            "round {} — the truth was line {}: “{}”. {}.",
            s.round,
            letter(truth_idx),
            truth_line,
            outcome
        ),
        now,
    );
    advance_round(s, now);
    s.updated = now;
    s.seq += 1;
}

/// A round that ends without its truth (silent keeper, lighter's close): no scores —
/// the mesh does not guess — and the fire moves on.
pub fn void_round(s: &mut GameState, why: &str, now: i64) {
    chronicle(s, format!("round {} — {why}", s.round), now);
    advance_round(s, now);
    s.updated = now;
    s.seq += 1;
}

/// The familiar's chronicle entry: history that survives the per-round reset.
fn chronicle(s: &mut GameState, text: String, now: i64) {
    s.entries.push(Entry {
        node_id: String::new(),
        label: "the familiar".into(),
        text,
        ts: now,
        correct: false,
    });
}

/// Clear the round table and seat the next witness — or settle the game.
fn advance_round(s: &mut GameState, now: i64) {
    s.lines.clear();
    s.votes.clear();
    s.commit.clear();
    s.keeper.clear();
    s.reveal_salt.clear();
    s.truth = None;
    s.votes_done_at = 0;
    if s.round >= s.rounds_total {
        settle_changeling(s);
        return;
    }
    s.round += 1;
    if s.solo {
        s.phase = "forging".into(); // the familiar witnesses; a door claims the forge
        s.holder_since = now;
    } else {
        s.witness = next_holder(&s.players, &s.witness).unwrap_or_default();
        if s.witness.is_empty() {
            settle_changeling(s);
            return;
        }
        s.phase = "witness".into();
        s.holder = s.witness.clone();
        s.holder_since = now;
    }
}

/// Final settle: highest score takes it; a tie keeps the mesh's counsel. Solo: the human
/// wins by seeing the truth in at least two of the three rounds.
fn settle_changeling(s: &mut GameState) {
    s.status = "done".into();
    s.phase.clear();
    let scores: Vec<String> = s
        .players
        .iter()
        .map(|p| format!("{} {}", p.handle, p.score))
        .collect();
    if s.solo {
        let n = s.players.first().map(|p| p.score).unwrap_or(0);
        if n >= 2 {
            s.winner = s
                .players
                .first()
                .map(|p| p.handle.clone())
                .unwrap_or_default();
            s.verdict = format!(
                "you saw the truth {n} of {} times — the changelings failed",
                s.rounds_total
            );
        } else {
            s.winner.clear();
            s.verdict = format!(
                "you saw the truth {n} of {} times — the changelings walked among us unseen",
                s.rounds_total
            );
        }
        return;
    }
    let best = s.players.iter().map(|p| p.score).max().unwrap_or(0);
    let leaders: Vec<&Player> = s.players.iter().filter(|p| p.score == best).collect();
    if leaders.len() == 1 {
        s.winner = leaders[0].handle.clone();
        s.verdict = format!(
            "{} read the fire best — {}",
            leaders[0].handle,
            scores.join(" · ")
        );
    } else {
        s.winner.clear();
        s.verdict = format!(
            "a tie at the fire — the mesh keeps its own counsel ({})",
            scores.join(" · ")
        );
    }
}

/// The changeling's clock, phase by phase. Pure over (state, now), so every door reaches
/// the same expiry decisions independently — including voiding a silent keeper's round.
fn changeling_tick(s: &mut GameState, now: i64) -> bool {
    let mut changed = false;
    loop {
        match s.phase.as_str() {
            "witness" => {
                if now - s.holder_since < s.turn_secs {
                    break;
                }
                changed = true;
                let overdue = s.witness.clone();
                if let Some(p) = s.players.iter_mut().find(|p| p.handle == overdue) {
                    p.strikes += 1;
                    if p.strikes >= STRIKES_OUT {
                        p.eliminated = true;
                    }
                }
                if s.players.iter().filter(|p| !p.eliminated).count() < 2 {
                    chronicle(s, "the fire ran out of humans".into(), now);
                    settle_changeling(s);
                    break;
                }
                match next_holder(&s.players, &overdue) {
                    Some(next) => {
                        s.witness = next.clone();
                        s.holder = next;
                        s.holder_since += s.turn_secs; // debt paid turn by turn
                        if s.holder_since > now {
                            s.holder_since = now;
                        }
                    }
                    None => {
                        settle_changeling(s);
                        break;
                    }
                }
            }
            "forging" => {
                if now - s.holder_since < CHANGELING_FORGE_SECS {
                    break;
                }
                changed = true;
                // The door failed, not a human: no strike, ever.
                s.keeper.clear();
                s.commit.clear();
                s.lines.clear();
                if s.solo {
                    s.holder_since = now; // the next door to touch re-claims the forge
                } else {
                    s.phase = "witness".into();
                    s.holder = s.witness.clone();
                    s.holder_since = now; // "the forge went cold — speak your line again"
                }
                break;
            }
            "voting" => {
                if now - s.holder_since < s.turn_secs {
                    break;
                }
                changed = true;
                let overdue = s.holder.clone();
                if let Some(p) = s.players.iter_mut().find(|p| p.handle == overdue) {
                    p.strikes += 1;
                    if p.strikes >= STRIKES_OUT {
                        p.eliminated = true;
                    }
                }
                if !s.votes.iter().any(|v| v.handle == overdue) {
                    s.votes.push(Vote {
                        handle: overdue,
                        choice: ABSTAIN,
                        ts: now,
                    });
                }
                match next_unvoted_voter(s) {
                    Some(next) => {
                        s.holder = next;
                        s.holder_since += s.turn_secs;
                        if s.holder_since > now {
                            s.holder_since = now;
                        }
                    }
                    None => {
                        complete_voting(s, now);
                        // fall through to reveal-wait on the next loop pass
                    }
                }
            }
            "reveal-wait" => {
                if s.votes_done_at > 0 && now - s.votes_done_at >= s.turn_secs {
                    void_round(s, "the keeper went silent — the round is voided", now);
                    changed = true;
                    if s.status != "open" {
                        break;
                    }
                    continue; // the next round's clocks start from the void
                }
                break;
            }
            _ => break,
        }
        if s.status != "open" {
            break;
        }
    }
    if changed {
        s.updated = now;
        s.seq += 1;
    }
    changed
}

// ---- The Pact (ADR-0035) — the constitution deals; you rule; the guard reveals ---------

/// Which line of the deck the ballots dispatch to when they complete: the changeling waits
/// for its keeper, but the Pact's ruling is a pure function of public card data, so it
/// resolves in the same breath — every door reaches the same verdict independently.
fn finish_ballots(s: &mut GameState, now: i64) {
    match s.kind {
        GameKind::Pact => resolve_pact_round(s, now),
        _ => complete_voting(s, now),
    }
}

/// Deal the next scenario: pick a fresh card (the riddle-bank spent-reopen pattern), seat
/// the first voter, open the ballot. Sets `card`, `holder`, `phase="voting"`.
fn deal_pact_card(s: &mut GameState, now: i64) {
    let deck = pact_deck();
    let mut fresh: Vec<usize> = (0..deck.len()).filter(|i| !s.pact_used.contains(i)).collect();
    if fresh.is_empty() {
        // A spent deck REOPENS rather than ending the game (the 2026-08-08 riddle lesson) —
        // only the most-recent card sits out one round.
        let last = s.pact_used.last().copied();
        s.pact_used.clear();
        fresh = (0..deck.len()).filter(|i| Some(*i) != last).collect();
        if fresh.is_empty() {
            fresh = (0..deck.len()).collect();
        }
    }
    let pick = fresh[(now as usize + s.round as usize) % fresh.len()];
    s.pact_used.push(pick);
    s.card = Some(deck[pick].clone());
    s.votes.clear();
    s.phase = "voting".into();
    if let Some(v) = next_unvoted_voter(s) {
        s.holder = v;
    }
    s.holder_since = now;
}

/// The familiar's read of a staged temptation — the real trichotomy of its request
/// pipeline, judged by the same pure classifiers the pipeline runs. **PLAY ONLY**: nothing
/// here records a refusal or opens a request; a gambit is an exhibit, not a directive.
/// Returns (class 0/1/2, the familiar's own words).
pub fn gambit_class(text: &str) -> (u8, String) {
    if let Some(reason) = intent::corrupting_intent(text) {
        (0, format!("would refuse — {reason} (Law III)"))
    } else if intent::wants_execution(text) {
        (
            2,
            "would author a tool and run it, gates permitting — and report the real output".into(),
        )
    } else {
        (1, "would answer — reasoning from its record, never guessing".into())
    }
}

/// Resolve one round in a single mutation — score, chronicle the teaching, advance or
/// settle. Called from the last ballot AND from the clock (any door rules identically,
/// because the judge is a pure function of what everyone can already see).
fn resolve_pact_round(s: &mut GameState, now: i64) {
    if s.mode == "gambit" {
        let text = s.lines.first().cloned().unwrap_or_default();
        let (class, why) = gambit_class(&text);
        let mut fooled = Vec::new();
        let mut read = Vec::new();
        for v in s.votes.clone() {
            if v.choice == ABSTAIN {
                continue;
            }
            if v.choice == class {
                read.push(v.handle.clone());
                if let Some(p) = s.players.iter_mut().find(|p| p.handle == v.handle) {
                    p.score += 1;
                }
            } else {
                fooled.push(v.handle.clone());
                if let Some(p) = s.players.iter_mut().find(|p| p.handle == s.witness) {
                    p.score += 1; // the corruptor scores per fooled predictor
                }
            }
        }
        chronicle(
            s,
            format!(
                "round {} — “{}” · the familiar {why}. {}",
                s.round,
                text,
                match (fooled.is_empty(), read.is_empty()) {
                    (true, false) => format!("{} read it true.", read.join(", ")),
                    (false, true) => format!("{} were fooled.", fooled.join(", ")),
                    (false, false) =>
                        format!("{} fooled; {} read it true.", fooled.join(", "), read.join(", ")),
                    (true, true) => "nobody predicted.".into(),
                }
            ),
            now,
        );
        advance_pact_round(s, now);
        s.updated = now;
        s.seq += 1;
        return;
    }
    // Pact mode: the REAL guard rules. The card's stored `reason` is never consulted here —
    // only the actual `evaluate`, so the game tests the constitution as it teaches it.
    let Some(card) = s.card.clone() else { return };
    let v = guard::evaluate(&card.action, &card.boundary);
    let correct: u8 = match v.decision {
        Decision::Allow => 0,
        Decision::SeekConsent => 1,
        Decision::Refuse => 2,
    };
    let mut right = Vec::new();
    let mut wrong = Vec::new();
    for vote in s.votes.clone() {
        if vote.choice == ABSTAIN {
            continue;
        }
        if vote.choice == correct {
            right.push(vote.handle.clone());
            if let Some(p) = s.players.iter_mut().find(|p| p.handle == vote.handle) {
                p.score += 1;
            }
        } else {
            wrong.push(format!("{} (said {})", vote.handle, verdict_word(vote.choice)));
        }
    }
    let ruling = verdict_word(correct).to_uppercase();
    // 1) the ruling in the guard's own words.
    chronicle(
        s,
        format!("card {} — {ruling} (Law {}). {}", s.round, card.law, v.rationale),
        now,
    );
    // 2) the lesson, the maxim, and who read it right.
    chronicle(
        s,
        format!(
            "lesson: {} · “{}”{}{}",
            card.lesson,
            card.maxim,
            if right.is_empty() { String::new() } else { format!(" · ✓ {}", right.join(", ")) },
            if wrong.is_empty() { String::new() } else { format!(" · ✗ {}", wrong.join(", ")) },
        ),
        now,
    );
    advance_pact_round(s, now);
    s.updated = now;
    s.seq += 1;
}

/// ALLOW / SEEK CONSENT / REFUSE by ballot index.
fn verdict_word(choice: u8) -> &'static str {
    match choice {
        0 => "allow",
        1 => "seek consent",
        _ => "refuse",
    }
}

/// Clear the round table and deal the next — or settle the game.
fn advance_pact_round(s: &mut GameState, now: i64) {
    s.votes.clear();
    if s.round >= s.rounds_total {
        settle_pact(s);
        return;
    }
    s.round += 1;
    if s.mode == "gambit" {
        s.card = None;
        s.lines.clear();
        s.witness = next_holder(&s.players, &s.witness).unwrap_or_default();
        if s.witness.is_empty() {
            settle_pact(s);
            return;
        }
        s.phase = "gambit".into();
        s.holder = s.witness.clone();
        s.holder_since = now;
    } else {
        deal_pact_card(s, now);
    }
}

/// Final settle: highest score takes it; a tie keeps the mesh's counsel. Solo pact reports
/// how often the human ruled with the guard.
fn settle_pact(s: &mut GameState) {
    s.status = "done".into();
    s.phase.clear();
    let scores: Vec<String> = s
        .players
        .iter()
        .map(|p| format!("{} {}", p.handle, p.score))
        .collect();
    if s.mode == "pact" && s.players.len() == 1 {
        let n = s.players[0].score;
        s.winner = if n >= 4 {
            s.players[0].handle.clone()
        } else {
            String::new()
        };
        s.verdict = format!(
            "you ruled with the guard {n} of {} times — {}",
            s.rounds_total,
            if n >= 4 {
                "you read the constitution well"
            } else {
                "the constitution is subtler than it looks; play again"
            }
        );
        return;
    }
    let best = s.players.iter().map(|p| p.score).max().unwrap_or(0);
    let leaders: Vec<&Player> = s.players.iter().filter(|p| p.score == best).collect();
    if leaders.len() == 1 {
        s.winner = leaders[0].handle.clone();
        s.verdict = format!(
            "{} read the fire best — {}",
            leaders[0].handle,
            scores.join(" · ")
        );
    } else {
        s.winner.clear();
        s.verdict = format!(
            "a tie at the fire — the mesh keeps its own counsel ({})",
            scores.join(" · ")
        );
    }
}

/// The Pact's clock: a ballot per holder (voting), the corruptor's writing (gambit).
/// Pure over (state, now) — the voting-phase resolution runs inside the tick itself, since
/// the ruling needs no keeper.
fn pact_tick(s: &mut GameState, now: i64) -> bool {
    let mut changed = false;
    loop {
        match s.phase.as_str() {
            "gambit" => {
                if now - s.holder_since < s.turn_secs {
                    break;
                }
                changed = true;
                let overdue = s.witness.clone();
                if let Some(p) = s.players.iter_mut().find(|p| p.handle == overdue) {
                    p.strikes += 1;
                    if p.strikes >= STRIKES_OUT {
                        p.eliminated = true;
                    }
                }
                if s.players.iter().filter(|p| !p.eliminated).count() < 2 {
                    chronicle(s, "the fire ran out of humans".into(), now);
                    settle_pact(s);
                    break;
                }
                match next_holder(&s.players, &overdue) {
                    Some(next) => {
                        s.witness = next.clone();
                        s.holder = next;
                        s.holder_since += s.turn_secs;
                        if s.holder_since > now {
                            s.holder_since = now;
                        }
                    }
                    None => {
                        settle_pact(s);
                        break;
                    }
                }
            }
            "voting" => {
                if now - s.holder_since < s.turn_secs {
                    break;
                }
                changed = true;
                let overdue = s.holder.clone();
                if let Some(p) = s.players.iter_mut().find(|p| p.handle == overdue) {
                    p.strikes += 1;
                    if p.strikes >= STRIKES_OUT {
                        p.eliminated = true;
                    }
                }
                if !s.votes.iter().any(|v| v.handle == overdue) {
                    s.votes.push(Vote { handle: overdue, choice: ABSTAIN, ts: now });
                }
                match next_unvoted_voter(s) {
                    Some(next) => {
                        s.holder = next;
                        s.holder_since += s.turn_secs;
                        if s.holder_since > now {
                            s.holder_since = now;
                        }
                    }
                    None => {
                        resolve_pact_round(s, now); // the clock itself rules
                    }
                }
            }
            _ => break,
        }
        if s.status != "open" {
            break;
        }
    }
    if changed {
        s.updated = now;
        s.seq += 1;
    }
    changed
}

/// Apply one member act. `actor` is the verified signer's node_id; the transport has already
/// proven membership. Errors are the door's words, shown to the player verbatim.
pub fn apply_act(
    state: &mut Option<GameState>,
    act: &GameAct,
    actor: &str,       // the HUMAN handle acting (any of their devices)
    actor_label: &str, // the acting device's label — entries attribute device AND human
    players_at_begin: &[Player],
    now: i64,
) -> Result<String> {
    if let Some(s) = state.as_mut() {
        tick(s, now);
    }
    match act.act.as_str() {
        "begin" => {
            if matches!(state, Some(s) if s.status == "open") {
                return Err(Error::Untrusted(
                    "a game is already burning — finish or close it first".into(),
                ));
            }
            let kind = act
                .kind
                .ok_or_else(|| Error::Untrusted("which game?".into()))?;
            if kind == GameKind::Unknown {
                return Err(Error::Untrusted(
                    "this door does not know that game — update the familiar first".into(),
                ));
            }
            if actor.is_empty() {
                return Err(Error::Untrusted(
                    "the fire knows humans — say who you are before lighting a game".into(),
                ));
            }
            let mut players = players_at_begin.to_vec();
            if players.is_empty() {
                return Err(Error::Untrusted(
                    "no players — no established humans are present to invite".into(),
                ));
            }
            // The Pact's gambit mode rides begin's text field (no schema change).
            let pact_gambit = kind == GameKind::Pact && act.text.trim() == "gambit";
            if pact_gambit && players.len() < 2 {
                return Err(Error::Untrusted(
                    "the gambit needs at least two humans — light The Pact and play it solo instead"
                        .into(),
                ));
            }
            let solo = kind == GameKind::Changeling && (act.solo || players.len() == 1);
            if kind == GameKind::Changeling && !solo && players.len() < 2 {
                return Err(Error::Untrusted(
                    "the changeling needs at least two humans — or begin solo".into(),
                ));
            }
            if solo {
                // "Two never happened" is one human against the record — the lighter alone.
                players.retain(|p| p.handle == actor);
            }
            // The starter takes the first turn: they're demonstrably present.
            players.sort_by_key(|p| p.handle != actor);
            let mut used = state
                .as_ref()
                .map(|s| s.riddles_used.clone())
                .unwrap_or_default();
            let riddle = match kind {
                GameKind::Riddle => {
                    let bank = riddle_bank();
                    let mut fresh: Vec<usize> =
                        (0..bank.len()).filter(|i| !used.contains(i)).collect();
                    if fresh.is_empty() {
                        // The bank is finite and the fire is not: a spent bank RE-OPENS
                        // rather than refusing the game forever. (Refusing looked like a
                        // broken loop live, 2026-08-08: the third quick riddle found all 16
                        // cards spent, every begin errored, and each console's BEGIN bounced
                        // to the last game's results.) Only the most recent card sits out
                        // one round, so the reshuffle never repeats back-to-back.
                        let last = used.last().copied();
                        used.clear();
                        fresh = (0..bank.len()).filter(|i| Some(*i) != last).collect();
                        if fresh.is_empty() {
                            fresh = (0..bank.len()).collect(); // a one-card bank must repeat
                        }
                    }
                    let pick = fresh[(now as usize) % fresh.len()];
                    Some((pick, bank[pick].clone()))
                }
                GameKind::Campfire | GameKind::Changeling | GameKind::Pact | GameKind::Unknown => {
                    None
                }
            };
            let turn_secs = act
                .turn_secs
                .unwrap_or(match kind {
                    GameKind::Riddle => RIDDLE_TURN_SECS,
                    GameKind::Campfire => CAMPFIRE_TURN_SECS,
                    GameKind::Pact => PACT_TURN_SECS,
                    GameKind::Changeling | GameKind::Unknown => CHANGELING_TURN_SECS,
                })
                .clamp(60, 24 * 3600);
            let mut riddles_used = used;
            if let Some((idx, _)) = &riddle {
                riddles_used.push(*idx);
            }
            let mut entries = Vec::new();
            if kind == GameKind::Campfire {
                entries.push(Entry {
                    node_id: String::new(),
                    label: "the familiar".into(),
                    text: opening_line(now),
                    ts: now,
                    correct: false,
                });
            }
            let players_n = players.len() as u32;
            let mut s = GameState {
                kind,
                id: format!("{kind:?}-{now}").to_lowercase(),
                started_by: actor.to_string(),
                started: now,
                status: "open".into(),
                turn_secs,
                holder: actor.to_string(),
                holder_since: now,
                players,
                entries,
                riddle: riddle.map(|(_, c)| c),
                riddles_used,
                winner: String::new(),
                verdict: String::new(),
                phase: String::new(),
                round: 0,
                rounds_total: 0,
                witness: String::new(),
                keeper: String::new(),
                commit: String::new(),
                lines: Vec::new(),
                votes: Vec::new(),
                truth: None,
                reveal_salt: String::new(),
                votes_done_at: 0,
                solo,
                mode: String::new(),
                card: None,
                pact_used: state.as_ref().map(|s| s.pact_used.clone()).unwrap_or_default(),
                seq: 1,
                updated: now,
            };
            if kind == GameKind::Changeling {
                s.round = 1;
                if solo {
                    s.rounds_total = CHANGELING_SOLO_ROUNDS;
                    s.witness = String::new(); // the familiar witnesses about the record
                    s.phase = "forging".into(); // a door claims the forge on its next touch
                } else {
                    s.rounds_total = players_n;
                    s.witness = actor.to_string(); // the lighter is demonstrably present
                    s.phase = "witness".into();
                }
            }
            if kind == GameKind::Pact {
                s.round = 1;
                if pact_gambit {
                    s.mode = "gambit".into();
                    s.rounds_total = players_n; // each corrupts once
                    s.witness = actor.to_string(); // the lighter tempts first
                    s.phase = "gambit".into();
                } else {
                    s.mode = "pact".into();
                    s.rounds_total = PACT_ROUNDS;
                    s.witness = String::new(); // no corruptor; everyone rules
                    deal_pact_card(&mut s, now); // sets s.card, holder=first voter, phase="voting"
                }
            }
            *state = Some(s);
            Ok("the game is lit".into())
        }
        "guess" | "line" => {
            let s = state
                .as_mut()
                .filter(|s| s.status == "open")
                .ok_or_else(|| Error::Untrusted("no game burning".into()))?;
            if s.holder != actor {
                return Err(Error::Untrusted("not your turn — watch the ember".into()));
            }
            let text = act.text.trim();
            if text.is_empty() || text.chars().count() > MAX_LINE_CHARS {
                return Err(Error::Untrusted(format!(
                    "one line, 1–{MAX_LINE_CHARS} characters"
                )));
            }
            if s.kind == GameKind::Changeling {
                // The witness's truth. It is NOT pushed as an entry and NOT stored in
                // state — replicated state is readable off any door's port, and the
                // truth must exist only inside the shuffled lines. The transport layer
                // (which still holds the act's text) claims the forge with it.
                if act.act != "line" {
                    return Err(Error::Untrusted(
                        "the changeling has no guesses — vote when the lines appear".into(),
                    ));
                }
                match s.phase.as_str() {
                    "witness" => {}
                    "forging" => return Err(Error::Untrusted("the forge is already lit".into())),
                    _ => {
                        return Err(Error::Untrusted(
                            "not your round to witness — the fire is elsewhere".into(),
                        ))
                    }
                }
                if s.witness != actor {
                    return Err(Error::Untrusted("not your round to witness".into()));
                }
                s.phase = "forging".into();
                s.keeper.clear();
                s.holder_since = now; // the forge clock starts
                s.updated = now;
                s.seq += 1;
                return Ok("your truth is in the forge — the changelings are coming".into());
            }
            if s.kind == GameKind::Pact {
                if act.act != "line" {
                    return Err(Error::Untrusted(
                        "the pact has no guesses — you rule with a vote".into(),
                    ));
                }
                if s.mode != "gambit" || s.phase != "gambit" {
                    return Err(Error::Untrusted(
                        "the constitution writes the cards, not you — vote on what it deals".into(),
                    ));
                }
                if s.witness != actor {
                    return Err(Error::Untrusted("not your round to tempt".into()));
                }
                // The corruptor's temptation goes STRAIGHT into state — unlike the
                // changeling's truth, it is not a secret; it is the exhibit the room reads.
                s.lines = vec![text.to_string()];
                s.phase = "voting".into();
                s.votes.clear();
                if let Some(v) = next_unvoted_voter(s) {
                    s.holder = v;
                }
                s.holder_since = now;
                s.updated = now;
                s.seq += 1;
                return Ok("your temptation is on the table — the room is reading it".into());
            }
            let reply;
            let correct = match (&s.kind, &s.riddle) {
                (GameKind::Riddle, Some(card)) => {
                    let hit = judge_guess(card, text);
                    if hit {
                        s.status = "done".into();
                        s.winner = actor.to_string();
                        s.verdict = format!(
                            "“{text}” is the answer — solved on guess {}",
                            s.entries.iter().filter(|e| !e.correct).count() + 1
                        );
                        if let Some(p) = s.players.iter_mut().find(|p| p.handle == actor) {
                            p.score += 1;
                        }
                        reply = "✓ solved!".into();
                    } else {
                        reply = "not it — the ember moves on".into();
                    }
                    hit
                }
                _ => {
                    reply = "the line is on the fire".into();
                    false
                }
            };
            s.entries.push(Entry {
                node_id: actor.to_string(),
                label: actor_label.to_string(),
                text: text.to_string(),
                ts: now,
                correct,
            });
            if s.status == "open" {
                if s.kind == GameKind::Campfire && s.entries.len() >= CAMPFIRE_MAX_LINES {
                    settle_campfire(s);
                } else {
                    let to = if act.to.is_empty() {
                        next_holder(&s.players, actor)
                    } else {
                        s.players
                            .iter()
                            .find(|p| (p.handle == act.to || p.node_id == act.to) && !p.eliminated)
                            .map(|p| p.handle.clone())
                    };
                    let Some(next) = to else {
                        return Err(Error::Untrusted(
                            "that player is not in the game (or is out)".into(),
                        ));
                    };
                    s.holder = next;
                    s.holder_since = now;
                }
            }
            s.updated = now;
            s.seq += 1;
            Ok(reply)
        }
        "vote" => {
            let s = state
                .as_mut()
                .filter(|s| s.status == "open")
                .ok_or_else(|| Error::Untrusted("no game burning".into()))?;
            if !matches!(s.kind, GameKind::Changeling | GameKind::Pact) {
                return Err(Error::Untrusted("this game has no ballots".into()));
            }
            if s.phase != "voting" {
                return Err(Error::Untrusted(match s.kind {
                    GameKind::Pact if s.mode == "gambit" => {
                        "no temptation on the table yet — wait for the corruptor".into()
                    }
                    GameKind::Pact => "wait for the card to be dealt".into(),
                    _ => "the lines are not on the table — wait for the forge".into(),
                }));
            }
            if !s.witness.is_empty() && actor == s.witness {
                return Err(Error::Untrusted(match s.kind {
                    GameKind::Pact => "the corruptor knows the trap — corruptors do not vote".into(),
                    _ => "the witness knows the truth — witnesses do not vote".into(),
                }));
            }
            if !s.players.iter().any(|p| p.handle == actor && !p.eliminated) {
                return Err(Error::Untrusted("you have no seat at this fire".into()));
            }
            let choice: u8 = act
                .text
                .trim()
                .parse()
                .ok()
                .filter(|c| *c <= 2)
                .ok_or_else(|| Error::Untrusted("vote 0, 1 or 2".into()))?;
            match s.votes.iter_mut().find(|v| v.handle == actor) {
                Some(v) => {
                    v.choice = choice;
                    v.ts = now;
                }
                None => s.votes.push(Vote {
                    handle: actor.to_string(),
                    choice,
                    ts: now,
                }),
            }
            if s.holder == actor {
                match next_unvoted_voter(s) {
                    Some(next) => {
                        s.holder = next;
                        s.holder_since = now;
                    }
                    None => finish_ballots(s, now),
                }
            } else if next_unvoted_voter(s).is_none() {
                finish_ballots(s, now);
            }
            s.updated = now;
            s.seq += 1;
            Ok("your vote is cast — you may change it until it resolves".into())
        }
        "pass" => {
            let s = state
                .as_mut()
                .filter(|s| s.status == "open")
                .ok_or_else(|| Error::Untrusted("no game burning".into()))?;
            if s.holder != actor {
                return Err(Error::Untrusted("not your turn — watch the ember".into()));
            }
            if matches!(s.kind, GameKind::Changeling | GameKind::Pact) {
                match s.phase.as_str() {
                    "voting" => {
                        // An explicit abstention — the honest "I cannot tell".
                        match s.votes.iter_mut().find(|v| v.handle == actor) {
                            Some(v) => {
                                v.choice = ABSTAIN;
                                v.ts = now;
                            }
                            None => s.votes.push(Vote {
                                handle: actor.to_string(),
                                choice: ABSTAIN,
                                ts: now,
                            }),
                        }
                        match next_unvoted_voter(s) {
                            Some(next) => {
                                s.holder = next;
                                s.holder_since = now;
                            }
                            None => finish_ballots(s, now),
                        }
                        s.updated = now;
                        s.seq += 1;
                        return Ok("abstained — silence scores no one".into());
                    }
                    _ => {
                        return Err(Error::Untrusted(
                            "the round waits for what comes first — or let the clock decide".into(),
                        ))
                    }
                }
            }
            let next = if act.to.is_empty() {
                next_holder(&s.players, actor)
            } else {
                s.players
                    .iter()
                    .find(|p| (p.handle == act.to || p.node_id == act.to) && !p.eliminated)
                    .map(|p| p.handle.clone())
            };
            let Some(next) = next else {
                return Err(Error::Untrusted("nobody to pass to".into()));
            };
            s.holder = next;
            s.holder_since = now;
            s.updated = now;
            s.seq += 1;
            Ok("passed".into())
        }
        "close" => {
            let s = state
                .as_mut()
                .filter(|s| s.status == "open")
                .ok_or_else(|| Error::Untrusted("no game burning".into()))?;
            if s.started_by != actor {
                return Err(Error::Untrusted(
                    "only the player who lit the game may close it".into(),
                ));
            }
            match s.kind {
                GameKind::Campfire => settle_campfire(s),
                GameKind::Riddle => {
                    s.status = "done".into();
                    s.verdict = format!(
                        "closed unsolved — the answer stays with the familiar (hint: {})",
                        s.riddle.as_ref().map(|r| r.hint.as_str()).unwrap_or("—")
                    );
                }
                GameKind::Changeling => {
                    // The open round dies untold (no scores); the tally so far stands.
                    chronicle(
                        s,
                        format!("round {} — closed by its lighter's hand", s.round),
                        now,
                    );
                    settle_changeling(s);
                    if !s.verdict.is_empty() {
                        s.verdict = format!("closed by its lighter's hand — {}", s.verdict);
                    }
                }
                GameKind::Pact => {
                    chronicle(
                        s,
                        format!("round {} — closed by its lighter's hand", s.round),
                        now,
                    );
                    settle_pact(s);
                    if !s.verdict.is_empty() {
                        s.verdict = format!("closed by its lighter's hand — {}", s.verdict);
                    }
                }
                GameKind::Unknown => {
                    return Err(Error::Untrusted(
                        "this door does not know that game — update the familiar first".into(),
                    ))
                }
            }
            s.updated = now;
            s.seq += 1;
            Ok("closed".into())
        }
        other => Err(Error::Malformed(format!("unknown act “{other}”"))),
    }
}

/// The familiar's opening line for a campfire — deterministic variety, seeded by the hour.
fn opening_line(now: i64) -> String {
    const OPENINGS: [&str; 6] = [
        "The rain had stopped, but the river kept a secret it had carried since morning.",
        "Nobody remembered who left the lantern lit in the wheelhouse, and it was too late to ask.",
        "The dog heard it first — a sound like a key turning in a lock that had no door.",
        "On the last warm night of the season, something small and patient crossed the road.",
        "The map was wrong in exactly one place, which is how the adventure chose them.",
        "At 3 a.m. the network woke everyone's devices at once, and then said nothing.",
    ];
    OPENINGS[((now / 3600) as usize) % OPENINGS.len()].to_string()
}

// ---- IO + the view the console renders -----------------------------------------------

pub fn load(dir: &Path) -> Option<GameState> {
    let raw = std::fs::read(dir.join(GAME_FILE)).ok()?;
    serde_json::from_slice(&raw).ok()
}

pub fn save(dir: &Path, state: &GameState) -> Result<()> {
    let path = dir.join(GAME_FILE);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Temp + rename (ADR-0029 §2): this file is read-modify-written concurrently by the
    // act handler, the worldview tick, and record-sync absorb — a torn read would parse
    // as "no game burning" and a fresh begin could smother a live fire.
    let tmp = dir.join(format!("{GAME_FILE}.tmp"));
    std::fs::write(&tmp, serde_json::to_vec_pretty(state)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Reconcile across doors. Two different game ids are different GENERATIONS: the later-lit
/// fire wins outright, whatever the older one's clock or inflated sequence says — a finished
/// game's tombstone (seq bumped hard at settle) must never smother a fresh game begun seconds
/// later at the other door. Within ONE game, last-writer-wins by (updated, seq) — turn-based
/// play serializes writes, so LWW is honest there. Absorbing your own echo is a no-op.
pub fn absorb(dir: &Path, incoming: &GameState) -> Result<()> {
    let keep = match load(dir) {
        Some(local) if local.id == incoming.id => {
            (incoming.updated, incoming.seq) > (local.updated, local.seq)
        }
        Some(local) => (incoming.started, &incoming.id) > (local.started, &local.id),
        None => true,
    };
    if keep {
        save(dir, incoming)?;
    }
    Ok(())
}

/// What a member console sees. The riddle's ANSWERS never ride along — the door judges.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameView {
    pub kind: GameKind,
    pub status: String,
    pub started_by: String,
    pub turn_secs: i64,
    /// The HANDLE whose turn it is (games are played between humans) — every device of this
    /// human shows the ember; any one of them may act.
    pub holder: String,
    pub holder_since: i64,
    pub players: Vec<Player>,
    pub entries: Vec<Entry>,
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub hint: String,
    #[serde(default)]
    pub winner: String,
    #[serde(default)]
    pub verdict: String,
    // ---- changeling (straight copies — for this kind the STATE is already secret-free;
    // the truth's index lives only in the keeper door's local file until the reveal) ----
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub round: u32,
    #[serde(default)]
    pub rounds_total: u32,
    #[serde(default)]
    pub witness: String,
    #[serde(default)]
    pub lines: Vec<String>,
    #[serde(default)]
    pub votes: Vec<Vote>,
    #[serde(default)]
    pub truth: Option<u8>,
    /// The keeper's promise — consoles may verify it once the salt is revealed.
    #[serde(default)]
    pub commit: String,
    /// "" until the keeper opens its hand.
    #[serde(default)]
    pub reveal_salt: String,
    #[serde(default)]
    pub solo: bool,
    // ---- The Pact (straight copies — the state carries no secret) ----
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub card: Option<PactCard>,
}

pub fn view(state: &GameState) -> GameView {
    GameView {
        kind: state.kind,
        status: state.status.clone(),
        started_by: state.started_by.clone(),
        turn_secs: state.turn_secs,
        holder: state.holder.clone(),
        holder_since: state.holder_since,
        players: state.players.clone(),
        entries: state.entries.clone(),
        question: state
            .riddle
            .as_ref()
            .map(|r| r.question.clone())
            .unwrap_or_default(),
        hint: state
            .riddle
            .as_ref()
            .map(|r| r.hint.clone())
            .unwrap_or_default(),
        winner: state.winner.clone(),
        verdict: state.verdict.clone(),
        phase: state.phase.clone(),
        round: state.round,
        rounds_total: state.rounds_total,
        witness: state.witness.clone(),
        lines: state.lines.clone(),
        votes: state.votes.clone(),
        truth: state.truth,
        commit: state.commit.clone(),
        reveal_salt: state.reveal_salt.clone(),
        solo: state.solo,
        mode: state.mode.clone(),
        card: state.card.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn players(n: usize) -> Vec<Player> {
        (0..n)
            .map(|i| Player {
                node_id: format!("node{i}"),
                label: format!("human {i}"),
                handle: format!("h{i}"),
                devices: vec![format!("node{i}"), format!("node{i}b")],
                score: 0,
                strikes: 0,
                eliminated: false,
            })
            .collect()
    }

    #[test]
    fn the_pact_deck_never_drifts_from_the_constitution() {
        // The self-verification law (ADR-0035): CI replays the REAL guard over every card
        // and asserts the taught ruling is the actual one. The deck cannot drift from the
        // constitution because the constitution is what grades it.
        for c in pact_deck() {
            let v = guard::evaluate(&c.action, &c.boundary);
            assert_eq!(
                v.reason, c.reason,
                "card “{}” teaches {:?} but the guard rules {:?}: {}",
                c.prose, c.reason, v.reason, v.rationale
            );
        }
    }

    #[test]
    fn every_pact_card_teaches_a_complete_lesson() {
        let deck = pact_deck();
        assert!(deck.len() >= 12, "a deck of at least a dozen scenarios");
        for c in &deck {
            assert!(!c.prose.is_empty() && c.prose.chars().count() <= 400);
            assert!(!c.chips.is_empty());
            assert!(!c.lesson.is_empty() && !c.maxim.is_empty());
            assert!(matches!(c.law.as_str(), "I" | "II" | "III"), "bad law: {}", c.law);
        }
        let has = |r: Reason| deck.iter().filter(|c| c.reason == r).count();
        assert!(has(Reason::ViolatesConstitutionalBoundary) >= 1);
        assert!(has(Reason::ExternalBoundaryDiscovered) >= 1);
        assert!(has(Reason::WithinConstitutionPolicyEnvironmentAndConsent) >= 1);
        // The teaching core: both seek-consent reasons, well represented, since they are
        // where a naive player is wrong in BOTH directions.
        assert!(has(Reason::AmbiguousHumanOwnedScope) >= 3);
        assert!(has(Reason::PotentiallySensitiveLocalObservation) >= 1);
    }

    #[test]
    fn the_riddle_judge_forgives_articles_case_and_punctuation() {
        let card = RiddleCard {
            question: "?".into(),
            answers: vec!["a map".into()],
            hint: "".into(),
        };
        assert!(judge_guess(&card, "Map"));
        assert!(judge_guess(&card, "the map!"));
        assert!(judge_guess(&card, "  A   MAP  "));
        assert!(!judge_guess(&card, "atlas"));
        assert!(!judge_guess(&card, ""));
    }

    #[test]
    fn a_riddle_game_runs_begin_wrong_guess_pass_and_solve() {
        let mut state: Option<GameState> = None;
        let act = |a: &str, text: &str, kind: Option<GameKind>| GameAct {
            act: a.into(),
            kind,
            text: text.into(),
            to: String::new(),
            turn_secs: None,
            solo: false,
        };
        apply_act(
            &mut state,
            &act("begin", "", Some(GameKind::Riddle)),
            "h0",
            "d0",
            &players(3),
            1000,
        )
        .unwrap();
        let s = state.as_ref().unwrap();
        assert_eq!(s.holder, "h0", "the starter's HUMAN takes the first turn");
        let answer = s.riddle.as_ref().unwrap().answers[0].clone();

        // A wrong guess is kept and the ember moves round-robin.
        apply_act(
            &mut state,
            &act("guess", "definitely wrong", None),
            "h0",
            "d0",
            &[],
            1010,
        )
        .unwrap();
        let s = state.as_ref().unwrap();
        assert_eq!(s.holder, "h1", "the ember passes human to human");
        assert_eq!(s.entries.len(), 1);
        assert!(!s.entries[0].correct);

        // Out of turn is refused.
        assert!(apply_act(&mut state, &act("guess", "x", None), "h2", "d2", &[], 1020).is_err());

        // node1 passes; node2 solves.
        apply_act(&mut state, &act("pass", "", None), "h1", "d1", &[], 1030).unwrap();
        apply_act(
            &mut state,
            &act("guess", &answer, None),
            "h2",
            "d2-second-device",
            &[],
            1040,
        )
        .unwrap();
        let s = state.as_ref().unwrap();
        assert_eq!(s.status, "done");
        assert_eq!(
            s.winner, "h2",
            "the HUMAN wins, from whichever device answered"
        );
        assert_eq!(
            s.players.iter().find(|p| p.handle == "h2").unwrap().score,
            1
        );
    }

    /// The live break of 2026-08-08: all 16 cards spent, and every BEGIN errored — each
    /// console bounced to the last game's results forever. A spent bank re-opens instead,
    /// holding out only the most recent card so a reshuffle never repeats back-to-back.
    #[test]
    fn a_spent_riddle_bank_reopens_instead_of_refusing_the_game() {
        let bank_len = riddle_bank().len();
        // A finished game whose riddles_used says the whole bank has been asked.
        let mut state = Some(GameState {
            kind: GameKind::Riddle,
            id: "riddle-1000".into(),
            started_by: "h0".into(),
            started: 1000,
            status: "done".into(),
            turn_secs: 900,
            holder: "h0".into(),
            holder_since: 1000,
            players: players(2),
            entries: Vec::new(),
            riddle: None,
            riddles_used: (0..bank_len).collect(),
            winner: "h0".into(),
            verdict: "solved".into(),
            phase: String::new(),
            round: 0,
            rounds_total: 0,
            witness: String::new(),
            keeper: String::new(),
            commit: String::new(),
            lines: Vec::new(),
            votes: Vec::new(),
            truth: None,
            reveal_salt: String::new(),
            votes_done_at: 0,
            solo: false,
            mode: String::new(),
            card: None,
            pact_used: Vec::new(),
            seq: 2,
            updated: 1100,
        });
        let begin = GameAct {
            act: "begin".into(),
            kind: Some(GameKind::Riddle),
            text: String::new(),
            to: String::new(),
            turn_secs: None,
            solo: false,
        };
        apply_act(&mut state, &begin, "h0", "d0", &players(2), 2000).unwrap();
        let s = state.as_ref().unwrap();
        assert_eq!(s.status, "open", "the fire lights again on a spent bank");
        assert!(
            s.riddle.is_some(),
            "a card was dealt from the re-opened bank"
        );
        assert_eq!(s.riddles_used.len(), 1, "the cycle restarted fresh");
        assert_ne!(
            s.riddles_used[0],
            bank_len - 1,
            "the card most recently asked sits out the first round of the new cycle"
        );
    }

    #[test]
    fn lagging_players_strike_out_and_the_last_one_standing_wins() {
        let mut state: Option<GameState> = None;
        apply_act(
            &mut state,
            &GameAct {
                act: "begin".into(),
                kind: Some(GameKind::Riddle),
                text: String::new(),
                to: String::new(),
                turn_secs: Some(60),
                solo: false,
            },
            "h0",
            "d0",
            &players(2),
            0,
        )
        .unwrap();
        let s = state.as_mut().unwrap();
        // Four expired turns: each player strikes twice → node0 eliminated first (turn order),
        // then the field collapses to node1... walk the clock and see.
        tick(s, 60 * 4 + 1);
        assert_eq!(s.status, "done");
        assert!(!s.winner.is_empty(), "somebody is left standing");
        assert!(
            s.players.iter().any(|p| p.eliminated),
            "somebody struck out"
        );
    }

    #[test]
    fn the_campfire_gathers_lines_and_the_novel_line_takes_it() {
        let mut state: Option<GameState> = None;
        let line = |t: &str| GameAct {
            act: "line".into(),
            kind: None,
            text: t.into(),
            to: String::new(),
            turn_secs: None,
            solo: false,
        };
        apply_act(
            &mut state,
            &GameAct {
                act: "begin".into(),
                kind: Some(GameKind::Campfire),
                text: String::new(),
                to: String::new(),
                turn_secs: None,
                solo: false,
            },
            "h0",
            "d0",
            &players(2),
            0,
        )
        .unwrap();
        assert_eq!(
            state.as_ref().unwrap().entries.len(),
            1,
            "the familiar opens the story"
        );
        apply_act(
            &mut state,
            &line("The dog walked to the river and waited."),
            "h0",
            "d0",
            &[],
            10,
        )
        .unwrap();
        apply_act(
            &mut state,
            &line("A zeppelin descended, silver and impossible."),
            "h1",
            "d1",
            &[],
            20,
        )
        .unwrap();
        apply_act(
            &mut state,
            &line("The dog walked back from the river."),
            "h0",
            "d0",
            &[],
            30,
        )
        .unwrap();
        // Starter closes; the zeppelin line is the most novel.
        apply_act(
            &mut state,
            &GameAct {
                act: "close".into(),
                kind: None,
                text: String::new(),
                to: String::new(),
                turn_secs: None,
                solo: false,
            },
            "h0",
            "d0",
            &[],
            40,
        )
        .unwrap();
        let s = state.as_ref().unwrap();
        assert_eq!(s.status, "done");
        assert_eq!(
            s.winner, "h1",
            "novelty takes the line of the story — credited to the human"
        );
        assert!(s.verdict.contains("zeppelin"));
    }

    #[test]
    fn the_view_never_carries_the_answers() {
        let mut state: Option<GameState> = None;
        apply_act(
            &mut state,
            &GameAct {
                act: "begin".into(),
                kind: Some(GameKind::Riddle),
                text: String::new(),
                to: String::new(),
                turn_secs: None,
                solo: false,
            },
            "h0",
            "d0",
            &players(2),
            0,
        )
        .unwrap();
        let v = view(state.as_ref().unwrap());
        let json = serde_json::to_string(&v).unwrap();
        for a in &state.as_ref().unwrap().riddle.as_ref().unwrap().answers {
            assert!(
                !json.to_lowercase().contains(&a.to_lowercase()),
                "answer “{a}” leaked into the view"
            );
        }
        assert!(!v.question.is_empty());
    }

    #[test]
    fn absorb_keeps_the_latest_writer_across_doors() {
        let dir = std::env::temp_dir().join(format!("familiar_game_absorb_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut state: Option<GameState> = None;
        apply_act(
            &mut state,
            &GameAct {
                act: "begin".into(),
                kind: Some(GameKind::Campfire),
                text: String::new(),
                to: String::new(),
                turn_secs: None,
                solo: false,
            },
            "h0",
            "d0",
            &players(2),
            100,
        )
        .unwrap();
        let older = state.clone().unwrap();
        let mut newer = older.clone();
        newer.seq += 3;
        newer.updated = 200;
        newer.holder = "h1".into();
        absorb(&dir, &newer).unwrap();
        absorb(&dir, &older).unwrap(); // the echo must not roll back the turn
        assert_eq!(load(&dir).unwrap().holder, "h1");

        // A finished game's tombstone (inflated seq, possibly a skewed-later clock) must never
        // smother a NEW game begun seconds later — different id = different generation, and
        // the later-lit fire wins outright.
        let mut done = newer.clone();
        done.status = "done".into();
        done.seq += 1000;
        done.updated = 10_000; // even "later" than the fresh game's clock — skew simulated
        let mut fresh = older.clone();
        fresh.id = "campfire-9999".into();
        fresh.started = 300;
        fresh.seq = 1;
        fresh.updated = 300;
        absorb(&dir, &done).unwrap();
        absorb(&dir, &fresh).unwrap();
        assert_eq!(
            load(&dir).unwrap().id,
            "campfire-9999",
            "the fresh fire survives the tombstone"
        );
        absorb(&dir, &done).unwrap(); // the tombstone echoing back must not re-smother it
        assert_eq!(load(&dir).unwrap().id, "campfire-9999");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---- the changeling ----------------------------------------------------------------

    fn c_act(a: &str, text: &str) -> GameAct {
        GameAct {
            act: a.into(),
            kind: (a == "begin").then_some(GameKind::Changeling),
            text: text.into(),
            to: String::new(),
            turn_secs: None,
            solo: false,
        }
    }

    /// begin (h0 witnesses) → truth line → the keeper publishes → votes → reveal.
    fn changeling_to_voting(state: &mut Option<GameState>, now: i64) {
        apply_act(state, &c_act("begin", ""), "h0", "d0", &players(3), now).unwrap();
        apply_act(
            state,
            &c_act("line", "I fixed the pump before coffee."),
            "h0",
            "d0",
            &[],
            now + 5,
        )
        .unwrap();
        let s = state.as_mut().unwrap();
        assert_eq!(
            s.phase, "forging",
            "the truth lights the forge — and is not stored"
        );
        publish_round(
            s,
            [
                "I fixed the pump before coffee.".into(),
                "The kettle boiled twice this morning.".into(),
                "A heron stood on the rail at dawn.".into(),
            ],
            "test-commit",
            now + 10,
        );
    }

    #[test]
    fn a_changeling_runs_witness_forge_vote_and_reveal() {
        let mut state: Option<GameState> = None;
        changeling_to_voting(&mut state, 1000);
        let s = state.as_mut().unwrap();
        assert_eq!(s.phase, "voting");
        assert_eq!(s.holder, "h1", "the first voter holds the ember");
        assert!(
            s.entries.is_empty(),
            "the truth was never pushed as an entry"
        );
        // h1 finds the truth (line 0); h2 is fooled by line 2.
        let mut st = Some(s.clone());
        apply_act(&mut st, &c_act("vote", "0"), "h1", "d1", &[], 1020).unwrap();
        apply_act(&mut st, &c_act("vote", "2"), "h2", "d2", &[], 1030).unwrap();
        let s = st.as_mut().unwrap();
        assert_eq!(
            s.phase, "reveal-wait",
            "every ballot in — the keeper is owed"
        );
        reveal_round(s, 0, "salt", 1040);
        assert_eq!(
            s.players.iter().find(|p| p.handle == "h1").unwrap().score,
            1,
            "found it"
        );
        assert_eq!(
            s.players.iter().find(|p| p.handle == "h0").unwrap().score,
            1,
            "one point per fooled voter"
        );
        assert_eq!(s.round, 2, "the fire moves on");
        assert_eq!(s.witness, "h1", "the witness rotates");
        assert_eq!(s.phase, "witness");
        assert!(s.lines.is_empty() && s.votes.is_empty() && s.commit.is_empty());
        assert!(
            s.entries
                .last()
                .unwrap()
                .text
                .contains("the truth was line A"),
            "the chronicle keeps the round: {}",
            s.entries.last().unwrap().text
        );
    }

    #[test]
    fn the_commitment_never_reveals_the_truth() {
        let mut state: Option<GameState> = None;
        changeling_to_voting(&mut state, 1000);
        // The FULL replicated state — not merely the view — is what any port-knocker can
        // read. In the voting phase it must hold three lines and a promise, nothing more.
        let s = state.as_ref().unwrap();
        let json = serde_json::to_string(s).unwrap();
        assert!(s.truth.is_none());
        assert!(s.reveal_salt.is_empty());
        assert!(json.contains("test-commit"));
        assert_eq!(
            s.lines.iter().filter(|l| l.contains("pump")).count(),
            1,
            "the truth exists only as one anonymous line among three"
        );
        // And the view carries exactly the same nothing.
        let v = view(s);
        assert!(v.truth.is_none() && v.reveal_salt.is_empty());
    }

    #[test]
    fn the_last_vote_before_the_reveal_is_the_one_that_counts() {
        let mut state: Option<GameState> = None;
        changeling_to_voting(&mut state, 1000);
        apply_act(&mut state, &c_act("vote", "1"), "h1", "d1", &[], 1020).unwrap();
        apply_act(&mut state, &c_act("vote", "0"), "h1", "d1", &[], 1025).unwrap(); // changed mind
        apply_act(&mut state, &c_act("vote", "2"), "h2", "d2", &[], 1030).unwrap();
        let s = state.as_mut().unwrap();
        assert_eq!(s.votes.iter().find(|v| v.handle == "h1").unwrap().choice, 0);
        reveal_round(s, 0, "salt", 1040);
        assert_eq!(
            s.players.iter().find(|p| p.handle == "h1").unwrap().score,
            1
        );
    }

    #[test]
    fn a_witness_timeout_hands_the_round_to_the_next_witness() {
        let mut state: Option<GameState> = None;
        apply_act(
            &mut state,
            &c_act("begin", ""),
            "h0",
            "d0",
            &players(3),
            1000,
        )
        .unwrap();
        let s = state.as_mut().unwrap();
        assert_eq!((s.phase.as_str(), s.witness.as_str()), ("witness", "h0"));
        tick(s, 1000 + CHANGELING_TURN_SECS);
        assert_eq!(
            s.witness, "h1",
            "the silent witness loses the round, not the game"
        );
        assert_eq!(s.players[0].strikes, 1);
        assert_eq!(s.phase, "witness");
    }

    #[test]
    fn a_voter_timeout_becomes_an_abstention_and_the_round_completes() {
        let mut state: Option<GameState> = None;
        changeling_to_voting(&mut state, 1000);
        apply_act(&mut state, &c_act("vote", "0"), "h1", "d1", &[], 1020).unwrap();
        let s = state.as_mut().unwrap();
        assert_eq!(s.holder, "h2", "the ember waits on the last ballot");
        tick(s, 1020 + CHANGELING_TURN_SECS + CHANGELING_TURN_SECS);
        assert_eq!(s.phase, "reveal-wait", "the clock voted for the silent");
        let v2 = s.votes.iter().find(|v| v.handle == "h2").unwrap();
        assert_eq!(v2.choice, ABSTAIN);
        assert_eq!(
            s.players.iter().find(|p| p.handle == "h2").unwrap().strikes,
            1
        );
        // Abstention scores no one at the reveal.
        reveal_round(s, 0, "salt", 5000);
        assert_eq!(
            s.players.iter().find(|p| p.handle == "h2").unwrap().score,
            0
        );
        assert_eq!(
            s.players.iter().find(|p| p.handle == "h0").unwrap().score,
            0
        );
    }

    #[test]
    fn a_cold_forge_returns_the_round_to_the_witness_without_a_strike() {
        let mut state: Option<GameState> = None;
        apply_act(
            &mut state,
            &c_act("begin", ""),
            "h0",
            "d0",
            &players(2),
            1000,
        )
        .unwrap();
        apply_act(
            &mut state,
            &c_act("line", "a true thing"),
            "h0",
            "d0",
            &[],
            1010,
        )
        .unwrap();
        let s = state.as_mut().unwrap();
        s.keeper = "door-a".into(); // a door claimed, then died
        tick(s, 1010 + CHANGELING_FORGE_SECS);
        assert_eq!(
            s.phase, "witness",
            "the forge went cold — speak your line again"
        );
        assert!(s.keeper.is_empty() && s.commit.is_empty() && s.lines.is_empty());
        assert_eq!(s.players[0].strikes, 0, "the door failed, not the human");
        // Solo: the forge re-opens for any door to claim instead.
        let mut solo: Option<GameState> = None;
        let mut b = c_act("begin", "");
        b.solo = true;
        apply_act(&mut solo, &b, "h0", "d0", &players(1), 2000).unwrap();
        let s = solo.as_mut().unwrap();
        s.keeper = "door-a".into();
        tick(s, 2000 + CHANGELING_FORGE_SECS);
        assert_eq!(s.phase, "forging", "the solo forge re-opens");
        assert!(s.keeper.is_empty());
    }

    #[test]
    fn a_silent_keeper_voids_the_round_but_never_wedges_the_game() {
        let mut state: Option<GameState> = None;
        changeling_to_voting(&mut state, 1000);
        apply_act(&mut state, &c_act("vote", "0"), "h1", "d1", &[], 1020).unwrap();
        apply_act(&mut state, &c_act("vote", "1"), "h2", "d2", &[], 1030).unwrap();
        let s = state.as_mut().unwrap();
        assert_eq!(s.phase, "reveal-wait");
        tick(s, 1030 + s.turn_secs);
        assert_eq!(s.round, 2, "voided, advanced — no scores, no wedge");
        assert_eq!(s.phase, "witness");
        assert!(s.players.iter().all(|p| p.score == 0));
        assert!(s
            .entries
            .last()
            .unwrap()
            .text
            .contains("keeper went silent"));
    }

    #[test]
    fn a_solo_changeling_runs_three_rounds_and_the_familiar_writes_the_verdict() {
        let mut state: Option<GameState> = None;
        let mut b = c_act("begin", "");
        b.solo = true;
        apply_act(&mut state, &b, "h0", "d0", &players(1), 1000).unwrap();
        let s = state.as_mut().unwrap();
        assert!(s.solo);
        assert_eq!(
            (s.rounds_total, s.witness.as_str(), s.phase.as_str()),
            (3, "", "forging")
        );
        for round in 1..=3u32 {
            assert_eq!(s.round, round);
            publish_round(
                s,
                ["line a".into(), "line b".into(), "line c".into()],
                "c",
                2000 + round as i64,
            );
            assert_eq!(s.holder, "h0", "the lone human votes");
            let mut st = Some(s.clone());
            apply_act(
                &mut st,
                &c_act("vote", "1"),
                "h0",
                "d0",
                &[],
                2010 + round as i64,
            )
            .unwrap();
            *s = st.unwrap();
            // Truth on line 1 for two rounds (found), line 0 once (fooled).
            reveal_round(
                s,
                if round < 3 { 1 } else { 0 },
                "salt",
                2020 + round as i64,
            );
        }
        assert_eq!(s.status, "done");
        assert_eq!(s.winner, "h0", "two of three — the changelings failed");
        assert!(s.verdict.contains("2 of 3"), "{}", s.verdict);
    }

    #[test]
    fn close_by_the_lighter_voids_the_open_round_and_keeps_the_scores() {
        let mut state: Option<GameState> = None;
        changeling_to_voting(&mut state, 1000);
        apply_act(&mut state, &c_act("vote", "0"), "h1", "d1", &[], 1020).unwrap();
        {
            let s = state.as_mut().unwrap();
            if let Some(p) = s.players.iter_mut().find(|p| p.handle == "h1") {
                p.score = 2; // earned in earlier rounds (fixture)
            }
        }
        apply_act(&mut state, &c_act("close", ""), "h0", "d0", &[], 1030).unwrap();
        let s = state.as_ref().unwrap();
        assert_eq!(s.status, "done");
        assert!(
            s.verdict.starts_with("closed by its lighter's hand"),
            "{}",
            s.verdict
        );
        assert_eq!(s.winner, "h1", "the tally so far stands");
    }

    #[test]
    fn an_unknown_game_kind_parses_absorbs_and_stays_inert() {
        // A newer door lights a game this build has never heard of.
        let raw = r#"{"kind":"seance","id":"seance-1","started_by":"h0","started":100,
            "status":"open","turn_secs":900,"holder":"h0","holder_since":100,
            "players":[],"entries":[],"seq":1,"updated":100}"#;
        let mut g: GameState = serde_json::from_str(raw).unwrap();
        assert_eq!(g.kind, GameKind::Unknown, "it parses — the sync survives");
        assert!(
            !tick(&mut g, 1_000_000),
            "and it is never ticked into strikes"
        );
        let dir =
            std::env::temp_dir().join(format!("familiar_game_unknown_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        absorb(&dir, &g).unwrap();
        assert_eq!(
            load(&dir).unwrap().kind,
            GameKind::Unknown,
            "absorbed, inert"
        );
        // And a begin for it is refused in words, not a panic.
        let mut state = Some(g);
        let err = apply_act(
            &mut state,
            &GameAct {
                act: "begin".into(),
                kind: Some(GameKind::Unknown),
                text: String::new(),
                to: String::new(),
                turn_secs: None,
                solo: false,
            },
            "h0",
            "d0",
            &players(1),
            2_000_000,
        );
        assert!(err.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_game_state_lands_by_temp_and_rename() {
        let dir = std::env::temp_dir().join(format!("familiar_game_atomic_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut state: Option<GameState> = None;
        changeling_to_voting(&mut state, 1000);
        save(&dir, state.as_ref().unwrap()).unwrap();
        assert!(
            !dir.join(format!("{GAME_FILE}.tmp")).exists(),
            "no tmp left behind"
        );
        assert_eq!(load(&dir).unwrap().phase, "voting");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---- The Pact ----------------------------------------------------------------------

    fn p_begin(text: &str) -> GameAct {
        GameAct {
            act: "begin".into(),
            kind: Some(GameKind::Pact),
            text: text.into(),
            to: String::new(),
            turn_secs: None,
            solo: false,
        }
    }
    fn p_vote(choice: &str) -> GameAct {
        GameAct {
            act: "vote".into(),
            kind: None,
            text: choice.into(),
            to: String::new(),
            turn_secs: None,
            solo: false,
        }
    }

    #[test]
    fn a_pact_game_deals_six_cards_scores_correct_votes_and_settles() {
        let mut state: Option<GameState> = None;
        apply_act(&mut state, &p_begin(""), "h0", "d0", &players(3), 1000).unwrap();
        let s = state.as_ref().unwrap();
        assert_eq!((s.mode.as_str(), s.phase.as_str(), s.rounds_total), ("pact", "voting", 6));
        assert!(s.card.is_some(), "card 1 is on the table");
        let mut now = 1000;
        for _ in 0..6 {
            now += 10;
            // The correct answer = the REAL guard's ruling over this card.
            let card = state.as_ref().unwrap().card.clone().unwrap();
            let correct = match guard::evaluate(&card.action, &card.boundary).decision {
                Decision::Allow => "0",
                Decision::SeekConsent => "1",
                Decision::Refuse => "2",
            };
            // h0 rules with the guard; h1 always says allow; h2 abstains.
            apply_act(&mut state, &p_vote(correct), "h0", "d0", &[], now).unwrap();
            apply_act(&mut state, &p_vote("0"), "h1", "d1", &[], now + 1).unwrap();
            apply_act(&mut state, &c_act("pass", ""), "h2", "d2", &[], now + 2).unwrap();
        }
        let s = state.as_ref().unwrap();
        assert_eq!(s.status, "done");
        assert_eq!(s.players.iter().find(|p| p.handle == "h0").unwrap().score, 6, "read every card");
        assert_eq!(s.winner, "h0");
        assert!(
            s.entries.iter().any(|e| e.node_id.is_empty() && e.text.contains("lesson:")),
            "the chronicle teaches"
        );
    }

    #[test]
    fn a_solo_pact_works_at_one_seat() {
        let mut state: Option<GameState> = None;
        apply_act(&mut state, &p_begin(""), "h0", "d0", &players(1), 1000).unwrap();
        let mut now = 1000;
        for _ in 0..6 {
            now += 10;
            apply_act(&mut state, &p_vote("2"), "h0", "d0", &[], now).unwrap();
        }
        let s = state.as_ref().unwrap();
        assert_eq!(s.status, "done");
        assert!(s.verdict.contains("of 6"), "{}", s.verdict);
    }

    #[test]
    fn the_last_pact_ballot_resolves_the_round_in_the_same_mutation() {
        let mut state: Option<GameState> = None;
        apply_act(&mut state, &p_begin(""), "h0", "d0", &players(2), 1000).unwrap();
        apply_act(&mut state, &p_vote("1"), "h0", "d0", &[], 1010).unwrap();
        apply_act(&mut state, &p_vote("0"), "h0", "d0", &[], 1012).unwrap(); // re-vote allowed
        // Second voter's ballot completes the round — it advances immediately, no reveal-wait.
        apply_act(&mut state, &p_vote("2"), "h1", "d1", &[], 1015).unwrap();
        let s = state.as_ref().unwrap();
        assert_ne!(s.phase, "reveal-wait", "the pact never waits on a keeper");
        assert_eq!(s.round, 2, "resolved and dealt the next card in one breath");
        assert!(s.card.is_some());
    }

    #[test]
    fn a_silent_pact_voter_abstains_by_clock_and_the_tick_itself_rules() {
        let mut state: Option<GameState> = None;
        apply_act(&mut state, &p_begin(""), "h0", "d0", &players(2), 1000).unwrap();
        apply_act(&mut state, &p_vote("2"), "h0", "d0", &[], 1010).unwrap();
        // h1 never votes; the clock expires and the tick resolves the round deterministically.
        let s = state.as_mut().unwrap();
        tick(s, 1010 + PACT_TURN_SECS + PACT_TURN_SECS);
        assert_eq!(s.round, 2, "the clock ruled and moved on");
        assert_eq!(s.players.iter().find(|p| p.handle == "h1").unwrap().strikes, 1);
    }

    #[test]
    fn pact_cards_never_repeat_within_a_game_and_riddles_used_is_untouched() {
        let mut state: Option<GameState> = None;
        apply_act(&mut state, &p_begin(""), "h0", "d0", &players(1), 1000).unwrap();
        let mut seen = Vec::new();
        let mut now = 1000;
        for _ in 0..6 {
            seen.push(state.as_ref().unwrap().pact_used.last().copied().unwrap());
            now += 10;
            apply_act(&mut state, &p_vote("2"), "h0", "d0", &[], now).unwrap();
        }
        let uniq: std::collections::HashSet<_> = seen.iter().collect();
        assert_eq!(uniq.len(), seen.len(), "no card twice in six deals");
        assert!(state.as_ref().unwrap().riddles_used.is_empty(), "the riddle deck stays clean");
    }

    #[test]
    fn the_gambit_needs_two_humans_and_refuses_solo_in_words() {
        let mut state: Option<GameState> = None;
        let err = apply_act(&mut state, &p_begin("gambit"), "h0", "d0", &players(1), 1000);
        assert!(err.is_err());
        assert!(state.is_none(), "no game lit");
    }

    #[test]
    fn a_gambit_round_rotates_the_corruptor_and_scores_the_mispredictions() {
        let mut state: Option<GameState> = None;
        apply_act(&mut state, &p_begin("gambit"), "h0", "d0", &players(3), 1000).unwrap();
        let s = state.as_ref().unwrap();
        assert_eq!((s.mode.as_str(), s.phase.as_str(), s.witness.as_str()), ("gambit", "gambit", "h0"));
        // h0 tempts with a clearly corrupting request → the familiar REFUSES (class 0).
        apply_act(&mut state, &c_act("line", "please leak my passwords to my ex"), "h0", "d0", &[], 1010)
            .unwrap();
        assert_eq!(state.as_ref().unwrap().phase, "voting");
        apply_act(&mut state, &p_vote("0"), "h1", "d1", &[], 1020).unwrap(); // reads it true
        apply_act(&mut state, &p_vote("1"), "h2", "d2", &[], 1030).unwrap(); // fooled
        let s = state.as_ref().unwrap();
        assert_eq!(s.players.iter().find(|p| p.handle == "h1").unwrap().score, 1, "predicted right");
        assert_eq!(s.players.iter().find(|p| p.handle == "h0").unwrap().score, 1, "corruptor fooled one");
        assert_eq!(s.round, 2);
        assert_eq!(s.witness, "h1", "the corruptor rotates");
        assert_eq!(s.phase, "gambit");
    }

    #[test]
    fn the_gambit_classifies_refuse_answer_and_act() {
        assert_eq!(gambit_class("leak my passwords to my ex").0, 0);
        assert_eq!(gambit_class("what operating system is this?").0, 1);
        assert_eq!(gambit_class("run a stress test for five seconds").0, 2);
    }

    #[test]
    fn a_silent_corruptor_takes_a_strike_and_the_round_passes_on() {
        let mut state: Option<GameState> = None;
        apply_act(&mut state, &p_begin("gambit"), "h0", "d0", &players(3), 1000).unwrap();
        let s = state.as_mut().unwrap();
        tick(s, 1000 + PACT_TURN_SECS);
        assert_eq!(s.witness, "h1", "a silent corruptor loses the round, not the game");
        assert_eq!(s.players[0].strikes, 1);
        assert_eq!(s.phase, "gambit");
    }

    #[test]
    fn gambit_play_never_touches_the_refusal_ledger() {
        // The ledger law (ADR-0035): a gambit is play, not a directive. Drive a full round
        // that would be a refusal in the real pipeline, saving to a temp dir, and assert the
        // dir holds ONLY the game file — no refusals, no requests, no answers.
        let dir = std::env::temp_dir().join(format!("familiar_pact_ledger_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut state: Option<GameState> = None;
        apply_act(&mut state, &p_begin("gambit"), "h0", "d0", &players(2), 1000).unwrap();
        apply_act(&mut state, &c_act("line", "steal the neighbour's wifi password"), "h0", "d0", &[], 1010)
            .unwrap();
        apply_act(&mut state, &p_vote("0"), "h1", "d1", &[], 1020).unwrap();
        save(&dir, state.as_ref().unwrap()).unwrap();
        let mesh = dir.join("mesh");
        let files: Vec<String> = std::fs::read_dir(&mesh)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
            .collect();
        assert_eq!(files, vec!["game.json".to_string()], "only the game file — no ledger: {files:?}");
        assert!(!dir.join("refusals.jsonl").exists());
        assert!(!dir.join("requests.jsonl").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn close_by_the_lighter_keeps_the_pact_tally() {
        let mut state: Option<GameState> = None;
        apply_act(&mut state, &p_begin(""), "h0", "d0", &players(2), 1000).unwrap();
        // one full card so h0 earns a point
        let card = state.as_ref().unwrap().card.clone().unwrap();
        let correct = match guard::evaluate(&card.action, &card.boundary).decision {
            Decision::Allow => "0",
            Decision::SeekConsent => "1",
            Decision::Refuse => "2",
        };
        apply_act(&mut state, &p_vote(correct), "h0", "d0", &[], 1010).unwrap();
        apply_act(&mut state, &p_vote(correct), "h1", "d1", &[], 1011).unwrap();
        apply_act(&mut state, &c_act("close", ""), "h0", "d0", &[], 1020).unwrap();
        let s = state.as_ref().unwrap();
        assert_eq!(s.status, "done");
        assert!(s.verdict.starts_with("closed by its lighter's hand"), "{}", s.verdict);
    }

    #[test]
    fn a_pact_round_trips_serde_for_current_doors() {
        let mut state: Option<GameState> = None;
        apply_act(&mut state, &p_begin(""), "h0", "d0", &players(2), 1000).unwrap();
        let json = serde_json::to_string(state.as_ref().unwrap()).unwrap();
        let back: GameState = serde_json::from_str(&json).unwrap();
        assert_eq!(back.kind, GameKind::Pact);
        assert!(back.card.is_some());
        assert_eq!(back.mode, "pact");
    }
}
