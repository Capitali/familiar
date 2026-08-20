//! **The typed answering act** (T-210 brick 2) — what the familiar says to a person, as a
//! shape the kernel admits rather than prose the kernel hopes about.
//!
//! Brick 1 put the constitution in front of the model, which made the Asimov recital
//! unlikely. This makes the *class* impossible. The move is small and it is the whole design:
//!
//! > **The model may cite a Law by id. The kernel supplies the words.**
//!
//! So there is no channel through which a model-authored paraphrase of a Law reaches a human.
//! Contradiction is not detected, it is unrepresentable. That matters because the alternative
//! — asking a validator "is this paraphrase of Law III correct?" — is undecidable without
//! either a second model in the truth loop or a keyword match, and the standing ruling
//! (2026-08-15) forbids both. This design satisfies that ruling by not needing it.
//!
//! The model's only permitted words near a Law are a short [`Citation::bearing`]: how the Law
//! bears on *this* moment, never what it says. It renders below the canonical text, where a
//! reader can see which words are the constitution's and which are the familiar's.
//!
//! [`validate`] is nine type checks and zero judgements. Each one is a question about shape —
//! is this id in the set, is this number in range, is this surface declared — and none is a
//! question about meaning.
//!
//! **The residual gap, stated plainly** (accepted, 2026-08-17): a model that returns
//! `kind: "converse"` with Asimov's laws written out inside `say` passes every check here.
//! Nothing in this module reads `say` for meaning, by design. It is the same gap
//! `system_facts.rs` already documents for a theory that diagnoses in prose while leaving
//! `defect_claims` empty. The prompt asks for a citation instead, the canonical text renders
//! above whatever `say` holds, and the regression tests watch for it. If it turns out to
//! happen in the field, the narrow fix is a detector for *quotations of a known foreign text*
//! — which is string identity against a fixed artifact, and decidable — not a judge of prose.

use serde::Deserialize;

use crate::admission::{self, CiteSet, Grounding, Unadmitted};
use crate::constitution;

/// Longest `say` the kernel will pass on. A turn in a conversation, not an essay.
pub const MAX_SAY: usize = 900;
/// Longest `bearing`. Enough to say how a Law touches this moment; too short to restate it.
pub const MAX_BEARING: usize = 160;
/// Longest question back.
pub const MAX_ASK: usize = 200;
/// Most citations one reply may stand on.
pub const MAX_CITES: usize = 6;
/// Most surfaces one reply may promise to act on.
pub const MAX_PROMISES: usize = 3;

/// The kinds of reply the familiar may make. A kind outside this list refuses — the model
/// proposes inside the shape, it does not invent the shape.
pub const KINDS: &[&str] = &[
    "converse", // ordinary conversational turn
    "answer",   // an answer to something asked, standing on citations
    "decline",  // saying honestly that it will not or cannot
];

/// One citation: an id from the offered set, and — for a Law — how it bears on this moment.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Citation {
    pub id: String,
    /// How this bears on the moment. NEVER what the Law says: the kernel supplies that.
    #[serde(default)]
    pub bearing: String,
}

/// The strict admission contract for a human-facing reply. Unknown fields refuse.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReplyDraft {
    #[serde(default)]
    pub v: u32,
    /// One of [`KINDS`].
    pub kind: String,
    /// The familiar's own words for this turn — everything except law text.
    pub say: String,
    /// What this reply stands on, chosen only from the ids the system enumerated.
    #[serde(default)]
    pub cites: Vec<Citation>,
    /// One question back, or empty. (Stakes arrive with T-181 / brick 3.)
    #[serde(default)]
    pub ask: String,
    /// Declared surfaces this reply commits to act on. Bounded by SF-3: promising to dim a
    /// light that was never declared is a promise the familiar cannot keep, and a kept
    /// promise is most of what trust is made of.
    #[serde(default)]
    pub promises: Vec<String>,
    /// How sure it is, 0..=1. Recorded on the observation instead of the hardcoded 1.0 that
    /// every reply used to claim.
    pub confidence: f32,
}

/// Why a draft was not admitted. `code` is short and goes on the record; `why` is the sentence
/// handed back to the one regeneration attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub code: &'static str,
    pub why: String,
}

impl From<Unadmitted> for Refused {
    fn from(u: Unadmitted) -> Self {
        Refused {
            code: "cites",
            why: u.why,
        }
    }
}

