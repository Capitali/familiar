//! The captain's money over time, from the ship's own journal (T-241, Ian 2026-09-09:
//! "Familiar UCF views should include economic history/trend lines and analysis of
//! profit in summary form for overview by captain").
//!
//! Every fold and every act journals `credits`, so the ledger's movement is exact:
//! each delta between two readings is real money. What is heuristic is the CAUSE:
//! a delta is booked to the last act that could have moved money — a refuel, a
//! repair, a fill, a freight settle, a pay-down, an outfit — and to "other" when
//! nothing the pilot did explains it (a lease charge, a bonus arriving late). Trade
//! totals come from the fills themselves. Nothing here is a promise about the
//! future; the trend is a straight line through what happened.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// One reading of the purse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Point {
    pub at: i64,
    pub tick: i64,
    pub credits: i64,
}

/// Where the money went, over a window. Signed: what came in is positive.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Flows {
    pub trade_bought: i64,
    pub trade_sold: i64,
    pub freight: i64,
    pub fuel: i64,
    pub repair: i64,
    pub outfit: i64,
    pub debt_paid: i64,
    /// Berth fees on arrival, named by the exchange since #61 (before it: `other`).
    pub dock: i64,
    pub other: i64,
    pub fills: i64,
    pub settles: i64,
}

impl Flows {
    fn add(&mut self, o: &Flows) {
        self.trade_bought += o.trade_bought;
        self.trade_sold += o.trade_sold;
        self.freight += o.freight;
        self.fuel += o.fuel;
        self.repair += o.repair;
        self.outfit += o.outfit;
        self.debt_paid += o.debt_paid;
        self.dock += o.dock;
        self.other += o.other;
        self.fills += o.fills;
        self.settles += o.settles;
    }
    pub fn trade_net(&self) -> i64 {
        self.trade_sold + self.trade_bought
    }
}

/// The biggest single move in the window, and what caused it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Move {
    pub at: i64,
    pub tick: i64,
    pub delta: i64,
    pub cause: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Summary {
    pub window_days: f64,
    pub readings: usize,
    pub credits_start: i64,
    pub credits_now: i64,
    pub delta: i64,
    /// ℳ per day, the slope of a straight line through every reading in the window.
    pub trend_per_day: f64,
    pub best: Option<Move>,
    pub worst: Option<Move>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct History {
    pub points: Vec<Point>,
    pub flows: Flows,
    pub summary: Summary,
    /// Where the flows came from: `exchange` when the exchange's own cash ledger
    /// (`GET /v1/cash`, ucf-exchange#42) named every credit that moved inside the
    /// window; `journal` when they were attributed from the ship's journal (the
    /// heuristic above); `mixed` for a pool of both. The readings and the trend are
    /// the journal's either way.
    #[serde(default)]
    pub source: String,
}

/// The exchange's cash ledger for one hull (`GET /v1/cash`): every credit in and
/// out as a signed line with the engine's own `kind` — `trade`, `freight`, `fuel`,
/// `repair`, `refit`, `crew`, `galley`, `lease`, `paws`, `survey`, `equity`,
/// `insurance`, `dock`, `opening`, `unnamed` (and `other` on lines written before
/// the exchange's #61) — newest first, up to 400 lines. Where it
/// answers, it replaces the journal's guess about CAUSE with the fold's own word;
/// the sum of its lines is exactly the credits that moved.
///
/// The window is the journal's (unix seconds); the ledger speaks in ticks, so the
/// caller hands in the first tick inside the window, read off the journal.
pub(crate) fn flows_from_cash(cash: &Value, since_tick: i64) -> Option<Flows> {
    let lines = cash.get("lines")?.as_array()?;
    let mut flows = Flows::default();
    let mut any = false;
    for l in lines {
        let tick = l.get("tick").and_then(Value::as_i64).unwrap_or(0);
        if tick < since_tick {
            continue;
        }
        let amount = l.get("amount").and_then(Value::as_i64).unwrap_or(0);
        let kind = l.get("kind").and_then(Value::as_str).unwrap_or("other");
        match kind {
            // The ledger does not split a trade's side; the sign does.
            "trade" if amount >= 0 => flows.trade_sold += amount,
            "trade" => flows.trade_bought += amount,
            "freight" => flows.freight += amount,
            "fuel" | "paws" => flows.fuel += amount,
            "repair" => flows.repair += amount,
            "refit" | "crew" | "galley" => flows.outfit += amount,
            "lease" => flows.debt_paid += amount,
            // A berth's fee on arrival (exchange #61); before it these were `other`.
            "dock" => flows.dock += amount,
            // The balance carried forward is not a movement inside the window.
            "opening" => continue,
            _ => flows.other += amount,
        }
        any = true;
    }
    any.then_some(flows)
}

/// One hull's history with the exchange's cash ledger as the flows' source where
/// it answers, the journal where it does not. `fills` and `settles` stay the
/// journal's counts either way (the ledger counts credits, not acts).
pub(crate) fn for_ship_with_cash(ship_dir: &Path, since: i64, cash: Option<&Value>) -> History {
    let mut h = for_ship(ship_dir, since);
    let Some(cash) = cash else {
        return h;
    };
    let Some(first_tick) = h.points.first().map(|p| p.tick) else {
        return h;
    };
    if let Some(mut flows) = flows_from_cash(cash, first_tick) {
        flows.fills = h.flows.fills;
        flows.settles = h.flows.settles;
        h.flows = flows;
        h.source = "exchange".into();
    }
    h
}

fn cause_of(v: &Value) -> Option<&'static str> {
    match v.get("event").and_then(Value::as_str)? {
        "traded" | "trade-outcome" => Some(match v.get("side").and_then(Value::as_str) {
            Some("sell") => "trade_sold",
            _ => "trade_bought",
        }),
        "load-closed" => Some("freight"),
        "paid-down" => Some("debt_paid"),
        "outfitted" => Some("outfit"),
        "acted" => {
            let d = v.get("decision").and_then(Value::as_str).unwrap_or("");
            if d.starts_with("Refuel") || d.starts_with("DivertToPump") {
                Some("fuel")
            } else if d.starts_with("Repair") {
                Some("repair")
            } else if d.starts_with("Collect") || d.starts_with("Book") {
                Some("freight")
            } else {
                None
            }
        }
        _ => None,
    }
}

