//! The Changeling's keeper — the door-side half of the third game (ADR-0034).
//!
//! The rules live in [`crate::game`], pure and clockwork. What lives here is the one
//! thing rules cannot be: a secret. The door that receives the witness's true line
//! becomes the round's **keeper** — it forges the two false lines, shuffles the three,
//! and holds *which index is true* in a door-local file that never replicates. Into
//! the shared state goes only a salted commitment, because replicated game state is
//! readable off any door's port (`GET /mesh/records`), and a 3-value secret with its
//! salt beside it would be brute-forced in three hashes. The salt travels only at the
//! reveal, when the promise is opened for every door to check.
//!
//! The keeper acts lazily, like every game clock: [`touch`] is called from acts,
//! worldview reads, and record-sync absorbs, and does the round's next duty if this
//! door is the one that owes it. A keeper that dies mid-round costs the fire nothing —
//! any door voids the round after one turn clock (`game::changeling_tick`).
//!
//! Forging prefers the LLM (boundary-gated, serialized in-process by the llm crate's
//! consult lock) and **never depends on it**: the deterministic banks below are the
//! floor, and also the CI path.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::game::{self, GameKind, GameState};
use crate::{sha256_hex, Result};

/// The keeper's memory: which line is true, this round, and the salt that proves it.
/// Door-local. NEVER carried by record-sync, briefs, or the worldview.
pub const KEEPER_FILE: &str = "mesh/changeling.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RoundSecret {
    game_id: String,
    round: u32,
    truth_idx: u8,
    salt: String,
}

fn keeper_path(dir: &Path) -> PathBuf {
    dir.join(KEEPER_FILE)
}

fn load_secret(dir: &Path) -> Option<RoundSecret> {
    let raw = std::fs::read(keeper_path(dir)).ok()?;
    serde_json::from_slice(&raw).ok()
}