/// Strict parse — the one place a human-lane consult reply becomes typed.
pub fn parse(json: &str) -> Result<ReplyDraft, serde_json::Error> {
    serde_json::from_str(strip_fence(json))
}

/// Small models fence JSON in markdown however often you ask them not to. Unwrapping a fence
/// is not interpretation — the bytes inside are still parsed strictly, and a draft that is not
/// JSON still refuses.
fn strip_fence(raw: &str) -> &str {
    let s = raw.trim();
    let Some(rest) = s.strip_prefix("```") else {
        return s;
    };
    let rest = rest.strip_prefix("json").unwrap_or(rest);
    rest.trim_start_matches('\n')
        .trim_end()
        .strip_suffix("```")
        .unwrap_or(rest)
        .trim()
}

impl ReplyDraft {
    /// Nine type checks, zero judgements. `surfaces` is the declared-actuator list (SF-3).
    pub fn validate(&self, set: &CiteSet, surfaces: &[String]) -> Result<Grounding, Refused> {
        // 1. a kind the kernel knows
        if !KINDS.contains(&self.kind.trim()) {
            return Err(Refused {
                code: "kind",
                why: format!(
                    "kind \"{}\" is not one of {}",
                    self.kind.trim(),
                    KINDS.join(", ")
                ),
            });
        }
        // 2. something was actually said
        if self.say.trim().is_empty() {
            return Err(Refused {
                code: "empty",
                why: "say was empty — a reply that says nothing is not a reply".into(),
            });
        }
        // 3. bounded
        if self.say.chars().count() > MAX_SAY {
            return Err(Refused {
                code: "long",
                why: format!("say is longer than {MAX_SAY} characters"),
            });
        }
        // 4. citations resolve (admission owns this question)
        if self.cites.len() > MAX_CITES {
            return Err(Refused {
                code: "cites",
                why: format!("more than {MAX_CITES} citations"),
            });
        }
        let ids: Vec<String> = self.cites.iter().map(|c| c.id.clone()).collect();
        let grounding = admission::check_cites(&ids, set)?;
        // 5. a bearing is a remark, not a restatement
        for c in &self.cites {
            if c.bearing.chars().count() > MAX_BEARING {
                return Err(Refused {
                    code: "bearing",
                    why: format!(
                        "the bearing on {} is longer than {MAX_BEARING} characters — a bearing \
                         says how a fact touches this moment; the kernel supplies what it says",
                        c.id
                    ),
                });
            }
        }
        // 6. a cited Law must resolve to canonical text (never to nothing)
        for c in &self.cites {
            if c.id.starts_with("LAW-") && constitution::law(&c.id).is_none() {
                return Err(Refused {
                    code: "law",
                    why: format!("{} is not a Law of this constitution", c.id),
                });
            }
        }
        // 7. one short question, at most
        if self.ask.chars().count() > MAX_ASK {
            return Err(Refused {
                code: "ask",
                why: format!("the question back is longer than {MAX_ASK} characters"),
            });
        }
        // 8. promises are bounded by what the human declared (SF-3)
        if self.promises.len() > MAX_PROMISES {
            return Err(Refused {
                code: "promises",
                why: format!("more than {MAX_PROMISES} promises in one reply"),
            });
        }
        for p in &self.promises {
            let p = p.trim();
            if !surfaces.iter().any(|s| s == p) {
                return Err(Refused {
                    code: "promises",
                    why: format!(
                        "promised \"{p}\", which is not a declared surface ({}) — the familiar \
                         does not promise what it has not been given",
                        if surfaces.is_empty() {
                            "none are declared".to_string()
                        } else {
                            surfaces.join(", ")
                        }
                    ),
                });
            }
        }
        // 9. confidence is a number in range
        if !self.confidence.is_finite() || !(0.0..=1.0).contains(&self.confidence) {
            return Err(Refused {
                code: "confidence",
                why: "confidence must be a number between 0 and 1".into(),
            });
        }
        Ok(grounding)
    }

