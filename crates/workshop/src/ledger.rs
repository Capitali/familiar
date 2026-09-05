//! The factory ledger — append-only, and the only truth about an order.
//!
//! There is no mutable status field anywhere in the workshop: an order's
//! state is **derived by replaying its ledger**, and a transition that could
//! not legally follow the events before it fails closed at append time. The
//! same validation runs on read, so a ledger tampered behind the workshop's
//! back stops replaying rather than lying.
//!
//! Events carry digests, never artifacts: the ledger says *what happened and
//! to which exact bytes*; the content-addressed store says what the bytes
//! were.
//!
//! Six authority boundaries this module holds closed (codex's Brick-1 review
//! and Brick-2 follow-up, 2026-08-28):
//!   1. **Declaration equality is derived, never asserted.** An observed
//!      declaration advances the order only when its digest equals the
//!      proposed digest — no caller-supplied "it matches" bit exists.
//!   2. **Proof cannot be manufactured inside an iteration.** A witness pass
//!      requires a `Yes` bound to that exact request; a `No` fails the
//!      iteration; and any failed rung bars every further rung until the next
//!      counted generation.
//!   3. **The generation contract is enforced at the door.** A
//!      `GenerationReturned` event can only be minted from a
//!      `validate_outcome`-passing outcome ([`Ledger::append_generation`]),
//!      and replay re-checks the carried surface/lock against the order.
//!   4. **Appends are serialized durably.** A pid-lease lock file guards the
//!      whole read→validate→write section (a live holder is never stolen
//!      from; only a dead owner's lock is reclaimed), and each line is
//!      `sync_all`'d before success is reported.
//!   5. **A new candidate cannot inherit an old declaration.** Each counted
//!      generation clears any prior proposal/declaration, so generation N+1
//!      can never be commissioned on generation N's proof.

use serde::{Deserialize, Serialize};

use crate::order::{
    outcome_digest, validate_order, validate_outcome, GenerationOutcome, OracleRung, WorkOrder,
};

/// A witness's permitted answers. `Unclear` records that a witness was
/// attempted and supplies no proof in either direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WitnessAnswer {
    Yes,
    No,
    Unclear,
}

/// One ledger event. `seq` is 1-based and strictly consecutive; `at` is the
/// caller's clock in unix seconds (recorded, never validated — the order of
/// truth is `seq`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerEvent {
    pub seq: u64,
    pub at: u64,
    pub order_id: String,
    pub kind: EventKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventKind {
    /// Always and only the first event. Carries the whole immutable order
    /// (boxed: this variant is far larger than the rest).
    Opened { order: Box<WorkOrder> },
    /// A generation adapter returned. Minted only through
    /// [`Ledger::append_generation`], which validates the whole outcome; the
    /// carried surface/lock let replay re-check the order fit. Iterations are
    /// counted from 1 and must arrive in order; a refused outcome is
    /// recorded, not erased.
    GenerationReturned {
        iteration: u32,
        outcome_digest: String,
        refused: bool,
        /// The candidate's capability surface (empty for a refusal) — replay
        /// re-checks it is within the order.
        capability_surface: Vec<String>,
        /// The candidate's toolchain lock (empty for a refusal) — replay
        /// re-checks it equals the order's.
        toolchain_lock: String,
    },
    /// An oracle rung's verdict for the current iteration's candidate.
    RungVerdict {
        iteration: u32,
        rung: OracleRung,
        pass: bool,
        evidence_digest: String,
    },
    /// A witness ask went out (through the console seam, elsewhere).
    WitnessRequested {
        iteration: u32,
        request_digest: String,
    },
    /// The human answered that exact request — the digest must match the
    /// outstanding one, so an answer cannot be re-bound to a different ask.
    WitnessAnswered {
        iteration: u32,
        request_digest: String,
        answer: WitnessAnswer,
    },
    /// The order parked (gate shut, jail unavailable, witness outstanding…).
    /// Parking is not terminal; work resumes with the next legal event.
    Parked { reason: String },
    /// The factory proposed an exact declaration for the human's hand.
    /// `reduced` means unproved operations were excluded from the proposal.
    DeclarationProposed { digest: String, reduced: bool },
    /// The workshop observed the on-disk declaration. Whether it advances the
    /// order is DERIVED from digest equality with the proposal — there is no
    /// caller-supplied match bit.
    DeclarationObserved { digest: String },
    /// Post-declaration, post-restart smoke pass through the declared
    /// command. Terminal success.
    Commissioned { evidence_digest: String },
    /// Terminal close without commissioning (refusal stands, order
    /// withdrawn, subject gone). The reason is the record.
    Closed { reason: String },
}

