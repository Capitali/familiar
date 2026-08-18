//! One prowl. Sniffs every way into United Cat Foods, times each, and brings back what it
//! found — including what it *failed* to find, which is half of what a cat is for.
//!
//! Nothing here writes. The cat opens a session and calls read-only tools exactly as any other
//! client would, so what it shows is what the flap really does, not a re-enactment of it.

use std::path::Path;
use std::time::Instant;

use familiar_kernel::guard::{self, Action, ActionKind, Decision};
use familiar_kernel::{boundary, observation};
use familiar_mcp::{ServerSet, Session};
use serde_json::{json, Value};

use crate::catch::{taste, Haul, Kibble, Perch, Purr, Tomcat, Yowl};

/// One pounce, and how long the cat was in the air. `Err` carries words meant for a human
/// reading a panel — the server's own, or the flap's refusal.
pub struct Pounce<T> {
    pub caught: Result<T, String>,
    pub millis: u128,
}

impl<T> Pounce<T> {
    fn missed(why: impl Into<String>) -> Self {
        Self {
            caught: Err(why.into()),
            millis: 0,
        }
    }
    pub fn got(&self) -> Option<&T> {
        self.caught.as_ref().ok()
    }
    pub fn why_not(&self) -> Option<&str> {
        self.caught.as_ref().err().map(String::as_str)
    }
}

/// The collar — what the human decided this cat is allowed to touch (`mcp/servers.json`),
/// re-read every prowl so an edit lands without a restart.
pub struct Collar {
    pub name: String,
    pub url: String,
    pub tools: Vec<String>,
    pub note: String,
    /// Whether the declared key file yields a token — read as a *presence*, never held here.
    /// A cat has no business carrying a bearer token around a rendering path.
    pub has_tag: bool,
    /// `Some` when the file is missing or names no such server: a cat staring at nothing must
    /// say so rather than sit smugly in front of an empty frame.
    pub problem: Option<String>,
}

/// The cat flap. Whether this reach is permitted, asked at the moment the cat would go
/// through it — the same gate `familiar_mcp::permitted` consults, asked the same way.
pub struct CatFlap {
    pub allow_network: bool,
    pub open: bool,
    pub rationale: String,
}

/// The nose boop — what the wire said about itself when the two of them said hello.
pub struct NoseBoop {
    pub server_name: String,
    pub server_version: String,
    pub protocol_version: String,
    pub wearing_tag: bool,
    pub millis: u128,
}

/// Paw prints: the familiar's own footprint on this flap, read from local state rather than
/// asserted.
///
/// The cat deliberately does **not** hardcode "the tick loop never goes through here" — that
/// would be a hand-written paraphrase of the code, the exact drift this project keeps finding.
/// It counts what local state actually records. If the metabolism ever starts hunting the
/// exchange, these numbers move on their own and the panel stops reading zero.
pub struct PawPrints {
    pub observations_total: usize,
    pub observations_naming_flap: usize,
    pub last_print: Option<String>,
}

/// Everything one prowl turned up.
pub struct Prowl {
    /// Which litter tray this cat actually sniffed. Shown on screen: an instrument that will
    /// not say where it is looking can be watching the wrong familiar and look perfectly smug.
    pub tray: String,
    pub collar: Collar,
    pub flap: CatFlap,
    pub boop: Result<NoseBoop, String>,
    pub on_offer: Pounce<Vec<String>>,
    pub purr: Pounce<Purr>,
    pub perches: Pounce<Vec<Perch>>,
    pub bowls: Pounce<Vec<Kibble>>,
    pub yowls: Pounce<Vec<Yowl>>,
    pub tomcats: Pounce<Vec<Tomcat>>,
    pub hauls: Pounce<Vec<Haul>>,
    pub prints: PawPrints,
    pub at: i64,
}

impl Prowl {
    /// Tools on offer that the collar does **not** name. Discovery is not permission
    /// (ADR-0032) — these are visible and untouchable, and the cat names them because a new
    /// one appearing is exactly the event a human needs to decide about.
    pub fn uncollared(&self) -> Vec<String> {
        match self.on_offer.got() {
            Some(o) => o
                .iter()
                .filter(|t| !self.collar.tools.iter().any(|d| d == *t))
                .cloned()
                .collect(),
            None => Vec::new(),
        }
    }

    /// Tools the collar names that the server no longer offers — a collar pointing at a bowl
    /// that is not there any more. Silent in every other view; a real drift signal here.
    pub fn collared_but_gone(&self) -> Vec<String> {
        match self.on_offer.got() {
            Some(o) => self
                .collar
                .tools
                .iter()
                .filter(|d| !o.iter().any(|t| t == *d))
                .cloned()
                .collect(),
            None => Vec::new(),
        }
    }