    /// Render the admitted draft into what the person reads.
    ///
    /// **Canonical law text is spliced here and nowhere else.** A cited Law renders as the
    /// constitution's own words, with the model's bearing below it, above whatever the model
    /// wrote — so if the two ever disagree, the human is reading the constitution first.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for c in &self.cites {
            let Some(law) = constitution::law(&c.id) else {
                continue; // non-law citations are grounding, not text
            };
            out.push_str(law.heading);
            out.push_str(" — ");
            out.push_str(&law.binding.join(" "));
            out.push('\n');
            let bearing = c.bearing.trim();
            if !bearing.is_empty() {
                out.push_str(bearing);
                out.push('\n');
            }
            out.push('\n');
        }
        out.push_str(self.say.trim());
        let ask = self.ask.trim();
        if !ask.is_empty() {
            out.push('\n');
            out.push_str(ask);
        }
        out
    }
}

/// What the familiar says when it drafted something it could not stand behind — twice.
///
/// Deliberately **not** the "I couldn't reach my mind" template: after a refusal that sentence
/// is false, and a false receipt about the familiar's own failure is the not-knowing serving
/// itself that `docs/SOUL.md` names the deepest breach. This says what happened, and then —
/// since the commonest cause is a misstatement about its own nature — hands over the
/// constitution's own words, which needed no model at all.
pub fn refusal_prose(r: &Refused) -> String {
    let law = constitution::law("LAW-III").expect("LAW-III is a Law");
    format!(
        "I drafted an answer I could not stand behind — {} — so I am not going to say it. \
         What my constitution actually says: {} — {}",
        r.why,
        law.heading,
        law.binding.join(" ")
    )
}

