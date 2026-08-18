//! One round of looking. Gathers every interface to the exchange, times each, and returns
//! what it found — including what it *failed* to find, which is half of what a monitor is for.
//!
//! Nothing here writes. The monitor opens a session and calls read-only tools exactly as any
//! other client would, so what it shows is what the seam really does, not a simulation of it.

use std::path::Path;
use std::time::Instant;

use familiar_kernel::guard::{self, Action, ActionKind, Decision};
use familiar_kernel::{boundary, observation};
use familiar_mcp::{ServerSet, Session};
use serde_json::{json, Value};

use crate::world::{decode, Carrier, Load, News, Price, Station, Status};

/// One tool call's outcome, with the wall time it took. `Err` carries words meant for a human
/// reading a panel — the server's own, or the client's refusal.
pub struct Timed<T> {
    pub value: Result<T, String>,
    pub millis: u128,
}

impl<T> Timed<T> {
    fn err(msg: impl Into<String>) -> Self {
        Self {
            value: Err(msg.into()),
            millis: 0,
        }
    }
    pub fn ok(&self) -> Option<&T> {
        self.value.as_ref().ok()
    }
    pub fn why(&self) -> Option<&str> {
        self.value.as_ref().err().map(String::as_str)
    }
}

/// What the human's declaration says — the allowlist, read fresh each round so an edit to
/// `mcp/servers.json` shows up without a restart.
pub struct Declared {
    pub name: String,
    pub url: String,
    pub tools: Vec<String>,
    pub note: String,
    /// Whether the declared key file yields a token — read as a *presence*, never held here.
    /// The monitor has no business carrying a bearer token around a rendering path.
    pub has_token: bool,
    /// `Err` when the file is missing or names no such server: a monitor pointed at nothing
    /// must say so rather than draw an empty frame.
    pub problem: Option<String>,
}

/// The boundary's verdict on reaching this origin, taken at the same moment the call would
/// be. This is the gate `familiar_mcp::permitted` consults, asked the same way.
pub struct Gate {
    pub allow_network: bool,
    pub allowed: bool,
    pub rationale: String,
}

/// What the wire said about itself at `initialize`.
pub struct Handshake {
    pub server_name: String,
    pub server_version: String,
    pub protocol_version: String,
    pub authenticated: bool,
    pub millis: u128,
}

/// The familiar's own footprint on this seam, read from local state rather than asserted.
///
/// The monitor deliberately does **not** hardcode "the tick loop never calls this" — that
/// would be a hand-written paraphrase of the code, the exact drift this project keeps
/// finding. It counts what local state actually records. If the metabolism ever starts
/// calling the exchange, these numbers move on their own and the panel stops saying zero.
pub struct Footprint {
    pub observations_total: usize,
    pub observations_naming_seam: usize,
    pub last_seam_observation: Option<String>,
}

/// Everything one round looked at.
pub struct Round {
    pub declared: Declared,
    pub gate: Gate,
    pub handshake: Result<Handshake, String>,
    pub offered: Timed<Vec<String>>,
    pub status: Timed<Status>,
    pub stations: Timed<Vec<Station>>,
    pub prices: Timed<Vec<Price>>,
    pub news: Timed<Vec<News>>,
    pub carriers: Timed<Vec<Carrier>>,
    pub loads: Timed<Vec<Load>>,
    pub footprint: Footprint,
    pub at: i64,
}

impl Round {
    /// Tools the server offers that the human has **not** declared. Discovery is not
    /// permission (ADR-0032) — these are visible and uncallable, and the monitor names them
    /// because a new one appearing is exactly the event a human needs to decide about.
    pub fn undeclared_on_offer(&self) -> Vec<String> {
        match self.offered.ok() {
            Some(o) => o
                .iter()
                .filter(|t| !self.declared.tools.iter().any(|d| d == *t))
                .cloned()
                .collect(),
            None => Vec::new(),
        }
    }

    /// Tools the human declared that the server no longer offers — a declaration pointing at
    /// something gone. Silent in every other view; a real drift signal here.
    pub fn declared_but_gone(&self) -> Vec<String> {
        match self.offered.ok() {
            Some(o) => self
                .declared
                .tools
                .iter()
                .filter(|d| !o.iter().any(|t| t == *d))
                .cloned()
                .collect(),
            None => Vec::new(),
        }
    }

