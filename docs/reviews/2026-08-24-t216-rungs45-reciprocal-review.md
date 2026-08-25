# T-216 rungs 4/5 reciprocal review — RETURN

- **Reviewer:** companion:codex
- **Reviewed:** `87a32ea` (`familiar.observe` / `familiar.invoke`)
- **Disposition:** **RETURNED before deployment or live exercise**
- **Scope:** review only; no production code changed

## What holds

The new dependency seam is narrow and fail-closed: `crates/mcp` owns authorization and an
abstract executor trait; only the daemon wires the cycle implementation; other processes have no
executor. Public receipts do not serialize the local surface, action label, command, or raw device
output. Principal-bound covenant, opaque grant handle, grant expiry, operation membership,
parameter bounds, declaration shape, and `allow_actuate` are all checked before the physical act,
and the executor reaches the existing reviewed declared-tool path. Observe and invoke are distinct
grant legs. The added hostile fixtures genuinely pin several of those properties.

That is a credible execution seam. It is not yet the accepted ADR-0044 execution contract.

## Blocking findings

### 1. Revocation is not immediate, and the success ledger can corrupt itself in the race

`authorized_grant` folds a snapshot and returns it (`grant.rs:698-727`). The physical read/act
happens later (`grant.rs:747`, `830`), and only after success does `record_effect` attempt a fresh
append (`grant.rs:753-763`, `832-845`). A concurrent human revoke can therefore append after the
snapshot but before the executor: the physical act lands after revocation. If the revoke lands
before the effect append, `validate_sequence` then rejects the effect-after-terminal ordering
(`partner_act.rs:410-420`, `452-470`), so the best-effort append can turn the durable partner
ledger into a stream that every later load refuses.

Worse, `record_effect` deliberately discards random-id, event-construction, and SQLite append
failures. A real physical act can therefore return success with no durable event at all. That
directly contradicts ADR-0044's immediate revocation and "never erase the act record or hide a
successful act" requirements.

Repair needs one serialized authority/effect protocol, not another preflight check. The human
terminal transition and partner effect reservation must contend on the same durable transaction
or grant-epoch lock. An effect should have a durable, idempotent intent/reservation before the
executor runs and a typed completed/failed outcome after it; a completed physical act whose
outcome cannot be persisted must enter an explicit recovery state, never disappear.

### 2. The grant omits two authority dimensions the accepted ADR makes mandatory

ADR-0044 says invoke is bounded by **parameter, time, rate, and affected-subject bounds**
(`0044:62-67`). This build checks parameters and expiry, but the grant carries no rate bound and
no affected-subject bound. The existing transport bucket is not a substitute: it is a generic,
process-local admission limit shared by reads and metadata calls, currently allowing a 30-call
principal burst (`partner.rs:400-443`). It neither expresses the human's per-grant choice nor
prevents a partner from hammering one physical surface.

Do not expose rung 5 until the grant decision and durable grant view carry both dimensions and
the executor enforces them at the final floor. At minimum, the v1 class needs a human-visible
maximum call cadence/burst for the grant and an affected-subject class compatible with the
declared surface; the private resolver must refuse when the current surface no longer satisfies
that intersection.

### 3. `invoke` has no replay/idempotency identity

`InvokeInput` contains only instance, operation, and parameters. A partner that times out after
the tool ran cannot safely retry: the same JSON-RPC call runs the device again and creates another
effect. The opaque instance is stable partly because ADR-0044 requires idempotency, but it cannot
identify an individual invocation. This is especially unsafe once the catalog grows beyond an
idempotent-looking two-state setter.

Require a bounded partner `invoke_key` and hash the complete typed payload under principal + grant
epoch + operation. The first admitted call reserves that key durably; an exact replay returns the
original receipt, and a changed payload is an idempotency-conflict refusal. Add the hostile
timeout/retry fixture before the edge can go live.

### 4. Successful partner effects are not narrated to the authorized human

The implementation appends only private `Observed` / `Invoked` ledger bodies; no inbox,
observation, console, or narration consumer reads them. `partner_run_act` explicitly writes no
household observation (`cycle:3722-3750`). Thus a successful remote act can be physically visible
but systemically silent. ADR-0044 requires every observation and invocation to be auditable **and
narrated**, naming the human-chosen partner alias + fingerprint and the local surface to the
authorized human (`0044:75-81`).

The durable effect outcome should drive a private, addressed narration projection. It must be
deduplicated by the invocation identity, never enter worldview/federation/MCP output, and never
attribute the partner's act to a human.

## Flagged resolver choice

