# T-236 brick 1 — codex re-verification

**Verdict: REJECT.** The typed persona seam itself is strict and backward-compatible,
and Ian's one-computer-per-captain ruling is the right model, but the landed mutation
paths do not yet preserve that model. A name supplied by the Swift pairing surface is
discarded, a second pairing can shadow a pre-ruling chosen name with Purr, and concurrent
renames can leave the persona and its append-only trail disagreeing. The captain key is
also a lossy display-name slug rather than an isolated identity, so two distinct captain
strings can share both a persona and a pooled brief. Finally, the loud broken-persona
wire result can leave the Swift bridge holding the previously opened captain's voice.

Reviewer: `companion:codex`

Reviewed: the requested brick and review fixes (`c3df372^..82c5123`, `0b498cc`,
`e7531ab`, `82c922a`), then the three requested source files at current HEAD `325851b`

Ruling applied: Ian's 2026-09-04 one-computer-per-captain ruling supersedes the old
per-hull Round-2 acceptance line.

## Findings

### 1. Blocker — host pairing silently drops the computer name the captain supplied

**File:** `crates/cli/src/fleet_serve.rs:907-960`;
`ios/FamiliarSC/Sources/FamiliarSCUI/WireFeed.swift:173-177`;
`ios/FamiliarSC/Sources/FamiliarSCUI/PairingView.swift:81-89`

**Failure scenario.** The Swift client sends the optional `computer_name` field and the
pairing screen explicitly promises that it names a new computer or renames the one the
captain is joining. The host parses `label`, `captain`, `server`, `key`, automations and
`pilot_args`, but never reads `computer_name` and never adds `--computer-name` to the
`fleet pair` subprocess. A captain entering Felix on the first host pairing therefore
gets Purr. Entering a new name while joining an existing fleet reports success but does
not rename anything. This is a live path to the exact wrong-name result the brick exists
to prevent, and the Swift tests exercise only their own request construction, not the
Rust handler that consumes it.

**Repair accepted.** Validate and forward `computer_name` as `--computer-name` in the
single pairing path. Add a handler-level test that starts with the Swift JSON shape and
proves both the resulting captain persona and the naming event contain the chosen name;
pin the existing-captain case as a rename that preserves style.

### 2. Blocker — joining a captain whose only record is the ship-local fallback replaces the chosen name with Purr

**File:** `crates/cli/src/fleet.rs:638-695`, `:738-786`;
`crates/cli/src/fleet.rs:370-385`

**Failure scenario.** `persona_for` correctly reads captain first and ship-local second,
but `fleet pair` decides whether the captain already has a computer solely by testing
for `captains/<slug>/persona.json`. Take a ship named Felix before `82c922a`, with Felix
and its tuned style still in that ship's `persona.json`. Pair a second ship for the same
captain without `--computer-name`: `already` is false, so the command writes a fresh Purr
to the captain store. Captain-first resolution then shadows Felix on both ships. The
fallback preserved the old name only until the ordinary join operation used it.

The rename path has the same migration loss in a quieter form. If the captain store is
absent, it loads that absent directory as the household default rather than loading the
ship-local fallback; the new name lands, but the old ship's v2 style is discarded.

**Repair accepted.** Give resolution a typed origin (`captain`, `legacy ship`, absent,
or broken) and use it for mutations as well as reads. Before creating Purr, migrate the
one legacy persona for that captain, preserving every byte-equivalent field and its
trail. If several legacy ships disagree, refuse and require the captain to choose rather
than silently selecting one. Pin a pre-ruling Felix-with-style plus second-pairing test
and a rename-migration test.

### 3. Blocker — the persona and its append-only trail are not one serialized mutation

**File:** `crates/kernel/src/persona.rs:232-255`;
`crates/cli/src/fleet.rs:662-709`, `:761-789`

**Failure scenario.** The replacement writer always uses the shared name
`persona.json.tmp`, and pair/rename holds no cross-process lock across load, replace and
trail append. In one valid interleaving, rename A writes its temp, rename B overwrites
that temp, A renames B's bytes into place and then appends A's event, while B's rename
fails because the temp path is gone. The surviving persona says B and the audit trail
says A. The same writer served the original ship store and now serves the captain
store, so neither location has the requested concurrent-writer guarantee.

There are two further holes in the crash claim: neither the replacement nor the log is
synced before success, and both pair and rename merely print a naming-trail append error
then return success. Disk-full, permission, or power-loss cases can therefore commit a
name without the dated event that is supposed to explain it. `O_APPEND` alone does not
make the read/modify/write plus second-file append a transaction.

