//! Grooming the screen, and remembering just enough between prowls to notice movement.
//!
//! A cat that only reports *levels* makes a busy world look like a sunbeam. What a human
//! watching a flap wants is the twitch — the tick advanced, this bowl changed price, that yowl
//! is new — so [`Whiskers`] keeps the previous prowl's shape and [`Twitches`] is what gets
//! drawn in the margin.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use crate::catch::{Haul, Kibble, Yowl};
use crate::sniff::{Pounce, Prowl};

const WIDTH: usize = 98;

/// Fur: ANSI colour, or a plain coat when the human asked for one.
pub struct Fur {
    pub colour: bool,
}

impl Fur {
    fn p(&self, code: &str, s: &str) -> String {
        if self.colour {
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
    pub fn happy(&self, s: &str) -> String {
        self.p("32", s)
    }
    pub fn ears_back(&self, s: &str) -> String {
        self.p("33", s)
    }
    pub fn hiss(&self, s: &str) -> String {
        self.p("31", s)
    }
    pub fn cool(&self, s: &str) -> String {
        self.p("36", s)
    }
}

/// What the last prowl looked like, kept so this one can notice what moved.
#[derive(Default)]
pub struct Whiskers {
    pub last_tick: Option<i64>,
    pub last_tick_at: i64,
    pub bowls: HashMap<String, f64>,
    pub yowls: HashSet<String>,
    /// True until the first real prowl has been remembered — the first screen must not report
    /// "3 new yowls" for the simple fact of having just woken up.
    pub asleep: bool,
}

impl Whiskers {
    pub fn new() -> Self {
        Self {
            asleep: true,
            ..Default::default()
        }
    }
}

/// What moved since the last prowl.
pub struct Twitches {
    pub tick_delta: i64,
    pub since_secs: i64,
    pub bowl_moves: Vec<(String, f64, f64)>,
    pub new_yowls: Vec<String>,
}

pub fn twitches(w: &Whiskers, p: &Prowl) -> Twitches {
    let tick_delta = match (w.last_tick, p.purr.got()) {
        (Some(prev), Some(s)) => s.tick - prev,
        _ => 0,
    };
    let mut bowl_moves = Vec::new();
    if let Some(ks) = p.bowls.got() {
        if !w.asleep {
            for k in ks {
                if let Some(was) = w.bowls.get(&k.bowl()) {
                    if (*was - k.mid).abs() > f64::EPSILON {
                        bowl_moves.push((k.bowl(), *was, k.mid));
                    }
                }
            }
        }
    }
    // Biggest jump first — a margin with room for three lines should spend them on the three
    // that matter, not the three that happened to sort first.
    bowl_moves.sort_by(|a, b| {
        (b.2 - b.1)
            .abs()
            .partial_cmp(&(a.2 - a.1).abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let new_yowls = match (p.yowls.got(), w.asleep) {
        (Some(ys), false) => ys
            .iter()
            .filter(|y| !w.yowls.contains(&y.headline))
            .map(|y| y.headline.clone())
            .collect(),
        _ => Vec::new(),
    };
    Twitches {
        tick_delta,
        since_secs: p.at - w.last_tick_at,
        bowl_moves,
        new_yowls,
    }
}

pub fn remember(w: &mut Whiskers, p: &Prowl) {
    if let Some(s) = p.purr.got() {
        if w.last_tick != Some(s.tick) {
            w.last_tick = Some(s.tick);
            w.last_tick_at = p.at;
        }
    }
    if let Some(ks) = p.bowls.got() {
        w.bowls = ks.iter().map(|k| (k.bowl(), k.mid)).collect();
    }
    if let Some(ys) = p.yowls.got() {
        w.yowls = ys.iter().map(|y| y.headline.clone()).collect();
    }
    // Awake only once a prowl has actually brought something back to compare against.
    if p.purr.got().is_some() || p.bowls.got().is_some() {
        w.asleep = false;
    }
}

/// A claw trim: shorten without severing a word, and mark the cut.
fn trim(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// `4m 12s`, `2h 05m`, `18s` — a stretch, in units a human reads without converting.
pub fn stretch(secs: i64) -> String {
    let s = secs.max(0);
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m {:02}s", s / 60, s % 60)
    } else {
        format!("{}h {:02}m", s / 3600, (s % 3600) / 60)
    }
}

/// `23:31:07Z` from unix seconds, without dragging in a date crate.
pub fn clock(unix: i64) -> String {
    let d = unix.rem_euclid(86_400);
    format!("{:02}:{:02}:{:02}Z", d / 3600, (d % 3600) / 60, d % 60)
}

fn row(out: &mut String, fur: &Fur, label: &str, body: &str) {
    let _ = writeln!(out, "  {:<13} {}", fur.dim(label), body);
}

fn cont(out: &mut String, body: &str) {
    let _ = writeln!(out, "  {:<13} {}", "", body);
}

/// One pounce's flight time for the right-hand margin.
fn ms<T>(fur: &Fur, p: &Pounce<T>) -> String {
    match p.why_not() {
        None => fur.dim(&format!("{}ms", p.millis)),
        Some(_) => fur.hiss("—"),
    }
}

/// The whole screen for one prowl.
pub fn draw(p: &Prowl, t: &Twitches, fur: &Fur, prowls: u64, interval: u64) -> String {
    let mut o = String::new();

    let _ = writeln!(
        o,
        "  {}  {}{}",
        fur.bold("UCF ⟷ FAMILIAR"),
        fur.dim("the cat flap, live"),
        fur.dim(&format!(
            "{:>width$}",
            format!("{} · prowl {} · every {}s", clock(p.at), prowls, interval),
            width = WIDTH.saturating_sub(38)
        ))
    );
    let _ = writeln!(o, "{}", fur.dim(&"─".repeat(WIDTH)));
    row(&mut o, fur, "SNIFFING", &fur.dim(&trim(&p.tray, 78)));

    // ---- the three things that decide whether the cat gets out at all -------------
    match &p.collar.problem {
        Some(why) => row(&mut o, fur, "COLLAR", &fur.hiss(why)),
        None => {
            row(
                &mut o,
                fur,
                "COLLAR",
                &format!(
                    "{} {} {}",
                    fur.cool(&p.collar.name),
                    fur.dim("→"),
                    trim(&p.collar.url, 60)
                ),
            );
            cont(
                &mut o,
                &fur.dim(&format!(
                    "{} tools allowed · name tag {} · mcp/servers.json",
                    p.collar.tools.len(),
                    if p.collar.has_tag { "on" } else { "MISSING" }
                )),
            );
            // The human's own words about this counterparty, carried verbatim: the note that
            // says the surface is read-only, and what happens if that ever changes.
            if !p.collar.note.is_empty() {
                cont(&mut o, &fur.dim(&format!("“{}”", trim(&p.collar.note, 72))));
            }
        }
    }

    // The flag and the verdict are two different facts, and collapsing them hides the
    // interesting case: an open `allow_network` whose scoped boundary still refuses THIS
    // origin. Show the flap, then show whether this particular cat gets through it.
    let flag = if p.flap.allow_network {
        fur.happy("unlatched")
    } else {
        fur.hiss("latched")
    };
    let verdict = if p.flap.open {
        fur.happy("THROUGH")
    } else {
        fur.hiss("BONK")
    };
    row(
        &mut o,
        fur,
        "CAT FLAP",
        &format!("allow_network {flag} · this reach {verdict}"),
    );
    if !p.flap.rationale.is_empty() {
        cont(&mut o, &fur.dim(&trim(&p.flap.rationale, 78)));
    }

    match &p.boop {
        Ok(b) => {
            row(
                &mut o,
                fur,
                "NOSE BOOP",
                &format!(
                    "{} {} · MCP {} · tag {}{}",
                    fur.cool(&b.server_name),
                    b.server_version,
                    b.protocol_version,
                    if b.wearing_tag { "✓" } else { "✗" },
                    if b.millis > 0 {
                        fur.dim(&format!(" · boop {}ms", b.millis))
                    } else {
                        String::new()
                    }
                ),
            );
            let (extra, gone) = (p.uncollared(), p.collared_but_gone());
            let offered = p.on_offer.got().map(Vec::len).unwrap_or(0);
            let drift = if extra.is_empty() && gone.is_empty() {
                fur.dim("nothing new under the sunbeam")
            } else {
                fur.ears_back("SOMETHING MOVED")
            };
            cont(
                &mut o,
                &fur.dim(&format!(
                    "offered {} · collared {} · {}",
                    offered,
                    p.collar.tools.len(),
                    drift
                )),
            );
            // Discovery is not permission: a tool that appeared is a decision for a human.
            if !extra.is_empty() {
                cont(
                    &mut o,
                    &fur.ears_back(&format!(
                        "⚠ on offer but NOT on the collar — untouchable until a human says so: {}",
                        extra.join(", ")
                    )),
                );
            }
            if !gone.is_empty() {
                cont(
                    &mut o,
                    &fur.ears_back(&format!(
                        "⚠ on the collar but no longer offered: {}",
                        gone.join(", ")
                    )),
                );
            }
        }
        Err(e) => row(&mut o, fur, "NOSE BOOP", &fur.hiss(&trim(e, 78))),
    }

    let _ = writeln!(o);

    // ---- the world on the other side of the flap ---------------------------------
    match p.purr.got() {
        Some(s) => {
            let moved = if t.tick_delta > 0 {
                fur.happy(&format!(" +{} in {}", t.tick_delta, stretch(t.since_secs)))
            } else {
                String::new()
            };
            let next = if s.tick_duration_sec > 0 && s.epoch_unix_seconds > 0 {
                let elapsed = p.at - s.epoch_unix_seconds;
                let left = s.tick_duration_sec - elapsed.rem_euclid(s.tick_duration_sec);
                fur.dim(&format!(" · next purr in ~{}", stretch(left)))
            } else {
                String::new()
            };
            row(
                &mut o,
                fur,
                "THE WORLD",
                &format!(
                    "{} · tick {}{}{}  {}",
                    fur.cool(&s.world_name),
                    fur.bold(&s.tick.to_string()),
                    moved,
                    next,
                    ms(fur, &p.purr)
                ),
            );
            cont(
                &mut o,
                &fur.dim(&format!(
                    "content v{} · seed {} · state {}",
                    s.content_version,
                    s.world_seed,
                    s.state_hash.chars().take(8).collect::<String>()
                )),
            );
        }
        None => row(
            &mut o,
            fur,
            "THE WORLD",
            &fur.hiss(&trim(p.purr.why_not().unwrap_or("unavailable"), 78)),
        ),
    }

    match p.perches.got() {
        Some(ps) => {
            let mut by_class: HashMap<&str, usize> = HashMap::new();
            for s in ps {
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
                fur,
                "PERCHES",
                &format!(
                    "{} — {}  {}",
                    ps.len(),
                    fur.dim(&shape),
                    ms(fur, &p.perches)
                ),
            );
        }
        None => row(
            &mut o,
            fur,
            "PERCHES",
            &fur.hiss(&trim(p.perches.why_not().unwrap_or("unavailable"), 78)),
        ),
    }

    bowls(&mut o, fur, p, t);
    yowls(&mut o, fur, p, t);

    match p.tomcats.got() {
        Some(cs) => {
            let working = cs.iter().filter(|x| x.in_service).count();
            let out = cs.iter().filter(|x| x.on_the_prowl()).count();
            row(
                &mut o,
                fur,
                "TOMCATS",
                &format!(
                    "{} in service · {} on the prowl · {} curled up  {}",
                    working,
                    out,
                    working.saturating_sub(out),
                    ms(fur, &p.tomcats)
                ),
            );
        }
        None => row(
            &mut o,
            fur,
            "TOMCATS",
            &fur.hiss(&trim(p.tomcats.why_not().unwrap_or("unavailable"), 78)),
        ),
    }

    hauls(&mut o, fur, p);
    let _ = writeln!(o);
    paw_prints(&mut o, fur, p);

    o
}

fn bowls(o: &mut String, fur: &Fur, p: &Prowl, t: &Twitches) {
    let Some(ks) = p.bowls.got() else {
        row(
            o,
            fur,
            "BOWLS",
            &fur.hiss(&trim(p.bowls.why_not().unwrap_or("unavailable"), 78)),
        );
        return;
    };
    let goods: HashSet<&str> = ks.iter().map(|k| k.good.as_str()).collect();
    let empty: Vec<&Kibble> = ks.iter().filter(|k| k.empty()).collect();
    row(
        o,
        fur,
        "BOWLS",
        &format!(
            "{} bowls · {} goods · {}  {}",
            ks.len(),
            goods.len(),
            if empty.is_empty() {
                fur.dim("none empty")
            } else {
                // The single most upsetting fact available to a cat.
                fur.ears_back(&format!("{} EMPTY", empty.len()))
            },
            ms(fur, &p.bowls)
        ),
    );
    for (bowl, was, now) in t.bowl_moves.iter().take(3) {
        let mark = if now > was {
            fur.happy("▲")
        } else {
            fur.hiss("▼")
        };
        cont(
            o,
            &format!(
                "{} {:<28} {} → {}",
                mark,
                trim(bowl, 28),
                fur.dim(&format!("{was:.0}")),
                fur.bold(&format!("{now:.0}"))
            ),
        );
    }
    if t.bowl_moves.len() > 3 {
        cont(
            o,
            &fur.dim(&format!(
                "… and {} more bowls moved",
                t.bowl_moves.len() - 3
            )),
        );
    }
}

fn yowls(o: &mut String, fur: &Fur, p: &Prowl, t: &Twitches) {
    let Some(ys) = p.yowls.got() else {
        row(
            o,
            fur,
            "YOWLS",
            &fur.hiss(&trim(p.yowls.why_not().unwrap_or("unavailable"), 78)),
        );
        return;
    };
    let live: Vec<&Yowl> = ys.iter().filter(|y| y.live()).collect();
    row(
        o,
        fur,
        "YOWLS",
        &format!(
            "{} still yowling · {} hoarse  {}",
            live.len(),
            ys.len() - live.len(),
            ms(fur, &p.yowls)
        ),
    );
    let tick = p.purr.got().map(|s| s.tick).unwrap_or(0);
    for y in live.iter().take(3) {
        let fresh = t.new_yowls.iter().any(|h| h == &y.headline);
        let flag = if fresh {
            fur.happy("NEW")
        } else {
            fur.dim("⚑")
        };
        cont(o, &format!("{} {}", flag, trim(&y.headline, 74)));
        let left = y.expires_at_tick - tick;
        cont(
            o,
            &fur.dim(&format!(
                "    {} · {} · hoarse at tick {}{}",
                y.tier,
                y.status,
                y.expires_at_tick,
                if tick > 0 && left > 0 {
                    format!(" ({left} ticks)")
                } else {
                    String::new()
                }
            )),
        );
    }
}

fn hauls(o: &mut String, fur: &Fur, p: &Prowl) {
    let Some(hs) = p.hauls.got() else {
        row(
            o,
            fur,
            "HAULS",
            &fur.hiss(&trim(p.hauls.why_not().unwrap_or("unavailable"), 78)),
        );
        return;
    };
    let mut by_status: HashMap<&str, usize> = HashMap::new();
    for h in hs {
        *by_status.entry(h.status.as_str()).or_default() += 1;
    }
    let mut kinds: Vec<_> = by_status.into_iter().collect();
    kinds.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    // The board carries six or more statuses; the line has room for the biggest few and a
    // count of the rest. Wrapping this row would shove the panel that matters off the mat.
    let mut shape = kinds
        .iter()
        .take(4)
        .map(|(k, v)| format!("{v} {k}"))
        .collect::<Vec<_>>()
        .join(" · ");
    if kinds.len() > 4 {
        let _ = write!(shape, " +{}", kinds.len() - 4);
    }
    let ours: Vec<&Haul> = p.our_hauls();
    let mine = if ours.is_empty() {
        fur.dim("0 ours")
    } else {
        fur.happy(&format!("{} ours", ours.len()))
    };
    row(
        o,
        fur,
        "HAULS",
        &format!(
            "{} on the board · {}  {}",
            hs.len(),
            mine,
            ms(fur, &p.hauls)
        ),
    );
    cont(o, &fur.dim(&shape));
    for h in ours.iter().take(3) {
        cont(
            o,
            &format!(
                "◆ {} {} {}→{} · {} units · net {:.0} · {}",
                fur.bold(&h.load_id),
                h.good,
                h.origin,
                h.dest,
                h.units,
                h.estimated_net,
                h.status
            ),
        );
    }
}

/// The panel that answers *is the familiar actually hunting here* — from local evidence only.
/// Every number is counted, never asserted, so the day the metabolism starts going through the
/// flap this panel changes without anyone editing it.
fn paw_prints(o: &mut String, fur: &Fur, p: &Prowl) {
    let pp = &p.prints;
    let ours = p.our_hauls().len();
    let hunting = pp.observations_naming_flap > 0 || ours > 0;

    row(
        o,
        fur,
        "PAW PRINTS",
        &if hunting {
            fur.happy("the familiar has been through this flap")
        } else {
            fur.ears_back("no paw prints — the familiar itself has never gone through")
        },
    );
    cont(
        o,
        &fur.dim(&format!(
            "{} of {} observations name it · {} hauls on the board are ours",
            pp.observations_naming_flap, pp.observations_total, ours
        )),
    );
    if let Some(last) = &pp.last_print {
        cont(o, &fur.dim(&format!("last: {}", trim(last, 74))));
    }
    if !hunting {
        cont(
            o,
            &fur.dim("every pounce on this screen was this cat's, not the metabolism's"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sniff::pretend::prowl_with_bowls;

    #[test]
    fn a_stretch_reads_the_way_a_human_says_it() {
        assert_eq!(stretch(18), "18s");
        assert_eq!(stretch(252), "4m 12s");
        assert_eq!(stretch(7500), "2h 05m");
        assert_eq!(stretch(-5), "0s");
    }

    #[test]
    fn the_clock_wraps_a_day_without_a_date_crate() {
        assert_eq!(clock(0), "00:00:00Z");
        assert_eq!(clock(86_399), "23:59:59Z");
        assert_eq!(clock(1_785_258_645), clock(1_785_258_645 + 86_400));
    }

    #[test]
    fn a_claw_trim_never_exceeds_the_budget() {
        assert_eq!(trim("short", 10), "short");
        assert_eq!(trim("abcdefghij", 5).chars().count(), 5);
        assert!(trim("abcdefghij", 5).ends_with('…'));
    }

    /// A cat that has just woken up has not seen anything move. Reporting "3 new yowls" for
    /// the act of opening one eye is a lie by presentation.
    #[test]
    fn a_sleeping_cat_reports_no_movement() {
        let w = Whiskers::new();
        let p = prowl_with_bowls(&[("catnip@a", 21.0)], 100);
        let t = twitches(&w, &p);
        assert!(t.bowl_moves.is_empty());
        assert!(t.new_yowls.is_empty());
        assert_eq!(t.tick_delta, 0);
    }

    /// Once awake, a changed bowl is movement and an unchanged one is furniture.
    #[test]
    fn an_awake_cat_reports_only_what_actually_moved() {
        let mut w = Whiskers::new();
        remember(
            &mut w,
            &prowl_with_bowls(&[("catnip@a", 21.0), ("grain@a", 9.0)], 100),
        );
        let t = twitches(
            &w,
            &prowl_with_bowls(&[("catnip@a", 19.0), ("grain@a", 9.0)], 101),
        );
        assert_eq!(t.bowl_moves.len(), 1);
        assert_eq!(t.bowl_moves[0].0, "catnip@a");
        assert_eq!(t.tick_delta, 1);
    }

    /// The margin has room for three. It must spend them on the biggest jumps.
    #[test]
    fn the_biggest_jumps_are_the_ones_shown() {
        let mut w = Whiskers::new();
        remember(
            &mut w,
            &prowl_with_bowls(&[("a@s", 10.0), ("b@s", 10.0), ("c@s", 10.0)], 1),
        );
        let t = twitches(
            &w,
            &prowl_with_bowls(&[("a@s", 11.0), ("b@s", 40.0), ("c@s", 12.0)], 1),
        );
        assert_eq!(t.bowl_moves[0].0, "b@s");
    }
}
