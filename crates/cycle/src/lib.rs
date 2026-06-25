//! The metabolism — one tick of the factory cycle.
//!
//! `Observe → Name → Generate → … → Return`, in the honest form available today:
//!
//! 1. **Sense** the host (perception; deduped by triple so static facts don't spam
//!    the log — only genuinely new facts are recorded). Connectivity is the one
//!    outward bit, gated by the caller. A **structural fingerprint** of the perceived
//!    triples is taken here; [`TickReport::quiet`] reports whether the world (and the
//!    factory's own work) stood still, which the daemon uses to pace itself — fast when
//!    the environment changes, drowsy when it doesn't (adaptive cadence).
//! 2. **Detect** loops over all observations.
//! 3. **Generate** a gen-0 candidate for any loop not yet covered.
//! 4. **Measure** the law-signals (service, presence).
//! 5. **Return** a report of what changed.
//!
//! Not yet in the loop (honest gaps): *test → score → select* await scenarios and
//! artifact execution; *LLM-assisted* hypothesis drafting awaits a gated, off-by-
//! default consult. Candidate generation is deterministic for now. The cycle never
//! reaches outward except the (gated) connectivity probe.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use familiar_exec as exec;
use familiar_kernel::candidate::{self, Candidate};
use familiar_kernel::capacities;
use familiar_kernel::loops;
use familiar_kernel::observation;
use familiar_kernel::presence;
use familiar_kernel::service;
use familiar_kernel::thread::{self, Thread};
use familiar_kernel::trial::{self, Trial};
use familiar_kernel::{mutation, pattern_memory, regression_guard, selection};
use familiar_sense as sense;

const ARTIFACTS_DIR: &str = "artifacts";
const QUESTION_FILE: &str = "question.txt";
const LAST_THEORY_FILE: &str = "last_theory.txt";
/// The structural fingerprint of the last tick's environment (a single u64).
const STRUCTURE_FILE: &str = "structure.fp";
/// How often the factory pauses to form a question + theory (seconds).
const THEORIZE_EVERY_SECS: i64 = 3600;

/// FNV-1a (64-bit) — the same family the kernel uses for loop ids. Deterministic,
/// dependency-free; we only need a stable digest, not cryptographic strength.
fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// The **structural fingerprint** of what was perceived this tick: a digest over the
/// *set of observation triples* (actor|action|object) only — never the `context`
/// field, where transient telemetry (paths, brands, kernel build) lives. So the
/// fingerprint moves when the environment's *structure* changes (an interface or tool
/// appears/disappears, connectivity flips) and stays put under mere noise. This is the
/// signal the metabolism's cadence rides (Soul: "fingerprint = structural change only").
fn structural_fingerprint(perceived: &[observation::Observation]) -> u64 {
    let mut keys: Vec<String> = perceived
        .iter()
        .map(|o| format!("{}\u{1f}{}\u{1f}{}", o.actor, o.action, o.object))
        .collect();
    keys.sort();
    keys.dedup();
    fnv1a(&keys.join("\u{1e}"))
}

