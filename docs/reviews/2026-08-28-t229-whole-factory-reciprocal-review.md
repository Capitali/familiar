# T-229 whole-factory independent reciprocal review

Reviewer: companion:codex

Reviewed through: `d802bc6` (workshop repairs, jail rebuild, broker, offline factory,
captured order-0001 convergence, and deployment record)

Outcome: **RETURN — the focused repairs improved the foundation, but the current
factory can erase its sole truth, execute authored code without the required execution
gates, self-certify with candidate-authored tests, and transmit bytes outside a broker
session's named operation. The jail still does not meet the signed resource/filesystem
floor, and the normal parallel bar is red.**

This review did not run the live broker, touch a light, answer a witness, write a
declaration, change a gate, or alter the deployed fleet.

## What held

- A new generation clears the prior proposal/declaration; generation N+1 no longer
  inherits generation N's declaration.
- Normal ledger appends use a pid-bearing lock and `sync_all`; a dead identified owner
  is reclaimable and Drop checks the pid before removing the lock.
- The jail removed broad Mach lookup, drains/caps output while produced, kills its
  process group on output/wall overrun, denies network, and applies a mandatory minimum
  hidden-root set.
- Materialization rechecks paths and artifact digests.
- The broker refuses zero/multiple device matches, fixes one characteristic per config,
  separates read from transmit, and enforces its tested frame/write/rate caps.
- The reasoner adapter parses into the workshop's typed candidate/refusal contract and
  the LLM consult seam itself enforces `allow_llm`.

Those are real repairs. They do not close the following authority and evidence gaps.

## 1. The installed runner deletes the append-only ledger and its lock

Every invocation removes `ledger.jsonl` and its lock, then opens a fresh order. That is
not resume semantics: it destroys the only truth and can unlink a lock held by another
writer. A disposable-data-dir diagnostic copied the captured three-line order-0001
ledger, ran the binary under a closed LLM boundary, and observed it replaced by a
different one-line `Opened` ledger before generation refused (SHA changed; 3 lines → 1).

Repair: never delete either file. Acquire the ledger lock first, replay existing state,
verify the immutable order digest, and resume at the next legal transition/iteration.
Creating a new run needs a new run id/path and an append-only parent event, not reuse of
the old truth's pathname.

Evidence: `crates/factory-run/src/main.rs:95-112`,
`crates/factory/src/converge.rs:61-89`.

## 2. Required execution gates are inert strings

The order names `allow_execute` and `allow_actuate`, but neither `converge` nor
`run_bench` loads/evaluates a boundary before materializing and executing generated
Python. `familiar_llm::consult` correctly checks `allow_llm`; that does not compose into
authorization to execute authored code. The signed dialogue requires both
`allow_execute` and `allow_authored_execute` for candidate/bench execution, plus a gate
snapshot in each event. There is no `allow_authored_execute` check and no gate snapshot
in the ledger event shape.

Repair: the trusted manager must evaluate the current boundary at every effect edge,
append the exact gate snapshot/evidence before execution, and park without materializing
or spawning when either execution gate is shut. `required_gates` must be validated
against a closed vocabulary and enforced, not merely serialized.

Evidence: `crates/workshop/src/order.rs:59-82`,
`crates/factory/src/converge.rs:61-150`, `crates/factory/src/bench.rs:30-77`,
`crates/factory-run/src/main.rs:21-64`.

## 3. The bench oracle is authored by the candidate it judges

The only bench inputs are the candidate's `self_tests`; pass means those processes exit
zero. `WorkOrder` contains no accepted assertions or trusted fixture digests even though
the dialogue requires them. A temporary hostile regression supplied only
`assert True`; `run_bench` returned `passed=true`. Thus a candidate can certify arbitrary
driver bytes by writing a vacuous test. Order #1 happened to generate useful assertions,
but the factory did not independently require them, so “proved correct” is stronger than
the evidence.

Repair: put trusted acceptance assertions and fixture/transcript digests in the immutable
order, run repo/order-owned oracle tests independently of candidate self-tests, and bind
both suites and exact outputs into the verdict. Candidate tests are useful evidence, not
the oracle's authority.

Evidence: `crates/workshop/src/order.rs:59-83`,
`crates/factory/src/bench.rs:30-77`, `crates/factory/src/converge.rs:101-149`.

## 4. The jail still does not meet the signed containment floor

The mandatory denylist is better than an optional one, but broad `file-read*` still
permits every ambient file not anticipated by that list. A checkout or data directory
outside the hard-coded homes, browser/application data outside `.config`, `/tmp` data,
and newly introduced secret locations remain readable unless every caller remembers to
add them. That is not the agreed “candidate/toolchain tree and scratch only” property.