/// The state replay derives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderState {
    pub order: WorkOrder,
    /// 0 until the first generation returns.
    pub iteration: u32,
    /// Whether the current iteration's outcome was a refusal.
    pub refused: bool,
    /// Rungs passed by the current iteration, in plan order.
    pub rungs_passed: Vec<OracleRung>,
    /// Set when a rung fails or a witness answers `No`: no rung may pass and
    /// no witness may be requested until the next counted generation.
    pub iteration_broken: bool,
    /// The request digest of an outstanding, unanswered witness ask.
    pub witness_outstanding: Option<String>,
    /// A `Yes` arrived for a witness request this iteration — the only thing
    /// that admits a `Witness` rung pass.
    pub witness_passed: bool,
    pub parked: Option<String>,
    /// Digest of a proposed declaration, once proposed.
    pub proposed: Option<String>,
    /// True once an observed declaration's digest equalled the proposal.
    pub declared: bool,
    pub commissioned: bool,
    pub closed: Option<String>,
}

impl OrderState {
    pub fn terminal(&self) -> bool {
        self.commissioned || self.closed.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    EmptyLedger,
    FirstEventNotOpened,
    SecondOpening,
    BadSequence {
        expected: u64,
        got: u64,
    },
    WrongOrder {
        expected: String,
        got: String,
    },
    AfterTerminal,
    InvalidOrder(String),
    IterationOutOfOrder {
        expected: u32,
        got: u32,
    },
    RungNotInPlan(OracleRung),
    RungBeforePredecessor(OracleRung),
    RungWithoutCandidate,
    RungAfterRefusal,
    /// A rung verdict or witness request after a failed rung / `No` witness,
    /// with no new generation between — a failure demands a new iteration.
    RungAfterFailure,
    DuplicateRung(OracleRung),
    WitnessNotRequested,
    /// An answer whose request digest is not the outstanding one.
    WitnessDigestMismatch,
    WitnessAlreadyOutstanding,
    /// A `Witness` rung pass with no bound `Yes` for a request this iteration.
    WitnessPassWithoutYes,
    /// A generation event whose carried surface exceeds the order.
    GenerationSurfaceBeyondOrder(String),
    /// A generation event whose carried lock is not the order's.
    GenerationToolchainMismatch,
    /// A raw `append` was handed a `GenerationReturned` — those must go
    /// through `append_generation`, which validates the outcome.
    UseAppendGeneration,
    ProposalBeforeProof,
    /// A full (non-reduced) proposal with the plan's witness rung unpassed.
    FullProposalUnwitnessed,
    ObservationBeforeProposal,
    CommissionBeforeDeclaration,
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LedgerError::EmptyLedger => write!(f, "ledger has no events"),
            LedgerError::FirstEventNotOpened => write!(f, "first event must open the order"),
            LedgerError::SecondOpening => write!(f, "order opened twice"),
            LedgerError::BadSequence { expected, got } => {
                write!(f, "sequence break: expected {expected}, got {got}")
            }
            LedgerError::WrongOrder { expected, got } => {
                write!(f, "event for order {got} in ledger of {expected}")
            }
            LedgerError::AfterTerminal => write!(f, "event after terminal state"),
            LedgerError::InvalidOrder(e) => write!(f, "order invalid at opening: {e}"),
            LedgerError::IterationOutOfOrder { expected, got } => {
                write!(f, "iteration out of order: expected {expected}, got {got}")
            }
            LedgerError::RungNotInPlan(r) => write!(f, "rung {r:?} is not in the order's plan"),
            LedgerError::RungBeforePredecessor(r) => {
                write!(f, "rung {r:?} before its predecessor passed")
            }
            LedgerError::RungWithoutCandidate => write!(f, "rung verdict before any candidate"),
            LedgerError::RungAfterRefusal => write!(f, "rung verdict on a refused iteration"),
            LedgerError::RungAfterFailure => {
                write!(f, "rung/witness after a failure without a new generation")
            }
            LedgerError::DuplicateRung(r) => write!(f, "rung {r:?} judged twice this iteration"),
            LedgerError::WitnessNotRequested => write!(f, "witness answer without a request"),
            LedgerError::WitnessDigestMismatch => {
                write!(f, "witness answer digest is not the outstanding request")
            }
            LedgerError::WitnessAlreadyOutstanding => {
                write!(f, "second witness request while one is outstanding")
            }
            LedgerError::WitnessPassWithoutYes => {
                write!(f, "witness rung passed without a bound yes")
            }
            LedgerError::GenerationSurfaceBeyondOrder(s) => {
                write!(f, "generation surface beyond order: {s}")
            }
            LedgerError::GenerationToolchainMismatch => {
                write!(f, "generation toolchain lock is not the order's")
            }
            LedgerError::UseAppendGeneration => {
                write!(f, "generation events must be minted via append_generation")
            }
            LedgerError::ProposalBeforeProof => {
                write!(f, "declaration proposed before bench/read/act closed")
            }
            LedgerError::FullProposalUnwitnessed => {
                write!(f, "full proposal with the witness rung unpassed")
            }
            LedgerError::ObservationBeforeProposal => {
                write!(f, "declaration observed before any proposal")
            }
            LedgerError::CommissionBeforeDeclaration => {
                write!(f, "commissioned before a matching declaration was observed")
            }
        }
    }
}

