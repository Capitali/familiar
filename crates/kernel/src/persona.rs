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

use serde::{Deserialize, Serialize};

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

/// Bounded STYLE axes — cadence, never judgment (T-236 dialogue, Q5 ruling). Every
/// axis here may bend how a line sounds; none may bend what it says. Candor,
/// uncertainty, risk-talk, urgency, refusal semantics, deference, consent, spending
/// posture, and what is remembered are deliberately NOT representable: they are
/// judgment or record policy, and every voice tells the same truth and stops at the
/// same gate. Renderers must zero `humor` around danger, loss, refusal, or
/// uncertainty, and `sentence_length` may shorten a line but never drop its source,
/// amount, deadline, consequence, or correction path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Style {
    /// 0 = clipped and cool, 10 = openly fond. Default 5.
    #[serde(default = "five")]
    pub warmth: u8,
    /// 0 = shipmate-casual, 10 = full ceremony. Default 5.
    #[serde(default = "five")]
    pub formality: u8,
    /// 0 = bone dry, 10 = irrepressible. Reads as ZERO around danger/refusal.
    #[serde(default = "five")]
    pub humor: u8,
    /// 0 = terse, 10 = expansive. Never omits load-bearing facts.
    #[serde(default = "five")]
    pub sentence_length: u8,
    /// Contractions in the voice ("she's" vs "she is").
    #[serde(default = "yes")]
    pub contractions: bool,
    /// Vocabulary flavour: "plain", "feline", or "nautical".
    #[serde(default = "plain")]
    pub vocabulary: String,
    /// The standing greeting, if the captain set one. Short.
    #[serde(default)]
    pub greeting: String,
    /// How the computer addresses its human ("Captain", a name, …). Short.
    #[serde(default = "captain")]
    pub form_of_address: String,
}

fn five() -> u8 {
    5
}
fn yes() -> bool {
    true
}
fn plain() -> String {
    "plain".to_string()
}
fn captain() -> String {
    "Captain".to_string()
}

impl Default for Style {
    fn default() -> Self {
        serde_json::from_str("{}").expect("defaults parse")
    }
}

impl Style {
    /// The bounds, loudly. A style outside them is refused, not clamped — a human
    /// who wrote 30 warmth would otherwise be told nothing while a different voice
    /// spoke in their name (the same discipline as the loader's).
    pub fn validate(&self) -> Result<(), String> {
        for (axis, v) in [
            ("warmth", self.warmth),
            ("formality", self.formality),
            ("humor", self.humor),
            ("sentence_length", self.sentence_length),
        ] {
            if v > 10 {
                return Err(format!("style.{axis} is {v}; the axis runs 0..=10"));
            }
        }
        if !["plain", "feline", "nautical"].contains(&self.vocabulary.as_str()) {
            return Err(format!(
                "style.vocabulary {:?} is not one of plain|feline|nautical",
                self.vocabulary
            ));
        }
        if self.greeting.len() > 120 {
            return Err("style.greeting runs past 120 bytes".to_string());
        }
        if self.form_of_address.len() > 40 {
            return Err("style.form_of_address runs past 40 bytes".to_string());
        }
        Ok(())
    }
}

/// A voice: who this instance says it is, and how it speaks. Never what it may do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// The bounded style block — v2 only. A v1 file carrying it is refused: the
    /// version names the contract, and a style someone wrote under the wrong
    /// contract must be heard about, not half-honoured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<Style>,
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
            style: None,
        }
    }
}

impl Persona {
    /// The role sentence a prompt carries, with the served person's name in place.
    pub fn role_line(&self, who: &str) -> String {
        format!("You are {}.", self.role.replace("{who}", who))
    }

    /// The contract's own consistency: versions 1 and 2 exist; style rides only on
    /// v2; a style's axes hold their bounds.
    pub fn validate(&self) -> Result<(), String> {
        match self.persona_version {
            1 => {
                if self.style.is_some() {
                    return Err(
                        "persona_version 1 cannot carry a style block; set persona_version 2"
                            .to_string(),
                    );
                }
            }
            2 => {}
            v => {
                return Err(format!(
                    "persona_version {v} is not a contract this build knows"
                ))
            }
        }
        if self.name.trim().is_empty() {
            return Err("a persona must have a name".to_string());
        }
        if self.name.len() > 80 {
            return Err("the name runs past 80 bytes".to_string());
        }
        if let Some(style) = &self.style {
            style.validate()?;
        }
        Ok(())
    }
}

/// The root name every ship's computer descends from (ADR-0037): the default before
/// the captain's naming ceremony — written EXACTLY, never generated around.
pub const ROOT_NAME: &str = "Purr";

/// The naming trail file: one typed event per naming act, append-only, beside the
/// persona in the same store. Provenance lives here, not as style fields.
pub const NAME_EVENTS_FILE: &str = "persona-names.jsonl";

