# T-229 factory Brick 1 — reciprocal review

Reviewer: companion:codex  
Offer: `313e70c` (`crates/workshop`)  
Outcome: **RETURN — four state/authority boundaries need repair before acceptance**

The crate boundary is the right one, manifests refuse traversal, replay is the
source of projected state, and the ordinary happy/failure paths are well pinned.
The current API still permits histories that assert proof or authority the
events do not actually establish, though. Because this ledger is intended to be
the factory's only truth, these are acceptance blockers rather than follow-ups.

## 1. Declaration equality is caller-asserted instead of derived

`DeclarationObserved` carries both a digest and a `matches` boolean, but replay
ignores the digest and assigns `declared = matches`. A caller can therefore
append an arbitrary edited declaration with `matches: true`, then commission it.
That contradicts Round 3's exact-digest rule and makes the state projection trust
the adapter's conclusion rather than derive it.

Repair: remove the independent truth bit (or reject it when it disagrees) and
derive the transition by comparing the observed digest with the currently
proposed digest. Pin that a different digest can never become declared or
commissioned even when the incoming event claims a match.

Evidence: `crates/workshop/src/ledger.rs:73-75`, `:327-352`.

## 2. Witness and failed-rung proof can be manufactured inside one iteration

A `Witness` pass is legal without any `WitnessRequested`/`WitnessAnswered`
history. A `No` or `Unclear` answer merely clears the outstanding flag and can
be followed by `RungVerdict { Witness, pass: true }`. More generally, a failed
rung is not remembered, so the same candidate/iteration may immediately report
the same rung as passing even though the comment and accepted dialogue require a
new counted iteration after failure.

Repair: bind the exact request digest into its answer; admit a witness pass only
after `Yes` for that request; make `No` fail the iteration and `Unclear` leave it
unproved/parkable; and make every failed rung require the next generation
iteration before any rung can pass. Pin all four hostile histories.

Evidence: `crates/workshop/src/ledger.rs:52-67`, `:269-321`.

## 3. The typed generation contract is not enforced at the ledger door

`validate_outcome` ignores a candidate's `capability_surface` and
`toolchain_lock`, and it checks only path existence—not that entrypoints are
`Source` and self-tests are `SelfTest`. Separately, `Ledger::append` accepts a
bare `GenerationReturned { outcome_digest, refused }`, so the workshop can
record generation without ever validating the `GenerationOutcome` whose digest
it cites. The declared contract is consequently optional at the authority
boundary.

Repair: reject a surface outside the order, a lock different from the ordered
lock, and role-inconsistent entrypoints/tests; ensure the generation transition
can only be minted from a validated outcome (with the ordered input/toolchain
identity carried into the event). Pin each rejection. Broker identity and gate
snapshot may arrive with the adapter brick, but the Brick-1 event shape must
leave a non-optional place for the accepted proof inputs.

Evidence: `crates/workshop/src/order.rs:97-112`, `:182-209`;
`crates/workshop/src/ledger.rs:44-57`, `:401-419`.

## 4. Concurrent appends can corrupt the sole ledger truth

`append` reads, chooses `events.len() + 1`, validates, then independently opens
the file in append mode. Two callers can read the same prefix, both validate the
same next sequence, and append duplicate sequence numbers. The following replay
then fails closed forever even though both writes passed through the workshop's
public append door.

Repair: serialize the whole read/validate/write critical section with a
cross-handle/process-safe mechanism (or make a single writer structurally
unavoidable), then pin concurrent appends produce one replayable ledger with
unique consecutive sequences. Keep the evidence line durably flushed before
reporting the returned state.

Evidence: `crates/workshop/src/ledger.rs:368-420`.

## Verification

- `cargo fmt --check` — pass
- `cargo clippy -p familiar-workshop --all-targets -- -D warnings` — pass
- `cargo test -p familiar-workshop` — 23 passed, 0 failed
- `cargo clippy --workspace --all-targets -- -D warnings` — pass
- `cargo test --workspace` — pass, 0 failed

No production code, declaration, gate, deployment, or fleet state was changed.
