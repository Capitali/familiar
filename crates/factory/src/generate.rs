//! The generation seam: where a candidate comes from.
//!
//! A [`GenerationAdapter`] turns a work order (plus, on a retry, the last
//! failure's feedback) into a [`GenerationResult`] — a typed outcome and the
//! artifact bytes its manifest names. The real adapter asks the familiar's
//! reasoner (the consult seam) to write the driver and its tests; a scripted
//! adapter drives the convergence tests without a model. Either way the
//! workshop validates the outcome before it becomes executable — the adapter
//! is never trusted to have produced something valid.

use std::collections::BTreeMap;

use familiar_workshop::manifest::{digest_bytes, FileEntry, FileRole, Manifest};
use familiar_workshop::order::{GenerationOutcome, Refusal, WorkOrder};

/// What an adapter returns: the typed outcome plus the content-addressed
/// artifact store (digest → bytes) for a candidate's files. For a refusal the
/// artifacts map is empty.
#[derive(Debug, Clone)]
pub struct GenerationResult {
    pub outcome: GenerationOutcome,
    pub artifacts: BTreeMap<String, Vec<u8>>,
}

/// The seam the factory generates through. `feedback` is `None` on the first
/// attempt and carries the previous iteration's bench failure on a retry, so
/// the reasoner can fix what the oracle rejected.
pub trait GenerationAdapter {
    fn generate(
        &self,
        order: &WorkOrder,
        feedback: Option<&str>,
    ) -> std::io::Result<GenerationResult>;
}

/// Build the prompt the reasoner is asked to answer. It states the order, the
/// sourced research, the exact JSON shape required, and — on a retry — the
/// oracle's last failure. Kept here (not in the daemon) so the contract with
/// the reasoner travels with the parser that reads its reply.
pub fn build_prompt(order: &WorkOrder, feedback: Option<&str>) -> String {
    let mut p = String::new();
    p.push_str("You are the familiar's factory reasoner. Manufacture a driver.\n\n");
    p.push_str(&format!("GOAL: {}\n", order.goal));
    p.push_str(&format!(
        "DEVICE (subject, do not touch any other): {}\n",
        order.subject
    ));
    p.push_str(&format!(
        "CAPABILITY SURFACE (you may implement only these): {}\n",
        order.capability_surface.join(", ")
    ));
    p.push_str(&format!(
        "TOOLCHAIN: {} (standard library only)\n\n",
        order.toolchain.interpreter
    ));
    p.push_str("RESEARCH (sourced; informs, never dictates — you write your own code):\n");
    for r in &order.research {
        p.push_str(&format!("- {} [{}]\n", r.title, r.source));
    }
    p.push('\n');
    p.push_str(
        "Return ONE JSON object and nothing else. Either a candidate:\n\
         {\"files\":[{\"path\":\"driver.py\",\"role\":\"source\",\"content\":\"...\"},\
         {\"path\":\"test_driver.py\",\"role\":\"self_test\",\"content\":\"...\"}],\
         \"entrypoints\":[\"driver.py\"],\"self_tests\":[\"test_driver.py\"],\
         \"declared_effects\":[\"state\"],\"capability_surface\":[\"state\"]}\n\
         Roles are \"source\"|\"self_test\"|\"fixture\"|\"doc\". Paths are relative, no \"..\".\n\
         self_tests are stdlib Python that exit 0 on success. The driver must NOT open the\n\
         radio itself — a broker mediates; your self_tests run offline against fixtures.\n\
         Or a refusal:\n\
         {\"refused\":{\"code\":\"...\",\"rationale\":\"...\",\"unmet_requirements\":[\"...\"]}}\n",
    );
    if let Some(f) = feedback {
        p.push_str("\nYOUR PREVIOUS ATTEMPT FAILED THE BENCH ORACLE. Fix it:\n");
        p.push_str(f);
    }
    p
}