    /// Hauls the exchange itself marks as ours.
    pub fn our_hauls(&self) -> Vec<&Haul> {
        self.hauls
            .got()
            .map(|h| h.iter().filter(|x| x.ours).collect())
            .unwrap_or_default()
    }
}

fn pounce<T>(f: impl FnOnce() -> Result<T, String>) -> Pounce<T> {
    let t = Instant::now();
    let caught = f();
    Pounce {
        caught,
        millis: t.elapsed().as_millis(),
    }
}

fn paw_at<T: for<'de> serde::Deserialize<'de>>(
    session: &mut Option<Session>,
    tool: &str,
    args: Value,
) -> Pounce<T> {
    let Some(s) = session.as_mut() else {
        return Pounce::missed("no session");
    };
    pounce(|| {
        let answer: Value = s.call(tool, args).map_err(|e| e.to_string())?;
        taste(&answer, tool)
    })
}

/// Count the paw prints. Best-effort: a cat must not fall over because the daemon is holding
/// a write lock on the database.
fn paw_prints(tray: &Path, server: &str) -> PawPrints {
    // Never open a database that is not already there. `observation::load` would CREATE one,
    // and a cat that leaves a stray `familiar.db` in whatever room it was carried into has
    // stopped being a watcher — that is the one promise this whole crate makes.
    if !tray.join(familiar_kernel::store::DB_FILE).is_file() {
        return PawPrints {
            observations_total: 0,
            observations_naming_flap: 0,
            last_print: None,
        };
    }
    let all = observation::load(tray).unwrap_or_default();
    let scent = server.to_ascii_lowercase();
    let naming: Vec<&observation::Observation> = all
        .iter()
        .filter(|o| {
            let trail = format!("{} {} {} {}", o.source, o.actor, o.action, o.object)
                .to_lowercase()
                + &o.context.to_lowercase();
            trail.contains(&scent) || trail.contains("mcp")
        })
        .collect();
    PawPrints {
        observations_total: all.len(),
        observations_naming_flap: naming.len(),
        last_print: naming
            .last()
            .map(|o| format!("{} {} {} {}", o.id, o.actor, o.action, o.object)),
    }
}

/// Go and have a look at everything.
///
/// `session` is carried between prowls so the nose boop is paid for once, and re-opened when a
/// prowl finds it dead — which is itself worth showing, so the reconnect is reported rather
/// than quietly papered over.
pub fn prowl(tray: &Path, server: &str, session: &mut Option<Session>, now: i64) -> Prowl {
    // 1. The collar — the human's allowlist, re-read every prowl.
    let set = ServerSet::load(tray);
    let naked = |e: String| Collar {
        name: server.into(),
        url: String::new(),
        tools: Vec::new(),
        note: String::new(),
        has_tag: false,
        problem: Some(e),
    };
    let collar = match set.as_ref().map(|s| s.get(server)) {
        Ok(Ok(s)) => Collar {
            name: s.name.clone(),
            url: s.url.clone(),
            tools: s.tools.clone(),
            note: s.note.clone(),
            has_tag: matches!(s.token(tray), Ok(Some(_))),
            problem: None,
        },
        Ok(Err(e)) => naked(e.to_string()),
        Err(e) => naked(e.to_string()),
    };

    // 2. The flap — asked exactly as the client asks it, at the moment of looking.
    let b = boundary::load(tray).unwrap_or_else(|_| boundary::Boundary::closed());
    let origin = familiar_mcp::Url::parse(&collar.url)
        .map(|u| u.origin())
        .unwrap_or_else(|_| collar.url.clone());
    let verdict = guard::evaluate(&Action::new(ActionKind::Network, origin), &b);
    let flap = CatFlap {
        allow_network: b.allow_network,
        open: verdict.decision == Decision::Allow,
        rationale: verdict.rationale,
    };

    // 3. Hello. A shut flap is not an outage to report as one — it is the boundary working, so
    //    the cat says so in the guard's own words and does not go through.
    if !flap.open {
        *session = None;
    }

    let mut boop_ms = 0u128;
    let mut boop_err: Option<String> = None;
    if flap.open && session.is_none() {
        let t = Instant::now();
        match Session::open(tray, server) {
            Ok(s) => {
                *session = Some(s);
                boop_ms = t.elapsed().as_millis();
            }
            Err(e) => boop_err = Some(e.to_string()),
        }
    }

    let boop = match (session.as_ref(), boop_err) {
        (Some(s), _) => Ok(NoseBoop {
            server_name: s.server_name.clone(),
            server_version: s.server_version.clone(),
            protocol_version: s.protocol_version.clone(),
            wearing_tag: collar.has_tag,
            millis: boop_ms,
        }),
        (None, Some(e)) => Err(e),
        (None, None) => Err(flap.rationale.clone()),
    };

    // 4. What is on offer, and 5. the world on the other side.
    let on_offer = {
        let t: Pounce<Vec<familiar_mcp::Tool>> = match session.as_mut() {
            Some(s) => pounce(|| s.tools().map_err(|e| e.to_string())),
            None => Pounce::missed("no session"),
        };
        Pounce {
            millis: t.millis,
            caught: t.caught.map(|v| v.into_iter().map(|x| x.name).collect()),
        }
    };

    let purr = paw_at::<Purr>(session, "ucf_status", json!({}));
    let perches = paw_at::<Vec<Perch>>(session, "ucf_stations", json!({}));
    let bowls = paw_at::<Vec<Kibble>>(session, "ucf_prices", json!({}));
    let yowls = paw_at::<Vec<Yowl>>(session, "ucf_news", json!({}));
    let tomcats = paw_at::<Vec<Tomcat>>(session, "ucf_carriers", json!({}));
    let hauls = paw_at::<Vec<Haul>>(session, "ucf_loadboard", json!({}));

    // A dead session shows itself as every pounce missing; drop it so the next prowl boops
    // again and the reconnect is visible as a fresh handshake time.
    if purr.caught.is_err() && on_offer.caught.is_err() {
        *session = None;
    }

    Prowl {
        tray: tray.display().to_string(),
        prints: paw_prints(tray, &collar.name),
        collar,
        flap,
        boop,
        on_offer,
        purr,
        perches,
        bowls,
        yowls,
        tomcats,
        hauls,
        at: now,
    }
}

