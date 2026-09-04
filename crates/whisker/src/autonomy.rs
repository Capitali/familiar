//! The autonomy dial, pure: how much of each control surface the captain has handed
//! to the ship's computer (Ian, 2026-09-03: "There should be a message window giving
//! advice, and the captain can choose to follow or not. there should also be the
//! ability to just allow auto a captain authorized command and the ship will
//! Proceede as KKII is now. Should have ability to granularity config autonomy for
//! each control surface category and family.").
//!
//! Every doctrine decision has one of three fates, chosen per control surface:
//! **advise** (say what it would do, do nothing), **confirm** (propose, act only on the
//! captain's yes within a fold), **auto** (act and journal). The dial is `autonomy.json`
//! in the ship store: `{"navigation.course": "auto", "market": "confirm", "*": "auto"}` —
//! a category, a family, or `*`, most specific wins. Absent means auto for everything
//! the captain has bought (KK II today). The grant model stands beneath it: an
//! automation not bought is not on the dial at all.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Advise,
    Confirm,
    Auto,
}

impl Level {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "advise" | "advice" | "advisory" => Some(Self::Advise),
            "confirm" | "ask" => Some(Self::Confirm),
            "auto" | "automatic" | "autonomous" => Some(Self::Auto),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Level::Advise => "advise",
            Level::Confirm => "confirm",
            Level::Auto => "auto",
        }
    }
}

/// A control surface: `family.category`. The families and categories the pilot
/// exercises today, plus racing's for when it lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Surface {
    NavigationCourse,
    NavigationFuel,
    NavigationRescue,
    FreightBook,
    FreightCollect,
    FreightCancel,
    MarketBuy,
    MarketSell,
    MarketCarry,
    ShipRepair,
    ShipRefit,
    ShipCrew,
    ShipFrame,
    ShipLease,
    RacingPlot,
    RacingLine,
    RacingRefusal,
}

impl Surface {
    pub fn family(self) -> &'static str {
        match self {
            Surface::NavigationCourse | Surface::NavigationFuel | Surface::NavigationRescue => {
                "navigation"
            }
            Surface::FreightBook | Surface::FreightCollect | Surface::FreightCancel => "freight",
            Surface::MarketBuy | Surface::MarketSell | Surface::MarketCarry => "market",
            Surface::ShipRepair
            | Surface::ShipRefit
            | Surface::ShipCrew
            | Surface::ShipFrame
            | Surface::ShipLease => "ship",
            Surface::RacingPlot | Surface::RacingLine | Surface::RacingRefusal => "racing",
        }
    }
    pub fn category(self) -> &'static str {
        match self {
            Surface::NavigationCourse => "course",
            Surface::NavigationFuel => "fuel",
            Surface::NavigationRescue => "rescue",
            Surface::FreightBook => "book",
            Surface::FreightCollect => "collect",
            Surface::FreightCancel => "cancel",
            Surface::MarketBuy => "buy",
            Surface::MarketSell => "sell",
            Surface::MarketCarry => "carry",
            Surface::ShipRepair => "repair",
            Surface::ShipRefit => "refit",
            Surface::ShipCrew => "crew",
            Surface::ShipFrame => "frame",
            Surface::ShipLease => "lease",
            Surface::RacingPlot => "plot",
            Surface::RacingLine => "line",
            Surface::RacingRefusal => "refusal",
        }
    }
    pub fn key(self) -> String {
        format!("{}.{}", self.family(), self.category())
    }
    pub fn all() -> &'static [Surface] {
        &[
            Surface::NavigationCourse,
            Surface::NavigationFuel,
            Surface::NavigationRescue,
            Surface::FreightBook,
            Surface::FreightCollect,
            Surface::FreightCancel,
            Surface::MarketBuy,
            Surface::MarketSell,
            Surface::MarketCarry,
            Surface::ShipRepair,
            Surface::ShipRefit,
            Surface::ShipCrew,
            Surface::ShipFrame,
            Surface::ShipLease,
            Surface::RacingPlot,
            Surface::RacingLine,
            Surface::RacingRefusal,
        ]
    }
    pub fn parse(s: &str) -> Option<Surface> {
        Surface::all().iter().copied().find(|x| x.key() == s.trim())
    }
}

/// The dial as the store holds it: keys are `family.category`, `family`, or `*`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dial {
    #[serde(flatten)]
    pub settings: BTreeMap<String, Level>,
}