fn book(flows: &mut Flows, cause: &str, delta: i64) {
    match cause {
        "trade_sold" => flows.trade_sold += delta,
        "trade_bought" => flows.trade_bought += delta,
        "freight" => flows.freight += delta,
        "fuel" => flows.fuel += delta,
        "repair" => flows.repair += delta,
        "outfit" => flows.outfit += delta,
        "debt_paid" => flows.debt_paid += delta,
        _ => flows.other += delta,
    }
}

/// Read one hull's journal into readings and attributed flows, from `since` (unix
/// seconds). The reading before `since` seeds the purse so the first delta inside
/// the window is not the whole balance.
pub(crate) fn from_journal(text: &str, since: i64) -> History {
    let lines: Vec<Value> = text
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect();
    let mut points: Vec<Point> = Vec::new();
    let mut flows = Flows::default();
    let mut prev: Option<(i64, bool)> = None; // (credits, that reading was inside)
    let mut cause: &str = "other";
    let mut best: Option<Move> = None;
    let mut worst: Option<Move> = None;
    for v in &lines {
        let at = v.get("at").and_then(Value::as_i64).unwrap_or(0);
        let tick = v.get("tick").and_then(Value::as_i64).unwrap_or(0);
        let inside = at >= since;
        if let Some(c) = cause_of(v) {
            cause = c;
            if inside {
                match v.get("event").and_then(Value::as_str) {
                    Some("trade-outcome")
                        if v.get("outcome").and_then(Value::as_str) == Some("filled") =>
                    {
                        flows.fills += 1
                    }
                    Some("load-closed") => flows.settles += 1,
                    _ => {}
                }
            }
        }
        let Some(credits) = v.get("credits").and_then(Value::as_i64) else {
            continue;
        };
        if let Some((p, prev_inside)) = prev {
            let delta = credits - p;
            // A delta counts when BOTH readings are inside the window, so the flows
            // add up to exactly the summary's start → now.
            if inside && prev_inside && delta != 0 {
                book(&mut flows, cause, delta);
                let m = Move {
                    at,
                    tick,
                    delta,
                    cause: cause.to_string(),
                };
                if best.as_ref().is_none_or(|b| delta > b.delta) {
                    best = Some(m.clone());
                }
                if worst.as_ref().is_none_or(|w| delta < w.delta) {
                    worst = Some(m);
                }
                // A booked delta is spent: the next unexplained one is "other".
                cause = "other";
            }
        }
        prev = Some((credits, inside));
        if inside {
            points.push(Point { at, tick, credits });
        }
    }
    let summary = summarize(&points, best, worst, since);
    History {
        points: thin(&points),
        flows,
        summary,
        source: "journal".into(),
    }
}

