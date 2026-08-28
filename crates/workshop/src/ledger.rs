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

use serde::{Deserialize, Serialize};

use crate::order::{validate_order, OracleRung, WorkOrder};

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
    /// A generation adapter returned. Iterations are counted from 1 and must
    /// arrive in order; a refused outcome is recorded, not erased.
    GenerationReturned {
        iteration: u32,
        outcome_digest: String,
        refused: bool,
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
    /// The human answered that exact request.
    WitnessAnswered {
        iteration: u32,
        answer: WitnessAnswer,
    },
    /// The order parked (gate shut, jail unavailable, witness outstanding…).
    /// Parking is not terminal; work resumes with the next legal event.
    Parked { reason: String },
    /// The factory proposed an exact declaration for the human's hand.
    /// `reduced` means unproved operations were excluded from the proposal.
    DeclarationProposed { digest: String, reduced: bool },
    /// The workshop independently observed the on-disk declaration.
    DeclarationObserved { digest: String, matches: bool },
    /// Post-declaration, post-restart smoke pass through the declared
    /// command. Terminal success.
    Commissioned { evidence_digest: String },
    /// Terminal close without commissioning (refusal stands, order
    /// withdrawn, subject gone). The reason is the record.
    Closed { reason: String },
}

/// The state replay derives. Deliberately small: rung progress lives in
/// [`OrderState::iteration`]/[`OrderState::rungs_passed`], not in extra
/// states.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderState {
    pub order: WorkOrder,
    /// 0 until the first generation returns.
    pub iteration: u32,
    /// Whether the current iteration's outcome was a refusal.
    pub refused: bool,
    /// Rungs passed by the current iteration, in plan order.
    pub rungs_passed: Vec<OracleRung>,
    /// An outstanding witness request awaiting its answer.
    pub witness_outstanding: bool,
    pub parked: Option<String>,
    /// Digest of a proposed declaration, once proposed.
    pub proposed: Option<String>,
    /// True once the on-disk declaration matched the proposal.
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
    DuplicateRung(OracleRung),
    WitnessNotRequested,
    WitnessAlreadyOutstanding,
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
            LedgerError::DuplicateRung(r) => write!(f, "rung {r:?} judged twice this iteration"),
            LedgerError::WitnessNotRequested => write!(f, "witness answer without a request"),
            LedgerError::WitnessAlreadyOutstanding => {
                write!(f, "second witness request while one is outstanding")
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
                witness_outstanding: false,
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
            iteration, refused, ..
        } => {
            let expected = state.iteration + 1;
            if *iteration != expected {
                return Err(LedgerError::IterationOutOfOrder {
                    expected,
                    got: *iteration,
                });
            }
            state.iteration = *iteration;
            state.refused = *refused;
            state.rungs_passed.clear();
            state.witness_outstanding = false;
            state.parked = None;
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
                state.rungs_passed.push(*rung);
            }
            // A failed rung leaves rungs_passed as it was: the next legal
            // step is another generation iteration or a park, and replay
            // shows exactly which rung refused to close.
            Ok(())
        }
        EventKind::WitnessRequested { iteration, .. } => {
            if *iteration != state.iteration || state.iteration == 0 {
                return Err(LedgerError::RungWithoutCandidate);
            }
            if state.witness_outstanding {
                return Err(LedgerError::WitnessAlreadyOutstanding);
            }
            state.witness_outstanding = true;
            Ok(())
        }
        EventKind::WitnessAnswered { iteration, .. } => {
            if !state.witness_outstanding || *iteration != state.iteration {
                return Err(LedgerError::WitnessNotRequested);
            }
            state.witness_outstanding = false;
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
        EventKind::DeclarationObserved { matches, .. } => {
            if state.proposed.is_none() {
                return Err(LedgerError::ObservationBeforeProposal);
            }
            // A diverged declaration is recorded but does not advance the
            // order — the changed surface must be revalidated upstream.
            state.declared = *matches;
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

/// The on-disk ledger: one JSON event per line, append-only. Append
/// revalidates the whole file plus the new event before a byte is written,
/// so an illegal transition can never reach disk through this door.
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

    /// Replay the file (empty is only legal for a first append, which must
    /// open the order), validate the new event against the proven state,
    /// then append one line.
    pub fn append(&self, at: u64, order_id: &str, kind: EventKind) -> Result<OrderState, String> {
        let mut events = self.read()?;
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
        Ok(state)
    }

    pub fn state(&self) -> Result<OrderState, String> {
        replay(&self.read()?).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::digest_bytes;
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

    fn generation(seq: u64, iteration: u32) -> LedgerEvent {
        ev(
            seq,
            EventKind::GenerationReturned {
                iteration,
                outcome_digest: digest_bytes(b"candidate"),
                refused: false,
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

    #[test]
    fn the_whole_happy_path_replays_to_commissioned() {
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
                    request_digest: digest_bytes(b"strip is red?"),
                },
            ),
            ev(
                7,
                EventKind::WitnessAnswered {
                    iteration: 1,
                    answer: WitnessAnswer::Yes,
                },
            ),
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
                    matches: true,
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
    fn a_failed_rung_loops_back_through_another_iteration() {
        let events = vec![
            opened(),
            generation(2, 1),
            verdict(3, 1, OracleRung::Bench, false),
            generation(4, 2),
            verdict(5, 2, OracleRung::Bench, true),
        ];
        let s = replay(&events).expect("legal history");
        assert_eq!(s.iteration, 2);
        assert_eq!(s.rungs_passed, vec![OracleRung::Bench]);
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
    fn sequence_breaks_fail_closed() {
        let mut e2 = generation(3, 1);
        e2.seq = 3;
        assert_eq!(
            replay(&[opened(), e2]),
            Err(LedgerError::BadSequence {
                expected: 2,
                got: 3
            })
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
    fn commissioning_requires_a_matching_observed_declaration() {
        let mut events = vec![
            opened(),
            generation(2, 1),
            verdict(3, 1, OracleRung::Bench, true),
            verdict(4, 1, OracleRung::Read, true),
            verdict(5, 1, OracleRung::Act, true),
            ev(
                6,
                EventKind::DeclarationProposed {
                    digest: digest_bytes(b"d"),
                    reduced: true,
                },
            ),
        ];
        events.push(ev(
            7,
            EventKind::Commissioned {
                evidence_digest: digest_bytes(b"smoke"),
            },
        ));
        assert_eq!(
            replay(&events),
            Err(LedgerError::CommissionBeforeDeclaration)
        );

        events.pop();
        events.push(ev(
            7,
            EventKind::DeclarationObserved {
                digest: digest_bytes(b"edited"),
                matches: false,
            },
        ));
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
    fn witness_answers_require_an_outstanding_request() {
        let events = vec![
            opened(),
            generation(2, 1),
            ev(
                3,
                EventKind::WitnessAnswered {
                    iteration: 1,
                    answer: WitnessAnswer::Yes,
                },
            ),
        ];
        assert_eq!(replay(&events), Err(LedgerError::WitnessNotRequested));
    }

    #[test]
    fn the_file_ledger_appends_validates_and_survives_reload() {
        let dir =
            std::env::temp_dir().join(format!("familiar-workshop-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("order-0001.jsonl");
        let _ = std::fs::remove_file(&path);
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
        ledger
            .append(
                1_001,
                id,
                EventKind::GenerationReturned {
                    iteration: 1,
                    outcome_digest: digest_bytes(b"candidate"),
                    refused: false,
                },
            )
            .expect("generation");
        // An illegal event is refused AND leaves no line behind.
        let before = std::fs::read_to_string(&path).unwrap();
        let err = ledger.append(
            1_002,
            id,
            EventKind::RungVerdict {
                iteration: 1,
                rung: OracleRung::Act,
                pass: true,
                evidence_digest: digest_bytes(b"evidence"),
            },
        );
        assert!(err.is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);

        let state = ledger.state().expect("replay from disk");
        assert_eq!(state.iteration, 1);
        assert!(!state.terminal());
        let _ = std::fs::remove_file(&path);
    }
}
