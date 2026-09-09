# T-236 brick 1 — codex re-verification, round 2

**Verdict: REJECT.** Repairs 1, 5 and 9 hold, and meaningful pieces of the other
repairs landed, but the mutation paths still do not preserve one computer per captain.
The legacy migration can leave two hulls for the same captain on different opaque ids,
and renaming a legacy hull discards its persona instead of migrating it. The rename
command still commits `persona.json` and then appends the trail separately, reports an
append failure as success, and can leave the live name disagreeing with the last complete
event. Pairing still commissions the world and writes the key and `captain.json` before
the fallible persona mutation. A broken persona also still reads as unnamed in the Swift
fleet row, and an in-flight failed open exposes the previous captain's voice under the
new world until the failure returns. Those are blockers against Ian's one named computer,
one memory, across a captain's whole fleet, with nothing observable shared across captains.

Reviewer: `companion:codex`

Reviewed: current requested checkout at `e7c09e1`, including repairs `c631907`,
`03bc9cb`, `7f7acff`, `6ce7609` and `c86fbb3` as they stand at HEAD

Ruling applied: Ian's 2026-09-04 one-computer-per-captain ruling.

## Findings

### 1. HELD — Blocker — host pairing silently drops the computer name the captain supplied

**File:** `ios/FamiliarSC/Sources/FamiliarSCUI/WireFeed.swift:189-193`;
`crates/cli/src/fleet_serve.rs:970-1021`;
`crates/cli/src/fleet.rs:686-748`, `:789-815`;
`crates/cli/src/fleet_serve.rs:239-281`

**Failure scenario.** The round-1 failure path is closed. Swift encodes the supplied name
as `computer_name`; the host accepts both that spelling and `computer-name` and forwards
the value as `--computer-name` to the one CLI pairing path. That path places the supplied
name into the validated persona, and `/ships` resolves the new captain-id store and emits
that persona. A first pairing with Felix therefore reads back Felix rather than Purr.
The host subprocess boundary is still not pinned by a handler-level test, but there is no
longer a dropped field between the two implemented request shapes.

**Repair accepted.** Keep the wire spelling test and add a host-handler fixture when this
surface next changes; no further production repair is required for finding 1.

### 2. PARTIAL — Blocker — joining a captain whose only record is the ship-local fallback replaces the chosen name with Purr

**File:** `crates/cli/src/fleet.rs:404-433`, `:682-748`, `:881-914`,
`:981-1007`

**Failure scenario.** The narrow second-pairing case now finds a pre-ruling ship-local
Felix and copies it into the new captain-id store, so Felix initially survives. It does
not migrate the existing hull's `captain.json`, its persona, or its naming trail. The old
hull therefore continues resolving its ship-local copy while the new hull resolves the
captain-id copy. Rename through the new hull changes only the latter; one captain then has
two observable voices.

The other round-1 migration loss remains literal in `fleet rename`: after assigning an
id, lines 895-901 load only the new captain directory. An absent file becomes the kernel
default; the ship-local fallback is never loaded. In the transient probe, the checked-in
fixture began as a v2 Purr with a full tuned style and ship-computer role. Renaming it to
Mittens exited 0 and printed `was "the familiar"`; the resulting captain persona contained
only the default household role, empty register/world, and no style. Lines 714-724 also
choose the first legacy sibling and exclude a tuned persona named exactly Purr, rather
than preserving the record or refusing conflicting siblings.

**Repair accepted.** Resolve a typed origin under the captain lock and migrate the whole
persona plus trail, byte-equivalent in every non-name field. Update every same-captain
sibling to the chosen id in the same recoverable operation, retain a tuned Purr, and refuse
multiple disagreeing legacy records instead of selecting the first.

### 3. NOT HELD — Blocker — the persona and its append-only trail are not one serialized mutation

**File:** `crates/kernel/src/persona.rs:238-317`, `:320-330`;
`crates/cli/src/fleet.rs:797-815`, `:907-923`

**Failure scenario.** The kernel helper adds an OS file lock, a pid-plus-sequence temp,
file syncs, and one prepared JSONL append; pairing uses that helper. Rename does not. It
still calls `persona::write`, releases that mutation's lock, calls the unlocked and
unsynced `record_naming`, prints any error, and returns success.

The transient probe made `persona-names.jsonl` a directory after a successful Felix
naming, then ran:

