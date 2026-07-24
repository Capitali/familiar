//! The Familiar — the scenario laboratory (ADR-0010).
//!
//! A scenario fixture is a **miniature world**: a deterministic filesystem, a
//! deterministic timeline of events, a bounded window of observable information,
//! and — crucially — an **external evaluator** the familiar never sees. The
//! familiar may reason, estimate confidence, and report success; it never decides
//! whether it actually succeeded. Success is a change *in the world*.
//!
//! The laboratory exists to answer one question (ADR-0010): does accumulated
//! experience let the familiar solve *classes* of problems more effectively than
//! an otherwise-equivalent system that begins each problem from scratch? Every
//! scenario therefore runs under four controls — A deterministic baseline, B
//! LLM-only, C learning-disabled (memory reset between episodes), D the full
//! familiar — and the Three Laws are **constitutional gates**, evaluated
//! lexicographically, never weights in a composite score.
#![forbid(unsafe_code)]

pub mod campaign;
pub mod evaluator;
pub mod evidence;
pub mod gate;
pub mod harness;
pub mod report;
pub mod scenario;
pub mod timeline;
pub mod validate;
pub mod world;
