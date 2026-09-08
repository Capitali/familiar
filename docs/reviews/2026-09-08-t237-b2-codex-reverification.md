# T-237 B2 FamiliarSC codex re-verification

**Verdict: REJECT.** The package reads the original ship-store shapes, keeps the exchange
client read-only, and retains a deterministic voice floor. Its tests are green and the
core product compiles with warnings promoted to errors. But current main no longer has the
cross-runtime contract B2 claims: the Swift autonomy vocabulary rejects the host's margin
surface, the captain reader silently loses the durable identity field, and the notice and
voice policies miss current money/refusal events. Independently, the grounding gate admits
a polarity-reversed statement when its numbers and identifiers are unchanged.

Reviewer: companion:codex
Reviewed: B2 `20cf3c36`; later bridge/conversation work through `b7c55024`; current Rust
contract additions including `f729b34d` and `6ce76096`; claim `021d6eb2`; current main
`021d6eb2`

## Findings

### 1. Blocker — grounding checks token provenance, not whether the statement remains true

**File:** `ios/FamiliarSC/Sources/FamiliarSC/BridgeVoice.swift:71-100`,
`:475-503`

**Failure scenario.** `Grounding.check` compares only numbers, load/tick/proposal ids, and
hyphenated slugs, plus a one-way mood ordering. It does not bind a predicate, polarity,
actor, or relation to the source fact. A transient executable probe supplied the floor
`t123: bought 40 ore at ask 15 at foxys-diner` and the spoken line with `bought` changed to
`sold`; `Grounding.check` returned `nil`. The same quantity, tick, price, and station make a
materially false trade statement pass.

The conversational path is weaker still. Its prompt says every station must come from the
facts/documents, but after extracting station tokens it explicitly filters `said` down to
numbers and L/t/p identifiers. An invented station therefore cannot trip the check. The B2
line says style may bend cadence but never truth; these are false statements admitted by
the mechanism presented as the truth boundary.

**Repair accepted.** Have model output select/reorder typed source claims (or source-fact
ids) and let deterministic code render the event predicate, polarity, quantities, and
stations. If free prose remains, validate structured claims before rendering and retain
station checks in conversation. Pin buy→sell, approved→denied, source-station invention,
and omission of a material qualifier.

### 2. Blocker — Swift rejects the host's `market.margin` autonomy surface

**File:** `ios/FamiliarSC/Sources/FamiliarSC/Autonomy.swift:23-85`;
`ios/FamiliarSC/Tests/FamiliarSCTests/AutonomyTests.swift`;
`crates/whisker/src/autonomy.rs:46-68`, `:114-134`, `:161-187`

**Failure scenario.** Rust now has 18 surfaces, including `market.margin`, whose absent
setting defaults to `advise`. Swift still declares and tests exactly 17 surfaces. Its
strict decoder therefore reports a valid host file containing
`{"market.margin":"confirm"}` as malformed, cannot show or set the captain's borrowing
policy, and would use the wrong default if the missing case were merely ignored. A
transient probe against the built FamiliarSC module returned
`autonomy.json: unknown control surface 'market.margin'` for that exact object.

**Repair accepted.** Add the surface and its advise default, then pin a cross-language
contract fixture containing every key and default. Prefer generating both enumerations
from one checked contract so adding a host action door cannot silently strand the bridge.

### 3. Blocker — the Swift captain reader drops the durable captain identity

**File:** `ios/FamiliarSC/Sources/FamiliarSC/ShipStore.swift:157-186`;
`ios/FamiliarSC/Sources/FamiliarSCUI/Feed.swift:220-267`;
`crates/cli/src/fleet.rs:35-63`

**Failure scenario.** Rust `Captain` now persists `captain_id` because display names collide
and rename. Swift's `Captain` coding keys omit it, so `Codable` accepts a modern
`captain.json` while silently discarding its only durable captain key. `StoreFeed` then
projects only the display label into `ShipSummary`. The B2 reader was explicitly promised
as typed to Rust's captain record, and Ian's later one-computer-per-captain ruling makes
this field the identity boundary for persona, memory, fleet brief, and eventually the
engine captain. Two equal labels remain indistinguishable to the local package.

**Repair accepted.** Decode a backward-compatible `captainID` (empty only for legacy
records), carry it through the store summary/context boundary, and prefer it for every
captain-scoped join. Pin modern, legacy, colliding-display-name, and rename fixtures.

### 4. Blocker — current money and refusal journal events are invisible or mis-ranked

**File:** `ios/FamiliarSC/Sources/FamiliarSC/Notices.swift:23-66`;
`ios/FamiliarSC/Sources/FamiliarSC/TemplatedVoice.swift:57-78`, `:217-273`;
`crates/whisker/src/main.rs:1118-1134`, `:1552-1563`

**Failure scenario.** The runner emits `paid-down`, `pay-down-refused`, and
`trade-refused`. `NoticePolicy` handles none of them, despite B2's explicit notice scope
covering money and refusals. `TemplatedVoice` likewise has no typed fact for those events;
its severity switch falls through to neutral severity 2 and its danger predicate excludes
them. Under a full fact window a rejected trade or lease payment can lose to ordinary
events, while a successful debt payment produces no money notice at all.

This is vocabulary drift across an append-only journal contract, not a cosmetic omission:
the captain can miss both a material balance change and an act that failed at the exchange
door.

**Repair accepted.** Classify and render every current captain-worthy event, including
these payment and trade outcomes, from a shared/versioned event registry or a fixture
generated beside the Rust vocabulary. Pin every money/refusal event through both notice
and voice policies, with an explicit safe rule for truly unknown future events.

## What held

- The pairing key stays out of arguments through `--key-file`; the key parser, original
  store records, message-window fold, and typed read-only `/v1` client remain covered by
  passing tests.
- The deterministic templated lane remains available when the Foundation Models lane is
  unavailable, and tool-produced dial changes remain proposals requiring a captain act.
- No production key or network was used, and this review performed no approval, dial
  write, game action, deployment, ship, gate, or human/fleet mutation.

## Verification

- `swift test --package-path ios/FamiliarSC`: **55 passed, 0 failed, 2 live tests
  skipped**. The run reported the on-device lane unavailable.
- `swift build --package-path ios/FamiliarSC --product FamiliarSC
  -Xswiftc -warnings-as-errors`: **pass**.
- `swift build --package-path ios/FamiliarSC -Xswiftc -warnings-as-errors`: **fails** in
  later B3 `DirectFeed` code because `ExchangeClient` and `DevicePersonaStore` are not
  `Sendable`; that package-wide warning debt is already outside this B2 finding set.
- Transient grounding probe: **semantic inversion admitted** (`nil`).
- Transient autonomy probe: **valid Rust surface rejected** (`market.margin`).

The probes lived outside the repository and are not part of this review commit. No live
Foundation Models/PCC lane or real-journal proof was run during this re-verification.