pub const DIAL_FILE: &str = "autonomy.json";

impl Dial {
    pub fn load(ship_dir: &Path) -> Dial {
        std::fs::read_to_string(ship_dir.join(DIAL_FILE))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, ship_dir: &Path) -> std::io::Result<()> {
        std::fs::write(
            ship_dir.join(DIAL_FILE),
            serde_json::to_vec_pretty(self).unwrap_or_default(),
        )
    }
    /// Most specific setting wins: category, then family, then `*`, then auto. The
    /// one surface with its own floor is the tanker on a real-time world — `rescue`
    /// defaults to ADVISE unless the captain says otherwise, because a PAWS call is
    /// a multi-day strand that also pins the hull (Ian's own Kibble Klipper, 2026-09).
    pub fn level(&self, s: Surface) -> Level {
        self.settings
            .get(&s.key())
            .or_else(|| self.settings.get(s.family()))
            .or_else(|| self.settings.get("*"))
            .copied()
            .unwrap_or(if s == Surface::NavigationRescue {
                Level::Advise
            } else {
                Level::Auto
            })
    }
    pub fn set(&mut self, key: &str, level: Level) -> Result<(), String> {
        let k = key.trim();
        let known = k == "*"
            || ["navigation", "freight", "market", "ship", "racing"].contains(&k)
            || Surface::parse(k).is_some();
        if !known {
            return Err(format!("unknown control surface `{k}`"));
        }
        self.settings.insert(k.to_string(), level);
        Ok(())
    }
}

/// A proposed act awaiting the captain (`proposals.jsonl`), and the captain's word
/// on it (`approvals.jsonl`: `{"id": …, "approved": true|false, "at": …}`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    pub tick: i64,
    /// The last tick this proposal may be acted on; after it, it lapses.
    pub expires_tick: i64,
    pub surface: String,
    pub describe: String,
    pub why: String,
    pub body: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approval {
    pub id: String,
    pub approved: bool,
    pub at: i64,
}

/// What the gate says about acting on a decision now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gate {
    /// Act, and journal it.
    Act,
    /// Say what you would do; do not act.
    Advise,
    /// Proposed and waiting; act on a later fold if approved.
    Proposed,
    /// Proposed earlier, lapsed unapproved.
    Lapsed,
}

/// A proposal is identified by its surface and body, so the same intent on the next
/// fold finds its own earlier proposal rather than filing another.
pub fn proposal_id(surface: Surface, body: &serde_json::Value) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in format!("{}|{}", surface.key(), body).bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("p-{h:016x}")
}

/// The gate: given the dial, the surface, the tick, the fold length for the TTL, the
/// proposals already on file and the approvals, decide this fold's fate for the act.
/// `now_proposal` is filled in when a new proposal must be written.
#[allow(clippy::too_many_arguments)]
pub fn gate(
    dial: &Dial,
    surface: Surface,
    tick: i64,
    body: &serde_json::Value,
    describe: &str,
    why: &str,
    proposals: &[Proposal],
    approvals: &[Approval],
    new_proposal: &mut Option<Proposal>,
) -> Gate {
    match dial.level(surface) {
        Level::Auto => Gate::Act,
        Level::Advise => Gate::Advise,
        Level::Confirm => {
            let id = proposal_id(surface, body);
            if let Some(a) = approvals.iter().rev().find(|a| a.id == id) {
                return if a.approved { Gate::Act } else { Gate::Lapsed };
            }
            match proposals.iter().rev().find(|p| p.id == id) {
                Some(p) if tick <= p.expires_tick => Gate::Proposed,
                Some(_) => Gate::Lapsed,
                None => {
                    *new_proposal = Some(Proposal {
                        id,
                        tick,
                        // One fold of grace beyond the next: enough for a captain
                        // watching, not enough for a stale plan to fire later.
                        expires_tick: tick + PROPOSAL_TTL_TICKS,
                        surface: surface.key(),
                        describe: describe.to_string(),
                        why: why.to_string(),
                        body: body.clone(),
                    });
                    Gate::Proposed
                }
            }
        }
    }
}

/// How long a proposal waits for the captain before it lapses, in ticks.
pub const PROPOSAL_TTL_TICKS: i64 = 4;

pub fn load_proposals(ship_dir: &Path) -> Vec<Proposal> {
    std::fs::read_to_string(ship_dir.join("proposals.jsonl"))
        .map(|t| {
            t.lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        })
        .unwrap_or_default()
}

