//! # Worlds are stores (ADR-0045)
//!
//! A ship world — Purr's UCF instance, or any future fiction — is its OWN STORE: its own
//! data dir, its own keys, its own boundary lease, its own lifetime. No ordinary
//! truth-bearing record anywhere carries a `world` discriminator, because a filter on a
//! shared store is an exclusion that eventually acquires one forgotten reader; a store the
//! household engine holds no handle to cannot be read by construction.
//!
//! This crate is the partition made literal, and only the partition:
//!
//! - [`instance`] — the household's minimal `WorldInstance` provisioning record: a record
//!   of a real software relationship (pubkey, label, commissioner, endpoint, lifecycle,
//!   grant epoch), never a copy of the ship world or a dossier of play (ADR-0045 §8).
//! - [`bridge`] — the narrow, typed crossings. Outward: `AttentionNotice`. Inward:
//!   exactly the human control-plane acts. A crossing is an envelope with provenance,
//!   never a record; what the household keeps of one is a RECEIPT — authorship, time,
//!   delivery — never the payload (§2–§4).
//! - [`lease`] — the signed, expiring projection of the one human-owned boundary. Purr
//!   owns no independently editable boundary; a stale, malformed, or missing lease fails
//!   closed (§5).
//!
//! What this crate deliberately does not do: open any gate, ingest any game datum, run
//! any ship cadence. Steps 3–7 of the ADR's build order live behind this one.

pub mod bridge;
pub mod instance;
pub mod lease;

use std::io;

/// One error surface for the partition primitives. IO stays IO; everything a hostile or
/// stale message can be wrong about is `Refused` — the caller's only honest reaction to
/// either is to not act.
#[derive(Debug)]
pub enum Error {
    Io(String),
    /// The envelope, lease, or registry state refuses this act; the string says why.
    Refused(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(m) => write!(f, "io: {m}"),
            Error::Refused(m) => write!(f, "refused: {m}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e.to_string())
    }
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn random_hex16() -> Result<String, Error> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|e| Error::Io(format!("getrandom: {e}")))?;
    Ok(hex(&bytes))
}