/// Prowl-builders the grooming tests share. Test-only: a cat has no production reason to
/// invent a prowl it did not go on.
#[cfg(test)]
pub mod pretend {
    use super::*;

    /// A prowl carrying nothing but a tick and a table of bowls — enough to exercise movement.
    pub fn prowl_with_bowls(bowls: &[(&str, f64)], tick: i64) -> Prowl {
        let ks: Vec<Kibble> = bowls
            .iter()
            .map(|(k, mid)| {
                let (good, station) = k.split_once('@').unwrap_or((k, ""));
                Kibble {
                    good: good.into(),
                    station: station.into(),
                    mid: *mid,
                    stock: 1,
                }
            })
            .collect();
        let mut p = super::tests::bare_prowl(&[], None);
        p.bowls = Pounce {
            caught: Ok(ks),
            millis: 1,
        };
        p.purr = Pounce {
            caught: Ok(Purr {
                tick,
                ..Default::default()
            }),
            millis: 1,
        };
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn bare_prowl(collared: &[&str], on_offer: Option<Vec<String>>) -> Prowl {
        Prowl {
            tray: "/tmp/test".into(),
            collar: Collar {
                name: "ucf".into(),
                url: "https://example.test/mcp".into(),
                tools: collared.iter().map(|s| s.to_string()).collect(),
                note: String::new(),
                has_tag: true,
                problem: None,
            },
            flap: CatFlap {
                allow_network: true,
                open: true,
                rationale: String::new(),
            },
            boop: Err("unused".into()),
            on_offer: match on_offer {
                Some(v) => Pounce {
                    caught: Ok(v),
                    millis: 1,
                },
                None => Pounce::missed("no session"),
            },
            purr: Pounce::missed("no session"),
            perches: Pounce::missed("no session"),
            bowls: Pounce::missed("no session"),
            yowls: Pounce::missed("no session"),
            tomcats: Pounce::missed("no session"),
            hauls: Pounce::missed("no session"),
            prints: PawPrints {
                observations_total: 0,
                observations_naming_flap: 0,
                last_print: None,
            },
            at: 0,
        }
    }

    /// The event a human must decide about: the partner grew a tool nobody put on the collar.
    #[test]
    fn a_new_tool_on_offer_is_named_as_uncollared() {
        let p = bare_prowl(
            &["ucf_status"],
            Some(vec!["ucf_status".into(), "ucf_place_order".into()]),
        );
        assert_eq!(p.uncollared(), vec!["ucf_place_order".to_string()]);
        assert!(p.collared_but_gone().is_empty());
    }

    /// The other drift direction: a collar naming a bowl that is not there any more.
    #[test]
    fn a_collar_pointing_at_nothing_is_named_too() {
        let p = bare_prowl(
            &["ucf_status", "ucf_retired"],
            Some(vec!["ucf_status".into()]),
        );
        assert_eq!(p.collared_but_gone(), vec!["ucf_retired".to_string()]);
        assert!(p.uncollared().is_empty());
    }

    /// Without a session there is no evidence either way, and a cat must not manufacture a
    /// drift report out of a nap.
    #[test]
    fn no_offer_list_means_no_drift_claim() {
        let p = bare_prowl(&["ucf_status"], None);
        assert!(p.uncollared().is_empty());
        assert!(p.collared_but_gone().is_empty());
    }
}