`familiar fleet rename world-37adbbf57dd334f1f3980477c61624f8 Sprocket --captain 'A. Captain' --data-dir /private/tmp/t236-b1r2-probe-data`

The command printed `the naming trail could not be written: Is a directory`, then printed
the success sentence and exited 0. `persona.json` said Sprocket while the last complete
trail event still said Felix. This is the exact round-1 failure.

Even `persona::name` is not the promised all-or-nothing operation at every failure point:
it appends and syncs the event before renaming the persona, so a rename failure leaves an
extra event, and it discards both directory-open and directory-`sync_all` errors at lines
314-315 while returning success.

**Repair accepted.** Route every naming path, including rename, through one locked helper
and propagate every trail and durability error. The helper must provide a recoverable
commit protocol for the two files so any reported failure restores the prior pair and any
reported success has a synced persona, synced trail, and synced containing directory.
Pin the CLI append-failure case, not only the helper's pre-rename append failure.

### 4. PARTIAL — Blocker — a lossy display-name slug is being used as captain identity

**File:** `crates/cli/src/fleet.rs:383-433`, `:981-1007`, `:1071-1079`;
`crates/cli/src/fleet_serve.rs:239-251`, `:325-358`, `:365-403`

**Failure scenario.** New records carry opaque ids, captain stores and direct captain
briefs use those ids, and newly assigned `A/B` and `A B` records get distinct store paths.
The migration is not yet idempotent read-old/write-new. `ensure_captain_id` ignores both
directory creation and legacy-store rename failures, then installs the id anyway; the next
read uses only the empty id path and abandons the old persona. It also does not detect the
legacy `A/B` / `A B` slug collision before moving their one ambiguous directory.

`fleet adopt-ids` passes only already visited records as siblings. The transient input put
an unmigrated `A. Captain` hull before a sibling already carrying
`cpt-18d3819859d34140-146a1`. The command exited 0 and assigned the first hull
`cpt-18d3819f54994970-14712`, leaving the second on its old id: one display captain,
two stores. Later rename output even claimed it applied "aboard probe, probe-two" although
it wrote only the first id's store. The status and fleet-brief accumulators also remain
keyed by display string at `fleet.rs:1073` and `fleet_serve.rs:337`, rather than by id.

Finally, a stale colliding route still risks naming a stranger: when no id matches,
`/captains/a-b/brief` uses `.find()` over all migrated ships and returns the first old-slug
match's id. With migrated `A/B` and `A B`, the old route cannot identify which captain was
intended, but returns one arbitrary captain's location rather than an ambiguity-safe 410.

**Repair accepted.** Make migration return `Result`, acquire one fleet/captain migration
lock, inspect all siblings before assigning anything, refuse legacy slug collisions, move
the store successfully before writing every sibling record, and resume idempotently after
each interruption. Key all grouping and money by `captain_id`. A stale ambiguous slug must
return 410 without selecting one captain; an unambiguous stale slug may include its single
new location.

### 5. HELD — Blocker — `--captain` can falsely act as another captain and still rename the target captain's computer

**File:** `crates/cli/src/fleet.rs:844-880`

**Failure scenario.** The ship record remains the subject, and a supplied actor label must
exactly match that record before any id migration or persona write. The transient hostile
probe ran `fleet rename ... Felix --captain Bob` against `A. Captain`'s isolated ship. It
printed that the world is A. Captain's ship, not Bob's, and exited 1; neither persona nor
trail changed.

**Repair accepted.** The requested hostile case is refused before mutation. No further
production repair is required for finding 5.

### 6. NOT HELD — Blocker — persona failure happens after pairing has already committed the ship and key

**File:** `crates/cli/src/fleet.rs:673-752`, `:754-815`

**Failure scenario.** Pairing now validates the in-memory persona before commissioning,
which closes the overlong-name case, but the fallible persona mutation still occurs at
lines 800-815 after `instance::commission`, `issuer.json`, `ucf.env`, automations, and
`captain.json` have all been written. A trail-open failure, temp write failure, rename
failure, or directory permission failure therefore returns pairing failure with a live
world, key and pairing record already discoverable by `paired_ships`; there is no rollback.
`ensure_captain_id` can also move the legacy captain directory before validation and
commissioning, without checking whether that move succeeded.

