//! Who to ask — addressing a question to a person instead of broadcasting at a room.
//!
//! A familiar that serves several people can only ever *announce* until it knows who is here. So
//! questions and goals carry an **owner**: the human they are addressed to and who is accountable
//! for them. Routing picks that owner from whoever is actually present.
//!
//! Two properties this deliberately holds:
//!
//! - **Ownership governs who is ASKED, not who may READ.** Once an answer is confirmed it becomes
//!   an ordinary observation in the shared worldview — public to the mesh, like any other. A
//!   question is addressed; an answer is common property. (Ian, 2026-07-29.)
//! - **Addressing is not authorisation** (ADR-0019). Being routed a question grants nothing; it is
//!   the familiar choosing who to speak to, and the choice carries a confidence.
//!
//! Presence here is derived from the observation stream, which already names its actors — so this
//! module needs no mesh, no camera and no network, and stays a pure function of what was observed.

use crate::observation::Observation;

/// How recent an actor's own activity must be to count them as here. Much tighter than
/// [`presence::WITHDRAWAL_HORIZON_SECS`] (which asks "has this person abandoned us?"): to be
/// *asked* something you have to be here now, not merely not-gone.
pub const HERE_WINDOW_SECS: i64 = 30 * 60;

/// A human the familiar believes is present, and how strongly.
#[derive(Debug, Clone, PartialEq)]
pub struct PresentHuman {
    pub handle: String,
    /// [0,1]. Mirrors the client-side ladder's tiers (ADR-0019): a human who spoke is more surely
    /// here than one whose watch merely reported motion.
    pub confidence: f64,
    pub last_seen: i64,
}

/// The human named by an actor namespace: `phone:ian` → `ian`, a bare `ian` → `ian`.
/// Device-shaped actors with no human (`phone:`) name nobody. `pub` for the same reason
/// [`subject_and_strength`] is: one parse, used everywhere (thread's confirm flip
/// matches an answerer against `origin_human` with exactly this rule).
pub fn human_of(actor: &str) -> String {
    match actor.split_once(':') {
        Some((_, h)) => h.to_string(),
        None => actor.to_string(),
    }
}

/// **The evidence ladder — one definition, used everywhere.** Who does this observation say is
/// present, how strongly, and by what kind of evidence?
///
/// A human speaking to the familiar is the strongest ordinary signal; a recognised face is
/// stronger still (and names its subject in the *object*, not the actor); a device merely
/// reporting activity is the weakest thing that counts at all.
///
/// `observer` is excluded deliberately: it is the *absence* of a name, and treating it as a person
/// means addressing questions to nobody in particular.
///
/// This is `pub` because the ladder had already been forked once — `mesh::members::presence_evidence`
/// carries a parallel copy of the same tiers. Two ladders that must agree and are edited separately
/// is a bug waiting for a quiet afternoon; new callers use this one.
pub fn subject_and_strength(o: &Observation) -> Option<(String, f64, &'static str)> {
    let (who, strength, via) = if o.action == "recognized" && o.object.starts_with("face:") {
        (
            o.object
                .strip_prefix("face:")
                .unwrap_or_default()
                .to_string(),
            0.95,
            "face",
        )
    } else if o.source == "observer" || o.action == "answered" || o.action == "told the familiar" {
        (human_of(&o.actor), 0.9, "dialogue")
    } else if o.action == "adjusted" {
        // A hand on a control surface (ADR-0032's poller): attributed only when the room
        // held exactly one person, so it lands between motion and dialogue in strength.
        (human_of(&o.actor), 0.5, "adjusted")
    } else if o.action == "reports" && o.object == "presence" {
        (human_of(&o.actor), 0.4, "activity")
    } else if o.action == "reports" {
        (human_of(&o.actor), 0.6, "motion")
    } else {
        return None;
    };
    // A headless mesh node reporting about ITSELF (the lighthouse announcing presence) carries
    // no human — its actor is `mesh:<node>`, not `phone:<name>`. It is substrate too: a peer
    // to be aware of, never a person to serve. (A human's device still names its human in the
    // actor, e.g. `phone:ian`, and passes.)
    if o.actor.starts_with("mesh") {
        return None;
    }
    let who = who.trim().to_lowercase();
    // `observer` is the absence of a name; `someone` is the poller's honest shrug when
    // the room was empty or ambiguous. Neither is a person to route to or pattern.
    if who.is_empty() || who == "observer" || who == "someone" || is_substrate(&who) {
        return None;
    }
    Some((who, strength, via))
}

