//! The ship store's file I/O — every read and write the RUNNER makes on the pilot's
//! behalf, in one place (T-237 B4, "one doctrine, two runtimes", Ian 2026-09-05).
//!
//! The decision crates — [`crate::doctrine`], [`crate::trade`], [`crate::autonomy`],
//! [`crate::chain`], [`crate::outfit`] — own no file, socket or clock: facts in, a
//! decision out. That is what lets the same doctrine fly the hull from the host loop
//! and answer the captain from inside the iPad through `core-ffi`, and later sit
//! behind a service in the cloud. Anything that touches the ship's data dir lives here,
//! and only the runner (`src/main.rs`) and the household's fleet commands call it.

use std::collections::BTreeSet;
use std::path::Path;

use crate::autonomy::{self, Approval, Dial, Proposal, DIAL_FILE};
use crate::trade::{self, Holding};
use crate::Automation;

/// The dial from `autonomy.json`; absent or unreadable is the default dial.
pub fn load_dial(ship_dir: &Path) -> Dial {
    std::fs::read_to_string(ship_dir.join(DIAL_FILE))
        .map(|t| Dial::parse(&t))
        .unwrap_or_default()
}

/// Write the dial to `autonomy.json`.
pub fn save_dial(ship_dir: &Path, dial: &Dial) -> std::io::Result<()> {
    std::fs::write(ship_dir.join(DIAL_FILE), dial.to_json())
}

pub fn load_proposals(ship_dir: &Path) -> Vec<Proposal> {
    std::fs::read_to_string(ship_dir.join("proposals.jsonl"))
        .map(|t| autonomy::parse_proposals(&t))
        .unwrap_or_default()
}

pub fn load_approvals(ship_dir: &Path) -> Vec<Approval> {
    std::fs::read_to_string(ship_dir.join("approvals.jsonl"))
        .map(|t| autonomy::parse_approvals(&t))
        .unwrap_or_default()
}

pub fn append_proposal(ship_dir: &Path, p: &Proposal) {
    append_jsonl(&ship_dir.join("proposals.jsonl"), p);
}

pub fn append_approval(ship_dir: &Path, a: &Approval) {
    append_jsonl(&ship_dir.join("approvals.jsonl"), a);
}

fn append_jsonl<T: serde::Serialize>(path: &Path, record: &T) {
    use std::io::Write;
    if let Ok(line) = serde_json::to_string(record) {
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// The speculative book from `holdings.json`; absent or unreadable is an empty book.
pub fn load_holdings(ship_dir: &Path) -> Vec<Holding> {
    std::fs::read_to_string(ship_dir.join("holdings.json"))
        .map(|t| trade::parse_holdings(&t))
        .unwrap_or_default()
}

/// Persist the book, zeroed-out positions dropped.
pub fn save_holdings(ship_dir: &Path, holdings: &[Holding]) {
    if let Some(bytes) = trade::holdings_json(holdings) {
        let _ = std::fs::write(ship_dir.join("holdings.json"), bytes);
    }
}

/// The automations this ship holds, read from `automations.json`. An absent file
/// grants nothing.
pub fn granted_automations(ship_dir: &Path) -> (BTreeSet<Automation>, Vec<String>) {
    match std::fs::read_to_string(ship_dir.join("automations.json")) {
        Ok(raw) => crate::parse_automations(&raw),
        Err(_) => (BTreeSet::new(), Vec::new()),
    }
}

/// Read `KEY=value` out of an env-format file — the same convention the MCP
/// declaration uses for its key files (mode 0600, never committed, never logged).
pub fn env_value(path: &Path, key: &str) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    for line in raw.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                return Some(v.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autonomy::{Level, Surface};

    fn dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("whisker-store-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_dial_round_trips_through_the_store_including_market_margin() {
        let d = dir("dial");
        assert_eq!(load_dial(&d), Dial::default(), "absent = default");
        let mut dial = Dial::default();
        dial.set("market.margin", Level::Confirm).unwrap();
        dial.set("*", Level::Auto).unwrap();
        save_dial(&d, &dial).unwrap();
        let back = load_dial(&d);
        assert_eq!(back, dial);
        assert_eq!(back.level(Surface::MarketMargin), Level::Confirm);
        std::fs::write(d.join(DIAL_FILE), "not json").unwrap();
        assert_eq!(
            load_dial(&d),
            Dial::default(),
            "unreadable = default, never a panic"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn holdings_and_automations_read_back_what_was_written() {
        let d = dir("book");
        assert!(load_holdings(&d).is_empty());
        std::fs::write(d.join("automations.json"), r#"["freight","trade","warp"]"#).unwrap();
        let (granted, unknown) = granted_automations(&d);
        assert!(granted.contains(&Automation::Freight) && granted.contains(&Automation::Trade));
        assert_eq!(unknown, vec!["warp".to_string()]);
        std::fs::write(
            d.join("ucf.env"),
            "# key\nUCF_SERVER=\"http://x:1\"\nUCF_KEY=abc\n",
        )
        .unwrap();
        assert_eq!(
            env_value(&d.join("ucf.env"), "UCF_SERVER").as_deref(),
            Some("http://x:1")
        );
        assert_eq!(
            env_value(&d.join("ucf.env"), "UCF_KEY").as_deref(),
            Some("abc")
        );
        assert_eq!(env_value(&d.join("ucf.env"), "NOPE"), None);
        let _ = std::fs::remove_dir_all(&d);
    }
}
