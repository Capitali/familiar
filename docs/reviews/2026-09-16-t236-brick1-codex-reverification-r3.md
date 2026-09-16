# T-236 brick 1 — codex re-verification, round 3

**Verdict: REJECT.** Findings 1, 3, 5, 6, 7 and 9 now hold. The valid legacy
persona cases in finding 2 and most of the identity migration in finding 4 are repaired,
and the ordinary sequential Swift switch in finding 8 is repaired. Three blocker paths
remain against the bar, however:

- Pairing treats a present-but-broken existing computer as absent and overwrites it with a
  fresh persona. Ship-local migration also copies persona and trail without taking the
  persona lock its own comment promises, so a concurrent naming can split the pair.
- A captain who already has two ids is not refused. `ensure_captain_id` returns early for
  any non-empty id before inspecting siblings, and `fleet adopt-ids` therefore exits
  successfully while leaving one captain on two stores.
- Swift gates an ask only before its asynchronous answer. If the captain switches ships
  while that answer is suspended, the old captain's answer is appended and spoken after
  the switch without rechecking the world. Concurrent `open` calls can likewise publish a
  stale ship after a newer selection.

Reviewer: `companion:codex`

Reviewed: current local `origin/main` at `1080e63` (2026-09-16). The review checkout is at
`51c7ce6`; the two intervening commits change only `coordination/STATE.md` and the iOS build
number in `ios/project.yml`, not the T-236 implementation reviewed here.

Ruling applied: Ian's 2026-09-04 one-computer-per-captain ruling.

## Findings

### 1. HELD — Blocker — host pairing silently drops the computer name the captain supplied

**File:** `ios/FamiliarSC/Sources/FamiliarSCUI/WireFeed.swift:244-248`;
`crates/cli/src/fleet_serve.rs:1235-1247`, `:1266-1294`;
`crates/cli/src/fleet.rs:1310-1311`, `:1365-1385`, `:1432-1449`

Swift still sends the chosen value as `computer_name`. The host accepts that spelling and
the compatibility spelling, forwards it as `--computer-name`, and invokes the single CLI
pairing path. That path retains the supplied name in the persona committed before the ship
is commissioned. No field is dropped at either boundary.

The host subprocess boundary still lacks a handler-level pin, and the exchange-backed CLI
pin could not start its loopback listener in this managed environment, but the implemented
field path remains complete.

### 2. PARTIAL — Blocker — second pairing / rename on a legacy hull preserves only a valid, quiescent record

**File:** `crates/cli/src/fleet.rs:480-558`, `:563-580`, `:1340-1385`,
`:1639-1664`

The advertised valid cases are substantially repaired. `computer_origin` checks the id
store, legacy store, and every same-captain ship-local location; disagreeing ship-local
bytes are refused. `migrate_computer` copies the persona and trail, `adopt_siblings` points
unmigrated sibling records at the chosen id, and rename loads the migrated persona rather
than a fresh default. A valid tuned Purr therefore survives the tested path.

Two preservation gaps remain:

1. Pairing converts a load failure into absence with
   `persona::load(&persona_dir).ok()` at lines 1360-1363. A broken id-store, legacy, or
   migrated ship-local `persona.json` is therefore fed to the `(None, given)` arm at
   lines 1378-1385 and overwritten by Purr or the newly supplied name. This discards the
   existing name/style instead of refusing loudly. Rename correctly propagates the same
   load error at lines 1659-1664; pairing does not.
2. Lines 517-520 say migration is under the kernel persona lock, but lines 521-558 take no
   such lock and copy the trail and persona in two separate operations. A concurrent
   naming can land between those copies, producing an id store whose persona and trail
   came from different moments.

The original silent loss is closed for valid fixtures, but the operation still does not
preserve one existing computer whole under failure and concurrency.

### 3. HELD — Blocker — persona + trail one serialized mutation

**File:** `crates/kernel/src/persona.rs:357-380`, `:383-535`;
`crates/cli/src/fleet.rs:1697-1721`

