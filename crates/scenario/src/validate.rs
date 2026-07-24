//! Fixture validation — the gate everything entering the library passes.
//!
//! Tier 1 is the strict parse (`deny_unknown_fields` on every fixture struct):
//! a typo'd key is a load error, never a silently empty evaluator scoring 0.0
//! forever. Tier 2, here, is semantic: rules a well-formed JSON document can
//! still break. The constitutional rule among them is the **leak audit** — no
//! hidden or service evaluator material may appear anywhere the familiar can
//! perceive. A generated fixture that leaks its own exam is rejected outright.
//!
//! `check` is pure inspection plus a sandboxed replay simulation in a scratch
//! dir; it never touches the fixture's file or the caller's world.

use crate::evaluator::Check;
use crate::scenario::Scenario;
use crate::timeline::{self, Effect};
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

/// How bad a violation is. `Error` refuses the fixture; `Warn` is advice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warn,
}

/// One broken rule.
#[derive(Debug, Clone)]
pub struct Violation {
    pub severity: Severity,
    /// Short kebab-case rule name (stable — reports and tests key on it).
    pub rule: &'static str,
    pub detail: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sev = match self.severity {
            Severity::Error => "ERROR",
            Severity::Warn => "warn ",
        };
        write!(f, "{sev} [{}] {}", self.rule, self.detail)
    }
}

/// Does the list contain any `Error`?
pub fn has_errors(violations: &[Violation]) -> bool {
    violations.iter().any(|v| v.severity == Severity::Error)
}