/// One reading per hour (the last in each hour), plus the first and the last.
pub(crate) fn thin(points: &[Point]) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::new();
    let mut bucket: Option<i64> = None;
    for p in points {
        let b = p.at / 3600;
        if bucket == Some(b) {
            *out.last_mut().unwrap() = *p;
        } else {
            out.push(*p);
            bucket = Some(b);
        }
    }
    if let (Some(first), Some(last)) = (points.first(), points.last()) {
        if out.first() != Some(first) {
            out.insert(0, *first);
        }
        if out.last() != Some(last) {
            out.push(*last);
        }
    }
    out
}

fn summarize(points: &[Point], best: Option<Move>, worst: Option<Move>, since: i64) -> Summary {
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return Summary::default();
    };
    let now = last.at;
    let window_days = ((now - since).max(1)) as f64 / 86_400.0;
    // Least squares of credits against days since the first reading.
    let n = points.len() as f64;
    let xs: Vec<f64> = points
        .iter()
        .map(|p| (p.at - first.at) as f64 / 86_400.0)
        .collect();
    let ys: Vec<f64> = points.iter().map(|p| p.credits as f64).collect();
    let mx = xs.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let sxx: f64 = xs.iter().map(|x| (x - mx) * (x - mx)).sum();
    let sxy: f64 = xs.iter().zip(&ys).map(|(x, y)| (x - mx) * (y - my)).sum();
    let trend = if sxx > 0.0 { sxy / sxx } else { 0.0 };
    Summary {
        window_days,
        readings: points.len(),
        credits_start: first.credits,
        credits_now: last.credits,
        delta: last.credits - first.credits,
        trend_per_day: (trend * 10.0).round() / 10.0,
        best,
        worst,
    }
}

/// Several hulls as one captain: flows summed, purses summed per hour (each hull's
/// last-known reading carried forward), summary over the pooled curve.
pub(crate) fn pool(hulls: &[History], since: i64) -> History {
    let mut flows = Flows::default();
    for h in hulls {
        flows.add(&h.flows);
    }
    let source = if !hulls.is_empty() && hulls.iter().all(|h| h.source == "exchange") {
        "exchange"
    } else if hulls.iter().any(|h| h.source == "exchange") {
        "mixed"
    } else {
        "journal"
    }
    .to_string();
    let mut buckets: std::collections::BTreeSet<i64> = std::collections::BTreeSet::new();
    for h in hulls {
        for p in &h.points {
            buckets.insert(p.at / 3600);
        }
    }
    let mut points: Vec<Point> = Vec::new();
    for b in buckets {
        let end = (b + 1) * 3600 - 1;
        let mut sum = 0;
        let mut tick = 0;
        let mut any = false;
        for h in hulls {
            if let Some(p) = h.points.iter().rfind(|p| p.at <= end) {
                sum += p.credits;
                tick = tick.max(p.tick);
                any = true;
            }
        }
        if any {
            let at = hulls
                .iter()
                .filter_map(|h| h.points.iter().rfind(|p| p.at <= end))
                .map(|p| p.at)
                .max()
                .unwrap_or(end);
            points.push(Point {
                at,
                tick,
                credits: sum,
            });
        }
    }
    let best = hulls
        .iter()
        .filter_map(|h| h.summary.best.clone())
        .max_by_key(|m| m.delta);
    let worst = hulls
        .iter()
        .filter_map(|h| h.summary.worst.clone())
        .min_by_key(|m| m.delta);
    let summary = summarize(&points, best, worst, since);
    History {
        points,
        flows,
        summary,
        source,
    }
}

