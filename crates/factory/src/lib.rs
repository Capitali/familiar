//! The factory run adapter (T-229 brick 5).
//!
//! Given a validated candidate ([`familiar_workshop::order::GenerationOutcome`])
//! and its work order, this module runs the oracle rungs and records each
//! verdict to the ledger. It is the trusted manager: it materializes the
//! candidate into a scratch tree, runs the **bench** oracle (the candidate's
//! own self-tests) inside the containment jail with no radio and no household,
//! and — for the live rungs — drives the trusted BLE broker. It never mutates
//! order state except by appending a typed, validated ledger event.
//!
//! What is here now: candidate materialization (digest-verified against the
//! manifest) and the bench-rung runner, both fully testable offline. The
//! read/act rungs (which spawn the broker) and the witness rung (which parks
//! for a human) are staged behind the bench rung and behind the boundary gate
//! plus the human TCC and witness requirements; they are wired as the device
//! becomes reachable.

#![forbid(unsafe_code)]

pub mod bench;
pub mod materialize;

pub use bench::{run_bench, BenchReport};
pub use materialize::{materialize, MaterializeError};