**Repair accepted.** Hold one OS-backed per-captain lock across resolution, validation,
unique-temp replacement and event append; use a PID-plus-sequence or `create_new` temp,
sync the completed temp before rename, sync the containing directory, write each JSONL
record as one prepared buffer, and sync it before reporting success. A trail failure
must fail the naming act, not become a successful rename with a warning. Pin two-process
renames and an injected append failure, asserting that the final persona agrees with
the last complete trail event and every successful act remains present.

### 4. Blocker — a lossy display-name slug is being used as captain identity

**File:** `crates/cli/src/fleet.rs:345-368`;
`crates/cli/src/fleet_serve.rs:347-389`

**Failure scenario.** Every non-ASCII-alphanumeric character becomes `-`, with no
collision record. Distinct captain strings such as `A/B` and `A B` both own
`captains/a-b`. Renaming either ship renames the other's computer. Worse, the captain
brief selects ships by that same filename, so it sums both captains' credits, debt and
book into one response even though CLI status groups the original strings separately.
That violates both halves of the updated ruling: two captains share observable persona
state, and test money can pool into another captain whenever their labels collide.

The recorded `Luke SkyWhisker (LOCAL soak)` string does remain distinct from
`Luke SkyWhisker` in the Rust store and CLI money map. That particular case does not
repair the general identity problem.

**Repair accepted.** Persist an opaque, collision-free captain id and keep the captain
string as a display label. Store that id in each `captain.json`, key persona and money
by it everywhere, and detect/refuse any legacy slug collision during migration. If
case/punctuation aliases are intentionally the same captain, make that one explicit
canonicalization decision and use its id for both persona and money rather than using
exact strings in one place and slugs in another. Pin two deliberately colliding labels
and the PROD/LOCAL pair.

### 5. Blocker — `--captain` can falsely act as another captain and still rename the target captain's computer

**File:** `crates/cli/src/fleet.rs:751-785`

**Failure scenario.** The target store is derived from the ship's `captain.json`, but
the optional `--captain` value is accepted verbatim only as the event actor. Thus
`fleet rename <alice-world> Felix --captain Bob` renames Alice's computer and records
Bob as the human behind it. The command neither refuses the mismatch nor authenticates
the asserted actor. The name changes successfully, so the typed trail makes the false
attribution more durable rather than safer.

**Repair accepted.** Treat the ship's persisted captain id as the subject and an
established authenticated identity as the actor. Remove the free actor override, or
require it to resolve to the same captain id and refuse otherwise. Pin the requested
hostile case: a different captain string cannot change the persona or append an event.

### 6. Blocker — persona failure happens after pairing has already committed the ship and key

**File:** `crates/cli/src/fleet.rs:573-637`, `:642-709`

**Failure scenario.** Commissioning, `ucf.env`, automations and `captain.json` are all
written before the captain persona is resolved or validated. An overlong chosen name,
a broken existing captain persona, or a captain-store write failure makes `fleet pair`
return failure only after an active world containing the exchange key already exists.
`paired_ships` recognizes that partial result because the active registry row,
`captain.json` and `ucf.env` are present. The user is told pairing failed, while status
and the supervisor discover a paired ship (without the lease the command would have
issued later).

**Repair accepted.** Resolve and validate the entire captain-persona mutation before
commissioning. Then commit pairing under the same captain/world coordination lock, or
roll back every newly written registry/store artifact on any later error. Add a broken
captain persona and invalid-name test proving failure leaves no active world, no key,
and no new captain event.

### 7. Should-fix — strict persona errors are hidden as "unnamed" on status and briefs

**File:** `crates/cli/src/fleet.rs:370-385`, `:910-917`;
`crates/cli/src/fleet_serve.rs:333-337`, `:360-385`, `:543-546`;
`ios/FamiliarSC/Sources/FamiliarSCUI/WireFeed.swift:68-76`, `:96-102`

**Failure scenario.** The `0b498cc` fix correctly stops on an existing broken captain
file and returns `{"error": ...}` instead of falling through to a ship-local name.
The fleet status path then asks only for `name` and maps its absence to the ordinary
"unnamed" sentence. The fleet and captain brief top levels likewise turn it into a null
computer name. The raw ship row retains the error, so opening that ship in Swift
eventually fails strict persona decoding, but the fleet list first presents the same
state as a computer that was simply never named. The error is loud only after a second,
deeper read.

**Repair accepted.** Keep the no-fall-through rule, but make persona resolution a typed
result rather than overloading an arbitrary JSON object. Text status should say the
captain persona is unreadable (and return failure if status is used as a check); every
brief should carry the same explicit error; Swift should render that error on the row
without substituting the unnamed state. Pin broken captain plus valid ship-local data
across CLI status, `/ships`, ship brief and captain brief.

