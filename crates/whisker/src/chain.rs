//! The supply chain, read as arithmetic — T-238 brick 1 (Ian, 2026-09-02: "our
//! plans looking forward should utilize this information in p&L maximization
//! routines").
//!
//! The exchange already serves the whole production graph: `/v1/reference`
//! carries every station's recipes (inputs, outputs, ticks per cycle) and every
//! good's decay; the quotes whisker already reads carry each shelf's live stock,
//! capacity, and equilibrium. This module turns those into the two numbers a
//! forward plan wants:
//!
//! - **runway** — how many ticks a works can keep eating a given input before
//!   its shelf runs dry (a shrinking runway is a bid that must rise: feed it);
//! - **headroom** — how many ticks its output shelf can keep filling before it
//!   hits capacity (a shrinking headroom is an ask that must fall: lift it).
//!
//! HONESTY BOUND, stated once and carried in the type: the wire does not serve
//! line UTILIZATION (the works screen's "2/10"), so every rate here assumes the
//! lines run FULL. Runway is therefore a LOWER bound and headroom is a lower
//! bound too — the works cannot eat faster or fill faster than this. The live
//! corrective is the shelf's own stock-vs-equilibrium pressure, which the
//! quotes serve and the merchant already prices. If Jeff exposes utilization,
//! `Flow::rate_per_kilotick` is where it lands.
//!
//! Pure: no socket, no clock. The runner hands in parsed reference/quote JSON;
//! fixtures hand in the same shapes. Integer arithmetic throughout, engine
//! style — rates are units per 1,000 ticks (a "kilotick") so short cycles keep
//! precision without floats.

use std::collections::BTreeMap;

use serde_json::Value;

/// One production line at a station, as `/v1/reference` serves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    pub id: String,
    pub station: String,
    /// Good → units consumed per cycle.
    pub inputs: BTreeMap<String, i64>,
    /// Good → units produced per cycle.
    pub outputs: BTreeMap<String, i64>,
    pub ticks_per_cycle: i64,
    /// The line's MEASURED share of full running, bps — `FULL_BPS` until the
    /// production ledger says otherwise (T-238 brick 4). This is where the
    /// header's "if Jeff exposes utilization" lands.
    pub utilization_bps: i64,
}