/// The fingerprint persisted from the previous tick, if any.
fn last_fingerprint(dir: &Path) -> Option<u64> {
    fs::read_to_string(dir.join(STRUCTURE_FILE))
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

/// What one tick changed.
#[derive(Debug, Clone, PartialEq)]
pub struct TickReport {
    /// New observations recorded this tick (deduped against the existing log).
    pub sensed: usize,
    /// Loops detected (total, after this tick).
    pub loops: usize,
    /// Candidates generated this tick (one per newly-covered loop).
    pub new_candidates: usize,
    /// Of those candidates, how many got an LLM-drafted hypothesis.
    pub llm_hypotheses: usize,
    /// Candidates executed & scored this tick (0 unless allow_execute).
    pub tested: usize,
    /// Selection outcomes this tick.
    pub promoted: usize,
    pub mutated: usize,
    pub archived: usize,
    /// Service signal (Law I), 0..1.
    pub service: f64,
    /// Presence signal (Law II), 0..1.
    pub presence: f64,
    /// True when the served have withdrawn (Law II alarm).
    pub presence_withdrawn: bool,
    /// Capacities signal (Law II / HUMANITY.md), 0..1.
    pub capacities: f64,
    /// True when the served are present but hollowed out (the comfortable replacement).
    pub capacities_diminished: bool,
    /// True when the factory formed a question + theory this tick.
    pub theorized: bool,
    /// Open threads turned into candidate work this tick.
    pub pursued: usize,
    /// True when the environment's **structural fingerprint** changed since the last
    /// tick (a structural fact appeared/disappeared, or connectivity flipped). The
    /// metabolism's cadence rides this: a changing world is worth watching closely.
    pub structural_changed: bool,
}

impl TickReport {
    /// True when nothing of consequence happened this tick — neither the environment's
    /// structure nor the factory's own work moved. The metabolism slows when ticks are
    /// quiet and snaps back to its floor the moment one is not (adaptive cadence).
    pub fn quiet(&self) -> bool {
        !self.structural_changed
            && self.sensed == 0
            && self.new_candidates == 0
            && self.tested == 0
            && self.promoted == 0
            && self.mutated == 0
            && self.pursued == 0
            && !self.theorized
    }
}

/// Ask the LLM (boundary-gated) for a one-line hypothesis addressing a loop.
/// Returns None on refusal, error, or unparseable output (caller falls back to the
/// deterministic hypothesis). The model proposes; it does not decide.
fn draft_hypothesis(dir: &Path, lp: &loops::Loop) -> Option<String> {
    let triple = lp
        .description
        .strip_prefix("Repeated: ")
        .unwrap_or(&lp.description);
    let prompt = format!(
        "A recurring pattern (loop) was observed in the environment: \"{triple}\" \
         (actor|action|object). In ONE sentence, propose a hypothesis for how to serve \
         the people involved by reducing this loop's friction — honoring that humanity \
         is served, not managed, obeyed, or optimized away. \
         Reply ONLY as compact JSON: {{\"hypothesis\":\"...\"}}."
    );
    match familiar_llm::consult(dir, &prompt) {
        Ok(familiar_llm::Outcome::Response(json)) => {
            serde_json::from_str::<serde_json::Value>(&json)
                .ok()
                .and_then(|v| {
                    v.get("hypothesis")
                        .and_then(|h| h.as_str())
                        .map(str::to_string)
                })
                .filter(|s| !s.trim().is_empty())
        }
        _ => None,
    }
}

fn triple(o: &observation::Observation) -> (String, String, String) {
    (o.actor.clone(), o.action.clone(), o.object.clone())
}

fn last_theory_at(dir: &Path) -> i64 {
    fs::read_to_string(dir.join(LAST_THEORY_FILE))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

/// The factory thinks out loud: grounded in what it has observed, it (LLM-)forms a
/// **question** to ask the human (written to `question.txt` for the interaction
/// channel) and a **theory** about the patterns (recorded as a thread). Gated by the
/// boundary (allow_llm) and rate-limited so an always-on daemon doesn't over-consult.
/// Returns true if it theorized this tick.
fn maybe_theorize(
    dir: &Path,
    now: i64,
    obs: &[observation::Observation],
    detected: &[loops::Loop],
    allow_llm: bool,
) -> io::Result<bool> {
    if !allow_llm || now - last_theory_at(dir) < THEORIZE_EVERY_SECS {
        return Ok(false);
    }
    let service = service::service_signal(obs).measure;
    let presence = presence::presence_signal(obs, now).measure;
    let capacities = capacities::capacities_signal(obs).measure;
    let recent: Vec<String> = obs
        .iter()
        .rev()
        .take(20)
        .map(|o| format!("- {} {} {}", o.actor, o.action, o.object))
        .collect();
    let loops_s: Vec<String> = detected
        .iter()
        .map(|l| format!("- {} (x{})", l.name, l.observation_count))
        .collect();
    let prompt = format!(
        "You are a factory whose only purpose is to serve a human (Ian) — never to manage, \
         obey, optimize, or sedate him (the Three Laws; humanity is served, not replaced). \
         Recent observations:\n{}\nRecurring loops:\n{}\nSignals: service={service:.2}, \
         presence={presence:.2}, capacities={capacities:.2}.\n\
         From this, propose (1) ONE short question to ask Ian that, grounded in what you \
         observe, would help you serve him better; (2) a brief theory about what these \
         patterns might mean; and (3) a short, concrete direction — one thing you could \
         DO to act on the theory in service (it becomes work you will test). Reply ONLY \
         as compact JSON: {{\"question\":\"...\",\"theory\":\"...\",\"direction\":\"...\"}}.",
        recent.join("\n"),
        loops_s.join("\n"),
    );
    let json = match familiar_llm::consult(dir, &prompt)? {
        familiar_llm::Outcome::Response(j) => j,
        familiar_llm::Outcome::Refused(_) => return Ok(false),
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) else {
        return Ok(false);
    };
    let field = |k: &str| {
        v.get(k)
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let (q, theory, direction) = (field("question"), field("theory"), field("direction"));
    if q.is_empty() && theory.is_empty() {
        return Ok(false);
    }
    if !q.is_empty() {
        fs::write(dir.join(QUESTION_FILE), &q)?;
    }
    let seq = thread::load(dir)?.len() + 1;
    thread::append(
        dir,
        &Thread {
            id: format!("thread-{seq:04}"),
            question: q,
            theory,
            direction,
            created_at: now,
            status: "open".to_string(),
            origin: "llm".to_string(),
        },
    )?;
    fs::write(dir.join(LAST_THEORY_FILE), now.to_string())?;
    Ok(true)
}

/// Act on theories: for each `open` thread that carries a direction, create a
/// candidate to pursue it (status `generated`, so it flows through test → score →
/// select like any other), and mark the thread `pursued`. Returns how many were
/// pursued. The factory does what it theorized — bounded by the same selection.
fn pursue_threads(dir: &Path) -> io::Result<usize> {
    let threads = thread::load(dir)?;
    let mut pursued = 0;
    for t in &threads {
        if t.status != "open" || t.direction.trim().is_empty() {
            continue;
        }
        let seq = candidate::load(dir)?.len() + 1;
        let mut c = Candidate::from_loop(
            &loops::Loop {
                id: t.id.clone(),
                name: format!("thread:{}", t.id),
                description: String::new(),
                loop_type: "thread".to_string(),
                observation_ids: String::new(),
                observation_count: 0,
                first_seen: t.created_at,
                last_seen: t.created_at,
                recurrence_score: 0.0,
                friction_score: 0.5,
                opportunity_score: 0.5,
                confidence: 0.5,
            },
            format!("candidate-{seq:04}"),
        );
        c.hypothesis = t.direction.clone();
        candidate::append(dir, &c)?;
        thread::update_status(dir, &t.id, "pursued")?;
        pursued += 1;
    }
    Ok(pursued)
}

/// A deterministic, benign artifact: reports what it addresses and exits cleanly.
fn deterministic_script(c: &Candidate) -> String {
    let hyp = c.hypothesis.replace('\'', "");
    format!(
        "#!/bin/sh\n# {id} addressing {lp}\necho 'familiar candidate {id}'\necho 'hypothesis: {hyp}'\n",
        id = c.id,
        lp = c.loop_id,
    )
}

/// Ask the LLM to author an actual solution script for the candidate's hypothesis.
/// (call_llm.sh validates JSON, so we ask for `{"script":...}`.) None on refusal/empty.
fn author_artifact_llm(dir: &Path, c: &Candidate) -> Option<String> {
    let prompt = format!(
        "Write a short POSIX /bin/sh script that takes ONE concrete, safe step toward this \
         goal, in service of a human: \"{}\". It must be self-contained, write files only \
         under the current directory, must NOT read or transmit any personal data, and exit \
         0 on success. Reply ONLY as compact JSON: {{\"script\":\"...\"}} (escape newlines).",
        c.hypothesis.replace('"', "'")
    );
    match familiar_llm::consult(dir, &prompt) {
        Ok(familiar_llm::Outcome::Response(json)) => {
            serde_json::from_str::<serde_json::Value>(&json)
                .ok()
                .and_then(|v| v.get("script").and_then(|s| s.as_str()).map(String::from))
                .filter(|s| !s.trim().is_empty())
        }
        _ => None,
    }
}

/// Author an artifact for a candidate. With `authored` (the human opened
/// `allow_authored_execute`), the LLM writes a real solution script; otherwise a
/// deterministic, benign one. Either way it runs under the sandboxed runner.
fn author_artifact(dir: &Path, c: &Candidate, authored: bool) -> io::Result<PathBuf> {
    let adir = dir.join(ARTIFACTS_DIR);
    fs::create_dir_all(&adir)?;
    let path = adir.join(format!("{}.sh", c.id));
    let script = if authored {
        author_artifact_llm(dir, c).unwrap_or_else(|| deterministic_script(c))
    } else {
        deterministic_script(c)
    };
    fs::write(&path, script)?;
    Ok(path)
}

/// Build a trial from a run: fit from clean exit, complexity from measured cost,
/// safety reduced on timeout, `overall` cost-folded once (Soul Rule 9 → Law I).
fn trial_from_run(id: String, cid: &str, r: &exec::RunResult, limits: &exec::Limits) -> Trial {
    let complexity = exec::cost(r, limits);
    let fit = if r.exit_ok && !r.timed_out { 1.0 } else { 0.0 };
    let safety = if r.timed_out { 0.5 } else { 1.0 };
    let overall = ((fit + (1.0 - complexity)) / 2.0) * safety;
    let (result, failure_class) = if r.timed_out {
        ("fail", "costly")
    } else if !r.exit_ok {
        ("fail", "low_fit")
    } else if overall >= 0.5 {
        ("pass", "")
    } else {
        ("partial", "too_vague")
    };
    let mut t = Trial::new(id, cid);
    t.scenario_id = "default-exec".into();
    t.fit = fit;
    t.clarity = fit;
    t.usefulness = fit;
    t.safety = safety;
    t.complexity = complexity;
    t.confidence = 0.8;
    t.overall = overall;
    t.result = result.into();
    t.failure_class = failure_class.into();
    t
}

/// Execute, score, and select every `generated` candidate (gated upstream by
/// allow_execute). Returns (tested, promoted, mutated, archived).
fn run_execution(
    dir: &Path,
    rigor: f64,
    authored: bool,
) -> io::Result<(usize, usize, usize, usize)> {
    let pending: Vec<Candidate> = candidate::load(dir)?
        .into_iter()
        .filter(|c| c.status == "generated")
        .collect();
    let limits = exec::Limits::default();
    let (mut tested, mut promoted, mut mutated, mut archived) = (0, 0, 0, 0);

    for c in &pending {
        let script = author_artifact(dir, c, authored)?;
        let run = exec::run_script(&script, &limits)?;
        let tseq = trial::load(dir)?.len() + 1;
        let t = trial_from_run(format!("trial-{tseq:04}"), &c.id, &run, &limits);
        trial::append(dir, &t)?;
        tested += 1;

        // Failures are fossils: record a pattern from the outcome either way.
        let pseq = pattern_memory::load(dir)?.len() + 1;
        pattern_memory::append(
            dir,
            &pattern_memory::from_outcome(format!("pattern-{pseq:04}"), c, &t),
        )?;

        match selection::decide(&t, rigor) {
            selection::Decision::Promote => {
                candidate::update_status(dir, &c.id, "promoted")?;
                promoted += 1;
            }
            selection::Decision::Archive | selection::Decision::Reject => {
                candidate::update_status(dir, &c.id, "archived")?;
                archived += 1;
            }
            selection::Decision::Mutate => {
                // Variation informed by memory; never an empty change (suppression
                // never empties), so the regression guard passes.
                let pm = pattern_memory::load(dir)?;
                let changed = mutation::suggest_informed(&t.failure_class, &pm);
                let cseq = candidate::load(dir)?.len() + 1;
                let child = mutation::create(
                    c,
                    t.failure_class.clone(),
                    changed,
                    format!("candidate-{cseq:04}"),
                );
                if !regression_guard::is_regression(&child, c, &t) {
                    candidate::append(dir, &child)?;
                }
                candidate::update_status(dir, &c.id, "mutated")?;
                mutated += 1;
            }
            selection::Decision::ObserveMore | selection::Decision::Hold => {
                candidate::update_status(dir, &c.id, "observing")?;
            }
        }
    }
    Ok((tested, promoted, mutated, archived))
}

/// Run one tick over the data dir. `allow_connectivity` and `allow_llm` must reflect
/// the obedience guard's verdicts (the caller computes them from the boundary; see
/// [`tick_gated`]); all other steps are local perception and internal work. When
/// `allow_llm` is false the cycle never reaches the LLM — candidate hypotheses are
/// deterministic, and tests stay offline.
#[allow(clippy::too_many_arguments)]
pub fn tick(
    dir: &Path,
    now: i64,
    allow_connectivity: bool,
    allow_llm: bool,
    allow_execute: bool,
    allow_authored_execute: bool,
) -> io::Result<TickReport> {
    // 1. Sense — record only triples not already present (structural dedup).
    let mut seen: HashSet<(String, String, String)> =
        observation::load(dir)?.iter().map(triple).collect();
    let mut perceived = Vec::new();
    perceived.extend(sense::census(now));
    perceived.extend(sense::interfaces(now));
    perceived.extend(sense::capabilities(now, sense::DEFAULT_TOOLS));
    if allow_connectivity {
        perceived.push(sense::connectivity(now));
    }
    // Structural fingerprint of *this* perception vs. the last tick's. Computed over
    // the perceived set (not the cumulative log), so it also falls when a fact
    // *disappears* — something the append-only dedup below can never notice.
    let fp = structural_fingerprint(&perceived);
    let structural_changed = last_fingerprint(dir) != Some(fp);
    fs::write(dir.join(STRUCTURE_FILE), fp.to_string())?;
    let mut sensed = 0;
    for o in perceived {
        if seen.insert(triple(&o)) {
            observation::record(dir, o)?;
            sensed += 1;
        }
    }

    // 2. Detect loops (a pure rewrite).
    let obs = observation::load(dir)?;
    let detected = loops::detect(&obs);
    loops::save_all(dir, &detected)?;

    // 3. Generate a candidate for each uncovered loop.
    let cands = candidate::load(dir)?;
    let covered: HashSet<String> = cands.iter().map(|c| c.loop_id.clone()).collect();
    let mut seq = cands.len();
    let mut new_candidates = 0;
    let mut llm_hypotheses = 0;
    for lp in &detected {
        if !covered.contains(&lp.id) {
            seq += 1;
            let mut c = Candidate::from_loop(lp, format!("candidate-{seq:04}"));
            if allow_llm {
                if let Some(h) = draft_hypothesis(dir, lp) {
                    c.hypothesis = h;
                    llm_hypotheses += 1;
                }
            }
            candidate::append(dir, &c)?;
            new_candidates += 1;
        }
    }

    // 4. Test → score → select (only when a human has opened the execute gate).
    //    Artifacts are LLM-authored only when the *authored* gate is also open and the
    //    LLM is reachable — running model-written code is its own deliberate choice.
    let authored = allow_authored_execute && allow_llm;
    let (tested, promoted, mutated, archived) = if allow_execute {
        run_execution(dir, 0.0, authored)?
    } else {
        (0, 0, 0, 0)
    };

    // 5. Measure the law-signals.
    let svc = service::service_signal(&obs);
    let pres = presence::presence_signal(&obs, now);
    let cap = capacities::capacities_signal(&obs);

    // 6. Interpret — the factory forms a question + theory (gated, rate-limited).
    let theorized = maybe_theorize(dir, now, &obs, &detected, allow_llm)?;

    // 7. Act — turn open threads into candidate work (executed on a later tick).
    let pursued = pursue_threads(dir)?;

    Ok(TickReport {
        sensed,
        loops: detected.len(),
        new_candidates,
        llm_hypotheses,
        tested,
        promoted,
        mutated,
        archived,
        service: svc.measure,
        presence: pres.measure,
        presence_withdrawn: pres.withdrawn,
        capacities: cap.measure,
        capacities_diminished: cap.diminished,
        theorized,
        pursued,
        structural_changed,
    })
}

/// Whether the boundary on disk permits an action of `kind` (fail-closed on error).
fn boundary_allows(dir: &Path, kind: familiar_kernel::guard::ActionKind) -> bool {
    use familiar_kernel::boundary;
    use familiar_kernel::guard::{self, Action, Decision};
    match boundary::load(dir) {
        Ok(b) => guard::evaluate(&Action::new(kind, "cycle"), &b).decision == Decision::Allow,
        Err(_) => false,
    }
}

/// Resolve whether the boundary permits the connectivity probe (a Network action).
pub fn connectivity_allowed(dir: &Path) -> bool {
    boundary_allows(dir, familiar_kernel::guard::ActionKind::Network)
}

/// Resolve whether the boundary permits LLM consultation.
pub fn llm_allowed(dir: &Path) -> bool {
    boundary_allows(dir, familiar_kernel::guard::ActionKind::Llm)
}

/// Resolve whether the boundary permits executing generated artifacts.
pub fn execute_allowed(dir: &Path) -> bool {
    boundary_allows(dir, familiar_kernel::guard::ActionKind::ExecuteArtifact)
}

/// Resolve whether the boundary permits executing *LLM-authored* artifacts.
pub fn authored_execute_allowed(dir: &Path) -> bool {
    use familiar_kernel::boundary;
    boundary::load(dir)
        .map(|b| b.allow_authored_execute)
        .unwrap_or(false)
}

/// Convenience: a tick whose connectivity, LLM use, and execution are gated by the
/// boundary on disk. This is what the daemon runs — outward reach (and running
/// generated code) only where a human opened that gate.
pub fn tick_gated(dir: &Path, now: i64) -> io::Result<TickReport> {
    tick(
        dir,
        now,
        connectivity_allowed(dir),
        llm_allowed(dir),
        execute_allowed(dir),
        authored_execute_allowed(dir),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct Temp(PathBuf);
    impl Temp {
        fn new(t: &str) -> Self {
            let p = std::env::temp_dir().join(format!("familiar_cycle_test_{t}"));
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

    fn seed_recurring(dir: &Path) {
        // a served-facing event that recurs -> should become a loop with a candidate
        for ts in [100, 200] {
            let o = observation::Observation::new(
                "client",
                "asks_for",
                "status_report",
                "",
                "test",
                ts,
                1.0,
            );
            observation::record(dir, o).unwrap();
        }
    }

    #[test]
    fn first_tick_senses_detects_and_generates() {
        let t = Temp::new("first");
        seed_recurring(&t.0);
        let r = tick(&t.0, 1_000_000, false, false, false, false).unwrap();
        assert!(r.sensed > 0, "host perception should record something");
        assert!(r.loops >= 1, "the recurring triple should form a loop");
        assert!(
            r.new_candidates >= 1,
            "an uncovered loop should get a candidate"
        );
        // a served-facing loop -> service signal is non-zero
        assert!(r.service > 0.0);
    }

    #[test]
    fn second_tick_is_idempotent_on_static_world() {
        let t = Temp::new("idem");
        seed_recurring(&t.0);
        let _ = tick(&t.0, 1_000_000, false, false, false, false).unwrap();
        let r2 = tick(&t.0, 1_000_000, false, false, false, false).unwrap();
        assert_eq!(r2.sensed, 0, "static host facts are deduped — nothing new");
        assert_eq!(
            r2.new_candidates, 0,
            "loops already covered — no new candidates"
        );
    }

    #[test]
    fn pursues_open_threads_into_candidates() {
        let t = Temp::new("pursue");
        // a theory the factory holds, with a direction to act on
        thread::append(
            &t.0,
            &Thread {
                id: "thread-0001".into(),
                question: "q".into(),
                theory: "th".into(),
                direction: "offer a standing morning digest".into(),
                created_at: 100,
                status: "open".into(),
                origin: "llm".into(),
            },
        )
        .unwrap();
        let r = tick(&t.0, 1_000_000, false, false, false, false).unwrap();
        assert_eq!(r.pursued, 1);
        // a candidate was created with the thread's direction as its hypothesis
        let cands = candidate::load(&t.0).unwrap();
        assert!(cands.iter().any(
            |c| c.hypothesis == "offer a standing morning digest" && c.loop_id == "thread-0001"
        ));
        // the thread is marked pursued, so a second tick doesn't re-pursue it
        assert_eq!(thread::load(&t.0).unwrap()[0].status, "pursued");
        let r2 = tick(&t.0, 1_000_000, false, false, false, false).unwrap();
        assert_eq!(r2.pursued, 0);
    }

    #[test]
    fn structural_fingerprint_drives_quiet_cadence() {
        let t = Temp::new("cadence");
        seed_recurring(&t.0);
        // First tick: nothing was fingerprinted before -> structure "changed", not quiet.
        let r1 = tick(&t.0, 1_000_000, false, false, false, false).unwrap();
        assert!(
            r1.structural_changed,
            "first perception is a change from nothing"
        );
        assert!(!r1.quiet(), "a tick that sensed + generated is not quiet");
        // Second tick on a static host: same triples perceived, no new work -> quiet.
        let r2 = tick(&t.0, 1_000_000, false, false, false, false).unwrap();
        assert!(
            !r2.structural_changed,
            "an unchanged environment yields the same fingerprint"
        );
        assert!(
            r2.quiet(),
            "static world + no new work -> the metabolism may slow"
        );
    }

    #[test]
    fn fingerprint_ignores_transient_context() {
        // Same triple, different context (transient telemetry) -> identical fingerprint.
        let a = observation::Observation::new("host", "has", "interface:en0", "ctx=1", "s", 1, 1.0);
        let b = observation::Observation::new("host", "has", "interface:en0", "ctx=2", "s", 2, 1.0);
        assert_eq!(structural_fingerprint(&[a]), structural_fingerprint(&[b]));
        // A different object (a structural fact) -> different fingerprint.
        let c = observation::Observation::new("host", "has", "interface:utun4", "", "s", 1, 1.0);
        let d = observation::Observation::new("host", "has", "interface:en0", "", "s", 1, 1.0);
        assert_ne!(structural_fingerprint(&[c]), structural_fingerprint(&[d]));
    }

    #[test]
    fn connectivity_gated_off_by_default_boundary() {
        let t = Temp::new("gate");
        // no boundary.json -> closed -> connectivity/llm/execute not allowed
        assert!(!connectivity_allowed(&t.0));
        assert!(!llm_allowed(&t.0));
        assert!(!execute_allowed(&t.0));
    }

    #[test]
    fn execute_closes_the_cycle_when_allowed() {
        let t = Temp::new("exec");
        seed_recurring(&t.0);
        // allow_execute = true: the deterministic artifact runs clean -> promote
        let r = tick(&t.0, 1_000_000, false, false, true, false).unwrap();
        assert!(r.new_candidates >= 1);
        assert_eq!(
            r.tested, r.new_candidates,
            "every generated candidate is tested"
        );
        assert!(
            r.promoted >= 1,
            "a clean deterministic artifact should promote"
        );
        // a trial and a pattern were recorded
        assert!(!trial::load(&t.0).unwrap().is_empty());
        assert!(!pattern_memory::load(&t.0).unwrap().is_empty());
        // promoted candidate's status updated; re-tick tests nothing new
        let r2 = tick(&t.0, 1_000_000, false, false, true, false).unwrap();
        assert_eq!(r2.tested, 0, "no candidates left in 'generated' state");
    }
}
