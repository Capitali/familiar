//! The outfitting doctrine, pure: what to buy for the hull, and when (Ian,
//! 2026-09-02: "The loaf has expansion capabilities that should be managed as
//! well. Hold expansion, refrigeration, engine tuning, as well as crew.").
//!
//! The exchange sells three fittings, one of each ever, at any berth (`refit`):
//! `drive-tune` (+15% rated thrust), `hold-extension` (+40 hold), `refrigeration`
//! (in-transit decay of the pilot's own contracted freight halved). Crew are hired
//! by station; the engine relieves wear, the hold speeds handling, the galley
//! props morale; the bridge has no rules in the engine yet. Frames and the lease
//! buyout are the captain's business decisions, not the pilot's.
//!
//! The rules, and the reasoning:
//! - **Never spend the reserve.** A purchase must leave enough cash for
//!   [`RESERVE_DAYS`] of the ship's fixed daily charges (mortgage payment and lease
//!   service) plus a tank of fuel — the charges are swept whether or not she earns.
//! - **Refrigeration only on evidence.** Deliveries pay a fixed company share
//!   whatever the cargo (the 85% KK II sees is not decay). Refrigeration is bought
//!   when the ship's OWN delivery record shows perishable loads paying a smaller
//!   share than durable ones by a margin — the fitting halves exactly that loss.
//! - **Drive-tune before hold-extension.** Thrust shortens every leg (time scales
//!   with the square root of acceleration) and widens the pickup window; hold
//!   only matters when loads or positions are hold-bound.
//! - **Crew after title.** On a leased hull the yard repairs wear for nothing, so
//!   an engineer's job is already done for free; wages are a recurring cost.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Days of fixed charges kept in hand after any purchase.
pub const RESERVE_DAYS: i64 = 3;
/// A perishable load paying this much less of its rate than durable ones do is
/// decay worth halving (3 percentage points of pay).
const DECAY_EVIDENCE_BPS: i64 = 300;
/// With no durable baseline yet, this shortfall alone is evidence (the company
/// share is ~15%; 18% and worse means something else is eating the pay).
const DECAY_ABSOLUTE_BPS: i64 = 1800;
/// Fewer perishable deliveries than this is not evidence, it is an anecdote.
const MIN_PERISHABLE_SAMPLES: usize = 3;

/// The fittings the exchange sells, in the order this doctrine buys them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fitting {
    Refrigeration,
    DriveTune,
    HoldExtension,
}

impl Fitting {
    /// The wire name (`refit {fitting}`), also how `/v1/me.fittings` lists it.
    pub fn wire(self) -> &'static str {
        match self {
            Fitting::Refrigeration => "refrigeration",
            Fitting::DriveTune => "drive-tune",
            Fitting::HoldExtension => "hold-extension",
        }
    }
    /// The param name the world publishes this fitting's price under.
    pub fn price_param(self) -> &'static str {
        match self {
            Fitting::Refrigeration => "refitCostRefrigeration",
            Fitting::DriveTune => "refitCostDriveTune",
            Fitting::HoldExtension => "refitCostHoldExtension",
        }
    }
    /// The shipped pack's price, used only when the world does not publish one.
    ///
    /// These used to be the ONLY source, copied out of params.json — "a client keeps
    /// a private copy of the pack and is silently wrong the day a dial or a crossing
    /// moves it" (ucf-exchange#22, which asked for them and got them on 2026-09-07).
    /// They are a fallback now, and the world's own figure wins.
    pub fn shipped_price(self) -> i64 {
        match self {
            Fitting::Refrigeration => 4_500,
            Fitting::DriveTune => 9_000,
            Fitting::HoldExtension => 6_000,
        }
    }
}

/// One settled delivery, as the ship store remembers it (`deliveries.jsonl`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryStat {
    pub load_id: String,
    pub good: String,
    pub perishable: bool,
    /// The rate booked (`freightRate`) and what the fold actually paid.
    pub booked: i64,
    pub paid: i64,
}