**Repair accepted.** Complete the durable persona/trail mutation before commissioning, or
use one recoverable transaction/rollback that removes every newly written world, registry,
key and captain artifact on any later failure. Pin an injected trail failure after valid
persona validation and assert there is no active world, no `ucf.env`, no `captain.json`,
and no new event.

### 7. PARTIAL — Should-fix — strict persona errors are hidden as "unnamed" on status and briefs

**File:** `crates/cli/src/fleet.rs:1082-1105`;
`crates/cli/src/fleet_serve.rs:239-281`, `:343-358`, `:408-417`, `:592-604`;
`ios/FamiliarSC/Sources/FamiliarSCUI/WireFeed.swift:68-76`, `:96-102`

**Failure scenario.** Rust text/JSON status now prints `will not load`, the `/ships` row
carries `persona.error`, and fleet and captain brief summaries turn that error into a loud
computer sentence. The ship brief's `context.computer` still selects only `name` and becomes
null on the same error. More importantly, Swift's fleet-row decoder selects only
`persona.name` or a top-level `computer`; the host `/ships` row has no top-level computer.
For `{"persona":{"error":"..."}}`, `summary(from:)` therefore produces
`(unnamed — fleet rename her)`, exactly the misleading round-1 state. Opening the row later
throws during strict persona decoding, but the fleet list has already hidden the breakage.

**Repair accepted.** Give every host brief context and Swift row a typed persona state
(named, absent, or broken with reason). Render broken distinctly from absent everywhere,
and pin one broken captain plus a valid ship-local fallback through CLI status, `/ships`,
fleet brief, captain brief, ship brief, and `WireFeed.summary`.

### 8. PARTIAL — Blocker — opening a broken persona leaves the previous captain's voice live in Swift

**File:** `ios/FamiliarSC/Sources/FamiliarSCUI/BridgeModel.swift:51-52`, `:94-125`,
`:128-135`, `:158-167`;
`ios/FamiliarSC/Tests/FamiliarSCTests/UITests.swift:201-237`

**Failure scenario.** After an awaited, non-cancelled failure, `clearVoice` now removes the
prior persona, journal, window, reports, book, conversation and turns; the sequential
Alice-then-broken-Bob test passes. Publication is still not atomic. `open` assigns
`self.world = Bob` at line 96 and then awaits five reads while Alice's persona and
conversation remain live. During any suspended read, `summary` is Bob, `computerName`
prefers Alice's persona, and `ask` has no `loading` or world-consistency guard. Alice can
therefore still answer while Bob is the selected ship; a slow failure merely lengthens the
cross-captain window. The test feed throws immediately and inspects only after `await open`
returns, so it cannot see this state.

**Repair accepted.** Either clear all voice/conversation state before publishing the new
world, or stage the world and complete snapshot together and publish them atomically. Gate
`ask` on the conversation's world as a final invariant. Pin a delayed broken persona read:
while it is suspended and after it fails, no previous name, turn, context, journal or
conversation may be readable or speakable under the new world.

### 9. HELD — Should-fix — Swift and Rust disagree on the captain brief route for the LOCAL rig

**File:** `crates/cli/src/fleet_serve.rs:239-251`;
`ios/FamiliarSC/Sources/FamiliarSCUI/WireFeed.swift:140-174`;
`ios/FamiliarSC/Tests/FamiliarSCTests/UITests.swift:136-150`

**Failure scenario.** The host now supplies a root-relative `captain_brief` on each ship
row. Swift looks up the selected row, strips the one leading slash, and calls that route;
it does not rebuild the id path from the captain label. For a pre-id host only, the fallback
matches Rust's byte behavior, including the double dash in
`Luke SkyWhisker (LOCAL soak)`. The current tests pin the host-provided id route and the
legacy punctuation, Unicode, repeated-separator and empty cases.

**Repair accepted.** The LOCAL bridge follows the server's fleet-document location. No
further production repair is required for finding 9.

## What held

- The kernel persona contract remains strict and backward-compatible: absent data is the
  v1 default, v1 cannot carry style, v2 style is bounded, and malformed, future-version or
  inconsistent records fail loudly. The default sentence remains byte-pinned.
- Captain-first, no-fall-through resolution still holds on reads. If the selected captain
  path contains a broken `persona.json`, `persona_for` returns its error immediately rather
  than speaking a valid ship-local fallback. Findings 2 and 6 concern mutation paths that
  bypass or overwrite that state, not this read precedence.
