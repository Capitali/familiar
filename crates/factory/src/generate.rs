//! The generation seam: where a candidate comes from.
//!
//! A [`GenerationAdapter`] turns a work order (plus, on a retry, the last
//! failure's feedback) into a [`GenerationResult`] — a typed outcome and the
//! artifact bytes its manifest names. The real adapter asks the familiar's
//! reasoner (the consult seam) to write the driver and its tests; a scripted
//! adapter drives the convergence tests without a model. Either way the
//! workshop validates the outcome before it becomes executable — the adapter
//! is never trusted to have produced something valid.

use std::collections::BTreeMap;

use familiar_workshop::order::{GenerationOutcome, WorkOrder};

/// What an adapter returns: the typed outcome plus the content-addressed
/// artifact store (digest → bytes) for a candidate's files. For a refusal the
/// artifacts map is empty.
#[derive(Debug, Clone)]
pub struct GenerationResult {
    pub outcome: GenerationOutcome,
    pub artifacts: BTreeMap<String, Vec<u8>>,
}

/// The seam the factory generates through. `feedback` is `None` on the first
/// attempt and carries the previous iteration's bench failure on a retry, so
/// the reasoner can fix what the oracle rejected.
pub trait GenerationAdapter {
    fn generate(
        &self,
        order: &WorkOrder,
        feedback: Option<&str>,
    ) -> std::io::Result<GenerationResult>;
}

#[cfg(test)]
pub(crate) mod scripted {
    //! A scripted adapter for the convergence tests: it yields a queued list
    //! of results in order, recording the feedback it was handed each call so
    //! a test can assert the loop fed failures back.
    use super::*;
    use std::cell::RefCell;

    pub struct Scripted {
        pub queued: RefCell<Vec<GenerationResult>>,
        pub feedback_seen: RefCell<Vec<Option<String>>>,
    }

    impl Scripted {
        pub fn new(results: Vec<GenerationResult>) -> Self {
            Scripted {
                queued: RefCell::new(results),
                feedback_seen: RefCell::new(Vec::new()),
            }
        }
    }

    impl GenerationAdapter for Scripted {
        fn generate(
            &self,
            _order: &WorkOrder,
            feedback: Option<&str>,
        ) -> std::io::Result<GenerationResult> {
            self.feedback_seen
                .borrow_mut()
                .push(feedback.map(|s| s.to_string()));
            let mut q = self.queued.borrow_mut();
            if q.is_empty() {
                return Err(std::io::Error::other("scripted adapter exhausted"));
            }
            Ok(q.remove(0))
        }
    }
}