/// What the purse and the hull look like this fold.
#[derive(Debug, Clone, Default)]
pub struct Purse {
    pub credits: i64,
    /// What the hull still owes. Ian, 2026-09-07: "always prioritize removing debt
    /// and clearing leases."
    ///
    /// The two are ONE act in this world, which is why this is a single field and
    /// not two: the engine transfers title the moment the balance reaches zero
    /// (`ShipTitle.settleTitleIfCleared` — "a pilot who never presses the button
    /// still gets the ceremony, on the morning the last payment lands"). So there is
    /// no separate lease to clear. There is a debt, and clearing it IS the ceremony.
    pub debt: i64,
    /// Mortgage payment + lease service per day, as observed or estimated.
    pub daily_fixed_cost: i64,
    /// A full tank's price, kept in hand too.
    pub tank_price: i64,
    pub titled: bool,
    /// `/v1/me.fittings`, wire names.
    pub fittings: Vec<String>,
    /// What the WORLD says each fitting costs, by param name. Empty falls back to
    /// the shipped pack — see [`Fitting::shipped_price`].
    pub refit_prices: BTreeMap<String, i64>,
}

impl Purse {
    /// The world's price for a fitting, or the shipped pack's when it publishes none.
    pub fn price_of(&self, f: Fitting) -> i64 {
        self.refit_prices
            .get(f.price_param())
            .copied()
            .filter(|p| *p > 0)
            .unwrap_or_else(|| f.shipped_price())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutfitDecision {
    Idle {
        why: String,
    },
    Refit {
        fitting: Fitting,
        price: i64,
    },
    /// Put credits against the balance. Clearing it transfers the title, so this is
    /// the act that turns a leased hull into the captain's own.
    PayLease {
        amount: i64,
    },
}

/// Cash that must remain after any purchase.
pub fn reserve(p: &Purse) -> i64 {
    RESERVE_DAYS * p.daily_fixed_cost.max(0) + p.tank_price.max(0)
}

/// Pay shortfall in bps of the booked rate, averaged over a set of deliveries.
fn shortfall_bps(stats: &[&DeliveryStat]) -> Option<i64> {
    let (mut booked, mut paid) = (0i64, 0i64);
    for s in stats {
        if s.booked > 0 {
            booked += s.booked;
            paid += s.paid;
        }
    }
    if booked <= 0 {
        return None;
    }
    Some(10_000 - paid * 10_000 / booked)
}

/// Does the ship's own record say perishables are losing pay to decay?
pub fn decay_evidence(stats: &[DeliveryStat]) -> Option<String> {
    let perishable: Vec<&DeliveryStat> = stats.iter().filter(|s| s.perishable).collect();
    let durable: Vec<&DeliveryStat> = stats.iter().filter(|s| !s.perishable).collect();
    if perishable.len() < MIN_PERISHABLE_SAMPLES {
        return None;
    }
    let p = shortfall_bps(&perishable)?;
    match shortfall_bps(&durable) {
        Some(d) if p - d >= DECAY_EVIDENCE_BPS => Some(format!(
            "perishable loads pay {:.1}% short vs {:.1}% for durable ({} vs {} deliveries)",
            p as f64 / 100.0,
            d as f64 / 100.0,
            perishable.len(),
            durable.len()
        )),
        None if p >= DECAY_ABSOLUTE_BPS => Some(format!(
            "perishable loads pay {:.1}% short over {} deliveries",
            p as f64 / 100.0,
            perishable.len()
        )),
        _ => None,
    }
}

/// The judgment: the next fitting worth buying that the purse can bear.
pub fn decide_outfit(p: &Purse, stats: &[DeliveryStat]) -> OutfitDecision {
    // DEBT OUTRANKS EVERY FITTING (Ian, 2026-09-07: "always prioritize removing debt
    // and clearing leases"). A refit is the most discretionary thing this hull can
    // buy: it is worth having, it is never worth having FIRST, and money spent on it
    // is money not spent on the balance that decides whether the ship is the
    // captain's at all.
    //
    // This is not a rule about thrift. Under this world's own arithmetic it is the
    // higher-returning trade: the balance carries a daily service charge, the title
    // transfers the moment it clears, and a titled hull stops paying the charge and
    // repairs for free. The fitting will still be for sale afterwards.
    if p.debt > 0 {
        // ...and where there is cash beyond the reserve, PAY it rather than merely
        // decline to spend it. Refusing the fitting alone would let credits pile up
        // beside a balance that is charging service every day.
        //
        // The reserve still stands: a hull that pays itself down to nothing cannot
        // fuel, and a stranded hull earns nothing to pay with. Debt first, but never
        // ahead of the ability to fly.
        let spare = p.credits - reserve(p);
        if spare > 0 {
            return OutfitDecision::PayLease {
                amount: spare.min(p.debt),
            };
        }
        return OutfitDecision::Idle {
            why: format!(
                "ℳ{} still owed and nothing spare over the reserve — the balance comes \
                 first, and the title lands the morning it clears",
                p.debt
            ),
        };
    }
    let has = |f: Fitting| p.fittings.iter().any(|x| x == f.wire());
    let keep = reserve(p);
    let mut wanted: Vec<(Fitting, String)> = Vec::new();
    if !has(Fitting::Refrigeration) {
        if let Some(why) = decay_evidence(stats) {
            wanted.push((Fitting::Refrigeration, why));
        }
    }
    if !has(Fitting::DriveTune) {
        wanted.push((
            Fitting::DriveTune,
            "+15% thrust: shorter legs, wider pickup windows".into(),
        ));
    }
    if !has(Fitting::HoldExtension) {
        wanted.push((Fitting::HoldExtension, "+40 hold".into()));
    }
    let Some((fitting, _why)) = wanted.first().cloned() else {
        return OutfitDecision::Idle {
            why: if p.titled {
                "fitted out; crew hiring is the next rung (engine first)".into()
            } else {
                "fitted out; crew waits for title (repairs are free on the lease)".into()
            },
        };
    };
    let price = p.price_of(fitting);
    if p.credits - price < keep {
        return OutfitDecision::Idle {
            why: format!(
                "saving for {}: ℳ{} + reserve ℳ{} > ℳ{} in hand",
                fitting.wire(),
                price,
                keep,
                p.credits
            ),
        };
    }
    OutfitDecision::Refit { fitting, price }
}

#[cfg(test)]
mod tests {

    /// The WORLD prices the yard, not our copy of the pack. ucf-exchange#22 asked
    /// for these and Jeff published them on 2026-09-07; a client carrying private
    /// copies is silently wrong the day a dial moves.
    #[test]
    fn the_worlds_price_beats_the_shipped_one() {
        let mut p = purse(50_000, &[]);
        assert_eq!(
            p.price_of(Fitting::DriveTune),
            9_000,
            "the shipped fallback"
        );
        p.refit_prices.insert("refitCostDriveTune".into(), 12_500);
        assert_eq!(p.price_of(Fitting::DriveTune), 12_500, "the world wins");
        // A published zero is not a price; the pack still answers.
        p.refit_prices.insert("refitCostDriveTune".into(), 0);
        assert_eq!(p.price_of(Fitting::DriveTune), 9_000);
    }

    /// Ian, 2026-09-07: "always prioritize removing debt and clearing leases."
    ///
    /// Kibble Klipper's own case: ℳ121,317 owed, and the outfit doctrine happily
    /// saving toward a ℳ9,000 drive-tune beside it.
    #[test]
    fn debt_outranks_every_fitting() {
        let mut p = purse(20_000, &[]);
        p.debt = 121_317;
        match decide_outfit(&p, &[]) {
            OutfitDecision::PayLease { amount } => {
                assert_eq!(amount, 20_000 - reserve(&p), "everything over the reserve");
            }
            other => panic!("the balance comes first, got {other:?}"),
        }
        // Cleared, and the fittings are back on the table.
        p.debt = 0;
        assert!(matches!(
            decide_outfit(&p, &[]),
            OutfitDecision::Refit { .. }
        ));
    }

    /// The reserve still stands. A hull that pays itself down to nothing cannot
    /// fuel, and a stranded hull earns nothing to pay with.
    #[test]
    fn the_balance_never_takes_the_reserve() {
        let mut p = purse(500, &[]);
        p.debt = 50_000;
        match decide_outfit(&p, &[]) {
            OutfitDecision::Idle { why } => assert!(why.contains("50000"), "{why}"),
            other => panic!("nothing spare over the reserve, got {other:?}"),
        }
    }

    /// Never more than is owed, however flush the purse.
    #[test]
    fn the_balance_is_never_overpaid() {
        let mut p = purse(80_000, &[]);
        p.debt = 300;
        match decide_outfit(&p, &[]) {
            OutfitDecision::PayLease { amount } => assert_eq!(amount, 300),
            other => panic!("expected a 300 payment, got {other:?}"),
        }
    }
    use super::*;

    fn purse(credits: i64, fittings: &[&str]) -> Purse {
        Purse {
            credits,
            debt: 0,
            refit_prices: BTreeMap::new(),
            daily_fixed_cost: 1_200,
            tank_price: 1_200,
            titled: false,
            fittings: fittings.iter().map(|s| s.to_string()).collect(),
        }
    }
    fn stat(good: &str, perishable: bool, booked: i64, paid: i64) -> DeliveryStat {
        DeliveryStat {
            load_id: "L".into(),
            good: good.into(),
            perishable,
            booked,
            paid,
        }
    }

    #[test]
    fn the_reserve_is_days_of_fixed_charges_plus_a_tank() {
        assert_eq!(reserve(&purse(0, &[])), 3 * 1_200 + 1_200);
    }

    #[test]
    fn drive_tune_first_when_nothing_says_decay_and_the_purse_can_bear_it() {
        // KK II, 2026-09-02: 11,182 in hand, reserve 4,800, drive-tune 9,000 → short.
        let d = decide_outfit(&purse(11_182, &[]), &[]);
        assert!(
            matches!(d, OutfitDecision::Idle { ref why } if why.contains("saving for drive-tune")),
            "{d:?}"
        );
        // 14,000 in hand: bought.
        let d = decide_outfit(&purse(14_000, &[]), &[]);
        assert_eq!(
            d,
            OutfitDecision::Refit {
                fitting: Fitting::DriveTune,
                price: 9_000
            }
        );
        // Then the hold.
        let d = decide_outfit(&purse(14_000, &["drive-tune"]), &[]);
        assert_eq!(
            d,
            OutfitDecision::Refit {
                fitting: Fitting::HoldExtension,
                price: 6_000
            }
        );
        // Fitted out on a lease: crew waits for title.
        let d = decide_outfit(&purse(14_000, &["drive-tune", "hold-extension"]), &[]);
        assert!(
            matches!(d, OutfitDecision::Idle { ref why } if why.contains("waits for title")),
            "{d:?}"
        );
    }

    #[test]
    fn refrigeration_only_on_the_ships_own_evidence_of_decay() {
        // The company share: everything pays 85%. Three perishables at 85% and
        // two durables at 85% is no evidence; drive-tune stays first.
        let even = vec![
            stat("kibble-loaf", true, 751, 639),
            stat("kibble-loaf", true, 755, 642),
            stat("salmon-mousse", true, 1541, 1310),
            stat("ore", false, 1000, 850),
            stat("tinplate", false, 900, 765),
        ];
        assert!(decay_evidence(&even).is_none());
        let d = decide_outfit(&purse(20_000, &[]), &even);
        assert_eq!(
            d,
            OutfitDecision::Refit {
                fitting: Fitting::DriveTune,
                price: 9_000
            }
        );
        // Perishables paying 78% against durables' 85%: evidence; refrigeration first.
        let leaky = vec![
            stat("kibble-loaf", true, 1000, 780),
            stat("pate", true, 1000, 770),
            stat("salmon-mousse", true, 1000, 790),
            stat("ore", false, 1000, 850),
        ];
        assert!(decay_evidence(&leaky).is_some());
        let d = decide_outfit(&purse(20_000, &[]), &leaky);
        assert_eq!(
            d,
            OutfitDecision::Refit {
                fitting: Fitting::Refrigeration,
                price: 4_500
            }
        );
        // Two samples are an anecdote.
        assert!(decay_evidence(&leaky[..2]).is_none());
        // No durable baseline: only a gross shortfall counts.
        let alone = vec![
            stat("pate", true, 1000, 800),
            stat("pate", true, 1000, 810),
            stat("pate", true, 1000, 790),
        ];
        assert!(decay_evidence(&alone).is_some());
        let fine = vec![
            stat("pate", true, 1000, 850),
            stat("pate", true, 1000, 850),
            stat("pate", true, 1000, 850),
        ];
        assert!(decay_evidence(&fine).is_none());
    }
}