/// Parse the reference's `recipes` array. Rows missing their load-bearing
/// fields are skipped — a chain model must never invent a line.
pub fn parse_recipes(reference: &Value) -> Vec<Recipe> {
    let Some(rows) = reference.get("recipes").and_then(Value::as_array) else {
        return Vec::new();
    };
    rows.iter()
        .filter_map(|r| {
            let goods = |key: &str| -> BTreeMap<String, i64> {
                r.get(key)
                    .and_then(Value::as_object)
                    .map(|m| {
                        m.iter()
                            .filter_map(|(g, v)| Some((g.clone(), v.as_i64()?)))
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let recipe = Recipe {
                id: r.get("id")?.as_str()?.to_string(),
                station: r.get("station")?.as_str()?.to_string(),
                inputs: goods("inputs"),
                outputs: goods("outputs"),
                ticks_per_cycle: r.get("ticksPerCycle")?.as_i64()?,
                utilization_bps: FULL_BPS,
            };
            (recipe.ticks_per_cycle > 0 && !(recipe.inputs.is_empty() && recipe.outputs.is_empty()))
                .then_some(recipe)
        })
        .collect()
}

/// Each good's decay in basis points per tick(ish), from the reference's
/// `goods` — the carry cost a forward plan charges itself for slow legs.
pub fn parse_decay(reference: &Value) -> BTreeMap<String, i64> {
    reference
        .get("goods")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|g| {
                    Some((
                        g.get("id")?.as_str()?.to_string(),
                        g.get("decayBps").and_then(Value::as_i64).unwrap_or(0),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A live shelf at one station, as the quotes serve it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shelf {
    pub station: String,
    pub good: String,
    pub stock: i64,
    pub capacity: i64,
    pub equilibrium: i64,
}

/// Which way a station moves a good.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowKind {
    /// The works EATS this good here — its shelf drains, its bid firms.
    Eats,
    /// The works MAKES this good here — its shelf fills, its ask softens.
    Makes,
}

/// One station×good flow, with the live shelf folded in when known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flow {
    pub station: String,
    pub good: String,
    pub kind: FlowKind,
    /// Units per 1,000 ticks at FULL lines (the honesty bound above).
    pub rate_per_kilotick: i64,
    /// The live shelf, when the caller had quotes for this berth.
    pub shelf: Option<Shelf>,
    /// Ticks until the shelf is empty (Eats) or full (Makes), at full lines —
    /// along the dispatch schedule below when one is set. None without a shelf,
    /// or when the rate is zero.
    pub horizon_ticks: Option<i64>,
    /// What the dispatch feed says this flow will do and when: the board's
    /// announced events on this station×good, as rate windows relative to now.
    /// Empty = the lines run at their full rate throughout (the bound above).
    pub windows: Vec<Window>,
}

/// One stretch of ticks over which a flow runs at a multiple of its full rate —
/// a dispatch card in force, weighted by the odds it fires. `from` and `to` are
/// ticks FROM NOW (`to` exclusive); `rate_bps` is the expected multiplier,
/// 10,000 = the full rate, 2,200 = a hold at 22%, 15,000 = a third press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub from: i64,
    pub to: i64,
    pub rate_bps: i64,
}

/// Fold recipes and live shelves into the full flow table, one row per
/// station×good×direction, rates summed across that station's lines.
pub fn flows(recipes: &[Recipe], shelves: &[Shelf]) -> Vec<Flow> {
    let mut rates: BTreeMap<(String, String, bool), i64> = BTreeMap::new();
    for r in recipes {
        // The line's rate at its measured share: full lines when unmeasured.
        let share = r.utilization_bps.clamp(0, FULL_BPS);
        for (good, units) in &r.inputs {
            *rates
                .entry((r.station.clone(), good.clone(), true))
                .or_insert(0) += units * 1000 / r.ticks_per_cycle * share / FULL_BPS;
        }
        for (good, units) in &r.outputs {
            *rates
                .entry((r.station.clone(), good.clone(), false))
                .or_insert(0) += units * 1000 / r.ticks_per_cycle * share / FULL_BPS;
        }
    }
    rates
        .into_iter()
        .map(|((station, good, eats), rate)| {
            let shelf = shelves
                .iter()
                .find(|s| s.station == station && s.good == good)
                .cloned();
            let horizon_ticks = shelf.as_ref().and_then(|s| {
                if rate <= 0 {
                    return None;
                }
                let room = if eats {
                    s.stock
                } else {
                    (s.capacity - s.stock).max(0)
                };
                Some(room * 1000 / rate)
            });
            Flow {
                station,
                good,
                kind: if eats {
                    FlowKind::Eats
                } else {
                    FlowKind::Makes
                },
                rate_per_kilotick: rate,
                shelf,
                horizon_ticks,
                windows: Vec::new(),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The dispatch feed — T-238 brick 3 (Ian, 2026-09-15: "be sure those changes are
// made and that familiar is optimized as the ships computer to maximize long term
// profits"). `/v1/news` announces an event BEFORE it bites: a headline, an honest
// prior (`tier`), the tick it takes effect and the tick it expires. Jeff's own
// words on the route: "Reading the board and moving before `effectiveAtTick` is
// the entire information game; the lead time is the point."
//
// What a headline MEANS is public knowledge: the exchange's content pack ships
// the deck (`Content/market/events.json`, ucf-exchange), every card a headline
// with its effect — a production or consumption multiplier on one station×good,
// a spread at one berth, a lane's cost — plus lead, duration and the odds it
// fires. The pilot carries a copy of the deck (`content/ucf-events.json`,
// `tools/sync-ucf-deck.sh` refreshes it) and reads the feed against it, so a
// "third press recommissioned at Tranquility" is a kibble-loaf shelf filling at
// 150% for the next fifty ticks, and a "production hold at Cannery Row" is a
// starving buyer downstream two ticks before the counter shows it.
//
// NOT read, on purpose: `/v1/events`, the overwatch route that publishes the
// resolved coin (`willFire`) and the exact magnitude. PROD answers a player key
// on it (2026-09-15) — api.md calls that route "the sharpest edge of the scope
// trap: a player reading this has no information game left to play". The
// familiar plays the game as designed, from the prior; the leak is Jeff's to
// close and is flagged to him through Ian.
// ---------------------------------------------------------------------------

/// What one card does to the world while it is in effect. Magnitudes are
/// multipliers in bps: 4,000 means 40% of normal, 15,000 means 150%.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// The station's lines MAKE this good at `bps` of their rate.
    Production {
        station: String,
        good: String,
        bps: i64,
    },
    /// The station EATS this good at `bps` of its rate.
    Consumption {
        station: String,
        good: String,
        bps: i64,
    },
    /// The berth's quoted spread, at `bps` of normal.
    Spread { station: String, bps: i64 },
    /// A lane's cost, at `bps` of normal (`lane` = `origin--dest`).
    LaneCost { lane: String, bps: i64 },
}

impl Effect {
    /// The target the exchange would name: `station/good`, a station, or a lane.
    pub fn target(&self) -> String {
        match self {
            Effect::Production { station, good, .. }
            | Effect::Consumption { station, good, .. } => {
                format!("{station}/{good}")
            }
            Effect::Spread { station, .. } => station.clone(),
            Effect::LaneCost { lane, .. } => lane.clone(),
        }
    }

    pub fn bps(&self) -> i64 {
        match self {
            Effect::Production { bps, .. }
            | Effect::Consumption { bps, .. }
            | Effect::Spread { bps, .. }
            | Effect::LaneCost { bps, .. } => *bps,
        }
    }
}

/// One card of the exchange's dispatch deck: a headline and what it means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub id: String,
    pub headline: String,
    pub effect: Effect,
    /// Ticks between the announcement and the effect — the lead the pilot has.
    pub lead_ticks: i64,
    pub duration_ticks: i64,
    /// The odds the card fires once announced, bps. The feed's `tier` is this
    /// number's display band (≥9,500 confirmed, ≥6,000 likely, else rumour).
    pub fire_bps: i64,
}

/// The deck as vendored from Jeff's pack — `content/ucf-events.json`, refreshed
/// by `tools/sync-ucf-deck.sh`. A pack with no deck is an eventless galaxy.
pub fn deck() -> Vec<Card> {
    serde_json::from_str::<Value>(include_str!("../content/ucf-events.json"))
        .map(|v| parse_deck(&v))
        .unwrap_or_default()
}

/// Parse the pack's `events.json` array. A card missing its load-bearing fields
/// or naming an effect this model does not know is skipped — the pilot must
/// never invent a meaning for a headline.
pub fn parse_deck(v: &Value) -> Vec<Card> {
    let Some(rows) = v.as_array() else {
        return Vec::new();
    };
    rows.iter()
        .filter_map(|c| {
            let effect = c.get("effect")?.as_object()?;
            let (kind, body) = effect.iter().next()?;
            let station = || body.get("station")?.as_str().map(String::from);
            let good = || body.get("good")?.as_str().map(String::from);
            let bps = body.get("magnitudeBps")?.as_i64()?;
            let effect = match kind.as_str() {
                "production" => Effect::Production {
                    station: station()?,
                    good: good()?,
                    bps,
                },
                "consumption" => Effect::Consumption {
                    station: station()?,
                    good: good()?,
                    bps,
                },
                "spread" => Effect::Spread {
                    station: station()?,
                    bps,
                },
                "laneCost" => Effect::LaneCost {
                    lane: body.get("lane")?.as_str()?.to_string(),
                    bps,
                },
                _ => return None,
            };
            Some(Card {
                id: c.get("id")?.as_str()?.to_string(),
                headline: c.get("headline")?.as_str()?.to_string(),
                effect,
                lead_ticks: c.get("leadTicks").and_then(Value::as_i64).unwrap_or(0),
                duration_ticks: c.get("durationTicks").and_then(Value::as_i64).unwrap_or(0),
                fire_bps: c
                    .get("fireProbabilityBps")
                    .and_then(Value::as_i64)
                    .unwrap_or(FULL_BPS),
            })
        })
        .collect()
}

/// A neutral multiplier, and the odds of a certainty.
pub const FULL_BPS: i64 = 10_000;

/// The honest prior behind each display band, used when a headline matches no
/// card (the band's floor in the pack's own thresholds; a rumour below it).
pub fn tier_odds_bps(tier: &str) -> i64 {
    match tier {
        "confirmed" => 9_500,
        "likely" => 6_000,
        _ => 4_000,
    }
}

/// One item off `/v1/news`, read against the deck: what was announced, what it
/// means if the feed's headline is a card we know, and the odds it bites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dispatch {
    pub headline: String,
    /// The card's id when the headline is one of the deck's; None for a headline
    /// the pack we carry does not know (a newer deck, or prose).
    pub card: Option<String>,
    pub effect: Option<Effect>,
    pub tier: String,
    /// `announced` or `in-effect` — withdrawn and expired items are not dispatches.
    pub status: String,
    pub announced_at: i64,
    pub effective_at: i64,
    pub expires_at: i64,
    /// The odds the effect applies, bps: certain once in effect; the card's own
    /// fire odds while announced; the tier's prior for an unknown headline.
    pub odds_bps: i64,
}

impl Dispatch {
    /// One line for the journal and the bridge: what, where, how much, when.
    pub fn line(&self) -> String {
        match (&self.card, &self.effect) {
            (Some(card), Some(effect)) => format!(
                "{card} @ {} ×{}bps t{}–t{} ({}, {}, odds {})",
                effect.target(),
                effect.bps(),
                self.effective_at,
                self.expires_at,
                self.tier,
                self.status,
                self.odds_bps
            ),
            _ => format!(
                "? \"{}\" t{}–t{} ({}, {})",
                self.headline, self.effective_at, self.expires_at, self.tier, self.status
            ),
        }
    }
}

/// Read the feed against the deck. Withdrawn and expired items are dropped;
/// the rest keep the feed's order.
pub fn parse_news(news: &Value, deck: &[Card]) -> Vec<Dispatch> {
    let Some(rows) = news.as_array() else {
        return Vec::new();
    };
    rows.iter()
        .filter_map(|n| {
            let status = n.get("status")?.as_str()?.to_string();
            if status != "announced" && status != "in-effect" {
                return None;
            }
            let headline = n.get("headline")?.as_str()?.to_string();
            let tier = n
                .get("tier")
                .and_then(Value::as_str)
                .unwrap_or("rumour")
                .to_string();
            let card = deck.iter().find(|c| c.headline == headline);
            let odds_bps = if status == "in-effect" {
                FULL_BPS
            } else {
                card.map_or_else(|| tier_odds_bps(&tier), |c| c.fire_bps)
            };
            Some(Dispatch {
                headline,
                card: card.map(|c| c.id.clone()),
                effect: card.map(|c| c.effect.clone()),
                tier,
                status,
                announced_at: n
                    .get("announcedAtTick")
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
                effective_at: n
                    .get("effectiveAtTick")
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
                expires_at: n.get("expiresAtTick").and_then(Value::as_i64).unwrap_or(0),
                odds_bps,
            })
        })
        .collect()
}

/// Lay the dispatches over the flows as rate windows and re-walk every touched
/// horizon. A production card moves the Makes flow at its station×good, a
/// consumption card the Eats flow; spread and lane cards move no shelf and are
/// left to the journal. A window that has already closed is not laid; one in
/// effect starts now. The expected multiplier weights the card's magnitude by
/// its odds, so a rumour of a hold slows the line a little and a hold in effect
/// slows it fully.
pub fn schedule(flows: &mut [Flow], dispatches: &[Dispatch], now_tick: i64) {
    for f in flows.iter_mut() {
        f.windows.clear();
    }
    for d in dispatches {
        let (station, good, bps, kind) = match &d.effect {
            Some(Effect::Production { station, good, bps }) => {
                (station, good, *bps, FlowKind::Makes)
            }
            Some(Effect::Consumption { station, good, bps }) => {
                (station, good, *bps, FlowKind::Eats)
            }
            _ => continue,
        };
        let from = (d.effective_at - now_tick).max(0);
        let to = d.expires_at - now_tick;
        if to <= from {
            continue;
        }
        // Expected rate: full, plus the card's departure from full at its odds.
        let rate_bps = FULL_BPS + (bps - FULL_BPS) * d.odds_bps.clamp(0, FULL_BPS) / FULL_BPS;
        for f in flows.iter_mut() {
            if f.kind == kind && &f.station == station && &f.good == good {
                f.windows.push(Window { from, to, rate_bps });
            }
        }
    }
    // Every horizon is re-walked, not only the touched ones: a flow whose window
    // just closed goes back to the counter's own arithmetic (the walk with no
    // windows IS that arithmetic).
    for f in flows.iter_mut() {
        f.windows.sort_by_key(|w| (w.from, w.to));
        f.horizon_ticks = f.walk_horizon();
    }
}

/// The feeds worth flying: inputs whose shelf runs dry inside the horizon, most
/// urgent first. A starving works is a rising bid with a deadline on it.
pub fn starving(flows: &[Flow], horizon_ticks: i64) -> Vec<&Flow> {
    let mut hungry: Vec<&Flow> = flows
        .iter()
        .filter(|f| f.kind == FlowKind::Eats)
        .filter(|f| f.horizon_ticks.is_some_and(|h| h <= horizon_ticks))
        .collect();
    hungry.sort_by_key(|f| f.horizon_ticks);
    hungry
}

/// The lifts worth flying: outputs whose shelf fills inside the horizon, most
/// urgent first. A glutting works is a softening ask — and a stalled line once
/// the shelf is full, which starves every buyer downstream.
pub fn glutting(flows: &[Flow], horizon_ticks: i64) -> Vec<&Flow> {
    let mut full: Vec<&Flow> = flows
        .iter()
        .filter(|f| f.kind == FlowKind::Makes)
        .filter(|f| f.horizon_ticks.is_some_and(|h| h <= horizon_ticks))
        .collect();
    full.sort_by_key(|f| f.horizon_ticks);
    full
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The reference, shaped exactly as PROD serves it (checked live 2026-09-02:
    /// the Biscuit Press row's own numbers).
    fn reference() -> Value {
        json!({
            "recipes": [
                {"id": "biscuit-press", "displayName": "Biscuit Press, Grade-2 Line",
                 "station": "tranquility", "ticksPerCycle": 8,
                 "inputs": {"grain": 22, "tinplate": 4},
                 "outputs": {"biscuit-substrate": 16}},
                {"id": "cannery-line", "station": "cannery-row", "ticksPerCycle": 10,
                 "inputs": {"fishmeal": 22, "grain": 16, "tinplate": 4},
                 "outputs": {"kibble-loaf": 33}},
                {"id": "gravy-reduction", "station": "cannery-row", "ticksPerCycle": 5,
                 "inputs": {"fishmeal": 10, "water-ice": 7},
                 "outputs": {"gravy-base": 10}},
                {"id": "broken-row", "station": "nowhere", "ticksPerCycle": 0,
                 "inputs": {"grain": 1}, "outputs": {}}
            ],
            "goods": [
                {"id": "fishmeal", "decayBps": 12},
                {"id": "kibble-loaf", "decayBps": 4}
            ]
        })
    }

    #[test]
    fn recipes_parse_the_wire_shape_and_refuse_broken_rows() {
        let recipes = parse_recipes(&reference());
        assert_eq!(recipes.len(), 3, "the zero-cycle row is not a line");
        let press = recipes.iter().find(|r| r.id == "biscuit-press").unwrap();
        assert_eq!(press.station, "tranquility");
        assert_eq!(press.inputs["grain"], 22);
        assert_eq!(press.outputs["biscuit-substrate"], 16);
        assert_eq!(press.ticks_per_cycle, 8);
        assert_eq!(parse_decay(&reference())["fishmeal"], 12);
    }

    #[test]
    fn rates_sum_across_a_stations_lines_per_kilotick() {
        // Cannery Row eats fishmeal on TWO lines: 22/10-tick and 10/5-tick →
        // 2200 + 2000 = 4200 units per kilotick. The chain sees the station's
        // whole appetite, not one line's.
        let flows = flows(&parse_recipes(&reference()), &[]);
        let fishmeal = flows
            .iter()
            .find(|f| f.station == "cannery-row" && f.good == "fishmeal")
            .unwrap();
        assert_eq!(fishmeal.kind, FlowKind::Eats);
        assert_eq!(fishmeal.rate_per_kilotick, 4200);
        assert_eq!(fishmeal.horizon_ticks, None, "no shelf, no horizon");
    }

    #[test]
    fn runway_and_headroom_come_from_the_live_shelf() {
        let shelves = vec![
            Shelf {
                station: "cannery-row".into(),
                good: "fishmeal".into(),
                stock: 862,
                capacity: 1200,
                equilibrium: 600,
            },
            Shelf {
                station: "cannery-row".into(),
                good: "kibble-loaf".into(),
                stock: 356,
                capacity: 400,
                equilibrium: 200,
            },
        ];
        let all = flows(&parse_recipes(&reference()), &shelves);
        let runway = all
            .iter()
            .find(|f| f.good == "fishmeal" && f.kind == FlowKind::Eats)
            .unwrap();
        // 862 units at 4200/kilotick → 205 ticks of eating left, at full lines.
        assert_eq!(runway.horizon_ticks, Some(205));
        let head = all
            .iter()
            .find(|f| f.good == "kibble-loaf" && f.kind == FlowKind::Makes)
            .unwrap();
        // 44 units of room at 3300/kilotick → 13 ticks until the shelf is full.
        assert_eq!(head.horizon_ticks, Some(13));
    }

    #[test]
    fn starving_and_glutting_rank_by_urgency_inside_the_horizon() {
        let shelves = vec![
            Shelf {
                station: "cannery-row".into(),
                good: "fishmeal".into(),
                stock: 40, // 9 ticks of appetite — urgent
                capacity: 1200,
                equilibrium: 600,
            },
            Shelf {
                station: "cannery-row".into(),
                good: "water-ice".into(),
                stock: 700, // 500 ticks — comfortable
                capacity: 900,
                equilibrium: 400,
            },
            Shelf {
                station: "cannery-row".into(),
                good: "gravy-base".into(),
                stock: 1990, // nearly full at 2000
                capacity: 2000,
                equilibrium: 900,
            },
        ];
        let all = flows(&parse_recipes(&reference()), &shelves);
        let feeds = starving(&all, 100);
        assert_eq!(
            feeds
                .iter()
                .map(|f| (f.good.as_str(), f.horizon_ticks.unwrap()))
                .collect::<Vec<_>>(),
            vec![("fishmeal", 9)],
            "the comfortable shelf stays off the feed list"
        );
        let lifts = glutting(&all, 100);
        assert_eq!(lifts.len(), 1);
        assert_eq!(lifts[0].good, "gravy-base");
        assert_eq!(lifts[0].horizon_ticks, Some(5));
    }

    // ── the dispatch feed (T-238 brick 3) ──────────────────────────────────

    #[test]
    fn the_vendored_deck_parses_and_every_kind_of_card_names_its_effect() {
        let deck = deck();
        assert_eq!(
            deck.len(),
            72,
            "the pack's deck as synced 2026-09-15 (72 cards)"
        );
        let press = deck
            .iter()
            .find(|c| c.id == "press-recommissioning")
            .unwrap();
        assert_eq!(
            press.effect,
            Effect::Production {
                station: "tranquility".into(),
                good: "kibble-loaf".into(),
                bps: 15_000
            }
        );
        assert_eq!(press.effect.target(), "tranquility/kibble-loaf");
        assert!(press.lead_ticks > 0 && press.duration_ticks > 0 && press.fire_bps > 0);
        let desk = deck.iter().find(|c| c.id == "desk-cover-premium").unwrap();
        assert_eq!(
            desk.effect,
            Effect::Spread {
                station: "clawson-drift".into(),
                bps: 13_000
            }
        );
        let lane = deck
            .iter()
            .find(|c| c.id == "approach-lane-closure")
            .unwrap();
        assert!(
            matches!(&lane.effect, Effect::LaneCost { lane, bps: 22_000 } if lane == "cannery-row--ganymede-yards")
        );
        assert!(deck
            .iter()
            .any(|c| matches!(c.effect, Effect::Consumption { .. })));
        // A card the model cannot read is skipped, never guessed at.
        let odd = json!([{"id": "x", "headline": "h", "effect": {"weather": {"magnitudeBps": 5}}}]);
        assert!(parse_deck(&odd).is_empty());
    }

    fn deck_fixture() -> Vec<Card> {
        parse_deck(&json!([
            {"id": "cannery-hold", "headline": "Cannery Row Fulfilment enters full production hold pending board inquiry",
             "effect": {"production": {"station": "cannery-row", "good": "kibble-loaf", "magnitudeBps": 2200}},
             "weight": 3, "leadTicks": 24, "durationTicks": 92, "fireProbabilityBps": 4400},
            {"id": "counter-rush", "headline": "Counter rush at Cannery Row as the shift changes; fishmeal clears fast",
             "effect": {"consumption": {"station": "cannery-row", "good": "fishmeal", "magnitudeBps": 11800}},
             "weight": 8, "leadTicks": 3, "durationTicks": 10, "fireProbabilityBps": 9600}
        ]))
    }

    fn feed() -> Value {
        json!([
            {"headline": "Cannery Row Fulfilment enters full production hold pending board inquiry",
             "tier": "rumour", "announcedAtTick": 100, "effectiveAtTick": 124, "expiresAtTick": 216, "status": "announced"},
            {"headline": "Counter rush at Cannery Row as the shift changes; fishmeal clears fast",
             "tier": "confirmed", "announcedAtTick": 100, "effectiveAtTick": 103, "expiresAtTick": 113, "status": "announced"},
            {"headline": "Weighbridge tared at Ganymede Plate Yards; ore intake pauses for the check",
             "tier": "likely", "announcedAtTick": 90, "effectiveAtTick": 91, "expiresAtTick": 95, "status": "expired"},
            {"headline": "Something the deck we carry has never heard of",
             "tier": "likely", "announcedAtTick": 100, "effectiveAtTick": 110, "expiresAtTick": 120, "status": "announced"},
            {"headline": "Cannery Row Fulfilment enters full production hold pending board inquiry",
             "tier": "rumour", "announcedAtTick": 50, "effectiveAtTick": 74, "expiresAtTick": 166, "status": "in-effect"}
        ])
    }

    /// The two items that lay windows, one per flow: the hold in effect and the
    /// rush announced. `feed()` above carries the same hold twice for the parser.
    fn feed_live() -> Value {
        json!([
            {"headline": "Counter rush at Cannery Row as the shift changes; fishmeal clears fast",
             "tier": "confirmed", "announcedAtTick": 100, "effectiveAtTick": 103, "expiresAtTick": 113, "status": "announced"},
            {"headline": "Cannery Row Fulfilment enters full production hold pending board inquiry",
             "tier": "rumour", "announcedAtTick": 50, "effectiveAtTick": 74, "expiresAtTick": 166, "status": "in-effect"}
        ])
    }

    #[test]
    fn the_feed_is_read_against_the_deck_and_the_odds_are_honest() {
        let ds = parse_news(&feed(), &deck_fixture());
        assert_eq!(ds.len(), 4, "the expired item is not a dispatch");
        // Announced and known: the card's own fire odds, not the tier's band.
        assert_eq!(ds[0].card.as_deref(), Some("cannery-hold"));
        assert_eq!(ds[0].odds_bps, 4_400);
        assert_eq!(ds[1].odds_bps, 9_600);
        // Unknown headline: no meaning invented; the tier's prior stands.
        assert_eq!(ds[2].card, None);
        assert_eq!(ds[2].effect, None);
        assert_eq!(ds[2].odds_bps, 6_000);
        assert!(ds[2].line().starts_with("? \"Something"));
        // In effect: the coin has landed.
        assert_eq!(ds[3].status, "in-effect");
        assert_eq!(ds[3].odds_bps, FULL_BPS);
        assert_eq!(
            ds[3].line(),
            "cannery-hold @ cannery-row/kibble-loaf ×2200bps t74–t166 (rumour, in-effect, odds 10000)"
        );
    }

    #[test]
    fn a_hold_pushes_the_full_shelf_out_and_a_rush_pulls_the_dry_shelf_in() {
        let shelves = vec![
            Shelf {
                station: "cannery-row".into(),
                good: "fishmeal".into(),
                stock: 862,
                capacity: 1200,
                equilibrium: 600,
            },
            Shelf {
                station: "cannery-row".into(),
                good: "kibble-loaf".into(),
                stock: 356,
                capacity: 400,
                equilibrium: 200,
            },
        ];
        let mut all = flows(&parse_recipes(&reference()), &shelves);
        let before: Vec<Option<i64>> = all.iter().map(|f| f.horizon_ticks).collect();
        // The in-effect hold and the announced rush: now = t100.
        let ds = parse_news(&feed_live(), &deck_fixture());
        schedule(&mut all, &ds, 100);
        let loaf = all
            .iter()
            .find(|f| f.good == "kibble-loaf" && f.kind == FlowKind::Makes)
            .unwrap();
        // The hold is IN EFFECT (odds certain) until t166 → 66 ticks from now at
        // 22%: 3300 × 0.22 = 726 milli-units a tick against 44 units of room →
        // 44,000 / 726 = 60 ticks, inside the window. Unscheduled it was 13.
        assert_eq!(
            before[all.iter().position(|f| std::ptr::eq(f, loaf)).unwrap()],
            Some(13)
        );
        assert_eq!(
            loaf.windows,
            vec![Window {
                from: 0,
                to: 66,
                rate_bps: 2_200
            }]
        );
        assert_eq!(loaf.horizon_ticks, Some(60));
        assert_eq!(loaf.stock_at(10), Some(Units(363)), "356 + 7,260 milli");
        // The rush is ANNOUNCED at 96% odds for t103–t113: expected 11,728 bps.
        // Eating 862 fishmeal at 4200/kilotick: 3 ticks full (12,600), 10 ticks
        // at 4925 (49,250), then full again — 800,150 left at 4200 = 190 more:
        // dry at 203, two ticks sooner than the 205 the counter alone showed.
        let meal = all
            .iter()
            .find(|f| f.good == "fishmeal" && f.kind == FlowKind::Eats)
            .unwrap();
        assert_eq!(
            meal.windows,
            vec![Window {
                from: 3,
                to: 13,
                rate_bps: 11_728
            }]
        );
        assert_eq!(meal.horizon_ticks, Some(203));
        assert_eq!(meal.stock_at(13), Some(Units(801)), "862 − 61,850 milli");
        // A shelf the feed never named keeps its unscheduled horizon.
        let ice = all
            .iter()
            .find(|f| f.good == "water-ice" && f.kind == FlowKind::Eats)
            .unwrap();
        assert!(ice.windows.is_empty());
        // Re-laying with an empty feed restores every horizon: the schedule is
        // the feed's, not the flow's.
        schedule(&mut all, &[], 100);
        assert_eq!(
            all.iter().map(|f| f.horizon_ticks).collect::<Vec<_>>(),
            before
        );
    }

    #[test]
    fn a_window_already_closed_or_pointed_at_no_shelf_lays_nothing() {
        let mut all = flows(&parse_recipes(&reference()), &[]);
        let ds = parse_news(&feed_live(), &deck_fixture());
        // At t500 every window in the feed has passed.
        schedule(&mut all, &ds, 500);
        assert!(all.iter().all(|f| f.windows.is_empty()));
        // At t100 the loaf flow takes its window, but with no shelf there is no
        // horizon to walk — and no panic.
        schedule(&mut all, &ds, 100);
        let loaf = all
            .iter()
            .find(|f| f.good == "kibble-loaf" && f.kind == FlowKind::Makes)
            .unwrap();
        assert_eq!(loaf.windows.len(), 1);
        assert_eq!(loaf.horizon_ticks, None);
        assert_eq!(loaf.stock_at(10), None);
    }

    // ── the production ledger (T-238 brick 4) ──────────────────────────────

    /// PROD's own shape, 2026-09-15: io-slagworks' extractor, one cycle a bucket
    /// on a 6-tick line (half its rate), blocked with 6 idle ticks of 12.
    fn production_reply(recipe: &str, cycles: &[i64], idle: &[i64], blocked: bool) -> Value {
        let buckets: Vec<Value> = cycles
            .iter()
            .zip(idle)
            .enumerate()
            .map(|(n, (c, i))| {
                let mut b = json!({"tick": 13008 + 12 * n as i64, "cyclesCompleted": c,
                                   "unitsProduced": c * 38, "unitsConsumed": 0, "idleTicks": i});
                if blocked {
                    b["blockedReason"] = json!("blocked");
                }
                b
            })
            .collect();
        json!({"station": "io-slagworks", "recipe": recipe, "displayName": "Io Extractor",
               "intervalTicks": 12, "buckets": buckets})
    }

    #[test]
    fn the_ledger_measures_a_lines_share_and_a_stall_is_reported_not_applied() {
        let half = parse_production(
            &production_reply("io-extractor", &[1, 1, 1, 1, 1, 1, 1, 1], &[6; 8], true),
            MEASURED_BUCKETS,
        )
        .unwrap();
        assert_eq!(
            (
                half.buckets,
                half.cycles,
                half.idle_ticks,
                half.blocked.as_deref()
            ),
            (4, 4, 24, Some("blocked"))
        );
        // Four buckets of 12 ticks hold eight 6-tick cycles; four ran → 5,000 bps.
        assert_eq!(half.utilization_bps(6), Some(5_000));
        // A 4-tick line in the same window could have run twelve: 4 of 12 → 3,333.
        assert_eq!(half.utilization_bps(4), Some(3_333));
        // Running flat out is full, never more (a bucket can straddle a cycle).
        let full =
            parse_production(&production_reply("r", &[2, 2, 2, 2], &[0; 4], false), 4).unwrap();
        assert_eq!(full.utilization_bps(6), Some(FULL_BPS));
        // Nothing completed: a stall, reported and not a rate.
        let dead = parse_production(
            &production_reply("titan-terraces", &[0, 0, 0], &[12; 3], true),
            4,
        )
        .unwrap();
        assert_eq!(dead.utilization_bps(6), None);
        assert!(dead.stalled() && dead.blocked.is_some());
        assert_eq!(full.blocked, None);
        // A young ledger reads what it has; an uncharted station reads nothing.
        assert_eq!(dead.buckets, 3);
        assert!(
            parse_production(&json!({"station": "x", "recipe": "", "buckets": []}), 4).is_none()
        );
        assert!(parse_production(&json!({"error": "no such route"}), 4).is_none());
    }

    #[test]
    fn a_measured_line_slows_its_flows_and_a_stalled_line_keeps_its_appetite() {
        let recipes = parse_recipes(&reference());
        assert!(recipes.iter().all(|r| r.utilization_bps == FULL_BPS));
        let readings = vec![
            LineReading {
                recipe: "cannery-line".into(),
                interval_ticks: 12,
                buckets: 4,
                cycles: 2,
                idle_ticks: 20,
                blocked: Some("blocked".into()),
            },
            LineReading {
                recipe: "gravy-reduction".into(),
                interval_ticks: 12,
                buckets: 4,
                cycles: 0,
                idle_ticks: 48,
                blocked: Some("starved".into()),
            },
        ];
        let measured = with_utilization(&recipes, &readings);
        let line = measured.iter().find(|r| r.id == "cannery-line").unwrap();
        // Four buckets hold 4.8 ten-tick cycles; two ran → 4,166 bps.
        assert_eq!(line.utilization_bps, 4_166);
        let gravy = measured.iter().find(|r| r.id == "gravy-reduction").unwrap();
        assert_eq!(gravy.utilization_bps, FULL_BPS, "a stall is not a rate");
        let press = measured.iter().find(|r| r.id == "biscuit-press").unwrap();
        assert_eq!(press.utilization_bps, FULL_BPS, "unmeasured runs full");
        // Cannery Row's fishmeal appetite: the line at 41.66% (2200 → 916) plus
        // the stalled gravy line at full (2000) = 2,916 a kilotick, was 4,200.
        let flows = flows(&measured, &[]);
        let meal = flows
            .iter()
            .find(|f| f.station == "cannery-row" && f.good == "fishmeal")
            .unwrap();
        assert_eq!(meal.rate_per_kilotick, 2_916);
        assert_eq!(
            measured_lines(&measured, &readings),
            vec![
                "cannery-line@cannery-row 41% blocked".to_string(),
                "gravy-reduction@cannery-row stalled starved".to_string()
            ]
        );
    }

    #[test]
    fn two_windows_on_one_line_compound_and_the_same_card_twice_is_two_windows() {
        // The same hold announced again while one is in effect: both lay a window.
        let mut all = flows(&parse_recipes(&reference()), &[]);
        schedule(&mut all, &parse_news(&feed(), &deck_fixture()), 100);
        let loaf = all
            .iter()
            .find(|f| f.good == "kibble-loaf" && f.kind == FlowKind::Makes)
            .unwrap();
        assert_eq!(loaf.windows.len(), 2);
        // Half rate over [0,10) and half again over [5,15): a quarter where they
        // overlap. 1000/kilotick: 500×5 + 250×5 = 3,750 milli → 3 units in 10.
        let flow = Flow {
            station: "w".into(),
            good: "g".into(),
            kind: FlowKind::Eats,
            rate_per_kilotick: 1_000,
            shelf: Some(Shelf {
                station: "w".into(),
                good: "g".into(),
                stock: 100,
                capacity: 200,
                equilibrium: 50,
            }),
            horizon_ticks: Some(100),
            windows: vec![
                Window {
                    from: 0,
                    to: 10,
                    rate_bps: 5_000,
                },
                Window {
                    from: 5,
                    to: 15,
                    rate_bps: 5_000,
                },
            ],
        };
        assert_eq!(flow.rate_bps_at(7), 2_500);
        assert_eq!(flow.stock_at(10), Some(Units(97)));
        // 100 units: 3,750 by t10, 500×5 = 2,500 more by t15 (6,250), then full
        // rate: 93,750 / 1000 = 93 more → dry at t108.
        assert_eq!(flow.walk_horizon(), Some(108));
    }
}

// ---------------------------------------------------------------------------
// The production ledger — T-238 brick 4. `/v1/stations/{id}/production?recipe=R`
// (ucf-exchange #5, live on PROD 2026-09-09) is what a station's lines actually
// MADE: per recipe, a short series of equal buckets (`intervalTicks`, 12) of
// `cyclesCompleted`, `unitsProduced`, `unitsConsumed`, `idleTicks` and a
// `blockedReason`. Not the candles — those count what crossed the counter — this
// counts cycles, so a line that has stopped shows as stopped while its shelf
// still lasts. It is the utilization the header above waited for: the share of
// full running a line achieved, measured, and it lands on `Recipe::utilization_bps`.
//
// A line that completed NOTHING over the window keeps its full appetite: a works
// blocked on an empty input shelf is not a works with no appetite, it is the
// most urgent feed on the map, and the shelf's own state (room zero) already says
// so. Scaling it to zero would make it vanish from the starving list at the
// moment it matters most. So the ledger stretches horizons where a line runs
// slow; it never erases a line.
// ---------------------------------------------------------------------------

/// The ledger's bucket, ticks: `MarketState.seriesIntervalTicks` on the engine.
pub const PRODUCTION_INTERVAL_TICKS: i64 = 12;
/// How many of the newest buckets a reading spans (four market hours).
pub const MEASURED_BUCKETS: usize = 4;

/// One line's measured throughput over the newest buckets of its ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineReading {
    pub recipe: String,
    pub interval_ticks: i64,
    /// Buckets actually read (a young ledger has fewer than asked).
    pub buckets: i64,
    pub cycles: i64,
    pub idle_ticks: i64,
    /// The newest `blockedReason` in the window ("blocked" on PROD's engine;
    /// "starved" and kin on newer ones) — None when every bucket ran free.
    pub blocked: Option<String>,
}

impl LineReading {
    /// The share of full running, bps, given the line's cycle time: cycles
    /// completed against the cycles the window could hold, capped at full. None
    /// when the line completed nothing — a stall is reported, never applied as a
    /// rate (see the banner above).
    pub fn utilization_bps(&self, ticks_per_cycle: i64) -> Option<i64> {
        if self.cycles <= 0 || self.buckets <= 0 || ticks_per_cycle <= 0 || self.interval_ticks <= 0
        {
            return None;
        }
        let window = self.buckets * self.interval_ticks;
        Some((self.cycles * ticks_per_cycle * FULL_BPS / window).min(FULL_BPS))
    }

    pub fn stalled(&self) -> bool {
        self.buckets > 0 && self.cycles <= 0
    }
}

/// Parse one `/production` reply, keeping the newest `last` buckets. None for a
/// reply with no recipe (a station the ledger has never charted) or no buckets.
pub fn parse_production(v: &Value, last: usize) -> Option<LineReading> {
    let recipe = v
        .get("recipe")?
        .as_str()
        .filter(|r| !r.is_empty())?
        .to_string();
    let rows = v.get("buckets")?.as_array()?;
    if rows.is_empty() {
        return None;
    }
    let tail = &rows[rows.len().saturating_sub(last.max(1))..];
    let i = |b: &Value, k: &str| b.get(k).and_then(Value::as_i64).unwrap_or(0);
    Some(LineReading {
        recipe,
        interval_ticks: v
            .get("intervalTicks")
            .and_then(Value::as_i64)
            .filter(|t| *t > 0)
            .unwrap_or(PRODUCTION_INTERVAL_TICKS),
        buckets: tail.len() as i64,
        cycles: tail.iter().map(|b| i(b, "cyclesCompleted")).sum(),
        idle_ticks: tail.iter().map(|b| i(b, "idleTicks")).sum(),
        blocked: tail
            .iter()
            .rev()
            .find_map(|b| b.get("blockedReason").and_then(Value::as_str))
            .filter(|r| !r.is_empty())
            .map(String::from),
    })
}

/// The recipes with each measured line's share applied; a line the ledger does
/// not know, or one that stalled, runs at full (the honesty bound, unchanged).
pub fn with_utilization(recipes: &[Recipe], readings: &[LineReading]) -> Vec<Recipe> {
    recipes
        .iter()
        .map(|r| {
            let share = readings
                .iter()
                .find(|m| m.recipe == r.id)
                .and_then(|m| m.utilization_bps(r.ticks_per_cycle))
                .unwrap_or(FULL_BPS);
            Recipe {
                utilization_bps: share,
                ..r.clone()
            }
        })
        .collect()
}

/// One line per measured line for the journal: the share, or the stall.
pub fn measured_lines(recipes: &[Recipe], readings: &[LineReading]) -> Vec<String> {
    let mut out = Vec::new();
    for r in recipes {
        let Some(m) = readings.iter().find(|m| m.recipe == r.id) else {
            continue;
        };
        match m.utilization_bps(r.ticks_per_cycle) {
            Some(bps) if bps < FULL_BPS => out.push(format!(
                "{}@{} {}%{}",
                r.id,
                r.station,
                bps / 100,
                m.blocked
                    .as_deref()
                    .map(|b| format!(" {b}"))
                    .unwrap_or_default()
            )),
            None if m.stalled() => out.push(format!(
                "{}@{} stalled{}",
                r.id,
                r.station,
                m.blocked
                    .as_deref()
                    .map(|b| format!(" {b}"))
                    .unwrap_or_default()
            )),
            _ => {}
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Money is not stock. The exchange quotes `stock`, `capacity` and `equilibrium`
// in the same INVENTORY unit and derives every price from them; a forecast that
// reads the equilibrium count as a meal-credit price manufactures trades out of
// incompatible units (codex T-238 design review, finding 1). So the two live in
// distinct types, and the only way from one to the other is the exchange's own
// formula, ported below from `UCFEngine/Economy/Pricing.swift` integer for
// integer.
// ---------------------------------------------------------------------------

/// Meal credits: a price or a sum of money. Never a count of anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Credits(pub i64);

/// Units on a shelf or in a hold. Never a price.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Units(pub i64);

/// The price register the exchange publishes on `/v1/reference`: each good's
/// `basePrice` and `swingBps`, and each priced berth's `spreadBps`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Pricing {
    /// good → (basePrice ℳ, swingBps)
    pub goods: BTreeMap<String, (i64, i64)>,
    /// station → spreadBps (absent for a berth the caller has not earned)
    pub spread: BTreeMap<String, i64>,
    /// station → dockFee ℳ, paid on every docking: a run's cost beside the fuel.
    pub dock: BTreeMap<String, i64>,
}

pub fn parse_pricing(reference: &Value) -> Pricing {
    let goods = reference
        .get("goods")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|g| {
                    Some((
                        g.get("id")?.as_str()?.to_string(),
                        (
                            g.get("basePrice").and_then(Value::as_i64)?,
                            g.get("swingBps").and_then(Value::as_i64).unwrap_or(0),
                        ),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let stations = reference.get("stations").and_then(Value::as_array);
    let spread = stations
        .map(|rows| {
            rows.iter()
                .filter_map(|st| {
                    Some((
                        st.get("id")?.as_str()?.to_string(),
                        st.get("spreadBps").and_then(Value::as_i64)?,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let dock = stations
        .map(|rows| {
            rows.iter()
                .filter_map(|st| {
                    Some((
                        st.get("id")?.as_str()?.to_string(),
                        st.get("dockFee").and_then(Value::as_i64)?,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    Pricing {
        goods,
        spread,
        dock,
    }
}

/// The exchange's clamp rails on the price multiplier (`Pricing.swift`).
pub const CLAMP_FLOOR_BPS: i64 = 2_000;
pub const CLAMP_CEILING_BPS: i64 = 20_000;
/// A neutral event modifier. Events scale this and we cannot see them from here,
/// so every projection below is "absent events" — and says so.
pub const NEUTRAL_MODIFIER_BPS: i64 = 10_000;

/// `MarketPricing.imbalanceBps`, verbatim: −10000..=+10000, positive when the
/// shelf is short of its equilibrium. `equilibrium <= 0` pins the price at base.
pub fn imbalance_bps(stock: Units, equilibrium: Units) -> i64 {
    if equilibrium.0 <= 0 {
        return 0;
    }
    let hi = stock.0.max(equilibrium.0);
    let lo = stock.0.min(equilibrium.0);
    if hi <= 0 {
        return 10_000;
    }
    let magnitude = (hi - lo) * 10_000 / hi;
    if stock.0 < equilibrium.0 {
        magnitude
    } else {
        -magnitude
    }
}

/// `MarketPricing.midPrice` with a neutral modifier: the mid in ℳ for a shelf
/// holding `stock` against `equilibrium`, for a good of `base` and `swing_bps`.
pub fn mid_price(base: Credits, stock: Units, equilibrium: Units, swing_bps: i64) -> Credits {
    let imbalance = imbalance_bps(stock, equilibrium);
    let swung = 10_000 + swing_bps * imbalance / 10_000;
    let modified = swung * NEUTRAL_MODIFIER_BPS / 10_000;
    let clamped = modified.clamp(CLAMP_FLOOR_BPS, CLAMP_CEILING_BPS);
    Credits((base.0 * clamped / 10_000).max(1))
}

/// `MarketPricing.sellUnitPrice`: what the station pays per unit. Rounds down.
pub fn sell_unit_price(mid: Credits, spread_bps: i64) -> Credits {
    Credits((mid.0 * (10_000 - spread_bps) / 10_000).max(0))
}

/// `MarketPricing.buyUnitPrice`: what the station charges per unit. Rounds up.
pub fn buy_unit_price(mid: Credits, spread_bps: i64) -> Credits {
    Credits(((mid.0 * (10_000 + spread_bps) + 9_999) / 10_000).max(1))
}

impl Flow {
    /// The shelf `ticks` from now, along the schedule: an eaten shelf drains to
    /// empty, a made shelf fills to capacity, and neither goes past. None
    /// without a live shelf.
    pub fn stock_at(&self, ticks: i64) -> Option<Units> {
        let s = self.shelf.as_ref()?;
        let moved = self.moved_milli(ticks.max(0)) / 1000;
        Some(Units(match self.kind {
            FlowKind::Eats => (s.stock - moved).max(0),
            FlowKind::Makes => (s.stock + moved).min(s.capacity.max(s.stock)),
        }))
    }

    /// The expected multiplier on the full rate at `t` ticks from now, bps:
    /// every window covering `t` scales it in turn (two holds on one line
    /// compound; a hold under a rush is a hold at a rush's fraction).
    fn rate_bps_at(&self, t: i64) -> i64 {
        self.windows
            .iter()
            .filter(|w| w.from <= t && t < w.to)
            .fold(FULL_BPS, |bps, w| bps * w.rate_bps.max(0) / FULL_BPS)
    }

    /// The segment edges of the schedule inside `[0, until)`, in order, so the
    /// rate is constant between neighbours.
    fn edges(&self, until: i64) -> Vec<i64> {
        let mut edges = vec![0];
        for w in &self.windows {
            for e in [w.from, w.to] {
                if e > 0 && e < until {
                    edges.push(e);
                }
            }
        }
        edges.push(until);
        edges.sort_unstable();
        edges.dedup();
        edges
    }

    /// Units moved in the next `ticks`, in thousandths: the rate per kilotick
    /// IS milli-units per tick, so this is Σ rate × length over the schedule's
    /// segments — and exactly `rate × ticks` when nothing is scheduled.
    fn moved_milli(&self, ticks: i64) -> i64 {
        let rate = self.rate_per_kilotick.max(0);
        if ticks <= 0 || rate == 0 {
            return 0;
        }
        let edges = self.edges(ticks);
        edges
            .windows(2)
            .map(|seg| rate * self.rate_bps_at(seg[0]) / FULL_BPS * (seg[1] - seg[0]))
            .sum()
    }

    /// Ticks until the shelf is empty (Eats) or full (Makes) along the schedule:
    /// the first segment whose cumulative movement reaches the room, and the
    /// tick inside it. Floors like the unscheduled formula, so the two agree
    /// whenever no window is laid.
    fn walk_horizon(&self) -> Option<i64> {
        let s = self.shelf.as_ref()?;
        let rate = self.rate_per_kilotick.max(0);
        if rate == 0 {
            return None;
        }
        let room = match self.kind {
            FlowKind::Eats => s.stock,
            FlowKind::Makes => (s.capacity - s.stock).max(0),
        } * 1000;
        let far = self.windows.iter().map(|w| w.to).max().unwrap_or(0).max(1);
        let mut edges = self.edges(far);
        edges.push(i64::MAX);
        let mut moved = 0i64;
        for seg in edges.windows(2) {
            let seg_rate = rate * self.rate_bps_at(seg[0]) / FULL_BPS;
            let len = seg[1].saturating_sub(seg[0]);
            if seg_rate > 0 && (seg[1] == i64::MAX || moved + seg_rate * len >= room) {
                return Some(seg[0] + (room - moved).max(0) / seg_rate);
            }
            moved += seg_rate * len;
        }
        None
    }
}

#[cfg(test)]
mod pricing_tests {
    use super::*;

    /// The numbers the exchange's own formula gives, so a reviewer can check
    /// them by hand: base 30, swing 9000, equilibrium 600.
    #[test]
    fn the_mid_is_the_exchange_formula_and_not_the_equilibrium_count() {
        let eq = Units(600);
        // 500 on a 600 shelf: imbalance 1666 → ×1.1499 → 34. Nowhere near 600.
        assert_eq!(imbalance_bps(Units(500), eq), 1_666);
        assert_eq!(mid_price(Credits(30), Units(500), eq, 9_000), Credits(34));
        // Empty shelf: imbalance saturates at 10000 → ×1.9 → 57.
        assert_eq!(imbalance_bps(Units(0), eq), 10_000);
        assert_eq!(mid_price(Credits(30), Units(0), eq, 9_000), Credits(57));
        // Glut: 900 on a 600 shelf → −3333 → ×0.7 → 21.
        assert_eq!(imbalance_bps(Units(900), eq), -3_333);
        assert_eq!(mid_price(Credits(30), Units(900), eq, 9_000), Credits(21));
        // No equilibrium pins the price at base whatever the stock does.
        assert_eq!(
            mid_price(Credits(30), Units(0), Units(0), 9_000),
            Credits(30)
        );
        // The rails: swing 20000 on an empty shelf would be ×3, clamped to ×2.
        assert_eq!(mid_price(Credits(30), Units(0), eq, 20_000), Credits(60));
        // Spread: the house rounds toward itself both ways.
        assert_eq!(sell_unit_price(Credits(34), 500), Credits(32));
        assert_eq!(buy_unit_price(Credits(34), 500), Credits(36));
    }

    #[test]
    fn a_shelf_projects_along_its_flow_and_stops_at_the_rails() {
        let shelf = Shelf {
            station: "works".into(),
            good: "brine".into(),
            stock: 500,
            capacity: 1_000,
            equilibrium: 600,
        };
        let eats = Flow {
            station: "works".into(),
            good: "brine".into(),
            kind: FlowKind::Eats,
            rate_per_kilotick: 13_333,
            shelf: Some(shelf.clone()),
            horizon_ticks: Some(37),
            windows: Vec::new(),
        };
        assert_eq!(eats.stock_at(0), Some(Units(500)));
        assert_eq!(
            eats.stock_at(24),
            Some(Units(181)),
            "13,333 a kilotick for 24 ticks is 319"
        );
        assert_eq!(
            eats.stock_at(1_000),
            Some(Units(0)),
            "an eaten shelf stops at empty"
        );
        let makes = Flow {
            kind: FlowKind::Makes,
            ..eats.clone()
        };
        assert_eq!(makes.stock_at(24), Some(Units(819)));
        assert_eq!(
            makes.stock_at(1_000),
            Some(Units(1_000)),
            "a made shelf stops at capacity"
        );
        assert_eq!(
            Flow {
                shelf: None,
                ..eats
            }
            .stock_at(24),
            None
        );
    }
}