/// The substrate the familiar runs ON or WITH — the host, its hardware, the network, the
/// command line. These are **never someone to serve** (Law II: humanity is served, not the
/// machine). They inform awareness and decisions freely — a connectivity signal, hardware
/// capacity — but they are not people: never a dossier subject, never a present "human",
/// never a need theorized on their behalf. A familiar starved of humans must not mistake the
/// machine it runs on for one and start worrying whether the host feels seen.
pub fn is_substrate(actor: &str) -> bool {
    matches!(
        actor.trim().to_lowercase().as_str(),
        "host" | "local_hardware" | "hardware" | "network" | "cli" | "familiar"
    )
}

/// Everyone the familiar has fresh evidence for, strongest first. `observer` is excluded: it is the
/// *absence* of a name, and addressing a question to "observer" is broadcasting with extra steps.
pub fn present_humans(obs: &[Observation], now: i64) -> Vec<PresentHuman> {
    let mut best: Vec<PresentHuman> = Vec::new();
    for o in obs {
        if now - o.ts > HERE_WINDOW_SECS || o.ts > now {
            continue;
        }
        let Some((who, conf, _via)) = subject_and_strength(o) else {
            continue;
        };
        match best.iter_mut().find(|p| p.handle == who) {
            Some(p) => {
                // Strongest evidence wins; freshness breaks ties.
                if conf > p.confidence || (conf == p.confidence && o.ts > p.last_seen) {
                    p.confidence = p.confidence.max(conf);
                    p.last_seen = p.last_seen.max(o.ts);
                }
            }
            None => best.push(PresentHuman {
                handle: who,
                confidence: conf,
                last_seen: o.ts,
            }),
        }
    }
    best.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.last_seen.cmp(&a.last_seen))
    });
    best
}

/// Choose who to address a question to.
///
/// - An existing owner keeps it **if they are here** — a question does not get taken away from the
///   person it was put to just because someone else walked in.
/// - Otherwise the human whose need it serves, if they are here. This is Law I: the question exists
///   for them.
/// - Otherwise the most surely-present human.
/// - Otherwise nobody, and the caller must not ask. An unaddressed question in an empty room is the
///   thing this whole mechanism exists to avoid.
pub fn route(current_owner: &str, origin_human: &str, present: &[PresentHuman]) -> String {
    let here = |h: &str| {
        !h.is_empty()
            && present
                .iter()
                .any(|p| p.handle.eq_ignore_ascii_case(h.trim()))
    };
    if here(current_owner) {
        return current_owner.trim().to_lowercase();
    }
    if here(origin_human) {
        return origin_human.trim().to_lowercase();
    }
    present
        .first()
        .map(|p| p.handle.clone())
        .unwrap_or_default()
}