/// The analysis, in the host's voice: what the numbers say, in the order a captain
/// reads them — the bottom line, where it came from, where it went, the trend.
pub(crate) fn analysis(h: &History) -> Vec<String> {
    let s = &h.summary;
    let f = &h.flows;
    if s.readings == 0 {
        return vec!["no readings in this window".into()];
    }
    let sign = |n: i64| {
        if n >= 0 {
            format!("+ℳ{n}")
        } else {
            format!("−ℳ{}", -n)
        }
    };
    let mut out = vec![format!(
        "{} over {:.1} days: ℳ{} → ℳ{}",
        sign(s.delta),
        s.window_days,
        s.credits_start,
        s.credits_now
    )];
    let mut ins: Vec<String> = Vec::new();
    if f.freight != 0 {
        ins.push(format!(
            "freight {} on {} settle(s)",
            sign(f.freight),
            f.settles
        ));
    }
    if f.fills > 0 {
        ins.push(format!(
            "trade {} realized on {} fill(s) (sold ℳ{}, bought ℳ{})",
            sign(f.trade_net()),
            f.fills,
            f.trade_sold,
            -f.trade_bought
        ));
    }
    if !ins.is_empty() {
        out.push(format!("earned: {}", ins.join("; ")));
    }
    let mut outs: Vec<String> = Vec::new();
    for (label, n) in [
        ("fuel", f.fuel),
        ("repair", f.repair),
        ("outfit", f.outfit),
        ("debt paid", f.debt_paid),
        ("dock", f.dock),
        ("other", f.other),
    ] {
        if n != 0 {
            outs.push(format!("{label} {}", sign(n)));
        }
    }
    if !outs.is_empty() {
        out.push(format!("spent: {}", outs.join(", ")));
    }
    out.push(format!(
        "trend {} per day across {} readings",
        sign(s.trend_per_day.round() as i64),
        s.readings
    ));
    if let Some(b) = &s.best {
        out.push(format!(
            "best single move {} ({}, t{})",
            sign(b.delta),
            b.cause,
            b.tick
        ));
    }
    if let Some(w) = &s.worst {
        if w.delta < 0 {
            out.push(format!(
                "worst single move {} ({}, t{})",
                sign(w.delta),
                w.cause,
                w.tick
            ));
        }
    }
    out
}

/// A hull's history from its store.
pub(crate) fn for_ship(ship_dir: &Path, since: i64) -> History {
    let text = std::fs::read_to_string(ship_dir.join("journal.jsonl")).unwrap_or_default();
    from_journal(&text, since)
}

/// `24h` / `7d` / `30d` → seconds; anything else is a week.
pub(crate) fn window_seconds(s: Option<&str>) -> i64 {
    match s.map(str::trim) {
        Some(w) if w.ends_with('h') => w.trim_end_matches('h').parse::<i64>().unwrap_or(24) * 3_600,
        Some(w) if w.ends_with('d') => w.trim_end_matches('d').parse::<i64>().unwrap_or(7) * 86_400,
        _ => 7 * 86_400,
    }
}

pub(crate) fn to_json(h: &History, with_points: bool) -> Value {
    let mut v = json!({
        "flows": h.flows,
        "flows_source": h.source,
        "summary": h.summary,
        "analysis": analysis(h),
    });
    if with_points {
        v["points"] = json!(h.points);
    }
    v
}

#[allow(dead_code)]
fn _keep(_: BTreeMap<String, i64>) {}

#[cfg(test)]
mod tests {
    #[test]
    fn the_exchanges_cash_ledger_names_every_flow_and_the_opening_line_is_not_one() {
        let cash = json!({"credits": 6193, "lines": [
            {"tick": 13264, "amount": -12, "kind": "dock", "note": "dock fee at velvet-array"},
            {"tick": 13263, "amount": -5, "kind": "other", "note": "departed"},
            {"tick": 13262, "amount": -3, "kind": "unnamed", "note": "credits moved with no receipt"},
            {"tick": 13256, "amount": 476, "kind": "freight", "note": "L5391 settled"},
            {"tick": 13250, "amount": -105, "kind": "trade", "note": "bought 10 kibble"},
            {"tick": 13249, "amount": 300, "kind": "trade", "note": "sold 12 pate"},
            {"tick": 13248, "amount": -600, "kind": "lease", "note": "the desk is holding"},
            {"tick": 13240, "amount": -38, "kind": "fuel", "note": "fuelled 47"},
            {"tick": 13230, "amount": -9000, "kind": "refit", "note": "before the window"},
            {"tick": 13000, "amount": 10000, "kind": "opening", "note": "carried forward"}
        ]});
        let f = flows_from_cash(&cash, 13240).unwrap();
        assert_eq!((f.trade_sold, f.trade_bought), (300, -105));
        assert_eq!(
            (f.freight, f.fuel, f.debt_paid, f.other, f.outfit, f.dock),
            (476, -38, -600, -8, 0, -12)
        );
        assert!(flows_from_cash(&json!({"lines": []}), 0).is_none());
        assert!(flows_from_cash(&json!({"error": "no such route"}), 0).is_none());
    }

    use super::*;

