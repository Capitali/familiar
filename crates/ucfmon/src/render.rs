//! Drawing the screen, and remembering just enough between rounds to show movement.
//!
//! A monitor that only shows *levels* makes a busy world look static. What a human watching a
//! seam wants is the delta — the tick advanced, this price moved, that headline is new — so
//! [`Memory`] keeps the previous round's shape and [`Changes`] is what is drawn in the margin.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use crate::probe::{Round, Timed};
use crate::world::{Load, News, Price};

const WIDTH: usize = 98;

/// ANSI, or nothing at all when the human asked for plain output.
pub struct Ink {
    pub color: bool,
}

impl Ink {
    fn p(&self, code: &str, s: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
    pub fn dim(&self, s: &str) -> String {
        self.p("2", s)
    }
    pub fn bold(&self, s: &str) -> String {
        self.p("1", s)
    }
    pub fn good(&self, s: &str) -> String {
        self.p("32", s)
    }
    pub fn warn(&self, s: &str) -> String {
        self.p("33", s)
    }
    pub fn bad(&self, s: &str) -> String {
        self.p("31", s)
    }
    pub fn cool(&self, s: &str) -> String {
        self.p("36", s)
    }
}

/// What the previous round looked like, kept so this one can show movement.
#[derive(Default)]
pub struct Memory {
    pub last_tick: Option<i64>,
    pub last_tick_at: i64,
    pub prices: HashMap<String, f64>,
    pub headlines: HashSet<String>,
    /// True until the first successful round has been absorbed — the first screen must not
    /// report "3 new headlines" for the simple fact of having just started looking.
    pub cold: bool,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            cold: true,
            ..Default::default()
        }
    }
}

/// Movement since the previous round.
pub struct Changes {
    pub tick_delta: i64,
    pub since_secs: i64,
    pub price_moves: Vec<(String, f64, f64)>,
    pub new_headlines: Vec<String>,
}

