//! **Admission** — the one place a draft's citations are checked against what the system
//! actually enumerated (D3, T-210 brick 2).
//!
//! Every generation the familiar admits is supposed to stand on something: a theory cites the
//! observations it explains, a reply cites the facts it answers from. Until this module, each
//! path invented its own version of that check — `maybe_theorize` compares anchors against a
//! `HashSet` it built inline, and the reply path checked nothing at all because it cited
//! nothing at all. One admission function means one answer to "may this draft say that", and
//! one place to fix when the answer is wrong.
//!
//! What admission is NOT: a judge of content. It never asks whether a claim is true, or
//! whether prose means what it says. It asks only whether every id a draft names was in the
//! set the system handed it — which is decidable, cheap, and immune to being talked out of.
//! (T-135 moves `TheoryDraft`'s anchor check onto this; the reply is its first citizen.)

use std::collections::BTreeSet;

use crate::system_facts::FactsView;

/// The ids a draft may cite, plus the identity of the registry view they came from — so a
/// [`Grounding`] records not only *what* was cited but *which* revision of the truth it was
/// cited against. A later revision then supersedes rather than silently reinterprets.
#[derive(Debug, Clone, Default)]
pub struct CiteSet {
    ids: BTreeSet<String>,
    schema: u32,
    revision: u32,
    declaration_digest: String,
}

impl CiteSet {
    /// The registry's own ids — LAW-I/II/III and SF-1/2/3 today, whatever `view()` emits
    /// tomorrow. Deliberately derived rather than listed: a fact added to the registry is
    /// citable the moment it exists, and a fact removed stops being citable at once.
    pub fn from_facts(v: &FactsView) -> Self {
        Self {
            ids: v.facts.iter().map(|f| f.id.clone()).collect(),
            schema: v.schema,
            revision: v.revision,
            declaration_digest: v.declaration_digest.clone(),
        }
    }

    /// Widen the set with ids from another enumeration (observation ids, loop names). The
    /// registry identity is unchanged — those ids are evidence, not facts.
    pub fn allow<I: IntoIterator<Item = S>, S: Into<String>>(mut self, ids: I) -> Self {
        self.ids.extend(ids.into_iter().map(Into::into));
        self
    }

    pub fn contains(&self, id: &str) -> bool {
        self.ids.contains(id.trim())
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// The set as the prompt should enumerate it — sorted, so the same world always produces
    /// the same prompt.
    pub fn ids(&self) -> Vec<&str> {
        self.ids.iter().map(String::as_str).collect()
    }
}

/// What an admitted draft stands on: the ids it cited and the registry revision they were
/// checked against. Recorded with the act, so the record can always answer "on what".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grounding {
    pub cites: Vec<String>,
    pub facts_schema: u32,
    pub facts_rev: u32,
    pub declaration_digest: String,
}

impl Grounding {
    /// The cited ids as one short field for the record — `"LAW-III,SF-3"`, or empty.
    pub fn cites_line(&self) -> String {
        self.cites.join(",")
    }
}

/// A draft that named something the system never offered it. Carries the offending ids, so
/// the regeneration attempt can be told exactly what went wrong instead of "try again".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unadmitted {
    pub why: String,
    pub unknown: Vec<String>,
}

/// Check a draft's citations against the enumerated set.
///
/// Every id must resolve. An empty citation list is *allowed here* and is the caller's policy
/// to refuse or permit — a passing remark cites nothing, an answer about the constitution had
/// better cite it, and admission is not the place that knows the difference.
pub fn check_cites(cites: &[String], set: &CiteSet) -> Result<Grounding, Unadmitted> {
    let mut seen: Vec<String> = Vec::new();
    let mut unknown: Vec<String> = Vec::new();
    for c in cites {
        let id = c.trim().to_string();
        if id.is_empty() {
            continue;
        }
        if !set.contains(&id) {
            if !unknown.contains(&id) {
                unknown.push(id);
            }
            continue;
        }
        if !seen.contains(&id) {
            seen.push(id);
        }
    }
    if !unknown.is_empty() {
        return Err(Unadmitted {
            why: format!(
                "cited {} outside the offered set ({})",
                unknown.join(", "),
                if set.is_empty() {
                    "nothing was offered".to_string()
                } else {
                    set.ids().join(", ")
                }
            ),
            unknown,
        });
    }
    Ok(Grounding {
        cites: seen,
        facts_schema: set.schema,
        facts_rev: set.revision,
        declaration_digest: set.declaration_digest.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_facts;

    fn tmp(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("admission_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    /// The set is the registry's own ids, and it carries the revision they were checked
    /// against — the identity a record needs to say what a citation meant when it was made.
    #[test]
    fn the_cite_set_is_the_registry_and_it_remembers_which_revision() {
        let d = tmp("set");
        let v = system_facts::view(&d).unwrap();
        let set = CiteSet::from_facts(&v);
        for id in ["LAW-I", "LAW-II", "LAW-III", "SF-1", "SF-2", "SF-3"] {
            assert!(set.contains(id), "registry id {id} must be citable");
        }
        assert!(!set.contains("SF-9"));
        let g = check_cites(&["LAW-III".into(), "SF-3".into()], &set).unwrap();
        assert_eq!(g.facts_rev, system_facts::FACTS_REVISION);
        assert_eq!(g.cites_line(), "LAW-III,SF-3");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// An invented id refuses and is NAMED — a regeneration told "cited LAW-IV outside the
    /// offered set" can fix itself; one told "invalid" can only guess.
    #[test]
    fn an_invented_citation_refuses_and_says_which() {
        let d = tmp("unknown");
        let set = CiteSet::from_facts(&system_facts::view(&d).unwrap());
        let e = check_cites(&["LAW-IV".into(), "LAW-I".into()], &set).unwrap_err();
        assert_eq!(e.unknown, vec!["LAW-IV"]);
        assert!(e.why.contains("LAW-IV"));

        // Whitespace and duplicates are shape, not dishonesty: trimmed and collapsed.
        let g = check_cites(&[" LAW-I ".into(), "LAW-I".into(), "".into()], &set).unwrap();
        assert_eq!(g.cites, vec!["LAW-I"]);
        // Citing nothing is not admission's business to refuse.
        assert!(check_cites(&[], &set).unwrap().cites.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Evidence ids widen the set without touching the registry identity it carries.
    #[test]
    fn evidence_widens_the_set_but_not_the_revision() {
        let d = tmp("widen");
        let v = system_facts::view(&d).unwrap();
        let set = CiteSet::from_facts(&v).allow(["obs-0042", "loop:lights"]);
        let g = check_cites(&["obs-0042".into(), "LAW-I".into()], &set).unwrap();
        assert_eq!(g.facts_rev, v.revision);
        assert_eq!(g.declaration_digest, v.declaration_digest);
        assert!(check_cites(&["obs-0043".into()], &set).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