/// Has the person a question is addressed to gone quiet? Used to hand a stale question to whoever
/// is actually here rather than leaving it addressed to an empty chair.
pub fn owner_is_absent(owner: &str, present: &[PresentHuman]) -> bool {
    !owner.trim().is_empty()
        && !present
            .iter()
            .any(|p| p.handle.eq_ignore_ascii_case(owner.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presence;

    const NOW: i64 = 1_785_000_000;

    fn ob(actor: &str, action: &str, object: &str, source: &str, ts: i64) -> Observation {
        Observation::new(actor, action, object, "", source, ts, 0.9)
    }

    #[test]
    fn present_humans_ranks_by_evidence_then_freshness() {
        let obs = vec![
            ob("watch:betty", "reports", "presence", "device", NOW - 60),
            ob(
                "phone:ian",
                "told the familiar",
                "hello",
                "device",
                NOW - 300,
            ),
            ob("ipad:kai", "reports", "motion:walking", "device", NOW - 30),
        ];
        let p = present_humans(&obs, NOW);
        assert_eq!(p.len(), 3);
        assert_eq!(
            p[0].handle, "ian",
            "someone who SPOKE outranks a device beacon"
        );
        assert_eq!(p[1].handle, "kai");
        assert_eq!(
            p[2].handle, "betty",
            "a bare presence beacon is the weakest thing that counts"
        );
    }

    #[test]
    fn a_recognised_face_names_its_subject_not_the_device() {
        let obs = vec![ob(
            "ipad:observer",
            "recognized",
            "face:betty",
            "device",
            NOW - 100,
        )];
        let p = present_humans(&obs, NOW);
        assert_eq!(p.len(), 1);
        assert_eq!(
            p[0].handle, "betty",
            "the face names betty, not the iPad's actor"
        );
        assert!(p[0].confidence > 0.9);
    }

    #[test]
    fn stale_and_anonymous_evidence_does_not_make_anyone_present() {
        let obs = vec![
            ob(
                "phone:ian",
                "told the familiar",
                "hi",
                "device",
                NOW - HERE_WINDOW_SECS - 1,
            ),
            ob("observer", "answered", "something", "observer", NOW - 10),
            ob("phone:", "reports", "presence", "device", NOW - 10),
        ];
        let p = present_humans(&obs, NOW);
        assert!(
            p.is_empty(),
            "stale evidence, an unnamed observer and a human-less device all name nobody: {p:?}"
        );
    }

    #[test]
    fn an_owner_who_is_here_keeps_the_question() {
        let present = vec![
            PresentHuman {
                handle: "betty".into(),
                confidence: 0.9,
                last_seen: NOW,
            },
            PresentHuman {
                handle: "ian".into(),
                confidence: 0.4,
                last_seen: NOW,
            },
        ];
        // Betty is the more surely-present, but the question was put to Ian and he is here.
        assert_eq!(route("ian", "", &present), "ian");
    }

    #[test]
    fn an_absent_owner_hands_off_to_whoever_the_question_serves() {
        let present = vec![
            PresentHuman {
                handle: "betty".into(),
                confidence: 0.4,
                last_seen: NOW,
            },
            PresentHuman {
                handle: "kai".into(),
                confidence: 0.9,
                last_seen: NOW,
            },
        ];
        // Ian is gone; the question exists for Betty's need, so it goes to Betty — not to Kai
        // merely for being the most confidently present.
        assert_eq!(route("ian", "betty", &present), "betty");
        assert!(owner_is_absent("ian", &present));
        assert!(!owner_is_absent("betty", &present));
    }

    #[test]
    fn with_no_one_to_serve_it_goes_to_whoever_is_most_surely_here() {
        let present = vec![
            PresentHuman {
                handle: "kai".into(),
                confidence: 0.9,
                last_seen: NOW,
            },
            PresentHuman {
                handle: "betty".into(),
                confidence: 0.4,
                last_seen: NOW,
            },
        ];
        assert_eq!(route("", "", &present), "kai");
    }

    #[test]
    fn an_empty_room_addresses_nobody() {
        assert_eq!(route("ian", "betty", &[]), "");
        assert_eq!(route("", "", &[]), "");
    }

    #[test]
    fn presence_here_is_tighter_than_the_withdrawal_horizon() {
        // Being askable is a stricter test than not-having-abandoned-us.
        assert!(HERE_WINDOW_SECS < presence::WITHDRAWAL_HORIZON_SECS);
    }

    #[test]
    fn an_attributed_adjustment_counts_but_someone_does_not() {
        // A hand on a control surface, attributed to its sole present human (ADR-0032):
        // between motion (0.6 for a device report) and the beacon tier.
        let o = ob("ian", "adjusted", "lights=dim", "actuator", NOW);
        let (who, strength, via) = subject_and_strength(&o).unwrap();
        assert_eq!((who.as_str(), via), ("ian", "adjusted"));
        assert!((strength - 0.5).abs() < f64::EPSILON);
        // The poller's honest shrug names nobody — like `observer`, excluded.
        let anon = ob("someone", "adjusted", "lights=dim", "actuator", NOW);
        assert!(subject_and_strength(&anon).is_none());
    }

    #[test]
    fn the_substrate_is_never_a_person_to_serve() {
        // The machine the familiar runs on is not someone it serves (Law II). A host
        // reporting its own connectivity used to read as a human named "host" — folding it
        // into the dossier and theorizing its "needs." No longer.
        assert!(subject_and_strength(&ob("host", "reports", "connectivity:online", "sensor", NOW)).is_none());
        assert!(subject_and_strength(&ob("local_hardware", "reports", "cpu:busy", "sensor", NOW)).is_none());
        assert!(subject_and_strength(&ob("network", "reports", "up", "sensor", NOW)).is_none());
        // A headless mesh node reporting its own presence (the lighthouse) is a peer, not a person.
        assert!(subject_and_strength(&ob("mesh:f56e5601d0cd", "reports", "presence", "mesh:x", NOW)).is_none());
        // But a real human's device still names its human and passes.
        let (who, _, _) = subject_and_strength(&ob("phone:ian", "reports", "motion:walking", "device", NOW)).unwrap();
        assert_eq!(who, "ian");
    }

    #[test]
    fn is_substrate_knows_the_machine_from_the_person() {
        assert!(is_substrate("host") && is_substrate("HOST") && is_substrate("local_hardware"));
        assert!(is_substrate("network") && is_substrate("familiar") && is_substrate("cli"));
        assert!(!is_substrate("ian") && !is_substrate("betty"));
    }
}