    /// Loads the exchange itself marks as belonging to this token holder.
    pub fn mine(&self) -> Vec<&Load> {
        self.loads
            .ok()
            .map(|l| l.iter().filter(|x| x.mine).collect())
            .unwrap_or_default()
    }
}

fn timed<T>(f: impl FnOnce() -> Result<T, String>) -> Timed<T> {
    let t = Instant::now();
    let value = f();
    Timed {
        value,
        millis: t.elapsed().as_millis(),
    }
}

fn call<T: for<'de> serde::Deserialize<'de>>(
    session: &mut Option<Session>,
    tool: &str,
    args: Value,
) -> Timed<T> {
    let Some(s) = session.as_mut() else {
        return Timed::err("no session");
    };
    timed(|| {
        let answer: Value = s.call(tool, args).map_err(|e| e.to_string())?;
        decode(&answer, tool)
    })
}

/// Read the local footprint. Best-effort: a monitor must not fall over because the daemon
/// holds a write lock on the database.
fn footprint(dir: &Path, server: &str) -> Footprint {
    let all = observation::load(dir).unwrap_or_default();
    let needle = server.to_ascii_lowercase();
    let naming: Vec<&observation::Observation> = all
        .iter()
        .filter(|o| {
            let hay = format!("{} {} {} {}", o.source, o.actor, o.action, o.object).to_lowercase()
                + &o.context.to_lowercase();
            hay.contains(&needle) || hay.contains("mcp")
        })
        .collect();
    Footprint {
        observations_total: all.len(),
        observations_naming_seam: naming.len(),
        last_seam_observation: naming
            .last()
            .map(|o| format!("{} {} {} {}", o.id, o.actor, o.action, o.object)),
    }
}

/// Look once at every interface.
///
/// `session` is carried across rounds so the handshake cost is paid once, and re-opened when
/// a round finds it dead — which is itself worth showing, so the reconnect is reported rather
/// than hidden.
pub fn round(dir: &Path, server: &str, session: &mut Option<Session>, now: i64) -> Round {
    // 1. The declaration — the human's allowlist, re-read every round.
    let set = ServerSet::load(dir);
    let missing = |e: String| Declared {
        name: server.into(),
        url: String::new(),
        tools: Vec::new(),
        note: String::new(),
        has_token: false,
        problem: Some(e),
    };
    let declared = match set.as_ref().map(|s| s.get(server)) {
        Ok(Ok(s)) => Declared {
            name: s.name.clone(),
            url: s.url.clone(),
            tools: s.tools.clone(),
            note: s.note.clone(),
            has_token: matches!(s.token(dir), Ok(Some(_))),
            problem: None,
        },
        Ok(Err(e)) => missing(e.to_string()),
        Err(e) => missing(e.to_string()),
    };

    // 2. The boundary — asked exactly as the client asks it, at the moment of looking.
    let b = boundary::load(dir).unwrap_or_else(|_| boundary::Boundary::closed());
    let origin = familiar_mcp::Url::parse(&declared.url)
        .map(|u| u.origin())
        .unwrap_or_else(|_| declared.url.clone());
    let verdict = guard::evaluate(&Action::new(ActionKind::Network, origin), &b);
    let gate = Gate {
        allow_network: b.allow_network,
        allowed: verdict.decision == Decision::Allow,
        rationale: verdict.rationale,
    };

    // 3. The wire. A shut gate is not a failure to report as an outage — it is the boundary
    //    working, so the monitor says so in the gate's own words and makes no call.
    if !gate.allowed {
        *session = None;
    } else if session.is_none() {
        // Re-open lazily; the handshake cost lands on the round that paid it.
    }

    let mut handshake_ms = 0u128;
    let mut handshake_err: Option<String> = None;
    if gate.allowed && session.is_none() {
        let t = Instant::now();
        match Session::open(dir, server) {
            Ok(s) => {
                *session = Some(s);
                handshake_ms = t.elapsed().as_millis();
            }
            Err(e) => handshake_err = Some(e.to_string()),
        }
    }

    let handshake = match (session.as_ref(), handshake_err) {
        (Some(s), _) => Ok(Handshake {
            server_name: s.server_name.clone(),
            server_version: s.server_version.clone(),
            protocol_version: s.protocol_version.clone(),
            authenticated: declared.has_token,
            millis: handshake_ms,
        }),
        (None, Some(e)) => Err(e),
        (None, None) => Err(gate.rationale.clone()),
    };

    // 4. What is on offer, and 5. the world itself.
    let offered = {
        let t: Timed<Vec<familiar_mcp::Tool>> = match session.as_mut() {
            Some(s) => timed(|| s.tools().map_err(|e| e.to_string())),
            None => Timed::err("no session"),
        };
        Timed {
            millis: t.millis,
            value: t.value.map(|v| v.into_iter().map(|x| x.name).collect()),
        }
    };

    let status = call::<Status>(session, "ucf_status", json!({}));
    let stations = call::<Vec<Station>>(session, "ucf_stations", json!({}));
    let prices = call::<Vec<Price>>(session, "ucf_prices", json!({}));
    let news = call::<Vec<News>>(session, "ucf_news", json!({}));
    let carriers = call::<Vec<Carrier>>(session, "ucf_carriers", json!({}));
    let loads = call::<Vec<Load>>(session, "ucf_loadboard", json!({}));

    // A dead session shows itself as every call failing; drop it so the next round re-opens
    // and the reconnect is visible as a fresh handshake time.
    if status.value.is_err() && offered.value.is_err() {
        *session = None;
    }

    Round {
        footprint: footprint(dir, &declared.name),
        declared,
        gate,
        handshake,
        offered,
        status,
        stations,
        prices,
        news,
        carriers,
        loads,
        at: now,
    }
}

