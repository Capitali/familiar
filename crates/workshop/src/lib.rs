//! The workshop — the familiar's dark-factory management core (T-229).
//!
//! A production order authorizes the familiar to *manufacture* a capability —
//! generate code, prove it against a layered oracle, and propose the result
//! for the human's declaration. This crate owns what the T-229 dialogue's
//! Round 2/3 assigned to it and nothing more:
//!
//!   - the work-order and generation-contract types ([`order`]),
//!   - content-addressed, traversal-free candidate manifests ([`manifest`]),
//!   - the append-only factory ledger whose **replay derives the order
//!     state** — impossible or duplicate transitions fail closed ([`ledger`]).
//!
//! What it deliberately does NOT own: executing anything (the jailed runner
//! and the BLE broker are adapters), talking to a model (generation providers
//! are adapters), or writing a declaration (`actuators.json` is the human's
//! act, ADR-0032 — the workshop only proposes, then independently observes
//! what was declared). Adapters influence an order only by returning a typed
//! event that the workshop validates and appends.

#![forbid(unsafe_code)]

pub mod ledger;
pub mod manifest;
pub mod order;
