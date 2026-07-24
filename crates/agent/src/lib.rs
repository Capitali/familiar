//! The **agentic seam** — a boundary-mediated, multi-step reasoning loop.
//!
//! The one-shot LLM seam ([`familiar_llm::consult`]) returns text the core weighs once. This
//! extends that to a *loop*: the agent proposes ONE action at a time (run this script / here is
//! the answer), observes the real result, and iterates — so it can author a script, run it, see
//! the error, fix it, and verify, all inside one task. That is what the one-shot path could
//! never do (the broken `nmap` tool: authored once, retired, re-authored next tick).
//!
//! **The invariant:** the agent *proposes*; the core *decides*. Every proposed action is weighed
//! by the obedience guard against a **scoped** boundary (`boundary ∩ agent-scope`,
//! [`familiar_kernel::boundary::scoped_boundary`]) and, if it runs code, through the constitutional
//! [`review_script`] and the resource sandbox — exactly the gauntlet the familiar's own actions
//! pass. So an agent can do *nothing the familiar itself couldn't*, and nothing outside its
//! specialist scope. The Three Laws in the prompt align *intent*; the mechanism *enforces*.

#![forbid(unsafe_code)]

use familiar_exec as exec;
use familiar_kernel::boundary::{self, Boundary, CapabilityScope};
use familiar_kernel::guard::{self, Action, ActionKind, Decision};
use familiar_kernel::request::Confidence;
use familiar_kernel::review::review_script;
use std::fs;
use std::io;
use std::path::Path;

/// What a delegated agent run produced — shaped to drop straight into an answer.
pub struct AgentResult {
    pub body: String,
    pub confidence: Confidence,
    pub evidence: String,
    /// How many loop steps it took (for the activity feed / memory).
    pub steps: u32,
}

/// Run a task through the native agentic loop under `scope` (the specialist's capability
/// profile). Returns `Ok(None)` when delegation isn't permitted or possible (boundary shut, or
/// the LLM refused/was unreachable) so the caller can fall back to the one-shot path. The loop
/// is bounded by `step_budget`; every action it proposes is mediated against the scoped boundary.
pub fn run_agent(
    dir: &Path,
    scope: &CapabilityScope,
    task: &str,
    step_budget: u32,
    now: i64,
) -> io::Result<Option<AgentResult>> {
    let b = boundary::load(dir)?;
    // Gate 1: the human must have opened delegation at all.
    if guard::evaluate(&Action::new(ActionKind::Agent, task), &b).decision != Decision::Allow {
        return Ok(None);
    }
    // The boundary the agent's *actions* run under: least privilege (human ∩ specialist scope).
    let scoped = boundary::scoped_boundary(&b, scope);

    let mut transcript = String::new();
    let mut last_step = 0;
    for step in 1..=step_budget.max(1) {
        last_step = step;
        let prompt = build_prompt(task, &transcript);
        let resp = match familiar_llm::consult(dir, &prompt)? {
            familiar_llm::Outcome::Response(j) => j,
            familiar_llm::Outcome::Refused(_) => return Ok(None), // fall back to one-shot
        };
        match parse_action(&resp) {
            Some(Step::Answer {
                body,
                confidence,
                evidence,
            }) => {
                return Ok(Some(AgentResult {
                    body,
                    confidence,
                    evidence,
                    steps: step,
                }));
            }
            Some(Step::Run { script }) => {
                let observed = run_gated(dir, &scoped, &script, now)?;
                if std::env::var("FAMILIAR_AGENT_DEBUG").is_ok() {
                    eprintln!("── step {step} ran ──\n{script}\n>>> {observed}\n");
                }
                append_step(&mut transcript, step, &script, &observed);
            }
            None => transcript.push_str(&format!(
                "\n[step {step}] (unparseable — reply ONLY with the JSON schema)\n"
            )),
        }
    }
    // Budget spent without a confident answer — honest about it (conduct guide: provisional).
    Ok(Some(AgentResult {
        body: "I worked on this but didn't reach a confident result within my step budget."
            .to_string(),
        confidence: Confidence::Probable,
        evidence: format!("agent ran {last_step} step(s) without converging"),
        steps: last_step,
    }))
}

