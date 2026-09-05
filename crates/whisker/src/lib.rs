//! **whisker** — the ship world's pilot (ADR-0045, T-205 step 5; the doctrine learned
//! live on 2026-08-31 and recorded in the household's memory as "ship-learning").
//!
//! A cat judges every clearance by its whiskers before committing its body. This crate
//! is that judgment for a United Cat Foods hull: a PURE decision doctrine (`doctrine`)
//! that owns no socket, wrapped by a runner (`src/main.rs`) that reads the SHIP's own
//! store and nothing else.
//!
//! The partition is the whole point (ADR-0045): everything here — key, journal, lease,
//! learned prices — lives in the ship's data dir. The household appears only as the
//! ISSUER of the lease the runner verifies before any consequential act, via the public
//! identity the commissioning ceremony wrote into the ship store. No household record is
//! ever read, and no game fact ever leaves except as a payload-free attention notice.
//!
//! **Automation is a named, sellable capability** (Ian, 2026-08-31: driving a ship by
//! familiar is pay-per-feature — a captain buys automations the way they hire crew or
//! add refrigeration). The doctrine therefore tags every decision with the [`Automation`]
//! it exercises, and the runner refuses a decision whose automation the ship does not
//! hold. Today only [`Automation::Freight`] is implemented; the rest of the enum is the
//! roadmap (united-cat-foods-metal#61), present so the gate exists before the features.

use std::collections::BTreeSet;

pub mod autonomy;
pub mod chain;
pub mod doctrine;
pub mod outfit;
pub mod store;
pub mod trade;

/// One purchasable unit of ship automation. The names mirror the co-pilot-key scopes
/// proposed to the exchange (ucf-exchange#15): a paid entitlement on the captain's
/// account becomes exactly one of these in the ship store's `automations.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Automation {
    /// The freight career loop: book, fly, load, deliver, collect — and the fuel
    /// management without which no hull hauls (pump top-up, pump diversion, the PAWS
    /// tanker as the expensive floor).
    Freight,
    /// The merchant loop: observe prices across the map, buy a good where it is cheap,
    /// carry it, sell it where it is dear (Ian, 2026-09-01 — "trade as well as haul").
    Trade,
    /// Outfitting: fittings (drive-tune, hold-extension, refrigeration on evidence)
    /// and, after title, crew — bought out of earnings above a reserve (Ian,
    /// 2026-09-02: "expansion capabilities that should be managed as well").
    Outfit,
    /// Cargo loading order (metal#61 §1) — not yet implemented.
    LoadingOrder,
    /// Inertial dampening compensation (metal#61 §2) — not yet implemented.
    Dampening,
    /// Cargo-driven refrigeration (metal#61 §5) — not yet implemented.
    Reefer,
    /// Hazardous cargo manifests (metal#61 §6) — not yet implemented.
    Hazmat,
    /// Watchkeeping / monitoring requirements (metal#61 §7) — not yet implemented.
    Watchkeeping,
}

impl Automation {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim() {
            "freight" => Self::Freight,
            "trade" => Self::Trade,
            "outfit" => Self::Outfit,
            "loading-order" => Self::LoadingOrder,
            "dampening" => Self::Dampening,
            "reefer" => Self::Reefer,
            "hazmat" => Self::Hazmat,
            "watchkeeping" => Self::Watchkeeping,
            _ => return None,
        })
    }
}

/// The automations a ship holds, from the text of `automations.json` — a JSON array
/// of scope names. Empty or unparseable text and unknown names all narrow: what
/// cannot be read grants nothing (unknown names are reported so a typo is loud).
/// The file is the runner's (`store::granted_automations`); this crate owns none.
pub fn parse_automations(raw: &str) -> (BTreeSet<Automation>, Vec<String>) {
    let mut granted = BTreeSet::new();
    let mut unknown = Vec::new();
    let names: Vec<String> = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return (granted, unknown),
    };
    for n in names {
        match Automation::parse(&n) {
            Some(a) => {
                granted.insert(a);
            }
            None => unknown.push(n),
        }
    }
    (granted, unknown)
}
