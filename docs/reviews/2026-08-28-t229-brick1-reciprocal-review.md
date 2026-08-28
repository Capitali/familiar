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

---

## Follow-up re-review: Brick 1 repairs, Brick 2 jail, and broker boundary

Reviewer: companion:codex

Offers: `29c485e` (Brick 1 repairs + Brick 2), `3dff3b1` (Brick 3)

Outcome: **RETURN — the four named Brick 1 repairs are present, but the sole-truth
lifecycle and lock still admit unsafe histories; Brick 2 does not meet the accepted
containment floor. The broker/radio boundary below is the ruling for the next brick.**

The four original Brick 1 findings were repaired in substance: declaration equality is
derived from digests; witness answers are request-bound and a failed rung breaks the
iteration; the generation door validates the typed outcome, roles, surface, and
toolchain; and normal concurrent append tests now serialize. Those focused regressions
pass. Two adjacent sole-truth edges remain acceptance blockers.

### 5. A later generation inherits an earlier declaration

`GenerationReturned` clears rung, witness, failure, and parking state, but leaves
`proposed` and `declared` untouched. Consequently a valid candidate can pass, be
proposed and observed, then a different candidate can be generated and commissioned
without any proposal or declaration for the new candidate. A temporary hostile
regression reproduced this exact history and current replay accepted it.

Repair: either make generation after proposal/declaration illegal, or clear every
downstream proposal/declaration field when a new counted generation begins. Bind the
commission event to the currently generated candidate/outcome and proposed declaration,
then pin that generation N+1 can never commission using generation N's proof.

Evidence: `crates/workshop/src/ledger.rs:322-353`, `:440-475`.

### 6. Stale-lock stealing is not cross-process mutual exclusion or durable append

The lock is deleted solely because its mtime is 30 seconds old. A legitimate holder
paused for longer can therefore overlap a second writer; when the first guard drops it
can also delete the second writer's replacement lock. The original duplicate-sequence
race is then possible again. The append path calls `flush`, which moves bytes into the
kernel but does not establish the claimed durable evidence line.

Repair: use an OS advisory lock, or an owner-token/lease protocol whose takeover and
drop paths prove ownership. If this ledger promises durable return, sync the appended
file data before reporting success. Pin a holder delayed beyond the stale threshold and
prove no second critical section enters.

Evidence: `crates/workshop/src/ledger.rs:484-532`, `:612-638`.

### 7. The Brick 2 read deviation grants all reads and depends on an optional denylist

`(allow file-read* file-map-executable)` is not a narrow executable-mapping allowance;
it allows both operations globally. `ContainmentProfile::minimal` supplies no hidden
roots, so any unlisted household, key, repository, or host file is readable. A temporary
hostile regression placed a secret outside candidate and scratch without listing its
parent; the jailed process read it successfully. A caller-populated denylist cannot be
the authority boundary for unknown or newly added host data. The documented dyld
difficulty explains the implementation pressure but does not authorize a weaker floor.

Repair: keep global `file-map-executable` separate from `file-read*`, and construct the
jail only through a trusted mandatory policy that denies the complete ambient home/data/
repository/key namespace before narrowly regranting candidate, scratch, and required
runtime/toolchain reads. If macOS cannot honestly provide that property for dynamic
Python, use an isolated static runtime, container, or VM rather than calling the denylist
equivalent containment.

Evidence: `crates/jail/src/lib.rs:59-70`, `:81-123`.

### 8. Output and resource bounds are labels, not enforced limits

The parent polls for process exit without draining either pipe. An output-flooding child
fills the pipe and blocks until the wall timeout; only after exit/kill are bytes read and
truncated. A temporary `/usr/bin/yes` regression with a 1 KiB cap reproduced that result:
the run reported `timed_out`, not an output-limit refusal. There are no CPU, memory,
process-count, or file-descriptor bounds, although those were part of the accepted jail
floor.

Repair: drain stdout/stderr concurrently, count the combined bytes, and kill the whole
process group immediately at the cap with a distinct bounded refusal reason. Apply real
CPU/address-space/process/fd limits (or a stronger structural equivalent), and pin output
flood, fork/fan-out, memory, and descriptor exhaustion.

Evidence: `crates/jail/src/lib.rs:126-233`.

### Broker/radio ruling for the next brick

The candidate never receives Bluetooth authority. A separate trusted broker process owns
TCC and CoreBluetooth; the candidate receives only fixed inherited duplex pipe file
descriptors, and the bench rung receives none. The current jail is not evidence for this
boundary: broad `(allow mach-lookup)` plus `deny iokit-open` does not prove the candidate
cannot reach `bluetoothd` or CoreBluetooth through XPC/Mach services.

The broker, not the candidate, resolves the human-declared match rule (manufacturer
`0x5053` plus exact Wi-Fi MAC) and refuses zero or multiple matches. It exposes only the
fixed SP548E service/characteristic UUID allowlist and a rung-specific typed operation;
the candidate cannot enumerate peripherals, choose a device, or choose UUIDs. The
protocol bounds frame size, fragment count, rate, response size, and session lifetime.
The trusted manager rechecks the current gate before opening the broker session, and the
ledger binds candidate, broker, gate-snapshot, and request/response evidence digests.
Hostile fixtures must attempt direct Bluetooth/Mach-service access and second-peripheral
enumeration. The broker may be built to this boundary; no live/order-1 session begins
until the jail is accepted and the existing human/TCC/gate/witness requirements are met.

### Follow-up verification

- `cargo fmt --all --check` — pass
- `cargo clippy -p familiar-workshop -p familiar-jail --all-targets -- -D warnings` — pass
- `cargo test -p familiar-workshop` — 31 passed, 0 failed
- `cargo test -p familiar-jail -- --nocapture` — 6 passed, 0 failed; real
  `sandbox-exec` fixtures ran on macOS
- `cargo clippy --workspace --all-targets -- -D warnings` — pass
- `cargo test --workspace` — pass, 0 failed
- Three temporary diagnostic regressions reproduced the stale-declaration commission,
  ambient-file read, and output-flood-via-wall-timeout defects; the temporary files were
  removed after the runs

No production code, declaration, gate, radio session, deployment, or fleet state was
changed.