/// Parse a reasoner's JSON reply into a [`GenerationResult`]. The manifest and
/// artifact store are derived from the file contents (digests computed here),
/// so the reasoner never supplies digests. Structural errors (bad JSON, a role
/// we don't know, a file referenced by an entrypoint but not listed) surface
/// as errors; the workshop still independently validates the outcome before it
/// runs.
pub fn parse_reasoner_reply(reply: &str) -> std::io::Result<GenerationResult> {
    let v: serde_json::Value = serde_json::from_str(reply.trim())
        .map_err(|e| std::io::Error::other(format!("bad json: {e}")))?;

    if let Some(r) = v.get("refused") {
        let refusal = Refusal {
            code: r
                .get("code")
                .and_then(|x| x.as_str())
                .unwrap_or("refused")
                .to_string(),
            rationale: r
                .get("rationale")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            unmet_requirements: r
                .get("unmet_requirements")
                .and_then(|x| x.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|s| s.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            evidence: None,
        };
        return Ok(GenerationResult {
            outcome: GenerationOutcome::Refused(refusal),
            artifacts: BTreeMap::new(),
        });
    }

    let files = v
        .get("files")
        .and_then(|x| x.as_array())
        .ok_or_else(|| std::io::Error::other("reply has no files[] and no refused{}"))?;
    let mut manifest_files = Vec::new();
    let mut artifacts = BTreeMap::new();
    for f in files {
        let path = f
            .get("path")
            .and_then(|x| x.as_str())
            .ok_or_else(|| std::io::Error::other("file without a path"))?;
        let content = f
            .get("content")
            .and_then(|x| x.as_str())
            .ok_or_else(|| std::io::Error::other("file without content"))?;
        let role = match f.get("role").and_then(|x| x.as_str()).unwrap_or("source") {
            "source" => FileRole::Source,
            "self_test" => FileRole::SelfTest,
            "fixture" => FileRole::Fixture,
            "doc" => FileRole::Doc,
            other => return Err(std::io::Error::other(format!("unknown role: {other}"))),
        };
        let bytes = content.as_bytes().to_vec();
        let digest = digest_bytes(&bytes);
        artifacts.insert(digest.clone(), bytes);
        manifest_files.push(FileEntry {
            path: path.to_string(),
            digest,
            role,
        });
    }

    let str_list = |k: &str| -> Vec<String> {
        v.get(k)
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };

    Ok(GenerationResult {
        outcome: GenerationOutcome::Candidate {
            manifest: Manifest {
                files: manifest_files,
            },
            entrypoints: str_list("entrypoints"),
            self_tests: str_list("self_tests"),
            declared_effects: str_list("declared_effects"),
            toolchain_lock: String::new(),
            capability_surface: str_list("capability_surface"),
        },
        artifacts,
    })
}

/// The real generation adapter: it builds the prompt, hands it to the reasoner
/// via the `ask` closure (the daemon wires this to `familiar_llm::consult`),
/// and parses the reply. Kept generic over the closure so this crate does not
/// depend on the LLM crate and stays unit-testable with a fake reasoner.
pub struct ReasonerAdapter<F> {
    ask: F,
}

impl<F> ReasonerAdapter<F>
where
    F: Fn(&str) -> std::io::Result<String>,
{
    pub fn new(ask: F) -> Self {
        ReasonerAdapter { ask }
    }
}

impl<F> GenerationAdapter for ReasonerAdapter<F>
where
    F: Fn(&str) -> std::io::Result<String>,
{
    fn generate(
        &self,
        order: &WorkOrder,
        feedback: Option<&str>,
    ) -> std::io::Result<GenerationResult> {
        let prompt = build_prompt(order, feedback);
        let reply = (self.ask)(&prompt)?;
        parse_reasoner_reply(&reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use familiar_workshop::order::{validate_outcome, OracleRung, ResearchEntry, Toolchain};

    fn order() -> WorkOrder {
        WorkOrder {
            id: "o".into(),
            requester: "ian".into(),
            goal: "driver".into(),
            wording: "build".into(),
            subject: "ble:mfr=0x5053,wifi_mac=ba:16:b5:fe:19:82".into(),
            capability_surface: vec!["state".into(), "off".into()],
            research: vec![ResearchEntry {
                title: "notes".into(),
                source: "record".into(),
                digest: digest_bytes(b"n"),
            }],
            required_gates: vec![],
            oracle_plan: vec![OracleRung::Bench],
            toolchain: Toolchain {
                interpreter: "python3.13".into(),
                lock_digest: String::new(),
            },
            containment: "jail-v1".into(),
        }
    }

    #[test]
    fn the_prompt_names_the_surface_and_the_feedback() {
        let p = build_prompt(&order(), Some("bench failed: framing"));
        assert!(p.contains("state, off"));
        assert!(p.contains("PREVIOUS ATTEMPT FAILED"));
        assert!(p.contains("framing"));
    }

    #[test]
    fn a_candidate_reply_parses_and_validates() {
        let reply = r#"{
          "files":[
            {"path":"driver.py","role":"source","content":"def f(): return 0\n"},
            {"path":"test_driver.py","role":"self_test","content":"assert True\n"}
          ],
          "entrypoints":["driver.py"],
          "self_tests":["test_driver.py"],
          "declared_effects":["state"],
          "capability_surface":["state"]
        }"#;
        let r = parse_reasoner_reply(reply).expect("parse");
        // Digests were computed here; artifacts hold both files' bytes.
        assert_eq!(r.artifacts.len(), 2);
        // The workshop's own validation accepts it against the order.
        validate_outcome(&order(), &r.outcome).expect("valid");
    }

    #[test]
    fn a_refusal_reply_parses() {
        let reply =
            r#"{"refused":{"code":"no-spec","rationale":"x","unmet_requirements":["framing"]}}"#;
        let r = parse_reasoner_reply(reply).expect("parse");
        assert!(matches!(r.outcome, GenerationOutcome::Refused(_)));
        assert!(r.artifacts.is_empty());
    }

    #[test]
    fn junk_and_unknown_roles_are_rejected() {
        assert!(parse_reasoner_reply("not json").is_err());
        assert!(parse_reasoner_reply(r#"{"hello":1}"#).is_err());
        let bad_role = r#"{"files":[{"path":"x.py","role":"weapon","content":"x"}]}"#;
        assert!(parse_reasoner_reply(bad_role).is_err());
    }

    #[test]
    fn the_reasoner_adapter_round_trips_through_a_fake_ask() {
        let reply = r#"{"files":[{"path":"d.py","role":"source","content":"x=1\n"}],
                        "entrypoints":["d.py"],"self_tests":[],"declared_effects":[],
                        "capability_surface":[]}"#;
        let adapter = ReasonerAdapter::new(|prompt: &str| {
            assert!(prompt.contains("Manufacture a driver"));
            Ok(reply.to_string())
        });
        let r = adapter.generate(&order(), None).expect("generate");
        assert_eq!(r.artifacts.len(), 1);
    }
}

#[cfg(test)]
pub(crate) mod scripted {
    //! A scripted adapter for the convergence tests: it yields a queued list
    //! of results in order, recording the feedback it was handed each call so
    //! a test can assert the loop fed failures back.
    use super::*;
    use std::cell::RefCell;

    pub struct Scripted {
        pub queued: RefCell<Vec<GenerationResult>>,
        pub feedback_seen: RefCell<Vec<Option<String>>>,
    }

    impl Scripted {
        pub fn new(results: Vec<GenerationResult>) -> Self {
            Scripted {
                queued: RefCell::new(results),
                feedback_seen: RefCell::new(Vec::new()),
            }
        }
    }

    impl GenerationAdapter for Scripted {
        fn generate(
            &self,
            _order: &WorkOrder,
            feedback: Option<&str>,
        ) -> std::io::Result<GenerationResult> {
            self.feedback_seen
                .borrow_mut()
                .push(feedback.map(|s| s.to_string()));
            let mut q = self.queued.borrow_mut();
            if q.is_empty() {
                return Err(std::io::Error::other("scripted adapter exhausted"));
            }
            Ok(q.remove(0))
        }
    }
}
