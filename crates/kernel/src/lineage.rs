//! Lineage — trace a candidate's ancestry. Faithful port of v1's `lineage.c`.

use crate::candidate::{self, Candidate};

/// The ancestry chain ending at `id`, **root ancestor first**. Walks `parent_id`
/// links; stops at a root (empty parent) or a missing/broken link.
pub fn chain(cands: &[Candidate], id: &str) -> Vec<Candidate> {
    let mut out = Vec::new();
    let mut current = id.to_string();
    while let Some(c) = candidate::find(cands, &current) {
        let parent = c.parent_id.clone();
        out.push(c.clone());
        if parent.is_empty() {
            break;
        }
        current = parent;
    }
    out.reverse(); // root first
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(id: &str, parent: &str) -> Candidate {
        Candidate {
            id: id.into(),
            parent_id: parent.into(),
            loop_id: "loop-x".into(),
            generation: 0,
            hypothesis: String::new(),
            artifact_type: "script".into(),
            artifact_path: String::new(),
            inherited_traits: String::new(),
            changed_traits: String::new(),
            mutation_reason: String::new(),
            status: "generated".into(),
        }
    }

    #[test]
    fn traces_root_first() {
        let cands = vec![c("c1", ""), c("c2", "c1"), c("c3", "c2")];
        let chain = chain(&cands, "c3");
        let ids: Vec<&str> = chain.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, vec!["c1", "c2", "c3"]);
    }

    #[test]
    fn root_is_itself() {
        let cands = vec![c("c1", "")];
        assert_eq!(chain(&cands, "c1").len(), 1);
    }

    #[test]
    fn missing_id_is_empty() {
        assert!(chain(&[c("c1", "")], "nope").is_empty());
    }
}