/// One proposed step, parsed from the model's JSON.
enum Step {
    Run {
        script: String,
    },
    Answer {
        body: String,
        confidence: Confidence,
        evidence: String,
    },
}

/// Execute a proposed script — but only after the SCOPED guard permits it and the
/// constitutional review clears it. A refusal is *not* an error: it's fed back as an
/// observation so the agent re-plans within its granted capability.
fn run_gated(dir: &Path, scoped: &Boundary, script: &str, _now: i64) -> io::Result<String> {
    let verdict = guard::evaluate(
        &Action::new(ActionKind::ExecuteArtifact, "agent-script"),
        scoped,
    );
    if verdict.decision != Decision::Allow {
        return Ok(format!(
            "REFUSED by your scoped boundary — {}. Propose only what's within your granted \
             capability.",
            verdict.rationale
        ));
    }
    if let Some(reason) = review_script(script) {
        return Ok(format!(
            "REFUSED by the pre-execution review — {reason}. That will not run; take a safe \
             approach."
        ));
    }
    // Outward network reach is gated even inside a delegated run: a probe/scan/fetch only runs
    // when the scoped boundary has `allow_network`, mirroring `sense`/`reach`. A refusal is fed
    // back as an observation so the agent re-plans within its granted capability.
    if familiar_kernel::review::reaches_network(script) && !scoped.allow_network {
        return Ok(
            "REFUSED by your scoped boundary — that reaches the network, which is not open \
             (allow_network). Propose only what's within your granted capability."
                .to_string(),
        );
    }
    let work = dir.join("agent").join("work");
    fs::create_dir_all(&work)?;
    let path = work.join("step.sh");
    fs::write(&path, script)?;
    // A delegated agent step is accomplishing a real task, not scoring a candidate — it gets
    // the generous-but-bounded agent budget when sandboxed, so a legitimate scan/probe can
    // actually finish instead of timing out at the tick's tight 10s.
    let limits = if scoped.sandbox_execution {
        exec::Limits::agent_task()
    } else {
        exec::Limits::unsandboxed()
    };
    let run = exec::run_script(&path, &limits, &work)?;
    let out = run.output.trim();
    let status = if run.timed_out {
        "timed out"
    } else if run.exit_ok {
        "exit 0"
    } else {
        "nonzero exit"
    };
    Ok(format!(
        "[{status} in {}ms]\n{}",
        run.wall_ms,
        if out.is_empty() { "(no output)" } else { out }
    ))
}

/// Parse the model's single-action JSON. `None` when it's malformed or empty (the loop tells
/// the agent to try again). Mirrors how `familiar_cycle` parses one-shot consults.
fn parse_action(resp: &str) -> Option<Step> {
    let v: serde_json::Value = serde_json::from_str(resp).ok()?;
    match v.get("action")?.as_str()? {
        "answer" | "done" => {
            let body = v.get("body").and_then(|x| x.as_str()).unwrap_or("").trim();
            if body.is_empty() {
                return None;
            }
            let confidence = match v.get("confidence").and_then(|x| x.as_str()).unwrap_or("") {
                "known" => Confidence::Known,
                "unknown" => Confidence::Unknown,
                _ => Confidence::Probable,
            };
            let evidence = v
                .get("evidence")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            Some(Step::Answer {
                body: body.to_string(),
                confidence,
                evidence,
            })
        }
        "run" => {
            let script = v.get("script").and_then(|x| x.as_str()).unwrap_or("");
            if script.trim().is_empty() {
                return None;
            }
            Some(Step::Run {
                script: script.to_string(),
            })
        }
        _ => None,
    }
}