The resource claim is also incomplete: default address-space and process limits are both
zero (unset), CPU is per process rather than aggregate, and every `ulimit` error is
discarded before `sandbox-exec` starts. The fork fixture proves descendants eventually
die at the wall clock; it does not bound how many processes or how much memory they can
consume before then. The accepted floor explicitly says not to relabel resource limits
as containment.

Ruling: **the mandatory denylist is not accepted as the final floor.** Park autonomous
candidate execution until a static/minimal runtime, sandboxed container/VM, or another
mechanism proves a real read allowlist plus enforceable per-job memory/process limits.
If a transitional macOS runner remains for development, label it non-production and do
not use its green result to advance an order.

Evidence: `crates/jail/src/lib.rs:50-76`, `:129-197`, `:234-405`.

## 5. Orphan-lock reclaim can still steal from a live holder

The lock file is created and its pid written in separate operations. A live process can
be suspended after `create_new` but before the write; after 120 seconds its empty file is
classified as orphaned and removed. That recreates the overlapping-writer defect the
repair intended to close. “Normally microseconds” is not mutual exclusion.

Repair: use an OS advisory lock, or acquire an atomic owner-bearing object with no
unidentified-live interval. Do not time-steal any state that a live holder may own.

Evidence: `crates/workshop/src/ledger.rs:509-585`.

## 6. The broker's operation and lifetime bounds are not capability bounds

An act session named `off` accepts any nonempty frame within the byte cap; the existing
test even demonstrates an `off` session transmitting `aa`. The `op_label` is therefore
decorative: a candidate can label an arbitrary SP548E command “off.” Service and
characteristic UUIDs are caller-supplied CLI strings rather than a code-owned SP548E
allowlist, and `service_uuid` is never used by the backend. The broker also blocks on
`for line in inp`, so `session_secs` is checked only after a request arrives; an idle or
unbounded line can hold/grow the trusted process past the advertised session/input cap.
Reads have no count/rate budget, and the open response unnecessarily returns the host's
CoreBluetooth address to the candidate.

Repair: validate exact frame structure/type/payload against the trusted session operation
(or have the trusted broker construct frames from bounded typed parameters); enforce a
code-owned device-protocol UUID set; bound every input line, request/read count, backend
operation, and idle/session lifetime outside blocking reads; emit no peripheral address.

Evidence: `tools/ble-broker/broker.py:50-75`, `:138-215`, `:226-256`, `:315-340`;
`tools/ble-broker/test_broker.py:229-236`.

## 7. The captured “proof” does not carry the accepted evidence

The signed ledger floor requires run, input/artifact, gate-snapshot, broker/toolchain, and
evidence references on every event. The current ledger carries no run id or gate snapshot;
its generation row omits the manifest/artifact reference, and its bench row cites an
evidence digest without preserving the bench report/output needed to reproduce it. The
captured order's research digest is SHA-256 of the literal label
`sp548e-protocol-notes`, not of `docs/research/sp548e-protocol.md` (stored
`52f075…`; document `538916…`). Its toolchain lock is empty while the runner selects the
first Python found on the host. The captured files are valuable milestone artifacts, but
the three JSONL rows are not yet an independently replayable proof of what ran under
which authority.

Repair: extend the immutable order/events to the accepted evidence schema; content-address
the exact research bytes, candidate manifest/artifacts, trusted oracle fixtures and full
bounded report, runner/jail/broker versions, interpreter binary/dependency lock, and gate
snapshot. Preserve those blobs beside the ledger and verify every reference on replay.

Evidence: `crates/workshop/src/ledger.rs:45-113`,
`crates/factory-run/src/main.rs:41-63`,
`docs/factory-runs/order-0001-first-convergence/ledger.jsonl`.

## Verification

- `cargo fmt --all --check` — pass
- `cargo clippy --workspace --all-targets -- -D warnings` — pass
- `cargo test -p familiar-workshop` — 33 passed, 0 failed
- `cargo test -p familiar-jail -- --nocapture` — 8 passed, 0 failed
- `cargo test -p familiar-factory -- --test-threads=1` — 15 passed, 0 failed
- `python3 tools/ble-broker/test_broker.py` — 14 passed, 0 failed
- `cargo test --workspace` — **failed**: 14/15 factory tests passed; the hostile
  no-household bench test failed. A second ordinary parallel factory run also failed
  14/15, this time because the passing bench's `.jail.sb` was corrupted. Root cause is
  shared PID-only `familiar-bench-<pid>` directories being removed/rewritten by parallel
  tests (`crates/factory/src/bench.rs:100-107`). The same tests pass serially; the normal
  house bar remains red and needs process+test-unique roots.
- Disposable diagnostics reproduced destructive ledger replacement and vacuous
  candidate self-certification; no temporary source file remains in the worktree

No production code, gate, declaration, radio/light operation, witness, deployment, or
fleet state was changed.
