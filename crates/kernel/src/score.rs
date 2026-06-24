//! Scoring and the adaptive promotion bar. Faithful port of v1's `score.c`.
//!
//! The promotion threshold is **self-regulating**: base 0.70, raised toward 0.95 as
//! the rigor drive rises — so when the factory promotes too easily it makes its own
//! bar harder. This is the selection-pressure invariant (do not change the constants
//! without an ADR; the threshold's *meaning* is constitutional).

use crate::trial::Trial;

/// The cost-weighted fitness selection reads. Cost is folded exactly once upstream
/// (into `trial.overall`); do not penalize cost again here.
pub fn overall(trial: &Trial) -> f64 {
    trial.overall
}

/// Adaptive promotion bar: `0.70 + 0.25 * rigor`, with `rigor` clamped to [0,1].
pub fn promote_threshold(rigor: f64) -> f64 {
    0.70 + 0.25 * rigor.clamp(0.0, 1.0)
}

/// The fixed mutation floor.
pub fn mutate_threshold() -> f64 {
    0.35
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promote_threshold_self_regulates() {
        assert!((promote_threshold(0.0) - 0.70).abs() < 1e-12);
        assert!((promote_threshold(1.0) - 0.95).abs() < 1e-12);
        assert!((promote_threshold(0.5) - 0.825).abs() < 1e-12);
        // clamped
        assert!((promote_threshold(-1.0) - 0.70).abs() < 1e-12);
        assert!((promote_threshold(2.0) - 0.95).abs() < 1e-12);
    }

    #[test]
    fn mutate_floor_is_fixed() {
        assert!((mutate_threshold() - 0.35).abs() < 1e-12);
    }
}
