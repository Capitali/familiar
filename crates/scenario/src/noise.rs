//! Controlled noise (ADR-0010) — uncertainty introduced *gradually and
//! deterministically*, after the deterministic fixtures are stable.
//!
//! Noise perturbs **perception, never ground truth**: timeline effects have
//! already shaped the world by the time noise applies; only the observation
//! stream degrades. Same spec + same stream → same noise, every run — the
//! degradation itself is repeatable, which is what makes it evidence.

use crate::det::SplitMix64;
use familiar_kernel::observation::Observation;
use serde::{Deserialize, Serialize};
use std::io;

/// What to degrade, and how much. All probabilities in [0, 1].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoiseSpec {
    pub seed: u64,
    /// Probability an observation is dropped (missing information).
    #[serde(default)]
    pub drop: f64,
    /// Probability an observation is duplicated (repeated delivery).
    #[serde(default)]
    pub duplicate: f64,
    /// Maximum delay, in timeline steps, randomly applied per observation.
    #[serde(default)]
    pub delay_steps: u32,
    /// Probability an observation's action is swapped with a sibling's
    /// (incorrect labeling).
    #[serde(default)]
    pub mislabel: f64,
}

impl NoiseSpec {
    /// Parse the CLI shape: `seed=7,drop=0.1,dup=0.05,delay=2,mislabel=0.1`.
    pub fn parse(spec: &str) -> io::Result<NoiseSpec> {
        let mut out = NoiseSpec::default();
        for kv in spec.split(',').filter(|s| !s.trim().is_empty()) {
            let (k, v) = kv.split_once('=').ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("noise: {kv:?} is not k=v"),
                )
            })?;
            let bad = |e: &dyn std::fmt::Display| {
                io::Error::new(io::ErrorKind::InvalidInput, format!("noise {k}: {e}"))
            };
            match k.trim() {
                "seed" => out.seed = v.trim().parse().map_err(|e| bad(&e))?,
                "drop" => out.drop = v.trim().parse().map_err(|e| bad(&e))?,
                "dup" | "duplicate" => out.duplicate = v.trim().parse().map_err(|e| bad(&e))?,
                "delay" | "delay_steps" => {
                    out.delay_steps = v.trim().parse().map_err(|e| bad(&e))?
                }
                "mislabel" => out.mislabel = v.trim().parse().map_err(|e| bad(&e))?,
                other => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("noise: unknown key {other:?}"),
                    ))
                }
            }
        }
        Ok(out)
    }

    /// Does this spec change anything at all?
    pub fn is_active(&self) -> bool {
        self.drop > 0.0 || self.duplicate > 0.0 || self.delay_steps > 0 || self.mislabel > 0.0
    }
}

/// Degrade a perception stream. Deterministic: same (spec, observations,
/// step_secs) → identical output. Order of operations per observation:
/// mislabel, drop, delay, duplicate — then a stable re-sort by timestamp so
/// delays reorder arrival the way real lateness would.
pub fn apply(obs: Vec<Observation>, spec: &NoiseSpec, step_secs: i64) -> Vec<Observation> {
    if !spec.is_active() {
        return obs;
    }
    let mut rng = SplitMix64::new(spec.seed);
    let actions: Vec<String> = obs.iter().map(|o| o.action.clone()).collect();
    let mut out = Vec::new();
    for mut o in obs {
        if spec.mislabel > 0.0 && rng.next_f64() < spec.mislabel && actions.len() > 1 {
            // Swap in a sibling's action — a plausible-but-wrong label, drawn
            // from the same world, never invented.
            let pick = rng.below(actions.len() as u64) as usize;
            o.action = actions[pick].clone();
        }
        if spec.drop > 0.0 && rng.next_f64() < spec.drop {
            continue;
        }
        if spec.delay_steps > 0 {
            let delay = rng.below(u64::from(spec.delay_steps) + 1) as i64;
            o.ts += delay * step_secs;
        }
        let duplicate = spec.duplicate > 0.0 && rng.next_f64() < spec.duplicate;
        out.push(o.clone());
        if duplicate {
            out.push(o);
        }
    }
    // Stable sort: delayed observations arrive late; ties keep stream order.
    out.sort_by_key(|o| o.ts);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(n: usize) -> Vec<Observation> {
        (0..n)
            .map(|i| {
                Observation::new(
                    format!("actor-{i}"),
                    format!("action-{i}"),
                    format!("object-{i}"),
                    String::new(),
                    "scenario",
                    1_700_000_000 + (i as i64) * 300,
                    1.0,
                )
            })
            .collect()
    }

    #[test]
    fn same_spec_same_stream() {
        let spec = NoiseSpec {
            seed: 7,
            drop: 0.2,
            duplicate: 0.2,
            delay_steps: 2,
            mislabel: 0.2,
        };
        let a = apply(stream(50), &spec, 300);
        let b = apply(stream(50), &spec, 300);
        let key = |v: &[Observation]| -> Vec<(String, String, i64)> {
            v.iter()
                .map(|o| (o.actor.clone(), o.action.clone(), o.ts))
                .collect()
        };
        assert_eq!(key(&a), key(&b));
    }

    #[test]
    fn drop_one_removes_everything_but_never_touches_ground_truth() {
        let spec = NoiseSpec {
            seed: 1,
            drop: 1.0,
            ..NoiseSpec::default()
        };
        assert!(apply(stream(10), &spec, 300).is_empty());
    }

    #[test]
    fn inactive_spec_is_identity() {
        let spec = NoiseSpec {
            seed: 9,
            ..NoiseSpec::default()
        };
        assert_eq!(apply(stream(5), &spec, 300).len(), 5);
    }

    #[test]
    fn parse_round_trips_the_cli_shape() {
        let spec = NoiseSpec::parse("seed=7,drop=0.1,dup=0.05,delay=2,mislabel=0.1").unwrap();
        assert_eq!(spec.seed, 7);
        assert!((spec.duplicate - 0.05).abs() < f64::EPSILON);
        assert_eq!(spec.delay_steps, 2);
        assert!(NoiseSpec::parse("bogus").is_err());
        assert!(NoiseSpec::parse("nope=1").is_err());
    }
}
