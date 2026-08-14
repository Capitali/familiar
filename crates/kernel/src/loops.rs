//! Loop detection — the temporal view of the observation log.
//!
//! A loop is reality repeating itself: the same `actor · action · object` triple
//! recurring. Detection groups observations by that key; each group of two or more
//! is a loop. Faithful port of v1's `loop.c`. Pure: a function of the observations,
//! so persisting it is a rewrite, not an append.

use crate::observation::Observation;
use crate::store;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

/// The detected-loops file (rewritten on each detect, never appended).
pub const LOOPS_FILE: &str = "loops.jsonl";

/// A recurring pattern across observations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Loop {
    pub id: String,
    pub name: String,
    pub description: String,
    pub loop_type: String,
    /// Comma-separated observation ids.
    pub observation_ids: String,
    pub observation_count: i32,
    pub first_seen: i64,
    pub last_seen: i64,
    /// Share of all observations belonging to this loop.
    pub recurrence_score: f64,
    pub friction_score: f64,
    pub opportunity_score: f64,
    pub confidence: f64,
}

/// FNV-1a — a small, deterministic, dependency-free hash so a loop keeps a stable
/// id across detection passes (candidate→loop links must stay consistent).
fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn loop_key(o: &Observation) -> String {
    format!("{}|{}|{}", o.actor, o.action, o.object)
}

/// Detect loops: group observations by `actor|action|object`, keep groups of ≥2.
/// Pure — no I/O. Returns loops in stable key order.
pub fn detect(obs: &[Observation]) -> Vec<Loop> {
    // group indices by key, preserving determinism via BTreeMap (sorted by key)
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, o) in obs.iter().enumerate() {
        groups.entry(loop_key(o)).or_default().push(i);
    }
    let total = obs.len().max(1) as f64;

    let mut out = Vec::new();
    for (key, idxs) in groups {
        let n = idxs.len();
        if n < 2 {
            continue;
        }
        let first = &obs[idxs[0]];
        let ids: Vec<&str> = idxs.iter().map(|&i| obs[i].id.as_str()).collect();
        let first_seen = idxs.iter().map(|&i| obs[i].ts).min().unwrap_or(0);
        let last_seen = idxs.iter().map(|&i| obs[i].ts).max().unwrap_or(0);
        let confidence = if n >= 5 { 1.0 } else { n as f64 * 0.2 };
        out.push(Loop {
            id: format!("loop-{:012x}", fnv1a(&key) & 0xffff_ffff_ffff),
            name: format!("{}_{}", first.actor, first.action),
            description: format!("Repeated: {key}"),
            loop_type: "recurrence_loop".to_string(),
            observation_ids: ids.join(","),
            observation_count: n as i32,
            first_seen,
            last_seen,
            recurrence_score: n as f64 / total,
            friction_score: 0.5,
            opportunity_score: 0.5,
            confidence,
        });
    }
    out
}

/// How close two events must land to count as "together", and the floors a pair must
/// clear before it becomes a loop. Deliberately coarse: this lens finds *candidates
/// for a theory*, not truths — the factory's question/trial machinery does the judging.
const COOCCUR_WINDOW_SECS: i64 = 600;
const COOCCUR_MIN_TOGETHER: usize = 3;
const COOCCUR_MIN_RATE: f64 = 0.5;
const COOCCUR_CAP: usize = 12;

/// An event's CLASS for the co-occurrence lens — the shared vocabulary
/// (`obs_class`, dialogue Q7): one classing contract for every lens, versioned so a
/// later scheme never reinterprets what an old record grouped.
fn event_class(o: &Observation) -> String {
    crate::obs_class::class_key(o)
}

