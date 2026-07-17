//! The marble — the familiar's presence in the macOS menu bar.
//!
//! A tiny accessory app (no Dock icon) that puts a glassy marble in the menu bar.
//! Click it to open **the Glass**; its menu can start/stop the familiar daemon. It is
//! deliberately separate from the Glass binary so the always-resident login item stays
//! small (no egui in it) — it just shells out to its sibling `glass` and `familiar`.
//!
//! Subcommands: `marble run` (default) shows the marble; `marble install` /
//! `marble uninstall` manage a launchd LaunchAgent (`io.river.marble`) so it appears at
//! login alongside the familiar. macOS only — a stub elsewhere keeps CI building.

#[cfg(target_os = "macos")]
mod mac;

#[cfg(target_os = "macos")]
fn main() {
    mac::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("the marble is a macOS menu-bar app; this platform isn't supported.");
}