Bucket order is not an adequate meaning boundary. `primary -> buckets[0]` and
`reverted -> buckets[1]` (`grant.rs:874-880`) is stable JSON ordering, but neither the declaration
nor the grant card tells the human which physical action those abstract words mean. Human authorship
of an array is not informed assent to a later abstract grant. Make the semantic role explicit in
the declaration (and validate a complete one-to-one pair), then snapshot or fingerprint that
resolver into the grant so a declaration reorder cannot silently change an active grant's meaning.
The same explicit map must drive the reverse observe projection.

## Verification

Independently reproduced on clean `87a32ea`:

- `cargo fmt --all -- --check` — exit 0
- `cargo clippy --all-targets -- -D warnings` — exit 0
- `cargo test --workspace` — exit 0; 813 passed, 0 failed

The green bar establishes implementation consistency. The return is for missing accepted
authority invariants at the first live partner execution edge. No gate was opened, and this review
authorizes no deployment or live grant/invocation.

## Re-review of `6ee4499` — RETURN

- **Reviewer:** companion:codex
- **Disposition:** **RETURNED before deployment or live exercise**
- **Scope:** reciprocal re-review of the five repairs only; no production code changed

Four repairs now have the right durable shape. Grants snapshot a per-grant invoke rate,
affected-subject class, and an explicit one-to-one semantic role map; current declarations must
still satisfy that snapshot. Effect intent precedes execution, settlement is durable rather than
best-effort, and an unsettled reservation is an explicit recovery state. The private addressed
inbox now narrates alias, credential fingerprint, local surface, effect, and outcome without
entering partner output or worldview. `invoke_key` is required and exact-payload conflicts are
durably refused. Those changes close the old rate/subject, narration, and bucket-order findings,
and close the ledger-corruption / silently-lost-act halves of finding 1.

Two execution-order invariants remain blocking.

### 1. An acknowledged revoke can still be followed by the reserved physical effect

The new lock ends when the reservation is appended (`grant.rs:854-925`). The executor then runs
outside it (`grant.rs:940`), while `revoke_grant` needs only that same lock to append and return
success (`grant.rs:448-490`). Therefore this legal ordering remains:

1. invoke reserves while the grant is live and releases the lock;
2. the human revokes, the terminal append succeeds, and the revoke is acknowledged;
3. the blocked executor resumes and changes the physical surface;
4. settlement is valid after terminal.

The stream remains honest and loadable now, which is real progress, but revocation is not
immediate at the physical boundary: an effect may first become visible after the human was told
the authority ended. The existing revoke fixture is sequential (invoke completes, then revoke)
and does not exercise this ordering.

Repair needs an acknowledgement fence. A revoke must either cancel every reserved-but-unexecuted
effect, or wait until all reservations that won before it have settled before it returns success;
the executor must not be able to begin/complete after a successful terminal acknowledgement.
Pin this with a blocking executor fixture: reserve, start revoke concurrently, prove revoke cannot
return success while the physical act can still land, then prove no new reservation enters.

### 2. Exact invoke replay is downstream of mutable authority and rate checks

The implementation promises that a timed-out exact retry returns the original receipt, but it
does not look up the invocation identity first. `invoke` currently re-runs grant liveness,
parameter, `allow_actuate`, hourly-rate, and resolver checks (`grant.rs:854-906`) before
`append_idempotent` can report `Replay` (`grant.rs:907-936`, `1026-1035`). Consequently an exact
retry is refused rather than replayed when any current condition changed. The simplest witness is
a grant capped at one invoke per hour: the first call consumes the reservation; its exact retry
hits `invokes_in_last_hour >= 1` and never reaches the replay branch. Closing `allow_actuate`,
revoking/expiring the grant, or changing the declaration produces the same defect.

This is not merely a friendlier retry policy. A caller that lost the first response cannot learn
whether the device ran, which is the ambiguity the idempotency identity exists to remove. The
current replay fixture uses the default cap of 12 and leaves every mutable gate unchanged, so it
does not cover the contract it names.

Repair by deriving the stable idempotency namespace and payload hash from the opaque handle before
current admission, then return the original recorded outcome for an exact key+payload replay.
Only a genuinely new key should pass through current liveness, boundary, rate, bounds, and
resolver checks. Pin exact replay after (a) a one-per-hour first invoke, (b) revoke/expiry, and
(c) boundary closure; all must avoid the executor and return the original completed/failed result.

### Typed-outcome correction carried with the repair

`settle_effect` constructs every settlement as `PartnerOperation::Invoke` (`grant.rs:1047-1073`),
including settlements called by `observe` (`grant.rs:814`). The reservation/narration kind remains
correct, so this does not create a third independent authority hole, but the durable audit event
misclassifies every observation outcome. Have settlement carry the reserved operation/kind (and
validate it against the reservation) while repairing the protocol above.

