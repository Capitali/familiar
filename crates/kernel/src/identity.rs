//! Names — the familiar becomes familiar.
//!
//! At this stage the human is simply *the observer* whose needs the familiar serves. The
//! familiar does not assume a name; it **asks**, and — once told — it does not forget. A
//! name is quality data: retained across sessions, it grounds the relationship and can, in
//! time, carry lineage and standing (the `relation` field is the seed of that). This is the
//! registry of everyone the familiar has come to know, append-only and rebuildable; plus a
//! tiny pointer (`observer.txt`) to whoever is present right now.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

use crate::store;

pub const IDENTITY_FILE: &str = "identities.jsonl";
/// The handle of the human currently being served (whoever last introduced themselves).
pub const OBSERVER_FILE: &str = "observer.txt";

/// Someone the familiar has come to know.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Identity {
    /// Stable id used as the `actor` across observations/requests/threads — a slug of the
    /// name first given. Stays put even if the display name is later refined.
    pub handle: String,
    /// What they asked to be called. Names are important; this is kept verbatim.
    pub name: String,
    /// Standing relative to the familiar — "observer" at this stage. Room for lineage and
    /// status to grow here later (e.g. "steward", "guest", a relation to another handle).
    pub relation: String,
    pub first_seen: i64,
    pub last_seen: i64,
    /// How many interactions have been recorded under this name — a visible measure of the
    /// bond, and why a known name should never be discarded.
    pub interactions: u32,
    /// A face embedding linked to this identity — a link to an existing identity, never a new
    /// record on its own (docs/design-orientation-and-mesh.md). Strongly sensitive: gated like
    /// the camera but with its own opt-in (`allow_face_recognition`, crates/kernel/src/
    /// guard.rs), and — unlike name/relation — NEVER federated to mesh peers under any
    /// `share_identities` state; see crates/mesh/src/brief.rs's `IdentityShare`. A wrong link
    /// must be correctable, never sticky: callers replace this via `link_face`, they don't
    /// hand-edit the vector.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face_signature: Option<Vec<f32>>,
}

/// A stable, lowercase handle derived from a name (`"Betty Jo"` -> `"betty-jo"`). Falls back
/// to `"observer"` if the name has no usable characters.
pub fn slug(name: &str) -> String {
    let s: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_string();
    // collapse runs of '-'
    let mut out = String::with_capacity(s.len());
    let mut last_dash = false;
    for c in s.chars() {
        if c == '-' {
            if !last_dash {
                out.push(c);
            }
            last_dash = true;
        } else {
            out.push(c);
            last_dash = false;
        }
    }
    if out.is_empty() {
        "observer".to_string()
    } else {
        out
    }
}

pub fn load(dir: &Path) -> io::Result<Vec<Identity>> {
    store::load(dir, IDENTITY_FILE)
}

pub fn find<'a>(people: &'a [Identity], handle: &str) -> Option<&'a Identity> {
    people.iter().find(|p| p.handle == handle)
}

/// Learn (or re-greet) a name. If the handle is already known, bump `interactions` and
/// `last_seen` in place — we don't forget, and we don't duplicate. Otherwise add a new
/// observer. Returns the resulting identity. Does **not** change who is "current".
pub fn remember(dir: &Path, name: &str, now: i64) -> io::Result<Identity> {
    let name = name.trim();
    let handle = slug(name);
    let mut people = load(dir)?;
    if let Some(existing) = people.iter_mut().find(|p| p.handle == handle) {
        existing.last_seen = now;
        existing.interactions += 1;
        // keep the freshest spelling of the name they gave
        existing.name = name.to_string();
        let updated = existing.clone();
        store::rewrite(dir, IDENTITY_FILE, &people)?;
        return Ok(updated);
    }
    let id = Identity {
        handle,
        name: name.to_string(),
        relation: "observer".to_string(),
        first_seen: now,
        last_seen: now,
        interactions: 1,
        face_signature: None,
    };
    store::append(dir, IDENTITY_FILE, &id)?;
    Ok(id)
}