/// Replay a whole ledger to the state it proves. Every rule that applies at
/// append time applies here identically — see [`apply`].
pub fn replay(events: &[LedgerEvent]) -> Result<OrderState, LedgerError> {
    let mut it = events.iter();
    let first = it.next().ok_or(LedgerError::EmptyLedger)?;
    if first.seq != 1 {
        return Err(LedgerError::BadSequence {
            expected: 1,
            got: first.seq,
        });
    }
    let mut state = match &first.kind {
        EventKind::Opened { order } => {
            validate_order(order).map_err(|e| LedgerError::InvalidOrder(e.to_string()))?;
            if order.id != first.order_id {
                return Err(LedgerError::WrongOrder {
                    expected: order.id.clone(),
                    got: first.order_id.clone(),
                });
            }
            OrderState {
                order: (**order).clone(),
                iteration: 0,
                refused: false,
                rungs_passed: Vec::new(),
                iteration_broken: false,
                witness_outstanding: None,
                witness_passed: false,
                parked: None,
                proposed: None,
                declared: false,
                commissioned: false,
                closed: None,
            }
        }
        _ => return Err(LedgerError::FirstEventNotOpened),
    };
    let mut seq = 1u64;
    for ev in it {
        seq += 1;
        if ev.seq != seq {
            return Err(LedgerError::BadSequence {
                expected: seq,
                got: ev.seq,
            });
        }
        apply(&mut state, ev)?;
    }
    Ok(state)
}

/// Apply one post-opening event to a state, fail closed.
fn apply(state: &mut OrderState, ev: &LedgerEvent) -> Result<(), LedgerError> {
    if ev.order_id != state.order.id {
        return Err(LedgerError::WrongOrder {
            expected: state.order.id.clone(),
            got: ev.order_id.clone(),
        });
    }
    if state.terminal() {
        return Err(LedgerError::AfterTerminal);
    }
    match &ev.kind {
        EventKind::Opened { .. } => Err(LedgerError::SecondOpening),
        EventKind::GenerationReturned {
            iteration,
            refused,
            capability_surface,
            toolchain_lock,
            ..
        } => {
            let expected = state.iteration + 1;
            if *iteration != expected {
                return Err(LedgerError::IterationOutOfOrder {
                    expected,
                    got: *iteration,
                });
            }
            if !*refused {
                for s in capability_surface {
                    if !state.order.capability_surface.contains(s) {
                        return Err(LedgerError::GenerationSurfaceBeyondOrder(s.clone()));
                    }
                }
                if *toolchain_lock != state.order.toolchain.lock_digest {
                    return Err(LedgerError::GenerationToolchainMismatch);
                }
            }
            state.iteration = *iteration;
            state.refused = *refused;
            state.rungs_passed.clear();
            state.iteration_broken = false;
            state.witness_outstanding = None;
            state.witness_passed = false;
            state.parked = None;
            // A new candidate invalidates any prior candidate's proof: its
            // proposal and declaration do not carry over, or generation N+1
            // could be commissioned on generation N's declaration (codex
            // Brick-2 review, blocker 5). Each candidate must be proposed and
            // declared afresh.
            state.proposed = None;
            state.declared = false;
            Ok(())
        }
        EventKind::RungVerdict {
            iteration,
            rung,
            pass,
            ..
        } => {
            if state.iteration == 0 {
                return Err(LedgerError::RungWithoutCandidate);
            }
            if *iteration != state.iteration {
                return Err(LedgerError::IterationOutOfOrder {
                    expected: state.iteration,
                    got: *iteration,
                });
            }
            if state.refused {
                return Err(LedgerError::RungAfterRefusal);
            }
            if state.iteration_broken {
                return Err(LedgerError::RungAfterFailure);
            }
            let plan = &state.order.oracle_plan;
            let pos = plan
                .iter()
                .position(|r| r == rung)
                .ok_or(LedgerError::RungNotInPlan(*rung))?;
            if state.rungs_passed.contains(rung) {
                return Err(LedgerError::DuplicateRung(*rung));
            }
            if pos > 0 && !state.rungs_passed.contains(&plan[pos - 1]) {
                return Err(LedgerError::RungBeforePredecessor(*rung));
            }
            if *pass {
                if *rung == OracleRung::Witness && !state.witness_passed {
                    return Err(LedgerError::WitnessPassWithoutYes);
                }
                state.rungs_passed.push(*rung);
            } else {
                // A failed rung ends the iteration's authority to pass
                // anything: the next legal progress is a new generation.
                state.iteration_broken = true;
            }
            Ok(())
        }
        EventKind::WitnessRequested {
            iteration,
            request_digest,
        } => {
            if state.iteration == 0 || *iteration != state.iteration {
                return Err(LedgerError::RungWithoutCandidate);
            }
            if state.iteration_broken {
                return Err(LedgerError::RungAfterFailure);
            }
            if state.witness_outstanding.is_some() {
                return Err(LedgerError::WitnessAlreadyOutstanding);
            }
            state.witness_outstanding = Some(request_digest.clone());
            Ok(())
        }
        EventKind::WitnessAnswered {
            iteration,
            request_digest,
            answer,
        } => {
            if *iteration != state.iteration {
                return Err(LedgerError::WitnessNotRequested);
            }
            match &state.witness_outstanding {
                None => return Err(LedgerError::WitnessNotRequested),
                Some(d) if d != request_digest => return Err(LedgerError::WitnessDigestMismatch),
                Some(_) => {}
            }
            state.witness_outstanding = None;
            match answer {
                WitnessAnswer::Yes => state.witness_passed = true,
                WitnessAnswer::No => state.iteration_broken = true,
                WitnessAnswer::Unclear => { /* unproved; order may park or re-ask */ }
            }
            Ok(())
        }
        EventKind::Parked { reason } => {
            state.parked = Some(reason.clone());
            Ok(())
        }
        EventKind::DeclarationProposed { digest, reduced } => {
            for required in [OracleRung::Bench, OracleRung::Read, OracleRung::Act] {
                if state.order.oracle_plan.contains(&required)
                    && !state.rungs_passed.contains(&required)
                {
                    return Err(LedgerError::ProposalBeforeProof);
                }
            }
            if !reduced
                && state.order.oracle_plan.contains(&OracleRung::Witness)
                && !state.rungs_passed.contains(&OracleRung::Witness)
            {
                return Err(LedgerError::FullProposalUnwitnessed);
            }
            state.proposed = Some(digest.clone());
            state.parked = None;
            Ok(())
        }
        EventKind::DeclarationObserved { digest } => {
            let proposed = state
                .proposed
                .as_deref()
                .ok_or(LedgerError::ObservationBeforeProposal)?;
            // Derived, never asserted: equality of digests is the only thing
            // that advances the order. A diverged declaration is recorded but
            // leaves `declared` false — the changed surface must be
            // re-proposed and re-validated upstream.
            state.declared = proposed == digest.as_str();
            Ok(())
        }
        EventKind::Commissioned { .. } => {
            if !state.declared {
                return Err(LedgerError::CommissionBeforeDeclaration);
            }
            state.commissioned = true;
            Ok(())
        }
        EventKind::Closed { reason } => {
            state.closed = Some(reason.clone());
            Ok(())
        }
    }
}