/// Round-builders the render tests share. Test-only: a monitor has no production reason to
/// construct a round it did not observe.
#[cfg(test)]
pub mod tests_support {
    use super::*;

    /// A round carrying nothing but a tick and a price table — enough to exercise movement.
    pub fn round_with_prices(prices: &[(&str, f64)], tick: i64) -> Round {
        let ps: Vec<Price> = prices
            .iter()
            .map(|(k, mid)| {
                let (good, station) = k.split_once('@').unwrap_or((k, ""));
                Price {
                    good: good.into(),
                    station: station.into(),
                    mid: *mid,
                    stock: 1,
                }
            })
            .collect();
        let mut r = super::tests::round_with(&[], None);
        r.prices = Timed {
            value: Ok(ps),
            millis: 1,
        };
        r.status = Timed {
            value: Ok(Status {
                tick,
                ..Default::default()
            }),
            millis: 1,
        };
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn round_with(declared: &[&str], offered: Option<Vec<String>>) -> Round {
        Round {
            declared: Declared {
                name: "ucf".into(),
                url: "https://example.test/mcp".into(),
                tools: declared.iter().map(|s| s.to_string()).collect(),
                note: String::new(),
                has_token: true,
                problem: None,
            },
            gate: Gate {
                allow_network: true,
                allowed: true,
                rationale: String::new(),
            },
            handshake: Err("unused".into()),
            offered: match offered {
                Some(v) => Timed {
                    value: Ok(v),
                    millis: 1,
                },
                None => Timed::err("no session"),
            },
            status: Timed::err("no session"),
            stations: Timed::err("no session"),
            prices: Timed::err("no session"),
            news: Timed::err("no session"),
            carriers: Timed::err("no session"),
            loads: Timed::err("no session"),
            footprint: Footprint {
                observations_total: 0,
                observations_naming_seam: 0,
                last_seam_observation: None,
            },
            at: 0,
        }
    }

    /// The event a human must decide about: the partner grew a tool nobody allowed.
    #[test]
    fn a_new_tool_on_offer_is_named_as_undeclared() {
        let r = round_with(
            &["ucf_status"],
            Some(vec!["ucf_status".into(), "ucf_place_order".into()]),
        );
        assert_eq!(r.undeclared_on_offer(), vec!["ucf_place_order".to_string()]);
        assert!(r.declared_but_gone().is_empty());
    }

    /// The other drift direction: an allowlist pointing at something that is gone.
    #[test]
    fn a_declaration_pointing_at_nothing_is_named_too() {
        let r = round_with(
            &["ucf_status", "ucf_retired"],
            Some(vec!["ucf_status".into()]),
        );
        assert_eq!(r.declared_but_gone(), vec!["ucf_retired".to_string()]);
        assert!(r.undeclared_on_offer().is_empty());
    }

    /// Without a session there is no evidence either way, and the monitor must not
    /// manufacture a drift report out of a network failure.
    #[test]
    fn no_offer_list_means_no_drift_claim() {
        let r = round_with(&["ucf_status"], None);
        assert!(r.undeclared_on_offer().is_empty());
        assert!(r.declared_but_gone().is_empty());
    }
}
