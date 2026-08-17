//! **The persona seam** — one soul, many voices (ADR-0037 §1).
//!
//! The familiar's character was compiled in: roughly eight inline framings — "You are a factory
//! whose only purpose is to serve …" — lived as format strings across `crates/cycle`,
//! `crates/agent` and `crates/mesh`. ADR-0037 specified this module in 2026-08-10 and it was
//! never built; T-210's audit found `role_line` returning nothing anywhere in `crates/`,
//! because there was nothing to return it from. This is the minimum that ends that: the role
//! phrase becomes data, loaded per data dir, defaulting to today's words byte-for-byte.
//!
//! **What a persona may never do.** It changes the mask, never the authority. The Three Laws
//! ([`crate::constitution`]), `guard::evaluate`, the boundary gates and the Law III voice are
//! constitutionally fixed and are always rendered *before* anything from here — a hostile or
//! merely enthusiastic `persona.json` can change tone and cannot touch law. And a costume
//! grants capability over nothing: ADR-0005 stands, no gate opens here.
//!
//! **The world partition is the data dir.** A game instance (Purr aboard a ship) is a separate
//! data dir with its own `persona.json`, its own declared surfaces and its own store, so the
//! ship's computer cannot inherit the household's facts by construction rather than by filter.

use std::io;
use std::path::Path;

use serde::Deserialize;

/// The file a data dir may carry to wear a different voice. Absent means the familiar itself.
pub const PERSONA_FILE: &str = "persona.json";

/// The role phrase the familiar has always used, with `{who}` where the served person's name
/// goes. Kept byte-for-byte identical to the literal it replaced in `cycle::maybe_reply`
/// (pinned by [`tests::the_default_persona_is_todays_words_byte_for_byte`]) — the seam must be
/// a no-op for every existing deployment, or it rots in a corner only Purr visits.
pub const DEFAULT_ROLE: &str = "a factory whose only purpose is to serve {who} (the Three Laws; \
                                humanity is served, never managed or replaced)";

/// The name the familiar answers to when no persona names it otherwise.
pub const DEFAULT_NAME: &str = "the familiar";

/// A voice: who this instance says it is, and how it speaks. Never what it may do.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Persona {
    /// Contract version of `persona.json`.
    #[serde(default = "one")]
    pub persona_version: u32,
    /// What this instance is called. In the game's ceremony this is the captain's to write;
    /// nothing else in the file is player-writable.
    #[serde(default = "default_name")]
    pub name: String,
    /// The role phrase, with `{who}` substituted at render time.
    #[serde(default = "default_role")]
    pub role: String,
    /// Cadence and vocabulary. Appended *after* the constitutionally fixed voice, never
    /// instead of it. Unused until the game shell lands; carried so a `persona.json` written
    /// against ADR-0037 parses today.
    #[serde(default)]
    pub register: String,
    /// The fiction frame prompts may assume. Same rule as `register`.
    #[serde(default)]
    pub world: String,
}

fn one() -> u32 {
    1
}
fn default_name() -> String {
    DEFAULT_NAME.to_string()
}
fn default_role() -> String {
    DEFAULT_ROLE.to_string()
}

impl Default for Persona {
    fn default() -> Self {
        Self {
            persona_version: 1,
            name: default_name(),
            role: default_role(),
            register: String::new(),
            world: String::new(),
        }
    }
}

impl Persona {
    /// The role sentence a prompt carries, with the served person's name in place.
    pub fn role_line(&self, who: &str) -> String {
        format!("You are {}.", self.role.replace("{who}", who))
    }
}

/// Load this data dir's persona. An absent file is the familiar itself — the common case, and
/// the one every existing deployment exercises. A file that exists but does not parse is an
/// **error**, not a silent fallback: a human who wrote `persona.json` and got the default
/// anyway would be told nothing while a different character spoke in their name.
pub fn load(dir: &Path) -> io::Result<Persona> {
    let path = dir.join(PERSONA_FILE);
    let raw = match std::fs::read_to_string(&path) {
        Ok(r) => r,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Persona::default()),
        Err(e) => return Err(e),
    };
    serde_json::from_str(&raw).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{PERSONA_FILE} is present but unreadable: {e}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("persona_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    /// The seam is a no-op by default. This string is the literal that stood in
    /// `cycle::maybe_reply` before the persona existed; if it changes, every deployment's
    /// voice changed with it, and that is a decision, not a refactor.
    #[test]
    fn the_default_persona_is_todays_words_byte_for_byte() {
        assert_eq!(
            Persona::default().role_line("ian"),
            "You are a factory whose only purpose is to serve ian (the Three Laws; humanity is \
             served, never managed or replaced)."
        );
    }

    /// An absent file is the familiar; a present one is honoured; a broken one is an error the
    /// human hears about rather than a costume silently swapped for the default.
    #[test]
    fn absent_is_default_present_is_honoured_broken_is_loud() {
        let d = tmp("load");
        assert_eq!(load(&d).unwrap(), Persona::default());

        std::fs::write(
            d.join(PERSONA_FILE),
            r#"{"persona_version":1,"name":"Purr","role":"the ship's computer of the vessel Kestrel, serving {who}","register":"clipped bridge-officer cadence","world":"the branch-grant story"}"#,
        )
        .unwrap();
        let p = load(&d).unwrap();
        assert_eq!(p.name, "Purr");
        assert_eq!(
            p.role_line("the captain"),
            "You are the ship's computer of the vessel Kestrel, serving the captain."
        );

        std::fs::write(d.join(PERSONA_FILE), "{ not json").unwrap();
        assert!(load(&d).is_err(), "a broken persona must not pass silently");
        // A file that parses but invents a field is refused too — the shape is the contract.
        std::fs::write(
            d.join(PERSONA_FILE),
            r#"{"name":"Purr","allow_actuate":true}"#,
        )
        .unwrap();
        assert!(
            load(&d).is_err(),
            "persona.json grants capability over nothing; an unknown field is refused"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