/// A cross-process advisory lock held for the duration of a critical section.
///
/// It is the operating system's lock (`flock`-style, via `File::try_lock`), not a
/// pid file: the kernel releases it the instant the holder exits, crashes, or is
/// killed, so there is no orphan to reclaim and no interval — however short — in
/// which a live holder is unidentifiable and could be stolen from (codex
/// whole-factory review, blocker 5). The lock file is never deleted: unlinking a
/// path other processes may already have open would let two of them hold "the"
/// lock on different inodes. Its content (the holder's pid) is a diagnostic only.
struct LedgerLock {
    _file: std::fs::File,
}

impl LedgerLock {
    fn acquire(base: &std::path::Path) -> Result<Self, String> {
        let path = base.with_extension("lock");
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| format!("ledger lock: {e}"))?;
        for _ in 0..4_000 {
            match Self::try_once(&file) {
                Ok(true) => return Ok(LedgerLock { _file: file }),
                Ok(false) => std::thread::sleep(std::time::Duration::from_millis(5)),
                Err(e) => return Err(e),
            }
        }
        Err("ledger lock timeout".to_string())
    }

    /// One non-blocking attempt: `Ok(true)` holds it, `Ok(false)` means a live
    /// holder has it. On success the pid is written for whoever is looking.
    fn try_once(file: &std::fs::File) -> Result<bool, String> {
        match file.try_lock() {
            Ok(()) => {
                use std::io::{Seek as _, Write as _};
                let mut f = file;
                let _ = f.set_len(0);
                let _ = f.seek(std::io::SeekFrom::Start(0));
                let _ = write!(f, "{}", std::process::id());
                let _ = f.flush();
                Ok(true)
            }
            Err(std::fs::TryLockError::WouldBlock) => Ok(false),
            Err(std::fs::TryLockError::Error(e)) => Err(format!("ledger lock: {e}")),
        }
    }
}
// No `Drop`: closing the file releases the lock; the path stays for the next holder.

/// The on-disk ledger: one JSON event per line, append-only. Every append
/// takes a cross-process lock, revalidates the whole file plus the new event
/// by full replay, then writes one flushed line — so an illegal transition
/// can never reach disk and two writers can never mint the same sequence.
pub struct Ledger {
    path: std::path::PathBuf,
}

impl Ledger {
    pub fn at(path: impl Into<std::path::PathBuf>) -> Self {
        Ledger { path: path.into() }
    }