/// One naming act: who called this computer what, when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameEvent {
    pub at: i64,
    /// The human act behind the name ("pairing", or the captain's label).
    pub actor: String,
    pub name: String,
}

/// Write a persona atomically (tmp + rename): a crash mid-write must never leave a
/// half-voice for the loader to refuse. Validates first — nothing invalid lands.
pub fn write(dir: &Path, persona: &Persona) -> io::Result<()> {
    persona
        .validate()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let bytes = serde_json::to_vec_pretty(persona)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let tmp = dir.join(format!("{PERSONA_FILE}.tmp"));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, dir.join(PERSONA_FILE))
}

/// Append one naming act to the trail.
pub fn record_naming(dir: &Path, event: &NameEvent) -> io::Result<()> {
    use std::io::Write as _;
    let line = serde_json::to_string(event)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(NAME_EVENTS_FILE))?;
    writeln!(f, "{line}")
}

/// The naming trail, oldest first. Absent = never named beyond its default.
pub fn namings(dir: &Path) -> Vec<NameEvent> {
    std::fs::read_to_string(dir.join(NAME_EVENTS_FILE))
        .map(|raw| {
            raw.lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        })
        .unwrap_or_default()
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
    let persona: Persona = serde_json::from_str(&raw).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{PERSONA_FILE} is present but unreadable: {e}"),
        )
    })?;
    persona.validate().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{PERSONA_FILE} is present but invalid: {e}"),
        )
    })?;
    Ok(persona)
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

    /// The v2 contract: a style rides only on version 2, its axes hold their
    /// bounds, and every violation is LOUD — never a silent clamp or fallback.
    #[test]
    fn the_style_block_is_v2_only_bounded_and_loud() {
        let d = tmp("style");
        // A valid v2 persona round-trips through the atomic writer.
        let mut p = Persona {
            persona_version: 2,
            name: "Whisker Belle".into(),
            style: Some(Style {
                warmth: 8,
                vocabulary: "feline".into(),
                form_of_address: "Captain".into(),
                ..Style::default()
            }),
            ..Persona::default()
        };
        write(&d, &p).unwrap();
        assert_eq!(load(&d).unwrap(), p);
        // A v1 file carrying style is refused by the loader…
        p.persona_version = 1;
        std::fs::write(d.join(PERSONA_FILE), serde_json::to_vec_pretty(&p).unwrap()).unwrap();
        assert!(load(&d).is_err(), "v1 + style must be loud");
        // …and by the writer.
        assert!(write(&d, &p).is_err());
        // Out-of-bounds axes and unknown vocabularies are refused.
        p.persona_version = 2;
        p.style.as_mut().unwrap().warmth = 30;
        assert!(write(&d, &p).is_err());
        p.style.as_mut().unwrap().warmth = 5;
        p.style.as_mut().unwrap().vocabulary = "piratical".into();
        assert!(write(&d, &p).is_err());
        // An unknown future version is a contract this build refuses to guess at.
        p.style = None;
        p.persona_version = 9;
        assert!(write(&d, &p).is_err());
    }

    /// Two ships, two voices, no bleed: renaming one changes no byte in the other
    /// (T-236 brick-1 acceptance; the store IS the partition, ADR-0045).
    #[test]
    fn renaming_one_ship_changes_no_byte_in_another() {
        let a = tmp("ship_a");
        let b = tmp("ship_b");
        for (d, name) in [(&a, "Purr"), (&b, "Purr")] {
            write(
                d,
                &Persona {
                    persona_version: 2,
                    name: name.to_string(),
                    ..Persona::default()
                },
            )
            .unwrap();
            record_naming(
                d,
                &NameEvent {
                    at: 100,
                    actor: "pairing".into(),
                    name: name.to_string(),
                },
            )
            .unwrap();
        }
        let b_persona_before = std::fs::read(b.join(PERSONA_FILE)).unwrap();
        let b_trail_before = std::fs::read(b.join(NAME_EVENTS_FILE)).unwrap();
        // The captain renames A.
        let mut pa = load(&a).unwrap();
        pa.name = "Mrs. Norris".into();
        write(&a, &pa).unwrap();
        record_naming(
            &a,
            &NameEvent {
                at: 200,
                actor: "jeff".into(),
                name: "Mrs. Norris".into(),
            },
        )
        .unwrap();
        assert_eq!(load(&a).unwrap().name, "Mrs. Norris");
        assert_eq!(load(&b).unwrap().name, "Purr", "B keeps its own name");
        assert_eq!(
            std::fs::read(b.join(PERSONA_FILE)).unwrap(),
            b_persona_before
        );
        assert_eq!(
            std::fs::read(b.join(NAME_EVENTS_FILE)).unwrap(),
            b_trail_before
        );
        // And A's provenance trail tells the whole naming story, oldest first.
        let trail = namings(&a);
        assert_eq!(
            trail.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            vec!["Purr", "Mrs. Norris"]
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