/// The second lens (reasoning brief 2026-08-14, A1): event classes that keep happening
/// TOGETHER — within [`COOCCUR_WINDOW_SECS`] — often relative to the rarer side's own
/// rate. Recurrence sees repetition; this sees relation ("the lights change when ian's
/// presence changes"), which recurrence alone can never surface. Pure, deterministic,
/// bounded. The familiar's own events are excluded: its acts echo its rules, and a
/// mind must not theorize about its own reflection.
pub fn detect_cooccurrence(obs: &[Observation]) -> Vec<Loop> {
    // Events: non-familiar, non-narration, in time order (log order is not guaranteed).
    let mut events: Vec<&Observation> = obs
        .iter()
        .filter(|o| o.actor != "familiar" && o.action != "narrated")
        .collect();
    events.sort_by_key(|o| o.ts);
    if events.len() < 4 {
        return Vec::new();
    }
    // Count each class, and each ordered-into-unordered pair landing inside the window.
    let mut singles: BTreeMap<String, usize> = BTreeMap::new();
    for e in &events {
        *singles.entry(event_class(e)).or_default() += 1;
    }
    // (pair key -> (count, contributing observation ids, first_seen, last_seen))
    let mut pairs: BTreeMap<String, (usize, Vec<String>, i64, i64)> = BTreeMap::new();
    for i in 0..events.len() {
        let a = &events[i];
        let ca = event_class(a);
        for b in events.iter().skip(i + 1) {
            if b.ts - a.ts > COOCCUR_WINDOW_SECS {
                break;
            }
            let cb = event_class(b);
            if ca == cb {
                continue; // same behaviour twice is recurrence's business
            }
            let key = if ca < cb {
                format!("{ca}\u{1}{cb}")
            } else {
                format!("{cb}\u{1}{ca}")
            };
            let e = pairs.entry(key).or_insert((0, Vec::new(), a.ts, b.ts));
            e.0 += 1;
            if e.1.len() < 24 {
                e.1.push(a.id.clone());
                e.1.push(b.id.clone());
            }
            e.2 = e.2.min(a.ts);
            e.3 = e.3.max(b.ts);
        }
    }
    let total = obs.len().max(1) as f64;
    let mut out: Vec<Loop> = Vec::new();
    for (key, (together, ids, first_seen, last_seen)) in pairs {
        if together < COOCCUR_MIN_TOGETHER {
            continue;
        }
        let (ca, cb) = key.split_once('\u{1}').unwrap_or((key.as_str(), ""));
        let na = singles.get(ca).copied().unwrap_or(1);
        let nb = singles.get(cb).copied().unwrap_or(1);
        // The rate is judged against the RARER side: "almost every time the rare thing
        // happens, the other is nearby" is the interesting sentence.
        let rate = together as f64 / na.min(nb).max(1) as f64;
        if rate < COOCCUR_MIN_RATE {
            continue;
        }
        let pretty = crate::obs_class::class_pretty;
        out.push(Loop {
            id: format!(
                "loop-{:012x}",
                fnv1a(&format!("cooccur|{key}")) & 0xffff_ffff_ffff
            ),
            name: format!("together_{:08x}", (fnv1a(&key) & 0xffff_ffff) as u32),
            description: format!(
                "Together: {} ↔ {} ({together}× within {}m; {}/{} of the rarer side)",
                pretty(ca),
                pretty(cb),
                COOCCUR_WINDOW_SECS / 60,
                together,
                na.min(nb)
            ),
            loop_type: "cooccur".to_string(),
            observation_ids: ids.join(","),
            observation_count: together as i32,
            first_seen,
            last_seen,
            recurrence_score: together as f64 / total,
            friction_score: 0.5,
            opportunity_score: 0.6,
            confidence: (together as f64 * 0.2).min(1.0),
        });
    }
    // Strongest first, capped — a flood of weak pairs is noise wearing a lens.
    out.sort_by(|x, y| {
        y.confidence
            .partial_cmp(&x.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(x.id.cmp(&y.id))
    });
    out.truncate(COOCCUR_CAP);
    out
}

/// The triple a loop was built from, recovered from its `description`
/// ("Repeated: actor|action|object"). `(actor, action, object)`, if parseable.
pub fn loop_triple(lp: &Loop) -> Option<(String, String, String)> {
    let rest = lp.description.strip_prefix("Repeated: ")?;
    let mut parts = rest.splitn(3, '|');
    Some((
        parts.next()?.to_string(),
        parts.next()?.to_string(),
        parts.next()?.to_string(),
    ))
}

/// Overwrite the loops file with exactly this set (detection is a pure rewrite).
pub fn save_all(dir: &Path, loops: &[Loop]) -> io::Result<()> {
    store::rewrite(dir, LOOPS_FILE, loops)
}

/// Load the detected loops.
pub fn load(dir: &Path) -> io::Result<Vec<Loop>> {
    store::load(dir, LOOPS_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(id: &str, actor: &str, action: &str, object: &str, ts: i64) -> Observation {
        let mut o = Observation::new(actor, action, object, "", "test", ts, 1.0);
        o.id = id.to_string();
        o
    }

    #[test]
    fn the_lighting_pattern_is_a_computable_relation_not_a_guess() {
        // The pattern that motivated the lens (reasoning brief L2): ian's presence
        // events and ian's light adjustments kept landing together, and recurrence
        // could only see two unrelated repetitions. Four paired episodes, half a day
        // apart, plus uncorrelated noise — the pair must surface, the noise must not.
        let mut data = Vec::new();
        for (i, t0) in [10_000i64, 50_000, 90_000, 130_000].iter().enumerate() {
            data.push(obs(&format!("p{i}"), "ian", "answered", "thread:1", *t0));
            data.push(obs(
                &format!("a{i}"),
                "ian",
                "adjusted",
                "lights=dim",
                t0 + 120,
            ));
        }
        // Noise: a repeated-but-unrelated event far from every pair window.
        for (i, t) in [30_000i64, 70_000, 110_000].iter().enumerate() {
            data.push(obs(&format!("n{i}"), "printer", "reports", "toner", *t));
        }
        let found = detect_cooccurrence(&data);
        assert_eq!(found.len(), 1, "one relation, no noise pairs");
        let lp = &found[0];
        assert_eq!(lp.loop_type, "cooccur");
        assert_eq!(lp.observation_count, 4);
        assert!(lp.description.contains("ian answered thread"));
        assert!(lp.description.contains("ian adjusted lights"));
        // Deterministic across runs — candidate links depend on stable ids.
        assert_eq!(detect_cooccurrence(&data)[0].id, lp.id);
    }

    #[test]
    fn cooccurrence_ignores_the_familiars_own_hand_and_far_apart_events() {
        let mut data = Vec::new();
        // The familiar's acts echo its rules — a mind must not theorize about its
        // own reflection: familiar+human pairs never form.
        for (i, t0) in [10_000i64, 50_000, 90_000].iter().enumerate() {
            data.push(obs(
                &format!("f{i}"),
                "familiar",
                "actuated",
                "lights=dim",
                *t0,
            ));
            data.push(obs(
                &format!("h{i}"),
                "ian",
                "answered",
                "thread:1",
                t0 + 60,
            ));
        }
        assert!(
            detect_cooccurrence(&data).is_empty(),
            "self-echo is not a relation"
        );
        // Two humans' events at similar counts but never inside one window: no pair.
        let mut far = Vec::new();
        for (i, t0) in [10_000i64, 50_000, 90_000].iter().enumerate() {
            far.push(obs(&format!("x{i}"), "ian", "answered", "thread:1", *t0));
            far.push(obs(
                &format!("y{i}"),
                "betty",
                "adjusted",
                "lights=dim",
                t0 + 3600,
            ));
        }
        assert!(detect_cooccurrence(&far).is_empty(), "apart is apart");
    }

    #[test]
    fn groups_recurring_triples_only() {
        let data = vec![
            obs("o1", "client", "asks_for", "report", 100),
            obs("o2", "client", "asks_for", "report", 200), // repeat -> loop
            obs("o3", "host", "reports", "cpu", 150),       // singleton -> no loop
        ];
        let loops = detect(&data);
        assert_eq!(loops.len(), 1);
        let lp = &loops[0];
        assert_eq!(lp.observation_count, 2);
        assert_eq!(lp.first_seen, 100);
        assert_eq!(lp.last_seen, 200);
        assert_eq!(lp.name, "client_asks_for");
        assert_eq!(lp.description, "Repeated: client|asks_for|report");
        assert!((lp.recurrence_score - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn stable_id_across_passes() {
        let a = detect(&[obs("o1", "a", "b", "c", 1), obs("o2", "a", "b", "c", 2)]);
        let b = detect(&[obs("x", "a", "b", "c", 9), obs("y", "a", "b", "c", 10)]);
        assert_eq!(a[0].id, b[0].id); // id derives from the triple, not position
    }

    #[test]
    fn triple_recovers_from_description() {
        let loops = detect(&[
            obs("o1", "betty", "asks_for", "digest", 1),
            obs("o2", "betty", "asks_for", "digest", 2),
        ]);
        assert_eq!(
            loop_triple(&loops[0]),
            Some(("betty".into(), "asks_for".into(), "digest".into()))
        );
    }

    #[test]
    fn confidence_ramps_with_count() {
        let mk = |n: usize| {
            let data: Vec<_> = (0..n)
                .map(|i| obs(&format!("o{i}"), "a", "b", "c", i as i64))
                .collect();
            detect(&data)[0].confidence
        };
        assert!((mk(2) - 0.4).abs() < 1e-9);
        assert_eq!(mk(5), 1.0);
    }
}