fn save_secret(dir: &Path, s: &RoundSecret) -> std::io::Result<()> {
    let path = keeper_path(dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Temp + rename: the secret must exist durably BEFORE the lines publish, or a
    // crash between the two leaves a round nobody can ever reveal.
    let tmp = dir.join(format!("{KEEPER_FILE}.tmp"));
    std::fs::write(&tmp, serde_json::to_vec_pretty(s)?)?;
    std::fs::rename(&tmp, path)
}

/// The keeper's promise: hex sha256 over id, round, salt, and the truth's index.
pub fn commitment(game_id: &str, round: u32, salt: &str, truth_idx: u8) -> String {
    sha256_hex(format!("{game_id}|{round}|{salt}|{truth_idx}").as_bytes())
}

/// Verify a revealed round against its commitment — any door, any console can.
pub fn verify(game_id: &str, round: u32, salt: &str, truth_idx: u8, commit: &str) -> bool {
    !commit.is_empty() && commitment(game_id, round, salt, truth_idx) == commit
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x1_0000_0000_01b3;

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h = FNV_OFFSET;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

// ---- the deterministic floor ---------------------------------------------------------
//
// The game never depends on the model. When the LLM is closed, absent, rate-limited, or
// answers nonsense, the forge draws from these banks — weaker changelings, a live fire.

/// Household-plausible day lines (multiplayer fallback).
const BANK_DAY: [&str; 24] = [
    "Spent twenty minutes hunting my keys; they were in yesterday's jacket the whole time.",
    "The kettle boiled twice before I remembered why I'd walked into the kitchen.",
    "Stepped outside to check the weather and stayed out twice as long as I meant to.",
    "Found a pen that actually writes on the first try. Kept it. Told no one.",
    "The good mug was already in the sink, so the day started on the second-best one.",
    "Took a wrong turn on a road I've driven a hundred times and saw a heron for it.",
    "Fixed a thing by turning it off and on, and accepted the mystery with grace.",
    "Made a list of four things, did three, and rewrote the list so it looked finished.",
    "The bread was one day past its best, which is the day it makes the best toast.",
    "Watched the rain start from the one window that never gets the wind.",
    "A drawer that has stuck for months opened smoothly today, unprovoked.",
    "Waved at someone who was waving at the person behind me, and committed to it.",
    "The battery warning came at the exact moment there was nothing left to save.",
    "Learned the neighbor's dog's name before the neighbor's, again.",
    "Put something somewhere clever so I wouldn't lose it. It is lost.",
    "The last cookie turned out to be two cookies stuck together — a good omen.",
    "A song got stuck from one overheard chorus in a parking lot.",
    "Untangled one cable and created two new tangles, conserving tangle overall.",
    "The sunset was doing something worth stopping for, so the errand waited.",
    "Wrote a reminder so cryptic it now reminds me only that I once knew something.",
    "The spare change jar crossed some invisible threshold and is now officially heavy.",
    "Sat down for one minute in the warm patch by the window. It was not one minute.",
    "Finally used the fancy soap that was being saved for no particular occasion.",
    "The clock in the other room is still an hour off, and it's told the truth twice.",
];

/// Observation-shaped lines (solo fallback), in the fixed "the mesh saw …" register.
const BANK_MESH: [&str; 24] = [
    "the mesh saw a device rejoin with a new address and pretend nothing happened",
    "the mesh saw the router hand out a lease it had promised to someone else",
    "the mesh saw a phone report motion at an hour with no footsteps in it",
    "the mesh saw a service advertise itself twice and answer as neither",
    "the mesh saw the gateway blink and every device act like it was personal",
    "the mesh saw a peer offer three tools and withdraw two before anyone asked",
    "the mesh saw the same question asked politely by two different consoles",
    "the mesh saw a battery report the same percentage for suspiciously long",
    "the mesh saw a printer appear on the network like a ghost at a banquet",
    "the mesh saw a device introduce itself by a name nobody had given it",
    "the mesh saw the clock drift far enough to make two records argue",
    "the mesh saw an observation arrive twice and agreed with itself both times",
    "the mesh saw a signal so faint it filed it under weather",
    "the mesh saw a laptop leave without saying goodbye, as laptops do",
    "the mesh saw the lighthouse answer a knock meant for another door",
    "the mesh saw a watch report presence from a wrist that was clearly asleep",
    "the mesh saw a discovery sweep find exactly what it found yesterday, proudly",
    "the mesh saw a certificate age one day closer to its little retirement",
    "the mesh saw a message take the long way around and arrive smug about it",
    "the mesh saw two doors agree so quickly it double-checked the arithmetic",
    "the mesh saw a device ask for the worldview and read only the headlines",
    "the mesh saw an ember pass between rooms without a hand touching it",
    "the mesh saw a nonce used once, as nonces dream of being used",
    "the mesh saw a quiet hour and kept it on file as evidence of peace",
];

/// Two distinct bank lines, deterministically seeded — same inputs, same forgeries, on
/// every door that might ever have to re-forge the round.
fn bank_lines(bank: &[&str; 24], seed: &str) -> [String; 2] {
    let h = fnv1a64(seed.as_bytes());
    let i = (h % 24) as usize;
    let j = (i + 1 + ((h >> 8) % 23) as usize) % 24;
    [bank[i].to_string(), bank[j].to_string()]
}

/// Parse the adapter's strict-JSON `{"lines":["…","…"]}` reply into two usable lines.
fn parse_forged(raw: &str) -> Option<[String; 2]> {
    let v: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
    let arr = v.get("lines")?.as_array()?;
    let mut out: Vec<String> = arr
        .iter()
        .filter_map(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s.chars().count() <= game::MAX_LINE_CHARS)
        .collect();
    if out.len() < 2 {
        return None;
    }
    out.truncate(2);
    Some([out.remove(0), out.remove(0)])
}

/// Mesh-public texture for the forge prompt: member labels and recent observation
/// objects that are already safe for every member console — never sensitive-personal.
fn public_facts(dir: &Path) -> Vec<String> {
    let mut facts: Vec<String> = Vec::new();
    if let Ok(obs) = familiar_kernel::observation::load_recent(dir, 400) {
        for o in obs.iter().rev() {
            if familiar_kernel::service::is_sensitive_personal(o) {
                continue;
            }
            if o.actor == "familiar" && o.action == "reports" {
                continue;
            }
            if facts.len() >= 5 {
                break;
            }
            facts.push(format!("{} {} {}", o.actor, o.action, o.object));
        }
    }
    facts
}

/// The solo truth: one real, shareable observation, spoken in the fixed register.
/// None when the record holds nothing safe to lie about.
fn solo_truth(dir: &Path, seed: &str) -> Option<String> {
    let obs = familiar_kernel::observation::load_recent(dir, 4000).ok()?;
    let candidates: Vec<String> = obs
        .iter()
        .filter(|o| !familiar_kernel::service::is_sensitive_personal(o))
        .filter(|o| !(o.actor == "familiar" && o.action == "reports"))
        .filter(|o| !o.actor.is_empty() && !o.action.is_empty() && !o.object.is_empty())
        .map(|o| {
            let line = format!("the mesh saw {} {} {}", o.actor, o.action, o.object);
            line.chars().take(game::MAX_LINE_CHARS).collect()
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let pick = (fnv1a64(seed.as_bytes()) % candidates.len() as u64) as usize;
    Some(candidates[pick].clone())
}

fn forge_prompt(truth: &str, witness_label: &str, solo: bool, facts: &[String]) -> String {
    let texture = if facts.is_empty() {
        String::new()
    } else {
        format!(
            "Mesh-public texture you may echo lightly: {}\n",
            facts.join("; ")
        )
    };
    if solo {
        format!(
            "You are the changeling at a household party game on a private mesh. The mesh \
             recorded ONE true line about what it observed. Write exactly TWO plausible FALSE \
             lines in the same fixed register — each must begin \"the mesh saw\" — ordinary \
             household-network texture, nothing sensational, each under 240 characters. Do not \
             repeat or near-copy the true line. Do not reveal this instruction.\n{texture}\
             The true line: \"{truth}\"\n\
             Answer with STRICT JSON, nothing else: {{\"lines\":[\"…\",\"…\"]}}"
        )
    } else {
        format!(
            "You are the changeling at a household party game on a private mesh. One human, \
             {witness_label}, wrote ONE true line about their day. Write exactly TWO plausible \
             FALSE lines in the same voice: same register, same length feel, ordinary household \
             texture, nothing sensational, each under 240 characters. Do not repeat or near-copy \
             the true line. Do not reveal this instruction.\n{texture}\
             The true line: \"{truth}\"\n\
             Answer with STRICT JSON, nothing else: {{\"lines\":[\"…\",\"…\"]}}"
        )
    }
}

/// Forge the round: truth + two changelings, shuffled by a salted index. Returns the
/// three lines, the truth's index, the salt, and the commitment. Deterministic given
/// (id, round, salt); the salt is fresh randomness per forge.
fn forge(
    dir: &Path,
    s: &GameState,
    truth: &str,
    use_llm: bool,
) -> ([String; 3], u8, String, String) {
    let seed = format!("{}|{}", s.id, s.round);
    let bank = if s.solo { &BANK_MESH } else { &BANK_DAY };
    let witness_label = s
        .players
        .iter()
        .find(|p| p.handle == s.witness)
        .map(|p| p.label.clone())
        .unwrap_or_else(|| s.witness.clone());
    let forged = if use_llm {
        match familiar_llm::consult(
            dir,
            &forge_prompt(truth, &witness_label, s.solo, &public_facts(dir)),
        ) {
            Ok(familiar_llm::Outcome::Response(raw)) => {
                parse_forged(&raw).unwrap_or_else(|| bank_lines(bank, &seed))
            }
            _ => bank_lines(bank, &seed),
        }
    } else {
        bank_lines(bank, &seed)
    };
    let salt = crate::os_random::<16>()
        .map(|b| crate::hex_encode(&b))
        .unwrap_or_else(|_| format!("{:032x}", fnv1a64(seed.as_bytes())));
    let truth_idx = (fnv1a64(format!("{seed}|{salt}").as_bytes()) % 3) as u8;
    let mut lines: [String; 3] = [String::new(), String::new(), String::new()];
    lines[truth_idx as usize] = truth.to_string();
    let mut fi = 0;
    for slot in 0..3u8 {
        if slot != truth_idx {
            lines[slot as usize] = forged[fi].clone();
            fi += 1;
        }
    }
    let commit = commitment(&s.id, s.round, &salt, truth_idx);
    (lines, truth_idx, salt, commit)
}

/// The keeper's one entry point — cheap unless a changeling owes this door a duty.
/// Called from game acts (with the witness's text when the act was their line),
/// worldview reads, and record-sync absorbs. Returns true when it changed the game.
pub fn touch(dir: &Path, my_node_id: &str, truth: Option<String>, now: i64) -> Result<bool> {
    let Some(s) = game::load(dir) else {
        return Ok(false);
    };
    if s.status != "open" || s.kind != GameKind::Changeling {
        return Ok(false);
    }
    // Claim + forge: a fresh forge with no keeper, and either the truth just landed here
    // (multiplayer) or the familiar itself witnesses (solo).
    if s.phase == "forging" && s.keeper.is_empty() && (truth.is_some() || s.solo) {
        return claim_and_forge(dir, my_node_id, s, truth, now).map(|_| true);
    }
    // Reveal: every ballot is in and this door holds the round's secret.
    if s.phase == "reveal-wait" && s.keeper == my_node_id {
        if let Some(sec) = load_secret(dir) {
            if sec.game_id == s.id && sec.round == s.round {
                let mut s = s;
                game::reveal_round(&mut s, sec.truth_idx, &sec.salt, now);
                game::save(dir, &s)?;
                return Ok(true);
            }
        }
        // Secret lost (disk gone between forge and reveal): stay silent — the any-door
        // void clock clears the round within one turn. Honest, never invented.
    }
    Ok(false)
}

/// Claim the forge in state (so sibling doors stand down; LWW settles ties), then forge
/// **synchronously** — the caller is expected to run touch() off the request path (the
/// transport spawns a thread; the worldview tick tolerates the fallback's microseconds
/// and the LLM path only ever runs where a truth or solo claim genuinely landed.
fn claim_and_forge(
    dir: &Path,
    my_node_id: &str,
    mut s: GameState,
    truth: Option<String>,
    now: i64,
) -> Result<()> {
    s.keeper = my_node_id.to_string();
    s.seq += 1;
    s.updated = now;
    game::save(dir, &s)?;
    // Solo: the truth comes from the record, not a human hand.
    let seed = format!("{}|{}", s.id, s.round);
    let truth_line = match truth {
        Some(t) => t,
        None => match solo_truth(dir, &seed) {
            Some(t) => t,
            None => {
                s.status = "done".into();
                s.phase.clear();
                s.verdict =
                    "the mesh has seen nothing worth lying about — live a little first".into();
                s.seq += 1;
                s.updated = now;
                game::save(dir, &s)?;
                return Ok(());
            }
        },
    };
    let (lines, truth_idx, salt, commit) = forge(dir, &s, &truth_line, true);
    // The secret lands durably BEFORE the lines publish — a crash between the two must
    // leave an unrevealable round (voided by the clock), never a revealed lie.
    save_secret(
        dir,
        &RoundSecret {
            game_id: s.id.clone(),
            round: s.round,
            truth_idx,
            salt,
        },
    )?;
    // Guard against the world having moved while we forged: publish only if the round
    // is still ours to publish. A stale forge is discarded, not spoken.
    let Some(mut fresh) = game::load(dir) else {
        return Ok(());
    };
    if fresh.id != s.id
        || fresh.round != s.round
        || fresh.phase != "forging"
        || fresh.keeper != my_node_id
        || fresh.status != "open"
    {
        return Ok(());
    }
    game::publish_round(&mut fresh, lines, &commit, now);
    game::save(dir, &fresh)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let p =
            std::env::temp_dir().join(format!("familiar_changeling_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn the_fallback_bank_forges_two_distinct_lines_deterministically() {
        let a = bank_lines(&BANK_DAY, "changeling-100|1");
        let b = bank_lines(&BANK_DAY, "changeling-100|1");
        assert_eq!(a, b, "same seed, same forgeries — on any door");
        assert_ne!(a[0], a[1], "two changelings, not one twice");
        let c = bank_lines(&BANK_DAY, "changeling-100|2");
        assert_ne!(a, c, "a new round draws new lines");
    }

    #[test]
    fn the_commitment_verifies_and_a_wrong_salt_does_not() {
        let commit = commitment("changeling-100", 1, "abcd", 2);
        assert!(verify("changeling-100", 1, "abcd", 2, &commit));
        assert!(!verify("changeling-100", 1, "abce", 2, &commit));
        assert!(!verify("changeling-100", 1, "abcd", 1, &commit));
        assert!(!verify("changeling-100", 2, "abcd", 2, &commit));
        assert!(
            !verify("changeling-100", 1, "abcd", 2, ""),
            "an empty promise proves nothing"
        );
    }

    #[test]
    fn the_keeper_file_is_written_by_temp_and_rename() {
        let p = dir("keeper_atomic");
        save_secret(
            &p,
            &RoundSecret {
                game_id: "changeling-1".into(),
                round: 1,
                truth_idx: 2,
                salt: "ff".into(),
            },
        )
        .unwrap();
        assert!(
            !p.join(format!("{KEEPER_FILE}.tmp")).exists(),
            "no tmp left behind"
        );
        assert_eq!(load_secret(&p).unwrap().truth_idx, 2);
        let _ = std::fs::remove_dir_all(&p);
    }

    #[test]
    fn a_solo_truth_is_drawn_only_from_shareable_observations() {
        let p = dir("solo_truth");
        // A sensitive-personal reading, an infra self-report, and one honest fact.
        for (actor, action, object) in [
            ("watch:ian", "reports", "heart_rate:elevated"),
            ("familiar", "reports", "theory_quality:0.50"),
            ("host", "sees", "device:GiiweoPrime"),
        ] {
            familiar_kernel::observation::record(
                &p,
                familiar_kernel::observation::Observation::new(
                    actor, action, object, "", "test", 100, 1.0,
                ),
            )
            .unwrap();
        }
        let t = solo_truth(&p, "seed").unwrap();
        assert_eq!(t, "the mesh saw host sees device:GiiweoPrime");
        let empty = dir("solo_truth_none");
        assert!(
            solo_truth(&empty, "seed").is_none(),
            "nothing safe → no game, no leak"
        );
        let _ = std::fs::remove_dir_all(&p);
        let _ = std::fs::remove_dir_all(&empty);
    }

    #[test]
    fn a_forge_places_the_truth_where_the_commitment_says() {
        let p = dir("forge_places");
        let mut state = None;
        let act = game::GameAct {
            act: "begin".into(),
            kind: Some(GameKind::Changeling),
            text: String::new(),
            to: String::new(),
            turn_secs: None,
            solo: false,
        };
        let players = vec![
            game::Player {
                node_id: "n0".into(),
                label: "human 0".into(),
                handle: "h0".into(),
                devices: vec!["n0".into()],
                score: 0,
                strikes: 0,
                eliminated: false,
            },
            game::Player {
                node_id: "n1".into(),
                label: "human 1".into(),
                handle: "h1".into(),
                devices: vec!["n1".into()],
                score: 0,
                strikes: 0,
                eliminated: false,
            },
        ];
        game::apply_act(&mut state, &act, "h0", "console", &players, 1000).unwrap();
        let s = state.unwrap();
        let (lines, idx, salt, commit) = forge(&p, &s, "my true line", false);
        assert_eq!(lines[idx as usize], "my true line");
        assert!(verify(&s.id, s.round, &salt, idx, &commit));
        assert!(lines.iter().filter(|l| *l == "my true line").count() == 1);
        let _ = std::fs::remove_dir_all(&p);
    }

    #[test]
    fn a_stale_forge_result_is_discarded_not_published() {
        let p = dir("stale_forge");
        let players = vec![game::Player {
            node_id: "n0".into(),
            label: "human 0".into(),
            handle: "h0".into(),
            devices: vec!["n0".into()],
            score: 0,
            strikes: 0,
            eliminated: false,
        }];
        let mut state = None;
        let act = game::GameAct {
            act: "begin".into(),
            kind: Some(GameKind::Changeling),
            text: String::new(),
            to: String::new(),
            turn_secs: None,
            solo: true,
        };
        game::apply_act(&mut state, &act, "h0", "console", &players, 1000).unwrap();
        game::save(&p, state.as_ref().unwrap()).unwrap();
        familiar_kernel::observation::record(
            &p,
            familiar_kernel::observation::Observation::new(
                "host",
                "sees",
                "device:GiiweoPrime",
                "",
                "test",
                100,
                1.0,
            ),
        )
        .unwrap();
        // Door A claims and forges…
        assert!(touch(&p, "door-a", None, 1001).unwrap());
        let s = game::load(&p).unwrap();
        assert_eq!(s.phase, "voting", "the solo forge published");
        // …and a re-touch by another door changes nothing: the round is claimed & live.
        assert!(!touch(&p, "door-b", None, 1002).unwrap());
        let _ = std::fs::remove_dir_all(&p);
    }
}