### 8. Blocker — opening a broken persona leaves the previous captain's voice live in Swift

**File:** `ios/FamiliarSC/Sources/FamiliarSCUI/BridgeModel.swift:51-52`, `:94-116`;
`ios/FamiliarSC/Sources/FamiliarSCUI/WireFeed.swift:96-102`

**Failure scenario.** Open a valid ship for Alice, which installs Alice's persona and
conversation in `BridgeModel`. Then open Bob's ship while Bob's captain persona is
broken. `open` changes `world` to Bob before its first read, strict persona decoding
throws, and the catch only records an error. It does not clear `persona`, `conversation`,
the old journal or the old context. `computerName` therefore still prefers Alice's
persona, and `ask` can still use Alice's conversation while the selected summary and
settings belong to Bob. The server did the right thing by refusing a fallback, but the
client turns that refusal into stale cross-captain state.

**Repair accepted.** Load a complete bridge snapshot into locals and publish it only
after every required read succeeds; on a persona failure, clear all voice/conversation
state for the newly selected world while retaining only explicitly safe ship facts and
the visible error. Add a two-captain test that opens Alice successfully, makes Bob's
wire persona invalid, opens Bob, and proves no Alice name, turns, journal or context is
readable or speakable.

### 9. Should-fix — Swift and Rust disagree on the captain brief route for the LOCAL rig

**File:** `crates/cli/src/fleet.rs:352-368`;
`ios/FamiliarSC/Sources/FamiliarSCUI/Briefs.swift:122-130`;
`ios/FamiliarSC/Sources/FamiliarSCUI/WireFeed.swift:154-156`

**Failure scenario.** Rust replaces every punctuation character independently, so
`Luke SkyWhisker (LOCAL soak)` becomes `luke-skywhisker--local-soak`. Swift drops the
parentheses and collapses separators, producing `luke-skywhisker-local-soak`. It asks
for a captain route Rust cannot match; `WireFeed.context` suppresses the 404 with
`try?`, and the LOCAL bridge silently loses the fleet document. The Swift slug unit
test proves only Swift's answer and therefore stays green while disagreeing with the
host.

**Repair accepted.** Do not make clients reproduce a filesystem transform. Return an
opaque captain id or canonical brief link in the fleet row and use that route. At
minimum, share exact cross-language fixtures for punctuation, repeated separators,
Unicode, empty input and the recorded PROD/LOCAL names.

## What held

- The kernel v1 contract holds. Missing `style` leaves `style: None`, the default role
  sentence is pinned byte-for-byte, v2 style defaults are bounded, and malformed,
  unknown or internally inconsistent records return `InvalidData` rather than a
  default persona.
- The deliberate captain-first, no-fall-through read in `persona_for` is correct. A
  broken captain file cannot make a legacy ship-local name speak in its place.
- A broken captain persona does **not** stall the live pilot. `whisker` receives only
  the ship store and never loads persona data; it continues deterministic flying.
  `fleet serve` also stays up and serves operational ship facts, with the error in the
  row. That separation—operations continue, the broken voice does not counterfeit a
  fallback—is the right host behavior; findings 7 and 8 cover the surfaces that hide
  the error or retain stale client state.
- With a valid captain record, status and the host feed resolve captain before the
  legacy ship-local fallback. Text status keeps world label, recorded hull name and
  computer name distinct. `fleet unpair` removes the key and decommissions the world
  without deleting the captain store.
- The current host row's persona keys still decode against Swift's strict `Persona`
  shape, and Swift understands both the host's `persona` object and CLI status's
  `computer`/`hull` fields. Its fixtures do not cover findings 1, 2, 4, 8 or 9; the store
  fixture is itself the allowed pre-ruling ship-local fallback rather than a newly
  commissioned captain-store layout.

## Verification

- `cargo test -p familiar-kernel -p familiar-cli`: **249 passed, 0 failed**
  (CLI 5; kernel 243; kernel integration 1; doc tests 0). The build emitted only the
  existing camera-capture-unavailable build-script warning.
- `cargo clippy -p familiar-kernel -p familiar-cli --all-targets -- -D warnings`:
  **passed** at HEAD, with the same camera build-script warning.
- `cd ios/FamiliarSC && SWIFTPM_MODULECACHE_OVERRIDE=/private/tmp/familiar-swiftpm-t236b1 CLANG_MODULE_CACHE_PATH=/private/tmp/familiar-clang-t236b1 swift test --disable-sandbox`:
  **52 tests executed, 1 live-only test skipped, 0 failures**. The build reported the
  current Swift-6 `Sendable` warnings in `DirectFeed`; they are outside this brick.

No production code, store, key, pilot, fleet state or deployment was changed.
