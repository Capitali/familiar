//! The Familiar — kernel — the deterministic core.
//!
//! This crate is the *compiled, deterministic kernel* of the hybrid architecture
//! (see `docs/ARCHITECTURE.md`): records, persistence, and the signals that make
//! the Three Laws (`docs/SOUL.md`) measurable. Behavior evolves in the periphery,
//! not here — so this core changes rarely and is held to a hard discipline.
//!
//! `#![forbid(unsafe_code)]` is the **Law III** commitment made literal: a
//! long-running autonomous process with unrestricted local and network reach must
//! not contain the memory-unsafety that would let it be turned against the served.
#![forbid(unsafe_code)]

pub mod activity;
pub mod boundary;
pub mod candidate;
pub mod capacities;
pub mod corruption;
pub mod guard;
pub mod identity;
pub mod lineage;
pub mod loops;
pub mod mutation;
pub mod observation;
pub mod parameters;
pub mod pattern_memory;
pub mod presence;
pub mod question;
pub mod regression_guard;
pub mod request;
pub mod review;
pub mod score;
pub mod selection;
pub mod service;
pub mod spec;
pub mod store;
pub mod thread;
pub mod tool;
pub mod trial;