pub fn diff(mem: &Memory, r: &Round) -> Changes {
    let tick_delta = match (mem.last_tick, r.status.ok()) {
        (Some(prev), Some(s)) => s.tick - prev,
        _ => 0,
    };
    let mut price_moves = Vec::new();
    if let Some(ps) = r.prices.ok() {
        if !mem.cold {
            for p in ps {
                if let Some(was) = mem.prices.get(&p.key()) {
                    if (*was - p.mid).abs() > f64::EPSILON {
                        price_moves.push((p.key(), *was, p.mid));
                    }
                }
            }
        }
    }
    // Biggest absolute move first — a monitor with room for three lines should spend them on
    // the three that matter, not the three that sorted first alphabetically.
    price_moves.sort_by(|a, b| {
        (b.2 - b.1)
            .abs()
            .partial_cmp(&(a.2 - a.1).abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let new_headlines = match (r.news.ok(), mem.cold) {
        (Some(ns), false) => ns
            .iter()
            .filter(|n| !mem.headlines.contains(&n.headline))
            .map(|n| n.headline.clone())
            .collect(),
        _ => Vec::new(),
    };
    Changes {
        tick_delta,
        since_secs: r.at - mem.last_tick_at,
        price_moves,
        new_headlines,
    }
}

pub fn absorb(mem: &mut Memory, r: &Round) {
    if let Some(s) = r.status.ok() {
        if mem.last_tick != Some(s.tick) {
            mem.last_tick = Some(s.tick);
            mem.last_tick_at = r.at;
        }
    }
    if let Some(ps) = r.prices.ok() {
        mem.prices = ps.iter().map(|p| (p.key(), p.mid)).collect();
    }
    if let Some(ns) = r.news.ok() {
        mem.headlines = ns.iter().map(|n| n.headline.clone()).collect();
    }
    // Cold only until a round actually landed something worth comparing against.
    if r.status.ok().is_some() || r.prices.ok().is_some() {
        mem.cold = false;
    }
}

fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// `4m 12s`, `2h 05m`, `18s` — durations a human reads without converting.
pub fn dur(secs: i64) -> String {
    let s = secs.max(0);
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m {:02}s", s / 60, s % 60)
    } else {
        format!("{}h {:02}m", s / 3600, (s % 3600) / 60)
    }
}

/// `23:31:07Z` from unix seconds, without a date dependency.
pub fn clock(unix: i64) -> String {
    let d = unix.rem_euclid(86_400);
    format!("{:02}:{:02}:{:02}Z", d / 3600, (d % 3600) / 60, d % 60)
}

fn row(out: &mut String, ink: &Ink, label: &str, body: &str) {
    let _ = writeln!(out, "  {:<13} {}", ink.dim(label), body);
}

fn cont(out: &mut String, body: &str) {
    let _ = writeln!(out, "  {:<13} {}", "", body);
}

fn rule(out: &mut String, ink: &Ink) {
    let _ = writeln!(out, "{}", ink.dim(&"─".repeat(WIDTH)));
}

/// Render one timing as ` 142ms` / ` refused` for the right-hand margin.
fn ms<T>(ink: &Ink, t: &Timed<T>) -> String {
    match t.why() {
        None => ink.dim(&format!("{}ms", t.millis)),
        Some(_) => ink.bad("—"),
    }
}

/// The whole screen for one round.
pub fn screen(r: &Round, c: &Changes, ink: &Ink, rounds: u64, interval: u64) -> String {
    let mut o = String::new();

    let _ = writeln!(
        o,
        "  {}  {}{}",
        ink.bold("UCF ⟷ FAMILIAR"),
        ink.dim("the seam, live"),
        ink.dim(&format!(
            "{:>width$}",
            format!("{} · round {} · every {}s", clock(r.at), rounds, interval),
            width = WIDTH.saturating_sub(34)
        ))
    );
    rule(&mut o, ink);
    row(&mut o, ink, "WATCHING", &ink.dim(&clip(&r.dir, 78)));

    // ---- the three interfaces that decide whether anything happens at all ----------
    match &r.declared.problem {
        Some(p) => row(&mut o, ink, "DECLARATION", &ink.bad(p)),
        None => {
            row(
                &mut o,
                ink,
                "DECLARATION",
                &format!(
                    "{} {} {}",
                    ink.cool(&r.declared.name),
                    ink.dim("→"),
                    clip(&r.declared.url, 60)
                ),
            );
            cont(
                &mut o,
                &ink.dim(&format!(
                    "{} tools allowed · bearer key {} · mcp/servers.json",
                    r.declared.tools.len(),
                    if r.declared.has_token {
                        "present"
                    } else {
                        "MISSING"
                    }
                )),
            );
            // The human's own words about this counterparty. Carried verbatim: this is the
            // note that says the surface is read-only and what happens if that ever changes.
            if !r.declared.note.is_empty() {
                cont(
                    &mut o,
                    &ink.dim(&format!("“{}”", clip(&r.declared.note, 72))),
                );
            }
        }
    }

    // The flag and the verdict are two different facts, and a monitor that collapses them
    // hides the interesting case: an open `allow_network` whose scoped boundary still refuses
    // *this* origin. Show the flag, then show what the guard actually decided about the reach.
    let flag = if r.gate.allow_network {
        ink.good("open")
    } else {
        ink.bad("shut")
    };
    let verdict = if r.gate.allowed {
        ink.good("ALLOW")
    } else {
        ink.bad("REFUSE")
    };
    row(
        &mut o,
        ink,
        "BOUNDARY",
        &format!("allow_network {flag} · this reach {verdict}"),
    );
    if !r.gate.rationale.is_empty() {
        cont(&mut o, &ink.dim(&clip(&r.gate.rationale, 78)));
    }

    match &r.handshake {
        Ok(h) => {
            row(
                &mut o,
                ink,
                "WIRE",
                &format!(
                    "{} {} · MCP {} · bearer {}{}",
                    ink.cool(&h.server_name),
                    h.server_version,
                    h.protocol_version,
                    if h.authenticated { "✓" } else { "✗" },
                    if h.millis > 0 {
                        ink.dim(&format!(" · handshake {}ms", h.millis))
                    } else {
                        String::new()
                    }
                ),
            );
            let (undeclared, gone) = (r.undeclared_on_offer(), r.declared_but_gone());
            let offered = r.offered.ok().map(Vec::len).unwrap_or(0);
            let drift = if undeclared.is_empty() && gone.is_empty() {
                ink.dim("no drift")
            } else {
                ink.warn("DRIFT")
            };
            cont(
                &mut o,
                &ink.dim(&format!(
                    "offered {} · declared {} · {}",
                    offered,
                    r.declared.tools.len(),
                    drift
                )),
            );
            // Discovery is not permission: a tool that appeared is a decision for a human.
            if !undeclared.is_empty() {
                cont(
                    &mut o,
                    &ink.warn(&format!(
                        "⚠ on offer but NOT declared — uncallable until a human writes it in: {}",
                        undeclared.join(", ")
                    )),
                );
            }
            if !gone.is_empty() {
                cont(
                    &mut o,
                    &ink.warn(&format!(
                        "⚠ declared but no longer offered: {}",
                        gone.join(", ")
                    )),
                );
            }
        }
        Err(e) => row(&mut o, ink, "WIRE", &ink.bad(&clip(e, 78))),
    }

    let _ = writeln!(o);

    // ---- the world on the other side ---------------------------------------------
    match r.status.ok() {
        Some(s) => {
            let moved = if c.tick_delta > 0 {
                ink.good(&format!(" +{} in {}", c.tick_delta, dur(c.since_secs)))
            } else {
                String::new()
            };
            let next = if s.tick_duration_sec > 0 && s.epoch_unix_seconds > 0 {
                let elapsed = r.at - s.epoch_unix_seconds;
                let left = s.tick_duration_sec - elapsed.rem_euclid(s.tick_duration_sec);
                ink.dim(&format!(" · next tick in ~{}", dur(left)))
            } else {
                String::new()
            };
            row(
                &mut o,
                ink,
                "WORLD",
                &format!(
                    "{} · tick {}{}{}  {}",
                    ink.cool(&s.world_name),
                    ink.bold(&s.tick.to_string()),
                    moved,
                    next,
                    ms(ink, &r.status)
                ),
            );
            cont(
                &mut o,
                &ink.dim(&format!(
                    "content v{} · seed {} · state {}",
                    s.content_version,
                    s.world_seed,
                    s.state_hash.chars().take(8).collect::<String>()
                )),
            );
        }
        None => row(
            &mut o,
            ink,
            "WORLD",
            &ink.bad(&clip(r.status.why().unwrap_or("unavailable"), 78)),
        ),
    }

    match r.stations.ok() {
        Some(st) => {
            let mut by_class: HashMap<&str, usize> = HashMap::new();
            for s in st {
                *by_class.entry(s.station_class.as_str()).or_default() += 1;
            }
            let mut classes: Vec<_> = by_class.into_iter().collect();
            classes.sort();
            let shape = classes
                .iter()
                .map(|(k, v)| format!("{v} {k}"))
                .collect::<Vec<_>>()
                .join(" · ");
            row(
                &mut o,
                ink,
                "STATIONS",
                &format!(
                    "{} — {}  {}",
                    st.len(),
                    ink.dim(&shape),
                    ms(ink, &r.stations)
                ),
            );
        }
        None => row(
            &mut o,
            ink,
            "STATIONS",
            &ink.bad(&clip(r.stations.why().unwrap_or("unavailable"), 78)),
        ),
    }

    market(&mut o, ink, r, c);
    news(&mut o, ink, r, c);

    match r.carriers.ok() {
        Some(cs) => {
            let live = cs.iter().filter(|x| x.in_service).count();
            let moving = cs.iter().filter(|x| x.under_way()).count();
            row(
                &mut o,
                ink,
                "CARRIERS",
                &format!(
                    "{} in service · {} under way · {} docked  {}",
                    live,
                    moving,
                    live.saturating_sub(moving),
                    ms(ink, &r.carriers)
                ),
            );
        }
        None => row(
            &mut o,
            ink,
            "CARRIERS",
            &ink.bad(&clip(r.carriers.why().unwrap_or("unavailable"), 78)),
        ),
    }

    loadboard(&mut o, ink, r);
    let _ = writeln!(o);
    footprint(&mut o, ink, r);

    o
}

fn market(o: &mut String, ink: &Ink, r: &Round, c: &Changes) {
    let Some(ps) = r.prices.ok() else {
        row(
            o,
            ink,
            "MARKET",
            &ink.bad(&clip(r.prices.why().unwrap_or("unavailable"), 78)),
        );
        return;
    };
    let goods: HashSet<&str> = ps.iter().map(|p| p.good.as_str()).collect();
    let out: Vec<&Price> = ps.iter().filter(|p| p.stock == 0).collect();
    row(
        o,
        ink,
        "MARKET",
        &format!(
            "{} rows · {} goods · {}  {}",
            ps.len(),
            goods.len(),
            if out.is_empty() {
                ink.dim("no stockouts")
            } else {
                ink.warn(&format!("{} stockouts", out.len()))
            },
            ms(ink, &r.prices)
        ),
    );
    for (key, was, now) in c.price_moves.iter().take(3) {
        let up = now > was;
        let mark = if up { ink.good("▲") } else { ink.bad("▼") };
        cont(
            o,
            &format!(
                "{} {:<28} {} → {}",
                mark,
                clip(key, 28),
                ink.dim(&format!("{was:.0}")),
                ink.bold(&format!("{now:.0}"))
            ),
        );
    }
    if c.price_moves.len() > 3 {
        cont(
            o,
            &ink.dim(&format!("… and {} more moves", c.price_moves.len() - 3)),
        );
    }
}

fn news(o: &mut String, ink: &Ink, r: &Round, c: &Changes) {
    let Some(ns) = r.news.ok() else {
        row(
            o,
            ink,
            "NEWS",
            &ink.bad(&clip(r.news.why().unwrap_or("unavailable"), 78)),
        );
        return;
    };
    let live: Vec<&News> = ns.iter().filter(|n| n.status != "expired").collect();
    row(
        o,
        ink,
        "NEWS",
        &format!(
            "{} live · {} expired  {}",
            live.len(),
            ns.len() - live.len(),
            ms(ink, &r.news)
        ),
    );
    let tick = r.status.ok().map(|s| s.tick).unwrap_or(0);
    for n in live.iter().take(3) {
        let fresh = c.new_headlines.iter().any(|h| h == &n.headline);
        let flag = if fresh {
            ink.good("NEW")
        } else {
            ink.dim("⚑")
        };
        cont(o, &format!("{} {}", flag, clip(&n.headline, 74)));
        let left = n.expires_at_tick - tick;
        cont(
            o,
            &ink.dim(&format!(
                "    {} · {} · expires tick {}{}",
                n.tier,
                n.status,
                n.expires_at_tick,
                if tick > 0 && left > 0 {
                    format!(" ({left} ticks)")
                } else {
                    String::new()
                }
            )),
        );
    }
}

fn loadboard(o: &mut String, ink: &Ink, r: &Round) {
    let Some(ls) = r.loads.ok() else {
        row(
            o,
            ink,
            "LOADBOARD",
            &ink.bad(&clip(r.loads.why().unwrap_or("unavailable"), 78)),
        );
        return;
    };
    let mut by_status: HashMap<&str, usize> = HashMap::new();
    for l in ls {
        *by_status.entry(l.status.as_str()).or_default() += 1;
    }
    let mut kinds: Vec<_> = by_status.into_iter().collect();
    kinds.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    // The board carries six or more statuses; the line has room for the biggest few and a
    // count of the rest. Wrapping this row would push the panel that matters off the screen.
    let mut shape = kinds
        .iter()
        .take(4)
        .map(|(k, v)| format!("{v} {k}"))
        .collect::<Vec<_>>()
        .join(" · ");
    if kinds.len() > 4 {
        let _ = write!(shape, " +{}", kinds.len() - 4);
    }
    let mine: Vec<&Load> = r.mine();
    let ours = if mine.is_empty() {
        ink.dim("0 the familiar's own")
    } else {
        ink.good(&format!("{} the familiar's own", mine.len()))
    };
    row(
        o,
        ink,
        "LOADBOARD",
        &format!("{} loads · {}  {}", ls.len(), ours, ms(ink, &r.loads)),
    );
    cont(o, &ink.dim(&shape));
    for l in mine.iter().take(3) {
        cont(
            o,
            &format!(
                "◆ {} {} {}→{} · {} units · net {:.0} · {}",
                ink.bold(&l.load_id),
                l.good,
                l.origin,
                l.dest,
                l.units,
                l.estimated_net,
                l.status
            ),
        );
    }
}

/// The panel that answers *how is the familiar interacting with UCF* — from local evidence
/// only. Every number here is counted, never asserted, so the day the metabolism starts
/// calling the exchange this panel changes without anyone editing it.
fn footprint(o: &mut String, ink: &Ink, r: &Round) {
    let f = &r.footprint;
    let mine = r.mine().len();
    let acting = f.observations_naming_seam > 0 || mine > 0;

    row(
        o,
        ink,
        "FAMILIAR→UCF",
        &if acting {
            ink.good("the familiar has a footprint on this seam")
        } else {
            ink.warn("no local record of the familiar itself calling this seam")
        },
    );
    cont(
        o,
        &ink.dim(&format!(
            "{} of {} observations name it · {} loads on the board are its own",
            f.observations_naming_seam, f.observations_total, mine
        )),
    );
    if let Some(last) = &f.last_seam_observation {
        cont(o, &ink.dim(&format!("last: {}", clip(last, 74))));
    }
    if !acting {
        cont(
            o,
            &ink.dim("every call on this screen was made by this monitor, not by the metabolism"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_read_the_way_a_human_says_them() {
        assert_eq!(dur(18), "18s");
        assert_eq!(dur(252), "4m 12s");
        assert_eq!(dur(7500), "2h 05m");
        assert_eq!(dur(-5), "0s");
    }

    #[test]
    fn the_clock_wraps_a_day_without_a_date_crate() {
        assert_eq!(clock(0), "00:00:00Z");
        assert_eq!(clock(86_399), "23:59:59Z");
        assert_eq!(clock(1_785_258_645), clock(1_785_258_645 + 86_400));
    }

    #[test]
    fn clipping_never_exceeds_the_budget() {
        assert_eq!(clip("short", 10), "short");
        assert_eq!(clip("abcdefghij", 5).chars().count(), 5);
        assert!(clip("abcdefghij", 5).ends_with('…'));
    }

    /// The first screen must not claim movement it never observed — a cold monitor reporting
    /// "3 new headlines" for the act of starting up is a lie by presentation.
    #[test]
    fn a_cold_memory_reports_no_movement() {
        let mem = Memory::new();
        let r = crate::probe::tests_support::round_with_prices(&[("catnip@a", 21.0)], 100);
        let c = diff(&mem, &r);
        assert!(c.price_moves.is_empty());
        assert!(c.new_headlines.is_empty());
        assert_eq!(c.tick_delta, 0);
    }

    /// Once warm, a changed price is movement and an unchanged one is not.
    #[test]
    fn a_warm_memory_reports_only_what_actually_moved() {
        let mut mem = Memory::new();
        let first = crate::probe::tests_support::round_with_prices(
            &[("catnip@a", 21.0), ("grain@a", 9.0)],
            100,
        );
        absorb(&mut mem, &first);
        let second = crate::probe::tests_support::round_with_prices(
            &[("catnip@a", 19.0), ("grain@a", 9.0)],
            101,
        );
        let c = diff(&mem, &second);
        assert_eq!(c.price_moves.len(), 1);
        assert_eq!(c.price_moves[0].0, "catnip@a");
        assert_eq!(c.tick_delta, 1);
    }

    /// The margin has room for three moves; it must spend them on the biggest.
    #[test]
    fn the_biggest_moves_are_the_ones_shown() {
        let mut mem = Memory::new();
        absorb(
            &mut mem,
            &crate::probe::tests_support::round_with_prices(
                &[("a@s", 10.0), ("b@s", 10.0), ("c@s", 10.0)],
                1,
            ),
        );
        let c = diff(
            &mem,
            &crate::probe::tests_support::round_with_prices(
                &[("a@s", 11.0), ("b@s", 40.0), ("c@s", 12.0)],
                1,
            ),
        );
        assert_eq!(c.price_moves[0].0, "b@s");
    }
}