/// Link (or replace) the face embedding for a known identity — the "recognition feeds
/// identity" seam (docs/design-orientation-and-mesh.md). A wrong match must be correctable:
/// calling this again with a new signature simply replaces the old one, never appends. `None`
/// clears the link entirely. No-op (returns `Ok(None)`) if the handle isn't known — recognition
/// links an *existing* identity, it never mints one.
pub fn link_face(dir: &Path, handle: &str, signature: Option<Vec<f32>>) -> io::Result<Option<Identity>> {
    let mut people = load(dir)?;
    let Some(existing) = people.iter_mut().find(|p| p.handle == handle) else {
        return Ok(None);
    };
    existing.face_signature = signature;
    let updated = existing.clone();
    store::rewrite(dir, IDENTITY_FILE, &people)?;
    Ok(Some(updated))
}

/// The handle of whoever is present now (`None` until someone introduces themselves).
pub fn current(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join(OBSERVER_FILE))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Set who is present now.
pub fn set_current(dir: &Path, handle: &str) -> io::Result<()> {
    std::fs::write(dir.join(OBSERVER_FILE), handle)
}

/// The full identity of whoever is present now, if known.
pub fn current_identity(dir: &Path) -> Option<Identity> {
    let handle = current(dir)?;
    let people = load(dir).ok()?;
    find(&people, &handle).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct Temp(PathBuf);
    impl Temp {
        fn new(t: &str) -> Self {
            let p = std::env::temp_dir().join(format!("familiar_identity_test_{t}"));
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

    #[test]
    fn slugs_are_stable_and_safe() {
        assert_eq!(slug("Betty Jo"), "betty-jo");
        assert_eq!(slug("  O'Brien  "), "o-brien");
        assert_eq!(slug("???"), "observer");
    }

    #[test]
    fn remembering_a_name_retains_and_does_not_duplicate() {
        let t = Temp::new("remember");
        let a = remember(&t.0, "Ada", 100).unwrap();
        assert_eq!(a.handle, "ada");
        assert_eq!(a.interactions, 1);
        // same person again: bumped in place, not duplicated
        let a2 = remember(&t.0, "Ada", 200).unwrap();
        assert_eq!(a2.interactions, 2);
        assert_eq!(a2.last_seen, 200);
        assert_eq!(load(&t.0).unwrap().len(), 1);
        // a different person is retained alongside
        remember(&t.0, "Grace", 300).unwrap();
        assert_eq!(load(&t.0).unwrap().len(), 2);
    }

    #[test]
    fn face_signature_defaults_none_and_round_trips_through_storage() {
        let t = Temp::new("face_default");
        let a = remember(&t.0, "Ada", 100).unwrap();
        assert!(a.face_signature.is_none(), "a plain remember() links no face");

        let linked = link_face(&t.0, "ada", Some(vec![0.1, 0.2, 0.3])).unwrap().unwrap();
        assert_eq!(linked.face_signature, Some(vec![0.1, 0.2, 0.3]));
        // persisted, not just in-memory
        let reloaded = find(&load(&t.0).unwrap(), "ada").cloned().unwrap();
        assert_eq!(reloaded.face_signature, Some(vec![0.1, 0.2, 0.3]));
    }

    #[test]
    fn a_wrong_face_link_is_correctable_never_sticky() {
        let t = Temp::new("face_correct");
        remember(&t.0, "Ada", 100).unwrap();
        link_face(&t.0, "ada", Some(vec![1.0, 0.0])).unwrap();
        // a correction replaces, it doesn't append or require clearing first
        let corrected = link_face(&t.0, "ada", Some(vec![0.0, 1.0])).unwrap().unwrap();
        assert_eq!(corrected.face_signature, Some(vec![0.0, 1.0]));
        // and can be cleared entirely
        let cleared = link_face(&t.0, "ada", None).unwrap().unwrap();
        assert!(cleared.face_signature.is_none());
    }

    #[test]
    fn linking_an_unknown_handle_is_a_no_op() {
        // Recognition links an EXISTING identity; it never mints one on its own.
        let t = Temp::new("face_unknown");
        let result = link_face(&t.0, "nobody", Some(vec![1.0])).unwrap();
        assert!(result.is_none());
        assert!(load(&t.0).unwrap().is_empty());
    }

    #[test]
    fn current_observer_round_trips() {
        let t = Temp::new("current");
        assert!(current(&t.0).is_none());
        remember(&t.0, "Ada", 100).unwrap();
        set_current(&t.0, "ada").unwrap();
        assert_eq!(current(&t.0).as_deref(), Some("ada"));
        assert_eq!(
            current_identity(&t.0).map(|i| i.name),
            Some("Ada".to_string())
        );
    }
}
