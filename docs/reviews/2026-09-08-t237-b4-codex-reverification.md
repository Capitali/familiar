# T-237 B4 one-doctrine/two-runtimes codex re-verification

**Verdict: REJECT.** The pure Rust seam is linked into UCFFamiliar and both the Rust
and Swift suites remain green, but the three acceptance lines do not hold end to end.
The iPad artifact predates the host's hull-specific rung quotes and its `TableRouter`
cannot represent them; a boundary fixture makes the two runtimes choose different
decisions. Direct mode also loses an active contract by passing only its id beside the
open board, and there is no captain-tap path from the displayed verdict to an act.

Reviewer: companion:codex
Reviewed: B4 steps `d89787ad`, `3f0da155`, `a98e216b`, `c3c244b1`; direct-mode
follow-up `f239e1ce`; claim `f364c188`; current main `ebb337b9`
Post-landing drift checked: `f2fc51e3` and `ebb337b9` changed the host's doctrine
contract after the last FamiliarCore.xcframework rebuild.

## Findings

### 1. Blocker — the host consumes hull-specific rung quotes that the iPad seam and shipped artifact cannot represent

**File:** `crates/whisker/src/main.rs:173-203`;
`crates/whisker/src/doctrine.rs:238-278`;
`crates/whisker/src/wire.rs:165-213`;
`ios/FamiliarCore/FamiliarCore.xcframework`

**Failure scenario.** Since `f2fc51e3`, the host `Wire` implements
`Router::quote_at_burn` by asking
`/v1/route?from=…&to=…&hull=me&serviceClass=…`. `reachable_pump` treats an answer
from the world as authoritative and deliberately refuses to fall back to its model
when the world says neither rung reaches. The JSON seam still accepts only one
reference `fuel` value plus `legs_km`; `TableRouter` has no `quote_at_burn`
implementation. The app therefore always uses the model. The checked-in
FamiliarCore.xcframework was last rebuilt at `a98e216b`, before `f2fc51e3`, so the
actual iPad artifact also predates the new rule.

While this review was in flight, `ebb337b9` added a second decision input: the
world's tick and each load's delivery deadline. Current host source rejects a load
that cannot land by that deadline; the checked-in iPad library still contains the
pre-deadline doctrine. The raw JSON happens to carry those fields, but an old static
library cannot begin using them. Thus rebuilding only after the route repair is not
optional, and the artifact/version skew needs a guard rather than a manual promise.

A focused transient regression probe used the exchange numbers already pinned in
`the_exchanges_rung_quote_beats_the_model`: a 188 mG hull at
`titania-cold-store`, 123 fuel, the pump at `foxys-diner`, reference fuel 168 and
legs `[3491917000, 1774626]`; the exchange says standard `(171, 67)` and economy
`(114, 95)`. With the doctrine's ten-percent reserve, the host says **CallPaws**
(economy needs 125), while `wire::advise` models economy as 112, rounds its reserve
to 123, and says **DivertToPump/economy**. The probe asserted those exact decisions
and passed. This is the acceptance line's prohibited same-facts/different-decision
case, at a safety boundary.

**Repair accepted.** Put every world fact that can affect a doctrine decision in the
seam. For routes, carry the hull-specific standard/economy quotes and implement
`TableRouter::quote_at_burn`; do not silently model when the host had an authoritative
answer. Rebuild both xcframework slices after every doctrine change. Pin one parity
test that runs the host adapter and the checked-in FFI artifact over the same JSON,
including this reachability boundary, and compares the complete decision.

### 2. Blocker — direct mode turns every active contract into `None`

**File:** `ios/FamiliarSC/Sources/FamiliarSCUI/DirectFeed.swift:193-227`;
`crates/whisker/src/wire.rs:235-249`;
`ios/FamiliarSC/Tests/FamiliarSCTests/Fixtures/wire/loadboard-open.json`;
`ios/FamiliarSC/Tests/FamiliarSCTests/Fixtures/wire/loadboard-mine.json`

**Failure scenario.** DirectFeed fetches the open board and the captain's own board
separately. It passes the open board as `board`, but reduces the mine response to only
`active_load_id`. `wire::advise` can construct `Active` only by finding that id in
`board`; if the row is absent, the `?` at the lookup returns `None` before the ledger
is read. The repository's own fixtures exhibit the ordinary shape: the open fixture
contains only `status: open` rows, while `L3249` is the mine fixture's
`status: inTransit` row and is absent from the open fixture. Passing `L3249` therefore
does not preserve it.

The host does the opposite: it keeps the owned active row and supplies an empty open
board while a contract is active. The iPad can consequently describe a travelling or
delivered hull as freight-idle and produce a hold or a new-load judgment instead of
the host's travel/collect decision.

**Repair accepted.** Carry the active load row as its own JSON object (or merge the
selected mine row into the doctrine input without making it a bookable board row),
then build `Active` from that object plus `/v1/me.freight`. Pin the checked-in open,
mine, and me fixtures through the Swift gather and Rust seam and prove `L3249` remains
active and produces the same decision as the host adapter.

### 3. Blocker — the displayed direct-mode verdict has no captain-confirm act path