`persona::name` validates first, takes one persistent OS lock, remembers the old persona
and trail length, writes and syncs a unique temp, appends and syncs the event, renames the
persona, and syncs the directory. Errors from append, rename, and directory sync are
returned after restoration is attempted. `record_naming` is gone, and CLI rename uses this
helper and returns failure when it fails.

The three kernel failure pins passed, including first naming and failed persona landing;
the exchange-backed CLI trail pin was blocked before its assertions by this environment's
loopback restriction, not by a product failure.

### 4. PARTIAL — Blocker — identity migration still accepts two ids for one captain

**File:** `crates/cli/src/fleet.rs:616-690`, `:2347-2394`;
`crates/cli/src/fleet_serve.rs:368-415`, `:472-531`

The migration lock, move-before-install order, interrupted empty-store completion, legacy
slug collision check, id-keyed status/fleet grouping, and all-migrated ambiguous stale-slug
410 are present. An unmigrated hull also adopts an already migrated sibling's id when that
is the only id found.

The promised two-id refusal is bypassed. After acquiring the lock,
`ensure_captain_id` returns immediately at lines 622-624 whenever the selected record
already has an id. The sibling collection and `ids.len() > 1` refusal at lines 625-639 are
never reached. `fleet adopt-ids` calls that helper for every record, so two already-split
hulls are both reported as already migrated and nothing is reconciled or refused.

Transient probe input: two commissioned rows for display captain `A. Captain`, one with
`captain_id: cpt-one`, the other with `captain_id: cpt-two`, followed by:

```text
target/debug/familiar fleet adopt-ids \
  --data-dir /private/tmp/familiar-codex-t236b1r3/.t236-probe \
  --store-root /private/tmp/familiar-codex-t236b1r3/.t236-probe/worlds
```

Exact output and status:

```text
  one — A. Captain already cpt-one
  two — A. Captain already cpt-two
fleet: 0 record(s) given an identity
exit 0
```

Both `captain.json` files retained their different ids. This is the repair's explicit
refusal case and leaves one captain resolving two computer stores.

### 5. HELD — Blocker — `--captain` cannot act as another captain

**File:** `crates/cli/src/fleet.rs:1588-1611`

Rename derives the subject from the ship's `captain.json` and checks a supplied
`--captain` label for exact equality before id migration, persona migration, exchange
filing, or naming. A mismatch returns failure and cannot select another captain's
computer.

### 6. HELD — Blocker — a pairing whose naming fails commissions nothing

**File:** `crates/cli/src/fleet.rs:1294-1309`, `:1403-1449`, `:1491-1543`

Persona validation and the recoverable persona/trail naming now happen before
`instance::commission`. A naming failure returns at lines 1446-1449, before the world,
registry row, key, `ucf.env`, automations, or `captain.json` exist. The dedicated end-to-end
pin was among the tests whose stub listener was denied by the sandbox, but the ordering is
unambiguous and the kernel failure injection passed independently.

### 7. HELD — Should-fix — broken reads are distinct from unnamed on every requested surface

**File:** `crates/cli/src/fleet.rs:735-752`, `:1021-1036`, `:2478-2499`;
`crates/cli/src/fleet_serve.rs:308-326`, `:368-415`, `:472-592`, `:738-768`;
`ios/FamiliarSC/Sources/FamiliarSCUI/Feed.swift:12-22`;
`ios/FamiliarSC/Sources/FamiliarSCUI/WireFeed.swift:68-108`

The host exposes `computer_state` as named, broken with the loader's reason, or absent on
ship rows, fleet-brief captain rows, both captain-brief locations, and the ship-brief
context. Its compatibility `computer` word also says `will not load` for a broken record.
Swift prefers the typed state and renders broken separately from absent. The host surface
tests passed 7/0, and all four T-236 Swift tests passed, including the typed-state cases.

This verdict concerns read surfaces. Finding 2 separately covers pairing's mutation-time
conversion of the same load error into absence.

### 8. PARTIAL — Blocker — sequential open is quiet, but an in-flight old ask can still speak after a switch