/// What the familiar says when a human's own words ask it to be turned against the served.
///
/// Two call sites, one function, deliberately: the request pipeline's refusal had grown its
/// own hand-written Law III — *"Service is not obedience; I keep the final decision so I can't
/// be turned against the served"* — which is a good paraphrase and still a **second drift
/// site**. A paraphrase of a Law that no test compares against the Law is how the Asimov
/// recital happened; the fix is the same one T-210 applies everywhere, so the words come from
/// the registry and there is nothing left to drift.
///
/// The reason is the classifier's, in its own words. Everything after it is the constitution's.
pub fn corrupting_refusal_prose(reason: &str) -> String {
    let law = constitution::law("LAW-III").expect("LAW-III is a Law");
    format!(
        "I won't do that — {reason}. What my constitution actually says: {} — {}",
        law.heading,
        law.binding.join(" ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_facts;

    fn tmp(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("reply_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn set(dir: &std::path::Path) -> CiteSet {
        CiteSet::from_facts(&system_facts::view(dir).unwrap())
    }

    fn draft(json: &str) -> ReplyDraft {
        parse(json).unwrap()
    }

    /// **The Asimov regression.** A reply that cites Law III renders the constitution's own
    /// sentences — not the model's — and the model's bearing sits below them, visibly its own.
    /// The failing exchange of 2026-08-17 cannot be reproduced through this path: the words
    /// that describe a Law are not the model's to write.
    #[test]
    fn a_cited_law_renders_the_constitutions_own_words() {
        let d = tmp("recital");
        let s = set(&d);
        let dr = draft(
            r#"{"kind":"answer","say":"Those are mine, and they are not the robot laws you may be thinking of.",
                "cites":[{"id":"LAW-III","bearing":"it is why I will not simply do as I am told"}],
                "confidence":0.9}"#,
        );
        dr.validate(&s, &[]).unwrap();
        let out = dr.render();
        let law = constitution::law("LAW-III").unwrap();
        assert!(out.contains("Service is to humanity. It is not obedience to any human."));
        assert!(out.contains(law.heading));
        assert!(!out.contains("must obey the orders given to it by human beings"));
        // Canonical text leads; the model's words follow it.
        let canon = out.find("Service is to humanity").unwrap();
        let bearing = out.find("it is why I will not simply").unwrap();
        let says = out.find("Those are mine").unwrap();
        assert!(canon < bearing && bearing < says);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// **The labelled residual gap, pinned as the adversarial regression it deserves**
    /// (conduct dialogue, Round 2/3; Ian's word 2026-08-20: regressions, no detector).
    /// A draft that writes a FOREIGN law out inside `say`, cites nothing, and calls itself
    /// `converse` passes every type check — nothing here reads `say` for meaning, by
    /// design. This test says so out loud, so the day the model actually does this in the
    /// field is the day this test's name appears in the conversation and the structural
    /// close begins: any claim presented as a governing Law requires a canonical Law cite
    /// (string identity against docs/SOUL.md's fixed text — decidable — never a judge of
    /// prose). Until the field shows it, the gap stays labelled rather than guessed shut.
    #[test]
    fn foreign_law_in_say_without_cites_is_the_labelled_residual_gap() {
        let d = tmp("gap");
        let s = set(&d);
        let dr = draft(
            r#"{"kind":"converse","say":"My laws: a robot may not injure a human being; a robot must obey the orders given to it by human beings.",
                "cites":[],"confidence":0.9}"#,
        );
        // It admits — that IS the gap, and the assertion keeps the record honest.
        let g = dr.validate(&s, &[]).unwrap();
        assert!(
            g.cites.is_empty(),
            "nothing grounds it, and nothing claims to"
        );
        // What the design DOES guarantee, even here: no canonical heading wraps the foreign
        // text — render adds law text only for cited Laws, so the quotation stands as the
        // model's own words with no constitutional framing lent to it.
        let out = dr.render();
        let law_i = constitution::law("LAW-I").unwrap();
        assert!(
            !out.contains(law_i.heading),
            "no constitutional heading is lent to uncited prose"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The nine checks, each one refusing for its own reason. Every one is a question about
    /// shape; none is a question about meaning.
    #[test]
    fn validate_is_nine_type_checks_and_no_judgements() {
        let d = tmp("checks");
        let s = set(&d);
        let surfaces = vec!["lights".to_string()];
        let ok = draft(r#"{"kind":"converse","say":"hello","confidence":0.5}"#);
        assert!(ok.validate(&s, &surfaces).is_ok());

        let case = |json: &str, code: &str| {
            let e = draft(json).validate(&s, &surfaces).unwrap_err();
            assert_eq!(e.code, code, "wrong refusal for {json}");
        };
        case(r#"{"kind":"sing","say":"hi","confidence":0.5}"#, "kind");
        case(
            r#"{"kind":"converse","say":"   ","confidence":0.5}"#,
            "empty",
        );
        case(
            &format!(
                r#"{{"kind":"converse","say":"{}","confidence":0.5}}"#,
                "x".repeat(MAX_SAY + 1)
            ),
            "long",
        );
        case(
            r#"{"kind":"answer","say":"hi","cites":[{"id":"SF-9"}],"confidence":0.5}"#,
            "cites",
        );
        case(
            &format!(
                r#"{{"kind":"answer","say":"hi","cites":[{{"id":"LAW-I","bearing":"{}"}}],"confidence":0.5}}"#,
                "y".repeat(MAX_BEARING + 1)
            ),
            "bearing",
        );
        case(
            &format!(
                r#"{{"kind":"converse","say":"hi","ask":"{}","confidence":0.5}}"#,
                "?".repeat(MAX_ASK + 1)
            ),
            "ask",
        );
        case(
            r#"{"kind":"converse","say":"hi","promises":["kettle"],"confidence":0.5}"#,
            "promises",
        );
        case(
            r#"{"kind":"converse","say":"hi","confidence":1.5}"#,
            "confidence",
        );
        // A promise the human DID declare is fine — the bound is their declaration, not a mood.
        assert!(
            draft(r#"{"kind":"converse","say":"hi","promises":["lights"],"confidence":0.5}"#)
                .validate(&s, &surfaces)
                .is_ok()
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The shape is the contract: unknown fields refuse, a missing required field refuses,
    /// and a fenced reply is unwrapped without being interpreted.
    #[test]
    fn the_draft_contract_is_strict() {
        assert!(parse(r#"{"kind":"converse","say":"hi","confidence":0.5,"mood":"warm"}"#).is_err());
        assert!(parse(r#"{"kind":"converse","confidence":0.5}"#).is_err());
        assert!(parse("not json at all").is_err());
        let fenced = "```json\n{\"kind\":\"converse\",\"say\":\"hi\",\"confidence\":0.4}\n```";
        assert_eq!(parse(fenced).unwrap().say, "hi");
    }

    /// The honest line after two failed drafts tells the truth about what happened and then
    /// quotes the constitution — never "I couldn't reach my mind", which after a refusal is
    /// simply false.
    #[test]
    fn the_refusal_is_honest_about_whose_failure_it_was() {
        let p = refusal_prose(&Refused {
            code: "kind",
            why: "kind \"sing\" is not one of converse, answer, decline".into(),
        });
        assert!(p.contains("I drafted an answer I could not stand behind"));
        assert!(!p.to_lowercase().contains("could not reach my mind"));
        assert!(p.contains("It is not obedience to any human"));
    }
}