- A broken persona does not stall the pilot. `crates/whisker` has no persona read, and seam
  v2 remains explicit at `crates/whisker/src/wire.rs:195`; operational ship facts can
  continue while the voice refuses.
- The kernel naming helper's OS lock, unique temp, temp-file sync and trail-file sync are
  real improvements, and pairing calls it. Finding 3 remains because rename bypasses it
  and because the helper still does not make every failure point atomic/durable.
- World label, exchange hull name and computer name remain separate fields. Unpair still
  leaves the captain store intact.

## Verification

- `cargo fmt --all -- --check`: **exit 0**.
- `cargo clippy --all-targets -- -D warnings`: **exit 0**. It emitted only the existing
  `familiar-eye.swift — camera capture unavailable` build-script warning.
- `cargo test --workspace`: **exit 101** after **130 passed, 2 failed** in the suites Cargo
  reached. Both failures were in `familiar-cycle`
  (`a_proven_tool_is_deployed_with_honest_health` and
  `a_tool_that_finds_nothing_is_not_deployed`) when this managed environment refused their
  spawned shell with `Operation not permitted`; Cargo then stopped, so there is no complete
  workspace count. This is outside the T-236 paths, but the required workspace bar is red
  in this checkout/environment.
- `cargo test -p familiar-kernel -p familiar-cli`: **exit 0; 256 passed, 0 failed**
  (CLI 9, kernel 246, kernel integration 1, doc tests 0).
- `cd ios/FamiliarSC && SWIFTPM_MODULECACHE_OVERRIDE=/private/tmp/familiar-swiftpm-t236b1r2
  CLANG_MODULE_CACHE_PATH=/private/tmp/familiar-clang-t236b1r2 swift test`:
  **exit 1** before manifest evaluation because SwiftPM's nested `sandbox-exec` was refused.
- The same command with `swift test --disable-sandbox`: **exit 0; 55 tests executed,
  2 live-only tests skipped, 0 failures**. The build emitted the existing Swift 6
  `Sendable` warnings in off-limits `DirectFeed.swift`.
- `xcodegen --spec /private/tmp/familiar-codex-t236b1r2/ios/project.yml --project
  /private/tmp/t236-b1r2-xcodegen --project-root
  /private/tmp/familiar-codex-t236b1r2/ios`: **exit 0**. The generated project and source
  symlinks lived outside the checkout.
- `xcodebuild ... -scheme FamiliarMac -configuration Release ...
  CODE_SIGNING_ALLOWED=NO ... build`: **exit 74** before compilation; Xcode's Swift package
  resolver tried `sandbox-exec`, which this managed environment refuses even with caches,
  DerivedData and generated project under `/private/tmp`.
- `xcodebuild ... -scheme UCFFamiliar -configuration Release -destination
  'generic/platform=iOS Simulator' ... CODE_SIGNING_ALLOWED=NO ... build`: **exit 74**
  before compilation for the same nested-package-sandbox refusal; CoreSimulatorService was
  also unavailable. The two app builds are therefore unverified here, not demonstrated
  source failures.
- `cargo build -p familiar-cli`: **exit 0** for the isolated CLI probes.
- Isolated `world commission` for `probe` and `probe-two`, under
  `/private/tmp/t236-b1r2-probe-data`: **exit 0** for each; no key, network, lease, pilot or
  game action was used.
- Hostile actor probe, `fleet rename ... Felix --captain Bob`: **exit 1**, with
  `is A. Captain's ship, not Bob's`; persona and trail unchanged.
- Legacy rename probe, `fleet rename ... Mittens --captain 'A. Captain'`: **exit 0**,
  printed `was "the familiar"`; output persona lost the fixture's role and full style.
- Trail failure probe, with `persona-names.jsonl` replaced by a directory, then
  `fleet rename ... Sprocket --captain 'A. Captain'`: **exit 0** after printing the append
  error; persona `Sprocket`, last complete event `Felix`.
- Migration-order probe, with the first `A. Captain` record unmigrated and the later sibling
  already on `cpt-18d3819859d34140-146a1`, then `fleet adopt-ids`: **exit 0** and assigned
  the first record a different `cpt-18d3819f54994970-14712`.

No production code, coordination record, checked-in fixture, key, live network, pilot,
fleet, game state or deployment was changed. All transient state was under `/private/tmp`;
the only checkout change is this report.