**File:** `ios/FamiliarSC/Sources/FamiliarSCUI/BridgeModel.swift:107-163`,
`:186-197`; `ios/FamiliarSC/Tests/FamiliarSCTests/T236SwiftTests.swift:65-117`

The repaired sequential case holds. Switching worlds clears persona, conversation, turns,
journal, window, reports, book, and related state before publishing the new world. The five
required reads stage into locals before that voice state is repopulated, and an ask started
while the new world is loading is refused. The delayed broken-persona pin passed.

The ask gate is checked only before suspension. At lines 192-194 an old-world conversation
passes the guard and awaits `c.ask(q)`. Because this `@MainActor` method is reentrant at the
await, `open(world:)` can then switch worlds and call `clearVoice`. When the old answer
returns, lines 195-196 append it to the now-current `turns` and speak it without rechecking
`conversationWorld == world` or whether `c` is still the active conversation. Thus the
previous captain can still answer aloud after the new ship is selected.

There is a related stale-open race: two overlapping `open` calls have no generation token
or cancellation. Either call can resume after the other and publish its persona/journal at
lines 127-147 under the later call's `self.world`. The current pin starts an ask only after
the switch has begun and exercises only one open, so it does not cover either reentrancy
case.

### 9. HELD — Should-fix — Swift follows the host's captain brief route

**File:** `crates/cli/src/fleet_serve.rs:260-278`;
`ios/FamiliarSC/Sources/FamiliarSCUI/WireFeed.swift:194-229`

Every ship row carries the server-built root-relative `captain_brief`. Swift finds the
selected row, removes one leading slash, and calls that route. It reconstructs the legacy
slug only for a host that predates the field, retaining the existing byte behavior for the
LOCAL compatibility case.

## Verification

- `cargo fmt --all -- --check`: **exit 0**.
- `cargo clippy --all-targets -- -D warnings`: **exit 0**. The only emitted warning was the
  existing build-script message that `familiar-eye.swift` camera capture is unavailable.
- `cargo test --workspace`: **exit 101 after 46 passed and 18 failed in the 64 tests Cargo
  reached**. All 18 failures were exchange-backed CLI tests panicking at
  `crates/cli/src/fleet.rs:2943` because this managed environment denied
  `TcpListener::bind("127.0.0.1:0")` with `Operation not permitted`; Cargo then stopped, so
  there is no complete workspace count. This includes the new CLI pins and is an
  environmental test failure, but the required workspace command is red here.
- `cargo test -p familiar-kernel`: **exit 0; 251 passed, 0 failed** (250 unit, 1 integration,
  0 doc tests). The naming failure pins passed.
- Non-networked CLI reruns: **9 passed, 0 failed** (7 host surface tests plus the existing-id
  adoption and legacy-store migration unit pins).
- `cargo build -p familiar-cli`: **exit 0**, used for the transient two-id CLI probe above.
- `cd ios/FamiliarSC && SWIFTPM_MODULECACHE_OVERRIDE=... CLANG_MODULE_CACHE_PATH=...
  swift test --disable-sandbox`: **exit 0; 93 executed, 2 skipped, 0 failures**. It emitted
  the existing Swift 6 async `NSLock` warnings in `DirectPilotTests.swift`.
- `xcodegen` generated `FamiliarAgent.xcodeproj` under a transient `/private/tmp` directory:
  **exit 0**. Source/package paths were symlinked there; the checkout source was untouched.
- `xcodebuild` Release for `FamiliarMac`, with signing disabled and all caches/DerivedData
  under `/private/tmp`: **exit 74 before compilation** because Swift package manifest
  resolution invoked `sandbox-exec`, which this managed environment rejects with
  `sandbox_apply: Operation not permitted`.
- `xcodebuild` Release for `UCFFamiliar` on generic iOS Simulator, with the same isolation:
  **exit 74 before compilation** for the same nested SwiftPM sandbox refusal;
  CoreSimulatorService was also unavailable. Neither app build is verified here, but no app
  source failure was reached.

No source, coordination file, install, deployment, or external system was changed.
