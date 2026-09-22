//! The running build's **release version** — the orderable identity self-upgrade compares against.
//!
//! Stamped by `build.rs` from the repo-root `VERSION` file into `FAMILIAR_BUILD` at compile time.
//! [`number`] is the monotonic release counter (0 if unstamped); [`stamp`] is the full human string
//! ("3 · adds foo"). A node adopts an incoming release only when its `number` is strictly greater
//! than this — the safety ordering that keeps an upgrade from flapping or going backwards.

/// The full stamped build string ("<n> · <label>"), or "0" if the build wasn't stamped.
pub fn stamp() -> &'static str {
    option_env!("FAMILIAR_BUILD").unwrap_or("0")
}

/// The monotonic release number of the running build (0 if unstamped).
pub fn number() -> u64 {
    stamp()
        .split(|c: char| !c.is_ascii_digit())
        .find(|s| !s.is_empty())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_parses_the_leading_integer() {
        // stamp() is whatever this build baked in; number() must be a clean parse of its first int.
        let n = number();
        // The repo VERSION is at least 1 in any real build; 0 only if unstamped.
        assert!(
            n >= 1 || stamp() == "0",
            "a stamped build has a positive version"
        );
    }
}