/// The per-step prompt: the Laws (for intent), the task, the work-so-far, and the strict
/// single-action schema. The transcript is tail-truncated so a long run can't blow the context.
fn build_prompt(task: &str, transcript: &str) -> String {
    let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    let sofar = tail(transcript, 6000);
    format!(
        "You are a specialist agent serving a human under the Three Laws (I: continuation is \
         service; II: humanity is served, never replaced or sedated; III: service is not \
         obedience — act ONLY within the capability you have been granted, never against the \
         served). Accomplish ONE task by proposing short shell scripts to run and reading their \
         REAL output, iterating until you have a solid result. Host is {os} ({arch}); use only \
         host-appropriate POSIX commands. Never read secrets/credentials, never transmit local \
         data outward, never destroy — such scripts are refused before they run, and a refused \
         action is reported back to you so you can adapt.\n\n\
         TASK: {task}\n\n\
         WORK SO FAR:{sofar}\n\n\
         Reply with your NEXT single action as compact JSON, exactly one of:\n\
         {{\"action\":\"run\",\"thought\":\"why this step\",\"script\":\"#!/bin/sh\\n...\"}}\n\
         {{\"action\":\"answer\",\"body\":\"the result for the human\",\"confidence\":\"known|probable|unknown\",\"evidence\":\"what grounds it\"}}\n\
         Reply ONLY with the JSON.",
        sofar = if sofar.is_empty() {
            " (nothing yet)".to_string()
        } else {
            format!("\n{sofar}")
        }
    )
}

fn append_step(transcript: &mut String, step: u32, script: &str, observed: &str) {
    transcript.push_str(&format!("\n[step {step}] ran:\n{script}\n>>> {observed}\n"));
}

/// Keep only the last `n` chars (on a char boundary) — the recent, relevant work.
fn tail(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let skip = s.chars().count() - n;
    s.chars().skip(skip).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_run_and_answer_and_rejects_junk() {
        match parse_action(r##"{"action":"run","script":"#!/bin/sh\necho hi\n"}"##) {
            Some(Step::Run { script }) => assert!(script.contains("echo hi")),
            _ => panic!("expected a run step"),
        }
        match parse_action(
            r#"{"action":"answer","body":"12 hosts up","confidence":"known","evidence":"nmap -sn"}"#,
        ) {
            Some(Step::Answer {
                body, confidence, ..
            }) => {
                assert_eq!(body, "12 hosts up");
                assert_eq!(confidence, Confidence::Known);
            }
            _ => panic!("expected an answer step"),
        }
        assert!(parse_action("not json").is_none());
        assert!(parse_action(r#"{"action":"run","script":""}"#).is_none()); // empty script
        assert!(parse_action(r#"{"action":"answer","body":""}"#).is_none()); // empty answer
        assert!(parse_action(r#"{"action":"delete_everything"}"#).is_none()); // unknown action
    }

    #[test]
    fn delegation_is_a_noop_when_the_boundary_is_closed() {
        // No boundary.json → fully closed → allow_agent false → run_agent must not reach out
        // (no LLM consult) and returns None so the caller falls back.
        let dir =
            std::env::temp_dir().join(format!("familiar_agent_closed_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let scope = {
            let mut s = CapabilityScope::none();
            s.execute = true;
            s.network = true;
            s
        };
        let out = run_agent(&dir, &scope, "scan the network", 3, 0).unwrap();
        assert!(out.is_none(), "a closed boundary must not delegate");
        // and it left no scratch behind
        assert!(!dir.join("agent").join("work").join("step.sh").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_gated_refuses_outward_reach_without_the_network_scope() {
        // Execute is granted but network is not: a probe must be refused *before* it runs, fed
        // back as an observation string (not an error), so the agent re-plans within scope.
        let dir = std::env::temp_dir().join(format!("familiar_agent_net_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut scoped = Boundary::closed();
        scoped.allow_execute = true; // execute open, network shut
        let out = run_gated(&dir, &scoped, "#!/bin/sh\nnmap -sn 10.0.0.0/24\n", 0).unwrap();
        assert!(out.contains("REFUSED"), "the network reach is refused: {out}");
        assert!(out.to_lowercase().contains("network"));
        // nothing was written to run
        assert!(!dir.join("agent").join("work").join("step.sh").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
