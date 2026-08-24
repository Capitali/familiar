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