**File:** `ios/FamiliarSC/Sources/FamiliarSCUI/Feed.swift:177-190`;
`ios/FamiliarSC/Sources/FamiliarSCUI/DirectFeed.swift:282-296`;
`ios/FamiliarSC/Sources/FamiliarSCUI/MessageWindowView.swift:16-34`, `:52-87`;
`ios/FamiliarSC/Sources/FamiliarSCUI/Briefs.swift:88-119`

**Failure scenario.** `CaptainActs` has no operation that accepts a doctrine verdict
or a typed exchange action. DirectFeed's proposal approval and dial operations both
throw `needsHost`; its other acts are pairing/settings operations. The only Approve
button is for a host pilot's journal proposal in `MessageWindowView`. The direct
pilot verdict is merely a context document consumed by the voice and has no button,
no idempotency key, no typed conversion to `/v1/actions`, and no POST. Thus the app
does refuse to act without a tap, but it also refuses to act *with* the captain's tap.
The recorded scope and acceptance require direct mode to let the captain confirm the
displayed act under the act scope.

**Repair accepted.** Define a typed, allowlisted conversion from actionable doctrine
decisions to exchange actions and expose it only behind an explicit confirmation UI.
Re-read the relevant wire facts before filing and refuse if the decision changed;
generate and retain one idempotency id for the confirmed action. A mock-wire test must
prove zero POSTs while merely rendering or speaking, zero POSTs on cancellation, and
exactly one correctly shaped POST after the captain taps confirm.

### 4. Blocker — actionable verdicts do not carry the promised reasons

**File:** `crates/whisker/src/wire.rs:148-162`, `:216-270`;
`ios/FamiliarSC/Sources/FamiliarSCUI/Briefs.swift:88-119`

**Failure scenario.** Only `Decision::Hold` serializes a `why`. Refuel, repair,
CallPaws, divert, book, travel, and collect return a type and identifiers but no
reason or evidence. `Briefs.pilot` turns those fields into a sentence describing
*what* would happen, followed by the surface and dial level; it cannot explain why a
load beat its alternatives, why repair outranked work, or why a pump/rung was chosen.
The B4 scope explicitly asks for decision + automation + reasons, and acceptance says
direct mode shows the decision with reasons. The current test only asserts the generic
action sentence and safety footer, so it cannot catch the missing contract.

**Repair accepted.** Make explanation part of the pure result for every decision:
stable reason code plus the bounded numeric facts that determined the branch. Keep
the prose in Swift, but render those facts rather than inventing a generic rationale.
Pin at least booking, repair, reachable-pump, and PAWS outputs in Rust and their Swift
renderings.

### 5. Should-fix — direct mode silently reports the default dial as the captain's setting

**File:** `ios/FamiliarSC/Sources/FamiliarSCUI/DirectFeed.swift:225-227`, `:284-285`;
`crates/whisker/src/wire.rs:250-261`; `crates/whisker/src/autonomy.rs:161-187`

**Failure scenario.** DirectFeed does not include `dial` in its input and cannot read
or set a host dial. `wire::advise` treats an absent field as `Dial::default`, which is
auto on most surfaces and advise for rescue/margin, then labels that value as the
captain's level. A direct-mode captain who has never authorized auto is therefore told
"the captain's setting is auto" even though this device has no such setting and no
pilot to obey it.

**Repair accepted.** Either give direct mode a captain-owned policy that is actually
read and enforced by its confirm path, or make the level explicitly unavailable and
omit all claims about the captain's setting. Never turn missing governance data into
an authorization-shaped default.

## What held

- The decision crates no longer own filesystem I/O. A focused search of
  `autonomy.rs`, `trade.rs`, `chain.rs`, `outfit.rs`, `doctrine.rs`, and `wire.rs`
  found no `std::fs`, `File`, or `OpenOptions`; persisted dial, holdings, and grants
  live behind `whisker::store`.
- `whisker_advise` is exported through UniFFI, appears in both xcframework headers and
  the generated Swift binding, links into UCFFamiliar, and performs no exchange act.
- A failed direct gather now remains a named pilot document rather than disappearing.
  The model still has no write surface, and this review performed no game action.

## Verification

- `cargo fmt --all -- --check`: **pass**.
- `cargo test -p familiar-whisker`: **78 passed, 0 failed** (77 library + 1 binary).
- `cargo test -p familiar-core-ffi`: **pass** (crate and doc tests; no unit cases).
- `cd ios/FamiliarSC && swift test`: **55 passed, 0 failed, 2 live tests skipped**;
  Swift 6 Sendable warnings remain for `DirectFeed.client` and `.personas`.
- `xcodebuild -project ios/FamiliarAgent.xcodeproj -scheme UCFFamiliar
  -configuration Debug -sdk iphonesimulator CODE_SIGNING_ALLOWED=NO build`:
  **BUILD SUCCEEDED** and linked `-lfamiliar_core`.
- Transient parity probe described in finding 1: **passed**; the probe file was
  removed immediately afterward and is not part of this review commit.

No production code, deployment, ship, gate, game action, human record, or fleet state
was changed.