pub fn load_approvals(ship_dir: &Path) -> Vec<Approval> {
    std::fs::read_to_string(ship_dir.join("approvals.jsonl"))
        .map(|t| {
            t.lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        })
        .unwrap_or_default()
}

pub fn append_proposal(ship_dir: &Path, p: &Proposal) {
    use std::io::Write;
    if let Ok(line) = serde_json::to_string(p) {
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(ship_dir.join("proposals.jsonl"))
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

pub fn append_approval(ship_dir: &Path, a: &Approval) {
    use std::io::Write;
    if let Ok(line) = serde_json::to_string(a) {
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(ship_dir.join("approvals.jsonl"))
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn absent_dial_is_auto_for_everything_but_the_tanker() {
        let d = Dial::default();
        assert_eq!(d.level(Surface::FreightBook), Level::Auto);
        assert_eq!(d.level(Surface::MarketBuy), Level::Auto);
        assert_eq!(d.level(Surface::NavigationRescue), Level::Advise);
    }

    #[test]
    fn most_specific_setting_wins() {
        let mut d = Dial::default();
        d.set("*", Level::Advise).unwrap();
        d.set("market", Level::Confirm).unwrap();
        d.set("market.sell", Level::Auto).unwrap();
        assert_eq!(d.level(Surface::FreightBook), Level::Advise);
        assert_eq!(d.level(Surface::MarketBuy), Level::Confirm);
        assert_eq!(d.level(Surface::MarketSell), Level::Auto);
        assert!(d.set("kitchen.sink", Level::Auto).is_err());
        let json = serde_json::to_string(&d).unwrap();
        let back: Dial = serde_json::from_str(&json).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn confirm_proposes_once_acts_on_approval_and_lapses_unapproved() {
        let mut d = Dial::default();
        d.set("freight.book", Level::Confirm).unwrap();
        let body = json!({"type": "book", "loadId": "L1"});
        let mut fresh = None;
        let g = gate(
            &d,
            Surface::FreightBook,
            100,
            &body,
            "book L1",
            "best rate",
            &[],
            &[],
            &mut fresh,
        );
        assert_eq!(g, Gate::Proposed);
        let p = fresh.clone().expect("a proposal was written");
        assert_eq!(p.expires_tick, 100 + PROPOSAL_TTL_TICKS);
        // Next fold, still waiting: the same proposal, not a second one.
        let mut again = None;
        let g = gate(
            &d,
            Surface::FreightBook,
            101,
            &body,
            "book L1",
            "best rate",
            std::slice::from_ref(&p),
            &[],
            &mut again,
        );
        assert_eq!(g, Gate::Proposed);
        assert!(again.is_none());
        // The captain says yes.
        let yes = Approval {
            id: p.id.clone(),
            approved: true,
            at: 1,
        };
        let g = gate(
            &d,
            Surface::FreightBook,
            102,
            &body,
            "book L1",
            "best rate",
            std::slice::from_ref(&p),
            &[yes],
            &mut None,
        );
        assert_eq!(g, Gate::Act);
        // Or says nothing and the fold passes.
        let g = gate(
            &d,
            Surface::FreightBook,
            100 + PROPOSAL_TTL_TICKS + 1,
            &body,
            "book L1",
            "best rate",
            std::slice::from_ref(&p),
            &[],
            &mut None,
        );
        assert_eq!(g, Gate::Lapsed);
        // Or says no.
        let no = Approval {
            id: p.id.clone(),
            approved: false,
            at: 2,
        };
        let g = gate(
            &d,
            Surface::FreightBook,
            102,
            &body,
            "book L1",
            "best rate",
            &[p],
            &[no],
            &mut None,
        );
        assert_eq!(g, Gate::Lapsed);
    }

    #[test]
    fn advise_never_acts_and_auto_always_does() {
        let mut d = Dial::default();
        d.set("navigation", Level::Advise).unwrap();
        let body = json!({"type": "travel", "station": "foxys-diner"});
        assert_eq!(
            gate(
                &d,
                Surface::NavigationCourse,
                1,
                &body,
                "",
                "",
                &[],
                &[],
                &mut None
            ),
            Gate::Advise
        );
        assert_eq!(
            gate(
                &d,
                Surface::ShipRepair,
                1,
                &json!({"type":"repair"}),
                "",
                "",
                &[],
                &[],
                &mut None
            ),
            Gate::Act
        );
    }
}