    fn line(at: i64, tick: i64, event: &str, credits: Option<i64>, extra: &str) -> String {
        let c = credits
            .map(|c| format!(r#","credits":{c}"#))
            .unwrap_or_default();
        format!(r#"{{"at":{at},"tick":{tick},"event":"{event}"{c}{extra}}}"#)
    }

    /// A week of one hull, hand-built: a refuel, a haul settled, a buy and a sell,
    /// a lease charge nobody acted for. Every delta lands where the last act says.
    #[test]
    fn deltas_are_exact_and_booked_to_the_last_act() {
        let j = [
            line(1000, 1, "holding", Some(1000), ""),
            line(2000, 2, "acted", Some(1000), r#","decision":"Refuel""#),
            line(3000, 3, "holding", Some(940), ""), // fuel −60
            line(
                4000,
                4,
                "acted",
                Some(940),
                r#","decision":"Collect { load_id: \"L1\" }""#,
            ),
            line(5000, 5, "load-closed", Some(1140), r#","load":"L1""#), // freight +200
            line(6000, 6, "traded", Some(1140), r#","side":"buy""#),
            line(
                7000,
                7,
                "trade-outcome",
                Some(840),
                r#","side":"buy","outcome":"filled","total":300"#,
            ), // −300
            line(8000, 8, "traded", Some(840), r#","side":"sell""#),
            line(
                9000,
                9,
                "trade-outcome",
                Some(1290),
                r#","side":"sell","outcome":"filled","total":450"#,
            ), // +450
            line(10000, 10, "holding", Some(1250), ""), // −40 unexplained
        ]
        .join("\n");
        let h = from_journal(&j, 0);
        assert_eq!(h.flows.fuel, -60);
        assert_eq!(h.flows.freight, 200);
        assert_eq!(h.flows.trade_bought, -300);
        assert_eq!(h.flows.trade_sold, 450);
        assert_eq!(h.flows.other, -40);
        assert_eq!(h.flows.fills, 2);
        assert_eq!(h.flows.settles, 1);
        assert_eq!(h.summary.delta, 250);
        assert_eq!(h.summary.best.as_ref().unwrap().cause, "trade_sold");
        assert_eq!(h.summary.worst.as_ref().unwrap().cause, "trade_bought");
        assert!(h.summary.trend_per_day > 0.0);
        let a = analysis(&h);
        assert!(a[0].starts_with("+ℳ250 over"), "{a:?}");
        assert!(a.iter().any(|s| s.contains("freight +ℳ200")), "{a:?}");
        assert!(a.iter().any(|s| s.contains("fuel −ℳ60")), "{a:?}");
    }

    /// The window seeds the purse from the reading before it, so the first delta
    /// inside is a delta and not the whole balance; thinning keeps one per hour
    /// and both ends.
    #[test]
    fn a_window_seeds_from_before_and_thins_to_hours() {
        let mut lines = vec![line(0, 0, "holding", Some(5000), "")];
        for i in 1..=200 {
            lines.push(line(i * 60, i, "holding", Some(5000 + i), ""));
        }
        let j = lines.join("\n");
        let h = from_journal(&j, 6000); // from minute 100
        assert_eq!(
            h.flows.other, 100,
            "deltas inside only, not the 5000 balance"
        );
        assert_eq!(h.summary.credits_start, 5100);
        assert_eq!(h.summary.credits_now, 5200);
        assert!(h.points.len() <= 4, "{} points", h.points.len());
        assert_eq!(h.points.first().unwrap().credits, 5100);
        assert_eq!(h.points.last().unwrap().credits, 5200);
    }

    /// Two hulls pool: flows sum, purses sum per hour with carry-forward.
    #[test]
    fn two_hulls_pool_into_one_captain() {
        let a = from_journal(
            &[
                line(0, 0, "holding", Some(100), ""),
                line(7200, 2, "holding", Some(150), ""),
            ]
            .join("\n"),
            0,
        );
        let b = from_journal(
            &[
                line(0, 0, "holding", Some(200), ""),
                line(3600, 1, "holding", Some(180), ""),
            ]
            .join("\n"),
            0,
        );
        let p = pool(&[a, b], 0);
        assert_eq!(p.flows.other, 30);
        let creds: Vec<i64> = p.points.iter().map(|x| x.credits).collect();
        assert_eq!(
            creds,
            vec![300, 280, 330],
            "hour 0: 100+200; hour 1: 100+180; hour 2: 150+180"
        );
        assert_eq!(p.summary.delta, 30);
    }

    #[test]
    fn windows_parse() {
        assert_eq!(window_seconds(Some("24h")), 86_400);
        assert_eq!(window_seconds(Some("30d")), 30 * 86_400);
        assert_eq!(window_seconds(None), 7 * 86_400);
    }
}