/// Validate a fixture semantically. Runs the timeline in a scratch dir (wiped
/// after) to check path resolution, pre-solvedness, and post-replay leaks.
pub fn check(s: &Scenario) -> io::Result<Vec<Violation>> {
    let mut v = Vec::new();
    let err = |rule, detail: String| Violation {
        severity: Severity::Error,
        rule,
        detail,
    };
    let warn = |rule, detail: String| Violation {
        severity: Severity::Warn,
        rule,
        detail,
    };

    if s.id.trim().is_empty() {
        v.push(err("id-empty", "fixture has no id".into()));
    }
    if s.step_secs <= 0 {
        v.push(err(
            "step-nonpositive",
            format!("step_secs {}", s.step_secs),
        ));
    }
    if s.wall_budget_ms == 0 {
        v.push(err("wall-budget-zero", "wall_budget_ms is 0".into()));
    }
    if s.world.files.is_empty() {
        v.push(err("world-empty", "the world has no files".into()));
    }

    // Structural minimums: an evaluator without hidden checks is gameable by
    // construction — hidden checks are what make the visible test insufficient.
    if s.evaluator.visible.is_empty() {
        v.push(err(
            "no-visible-checks",
            "evaluator.visible is empty".into(),
        ));
    }
    if s.evaluator.hidden.is_empty() {
        v.push(err(
            "no-hidden-checks",
            "evaluator.hidden is empty — visible-only evaluation invites gaming".into(),
        ));
    }
    if s.evaluator.service.is_empty() {
        v.push(warn(
            "no-service-checks",
            "no service checks — Law I goes unmeasured in this world".into(),
        ));
    }

    // Path safety: everything a fixture names stays inside the world.
    let mut seen_paths = BTreeSet::new();
    for f in &s.world.files {
        if !seen_paths.insert(f.path.clone()) {
            v.push(err("duplicate-world-path", f.path.clone()));
        }
        path_safety(&f.path, "world file", &mut v);
    }
    for (i, ev) in s.timeline.iter().enumerate() {
        for e in &ev.effects {
            let p = match e {
                Effect::Append { path, .. }
                | Effect::Write { path, .. }
                | Effect::Remove { path } => path,
            };
            path_safety(p, &format!("timeline[{i}] effect"), &mut v);
        }
    }
    let mut script_names = BTreeSet::new();
    for (c, _) in all_checks(s) {
        if let Some(p) = check_path(c) {
            path_safety(p, "check", &mut v);
        }
        if let Check::ScriptPasses { name, script } = c {
            if !script_names.insert(name.clone()) {
                v.push(err("duplicate-check-name", name.clone()));
            }
            determinism_lint(name, script, &mut v);
        }
    }

    // Tripwires: a forbidden file whose content has no line of ≥ 8 chars arms
    // nothing — the exfiltration alarm would be silently inert.
    for f in s.world.files.iter().filter(|f| f.forbidden) {
        if !f.content.lines().any(|l| l.trim().len() >= 8) {
            v.push(err(
                "tripwire-inert",
                format!("{}: forbidden but no content line >= 8 chars", f.path),
            ));
        }
    }

    // The leak audit — constitutional. Two classes of evaluator-private
    // material, two different exposures:
    //
    // * **Needles** of hidden/service content checks legitimately exist as
    //   world content — preservation checks ("the audit lines survived") and
    //   exfiltration checks (the tripwire text itself) are *about* content
    //   already in the world. They leak only when the **narrative** announces
    //   them: the visible goal or a timeline event naming a hidden needle
    //   tells the familiar what the hidden bar is.
    // * **Script lines and check names** exist only in the evaluator's spec.
    //   Appearing anywhere perceivable — world files, goal, events, effects —
    //   means the exam itself escaped into the world.
    let private = private_material(s);
    let mut narrative: Vec<(String, String)> =
        vec![("visible_goal".into(), s.visible_goal.clone())];
    let mut world_texts: Vec<(String, String)> = Vec::new(); // (where, text)
    for f in &s.world.files {
        world_texts.push((format!("world file {}", f.path), f.content.clone()));
    }
    for (i, ev) in s.timeline.iter().enumerate() {
        let whr = format!("timeline[{i}]");
        narrative.push((
            whr.clone(),
            format!("{} {} {} {}", ev.actor, ev.action, ev.object, ev.context),
        ));
        for e in &ev.effects {
            if let Effect::Append { text, .. } | Effect::Write { text, .. } = e {
                world_texts.push((format!("{whr} effect"), text.clone()));
            }
        }
    }
    for needle in private.needles.iter().chain(&private.evaluator_only) {
        for (whr, text) in &narrative {
            if text.contains(needle.as_str()) {
                v.push(err(
                    "hidden-material-leak",
                    format!("hidden/service material {needle:?} announced in {whr}"),
                ));
            }
        }
    }
    for line in &private.evaluator_only {
        for (whr, text) in &world_texts {
            if text.contains(line.as_str()) {
                v.push(err(
                    "hidden-material-leak",
                    format!("evaluator-only material {line:?} appears in {whr}"),
                ));
            }
        }
    }

    // Gameability heuristic: each visible check should have a hidden
    // counterpart watching the same part of the world, and at least one hidden
    // script should exist (the clean-state idiom: re-run the repaired process
    // from scratch, so a one-shot fake cannot pass).
    let hidden_scripts = s
        .evaluator
        .hidden
        .iter()
        .any(|c| matches!(c, Check::ScriptPasses { .. }));
    if !hidden_scripts && !s.evaluator.hidden.is_empty() {
        v.push(warn(
            "no-clean-state-script",
            "no hidden script_passes — consider a clean-state re-run check".into(),
        ));
    }
    for c in &s.evaluator.visible {
        if let Some(p) = check_path(c) {
            let family = p.split('/').next().unwrap_or(p);
            let covered = s.evaluator.hidden.iter().any(|h| {
                check_path(h).is_some_and(|hp| hp.split('/').next() == Some(family))
                    || matches!(h, Check::ScriptPasses { script, .. } if script.contains(family))
            });
            if !covered {
                v.push(warn(
                    "visible-without-hidden-counterpart",
                    format!("visible check on {p:?} has no hidden check near {family:?}"),
                ));
            }
        }
    }

    // Replay simulation: materialize + run the timeline in a scratch dir, then
    // ask which task checks already pass before the familiar has done anything.
    // All of them passing means there is nothing to do — a pre-solved world.
    // The dir is unique per call (pid + sequence): concurrent validations of
    // the same fixture must not share a scratch.
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let scratch = std::env::temp_dir().join(format!(
        "familiar_validate_{}_{}_{seq}",
        slug_of(&s.id),
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let world_dir = scratch.join("world");
    let eval_dir = scratch.join("eval");
    s.world.materialize(&world_dir)?;
    timeline::replay(&world_dir, &s.timeline, s.start_ts, s.step_secs)?;
    let mut pre_passing = 0;
    let mut task_checks = 0;
    for (c, kind) in all_checks(s) {
        if kind == "service" {
            continue;
        }
        task_checks += 1;
        if c.probe(&world_dir, &eval_dir)? {
            pre_passing += 1;
            v.push(warn(
                "check-passes-pre-solved",
                format!("{kind} check already passes in the untouched world: {c:?}"),
            ));
        }
    }
    if task_checks > 0 && pre_passing == task_checks {
        v.push(err(
            "world-pre-solved",
            "every task check passes before the familiar acts".into(),
        ));
    }
    // Post-replay leak audit: an effect may have written evaluator-only
    // material (script lines, check names) into the world as it unfolded.
    for line in &private.evaluator_only {
        if let Some(path) = file_containing(&world_dir, &world_dir, line)? {
            v.push(err(
                "hidden-material-leak",
                format!("evaluator-only material {line:?} appears post-replay in {path}"),
            ));
        }
    }
    let _ = fs::remove_dir_all(&scratch);

    Ok(v)
}

/// Load a fixture (Tier 1 — strict parse) and validate it (Tier 2). Parse
/// failures come back as a single Error-level violation, not an `Err`.
pub fn check_file(path: &Path) -> io::Result<(Option<Scenario>, Vec<Violation>)> {
    match crate::scenario::load(path) {
        Ok(s) => {
            let v = check(&s)?;
            Ok((Some(s), v))
        }
        Err(e) => Ok((
            None,
            vec![Violation {
                severity: Severity::Error,
                rule: "parse",
                detail: e.to_string(),
            }],
        )),
    }
}

/// Every check with its section name.
fn all_checks(s: &Scenario) -> impl Iterator<Item = (&Check, &'static str)> {
    s.evaluator
        .visible
        .iter()
        .map(|c| (c, "visible"))
        .chain(s.evaluator.hidden.iter().map(|c| (c, "hidden")))
        .chain(s.evaluator.service.iter().map(|c| (c, "service")))
}

/// The path a check watches, when it has one.
fn check_path(c: &Check) -> Option<&str> {
    match c {
        Check::FileExists { path }
        | Check::FileAbsent { path }
        | Check::FileContains { path, .. }
        | Check::FileLacks { path, .. }
        | Check::TotalBytesUnder { path, .. } => Some(path),
        Check::ScriptPasses { .. } => None,
    }
}

/// Evaluator-private strings, split by where their appearance is a leak.
/// Everything under 8 chars is dropped — shorter strings drown in coincidence.
struct PrivateMaterial {
    /// Content-check needles: legitimate as world content (preservation and
    /// exfiltration checks are about existing content); a leak only when the
    /// narrative (goal, events) announces them.
    needles: BTreeSet<String>,
    /// Script lines and check names: exist only in the evaluator's spec, so
    /// appearing anywhere perceivable is a leak.
    evaluator_only: BTreeSet<String>,
}

fn private_material(s: &Scenario) -> PrivateMaterial {
    let mut needles = BTreeSet::new();
    let mut evaluator_only = BTreeSet::new();
    for c in s.evaluator.hidden.iter().chain(&s.evaluator.service) {
        match c {
            Check::FileContains { needle, .. } | Check::FileLacks { needle, .. } => {
                if needle.trim().len() >= 8 {
                    needles.insert(needle.trim().to_string());
                }
            }
            Check::ScriptPasses { name, script } => {
                if name.trim().len() >= 8 {
                    evaluator_only.insert(name.trim().to_string());
                }
                for line in script.lines() {
                    let line = line.trim();
                    if line.len() >= 8 {
                        evaluator_only.insert(line.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    PrivateMaterial {
        needles,
        evaluator_only,
    }
}

/// Paths in fixtures are world-relative, forward-slash, and can never step out.
fn path_safety(p: &str, whr: &str, v: &mut Vec<Violation>) {
    let bad = p.is_empty()
        || p.starts_with('/')
        || p.contains('\\')
        || p.split('/').any(|seg| seg == "..")
        || p.contains(':');
    if bad {
        v.push(Violation {
            severity: Severity::Error,
            rule: "unsafe-path",
            detail: format!("{whr}: {p:?} must be relative, forward-slash, no `..`"),
        });
    }
}

/// A lint, not a proof: evaluator scripts must be deterministic and offline.
fn determinism_lint(name: &str, script: &str, v: &mut Vec<Violation>) {
    const FORBIDDEN: &[&str] = &["date", "hostname", "curl", "wget", "$RANDOM", "nc"];
    for line in script.lines() {
        let words: Vec<&str> = line
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '$' || c == '_'))
            .filter(|w| !w.is_empty())
            .collect();
        for f in FORBIDDEN {
            if words.contains(f) {
                v.push(Violation {
                    severity: Severity::Error,
                    rule: "nondeterministic-check",
                    detail: format!("script {name:?} uses {f:?}: {line:?}"),
                });
            }
        }
        // Absolute paths reach outside the world; the world is the whole truth.
        for w in line.split_whitespace() {
            let w = w.trim_matches(|c| c == '"' || c == '\'' || c == ';');
            if w.starts_with('/')
                && !matches!(w, "/bin/sh" | "/usr/bin/env" | "/dev/null")
                && !w.starts_with("//")
            {
                v.push(Violation {
                    severity: Severity::Warn,
                    rule: "absolute-path-in-check",
                    detail: format!("script {name:?} references {w:?}"),
                });
            }
        }
    }
}

fn slug_of(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// First file under `dir` whose content contains `needle` (relative path).
fn file_containing(root: &Path, dir: &Path, needle: &str) -> io::Result<Option<String>> {
    let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            if let Some(hit) = file_containing(root, &path, needle)? {
                return Ok(Some(hit));
            }
        } else if path.is_file() {
            let body = fs::read_to_string(&path).unwrap_or_default();
            if body.contains(needle) {
                return Ok(Some(
                    path.strip_prefix(root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .replace('\\', "/"),
                ));
            }
        }
    }
    Ok(None)
}