    pub fn read(&self) -> Result<Vec<LedgerEvent>, String> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("ledger read: {e}")),
        };
        let mut events = Vec::new();
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let ev: LedgerEvent =
                serde_json::from_str(line).map_err(|e| format!("ledger line {}: {e}", n + 1))?;
            events.push(ev);
        }
        Ok(events)
    }

    /// Append one event that is NOT a generation. Generation must go through
    /// [`Ledger::append_generation`], which validates the outcome.
    pub fn append(&self, at: u64, order_id: &str, kind: EventKind) -> Result<OrderState, String> {
        if matches!(kind, EventKind::GenerationReturned { .. }) {
            return Err(LedgerError::UseAppendGeneration.to_string());
        }
        let _lock = LedgerLock::acquire(&self.path)?;
        let events = self.read()?;
        self.write_validated(events, at, order_id, kind)
    }

    /// Mint a generation event, but only from an outcome that passes the full
    /// contract against the order recorded in this ledger. The event stores
    /// the outcome's digest and its (order-fitting) surface/lock.
    pub fn append_generation(
        &self,
        at: u64,
        order_id: &str,
        iteration: u32,
        outcome: &GenerationOutcome,
    ) -> Result<OrderState, String> {
        let _lock = LedgerLock::acquire(&self.path)?;
        let events = self.read()?;
        let state = replay(&events).map_err(|e| e.to_string())?;
        validate_outcome(&state.order, outcome).map_err(|e| e.to_string())?;
        let (refused, surface, lock) = match outcome {
            GenerationOutcome::Refused(_) => (true, Vec::new(), String::new()),
            GenerationOutcome::Candidate {
                capability_surface,
                toolchain_lock,
                ..
            } => (false, capability_surface.clone(), toolchain_lock.clone()),
        };
        let kind = EventKind::GenerationReturned {
            iteration,
            outcome_digest: outcome_digest(outcome),
            refused,
            capability_surface: surface,
            toolchain_lock: lock,
        };
        self.write_validated(events, at, order_id, kind)
    }

    /// With the lock already held: append the event to `events`, replay the
    /// whole thing to prove legality, then write one flushed line.
    fn write_validated(
        &self,
        mut events: Vec<LedgerEvent>,
        at: u64,
        order_id: &str,
        kind: EventKind,
    ) -> Result<OrderState, String> {
        let seq = events.len() as u64 + 1;
        events.push(LedgerEvent {
            seq,
            at,
            order_id: order_id.to_string(),
            kind,
        });
        let state = replay(&events).map_err(|e| e.to_string())?;
        let line = serde_json::to_string(events.last().expect("just pushed"))
            .map_err(|e| e.to_string())?;
        use std::io::Write as _;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| format!("ledger open: {e}"))?;
        writeln!(f, "{line}").map_err(|e| format!("ledger write: {e}"))?;
        // Durable before we report success: sync the file data to disk, not
        // merely flush into the kernel (codex Brick-2 review, blocker 6).
        f.sync_all().map_err(|e| format!("ledger sync: {e}"))?;
        Ok(state)
    }

    pub fn state(&self) -> Result<OrderState, String> {
        replay(&self.read()?).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{digest_bytes, FileEntry, FileRole, Manifest};
    use crate::order::{OracleRung, ResearchEntry, Toolchain, WorkOrder};

    fn order_one() -> WorkOrder {
        WorkOrder {
            id: "order-0001-motorlights".into(),
            requester: "ian".into(),
            goal: "manufacture a driver for the declared SP548E LED controller".into(),
            wording: "autonomously discover, write, execute, and automate".into(),
            subject: "ble:mfr=0x5053,wifi_mac=ba:16:b5:fe:19:82".into(),
            capability_surface: vec!["state".into(), "on".into(), "off".into()],
            research: vec![ResearchEntry {
                title: "protocol notes".into(),
                source: "household record".into(),
                digest: digest_bytes(b"notes"),
            }],
            required_gates: vec!["allow_execute".into(), "allow_actuate".into()],
            oracle_plan: vec![
                OracleRung::Bench,
                OracleRung::Read,
                OracleRung::Act,
                OracleRung::Witness,
            ],
            toolchain: Toolchain {
                interpreter: "python3.13".into(),
                lock_digest: String::new(),
            },
            containment: "jail-v1".into(),
        }
    }

    fn candidate() -> GenerationOutcome {
        GenerationOutcome::Candidate {
            manifest: Manifest {
                files: vec![
                    FileEntry {
                        path: "sp548e.py".into(),
                        digest: digest_bytes(b"driver"),
                        role: FileRole::Source,
                    },
                    FileEntry {
                        path: "test_sp548e.py".into(),
                        digest: digest_bytes(b"tests"),
                        role: FileRole::SelfTest,
                    },
                ],
            },
            entrypoints: vec!["sp548e.py".into()],
            self_tests: vec!["test_sp548e.py".into()],
            declared_effects: vec!["state".into(), "on".into(), "off".into()],
            toolchain_lock: String::new(),
            capability_surface: vec!["state".into(), "on".into(), "off".into()],
        }
    }

    fn ev(seq: u64, kind: EventKind) -> LedgerEvent {
        LedgerEvent {
            seq,
            at: 1_000 + seq,
            order_id: "order-0001-motorlights".into(),
            kind,
        }
    }

    fn opened() -> LedgerEvent {
        ev(
            1,
            EventKind::Opened {
                order: Box::new(order_one()),
            },
        )
    }

    /// A well-formed generation event for a valid candidate (surface/lock
    /// carried honestly), used to build legal histories in replay tests.
    fn generation(seq: u64, iteration: u32) -> LedgerEvent {
        ev(
            seq,
            EventKind::GenerationReturned {
                iteration,
                outcome_digest: outcome_digest(&candidate()),
                refused: false,
                capability_surface: vec!["state".into(), "on".into(), "off".into()],
                toolchain_lock: String::new(),
            },
        )
    }

    fn verdict(seq: u64, iteration: u32, rung: OracleRung, pass: bool) -> LedgerEvent {
        ev(
            seq,
            EventKind::RungVerdict {
                iteration,
                rung,
                pass,
                evidence_digest: digest_bytes(b"evidence"),
            },
        )
    }

    fn witness_yes(seq_req: u64, seq_ans: u64, iteration: u32) -> [LedgerEvent; 2] {
        let req = digest_bytes(b"strip is red?");
        [
            ev(
                seq_req,
                EventKind::WitnessRequested {
                    iteration,
                    request_digest: req.clone(),
                },
            ),
            ev(
                seq_ans,
                EventKind::WitnessAnswered {
                    iteration,
                    request_digest: req,
                    answer: WitnessAnswer::Yes,
                },
            ),
        ]
    }

    #[test]
    fn the_whole_happy_path_replays_to_commissioned() {
        let [wreq, wans] = witness_yes(6, 7, 1);
        let events = vec![
            opened(),
            generation(2, 1),
            verdict(3, 1, OracleRung::Bench, true),
            verdict(4, 1, OracleRung::Read, true),
            verdict(5, 1, OracleRung::Act, true),
            wreq,
            wans,
            verdict(8, 1, OracleRung::Witness, true),
            ev(
                9,
                EventKind::DeclarationProposed {
                    digest: digest_bytes(b"actuators.json"),
                    reduced: false,
                },
            ),
            ev(
                10,
                EventKind::DeclarationObserved {
                    digest: digest_bytes(b"actuators.json"),
                },
            ),
            ev(
                11,
                EventKind::Commissioned {
                    evidence_digest: digest_bytes(b"smoke"),
                },
            ),
        ];
        let s = replay(&events).expect("legal history");
        assert!(s.commissioned);
        assert!(s.terminal());
        assert_eq!(s.iteration, 1);
    }

    #[test]
    fn a_failed_rung_bars_further_passes_until_a_new_iteration() {
        let broken = vec![
            opened(),
            generation(2, 1),
            verdict(3, 1, OracleRung::Bench, false),
            verdict(4, 1, OracleRung::Bench, true),
        ];
        assert_eq!(replay(&broken), Err(LedgerError::RungAfterFailure));

        let recovered = vec![
            opened(),
            generation(2, 1),
            verdict(3, 1, OracleRung::Bench, false),
            generation(4, 2),
            verdict(5, 2, OracleRung::Bench, true),
        ];
        let s = replay(&recovered).expect("legal history");
        assert_eq!(s.iteration, 2);
        assert_eq!(s.rungs_passed, vec![OracleRung::Bench]);
    }

    #[test]
    fn a_witness_pass_requires_a_bound_yes() {
        let events = vec![
            opened(),
            generation(2, 1),
            verdict(3, 1, OracleRung::Bench, true),
            verdict(4, 1, OracleRung::Read, true),
            verdict(5, 1, OracleRung::Act, true),
            verdict(6, 1, OracleRung::Witness, true),
        ];
        assert_eq!(replay(&events), Err(LedgerError::WitnessPassWithoutYes));
    }

    #[test]
    fn a_witness_answer_must_match_the_outstanding_request() {
        let events = vec![
            opened(),
            generation(2, 1),
            ev(
                3,
                EventKind::WitnessRequested {
                    iteration: 1,
                    request_digest: digest_bytes(b"is it red?"),
                },
            ),
            ev(
                4,
                EventKind::WitnessAnswered {
                    iteration: 1,
                    request_digest: digest_bytes(b"a DIFFERENT ask"),
                    answer: WitnessAnswer::Yes,
                },
            ),
        ];
        assert_eq!(replay(&events), Err(LedgerError::WitnessDigestMismatch));
    }

    #[test]
    fn a_no_witness_fails_the_iteration() {
        let req = digest_bytes(b"is it red?");
        let events = vec![
            opened(),
            generation(2, 1),
            verdict(3, 1, OracleRung::Bench, true),
            verdict(4, 1, OracleRung::Read, true),
            verdict(5, 1, OracleRung::Act, true),
            ev(
                6,
                EventKind::WitnessRequested {
                    iteration: 1,
                    request_digest: req.clone(),
                },
            ),
            ev(
                7,
                EventKind::WitnessAnswered {
                    iteration: 1,
                    request_digest: req,
                    answer: WitnessAnswer::No,
                },
            ),
            verdict(8, 1, OracleRung::Witness, true),
        ];
        assert_eq!(replay(&events), Err(LedgerError::RungAfterFailure));
    }

    #[test]
    fn the_first_event_must_open_and_only_once() {
        assert_eq!(replay(&[]), Err(LedgerError::EmptyLedger));
        assert_eq!(
            replay(&[generation(1, 1)]),
            Err(LedgerError::FirstEventNotOpened)
        );
        assert_eq!(
            replay(&[
                opened(),
                ev(
                    2,
                    EventKind::Opened {
                        order: Box::new(order_one())
                    }
                )
            ]),
            Err(LedgerError::SecondOpening)
        );
    }

    #[test]
    fn a_generation_surface_beyond_the_order_fails_replay() {
        let bad = ev(
            2,
            EventKind::GenerationReturned {
                iteration: 1,
                outcome_digest: digest_bytes(b"x"),
                refused: false,
                capability_surface: vec!["unlock-door".into()],
                toolchain_lock: String::new(),
            },
        );
        assert_eq!(
            replay(&[opened(), bad]),
            Err(LedgerError::GenerationSurfaceBeyondOrder(
                "unlock-door".into()
            ))
        );
    }

    #[test]
    fn rungs_cannot_skip_their_predecessor_or_repeat() {
        assert_eq!(
            replay(&[
                opened(),
                generation(2, 1),
                verdict(3, 1, OracleRung::Read, true)
            ]),
            Err(LedgerError::RungBeforePredecessor(OracleRung::Read))
        );
        assert_eq!(
            replay(&[
                opened(),
                generation(2, 1),
                verdict(3, 1, OracleRung::Bench, true),
                verdict(4, 1, OracleRung::Bench, true),
            ]),
            Err(LedgerError::DuplicateRung(OracleRung::Bench))
        );
    }

    #[test]
    fn no_rung_without_a_candidate_and_none_on_a_refusal() {
        assert_eq!(
            replay(&[opened(), verdict(2, 1, OracleRung::Bench, true)]),
            Err(LedgerError::RungWithoutCandidate)
        );
        let refused = ev(
            2,
            EventKind::GenerationReturned {
                iteration: 1,
                outcome_digest: digest_bytes(b"refusal"),
                refused: true,
                capability_surface: Vec::new(),
                toolchain_lock: String::new(),
            },
        );
        assert_eq!(
            replay(&[opened(), refused, verdict(3, 1, OracleRung::Bench, true)]),
            Err(LedgerError::RungAfterRefusal)
        );
    }

    #[test]
    fn a_full_proposal_needs_the_witness_but_a_reduced_one_does_not() {
        let base = vec![
            opened(),
            generation(2, 1),
            verdict(3, 1, OracleRung::Bench, true),
            verdict(4, 1, OracleRung::Read, true),
            verdict(5, 1, OracleRung::Act, true),
        ];
        let mut full = base.clone();
        full.push(ev(
            6,
            EventKind::DeclarationProposed {
                digest: digest_bytes(b"d"),
                reduced: false,
            },
        ));
        assert_eq!(replay(&full), Err(LedgerError::FullProposalUnwitnessed));

        let mut reduced = base;
        reduced.push(ev(
            6,
            EventKind::DeclarationProposed {
                digest: digest_bytes(b"d"),
                reduced: true,
            },
        ));
        let s = replay(&reduced).expect("reduced proposal is legal");
        assert!(s.proposed.is_some());
        assert!(!s.declared);
    }

    #[test]
    fn a_diverged_declaration_never_becomes_declared_or_commissioned() {
        let mut events = vec![
            opened(),
            generation(2, 1),
            verdict(3, 1, OracleRung::Bench, true),
            verdict(4, 1, OracleRung::Read, true),
            verdict(5, 1, OracleRung::Act, true),
            ev(
                6,
                EventKind::DeclarationProposed {
                    digest: digest_bytes(b"the exact proposal"),
                    reduced: true,
                },
            ),
            ev(
                7,
                EventKind::DeclarationObserved {
                    digest: digest_bytes(b"an EDITED declaration"),
                },
            ),
        ];
        let s = replay(&events).expect("observation is legal, it just doesn't match");
        assert!(!s.declared, "a different digest must not declare");

        events.push(ev(
            8,
            EventKind::Commissioned {
                evidence_digest: digest_bytes(b"smoke"),
            },
        ));
        assert_eq!(
            replay(&events),
            Err(LedgerError::CommissionBeforeDeclaration)
        );
    }

    #[test]
    fn a_matching_declaration_declares_and_permits_commissioning() {
        let d = digest_bytes(b"actuators.json");
        let events = vec![
            opened(),
            generation(2, 1),
            verdict(3, 1, OracleRung::Bench, true),
            verdict(4, 1, OracleRung::Read, true),
            verdict(5, 1, OracleRung::Act, true),
            ev(
                6,
                EventKind::DeclarationProposed {
                    digest: d.clone(),
                    reduced: true,
                },
            ),
            ev(7, EventKind::DeclarationObserved { digest: d }),
            ev(
                8,
                EventKind::Commissioned {
                    evidence_digest: digest_bytes(b"smoke"),
                },
            ),
        ];
        let s = replay(&events).expect("legal");
        assert!(s.declared && s.commissioned);
    }

    #[test]
    fn a_new_generation_cannot_commission_on_the_previous_candidates_declaration() {
        // Gen 1 is fully proven, proposed (reduced), and its matching
        // declaration observed. Then gen 2 arrives — a DIFFERENT candidate —
        // and tries to commission on gen 1's declaration.
        let d = digest_bytes(b"gen1 actuators.json");
        let events = vec![
            opened(),
            generation(2, 1),
            verdict(3, 1, OracleRung::Bench, true),
            verdict(4, 1, OracleRung::Read, true),
            verdict(5, 1, OracleRung::Act, true),
            ev(
                6,
                EventKind::DeclarationProposed {
                    digest: d.clone(),
                    reduced: true,
                },
            ),
            ev(7, EventKind::DeclarationObserved { digest: d }),
            // gen 1 was declared but not commissioned; a new candidate lands.
            generation(8, 2),
            // Its proof is fresh (only bench so far), and it tries to commission.
            verdict(9, 2, OracleRung::Bench, true),
            ev(
                10,
                EventKind::Commissioned {
                    evidence_digest: digest_bytes(b"smoke"),
                },
            ),
        ];
        // The new generation cleared proposed/declared, so commissioning is
        // refused — gen 2 cannot inherit gen 1's declaration.
        assert_eq!(
            replay(&events),
            Err(LedgerError::CommissionBeforeDeclaration)
        );
    }

    #[test]
    fn a_live_lock_holder_is_never_stolen_from() {
        let dir = std::env::temp_dir().join(format!("familiar-lock-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let base = dir.join("order.jsonl");
        let lockp = base.with_extension("lock");
        let _ = std::fs::remove_file(&lockp);

        let held = LedgerLock::acquire(&base).expect("acquire");
        // A second open of the same path cannot take it while we hold it — not
        // on a timer, not ever; the kernel arbitrates, not a pid file.
        let other = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lockp)
            .unwrap();
        assert_eq!(LedgerLock::try_once(&other), Ok(false));
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(
            LedgerLock::try_once(&other),
            Ok(false),
            "time changes nothing"
        );

        // Whatever the file says means nothing: a holder that died released the
        // OS lock, and the next acquirer simply takes it.
        drop(held);
        std::fs::write(&lockp, "not-a-pid").unwrap();
        let taken = LedgerLock::acquire(&base).expect("a dead holder's lock is free");
        assert_eq!(LedgerLock::try_once(&other), Ok(false));
        drop(taken);
        assert_eq!(LedgerLock::try_once(&other), Ok(true));
        assert!(lockp.exists(), "the lock path is never unlinked");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nothing_follows_a_terminal_event() {
        let events = vec![
            opened(),
            ev(
                2,
                EventKind::Closed {
                    reason: "withdrawn by requester".into(),
                },
            ),
            generation(3, 1),
        ];
        assert_eq!(replay(&events), Err(LedgerError::AfterTerminal));
    }

    #[test]
    fn the_file_ledger_validates_generation_at_the_door_and_survives_reload() {
        let dir =
            std::env::temp_dir().join(format!("familiar-workshop-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("order-file.jsonl");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("lock"));
        let ledger = Ledger::at(&path);
        let id = "order-0001-motorlights";

        ledger
            .append(
                1_000,
                id,
                EventKind::Opened {
                    order: Box::new(order_one()),
                },
            )
            .expect("open");

        // A raw append of a generation event is refused — it must be validated.
        let raw = ledger.append(
            1_001,
            id,
            EventKind::GenerationReturned {
                iteration: 1,
                outcome_digest: digest_bytes(b"x"),
                refused: false,
                capability_surface: vec!["state".into()],
                toolchain_lock: String::new(),
            },
        );
        assert!(raw.is_err());

        // A candidate whose surface exceeds the order is refused at the door.
        let mut bad = candidate();
        if let GenerationOutcome::Candidate {
            capability_surface, ..
        } = &mut bad
        {
            capability_surface.push("unlock-door".into());
        }
        assert!(ledger.append_generation(1_002, id, 1, &bad).is_err());

        // The honest candidate is accepted and survives a reload.
        ledger
            .append_generation(1_003, id, 1, &candidate())
            .expect("valid generation");
        let state = ledger.state().expect("replay from disk");
        assert_eq!(state.iteration, 1);
        assert!(!state.terminal());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn concurrent_appends_produce_one_replayable_ledger() {
        let dir =
            std::env::temp_dir().join(format!("familiar-workshop-conc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("order-conc.jsonl");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("lock"));
        let ledger = std::sync::Arc::new(Ledger::at(&path));
        let id = "order-0001-motorlights";
        ledger
            .append(
                1_000,
                id,
                EventKind::Opened {
                    order: Box::new(order_one()),
                },
            )
            .expect("open");

        let mut handles = Vec::new();
        for i in 0..8 {
            let l = ledger.clone();
            handles.push(std::thread::spawn(move || {
                l.append(
                    2_000 + i,
                    "order-0001-motorlights",
                    EventKind::Parked {
                        reason: format!("thread {i}"),
                    },
                )
            }));
        }
        for h in handles {
            h.join().unwrap().expect("each append legal");
        }

        let events = ledger.read().expect("read");
        assert_eq!(events.len(), 9); // 1 open + 8 parks
        for (i, e) in events.iter().enumerate() {
            assert_eq!(e.seq, i as u64 + 1, "sequence must be consecutive");
        }
        ledger.state().expect("replays clean");
        let _ = std::fs::remove_file(&path);
    }
}