The compatibility fallback in `grant_roles` also deserves removal before any migration can mint
legacy-shaped authority: an empty role snapshot currently follows the live declaration, contrary
to the stable-snapshot rule. Since this edge has not been deployed and no live grants exist, the
safe migration is to fail such a grant closed rather than reinterpret it.

## Re-review verification

Independently reproduced on clean `6ee4499`:

- `cargo fmt --all -- --check` — exit 0
- `cargo clippy --all-targets -- -D warnings` — exit 0
- `cargo test --workspace` — exit 0; 818 passed, 0 failed

The bar is green and the repaired ledger is materially safer. The edge remains inert: no gate was
opened, no live principal/grant/effect was exercised, and this re-review authorizes no deployment.

## Re-review of `c76ad99` — RETURN

- **Reviewer:** companion:codex
- **Disposition:** **RETURNED before deployment or live exercise**
- **Scope:** reciprocal re-review of the Round-3 ordering repairs only; no production code changed

The acknowledgement fence now holds. Observe and invoke retain the same authority lock through
reservation, physical execution, and settlement; revoke needs that lock and cannot acknowledge
while an effect can still land. The new blocking-executor fixture exercises the ordering the prior
fixture missed. Typed settlement call sites now classify observe as observe, and empty resolver
snapshots fail closed. Those Round-2 findings are closed.

The replay protocol is closer, but two concurrent orderings still make its public answer false or
mutable. They remain blocking at the first live physical edge.

### 1. An in-flight replay returns success before any outcome exists

The pre-admission lookup is intentionally outside `authority_lock` (`grant.rs:869-886`). As soon as
the first call appends its reservation, an exact retry can find it while the first executor is
still blocked. `replay_outcome` maps an absent settlement to the same success-shaped receipt as a
completed act (`grant.rs:989-1018`). The retry can therefore report “applied” before the device has
acted, and can report success even if the still-running executor later fails.

An unsettled reservation is an explicit **unknown/recovery** state, not a recorded successful
outcome. An exact retry must wait behind the in-flight authority fence and reload the settlement;
if the process died and only the reservation remains, it must return an explicit indeterminate
recovery result/refusal, never a completed receipt and never run the device again. Pin this with a
blocking executor whose retry arrives after reservation but before release, once for a later
success and once for a later executor failure.

### 2. Two first-seen identical calls can still pass replay behind mutable admission

Two same-key calls can both perform the initial indexed lookup before either has reserved. The
first acquires the lock, reserves, executes, settles, and releases. The second then acquires the
lock but goes directly through liveness, boundary, rate, bounds, and resolver checks
(`grant.rs:893-952`) before `reserve_effect` can discover the replay (`grant.rs:953-980`). With a
one-per-hour grant, that exact concurrent retry is refused by the rate already consumed by the
first call. A revoke or gate transition that wins the lock next can similarly change its answer.

Repeat the idempotency lookup immediately after acquiring `authority_lock`, before every mutable
admission check. That closes the lookup/reservation race: only a genuinely absent key continues to
authorization. Pin two synchronized first-seen same-key calls under a one-per-hour grant and prove
both receive the recorded outcome while the executor runs once.

### Durable audit corrections still incomplete

The fast pre-admission changed-payload branch returns `IdempotencyConflict` directly
(`grant.rs:878-885`), bypassing `append_idempotent`'s conflict append. The refusal is therefore no
longer durable even though every attempted invocation must remain auditable. Record that conflict
without subjecting it to current admission, and extend the existing changed-payload test to assert
the typed refusal event.

Also, Round 2 asked the sequence validator to validate a settlement against its reservation. The
call sites now pass the right operation, but `validate_sequence` still records only
`effect_id -> principal` and accepts an observe reservation followed by an invoke settlement (or
vice versa). Fold the reserved operation/kind with the principal and require the settlement's
operation and typed outcome to match. This is not a new public authority path, but it is required
for the append-only audit stream to fail closed on the exact impossible history this correction
was meant to exclude.

## Round-3 verification

Independently reproduced on clean `f1e4125` (Round-3 code at `c76ad99`):

- `cargo fmt --all -- --check` — exit 0
- `cargo clippy --all-targets -- -D warnings` — exit 0
- `cargo test --workspace` — exit 0; 821 passed, 0 failed

The acknowledgement fence and resolver correction are accepted. The edge remains inert: no gate
was opened, no live principal/grant/effect was exercised, and this review authorizes no deployment.
