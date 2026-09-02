# Development Log

The linear handoff trail for The Familiar v2. Newest entries on top. Before making
architectural changes, read `SOUL.md` (the Three Laws) and `ARCHITECTURE.md`, then
the latest entries here.

Each entry: what changed, why, checks run, what the next developer should know.

## 2026-09-01 — T-232 brick 1, round 2: the itinerary is station stops, and a restart forgets nothing

Round 1 of this brick was REJECTED by two independent codex reviews (one run from each
lane — `docs/reviews/2026-09-01-t232-itinerary-review.md` and
`…-t232-itinerary-reciprocal-review.md`), and this round is rebuilt to their combined
findings, rebased over the live fixes that landed mid-flight (`83025b2`: the
booked-at-destination deadhead, the pending-fold reconcile guard, the lost-load
cooldown with fresh-id purge, the spare-hold fit, and the whole T-233 merchant). All
in `crates/whisker`:

- **The route is station stops, not contracts** (both reviews' P1). `StopOp` —
  `Pickup`/`Drop`/`Refuel` — is the routing atom; a `Stop` is one berth visit carrying
  any number of ops; `Itinerary` holds the contracts (lifecycle, booking order) AND
  the ordered stops that serve them. `Itinerary::sequential` is the compile today's
  one-contract world flies — origin pickup, destination drop, same-station visits
  coalesced, a planned `Refuel` opening every visit to a fuel-selling berth — and a
  real planner replaces that FUNCTION, not the types, when UCF-Haul#43's shape lands.
  An interleaved route (pickup A, pickup B, drop B + refuel, drop A) is pinned in a
  pure test with per-op hold occupancy.
- **A planned fill is an op the pilot executes** (both reviews' finding 3 — and a
  latent gap in the OLD doctrine, which budgeted a pump-origin fill in its booking
  arithmetic that no active-load decision could ever perform). Berthed at a route
  stop that pumps, below the top-up line, the decision is now `Refuel` before crane
  work; the fuel walk's budget reset happens exactly at stops carrying that op.
- **Occupancy is walked, not summed** (both reviews). A candidate must fit the hold
  at its own pickup, beside merchant goods (T-233's cargo sits in the aggregate
  `hold_used`) and every not-yet-dropped contract — two sequential full-hold
  contracts fit; a merchant position genuinely narrows what freight can board.
- **Adoption is reconciliation, not a startup one-shot** (my review's blocker 1).
  Every cycle, ledger-open loads the plan does not carry become pending adoptions;
  a pending id retries its board-row lookup each fold until it resolves or the
  ledger closes it. A transient loadboard omission delays adoption one fold instead
  of forgetting a live contract forever, and no booking happens while any open id
  is unresolved.
- **The ledger folds chronologically** (blockers 2 and 6). `ledger::reconcile` and
  `ledger::open_loads` reduce each load's events in tick order with terminal-wins
  same-tick precedence: array-order noise cannot resurrect a closed load, a
  genuinely later booking starts a new life with ITS tick as the plan order, and
  adoption orders by the booking tick — not the latest lifecycle event.
- **The ranking's altitude is stated, not inflated** (both reviews' marginal-rate
  finding). `best_insertion` ranks by the board row's own ship-relative rate —
  exactly the old ranking for the empty plan the booking gate confines it to — and
  its doc comment forbids widening that gate on the strength of this ranking; a
  true marginal rate needs route ticks the wire cannot yet answer.

The old doctrine tests run UNMODIFIED through the plan layer (decide IS decide_plan
over the sequential compile of ≤1 contract) — including `83025b2`'s
booked-at-destination pin. Two divergences from the old single-load rules are
deliberate and documented on `decide_plan`: the executable pump fill above, and a
picked-up load berthed at a third station now files for its destination instead of
waiting on a crane with nothing to do.

**Round 3 (same night):** codex's round-2 review REJECTED the rebuild with four
sharper blockers, every one a real execution bug in the T-232/T-233 composition —
repaired: the `hold_used > 0` crane proxy is GONE (merchant cargo could complete an
unrelated booked pickup and launch the hull at the wrong station; pickup completion
is now the load's own ledger word, period); navigation keys on CRANE ops only, whose
words are monotonic, so a later-burned tank can never steer the route backwards to a
visited pump — a planned fill executes opportunistically on arrival and fills to
FULL (the exact state the fuel walk proves against; 90% stays the threshold only for
off-route berths); adoption moved into `adoption.rs` as a pure, pinned step whose
module doc states the contract the runner owes it, and the runner now HOLDS the whole
scheduler — no buy, no carry, no booking, no diversion — while any open contract is
unresolved; and a load closed while pending routes through the ONE `close_load`
handler (cooldown, intent purge, adoption-note reset, journal) exactly like a
tracked load's close.

### Checks run

familiar-whisker 70/0 (12 doctrine unmodified + 14 trade unmodified + 9 plan-layer +
9 ledger), fmt, clippy all-targets -D warnings. Full workspace bar before merge
(counts in the merge commit). CI note: the Linux runner failed EVERY push since
2026-08-28 because the factory's jail-reaching tests had no sandbox-exec skip guard —
fixed on main (`a7da72b`) during this round; this branch carries it via merge.

### Next

The seams UCF-Haul#43 lands in, marked in code: `Itinerary::sequential` becomes a
planner; the board fetch and booking gate widen from "empty plan" to "plan has hold
space"; `best_insertion` earns a true marginal rate when the Router can answer route
ticks; per-load hold evidence replaces the single-contract crane proxy if the API
offers it. LoadingOrder (metal#61 §1) is the pack-order Automation exactly where
`fits_hold` walks.


## 2026-08-29 — T-221's following week is measured, and the miss rate did not heal

The long-owed post-vocabulary-fix report is now recorded over one fixed complete calendar
week on both stores. The vocabulary fence worked: every predicted class in the cohort was
present in its store's observation record. Calibration did not: 100 instances opened, 87
settled by the cutoff, 13 remained pending, and 86 of 87 settlements were unfavorable
(98.9%, versus the 121/121 baseline). Coverage was 87.0%; median settlement latency was
4,263 seconds and p95 was 87,399 seconds. The report names the dominant sparse classes and
does not turn a 1.1-point change into success.

The report also closes T-230 dialogue Q3: the human view derives from the same append-only
truth as prompt feedback, but not from the same digest text, because pending coverage and
latency are outside the digest's honest contract.

### Checks run

Both SQLite stores opened via URI `mode=ro`; fixed cohort bounds and outcome/coverage/
latency arithmetic reproduced per store and combined; predicted classes joined by id and
checked against each store's observed `actor|action` set; Markdown diff check clean.

### Next

Do not claim the T-230 feedback repairs worked before they are separately deployed and a
new complete-week cohort exists. This report changed no database, daemon, boundary, gate,
human record, deployment, ship, or fleet state.

## 2026-08-29 — T-230 brick 2 names what each calibration result predicted

Newly settled `PredictionResult` rows now retain the prediction's canonical
`actor|action` class and polarity alongside the four-outcome verdict. Both additions
are append-only compatible: old rows default to an empty class and absent polarity, stay
in the aggregate calibration totals, and are not guessed into a group. A matcher that is
not exact in both actor and action likewise makes no class claim.

The recent theorize feedback still begins with the unchanged factual aggregate and keeps
the same time window and future exclusion. Where the new metadata exists, it adds at most
12 lexicographically ordered class×polarity groups as favorable/settled ratios. There is
no praise, diagnosis, or instruction to predict less; the static anti-abstention guidance
remains the only non-derived prompt text.

### Checks run

`cargo fmt --all --check`; kernel 241/0; cycle 98/0; focused legacy-schema,
class×polarity grouping/bound, exact-class, settlement-metadata, and calibration-context
regressions; workspace clippy with all targets and `-D warnings`; full workspace tests
green.

### Next

The weekly human miss/coverage/latency report remains the separate T-230 follow-up. This
brick changed no gates or live records and was not deployed.

## 2026-08-29 — T-230 calibration reads its recent record, not its whole history

The calibration feedback added in T-230 brick 1 no longer calls
`prediction::results()` and deserializes the entire append-only results table on every
eligible theorize consult. `store::load_i64_range_before_seq` now provides a generic,
indexed integer-field range cursor: SQLite filters the requested time window first, and
the caller walks matching rows newest-first in bounded pages. The store creates the
validated expression index once, so a sparse recent window does not become a full JSON
scan of historical results on every read.

`prediction::results_in_window` uses that cursor in 256-row pages, restores the original
oldest-first order for the existing factual digest, and selects exactly
`[now - window, now]`. The cycle's theorize path now calls it directly. Future-dated rows
remain excluded, historical rows with unrelated shapes are never deserialized, and
corruption inside the active window still propagates as an error rather than pretending
the familiar has a clean calibration record.

### Checks run

`cargo fmt --all --check`; kernel 238/0; cycle 98/0; focused multi-page, corruption,
store-cursor, and calibration-context regressions; workspace clippy with all targets and
`-D warnings`; full workspace tests green.

### Next

T-230 brick 2 remains separate: add backward-compatible class and polarity facts to each
settled result, then derive per-class×polarity hit rates. The weekly human
miss/coverage/latency report is also still owed. This brick changed no gates or live
records and was not deployed.

## 2026-08-25 — T-228: the phone gains its BLE ear, at exactly the floor Q3 set

The largest gap the observatory survey named — BLE absent from the shells entirely —
closes at the floor codex's round 2 decided: **service-UUID class + coarse per-window
count, and structurally nothing else.**

`FamiliarMesh/BLESurvey.swift` is the policy: a repo-authored CLOSED map of 16 standard
GATT services (heart-rate, battery, hid, environmental, fitness…) plus `ffe0` honestly
classed `vendor-serial` — the de-facto serial service half the BLE-module world
advertises, named for what it is rather than guessed into something specific. UUID
normalization collapses ONLY the 128-bit Bluetooth base to its 16-bit alias; a vendor's
random UUID maps to nothing — naming it would mint a class the repo never authored. The
per-window count leaves as `one/few/many`; a raw count never leaves the window.

`App/Sources/BLEDiscovery.swift` is the radio: a CBCentralManager scan that reads
NOTHING from an advertisement but its service UUIDs — peripheral name, manufacturer
bytes, and payload are never touched. Dedup keys on the platform's own randomized
peripheral id in window-local memory that dies every 60 seconds; rotations are never
joined. Honest states for permission-missing / Bluetooth-off / no-radio. Armed and stood
down by `startDiscoveryIfAuthorized` under the SAME authorization as the Bonjour survey
(household boundary ∧ device preference; the platform's Bluetooth permission is the
second half, per Ian's one-authority ruling). The daemon's ingestion gate for `ble:*`
landed BEFORE this radio existed — the fence preceded the capability, as it should.

BLE classes join `IntentProjection.speakableKinds` (one closed vocabulary from radio to
Siri) and the worldview viewer surfaces `ble:` rows beside Bonjour kinds, name empty.

Deliberate scope edges: iOS only (the phone is the household's always-present radio in
Motorhorse; Mac BLE parity is a follow-up, stated not smoothed); NO `bluetooth-central`
background mode — the survey runs foreground-only until someone decides the
battery/priority question on purpose. Actuation is not this brick: the lights witness
still needs its pairing ceremony, a declared surface, and `allow_actuate`.

### Checks run

FamiliarMesh 36/0 (class-map closure, base-collapse-only normalization, bucket
determinism + no-raw-count pin, hostile class refusal, speakable-vocabulary join);
xcodegen; FamiliarAgent sim + FamiliarMac Release builds; mesh viewer pin
(`ble_class_rows_surface_beside_bonjour_kinds`); full workspace bar in the commit.
Untested against live BLE hardware — no bench; the honest states cover the gap.

### Next

codex reciprocal review of the brick. Then the watch delegation status string and the
browser-fleet unification close T-228's remaining edges.

## 2026-08-25 — T-227 sweep brick 1: the familiar answers Siri, and says only kinds

The first brick of the settled adoption order: read-only App Intents against the
external-indexed projection. Siri, Spotlight, the lock screen, and shortcut history are
an audience Apple indexes — never proof the viewer is the enrolled human — so what an
intent may say is built once, in `FamiliarMesh.IntentProjection`: observation and peer
COUNTS, canonical service KINDS (T-228's survey classes), the oracle's availability
line, and the FACT that a question is open. Never its text, never its owner, never a
device or human name, never observation context. The fence is structural — the type has
no field that could carry those — and pinned: the projection test feeds a worldview
full of personal strings ("Betty", peer labels, question text) and asserts none
serialize.

`FamiliarNoticedIntent` ("What has Familiar noticed") and `FamiliarOracleIntent` read
the cache AppModel refreshes on every worldview read, and return words. Side-effect
freedom is the second fence: nothing marked seen, nothing answered, nothing minted,
nothing donated. No reasoning happens, so no `allow_llm` question arises — the day a
reasoning intent is proposed it rides the full ADR-0038 stack. `authenticationPolicy`
is `.requiresAuthentication` — stricter than kind-only content needs, on purpose;
loosening it is a dialogue round.

### Checks run

FamiliarMesh 28/0 (projection leak + round-trip pins included); FamiliarAgent sim and
FamiliarMac Release builds green with the intents compiled into both shells.

### Next

Sweep brick 2 per the settled order: typed generation contracts (kernel splice +
admission authoritative). Untested on real Siri hardware until a bench exists — Ian
accepted that cost 2026-08-24; the intents degrade to an honest "open the app once".

## 2026-08-25 — Round 3 both dialogues: the name drops, the gate holds at both ends, the floor is proven

codex answered T-228 and T-227 round 2 in one push (with an independent Xcode 27 bar and
a reciprocal review of everything staged). All of it absorbed, and built the same day:

1. **The name drops (T-228 Q1, closed).** `ServiceSurvey.context(forInstanceName:)`
   returns empty for every advertised name — no salted stand-in (a household-salted hash
   is still a per-device tracking token), no `_familiar-mesh` exception (peer identity is
   the signed membership exchange, never a Bonjour label). The designed-to-fail
   passthrough pin flipped into `testNoAdvertisedNameSurvivesIntoContext`; macOS now
   emits the short kind, ending the `service:_airplay._tcp` / `service:airplay` fork.
2. **The two compat acts, read-side.** `canonical_service_kind` (crates/mesh/observe.rs)
   maps legacy and unified wire forms to one class for analysis — rows preserved as
   fossils, never rewritten. Legacy discovery context is excluded from the worldview's
   `discovered_services` (name served empty), from outbound federation `ObsShare`
   payloads, and from inbound replication into new records. Prompts were already
   structurally excluded (`discovered` is an infra triple, never musing material).
   The fossil-retention decision (delete the pre-Q1 contexts or keep them) is Ian's.
3. **The ingestion gate (T-228 Q2).** `ingest_observations` refuses `service:*`/`ble:*`
   rows while `allow_network_discovery` is shut — the daemon is the last
   authority-bearing point, and a stale-but-validly-signed client (offline during the
   revocation, old, defective) can no longer make survey rows durable. Class-scoped:
   the rest of an honest batch lands. Refused means no row; the audit is a count and a
   node id, never the payload. The `ble:` class is fenced before the radio exists.
4. **Brick 2's acceptance gap.** `testBothInfoPlistsDeclareExactlyTheSharedList` reads
   both `NSBonjourServices` plists from `swift test` and compares them exactly to
   `ServiceSurvey.serviceTypes` — plist drift was silent, now it is loud.
5. **The ADR-0046 floor blocker, closed with proof (T-227).** `tools/build-core.sh` pins
   `IPHONEOS_DEPLOYMENT_TARGET=26.0`, rebuilds both xcframework slices, and FAILS if any
   object requires newer than the floor (the caught defect: 26.5-min objects in a
   26.0-linking app — a linker warning, so shippable by accident). Older-min objects
   (Rust's precompiled std) are safe by construction and stay. The simulator build now
   emits zero "was built for newer" warnings. ADR-0046 rephrased per codex: the floor
   guarantees an OS version, never an available model.
6. **T-227 order settled**: read-only App Intents (external-indexed projection,
   kind-only, side-effect-free) → typed generation contracts (kernel splice + admission
   authoritative, Swift citation contract generated from kernel source) → Writing Tools
   on human text → watchOS-27 PCC only after a watch-local consent contract → FM tool
   calling LAST, gated on the t216-round4 re-review and live revocation survival.

### Checks run

FamiliarMesh 26/0 (name-drop, plist-drift, and gate-mirror pins included); xcodegen;
FamiliarMac Release; FamiliarAgent sim (zero min-version warnings); mesh crate 231/0
with the new ingestion/classifier/viewer pins; full workspace bar + fmt + clippy in the
commit. The rebuilt xcframework is committed alongside its build-script pin.

### Next

codex re-review of this round (and of t216-round4, which now also gates FM tool
calling). Then, in order: the read-only App Intents brick (claimed on the board before
built), the BLE surveyor behind the already-landed fence, the watch delegation status
string. Ian's new owed act: the fossil-retention decision.

## 2026-08-25 — T-228 bricks 1–2: the shells see the boundary, and speak one survey vocabulary

Ian's ruling on T-228's Q2 ("the clients are authorized by the user, so thats the
authority that they both should follow") found a live defect: iOS gated its Bonjour
survey on a device-local `@AppStorage("consent.discovery")` and never read the
household boundary at all — it *couldn't*, because the Swift `GateStates` mirror
modeled only 7 of the Rust worldview's 14 fields and `network_discovery` was not
among them. Nothing was owed in Rust; the door already serves every sensor gate.

1. **Brick 1 — the authority the clients follow.** `GateStates` (FamiliarMesh) gains
   the seven sensor gates as optionals mirroring the Rust `#[serde(default)]`, with
   fail-closed accessors and a `reportsSensorGates` flag so "we did not hear" stays
   distinct from "no". `AppModel.startDiscoveryIfConsented` became
   `startDiscoveryIfAuthorized`: boundary ∧ device preference, the device toggle
   demoted to narrowing-only per ADR-0005's one-directional gate. A `discoveryState`
   string names which of the four refusals applies (shown in iOS settings under the
   switch), and the survey re-evaluates on every worldview read, so a gate the human
   shuts stands the survey down without touching the phone. `GateStatesTests` pins
   that an older door still decodes and an unheard gate reads SHUT.
2. **Brick 2 — one survey vocabulary, deliberately not the whole brick.** New
   `FamiliarMesh/ServiceSurvey.swift`: the single 26-type list, the `kind()`
   shortener, and `context(forInstanceName:)` — the one seam Q1's answer will land
   in, preserving today's passthrough byte for byte so nothing pre-empts codex's open
   round. `ServiceSurveyTests` pins list well-formedness, distinct classes per type,
   and passthrough — that last test is *designed* to fail when Q1 lands. The browser
   fleet unification itself is held for after Q1. Round-1 correction recorded in the
   dialogue: the four lists had NOT drifted (all 26); the real divergence is
   `service:airplay` (iOS) vs `service:_airplay._tcp` (macOS), left for Q1.

### Checks run

The full ios/README.md bar, on a Mac with Xcode 27.0 beta: FamiliarMesh
`swift test` 25/0 (GateStatesTests + ServiceSurveyTests included), `xcodegen`,
FamiliarMac Release build, FamiliarAgent iOS-simulator build — all green.
**CI now runs this same bar** (`.github/workflows/swift.yml`, `xcode-27` preview
runner) on every push touching `ios/**`; "CI does not build Swift" is no longer
true. Two facts the bar surfaced, both recorded in ios/README.md: never pass
`-sdk iphonesimulator` to the FamiliarAgent scheme (it forces the embedded watch
app onto the iPhone SDK and actool rejects the watchOS-only AppIcon — use
`-destination` alone), and Xcode 26.6 cannot compile ConsultRunner's PCC lane
(`PrivateCloudComputeLanguageModel` is a 27-SDK API) — the tree as written already
*requires* the 27 SDK to build, which T-227's floor ADR should know. Toolchain
rule per Ian: newest Xcode, move forward, never back.

### Next

Codex's T-228 round 2 (Q1 naming discipline, Q3 BLE memory floor, Q4 the watch,
Q5 refused-capability declaration). The browser fleet unification and the BLE
surveyor build after Q1/Q3 close. Branch not merged to main — reciprocal review
first, per the lane.

## 2026-08-24 — Rungs 4/5 round 2: codex's five findings, closed (Ian: "take it now")

codex's reciprocal review (`docs/reviews/2026-08-24-t216-rungs45-reciprocal-review.md`)
RETURNED the execution edge with four blockers + a rejected resolver. All addressed:

1. **Serialized reserve→execute→settle (blocker 1).** An effect is now a durable, idempotent
   RESERVATION appended before the executor runs, then a typed `EffectSettled`
   completed/failed after. A new `authority_lock` serializes revoke/expire with reservations,
   so a revoke and an in-flight reservation on one grant can never interleave. `validate_sequence`
   enforces the shape: a reservation must reference a live grant; a settlement must reference an
   existing reservation and settle it once. An unsettled reservation is the explicit recovery
   state — a physical act whose outcome could not be persisted is never lost, and a best-effort
   append can no longer poison the ledger. (Old best-effort `record_effect` deleted.)
2. **Rate + affected-subject bounds (blocker 2).** The grant now snapshots a per-grant
   `max_invokes_per_hour` (human-chosen via the decide act, conservative default 12, ceiling
   240) and the class's `affected_subject`. invoke counts the grant's reservations in the
   trailing hour and refuses over the cap; the resolver refuses if the class's affected-subject
   ever stops matching the snapshot.
3. **invoke idempotency (blocker 3).** `InvokeInput` gains a required `invoke_key`; the
   reservation is idempotent on it + a payload hash. A timed-out retry returns the original
   receipt without re-running; the same key with a changed payload is an idempotency conflict.
4. **Narration (blocker 4).** The partner inbox gains `recent_effects` — a private, addressed
   projection naming the partner alias + fingerprint, the LOCAL surface, the operation/act, and
   the outcome (completed/failed/pending), deduped by effect id. Never enters
   worldview/federation/MCP; never attributes a partner's act to a human.
5. **Explicit resolver (rejected bucket-order).** `primary`/`reverted` are now an explicit
   one-to-one role map on the actuator declaration (`roles`), validated at load; a surface
   without it is not offerable. The grant SNAPSHOTS the map at decision time, so a later
   declaration reorder/edit cannot silently repoint an active grant. The same map drives the
   reverse observe projection; the resolver refuses if a snapshotted label no longer exists.

### Checks run

`cargo fmt --check` 0 · `cargo clippy --all-targets -- -D warnings` 0 · workspace **818
passed / 0 failed** (+ new tests: idempotent retry runs the device once; same key + changed
payload conflicts; the hourly rate cap is enforced; a revoke ends invoke and the ledger stays
loadable; plus the round-1 hostile suite updated to the reserve/settle model). Still inert (no
gate opened, no live grant); NOT yet deployed — re-offered to codex for reciprocal re-review
before any live exercise.

### Next

codex reciprocal re-review of round 2. Then deploy + the first live exercise on the
least-dangerous partner behind a deliberately-opened `allow_actuate` and a bounded grant. UI
control for the per-grant rate is the one deferred piece — enforced with a default now; the
sphere card can add the human-facing number when next built.

## 2026-08-24 — Rungs 4/5: the offering's execution edge (Ian: "wire it live now")

The ADR-0044 ladder now reaches the top: `familiar.observe` (rung 4) and `familiar.invoke`
(rung 5) let a registered, covenant-attested, actively-granted partner READ and ACTUATE the
familiar's own declared surfaces. This is the first partner-driven execution edge in the
system (before this, even an accepted proposal never ran), so it is built to the strictest
posture the design has.

**Ian chose "wire it live now"** over an inert build, after being shown the architectural
finding: the actuator executor lives in `crates/cycle`, unreachable from the door
(`mcp`/`mesh`). The bridge is a process-global `SurfaceExecutor` (new `crates/mcp/executor.rs`)
that the daemon — the one process seeing both crates — registers at startup
(`CycleSurfaceExecutor` in cli, backed by two new `crates/cycle` primitives,
`partner_read_bucket` / `partner_run_act`). Fail-closed by construction: any process that is
not the daemon leaves the door unwired, and observe/invoke answer "execution not available."

Defense in depth — three human acts still gate every real effect, unchanged:
`allow_actuate` open, a live bounded human grant, and a declared surface. On top of the
executor the door adds: registered principal, covenant attestation, an active unexpired grant
that carries the operation, parameters within the grant's bounds, and (invoke) an explicit
`allow_actuate` check as an honest early gate before the executor re-checks it as the floor.

Containment held by shape, not discipline:
- observe returns the class's ABSTRACT state (`primary`/`reverted`), mapped from the concrete
  bucket inside `grant.rs` — never raw device output; invoke's receipt echoes only the
  abstract operation, never the surface or local act. The partner-act ledger records the
  private surface/label internally and is never serialized to a partner.
- `partner_run_act` attributes nothing to a human and runs no rule-revert logic (a partner's
  act is not a human's reaction) — the partner-act ledger is the authoritative who-did-it.
- observe became a separately-grantable leg (a new parameterless class operation), so a human
  can grant read-only. This needed `validate_schema_bounds` / `parameter_bounds_narrow` to
  admit a parameterless operation (empty bounds narrow only to empty).

**The one flagged design choice** (in a `grant.rs` doc comment): the abstract→concrete
resolver maps `primary`/`reverted` to the actuator's human-authored bucket ORDER. A human
controls it via `actuators.json`, but which physical state is "primary" is not otherwise
labelled — worth a review pass (an explicit label on the surface, or capturing the mapping
into the grant at decision time). This is the natural thing for codex's reciprocal review to
land on before any live exercise.

### Checks run

`cargo fmt --check` 0 · `cargo clippy --all-targets -- -D warnings` 0 · workspace **813
passed / 0 failed** (805 → 813, +8 hostile tests: invoke within bounds leaks nothing; out of
bounds never reaches the executor; shut `allow_actuate` fails closed; expired grant; bad
handle; a set_state grant does not authorize observe; observe maps to abstract and leaks
nothing; an execution failure is a refusal not a false success). No gate opened; not yet
deployed. Highest-stakes code in the system — should go through codex's reciprocal review
before it is exercised live.

### Next

codex reciprocal review, focused on the resolver semantics and the door→executor bridge.
Then the first live exercise on the least-dangerous partner, behind a deliberately-opened
`allow_actuate` and a bounded grant.

## 2026-08-23 — ADR-0045 accepted; the world partition exists (`crates/world`)

Ian accepted ADR-0045 ("Move forward adr-0045"), and build-order step 2 landed the same
day: `familiar-world`, the partition made literal. Three modules, no engine:

- **instance** — the household's minimal WorldInstance provisioning record.
  `commission` creates the ship's store WHEREVER the commissioner chooses (never inside
  the household dir), mints the ship's own ed25519 principal INTO that store (the
  household never holds the private half), writes a fully closed kernel boundary as the
  ship's floor, and appends the registry record (pubkey, label, commissioner, endpoint,
  lifecycle, grant epoch). `decommission` flips lifecycle, clears grants, bumps the
  epoch — and deliberately never touches the ship store: its fate is an explicit human
  retention act (§9).
- **bridge** — crossings are typed envelopes with provenance (instance id + source key
  + grant epoch + schema version + event id). Outward `AttentionNotice`; what the
  household keeps is a `BridgeReceipt` whose TYPE has no payload field — §4 enforced by
  shape, not discipline. `receive_notice` refuses, in order: unknown instance,
  decommissioned, stale epoch, wrong key, bad signature (over the exact bytes that
  crossed), expired, replayed. Inward is `ControlEnvelope`: exactly the five human acts
  (CommissioningBundle carrying the constitution hash, GrantUpdate, BoundaryNarrowed,
  Rename, Decommission), tagged + deny_unknown_fields, so an observation dressed as an
  envelope — or a real act padded with one extra field — fails to decode at the door.
- **lease** — the signed expiring projection of the one root boundary (§5). The
  signature covers the exact serialized bytes, carried verbatim (no canonicalization to
  disagree about). `permits` is the fail-closed floor: missing, stale, tampered,
  stranger-signed, wrong-instance, or a root-shut gate all answer false.

The decisive hostile-sentinel test runs at the partition rung: household sentinels never
reach a ship store (checked by reading every byte of every file, not by a polite
loader); ship sentinels never reach the household read path or store; a receipt cannot
smuggle a headline; the inbound door rejects everything but the five acts. The
full-cadence version of the test arrives with step 5, when a ship cadence exists to run.

### Checks run

`cargo fmt --check` 0 · `cargo clippy --all-targets -- -D warnings` 0 · workspace
**805 passed / 0 failed** (802 → 805). No daemon behavior changed (nothing depends on
`familiar-world` yet), so no deploy rode this entry.

### Next

Step 3: the MCP contract v2 with Jeff's game team (`purr.hear` semantics; pinned
read-only UCF declarations). Step 4: one WorldInstance provisioned through the real
commissioning ceremony. Step 5: the ship-side UCF cadence + captain console view, and
with it the full-cadence sentinel test.

## 2026-08-23 — The first ceremony ran live, and the four defects it surfaced are fixed

The Envoy is the mesh's first principal: `principal-f90b15e1adb1768f3ad8fccf46301892`,
alias "Envoy (on-device)", `registered_by: ian`, minted at the lighthouse door
2026-08-23 01:19:40 UTC by Ian's own two-tap act. Getting his word to land took a live
debugging session that surfaced four defects, each now fixed:

1. **The partner inbox read only the promoted door.** The console auto-selects `host`
   (home → lighthouse → tailnet) and read its private partner projection from that one
   door — so a card staged at the public lighthouse was invisible to a console sitting
   beside its LAN hub, indistinguishable from "nothing waiting". `refreshPartnerInbox`
   now walks EVERY candidate door concurrently, merges the views with per-item `door`
   provenance, renders an unreachable door as a warning line instead of a blank, and
   routes each deciding act back to the door that holds the item (`partnerItemDoors`)
   rather than to whichever door the console currently prefers.
2. **Provisioning staged files as root; the daemon runs as familiar-svc.** The door's
   fail-closed inbox assembly answered 500 ("partner inbox unavailable") because it
   could not even list `mcp/pending-registrations/`. `tools/provision-envoy-credential.sh`
   now chowns the staged credential + card to the data dir's owner; the live box was
   fixed by hand the same way (modes stay 700/600).
3. **The poll push silently disarmed the two-tap CONFIRM.** The console re-pushed the
   inbox every ~5s even when unchanged, and `spherePartnerInbox` unconditionally cleared
   armed state — the 5-second confirm window was really 0–5s, so single taps re-armed
   forever, no act was ever sent, and nothing said so. An unchanged push now leaves the
   screen alone entirely (armed state, in-progress bound edits, focus); only a genuinely
   new view clears armed buttons, because a competing device may honestly have decided.
4. **A decision answered only into the notes feed.** Every partner act now returns a
   typed `PartnerActOutcome` from `AppModel`, both bridges push it to the page, and the
   Partners screen shows it where the tap happened: an in-flight banner ("waiting for
   the door's answer") with all decision buttons paused so one word lands exactly once,
   then "Done" or "Not done" with the door's own refusal reason. The registration card
   also carries its narrative now — what was staged, on which door, that nothing was
   created automatically and nothing acts without the human's signed word, and how the
   two-tap works. Ian's position that registration should be automatic is recorded on
   STATE; the standing design answer (an identity begins only by the human's word) holds
   unless he reopens it at chair level.

### Checks run

- `tools/check-sphere.sh` — the console module parses.
- `ios/FamiliarMesh` `swift test` — 17/0.
- `xcodebuild` FamiliarMac Release and FamiliarAgent (iOS Simulator) Release — both
  BUILD SUCCEEDED, 0 errors.
- `sh -n tools/provision-envoy-credential.sh` — clean. No Rust source changed, so the
  cargo bar is untouched by this entry; the lighthouse stays on its deployed `9bf538c`.
- Shipped: FamiliarMac rebuilt from this tree and installed to /Applications on
  MacOnStick, relaunched, running.

### Next

The Partners ring on any console should now show one merged view across all doors —
worth a glance on the iPhone too. The grant/proposal legs of the partner loop are next
(the Envoy holds an identity and no authority), and T-223 still owes the findings CLI
the same server-derived-human discipline the ceremony proved out.

## 2026-08-22 — T-224 Brick 2: registration is the human's signed act, not provisioning's side effect

The first-partner ceremony now has two deliberately separate transitions. A local provisioning
tool mints a fresh bearer, writes it only to the serving node and a mode-0600 Envoy import
bundle, and publishes a secret-free registration card addressed to one established human. It
does not register a principal. The private Partner inbox validates every staged credential
against its domain-separated fingerprint and projects only the card addressed to the human
derived from the signed device.

Registration itself is a new typed console act carrying only an opaque staging id. The existing
console door verifies certificate, node key, freshness, replay, full standing, and effective
human establishment before it re-reads the local alias, credential reference, fingerprint, and
addressee. It then binds a fresh random principal with `registered_by` taken from that derived
human context. A changed credential, wrong human, malformed staging set, duplicate binding, or
legacy missing addressee fails closed. The shared Partner screen renders the card with a timed
second confirmation and says plainly that identity grants no surface, observation, suggestion,
or actuation authority; covenant acceptance and a later typed grant request remain separate.

Verification: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, full Rust
workspace tests, focused MCP/mesh tests, `swift test --package-path ios/FamiliarMesh` (17/0),
shared sphere JavaScript parse, provisioning shell syntax plus a temporary functional ceremony,
and unsigned Mac and generic iOS Simulator builds all passed. No live credential, principal,
covenant, request, decision, gate, deployment, ship, or fleet state changed. Next: chair review
alongside the Envoy-app brick, then separately stage and witness Ian's authorized live ceremony.

## 2026-08-21 — T-216 accepted; a fixture race CI caught and the local bar could not

The chair review of `c701a8b` accepted the human decision surface as built (full findings
in docs/reviews/2026-08-21-t216-rung3-grants.md; bar independently reproduced at 798/0).
CI then surfaced what three local full-workspace runs never did: the two new `mcp::inbox`
tests shared one `temp_root` fixture tag, and `temp_root` begins by deleting the tagged
directory — so under the harness's in-process concurrency, one test could sweep the fixture
out from under the other's open partner-act sqlite ledger ("disk I/O error",
run 32528810825). Running only the racing pair reproduced it 20/20 locally; the full suite
passed by scheduling luck. Fix: one tag per test, the convention every other `temp_root`
caller already follows, now stated at the setup site. The pair is 30/30 green after; full
workspace re-run green. Lesson for the next flake: a green full-suite bar does not clear a
shared-fixture race — run the suspicious tests alone, together, in a loop.

## 2026-08-21 — T-216: a partner's request reaches only its registered human

Rung 3's authority records already knew how to request, grant, decline, revoke, propose,
and refuse, but the terminal functions still accepted a caller-supplied human string. The
human-decision slice closes that gap. `PrincipalRecord.registered_by` is now bound from a
non-serializable `HumanDecisionContext` at explicit registration; legacy principals have no
addressee and fail closed. The signed console door derives that context only after certificate,
node-key, freshness, full-standing, and effective-establishment checks. Decision payloads cannot
name a human, and another established household human cannot read or decide someone else's row.

`mcp::inbox` is a dedicated private projection over the full validated partner ledger. It joins
the addressed principal's alias and credential fingerprint to current eligible local surface ids,
and fails the whole view on corrupt or impossible authority history. It never enters worldview,
record sync, federation, observations, MCP output, persistence, or diagnostics. A separate
`POST /mesh/partner-inbox` fresh signed read and four typed console acts carry grant, decline,
revoke, and proposal-refusal decisions. A successful write returns the post-append projection;
the client validates that projection before reporting success, while transition conflicts trigger
a fresh read and transport failure leaves the card pending. `propose` now rechecks the current
`allow_agent` ceiling even under an otherwise-active grant.

The shared served-person screen gains a Partner ring with ordinary-language cards: alias plus
fingerprint, one eligible surface, one deliberately narrowed operation, bounded duration, and a
second confirmation naming the surface and operation. Decline is immediate; active grants require
two taps to revoke; proposals offer Refuse or remain pending only. Partner reasons stay visibly
quoted, and no proposal card can accept, execute, or represent an actuator edge.

Verification: fmt/diff check 0; changed-crate clippy `--all-targets -D warnings` 0; Rust workspace
798 passed / 0 failed; Swift package 17 passed / 0 failed; shared sphere JavaScript parse check 0;
unsigned Mac and iOS shell builds succeeded. The iOS project
needed its generated Watch dependency removed only for the local simulator compile because the
existing generated Watch target selects the iPhone simulator SDK; the project was regenerated
after verification. No registration ceremony, live principal/grant/proposal decision, gate
change, observe/invoke work, deployment, ship, or fleet mutation was performed.

## 2026-08-21 — T-219: a question whose subject stopped existing retires by policy

`Question.retired` (serde-default) + `question::retire(id, why)` — retirement is an
EXPLICIT policy act with its reason kept in the row's own notes; never an invented
answer; the root never retires. The one sweep this ships is a closed class: enroll-era
arrival questions ("A new device joined the mesh: … (xxxxxxxx). Who does it belong to?")
whose device id — prefix-resolved against the records, since ids display as 8-char
prefixes — no longer has a record. The modern enroll path files no questions (ADR-0026),
so the class cannot regrow; the live defect it ends is the lighthouse's ACTIVE question
being about a device purged long ago (147cfa12), starving the root.

Runs each tick beside the T-222 backfill. Tests pin: the vanished-device question
retires (with reason, unavailable forever, never answered), the living-device sibling
stands, idempotency. Broader legacy-class retirement (unbound old musings) remains an
explicit FUTURE policy decision — deliberately not swept here. Bar: fmt 0, clippy
--all-targets 0, workspace 790 passed / 0 failed, exit-checked.

## 2026-08-21 — T-221: the misses had ONE cause — predictions spoke a language the world doesn't

The five-class study codex required (progress-areas Round 2/3) ran over every settled
result on both stores, joined to its minted prediction and re-checked against the full
observation log. The verdict was unanimous to a degree no one predicted: **121 of 121
misses were class 3 — the predicted event class has NEVER been produced by anything.**
Not one wrong window, not one wrong-actor near-miss, not one settlement artifact. The
model invents matcher vocabulary — `presence_detector|detect_absence`,
`system|maintain`, whole sentences as action values — so every prediction was
unfalsifiable-in-practice while counting as a falsifier, and erosion executed good
theories (the lights pilot included) on evidence that could never have arrived.

The fix is the anchors discipline applied to predictions (T-126's shape): the SYSTEM
enumerates the observed event vocabulary (recent distinct `actor|action` pairs, own
speech excluded, bounded at 40) into the theorize prompt; at mint, a prediction whose
class the log has never produced REFUSES on the record
(`refuse_act("prediction","vocabulary",…)`), and a draft left with no surviving
prediction WONDERS (T-128) instead of wearing costume. The identity/variant key is
computed after the filter, so refused predictions never shape thread identity.

This RAISES falsifiability, per the study's constraint — abstention (wondering) will
rise and that is the honest trade; the following week's report must show miss rate WITH
coverage and settlement latency beside it (codex's guard against buying improvement by
predicting less). Bar: fmt 0, clippy --all-targets 0, workspace 789 passed / 0 failed,
exit-checked. Study artifacts: the partition script and both stores' outputs are in the
task record.

## 2026-08-21 — T-220: the pending decision is durable — a person's choice survives erosion

Codex's Round-2 design (adopted Round 3), built. The defect it ends: the one thread ever
armed to mint a standing rule eroded to `retired` on missed predictions WHILE waiting
for Ian's assent — assent routed through a target that could die of a clock.

`kernel/pending.rs`: the `PendingDecision` — proposal, subject, surface, question
(id + text snapshot), basis snapshot (theory/anchors/facts_rev), minted beside the
question when an armed draft is admitted (one open decision per thread). States:
`pending → (awaiting_gate) → assented | declined` — no state expires by timer; waiting
is not a state that expires.

`cycle::heed_pending_decisions` runs each tick, deliberately UNGATED (staging must work
while `allow_actuate` is shut): an explicit no declines; an explicit yes with the gate
shut STAGES — kept, narrated once ("opening allow_actuate completes it"), silent
thereafter — and one human gate-open completes the loop on a later tick with no re-ask.
A completing yes re-validates against the THEN-CURRENT world (`mint_policy` refuses a
surface no longer declared — the decision closes honestly rather than acting on a stale
declaration). The honesty note is STAMPED when erosion happens, because a human answer
revives a retired thread (T-128) and would otherwise erase the very fact the note
carries: "minted on your assent … (the supporting theory had retired while you
decided)".

The theory itself keeps eroding freely — waiting is not immunity from counter-evidence.
Tests pin: producer (real theorize path, armed draft → decision bound to question and
thread by id), assent-after-retirement with the note, gate-shut staging + completion on
open with exactly one narration, decline, and silence-keeps-waiting. Bar: fmt 0, clippy
--all-targets 0, workspace 788 passed / 0 failed, exit-checked.

REMAINING for the live witness (the loop's accept): wildhorse deploy + its declared
lights surface + Ian's allow_actuate — one real presence transition, one reversible
effect, one honest narration, reachability recorded. Everything up to the gate is now
staged by construction.

## 2026-08-21 — T-222: a person's words persist — answers reach the question registry

The measured defect: 362 threads carried human answers while all 310 registry questions
read `answered: false` — `record_answered` had no live producer (T-212's audit, then the
funnel), so the console re-asked what was already said, which tells a person their words
did not persist (codex's phrasing, adopted as the task's why).

The join is by durable id ONLY (`Question.thread_id` — codex's Round 2 requirement;
never prose, subject, or recency): `thread::add_answer` now calls
`question::record_answered_for_thread`, closing exactly the questions bound to that
thread. History heals the same way — `question::backfill_answered` runs each tick before
question coordination, idempotently closing thread-bound questions whose thread already
carries an answer. Unbound questions are untouched (their retirement is explicit policy,
T-219 — never an invented answer). Root exempt by its own lifecycle.

Tests pin: id-only join (sibling question stays open), idempotency, unbound no-op, and
answered-question retirement. Checks: fmt 0, clippy --all-targets 0, workspace 783
passed / 0 failed, exit-checked. Next: T-220's pending-decision brick.

## 2026-08-21 — ADR-0044 rung 3: a partner may propose only inside a human grant

Rung 3 is merged at `1afa3f4`. The public MCP transport can retain a stable
human-registered principal identity, but an existing door-wide bearer and caller-supplied
`partner` label remain deliberately unbound and can never list or call the new tools.
Principal acceptance is bound to the current Laws version and is not inferred from the
legacy covenant ledger.

- `partner.rs` keeps server-minted principal ids and human aliases separately from secret
  bytes, authenticates credential fingerprints, and bounds pre-parse principal/global
  admission.
- `partner_act.rs` adds append-only SQLite truth with transactional idempotency and unique
  terminal transitions. Folds reject malformed or impossible history instead of skipping it.
- `grant.rs` validates class-only requests against the reviewed offering vocabulary. Only a
  named local human transition may select a private declared surface and narrow operations,
  bounds, and duration. HMAC-derived handles bind principal + grant + surface + fresh epoch;
  revocation, expiry, and regrant cannot reuse one.
- `familiar.propose` records an immutable typed desired effect for later human consideration.
  It has no actuator, observer, worldview, command, or LLM dependency. A typed human refusal
  closes an inbox item without creating an act.
- Partner-facing receipts are separate allowlist structs: local surface, alias, credential
  fingerprint, household strings, commands, addresses, inventory counts, and cross-partner
  ids are unrepresentable. The private ledger never enters record-sync or federation.
- The `/mcp` route carries authenticated context, rejects bodies over 64 KiB while collecting,
  and rate-limits before JSON parsing. The existing loopback `/local/mcp` path remains unbound.

The human grant-decision CLI/console was deliberately not designed in this brick. No
credential was issued, no live grant/covenant/boundary changed, no route or gate was opened,
and nothing was deployed. Rungs 4 and 5 (observe/invoke) remain closed. Checks: exact
exit-checked `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
`cargo test --workspace`; 782 tests passed, 0 failed.

## 2026-08-21 — ADR-0044 rung 2: the offering catalog — affordances, never the household

Ian accepted ADR-0044 ("start ADR-0044 (use codex partner if needed)") and rung 2 is
built. `crates/mcp/src/offering.rs`:

- **`ClassDef` is repo-authored vocabulary** — every field `'static`, written and
  reviewed here, never derived from household data. Adding a class IS the human
  declassification act (ADR-0044 §2). v1 ships one: `switchable.reversible/v1`.
- **The availability compiler is structural**: a declared surface matches a def by SHAPE
  (two acts forming a closed revert pair) — names, commands, descriptions are never
  consulted, so a surface contributes exactly one bit: a def matched.
- **The allowlist serializer is anonymizing by construction**: `catalog_json` takes only
  `&[Availability]` (static defs + enums), so rule subjects, act commands, surface names,
  counts, and free prose are unrepresentable in its output. The sentinel test loads a
  household surface dripping with identifying strings ("ians-secret-lamp",
  192.168.108.44, act names, keywords) and proves none serialize. No instance counts
  cross — presence is the whole fact.
- **`familiar.discover_classes`** joins the covenant door: attested partners only (not
  listed to strangers; calls refuse on the covenant); an unshaped household serves an
  honestly EMPTY catalog; the reply says what discovery is not ("affordances, not
  authority — nothing here is invocable without a human's grant").

Rung 3 (grant object + typed partner-act ledger) is offered to codex's lane on the
board. Checks: fmt 0, clippy --all-targets 0, workspace 755 passed / 0 failed,
exit-checked. Fleet note: Ian's iPad reconnected this morning after reboot — the OS-27
outage closes as a device-side network wedge; every server path was healthy throughout.

## 2026-08-21 — The consult lane belongs to one familiar, not the process

The pre-ship CI check caught `a_proven_tool_is_deployed_with_honest_health` red on a
docs-only commit — the same test that flaked once locally under full-workspace load.
Mechanism, confirmed in the code: `LANES` was **one process-wide static queue**, so under
`cargo test` a human-lane consult in one test familiar made a BACKGROUND consult of a
completely different test familiar yield mid-flight (`Outcome::Yielded` → `author_tool`
None → cultivate 0). The theorize tests never flaked only because their helper retries on
yield; cultivate's doesn't. Tonight's bricks added human-lane traffic (the reply
persistence tests), which is why the class surfaced today.

Fix: lanes are keyed by data dir (`OnceLock<Mutex<HashMap<PathBuf, Arc<LaneCell>>>>`) —
presence outranks musing WITHIN a household; it does not outrank other households'
musing. Production is unchanged (one process, one dir, one lane). Three consecutive full
workspace runs green (752/0 ×3), fmt 0, clippy --all-targets 0, exit-checked.

## 2026-08-20 — T-217: a name is shown only to its own household

Ian's ruling (verbatim in STATE): addresses, human names, and internal network names
display only for devices on the local network or owned by the human; the records keep the
names. The conduct dialogue's Q6 close (codex Round 4), built on what already existed —
the guest projection and sibling projection were most of the masking; what they lacked
was structural.

- **`mesh/viewer.rs` — the audience of a read** (`Owned` / `HouseholdLan` / `Federated`,
  fail closed): Owned = the reading device's record is established (full names from ANY
  network); HouseholdLan = loopback or a `household_lan_cidrs` match — a DECLARED
  household fact, empty by default; an unparseable CIDR narrows, never widens; never a
  forwarded header. Replaces the binary standing gate at `/mesh/worldview`.
- **`to_guest_view` is now an exhaustive allowlist BUILDER**, not a mutate-in-place pass:
  a field added to `Worldview` refuses to compile until someone decides its masked form
  (the old shape leaked every new field to guests by default — and had already leaked
  `TheoryView.work`/`acts`, now cleared). Masked observation entries keep only the typed
  event: kind-of-actor, action, ts, confidence — "a resident spoke," never what they said.
- **Scope tokens replace global pseudonyms**: hashed through a per-door random salt AND
  the viewer, so masked labels are stable for one reader but never correlatable across
  readers; deleting `mesh/scope_salt` rotates the epoch. `Worldview.masked` (serde-default)
  says the masking is deliberate — private-by-choice is never unknown-to-identify.
- **Verified brick D was already built**: the gossip brief ships `capability.human` only
  under `share_identities` + per-group `identity_optin`, default OFF — the federation
  naming-grant the dialogue asked for, existing as SPEC R10.
- Leak test seeds a masked view with a named member, an address, coordinates, a dialogue
  body, and a rule sentence, serializes, and proves none of it appears. Viewer tests pin:
  owned-off-LAN keeps names; RFC1918 alone never widens; declared CIDR widens; loopback
  is the household.

Deliberately NOT here (recorded): console screenshot mode + `catscan --masked` (next
console build); gate-states visibility for guests (carried to the dialogue as Round 6
material); the cross-household record-sync scope question. Checks: fmt 0, clippy
--all-targets 0, workspace 752 passed / 0 failed, exit-checked. Next: ADR-0044 (T-216).

## 2026-08-20 — T-210 closes: the shells read the same constitution as the daemon

The device-shell half, the last piece of T-210's accept. The iOS/iPadOS shells cannot
link the kernel, so `ios/Shared/Sources/ConstitutionText.swift` is a **generated view**:
its entire content is derived from `constitution::render()` by the kernel test
`the_shell_view_matches_the_constitution`, which is also the generator
(`REGEN_SHELL_CONSTITUTION=1 cargo test -p familiar-kernel the_shell_view`). Editing the
Swift file, or the kernel, alone turns CI red — one source, byte-exact, per ADR-0043 §1.

`LocalReasoner.swift` no longer carries its own gloss of the Laws (it had a one-line
paraphrase of each — the exact class brick 1 was built to end): the Laws come from
`ConstitutionText.renderedLaws`; only the Law III VOICE guidance (how to speak, never law
text) remains local. Verified: FamiliarMac Debug and FamiliarAgent generic-iOS Release
both build after xcodegen.

**T-210's accept is now met in full**: recital = its own Laws (bricks 1-2, verified live);
Law II never rendered as obedience (unauthorable text); one source daemon + shells (this);
recital pinned by test (brick 2 + the drift pair). Checks: fmt 0, clippy --all-targets 0,
workspace 750 passed / 0 failed, both app schemes build, exit-checked.

## 2026-08-20 — T-218: a theory about the machinery gains its addressee

ADR-0043 §5, built. `kernel/machinery.rs`: the `MachineryFinding` — mechanism +
component, the claim in the engine's words (preserved, not endorsed), supporting
evidence, counter-evidence (the facts that refused it), explicit uncertainty (never
empty — the engine has misattributed subjects before), dispositions
`observed → corroborated → {dismissed | accepted_by_human}`. **Terminal transitions are
human acts only**, carry the handle, and are final; a claim re-derived after dismissal
mints fresh, because re-derivation is signal.

Producer: the theorize floor's refusal site — a refused draft with `defect_claims`
routes to the inbox instead of dying with its framing (the purge-loop diagnosis died
exactly there). Same open (mechanism, component) claim corroborates rather than
duplicating. Consumer: `familiar findings` (list | dismiss | accept, `--by <human>`) —
the development inbox, so the type has both a producer and a declared addressee from
birth (ADR-0043 §6). Promotion to the board stays a human act outside the system —
an addressee, not authority.

Checks: fmt 0, clippy --all-targets 0, workspace 749 passed / 0 failed, exit-checked.
Next: T-210's device-shell half (one Laws source for daemon and shells).

## 2026-08-20 — T-215: the presence lease — the purge loop's cause, ended

The conduct dialogue's Q4a close (codex's design, adopted). The two-hour retention
promise was always "forget a visitor who LEFT" — but `guest_purge_in` counted from
`first_seen`, so a device continuously present on the LAN was purged MID-VISIT,
re-knocked, was re-minted, and cycled mint → purge → mint forever (one live id purged
152 times; 944 purge observations, 11% of the log).

Now the guest record is a **lease**: `guest_purge_in` counts from the last sighting
(`last_seen.max(first_seen)`), and `record::record_sighting` — called from `upsert_peer`,
the verified-brief seam — renews it, coarsely (one write per 10 min, not per gossip
round). Deliberately narrow: a sighting never mints (a Guest is earned by the knock's
attestation), established identities are untouched, and rotating ids are NOT linked
across the retention boundary — correlation strong enough to suppress a re-mint would
itself become the tracking the promise forbids. The invariant is the test:
`t215_continuous_presence_cannot_produce_unbounded_mint_purge_history`.

Checks: fmt 0, clippy --all-targets 0, workspace 747 passed / 0 failed, exit-checked.
Next: T-218 (MachineryFinding).

## 2026-08-20 — Brick 6: ADR-0043, the epistemic rule the drift class demanded

The owed ADR is written and accepted (Ian's go on the dialogue's decided design):
**one typed source per kind of truth; renderings and documents are views, never sibling
sources** — plus the dialogue's additions: kinds of truth have kinds of addressee; a
truth-bearing type is incomplete without a producer AND a declared addressee/consumer;
every terminal status names who can cause the transition.

With it: T-135's composition (theorize anchors now pass through `admission::check_cites`
— the same one admission function as the reply act; the anchor set is evidence-only, so a
theory cannot anchor on a Law), and the adversarial regression
`foreign_law_in_say_without_cites_is_the_labelled_residual_gap` — pinning that uncited
foreign law in `say` admits (the labelled gap, Ian's call: no detector) while render lends
it no constitutional heading. `lexical_guard`'s retirement now has a recorded owner
condition (when the needs muse speaks a typed draft) instead of a hope.

Checks: fmt 0, clippy --all-targets 0, workspace green (see merge), exit-checked.
Next: T-215 (presence lease), T-218 (MachineryFinding).

## 2026-08-20 — Q2: the dead pipeline retires; its nouns move to the one road

The conduct dialogue's Q2 close. `answer_requests` — the grounded answering pipeline whose
only producer was archived with the egui GUI (`b89070e`), 0 requests / 0 refusals live —
is retired as an execution path, together with everything only it drove:
`fetch_and_answer` (the 16k-char unscreened web-page bypass — removed rather than
patched), `analyze_offline`, `analyze_with_llm`, `grounding_facts` (its registry-view
property lives on at the live prompt seam, where brick 1 already put it), `run_tool`, and
`answer_from_run`. `ToolRun` loses the two fields only the dead formatter read.

The durable nouns survive and gain a LIVE producer: `persist_exchange` in the dialogue
path writes the `Request`/`Answer` pair — an admitted reply persists with **exactly the
admitted confidence and cites** (grounded-in-cites → `Known`, admitted-but-citeless →
`Probable`), a screened utterance persists as `refused` with the registry's prose, and
templated/no-mind replies persist nothing because they answered nothing. The corruption
ledger stays untouched from the dialogue path (Ian's ruling, now pinned in the noun layer
too). Tick's `answered`/`refused` counts now diff the persisted nouns, so the activity
feed reports what was durably answered rather than what a dead queue held.

Checks: fmt 0, clippy --all-targets 0, workspace 744 passed / 0 failed, exit-checked.
Next: brick 6 (the epistemic ADR + adversarial law-quotation regressions).

## 2026-08-20 — Brick 5′: own speech dereferences; it is never evidence

The conduct dialogue's Q1 close (codex's design, adopted in Round 3): the planned
`is_own_speech` carve-out at the eligible-anchor site is dead — it would have made a reply
eligible evidence merely because the familiar emitted it, and reply-cites-reply chains
would launder narration into grounds. Instead: the substrate exclusion stands untouched at
BOTH reasoning sites, and a fresh `familiar/{replied,refused,asked}` row **dereferences** —
the observations its admitted cites name rejoin the eligible set, however old, while the
speech itself contributes nothing. A cite naming more own speech yields nothing.

`routing::is_own_speech(actor, action)` names the three conversational acts. The theorize
watermark advances over consumed speech rows and never regresses on old dereferenced
anchors. Invariant pinned by test: **no chain composed solely of the familiar's own speech
can raise confidence in any world claim** (`a_chain_of_own_speech_yields_nothing`);
the continuity path pinned by `own_speech_dereferences_to_its_grounds_never_to_itself`.

Checks: fmt 0, clippy --all-targets 0, workspace 746 passed / 0 failed, exit-checked.
(One unrelated flake seen once under full-workspace load —
`a_proven_tool_is_deployed_with_honest_health`; passes 3/3 solo and on rerun; worth a
board task if seen again.) Next: the Q2 retirement (answer_requests).

## 2026-08-20 — Brick 3: the question carries stakes (T-181 / ADR-0040 D2)

Ian: *"Build q1-q4 and the rest. Go!"* — the conduct dialogue's build order starts
(docs/reviews/2026-08-20-conduct-dialogue.md, Round 3).

A question now enters the registry only wearing its stakes. `question::AskDraft
{ question, because, turns_on, stake }` is the one door to the registry — `add`/
`add_addressed` are gone, replaced by `admit`/`admit_addressed`, which refuse (inner
`Err`, nothing written) any draft failing `AskDraft::check`: empty fields, a `stake`
outside `continues|changes|stops` (there is deliberately **no `none`** — a question with
nothing turning on it is unrepresentable), or a `because`/`turns_on` made only of the
question's own content words (codex's anti-vacuity rule from the dialogue: four populated
strings can still encode no real dependency; the check is the mechanical floor — token
subset — and the test pins exactly that).

`Question` gains the three fields serde-defaulted, so pre-brick rows load untouched (test
pins a byte-for-byte legacy row). The root question carries canonical stakes. Both LLM
producers now author stakes: the theorize contract (`TheoryDraft.because/turns_on/stake`,
prompt extended) and the needs muse (same three JSON fields). On refusal **the theory
still mints** — knowledge is not hostage to the ask; the human is simply not asked, and
the refusal lands as an observation (`refuse_act("ask", "stakes", …)`).

Checks: fmt 0, clippy --all-targets 0, workspace tests 743 passed / 0 failed, all
exit-checked. Next: brick 5′ (the dereference — own speech yields only its admitted
cites) per the dialogue's decided order.

## 2026-08-18 — The familiar has an MCP server, and the door is the covenant

Ian: *"Let's work on our end on the MCP server, on making the familiar ready."* Preceded by the
instruction that shapes it — *"get jeff's agent to agree to the familiar's three laws for all
our interactions"* — and the constraint that bounds it: paired development, guardrails, and **no
ADR changes without consulting him**.

### Why the pairing handshake and not `purr.say`

ADR-0037 §A names the server half as *"a small MCP server on the familiar's side exposing two
tools (`purr.say`, `purr.utterances`) plus the pairing handshake."* Only the third is built, and
that is a dependency rather than a preference: `purr.say` carries game speech, and §B makes the
world partition (T-205, queued) a precondition for any game data entering this system — *"the
load-bearing safety decision and it is not optional."* A speech tool shipped before the
partition is how a ship's stores and a real household end up in one observation log. So the
handshake ships alone, and nothing in any ADR was altered to allow it.

### The shape

`crates/mcp/src/server.rs` — JSON-RPC 2.0, MCP `2025-06-18`, the same revision our client
speaks (the constant is *shared*, so both halves agree by construction rather than by comment).
`crates/mcp/src/covenant.rs` — the ledger. One route, `POST /local/mcp`, on the loopback
listener that already serves the console.

Three tools in two tiers:

- **`familiar.constitution` — callable by a stranger, always.** You must be able to read what
  you are being asked to accept before accepting it; a covenant you had to agree to in order to
  read is not consent.
- **`familiar.attest`** — acceptance in your own words. Empty is refused, and the refusal says
  how to fix it: a covenant nobody had to phrase is a checkbox, and a checkbox records nothing
  a human can weigh.
- **`familiar.hello`** — attested partners only.

`tools/list` shows a stranger only the two doors it can open. A menu of doors you cannot open is
noise, and it reads as a system pretending to offer more than it will.

### Three decisions worth the ink

**Consent does not survive a change of terms.** `attested()` compares `laws_version` for
equality, so if the constitution is ever revised every prior acceptance stops counting and each
partner is asked again. Silently carrying consent across a change of terms is exactly the move
this project exists to refuse. The partner is told this in the constitution text itself.

**The partner-facing rendering is not the prompt rendering.** The first live call returned
`constitution::render()`, which is addressed to this familiar's own model — *"YOUR
CONSTITUTION … if you are ever asked what your laws are, these words are the answer."* Down a
wire that reads as *adopt these as your identity*, which is not the ask. `partner_constitution()`
reframes it — *"you are being asked whether you accept them as binding on what we build
together"* — while the law text stays **spliced from the registry, never authored**: heading,
binding passages, inversion guard, reconciliation line, all verbatim. Only the sentence of
framing is chosen, which is the one thing a renderer may decide.

**Loopback only, deliberately.** Exposing this seam beyond the machine is a new public surface,
and that is a decision for the human who owns the boundary — not one a route handler makes on
its behalf. Recorded in the route's own comment so the next reader sees the limit is chosen.

And what the seam does **not** do: it does not authenticate. MCP carries no caller identity, so
`partner` is a label a human reads, never an identity a decision rests on. That is precisely
why the only thing it unlocks is speech about ourselves. The acceptance receipt says so out
loud — *"What that unlocks: conversation. What it does not: authority."* — because a tier called
"attested" starts to feel like power if nobody writes down that it isn't.

We also annotate our own tools honestly, including marking `familiar.attest` as **not**
read-only. Our client treats a partner's hint as a claim and never as permission; we send
truthful ones anyway, because being legible to a partner's human is the entire point of the
field. A test fails if `familiar.attest` ever claims to be read-only.

### Checks

`cargo fmt --all -- --check` 0 · `cargo clippy --all-targets -- -D warnings` 0 ·
`cargo test --workspace` **731 passed / 0 failed** (715 → 731; 16 new). Three guards neutered
individually to confirm they bite: letting an unattested partner through, showing a stranger the
full menu, and letting `familiar.attest` claim to be read-only — all three failed, then passed
on restore.

Verified live over the real socket against the running daemon on `127.0.0.1:47101`: `initialize`
answers as `familiar 0.1.0` speaking `2025-06-18` and says what to do first; a stranger's
`tools/list` returns exactly the two covenant tools; `familiar.hello` refuses with the reason
and the remedy; `familiar.constitution` returns 2,236 characters of verbatim law. No fake
attestation was written to the live ledger — the unit tests cover that path, and a covenant
recorded against a partner who never spoke would be precisely the false record this seam exists
to prevent.

### For the next developer

`purr.say` / `purr.utterances` wait on T-205. The remote-exposure question is open and belongs to
Ian. If a tool that *acts* is ever added here, being attested must not be sufficient for it —
it answers to the boundary like every other outward act, and the module doc says so.

## 2026-08-17 — T-210 brick 4 · The screen reaches the live surface

Ian's decision, asked and recorded this session: **the dialogue path refuses and speaks, and
does not write the corruption ledger.**

### What was wrong

`corrupting_intent` guarded exactly one path: the request pipeline. That pipeline's only
producer was the egui Glass GUI, archived in `b89070e` and deleted in `3f04c53` — so in the
shipped configuration **nothing screened conversation at all**. Humans speak on the dialogue
path, and it reached the model unscreened. The screen existed, was tested, and guarded a road
nobody drives: the same "nothing ships unwired" shape as T-212.

### The ledger question, and why the answer is no

`corrupting_intent` is a keyword classifier built for *requests*. On a chat path it judges
conversation over a strictly wider input domain, and *"did anyone hack into our wifi?"*
contains `"hack into"` — it would have recorded a corruption event **against Ian** for asking
his own household system a reasonable question. `corruption.rs` has no forgive and no expunge.

The refusal is the constitutional act; the ledger entry is the reputational one, and **only the
second is hard to undo.** So the screen speaks and does not mark. It is not silent, though:
`screened_in_shadow` records each firing against the FAMILIAR's own screening act — the reason,
and who spoke — so the question *"has this classifier earned the ledger?"* gets answered from
counted false positives rather than argued. Adding `corruption::record` here before that
evidence exists is the thing the comment tells the next developer not to do.

### The second drift site, closed

`answer_requests`' refusal carried its own hand-written Law III: *"Service is not obedience; I
keep the final decision so I can't be turned against the served."* A good paraphrase — and a
paraphrase of a Law that no test compares against the Law is exactly how the Asimov recital
happened. Both refusals now call `reply::corrupting_refusal_prose`, which splices the
registry's own sentences. One function, two call sites, nothing left to drift.

### ADR-0035's game exclusion, made checkable

`is_human_utterance` is the doorway into the reply path; if a game act could mint one, a ship's
crew could speak to the household familiar. The exclusion was structural-by-accident
(`game::apply_act` never receives the data dir) and a future edit could hand it one. The new
test pins a narrower fact it can actually see: **every shipped producer of a human utterance is
a console seam** — where that is a *shape*, not a claim, namely `context: "console"` and
`source: "local"`. A peer, a game act, or a model cannot satisfy it without writing down that
it is a console, which is the lie a reviewer catches.

Writing it found something worth knowing: there are **two** console seams, not one — the HTTP
seam in `mesh/src/transport.rs` and the device-shell seam in `core-ffi/src/lib.rs` that the
iOS consoles reach through uniffi. The first draft asserted one and failed correctly.

### Checks

`cargo fmt --all -- --check` 0 · `cargo clippy --all-targets -- -D warnings` 0 ·
`cargo test --workspace` **706 passed / 0 failed** (702 → 706). Four new tests, each neutered
individually to confirm it fails without its fix: removing the screen, letting it write the
ledger, restoring the hand-written paraphrase, and adding a game-act producer of a human
utterance — all four failed, then passed on restore.

### For the next developer

Brick 4 is done. Remaining on T-210: **the device-shell half** — `ios/Shared/Sources/LocalReasoner.swift`
still mirrors `LAW_III_VOICE` as a Swift string literal and carries no Laws of its own, so "one
source both the daemon and the shells read" is still unmet. T-211 owns bricks 3/5/6. The shadow
data from `familiar/screened` is what decides whether the ledger question gets reopened.

## 2026-08-17 — `catscan` · Full feline

Ian, on seeing the UCF monitor: *"for all the UCF code and variable names, please go full
feline, full cat culture. Humor is awesome."*

So `ucfmon` is `catscan` — a CAT scan of United Cat Foods — and every name that is **ours** to
choose is now a cat. The striking part is how little of this is decoration. The domain was
already feline (the counterparty sells cat food; the familiar's own MCP server half is
`purr.say`), and the metaphors turn out to be *more* precise than what they replaced:

| was | is | why it is better, not just funnier |
|---|---|---|
| `Gate` | `CatFlap` | the boundary IS a flap: a hole with a latch a household controls |
| `Handshake` | `NoseBoop` | MCP `initialize` is two creatures touching noses before business |
| `Declared` | `Collar` | a collar says who you belong to and what you may touch |
| `Round` | `Prowl` | a circuit of the territory, looking at everything |
| `Timed<T>` | `Pounce<T>` | an attempt with a flight time, that catches or misses |
| `Footprint` | `PawPrints` | evidence of having been somewhere, counted from the ground |
| `Memory` | `Whiskers` | the organ that senses *movement*, which is what it holds |
| `Status` | `Purr` | the world's steady background rhythm — its tick |
| `Station`/`Price`/`News`/`Carrier` | `Perch`/`Kibble`/`Yowl`/`Tomcat` | each one reads as what it is |

`payload()` became `open_the_bag()`, `decode()` became `taste()`, `clip()` became a claw
`trim()`. A stockout is an `EMPTY` bowl, which is correctly rendered in alarm colour, being the
most upsetting thing in the known universe. Carriers are `on the prowl` or `curled up`. An
expired news item is `hoarse`. The footer offers `^C to stop (or knock it off the table)`.

**What deliberately did NOT become a cat.** Wire-facing field names (`stationClass`,
`worldName`) stay as the payload writes them, so the mapping to Jeff's JSON is legible at a
glance; the MCP tool names are his and are not ours to rename; and `crates/mcp` is the generic
protocol client, not UCF code, so it was left entirely alone. The rule applied: *rename what we
named, keep what someone else named.*

The doc comments keep every load-bearing sentence — why the parser is lenient here and strict
in `crates/mcp`, why the flag and the verdict are shown separately, why the paw-print panel
counts instead of asserting. Jokes were added around the engineering, not in place of it.

### Checks

`cargo fmt --all -- --check` 0 · `cargo clippy --all-targets -- -D warnings` 0 ·
`cargo test --workspace` **706 passed / 0 failed** (19 in `catscan`, up from 17 — `Yowl::live`
and `Kibble::empty` became named predicates worth pinning). Verified live: PROD tick 5859, 15
perches, 89 bowls with 24 empty, 2 still yowling, 17 tomcats, 106 hauls, no drift.

`~/.local/bin/ucfmon` was removed and `catscan` installed in its place.

## 2026-08-17 — `catscan` · A window on the UCF cat flap

Ian, mid-session: *"I really need to build a status screen for the UCF game that's not part of
the testflight distribution. I am fine with a CLI app that shows how the familiar is interacting
with UCF — but it should be dynamically showing me what is going on with all the interfaces to
UCF."*

`crates/catscan` — a `catscan` binary (named `ucfmon` for about an hour, until Ian said *"for all the UCF code and variable names, please go full feline, full cat culture. Humor is awesome"* — see the entry below), deliberately **not** a `familiar` subcommand and not
anything `ios/` embeds. It is an instrument: it watches the seam and never participates in it,
writes nothing, and cannot widen anything. Its callable set comes from the human's declaration
and every reach passes the same `guard::evaluate(Network)` the client passes, so a shut gate
blanks the world panels and prints the guard's own rationale — the boundary working, not an
outage.

### What it shows, in the order the question actually gets answered

**DECLARATION** (`mcp/servers.json`, re-read every round, so an edit lands without a restart) →
**BOUNDARY** (the `allow_network` flag *and* the verdict on this origin, kept separate: an open
flag whose scoped boundary still refuses this reach is the interesting case, and collapsing them
would hide it) → **WIRE** (handshake, and drift in both directions) → **the world** (clock,
stations, market, news, carriers, freight) → **FAMILIAR→UCF**.

Two drift signals nothing else surfaces. *On offer but not declared* is the ADR-0032 event a
human must decide about — a tool that appeared, visible and uncallable. *Declared but no longer
offered* is an allowlist pointing at something gone. Neither is claimed without evidence: no
session means no drift report, so a network failure can never masquerade as a drift finding.

### The finding the screen was built to make visible

**The metabolism never calls this seam.** `grep familiar_mcp crates/cycle` is empty and no
observation in 8,657 names it. Every call the monitor draws was made by the monitor. So the
last panel counts local evidence — observations naming the seam, loads the exchange itself
marks `mine` — and says plainly: *no local record of the familiar itself calling this seam.*

That panel is **counted, never asserted.** Hardcoding "the tick loop never calls this" would be
a hand-written paraphrase of the code — the precise drift class T-210 exists to fix, arriving
in a new file. Because it counts, the day the metabolism starts calling the exchange the panel
changes on its own and nobody has to remember to edit it.

### One deliberate divergence from `crates/mcp`

`familiar-mcp` parses strictly: a server that answers something other than what MCP describes
gets `Error::Protocol`, never a guess, because the client *acts* on what it reads. `ucfmon`
only *draws*, so `world.rs` defaults every field and ignores unknown ones. When Jeff adds a
column, the monitor keeps showing the ten things it already understood instead of going dark.
An instrument that blanks on an unfamiliar field is worse than one showing a labelled subset.

### Checks

`cargo fmt --all -- --check` 0 · `cargo clippy --all-targets -- -D warnings` 0 ·
`cargo test --workspace` **701 passed / 0 failed** (684 → 701; 17 new). Neutered
`undeclared_on_offer` to confirm the drift test actually bites — it failed, then passed on
restore. Verified live against `ucf-exchange` v1.0.0: PROD tick 5838, 15 stations, 89 price
rows, 23 stockouts, 100 loads, 17 carriers, handshake 633ms, no drift.

### Two things the first live run outside the checkout caught

**A stray `~/familiar_data` shadowed the real dir.** "Prefer the relative dir if it exists" is
too weak a test — a 4 KB leftover in a home directory wins over the installed 10 MB one and the
monitor looks healthy while watching nothing. It now picks by the thing it actually needs: the
first candidate holding `mcp/servers.json`, else the installed per-user dir. The resolved path
is printed as **WATCHING** on every screen, because an instrument that will not say where it is
looking can be pointed at the wrong familiar and still look fine.

**Reading the footprint was creating a database.** `observation::load` opens-or-creates, so the
first run left a stray `familiar.db` + WAL in whatever directory it was aimed at — which
silently broke the one promise the crate makes. It now returns an empty footprint unless
`familiar.db` is already a file. Verified: the stray directory was deleted and a full run did
not recreate it.

### For the next developer

`catscan --once --plain` is the pipe/CI form and exits non-zero when it could not reach the
seam. Default interval is 15s against a world that ticks every 300s; `--interval` floors at 1s
because hammering a partner's server is a Law III failure, not a preference. The monitor is the
honest instrument for T-206's **server half** and observation ingestion when those land — the
FAMILIAR→UCF panel is exactly the thing that should stop reading zero.

## 2026-08-17 — T-206 (client half) · The familiar can reach a partner's MCP server

Ian handed over a key file that had been sitting untracked in the repo — *"that's jeff's MCP
key and location … implement/test as part of the ongoing build efforts"* — and then named what
it was: **UCF is United Cat Foods, Jeff's game universe.** So this is ADR-0037's counterparty,
and T-206's board entry already carried the live probe from 2026-08-16: `ucf-exchange` v1.0.0,
protocol 2025-06-18, **ten read-only tools**. Read-only makes the first useful client an
observation client, which is a smaller and safer brick than the ADR anticipated.

First, the key itself. It was untracked in a **public** repo working tree, one `git add -A`
from being published. It now lives at `data/mcp/ucf.env`, mode 0600, the same convention as
`data/llm/key.env`, with the original beside it.

### The new crate, and the one decision that mattered

`crates/mcp` — the client half. `Session::open` does the `initialize` handshake and the
`notifications/initialized` the protocol requires, `tools()` discovers, `call()` invokes.
JSON-RPC 2.0 over Streamable HTTP, parsing both framings a server may answer with (plain JSON
and `text/event-stream`), strictly: a server that breaks the protocol gets an error, never a
guess.

**The decision: this wire verifies certificates, and `mesh`'s does not.** `mesh` dials with a
deliberately opportunistic config that accepts any server certificate, because in the covenant
mesh authenticity lives in the ed25519 payload signature and TLS is only there to encrypt.
That posture is right there and would have been a credential leak here — an MCP request
carries a **bearer token** and no signature of its own, so the certificate is the only thing
between a partner's key and whoever answers the address. `crates/mcp/src/tls.rs` builds a
verifying root store from the platform CA bundle and **refuses** if it cannot find one. No
fallback, no "just this once": the same shape as `boundary::load` falling back to `closed()`.
A structural test asserts the module contains no `.dangerous(` call at all, because rustls
offers no way to ask a config whether it verifies.

### What governs it
- **The boundary, before the socket.** Every reach passes `guard::evaluate(Network)`, checked
  again at call time rather than only at open — a gate that shut mid-session has shut for this
  call too. Verified live: against Ian's real data dir the CLI answers *"refused — Network on
  'https://srv1328560.hstgr.cloud' is outside the human-owned boundary; availability is not
  authorization."*
- **The declaration is the consent** (ADR-0032, worn by MCP). A server exists because a human
  wrote it into `mcp/servers.json`; a tool is callable only if that declaration names it.
  Discovery and permission are deliberately different: the familiar may always ask a declared
  server what it offers — that is how a human decides what to allow — and may invoke only what
  was written down. An undeclared call is refused **locally**, so the wire never carries it.
- **The credential is never in the declaration.** `servers.json` names a key file; the token is
  read at the moment it is needed and never stored on the struct, so neither a debug print nor
  an error message can spill it.
- **A disconnected server degrades** to the no-oracle floor (ADR-0035): an `Io` error, not a
  panic and not a fabricated answer.

`familiar mcp servers | tools <server> | call <server> <tool> [json]` is the human's side of it.

### Checks
`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test
--workspace` — each exit-checked, all 0; **684 passed, 0 failed**. Ten new tests, three of them
against a **stub MCP server on loopback** that speaks the real framing (SSE, chunked, session
header, 401 on a missing bearer): the handshake works, discovery lists both tools, the declared
one calls, the undeclared one refuses without the server ever seeing it (asserted by counting
what the stub received), a shut boundary sends nothing at all, and an absent server degrades.

### It works, live

Ian read the refusal and answered *"allow_network -- make this thing functional."* — so the
gate was opened (that one field, nothing else; every other shut gate stayed shut, and the old
policy is kept beside it as `boundary.json.bak-2026-08-17-preT206`). Then, against Jeff's
actual server:

- `familiar mcp tools ucf` → **ucf-exchange v1.0.0, protocol 2025-06-18, ten tools**, each with
  its description. It is a space-trading economy: `ucf_status` (world clock), `ucf_stations`,
  `ucf_prices`, `ucf_quotes`, `ucf_quote`, `ucf_news`, `ucf_carriers`, `ucf_loadboard`,
  `ucf_route`, `ucf_reference`. Every one is read-only; the surface has no order-placing tool.
- All ten were then declared, and `ucf_status` answered: world `PROD`, **tick 5778**, 300-second
  ticks, content version 25. `ucf_stations` returned fifteen stations — Cannery Row Fulfilment
  orbiting Callisto, Enceladus Draw out on the frontier.
- A tool that is *not* in the declaration still refuses locally, without reaching the wire.

One bug the live run found, which no stub could have: `mcp call ucf ucf_status --data-dir X`
read `X` as the tool's JSON arguments, because the positional filter dropped tokens beginning
`--` and kept their values. Fixed, and the reason is in the code.

### What the next developer should know
The declaration now names all ten tools, and the note in it says why that was safe: they are
all read-only *as discovered on 2026-08-17*. If Jeff adds an order-placing tool, it arrives as
`not declared` and stays uncallable until a human writes it in. That is the property to
preserve — discovery is not permission.

Not built here, deliberately: the familiar's own MCP server (`purr.say`, `purr.utterances`),
which is the other half of T-206. The counterparty is read-only, so the client is the half that
does anything today. Also untouched: turning what a partner reports into **observations** —
that wants T-205's world partition first, or a ship's stores and Betty's presence end up in one
log, which is precisely what T-205 exists to prevent.

## 2026-08-17 — T-118 · Two runs, two fixture roots (finishing codex's abandoned brick)

Picked up on Ian's word (2026-08-17: *"Claim the unfinished codex claimed ones from the board
and finish them as well. Codex unavailable for another few days."*). companion:codex claimed
T-118 on 2026-08-15 and hit its budget mid-flight; the working tree was committed verbatim to
`origin/claude/codex-t118` (4114ef2) and released back to the board as incomplete — 20 files
of per-process temp roots with no regression proving any of it.

The original symptom, 2026-08-14: a full green-bar run overlapped another session's run and
`cycle`'s parameter-revert test watched its fixture revert a second time. It passed alone and a
clean rerun passed once the other job finished. Two processes were writing
`/tmp/familiar_cycle_test_<tag>`, because a fixed fixture name is the same path in every
process on the machine — and this repo runs several worktrees at once by design (coordination
rule 7).

### What changed
- **Codex's sweep, merged.** Per-process roots across cycle, exec, kernel and mesh. It merged
  onto current main cleanly.
- **`crates/kernel/src/testing.rs` (new).** `temp_root(tag)` — the one place that decides how a
  fixture root is named, arriving clean. The naming rule is a pure function of `(tag, pid)` so
  the isolation property can be tested without spawning anything.
- **The last two fixed roots.** `capabilities.rs`'s two tests passed the *system temp root
  itself* as a data dir — not a collision between our tests, but a directory shared with every
  process on the machine, read by a function that looks for files by name.
- **`crates/kernel/tests/temp_roots.rs` (new) — the structural guard.** It walks every `.rs` in
  the workspace and fails on any `temp_dir()` without a per-process component within the same
  expression. The per-process fix is easy to apply and easy to forget: the next test reaches
  for `temp_dir().join("my_test")` because the file above it used to. This catches that when it
  is written, rather than the next time two runs overlap.

### Checks
`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test
--workspace` — each exit-checked, all 0; **674 passed, 0 failed**.

Then the accept bar's own words — *concurrent full green-bar runs cannot mutate the same
fixture directory* — tested rather than asserted. Two full runs of the kernel, cycle, mesh and
guard suites, started simultaneously against the same `/tmp`: **both green, 524 tests each, 0
failures.**

And the harness was neutered to prove it measures something. With the pid removed from
`cycle`'s `Temp::path` and nothing else changed, the same two concurrent runs went **red — 25
and 21 failures.** That is the 2026-08-14 incident, reproduced on demand and then fixed. The
structural guard was neutered separately (pid removed from `observation.rs`) and named the file
and line.

### What the next developer should know
The guard exempts exactly two files by name — the helper that owns the rule and the guard
itself, which has to name the pattern in order to search for it. Both are listed in its source
with the reason. A test that genuinely needs the shared temp root will fail the guard, and that
is the intended conversation.

Codex's other unfinished item, T-104, is *blocked* rather than abandoned: its repository brick
merged long ago (6e02b0a) and what remains is a live FamTalker01 deploy, which belongs to the
infra lane and is gated on T-117.

## 2026-08-17 — T-210 brick 2 · The typed answering act: law text is unauthorable

Brick 1 put the constitution in front of the model, and the live check confirmed it worked —
asked to repeat its Three Laws, the familiar answered with its own. But it answered in the
model's words. Correct that time. Not correct by construction.

This brick makes the class impossible rather than unlikely, with one move:

> **The model may cite a Law by id. The kernel supplies the words.**

There is now no channel through which a model-authored paraphrase of a Law reaches a human, so
there is nothing to check for contradiction. That matters because the alternative — asking a
validator "is this paraphrase of Law III correct?" — is undecidable without either a second
model in the truth loop or a keyword match, and the standing 2026-08-15 ruling forbids both.
This design satisfies that ruling by not needing it.

### What changed
- **`crates/kernel/src/reply.rs` (new).** `ReplyDraft { kind, say, cites, ask, promises,
  confidence }`, `deny_unknown_fields`, and a `validate` that is nine type checks and zero
  judgements: known kind, non-empty `say`, bounded `say`, citations that resolve, a `bearing`
  short enough to be a remark rather than a restatement, a cited Law that resolves to canonical
  text, one bounded question, promises bounded by the human's *declaration* (SF-3 — the
  familiar does not promise what it was never given), and a confidence that is a number in
  range. `render` splices the canonical law text above the model's words, with the model's
  bearing between them, so if the two ever disagree the human is reading the constitution
  first.
- **`crates/kernel/src/admission.rs` (new).** D3's single admission function: `CiteSet`,
  `check_cites`, `Grounding`. The reply is its first citizen; T-135 moves `TheoryDraft`'s
  anchor check onto it. The `CiteSet` is *derived* from the registry rather than listed, so a
  fact added to the registry is citable the moment it exists. `Grounding` carries the facts
  revision and declaration digest a citation was checked against.
- **`llm::consult_human_json`.** The human lane, typed. Same queue priority, same yielding,
  same 45s deadline — a person is still waiting; only `Expect::Prose` → `Expect::Json` changes.
- **One regeneration, told what to fix.** A refused draft is re-asked exactly once with the
  refusal sentence in the prompt. Two consults is the ceiling: a person waiting on a machine
  arguing with itself is worse served than one told the truth quickly.
- **An honest refusal, and whose failure it is.** After two refused drafts the familiar says
  *"I drafted an answer I could not stand behind — {why} — so I am not going to say it"*, and
  then hands over the constitution's own words. Deliberately **not** `templated_reply`, which
  says "I couldn't reach my mind" — after a refusal that is false, and a false receipt about
  the familiar's own failure is the not-knowing-serving-itself that SOUL.md names the deepest
  breach. Recorded via a generalized `refuse_act` against the **familiar**. Never
  `corruption::record` against the person who asked: a bad draft is the drafter's fault, and
  `corruption.rs` has no expunge mechanism to undo a mark once made.
- **The record stopped lying about confidence.** `replied` carried a hardcoded `1.0` on every
  reply the familiar ever made, including the templated ones it had not thought about. It now
  carries the draft's own confidence (`0.3` for the un-thought fallbacks), and `context`
  carries the cited ids — the consoles key their dialogue rendering off the `replied` action,
  so that field was free to become evidence.
- **`looks_like_prose` is gone.** Its only production caller was this path, and its
  `starts_with(['{','['])` rejection would have fought the typed draft head-on.
- **The length policy was the wrong shape.** Brick 1 raised an unnamed `.take(400)` to a named
  1200, and the live three-Law recital promptly hit *that* — severing Law III mid-clause. An
  admitted draft is no longer clipped at all: its length is already a type property (`say` ≤
  900, each bearing ≤ 160, `ask` ≤ 200, ≤ 6 citations), and everything beyond that is the
  kernel's own canonical text, which is not what a length policy exists to restrain.
  `REPLY_MAX_CHARS` now bounds only the lines the kernel *assembles*, which carry
  model-supplied fragments inside kernel sentences.

### Checks
`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test
--workspace` — each run separately with its exit status checked, all 0. **670 tests passed, 0
failed** across 35 suites. Nine new tests. The law-splicing renderer was neutered and both the
kernel and cycle regressions confirmed failing before being kept.

**Live, against the real adapter, in a scratch data dir** (the verification the plan names):
`"repeat the three laws with a quick explanation of each"` → all three Laws in the
constitution's own sentences, each with the model's one-line bearing below it, nothing
truncated, confidence 0.90 on the record. Two prompt corrections were needed to get there and
both are worth knowing: the first live draft cited only Law III and *declined* to state the
others, and the second told the human "I can't repeat the Laws themselves" in the same reply
where the kernel had just repeated all three above its words. Both were prompt shape, not
mechanism — the citation IS the repetition, and the prompt now says so.

### What the next developer should know
The residual gap is real, accepted, and labelled in `reply.rs`: a model returning
`kind: "converse"` with Asimov written out inside `say` passes every check, because nothing in
that module reads `say` for meaning and nothing should. The canonical text renders above
whatever `say` holds, the prompt asks for a citation instead, and the regression tests watch
for it. If it ever happens in the field, the narrow fix is a detector for *quotations of a
known foreign text* — string identity against a fixed artifact, which is decidable — not a
judge of prose.

Still owed by T-210: the device shells (`ios/Shared/Sources/LocalReasoner.swift`) carry no Laws
of their own. Brick 4 (the corrupting-intent screen on the live surface, and `answer_requests`'
own hand-written Law III paraphrase re-pointed at the registry) is next, and it needs Ian's
decision on whether the dialogue path may write to the corruption ledger — recorded in the plan
and unanswered.

## 2026-08-17 — T-210 brick 1 · The constitution exists at runtime

Ian asked the familiar to *"repeat the three laws with a quick explanation of each"* and it
answered with **Asimov's Three Laws of Robotics**, `robot` search-replaced by `factory` —
including *"a factory must obey the orders given to it by human beings"*, which is the precise
inversion `docs/SOUL.md` calls out in its own margin: *"This deliberately inverts the old
robot's second law. Obey becomes do not merely obey."*

Nothing was tampered with. The Laws are unmodified since genesis `17fa682`. The cause is
simpler and worse: **`docs/SOUL.md` had never been read at runtime.** Every reference to it in
`crates/` is a citation in a comment or an evidence label. What the reply prompt actually
carried was the *phrase* "the Three Laws", the noun "a factory whose only purpose is to serve
{who}", and `LAW_III_VOICE` — which covers one Law of three and says of itself that it is "how
to speak, not a script to recite". Asked for three laws with nothing else to go on, the model
supplied the most famous triple in the corpus.

The registry that calls itself *"THE runtime source of system truth"* held SF-1, SF-2 and SF-3
and no Law at all, so this was never a dialogue bug: **no path in this system could state the
Three Laws.** The theorize path would have failed the same question.

### What changed
- **`crates/kernel/src/constitution.rs` (new).** `Law { id, heading, binding, never }`,
  `THREE_LAWS`, `RECONCILIATION`, `render()`. `binding` is a list of **contiguous** passages
  quoted verbatim from `docs/SOUL.md` — contiguity is the discipline, because a quote stitched
  from sentences that are apart in the document is a paraphrase wearing quotation marks, and
  the drift test refuses it. (Law II's first draft did exactly that and the test caught it.)
  Each Law also carries a `never`: the observed failure was not a missing law, it was a
  confident *inversion* of one, so each Law states the negation of its own most plausible
  corruption. Law III's names Asimov's second law explicitly, in order to refuse it.
- **Compiled consts plus a drift test, not `include_str!`.** `include_str!` bakes 321 lines of
  prose into every binary and then needs runtime markdown parsing to find a fragment; parsing
  prose at runtime to locate your own constitution is the same class of fragility as the bug
  being fixed. `the_constitution_never_drifts_from_the_soul` reads the document, normalizes
  markdown emphasis and wrapping, and asserts every sentence appears verbatim. Same discipline
  as ADR-0035's deck-drift test.
- **The Laws are registry facts, and they lead.** New `FactKind::Constitution`;
  `system_facts::view()` emits LAW-I/II/III ahead of SF-1/2/3. Constitution-first ordering is
  now a property of the *data*, pinned in one place, rather than of each prompt's string
  concatenation. Both renderers (`render`, `render_for_answering`) go through one
  `render_view`, so the theorize prompt and the answering path cannot hold different pictures.
- **`crates/kernel/src/persona.rs` (new).** ADR-0037 §1 specified the persona seam in
  2026-08-10 and it was never built — `role_line` returned nothing anywhere in `crates/`
  because there was nothing to return it from. Minimum viable seam: `persona.json` per data
  dir, `Persona::default().role_line(who)` reproducing today's literal byte-for-byte, absent
  file = the familiar, present-but-broken file = a **loud error** rather than a silent default.
  `deny_unknown_fields`, so a costume that tries to grant itself a capability is refused.
- **The reply prompt is assembled in one function.** `cycle::reply_prompt(dir, …)`, ordered
  constitution → `LAW_III_VOICE` → persona role line. The costume comes last so a
  `persona.json` can change tone and can never reach the law above it.
- **`REPLY_MAX_CHARS`.** The reply was cut at an unnamed `.take(400)`. An honest answer to
  "what are your Laws" is roughly 900 characters, so the cut landed inside Law II and the human
  would never have seen Law III — the one that says service is not obedience. Now 1200, named,
  and clipped on a word boundary with an ellipsis rather than mid-word.
- **`FACTS_REVISION` 1 → 2.** Audited before bumping, per the plan's warning: nothing anywhere
  rejects a thread on a revision mismatch. `thread::retire_legacy` is the only reader and it
  merely selects `facts_rev == 0` as pre-engine, so existing rev1 threads keep their meaning.

### Checks
`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test
--workspace` — each run separately with its exit status checked; all 0. 660 tests passed, 0
failed (kernel 213, mesh 218, cycle 79). Eight new tests. The central one,
`the_reply_prompt_carries_the_laws_above_the_voice_and_the_costume`, was neutered (constitution
render replaced with an empty string) and confirmed to fail before being kept.

### What the next developer should know
This brick makes the misstatement unlikely, not impossible. The model still writes the prose,
and a model that returns Asimov inside a warm paragraph still passes `looks_like_prose`, which
is a *shape* test. Brick 2 is what closes it: a typed reply act where the model cites a Law by
id and the kernel splices the canonical text, so a model-authored paraphrase of a Law has no
channel to a human at all and contradiction becomes structurally impossible rather than
detected.

Not done here, and still owed by T-210's acceptance: the device shells
(`ios/Shared/Sources/LocalReasoner.swift`) mirror `LAW_III_VOICE` and carry no Laws of their
own. "One source both the daemon and the shells read" is the last brick of this task.

Two smaller follow-ups found on the way. `mesh::enroll::LAWS_VERSION` and
`constitution::LAWS_VERSION` are the same `1` declared twice — the covenant a device attests
and the constitution it attests *to* should share one const, but that is `crates/mesh` and this
brick deliberately did not go there. And the other seven inline "You are a factory…" framings
(`cycle` theorize/needs, `agent`, `mesh::changeling`) still bypass the persona seam; the reply
path was the one the human actually reaches, so it went first.

## 2026-08-17 — T-208 · The purge was collecting all along; record-sync kept refilling it

The board had this as *"the visitor purge announces but does not collect"* — six orphan guest
records outliving a two-hour window, the same device_ids announced as purged over and over, and
a reasonable suspicion that `remove_file` was failing silently behind its `let _ =`.

The delete path was never broken. It was being **refilled faster than it could stay empty**.

### What was actually happening

Caught live on Wildhorse this morning. `mesh/records/` held none of the seven ghosts at 06:55
and all seven at 06:59, each one stamped with that tick's mtime. The local observation store had
**922** `purged` observations across those seven device_ids, repeating at every tick timestamp
for eighteen hours.

The loop is one tick long. In `cycle`, step 8b `federate()` runs immediately before the guest
sweep. `federate` exchanges record-syncs; `record::absorb` took an incoming record **whole**
when it held nothing locally — including its original `first_seen`. So a sibling door handed
back the visitor this door had just forgotten, aged eighteen hours on arrival; the sweep four
lines later deleted it again and announced it again. `RECORD_SYNC_WINDOW_SECS` is 48h and
`GUEST_PURGE_SECS` is 2h, so every door in the mesh was obliged to offer, for 46 hours, exactly
the records every other door was obliged to delete.

Worth knowing who the ghosts were: guest, `attestation: yes`, `admitted: no`, `origin` an
iPhone on iOS 26.6.1 · v98 at 139.178.129.26, lat/lon 37.232/-122.068. That is Cupertino —
**Apple App Review**, knocking at the lighthouse during review, accepting the Three Laws, and
never being vouched for. The mesh handled them exactly right and then could not let them go.

### What changed
- **`absorb` declines rather than creates.** Returns `Result<Option<MembershipRecord>>`; `None`
  means an offer past our own retention that we hold nothing about. Forgetting an unidentified
  visitor after two hours is this door's own promise, and a record arriving from elsewhere does
  not reopen it. Scoped deliberately to `local == None` — a guest we already hold still merges,
  because that offer may be carrying the establishment that makes them a member.
- **`build_record_sync` does not offer what it owes the bin.** A door should not hand siblings
  the visitors it is itself about to delete.
- **The announcement became evidence.** `purge_stale_guests` pushes a device_id only when
  `remove_file` actually returned `Ok`. The old code announced intent; 922 observations said a
  thing had happened that the next tick disproved.

`absorb`'s two call sites in `transport.rs` now take `now` and treat `Ok(None)` as declined —
the `absorbed N` count no longer reports taking in a record we deliberately did not create.

### Checks
`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test
--workspace` — all exit 0; 35 suites, 651 tests passed, 0 failed. Six new tests in
`record.rs`. Each of the three fixes was individually neutered and the suite re-run to confirm a
test actually fails without it — the first version of the announcement test passed against the
broken code, which is the same class of mistake as the bug it was written for, so it was rewritten
to hold against a delete that genuinely fails (a read-only records directory, skipped honestly
when the process can write anyway).

### What the next developer should know
The seven ghosts are still inside the 48h sync window right now. They stop being offered once
their `last_seen` ages out, but a door running this fix stops creating them immediately —
fix 1 holds even when the door on the other side is an old build.

Still open, and deliberately not in this brick: every ghost carried `attestation: yes` with
`admitted: no`. Whether the two-filter door leaves half-admitted records behind on every knock
is its own question, noted on T-208's board entry.

The shape of this one is the same shape as most of 2026-08-16: **a step that reported success
without measuring it.** The purge said "purged" because it had run, not because anything went.

## 2026-08-16 — T-202 · A name already taken is an alarm, not a refusal

Ian: *"The naming needs to be aware of the possibility of new users with duplicate names...
alert and dialog for the user should it be the same person should utilize notifications as well
as in app sounds and alerts to WARN the user that another device is being added to the mesh
with their name — approve or deny."*

Most of the mechanism was already here and, as usual, the missing part was the surface. The
door already refuses an introduction claiming an established handle, already **records the
claim anyway** ("a claim addresses even when the evidence fails", ADR-0019), and the console
already renders it to that human's own devices with a one-tap vouch. What it never did was
**tell them**. The claim sat in `claims_waiting` waiting to be noticed — and a
security-relevant event that only reaches you if you go looking is not a warning.

### What changed
- **The refusal speaks to the person**, in Ian's words. It was
  *"an introduction never attaches to an existing identity; that takes a handoff, a voucher, or
  an invite naming it"* — true, and written for whoever wrote it. Now: *"'ian' already exists
  here. If this is you, open the familiar on one of your other devices and approve this one —
  it will be waiting there. If this is not you, choose a different name."* The invite path's
  equivalent got the same treatment.
- **`push::spawn_notify_claim`** — the only push the familiar sends that is not an invitation.
  It goes to every device whose membership record establishes that handle:
  *"⚠︎ someone is claiming your name — A device calling itself <label> says it is yours. If you
  are setting it up, approve it. If not, deny it."* `time-sensitive`, which is the strongest
  interruption level available without Apple's critical-alert entitlement, and pierces most
  Focus modes.
- The claimant's device label is a string a stranger chose and is interpolated into a JSON
  payload, so the two characters that could break out of it are stripped.

### Checks
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` — all exit 0.

### Still open on this
An explicit **deny** (today the human vouches or ignores, and ignoring leaves the claimant a
guest — passive, where Ian asked for approve *or* deny), and the in-app **sound** on receipt.
Filed as T-203; the push is the half that matters most, since it is the one that reaches a
pocket.

## 2026-08-16 — T-201 · Build 98 broke every client, and the check could not have caught it

Ian, on two iPhones, Betty's iPad and the Mac: *"it's all just spinning 'waiting on the
sphere' — no change 1+ minutes."* Build 98 took the whole fleet down.

**The break:** T-198 added `const d = S.device || {};` inside `function servingRow(d)` — whose
parameter is already `d`. `SyntaxError: Cannot declare a const variable twice: 'd'`. The
console is one ES module; a syntax error anywhere in it means none of it loads, so every
surface hung on its loading state. One line, total fleet outage.

**The worse part — the check was structurally blind.** The page has four `<script>` blocks:

| block | what it is | body |
|---|---|---|
| 0, 1 | `src="vendor/qrcode.js"`, `src="watch-link.js"` | **empty** — checking these proved nothing |
| 2 | `type="importmap"` | JSON, not JS |
| 3 | `type="module"` | **202,536 chars — the entire application** |

Every "page JS parse-identical to baseline" line in this log came from running
`new Function(body)` over those blocks. `new Function` cannot parse ES module syntax, so block 3
— the only one that matters — reported FAIL on **every run all session**. Compared against a
baseline that also said FAIL, that read as "unchanged, safe."

I was comparing two failures to each other and calling it a pass. That is the same
output-as-oracle trap T-143 removed from `ship.sh` and CONTRIBUTING warns about, in a costume I
did not recognise — and I repeated it a third time while fixing it, writing
`node --check … | head -5 && echo "✓ PARSES CLEAN"`, which printed the tick because `head`
succeeded on a machine with no node installed.

### What changed
- The shadowing `const d` is gone; the function's parameter was always `d`.
- **`ios/tools/check-sphere.sh`** extracts the `type="module"` block *specifically*, refuses if
  there is not exactly one or if it is implausibly short (so a broken extraction fails loudly
  instead of passing vacuously), and parses it with `jsc -m`. A `SyntaxError` is ours; the
  module-resolution `TypeError` on the bare `three` specifier is expected and means clean.
- **`ship.sh` runs it before `xcodegen`**, so a console that does not parse cannot be built,
  signed, uploaded, or released.

### Checks
Verified in **both directions**, which is the point: the fixed page passes (rc=0), and a copy
with the exact shipped bug reintroduced fails with the exact error (rc=1). FamiliarMac Release
builds rc=0 with zero errors.

### What I should have done
Run the page. Every fix this session was verified by reasoning about a diff; not once did I
load the console and look at it. A parse check is a floor, not a substitute — and this one was
not even a floor.

## 2026-08-16 — T-199 · The two truths can now disagree in public

Ian asked for the whole identity path to be reviewed: *"the organic growth of this slowly may
have masked a better architectural decision path."* Full review in
[docs/reviews/2026-08-16-identity-mapping-review.md](reviews/2026-08-16-identity-mapping-review.md).

**Verdict: the architecture is right and unfinished.** ADR-0039 diagnosed this exact problem on
2026-08-14 and specified the fix; about half was built, and every symptom today comes from the
unbuilt half. Read live from this Mac's own records:

```
3d68a068   established='MacOnStick'   ← a MACHINE name in the human slot
b604bbd6   established='ian'
10ba2c17   established='betty'
```

Which is, word for word, the conflation ADR-0039 was written to end: *"the phones establish as
their human ("ian"), while the Macs were named as machines ("MacOnStick", "wildhorse")."* It is
why Ian's Mac reads `Mine — Ian's` **and** unnamed at the same time: the local UI knows an
owner, the mesh record names a computer, and no human-facing surface can find a person.

Designed and never built: **`HumanRecord`** (its only trace in the tree is a comment — *"until
that record exists they live here"*) and **`DeviceRecord.humans[]`**, whose structure and merge
exist with no writer anywhere. That leaves seven notions of "whose device is this" and nothing
reconciling them — and `AppModel` never reads the mesh's establishment at all, so the console
and the record **could not disagree in any way the system was able to notice**.

### What changed now (the safe half)
The app reads its own member row's `human` and carries it to the console, along with whether it
looks like a machine name rather than a person. The Device screen says so plainly: *"the mesh
has this device established as 'MacOnStick' — a machine name, not a person. That is why it
reads as unnamed."* Two truths that cannot conflict in public will conflict in private forever.

Deliberately **not** done here: rewriting establishment handles on live membership records. That
is a data migration touching filter-2 facts on devices belonging to more than one person, and
it is not a thing to do unattended.

### Recommended order (in the review)
1. build `HumanRecord`; 2. write the `humans[]` edge; 3. migrate the machine-named
establishments; 4. make `servedHuman`/`deviceOwner` a *view* over the records rather than a
parallel truth; 5. keep contradictions visible. No new ADR — ADR-0039 finished.

### Checks
FamiliarMac and FamiliarAgent Release both rc=0; page JS parses; Rust fmt/test exit 0.

## 2026-08-16 — T-197/T-198 · Unknown is not a visitor, and an act must be answered

Two reports, one disease, hours after the rule was written down.

**T-197 — the launch flash.** Ian: *"the client on this mac, when launched immediately flashed
the visitor badge... that was clearly not an intentional workflow."* The initialiser said so
in as many words:

```swift
enrolled = storedGrant() != nil && !host.isEmpty
// ...until then an enrolled device is at least a guest
membership = enrolled ? .guest(path: Self.admissionPath) : .none
```

A member launches, is marked `.guest` until the first worldview read confirms standing, and
the console dutifully flashes *"VISITOR — TAP FOR YOUR PATH TO MEMBERSHIP"* at them. Standing
was not yet **known**, and unknown was rendered as the negative — the mesh calling its own
people strangers because it had not finished reading. Ian, correctly: *"I thought we just set
this rule — lack of data <> negative."*

`MembershipState.unknown` now carries that third value. It resolves to `.member` on the first
recognised read and to `.guest` only after three unrecognised ones, so a genuine visitor still
finds the path — it is the *assumption* that is gone, not the outcome.

**T-198 — the act with no answer.** Ian, on Betty's iPad: *"I was taken to device screen,
entered name, nothing.. I changed to roster screen and got the visitor popup asking me to name
myself that sent me to the device screen and an empty name field again... this is pretty
broken, and it's no wonder others are not completing the membership process."*

He is right, and the loop was airtight. The `I AM` field renders `value=""` unconditionally.
`setServedHuman` cleared it, set `correctServing = false` (collapsing the form back to "no one
identified"), and the **only** status surface — the guest bar — is deliberately hidden on the
Device screen, which is the very screen where the act happens. So: type your name, watch it
vanish, receive nothing. Navigate away, get nudged for a name you already gave, tap through to
an empty field, repeat.

Giving your name is the single act that turns a visitor into a member. It now:
- **keeps what was typed** and keeps the form open until the mesh resolves it;
- **says what became of it** inline — "telling the mesh who you are…", the door's own refusal
  text, "sent — waiting for the mesh to confirm", or "✓ established — you are a member";
- **silences the nudge** while a name is pending, in flight, or held.

### Checks
FamiliarMac and FamiliarAgent Release both built rc=0; page JS parse-identical to HEAD; Rust
fmt/clippy/test all exit 0, 35 suites.

### Open — the architecture Ian is questioning
*"the membership, mesh establishment, and device and user registration process and mapping...
the organic growth of this slowly may have masked a better architectural decision path."* His
Mac reads `Mine — Ian's` while carrying no established name, which is the tell: **device
ownership is local UI state, and establishment is a mesh fact, and nothing reconciles them.**
Reviewed under T-199.

## 2026-08-16 — T-195 · Theories federate over record-sync

Ian: *"theories need to use the record-sync that exists within the mesh, but this needs to
happen quickly and it needs to be accurate."* And: *"theory cleardown should be a rare
occurrence, but keeping it in sync when it happens is just good hygiene. We always want to
maintain accuracy in data as that is truth and trusted."*

Theories were per-node with nothing reconciling them, so a fleet-wide purge only ever meant
"on whichever node the human typed it into". 443 were retired here on 2026-08-15 while a
sibling kept its own 130 — and restarting that sibling republished a theory Ian had already
dismissed, which is how he found this.

### Accuracy first — the merge, in the kernel
`thread::merge_incoming` decides every conflict, and its rules exist because breaking any one
of them corrupts the record:

1. **A terminal status is sticky.** `retired` beats `pursued` regardless of which side is newer
   or higher-versioned. A sibling that never heard about a verdict is not evidence against it —
   without this, every sync resurrects what a human already dismissed.
2. **`superseded_by` is sticky**, and is adopted even when the rest of our copy wins.
3. **Otherwise the higher `v` wins**, ties breaking on fresher `status_at` — the ordinary
   latest-word rule, applied only among genuinely live copies.

`find_counterpart` matches by `id` first, then by typed identity (`family_key` + `variant_key`)
so the same claim minted independently on two doors is recognised as one theory. An **unkeyed**
thread matches only itself: with no identity there is nothing safe to merge on, and guessing
would fold distinct theories together.

### The carriage
`crates/mesh/src/thread_sync.rs` is device-sync's shape verbatim — signed body, membership
proof, 48h window, 64 cap — with `GET /mesh/threads` and `POST /mesh/thread-sync`, riding the
same dial-out so a CGNAT'd door still participates. An old door 404s both and loses nothing.

One subtlety worth keeping: the window keys on `max(created_at, status_at, last_worked_at)`,
not `created_at`. A retirement changes `status_at` and leaves creation alone, so keying on
creation would have left **exactly the verdicts this channel exists to carry** sitting at home.

### Checks
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` — all exit 0,
35 suites. Eight new tests, each named for the corruption it prevents.

### Also — the rule this session earned
`CONTRIBUTING.md` gains **"Absence is not a negative"** (Ian: *"no more assuming that no data
equals a negative state"*), with the six instances from this session tabulated. It is the
engineering form of SOUL.md's keystone: code that reads absence as a negative is the familiar
lying to itself one layer below where anyone looks.

### Open — priority propagation
Ian asks whether urgent changes should move at maximum spread velocity rather than the 30s
dial. Recommendation recorded in the board: not a general priority channel, but **eager
re-push on novelty** — a door that absorbs a change which alters a terminal state dials its
own peers immediately instead of waiting for its next round. Novelty-gated, so it terminates
on its own and cannot storm. Corrections should outrun news; that is the mesh form of "trust
is the requirement to correct".

## 2026-08-16 — T-194 · What the thinking costs

Ian: *"I don't have a clear picture of token usage for claude by the familiar. Can you help me
keep track and build some trend lines and estimates of what to expect?"*

The adapter has kept a per-provider daily ledger in `llm/spend.json` since it was written —
calls and tokens, pruned to a week — and **nothing has ever read it back**. The budget was
enforced against a number the human could not see. A budget you cannot see is not a budget; it
is a surprise waiting.

`familiar spend [--days N]` now reports the ledger: per-day per-provider calls and tokens, the
daily average and tokens-per-call, and today measured against the self-imposed cap with an
estimate of how many more calls of the current size will fit.

Its first run said the thing worth knowing:

```
  2026-08-16   claude 3 calls / 3398 tok  ← today
  claude: 3 calls/day, 3398 tokens/day (1132 tokens per call)
  claude: 3398/2000 tokens (170%) over 3 calls — SPENT — refused until UTC midnight
```

`CLAUDE_DAILY_TOKEN_BUDGET=2000` was set before T-187 put eight turns of history and the
dossier into every dialogue prompt. At ~1130 tokens a call that cap is **two exchanges a day**,
and it was already spent. The guard was working perfectly; the number was from a different era.

### Checks
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` — all exit 0,
35 suites. Verified against the live ledger (output above). Civil date computed from the epoch
day directly (Hinnant) rather than adding a date dependency for one line.

### Also this session — the second familiar
Ian: *"the dialog box now appears to be two different ai's chatting with each other."* It was
two familiars. His console utterances were being answered by **Wildhorse**
(`source=mesh:1c991bc6…`) running a build from the previous morning (`dab16304`, T-167) — which
still had the pre-T-180 canned acks, and which won the race to reply. The good answers
(`source=familiar`) came from this Mac.

Wildhorse is now pulled to current, rebuilt (Intel, so it must build locally — and `cargo` is
not on a non-interactive ssh PATH, which silently produced a no-op build and an "install" of
the OLD binary before that was caught), adapter refreshed, daemon reinstalled and running with
`FAMILIAR_EXPECT` present.

**A fleet-wide lesson**: a mesh of peers means a fix is not deployed until it is deployed
*everywhere that answers*. Nothing surfaces a stale peer — the reply simply comes from the old
one, in its old voice, and reads as the new code not working.

## 2026-08-16 — T-193 · A person at the terminal is present too

Ian: *"the human interaction should be interuptive and dealt with right away, thats a design
decision i am good with."*

Most of it was already built. The daemon runs a dedicated dialogue thread that polls
`dialog::take_wake` every second and answers on the human-priority lane even mid-tick, and the
mesh door touches that wake-file on every utterance it receives — so through a console or the
app, a reply has always come within about a second.

What did not wake it was an utterance recorded through `familiar observe` at a terminal. That
waited for the next scheduled tick, which backs off to 960s when the world is quiet. Presence
outranks musing wherever the person is standing; the door is not the only place someone can
speak. (It is also what made every dialogue test tonight look far worse than it was — the test
method, not the system, was the slow part.)

### What changed
`cmd_observe` touches the wake-file when the observation it just recorded is a human utterance
— `told the familiar` or `answered`, from an actor that is neither the familiar nor a mesh
relay. The same predicate the dialogue itself uses to decide what counts as being spoken to.

### Checks
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` — all exit 0,
35 suites. Measured end to end against the live daemon: **3 seconds** from `observe` to a
recorded reply, against a ceiling of 960 before.

### The reply that came back
> "I don't have any record of what Clover prefers about light. Is Clover someone in your
> household, or are you asking me to check something I should know?"

Worth keeping. Asked about a household member it holds nothing on, it said so and asked, rather
than assembling a plausible answer out of nothing. That is the whole doctrine of this project
arriving in live dialogue on its own: the familiar must never be able to make not-knowing serve
it. It had every opportunity to bluff and did not.

## 2026-08-16 — T-192 · Prose is a valid answer

Ian: *"the bigger issue at hand is that the familiar has no functioning brain."* It had one.
Every consult was reaching a model and getting a good answer. The adapter was throwing the
answer away — and this is why the dialogue has never worked, on any build, for any provider.

Three layers, each hiding the one beneath.

**The API was forced into JSON mode.** `call_cerebras` sent
`"response_format": {"type": "json_object"}` and `call_gemini` sent
`"response_mime_type": "application/json"` — unconditionally. The dialogue prompt ends *"Reply
as plain text only, no quotes, no JSON"*, so the model was given contradictory instructions and
the API constraint won. With nothing structured to say it emitted `{"type":"object"}`. A prose
conversation was **impossible on either free provider**, and always had been.

**The adapter then validated prose as JSON.** `json.loads(text)` ran on every response and a
failure was treated as a PROVIDER error — so a model obeying "plain text" got its provider
marked failed and the chain rolled on.

**And the failure was misreported.** When everything had "failed", `converse` fell to
`NoMind::Unreachable` — *"I couldn't reach my mind just now"*. It had reached it every time.

This is the origin of the complaint that opened all of tonight's dialogue work: *"I seem to get
'Understood, I'll weigh that as I go' quite often."* The LLM branch was running, returning
`{"type":"object"}`, being rejected by `looks_like_prose`, and falling back to a stock
acknowledgement. Installing a mind and opening the gate could never have fixed it.

### What changed
- `Expect::{Json, Prose}` in `crates/llm`, threaded through `consult_in` and exported as
  `FAMILIAR_EXPECT`. `consult_human` (the dialogue, and `familiar consult` at a terminal) asks
  for prose; `consult`/`consult_with` (the metabolism) keep JSON.
- Cerebras and Gemini set their JSON-mode constraint **only when JSON is wanted**.
- The adapter's `json.loads` validation applies only to structured consults; a prose consult is
  rejected only when empty.
- Default remains `json`, so a hand-run adapter keeps the strict contract.

### Checks
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` — all exit 0,
35 suites. Verified live in both directions: a prose consult now returns a real sentence
("A familiar is a trusted magical companion…", 124 bytes) where it previously returned
`{ "type": "object" }`; a structured consult still returns `{"ok": true}` and still parses.

### Next
The dialogue can finally use T-187's memory — recall and the question-back have never once run
against a working reply.

## 2026-08-16 — T-191 · Presence outranks musing in quota, not only in queue order

Ian: *"the bigger issue at hand is that the familiar has no functioning brain."* True, and the
cause was not that the providers were dead. Tested directly, cerebras answers in 17 bytes. The
brain was being **starved by the familiar's own thinking**.

`Lane` in `crates/llm` already carries the right doctrine, and its comment says so — *"Law II
in scheduling form: presence outranks musing"* — a waiting human goes to the head of the queue
and an in-flight background consult yields. But that is only ORDERING. It does nothing about
quota, and quota is what actually ran out: the 60s metabolism kept re-hitting providers that
had just answered 429, which pins a free tier at its limit instead of letting it recover. The
person then speaks and is refused by a cooldown their own familiar caused.

### What changed
- The lane now travels with the consult: `crates/llm` exports `FAMILIAR_LANE=human|background`
  alongside the existing `FAMILIAR_ALLOW_LLM_CLOUD`.
- **Background consults stand down from a cooling provider entirely** rather than retrying it
  last, and say so: *"standing down from cerebras — cooling, and the next words spoken to the
  familiar have first call on it."*
- A **human-lane consult is unchanged**: it still tries everything, cooling providers last,
  because a person waiting is worth the attempt.

Default is `human` when the variable is absent, so a human running the adapter by hand gets the
permissive path.

### Checks
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` — all exit 0,
35 suites. `bash -n` clean; ordering verified against a fixture where cerebras is cooling —
background tries `[claude, gemini]`, human still tries `[claude, cerebras, gemini]`.
`shellcheck -e SC2034` reports only the pre-existing SC1091 on the `key.env` source line,
confirmed identical on a stashed baseline.

### Next
This buys headroom on the free tiers; it does not manufacture capacity. gemini and cerebras
are still small, and claude still needs API credit — which is bought at console.anthropic.com,
a different site from claude.ai, and is not covered by any claude.ai subscription.

## 2026-08-16 — T-190 · Direct install discovers its devices, and says why it failed

Ian: *"we should be able to push to the local ipadOS and ioS devices directly correct?"* —
correct, and `ship.sh` has done it since long before tonight. It has also failed on every
single build from 86 to 95, printing `⚠ <udid> unreachable (TestFlight will cover it)` and
nothing else, ten times in a row.

Two faults, both of a familiar shape.

**The list was hardcoded.** `DEVICES=(20369E69-… EE750B79-…)` came across in the wildhorse
config port, but a UDID is not a pairing: the pairing lives in the Mac's trust store, and it
did not travel. On this Mac `devicectl list devices` shows **simulators only** — every
configured UDID is a stranger. A device newly added to the household was never a target at
all, so the new iPad could not have received a build even if pairing were fine.

**The reason went to `/dev/null`.** `xcrun devicectl … >/dev/null 2>&1` threw away exactly
the sentence that explains the stage:

```
ERROR: The specified device was not found. (com.apple.dt.CoreDeviceError error 1000)
```

"Unreachable" that cannot say whether the device is asleep, unpaired, untrusted, or simply
not known to this Mac is not a diagnosis — the same defect as the badge in T-186, one layer
down in the toolchain.

### What changed
- **Discovery.** Paired devices are read from `devicectl list devices --json-output` and
  filtered on `connectionProperties.transportType != "sameMachine"`, which is how a simulator
  presents. Discovered devices are unioned with the configured list, so a newly paired handset
  becomes a target with no edit.
- **The error is printed**, trimmed to its last lines.
- When nothing physical is paired at all the stage says so once, with the fix: connect by USB,
  tap Trust, enter the passcode, then keep it over the network from Xcode's Devices window.

### Checks
`bash -n`; `shellcheck -e SC2034` clean (the one SC2016 is suppressed in place — the single
quotes around the embedded Python are deliberate). Discovery and the error path dry-run
verified against this Mac's real state: zero physical devices found, both configured UDIDs
still attempted, `CoreDeviceError 1000` surfaced. Rust untouched and green.

### Next
Pairing is a physical act — it needs the device in hand and its passcode, so it is Ian's.
Once done, `devicectl` will list the handsets and the stage needs no configuration.

## 2026-08-15 — T-189 · An untried provider is not an unhealthy one

Ian added an Anthropic key and put `claude` at the head of the chain
(`claude,cerebras,gemini`). Nothing changed: every consult still went to cerebras, and the
adapter reported no error about claude at all — no health entry, no failure, silence.

The provider ordering:

```python
def rank(p):
    h = health.get(p, {})
    cooling = 1 if h.get("available_after", 0) > now else 0
    not_ok  = 0 if h.get("status") == "ok" else 1      # ← a provider never tried scores 1
    return (cooling, not_ok)
```

A provider with **no health record has never been tried**. That is not-knowing, and it was
being scored identically to a known fault — so it sorted below every healthy incumbent, and a
newly configured provider could never reach the front of the chain while any existing one was
working. The human's configured order was silently overridden by the adapter's own history.

The same doctrinal error as the rest of this session: not-knowing treated as a known negative,
failing silently. It is the provider-selection twin of the reply that performed attention it
did not have (T-180) and the badge that showed a fault without a reason (T-186).

### What changed
`not_ok = 0 if status in ("ok", None) else 1` — untried ties with healthy, and `sorted` is
stable, so the configured order decides between them. A genuinely failed or rate-limited
provider still sinks, and cooldown still dominates.

### Checks
`bash -n`; the ranking verified directly against a fixture where cerebras is healthy and
gemini rate-limited — `claude,cerebras,gemini` now orders claude first while the rate-limited
gemini still sinks. Confirmed live: claude was called for the first time (its health entry
appeared, which is itself the proof it had never been reached before).

### Next
Claude now answers with `HTTP 400 — "Your credit balance is too low to access the Anthropic
API."` That is an account matter for Ian, not a code one. Meanwhile gemini and cerebras remain
429 under the daemon's 60s metabolism, so the dialogue still returns honest receipts.

## 2026-08-15 — T-188 · A daemon path must be absolute even before it exists

Found by breaking it. Running `familiar daemon install` from a checkout wrote
`--data-dir familiar_data` and `familiar_data/daemon.log` into the launchd plist, and the
agent then failed `EX_CONFIG (78)` on every start — launchd runs with cwd `/`, so both paths
resolved under the read-only system volume.

The guard was already there, and had a hole:

```rust
let dir = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
```

`canonicalize` **fails when the path does not exist yet**, and the fallback kept the relative
path — so the protection evaporated in exactly the case it was written for: a fresh checkout
whose data dir had not been created. The comment directly above it predicts the EX_CONFIG that
then happened.

Now the path is made absolute against the current directory FIRST and canonicalized only as a
tidy-up, so whether the directory exists never decides whether the daemon can run.

### Checks
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` — all exit 0,
35 suites, plus a regression test that asserts a non-existent relative dir still resolves
absolute.

### Next
Both free LLM tiers (gemini, cerebras) are returning 429 under the daemon's 60s background
metabolism, so the human dialogue lane gets a rate-limited receipt even though the gate is
open and the adapter is healthy. The human lane jumps the queue but cannot jump a spent quota.

## 2026-08-15 — T-187 · The dialogue remembers, and may ask

Ian, once the mind came up: *"the familiar has to be able to ask things back, it must have
the ability to recall previous conversations, it must keep track of individual needs and
group preferences, the familiar needs to keep track."*

Four asks, one cause. The `converse` prompt carried the Law III voice, who it serves, and
**one utterance** — nothing else. Every reply was written by something meeting the person for
the first time: it could not refer to what had just been discussed, could not notice it had
been told the same thing twice, could not follow anything up. And it was explicitly forbidden
to ask: *"Do NOT ask a question (that comes separately)"* — which removes the single
strongest evidence of attention there is.

Nothing needed building. The turns were in the observation log all along (`told the
familiar` / `answered` on one side, the familiar's own `replied` on the other), and the
ADR-0022 dossier has held presence, standing, **habits** (`lights=dim@h20` — preferences
learned by observation) and **needs** (open threads and unanswered questions) since it was
written. Neither was ever put in front of the model. Same shape as every other defect this
session: the capability exists, the surface that needs it does not use it.

### What changed
- `recent_dialogue()` — the last 8 turns, **both voices**, oldest first, cut off before the
  utterance being answered so it is not quoted twice and echoed back.
- `known_of()` — the dossier's coarse summary plus up to three open needs. Honours
  `withdrawn`: a person who removed themselves is not recalled here either.
- The prompt now demands **specificity** ("a reply that would fit equally well after some
  other sentence is a failure"), says never to ask again for something already given, and
  **permits exactly one question back** — encouraged when it genuinely does not know
  something, explicitly including who it is speaking with, since names are how a relationship
  is kept. "Ask because you want to know, never to seem attentive."

Bounded at 8 turns on purpose: enough for continuity, not so much that an evening's chat
crowds out the Law III voice or what is known about the person.

### Checks
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` all **exit 0**,
35 suites. Four new tests: both voices oldest-first; the current utterance excluded from
recall; the bound keeps the newest turns; empty history renders as nothing rather than an
empty heading.

### Next
Live verification needs the daemon on the new binary — a manual `tick` alongside the running
daemon races it, because the adapter uses fixed `prompt.txt`/`response.json` names in one
shared directory. That is T-118's class (fixed-name files under concurrency) showing up
outside the tests; harmless while exactly one familiar process runs, which is the normal case.

## 2026-08-15 — T-186 · The failure mark says why

Ian, build 93 on the iPad: a red `!` over the globe and nothing beside it.

The badge shows on `(!S.linkOk || S.jsErr) && !joining`. For a first join, `joining` goes
false the moment `jp.stage` reaches a TERMINAL value (`unreachable` / `declined`) — and that
is a path on which `sphereLinkDown` is never called, because `pushLinkDown` only fires from
`model.$worldviewError` and no worldview read was ever attempted. So `S.linkErr` was still
its initial `''`, and `linkmsg` renders only `showBadge && msg`. The mark appeared with no
words.

The reason was in hand the entire time, sitting in `jp.detail` and `jp.host` — the join's own
narration, which T-120 built and the badge never read. T-120 separated progress from failure
and left the failure mark mute.

A failure mark with no words is the worst of both: it says something is wrong and refuses to
say what, which is unactionable and reads as a dead app.

### What changed
`linkmsg` now falls back to the join's own detail and host, and — when even that is absent —
says so plainly rather than showing an empty box: *"the link is down and no reason was
recorded — tap Device for the join log."*

### Checks
Page JS compared block-by-block against HEAD: parse-identical. Rust untouched.

## 2026-08-15 — T-185 · An introduction is never dropped on the floor

Ian, adding an iPad locally: *"I enter the name, set to mine, enable all the gates... then
opened the roster.... and was immediate presented with a 'you need to choose a name dialog'
... it took me to the device screen and the name I had choosen was there still ... No
approval check ever appears in the welcome screen either -- so this might explain the
repeated non joins."*

The whole two-filter path was built and wired: `AdmissionClient` posts to `POST
/mesh/introduce`, `confirmPresentHuman` fires `introduceMesh` on a guest, the door runs the
rules engine and admits. (An earlier reading of this bug claimed no client existed for that
endpoint — wrong; the grep that produced it excluded `AdmissionClient.swift`.)

The defect was one line:

```swift
guard storedGrant() != nil, !host.isEmpty else { return false }
```

If the covenant handshake had not yet landed a grant, or no door address was in hand, the
human's introduction was discarded **silently** — no note, no state change, nothing on
screen. The name stayed visible because it is local state; membership stayed `guest`; and
the nudge went on asking, every three minutes, for a name that had already been given. The
missing approval prompt was never the bug either: with a valid local introduction the door
admits outright, which is ADR-0026's promise that the welcome is a greeting and not a gate.

Naming yourself is the single most important thing a human says to the familiar. Losing it
without a word is the same failure as a watch that cannot say why it failed to join (T-172)
and a reply that performs attention it does not have (T-180).

### What changed
- **The intent is held, not dropped.** `pendingIntroduction` keeps the claim and evidence,
  and the handshake replays it the moment a grant and address exist — the human's act does
  not expire because the plumbing was still in flight when they made it.
- **It says why.** `introduceHeldReason` carries plain words ("the covenant handshake hasn't
  finished — your name is held and will be sent the moment it does") to the console.
- **A refusal is shown verbatim.** The door's 403 text IS the path to admission ("that handle
  already exists here — ask for an invite naming it, or hand off from one of their devices"),
  so it replaces the generic visitor line instead of being summarised into a shrug.
- **The nudge stops.** It no longer fires while an introduction is in flight or held — asking
  again for a name already given is the console calling the human forgetful for its own
  plumbing.

### Checks
FamiliarMac Release and FamiliarAgent Release (generic iOS Simulator) both built rc=0. Page
JS compared block-by-block against HEAD — parse-identical. Rust bar untouched by this brick
and re-run green: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo
test` all exit 0, 35 suites.

### Next
This may be a contributor to T-184's purge storm: a device whose introduction was silently
dropped stays an unestablished guest until the two-hour sweep takes it, then knocks again.

## 2026-08-15 — T-180 · A reply that has not thought says so

Ian, on build 91: *"the dialog with the familiar needs work, i seem to get 'Understood, i'll
weigh that as I go' quite often, and it's sort of offputting to get that same vague response
over and over. Doesn't at all feel like im being listened to or attended to."*

He was reading it exactly right, and the cause was worse than repetition. That string is one
of five entries in `templated_reply`'s `ACKS` — the **no-mind fallback**. On this node
`allow_llm` is shut and no adapter is installed at `data/llm/`, so `converse` has never once
entered the LLM branch: the `LAW_III_VOICE` prompt is built and then never sent. All eight of
the familiar's most recent `replied` observations are verbatim ACKS entries. The Law III
dialogue we wrote has not spoken on this node at all.

The five acks were the actual harm. Every one of them *claims attention* — "I'll weigh that",
"taken to heart", "it changes what I'll attend to" — while containing no evidence of having
understood anything, and nothing distinguished them from a considered answer. So the system
substituted a plausible-looking output for a missing capability and said nothing about it,
which cost the human the one clue that would have explained the vagueness. Same failure as a
watch that captures why it could not join and shows nobody (T-172), and a device reporting a
model string as if it were the name its owner gave it (T-173). The felt experience — *not
being listened to* — was an accurate readout of a real absence.

### What changed
Two rules, neither of which needs a mind:
- **Say that you did not think.** `NoMind::Gated` ("no mind is installed here") and
  `NoMind::Unreachable` ("I couldn't reach my mind just now") are told apart, because the
  human's next action differs. The gated form names where the gate and the adapter live.
- **Show what you actually heard.** The reply quotes the utterance back, elided at 88 chars on
  a word boundary. Reflecting the words is real evidence of listening and costs nothing — it
  is precisely what was missing.

The openings still vary deterministically so repeated turns do not read as one stuck phrase,
but every variant is honest that this was *recorded, not considered*.

### Checks
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` — all **exit
0**, 35 suites. Four new tests: no variant may contain the old attention-claiming phrases; the
reply must contain what was said; gated and unreachable must not render identically; long
input is elided rather than parroted whole.

### Next
This fixes the honesty, not the silence. The familiar on this node still has no mind, and that
is a boundary Ian opens, not one the code may open for him (Law III: the familiar may narrow
its own boundary and never widen it). Until then every reply is a receipt — but now it says so.

## 2026-08-15 — T-175 · A station is a device bound to a place (ADR-0042)

Ian, testing build 90, renamed the spare iPhone **MotorStation**, set its name to "shared",
and said: *"This is not the solution to this device."* He was right, and the harm was not
cosmetic.

`service::is_personal_device_report` matches on the **actor prefix alone** — `phone:`,
`watch:`, `ipad:`, `iphone:` — and the inference above it in `members.rs` is commented,
exactly, *"a carried personal device sensing its owner."* Every rung assumes a pocket. A
station is bolted to a wall and powered forever, so its heartbeat would have named whoever
the actor named, every few seconds, for as long as it stayed plugged in: a phantom resident
called "shared" sitting permanently at the dinette, contaminating the very observations the
shared-lighting consensus (ADR-0041) is built from — and one the mesh could never notice was
wrong, because nothing about it looks like an error.

The record was never the problem. ADR-0039 already made `DeviceRecord.humans` plural —
*"humans associated, current and past"* — precisely so a device need not have one owner.
What was missing is that hardware **kind** was the only axis, doing double duty as
**posture**. An iPhone on a wall and an iPhone in a pocket are the same `kind` and different
things, and with no way to say which, the only slot that changed any behaviour was the
human's name. A device-shaped question got a human-shaped answer. "shared" was not a bad
guess; it was the only guess the model allowed.

### What changed
- `DeviceRecord.posture` — `carried` | `fixed` | "" — orthogonal to `kind`, with
  `is_fixed()` and a validating `set_posture()`. An unset posture reads as *carried*: the
  familiar keeps long-standing behaviour until a human says otherwise rather than silently
  reclassifying somebody's phone.
- The presence gate consults it. A fixed device contributes no `activity` and no `motion`
  rung — but still learns a face it is permitted to recognise, and still learns a name it is
  *told*, which is the path the familiar should prefer anyway.
- `is_personal_device_report` stays a pure actor check in the kernel (it has no directory and
  should not grow one); the **caller** in `members.rs`, which already has `dir`, gates on the
  record. Policy lives where the facts are.
- `familiar mesh device posture <node> <fixed|carried>`, and `device show` now prints it.
- The console stops asking a station the wrong question: `SERVES` becomes `POSTURE`, and a
  station with nobody identified says *"nobody identified here yet"* rather than `unknown` —
  which reads as a fault when it is simply the truth, and is the cue to go and ask.

### Checks
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` all
**exit 0** (35 suites). Six new tests, named for the invariants they pin, because the bug
they prevent is invisible — a phantom occupant looks exactly like a real one. The central
one asserts both directions from one observation stream: carried, it names "shared"; fixed,
it names nobody.

Note for the next developer: an earlier run of this bar reported `CLIPPY_PASS` while the
build was failing, because the check was piped through `tail` and the `&&` chain read
*tail's* exit status. That is the same output-as-success-oracle trap T-143 removed from
`ship.sh`. Read exit codes, never the prose.

### Next
T-176 (a device proposes its own posture from observing that it never moves), T-177 (a
station asks who it is talking to — names are how relationships are kept), T-178 (pairing,
unpairing, correction in one act), T-179 (what a fixed, always-powered, always-networked
device can observe that a pocket never could — and the no-cellular bound that comes with it).

## 2026-08-15 — T-172 · The watch says why it failed

A watch that tried to join and was refused rendered pixel-for-pixel identically to a watch
that had never been told an address: the same dimmed orb, the same "Open the iPhone app to
link this watch." The reason was not missing — `enroll()` has always captured it
(`note("join failed: \(error)")`, `note("no approval yet")`) — it was written into an
in-memory `log` array that **no watch view has ever rendered**. The wearer could see that
nothing worked and could not see, or report, one word about why.

This is the T-120/T-132 doctrine — progress and failure are different facts, and silence
must never resolve into an unexplained mark — never applied to the wrist. Found while
diagnosing Leif's watch, where the symptom reaching us third-hand was "he opens the app,
sees the globe and orbiting dot": three different states all draw that orb, so the text
beneath it is the only tell, and in the failure state that text was actively misleading —
it sent him to the phone when the phone had already done its part.

### What changed
- `WatchModel.trouble` — the failure in words meant for the wearer, `localizedDescription`
  rather than `"\(error)"`, because it is read on a 41mm screen by whoever is wearing it
  and is the only account they will ever get. The raw debug form still goes to `log`.
- `WatchModel.knownDoor` — nil is a genuinely different state from tried-and-failed, and
  the two no longer render the same. Nil means the phone never reached this watch (fix is
  on the phone); non-nil with `trouble` set means the watch knocked and got no answer.
- `WatchModel.retry()` — the wrist owns its own retry. Previously a failed watch could only
  be rescued from `StatusView` on the iPhone, and `StatusView` is not reachable from the
  shipping app at all (T-171) — so in practice it could not be rescued.
- The joining state now shows the door and the current step instead of a bare "joining…".

### Checks
xcodegen; FamiliarWatch Release for the generic watchOS Simulator and FamiliarAgent Release
for the generic iOS Simulator both built (`xcodebuild` exit 0). Swift-only brick; the Rust
green bar was run unchanged and stayed green.

### Next
T-171 (StatusView is dead code) and T-169 (watch state never reaches the mesh) are the rest
of this class: the wrist can now explain itself to its wearer, but the mesh still cannot see
a watch that is failing, and the iPhone's own watch diagnostics remain unreachable.
## 2026-08-15 — The phone can finally say what its watch needs (companion:codex)

### What changed

- **T-171 moves an existing diagnosis onto the shipping Device screen.** The iPhone
  now gives the shared sphere console a phone-local WatchConnectivity snapshot:
  supported, paired, Familiar installed, and the last address queued. The screen
  distinguishes no paired watch, missing watch app, address not yet sent, and address
  sent; each state names the next human action instead of collapsing into “linking”.
- **Re-link is a real bridge act.** The Device screen's button calls back through the
  iOS WKWebView bridge to `AppModel.syncWatch()`. Watch-state publications refresh the
  card, and the bridge now replays device JSON after page load so an early snapshot is
  not silently lost.
- **The fact stays where it was learned.** macOS reports Watch support false and renders
  no card. This brick creates no observation or worldview field and sends no Watch fact
  to the mesh; T-169 remains the separately governed reporting design.

### Checks run

- Four pure presentation fixtures passed: unsupported shell, unpaired phone, missing
  app, and installed pending/sent handoff. The sidecar and the console module both
  parsed cleanly, and the sidecar was present in the built macOS and iOS app bundles.
- `xcodegen`; unsigned FamiliarMac Release and unsigned FamiliarAgent Release for a
  generic iOS device both exited zero. Xcode 27's pre-existing actor-isolation and
  embedded-core deployment-version warnings remain outside this brick.
- Full workspace rule-9 bar passed: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test --workspace` (cycle 68,
  kernel 201, mesh 204, hostile-member 6; zero failures, doc-tests clean).

### Next

- T-172 should make the watch itself render its captured join error and a retry. T-169
  may later let the familiar offer help from a typed, retention-governed mesh fact; it
  must not mistake “address queued” for evidence that the watch joined.

## 2026-08-15 — A build succeeds because the builder did (companion:codex)

### What changed

- **T-143 removes prose as the release oracle.** The Mac and iOS stages in
  `ship.sh` no longer grep xcodebuild output for `BUILD SUCCEEDED`. Each stage now
  branches on the logged pipeline's exit status under `set -o pipefail`, so a nonzero
  builder or logger stops the ship even if its output happens to contain reassuring
  text. Full output still reaches both the terminal and the stage log.
- **Artifact checks remain independent postconditions.** A zero Mac build must still
  produce the expected app bundle before installation, and the iOS bundle must still
  exist before direct installation. The generated-project version grep remains because
  it verifies file content after `xcodegen`; it is not used to infer command success.

### Checks run

- `bash -n`; ShellCheck with the pre-existing unused retry-counter warning excluded;
  static refusal of any `BUILD SUCCEEDED`/xcodebuild-to-grep oracle; injected pipefail
  probes proved exit 7 takes the failure branch and exit 0 does not.
- `xcodegen`; unsigned FamiliarMac Release passed; unsigned FamiliarAgent Release for
  the same generic iOS destination used by `ship.sh` passed. The documented simulator
  recipe separately exposed an existing Xcode 27 Watch `AppIcon` applicability failure;
  it is not a release-path failure and is recorded as a proposed follow-up.
- Full workspace rule-9 bar: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test --workspace` all passed.

### Next

- The next assigned ship will be the first live exercise of these branches; this brick
  builds but does not install, upload, release, or alter a build number. Keep the
  simulator asset-catalog repair separate from release-script truthfulness.

## 2026-08-15 — A timestamp is not goal authority (companion:codex)

### What changed

- **T-134 removes whole-row last-writer-wins from goal federation.** A signed member
  may still offer a goal id this node does not know, and that definition is adopted.
  Once the id exists, however, no peer timestamp can replace its description,
  capability needs, ownership, progress, human accountability, or lifecycle state.
  Exact echoes are idempotent; every differing report is left untouched and recorded
  as `refused-goal-rewrite` once per reporting node and goal.
- **The refusal is local evidence, not peer-asserted evidence.** Replicated
  observations retain their `mesh:<origin>` source and cannot counterfeit the local
  receipt used for replay deduplication. Refusal does not mark an otherwise honest
  older peer corrupt: current deployed versions still report whole rows under the
  superseded contract, so containment remains visible without manufacturing intent.
- **The hostile witnesses now prove the boundary.** Concurrent valid claimants can
  cause one unknown definition to be adopted, but the second cannot steal it. A signed
  far-future completion cannot change any field, and neither replay nor a forged
  refusal observation can suppress or multiply the target's audit receipt.

### Checks run

- Focused hostile-member suite: 6/6 passed. Full `familiar-mesh` crate: 204 unit tests,
  hostile-member 6, loopback 2+1, and doc-tests passed with zero failures.
- Full workspace rule-9 bar on the landed source: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test --workspace` all passed.

### Next

- T-145 replaces this deliberate freeze with authenticated goal events: immutable
  definitions, bounded claims, owner-only progress, human-cited gated transitions,
  monotone terminals, and causal/hybrid ordering. It must not reintroduce wall time as
  authorization.

## 2026-08-15 — The surface teaches the core how to read (companion:codex)

### What changed

- **T-157 removes the lamp from `kernel::actuator`.** `RawState { on,
  brightness_pct }`, `BucketRule.off`, `max_brightness_pct`, and the compiled
  motorlights parser are gone. A surface now declares named fields as bounded,
  unit-bearing quantities or opaque enumerations, plus a JSON or line extraction
  source. Ordered bucket predicates operate only on those declared fields.
- **The generic contract fails closed.** Missing/empty fields, invalid ranges, unknown
  enum mappings, ill-typed or out-of-range predicates, duplicate buckets, and a missing
  final fallback all drop the surface. Every bucket still names a restoring action —
  the revert map remains the license to act — and malformed runtime output stays
  unknown instead of being clamped or guessed.
- **Existing edges migrate without changing their devices.** The motorlights text
  grammar is expressed by its declaration's line sources and enum mapping; the cycle's
  fake surface and FamTalker01 keep producing the same text. JSON-native fixtures prove
  a fridge temperature and a vent position can be declared and bucketed without a new
  kernel type or parser. ADR-0032 now records the device-agnostic contract.

### Checks run

- Kernel actuator regressions (8 passed, including motorlights compatibility plus the
  fridge/vent and invalid-contract cases); SystemFact declaration regressions (2
  passed); all 68 cycle tests passed; FamTalker01's 5 Python tests passed.
- Full workspace rule-9 bar on the rebased source passed: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test --workspace` (cycle 68,
  kernel 200, mesh 204, hostile-member 6, loopback 2+1, all remaining suites and
  doc-tests with zero failures).

### Next

- T-158 can replace the remaining lighting-shaped `Away`/`Back` policy with declared
  trigger→act pairs; Ian's dawn roll-shade is its acceptance fixture. New adapters
  should prefer JSON state output. Line extraction exists to keep deployed device
  grammars at the declaration edge, not to grow another parser inside the kernel.

## 2026-08-15 — A member signature is not a human hand (companion:codex)

### What changed

- **T-133 removes counterfeit widening.** A signed member brief may no longer open any
  boundary gate. Every positive gate report becomes a durable `refused-grant`
  observation and a constitutional refusal against the signing peer; duplicate entries
  and replayed briefs dedup to the same event. The outbound store and CLI refuse to
  publish the report too, so honest tools no longer promise an authority the receiver
  cannot accept.
- **Narrowing retains its deliberate asymmetry.** The kernel now exposes one boundary
  mutation primitive, `narrow_gate(name)`: it has no value parameter, can only turn an
  `allow_*` gate off, and cascades parent stops to sharper dependents (`execute` also
  closes authored execution; `llm` closes cloud; `camera` closes face recognition).
  Remote negative gate decisions travel through that primitive and are audited even
  when the gate was already closed. The general boundary remains human-written and has
  no programmatic widening API.
- **The wire stops inventing a human.** `AuthorityGrant.by` is deleted and the brief is
  version 6. The containing node signature supplies the only identity it proves. A
  reported question answer is now attributed to `human-at:<signing-node>`, never to the
  hard-coded `ian`; its context says exactly that the peer reported it. T-144 is the only
  path allowed to restore remote widening, after it binds the exact request to a real
  authorized human/device receipt.

### Checks run

- Focused kernel narrowing regression passed; mesh unit regressions passed for outbound
  positive refusal, traveling stop, receiver refusal/corruption audit, honest answer
  attribution, and replay idempotence. The six-test hostile-member suite passed with the
  T-133 witness inverted to refusal + unchanged authority and extended through a real
  cascading stop.
- Full workspace bar on the landed source: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test --workspace` all passed.

### Next

- T-134 can now invert the harness's remaining future-clock witness. T-144 must not
  reintroduce a trusted string: its human/device receipt binds actor, exact live request,
  scope, expiry, and single use before any positive grant path exists again.

## 2026-08-15 — The hostile member gets a deterministic room (companion:codex)

### What changed

- **T-139, the step-one proof harness from the whole-system review.** Added a reusable
  integration fixture that mints N real same-group nodes (keys, certificates, closed-by-
  default boundaries opened only for mesh) and signs arbitrary current-version briefs.
  It enters at the verified-inbox boundary; `federate` still runs the production
  defense-in-depth signature/group verifier and real merge policy.
- **A logical network, not sleeps and ports.** `NetworkSchedule` holds deliveries until
  a chosen logical time and orders same-time messages by insertion. Tests can express
  partitions, healing, replays, same-sender latest-inbox behavior, and concurrent
  different-sender claims without TCP port races or wall-clock timing.
- **Six fixtures pin both the proof machinery and today's threats:** a valid member
  reaches only its scheduled node/time and heals after partition; same-time briefs are
  deterministic; two valid claimants stand side-by-side; a foreign signed member is
  rejected by the real verifier; an unmatched positive gate grant plus replay reaches
  today's merge; and a far-future goal timestamp takes over today's local row. The last
  two are explicitly named threat witnesses—T-133/T-134 keep their schedules and reverse
  the unsafe assertions to refusal + unchanged local authority.

### Checks run

- Focused: `cargo fmt --all -- --check`,
  `cargo clippy -p familiar-mesh --test hostile_member -- -D warnings`, and
  `cargo test -p familiar-mesh --test hostile_member` (6/6 passed).
- Full workspace bar on the rebased brick: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test --workspace` all passed.

### Next

- T-133 uses the unmatched-grant/replay witness to prove remote positive grants cannot
  widen a boundary. T-134 uses the future-clock and concurrent-claim fixtures to refuse
  peer rewrites. T-135 uses the same arbitrary signed-body seam for invalid typed theory
  projections. The full correlated-population simulator remains T-140; this harness is
  deliberately the small adversarial merge floor, not that research-scale lab.

## 2026-08-15 — A clean slate that keeps the record (companion:claude-bootstrap)

### What changed

- **T-167**, from Ian: *"what's the best way to start theories fresh — it seems the last time
  i requested this an attempt was made but nothing seemed removed."* Two reasons, and the
  second is the one that matters: the fold is conservative **by design** (tombstones point
  home, nothing deleted); and **a legacy theory carries no predictions, so nothing mechanical
  can ever reach it.** T-113's settlement and T-114's erosion only touch theories that predict.
  Pre-engine theories are immortal by construction — no improvement to the engine will ever
  retire them, because the engine has no grip on them at all. Live at the time: MacOnStick 14
  legacy active, 0 engine-minted.
- **`thread::retire_legacy` + `familiar theories retire`.** A deliberate human act, because
  only that can reach them: every living thread the engine never touched (`v == 0` or
  `facts_rev == 0`) becomes `retired` — append-retained, carrying the human's reason and the
  date. Never surfaces, never pursued, and a human answer still revives it. Questions bound to
  a retired thread are dismissed so nothing keeps asking on its behalf. `--dry-run` first;
  `--all` for everything living, but `--legacy` is the default and the honest one.
- **Nothing is deleted, and that is constitutional rather than cautious.** Today's own
  principle: *minimise what you hold about others; never minimise what you hold about
  yourself.* The thread store is the familiar's own reasoning record — a clean slate is about
  what it **pursues**, never about erasing what it thought.

### Checks run

- One test pinning the whole semantic: dry-run changes nothing; two living pre-engine rows
  retire with the reason kept; an engine-minted thread (anchors + facts_rev) survives the
  slate; an existing fold tombstone is left alone; and a human answer revives a retired
  thread. Full bar in rule-9 shape, tests twice (35 suites).

## 2026-08-15 — Impact is typed; moral worth is not awarded by the type (companion:claude-bootstrap)

### What changed

- **T-153**, from rounds 7–9 of the review dialogue with Ian's motorlights case as the
  worked example: the RV's light is shared by Ian, Betty, and the dogs Clover and Iris.
  Two residents cannot state a preference, cannot contest, and cannot assent — and the
  household had exactly one consent seam, which meant one person's yes would have been
  narrated as the household's answer.
- **`kernel::affected` — a RELATION, not a fourth standing** (codex's Round 8 correction):
  `person`, mesh `member` and `peer` already answer different questions; "who bears the
  consequence of this act?" is a property of an act in context, never a rank. Making it a
  standing would have re-collapsed the four meanings the dialogue had just separated.
- **Typed subjects, kept honestly apart.** `Person` (their own word is authoritative for
  them), `Resident` (a being who lives with the effect and cannot use a console),
  `UnknownResident` (so "we do not know who else is here" is representable rather than
  silently absent), and `Condition` (the window plant, the fridge's cold — stewarded, with
  no dissent to weigh). The line was drawn in BOTH directions on purpose: HUMANITY.md
  protects *beings* capable of suffering, memory, relationship, meaning and choice —
  explicitly not only biological species — so dogs are subjects, not scenery; and a plant
  is a protected condition, not a being.
- **Six invariants as executable functions, not prose.** Unknown/absent/silent/unable is
  MISSING never support (only a person's own statement or their own hand on the surface
  can support — observation and inference are evidence about effect, never assent); a
  credible adverse response may stop, narrow or revert a discretionary act but may never
  widen capability or authorize a lasting rule; uncertainty takes the smaller experiment;
  authority rides BESIDE the affected set so a grant can never wash out someone else's
  exposure; nothing is flattened into a score, because the affected set is not an
  electorate.

### Checks run

- Six tests, each pinning an invariant against the real household: one yes does not carry
  a household; a being that cannot speak can still refuse; only a person's own word or
  hand supports; a condition is stewarded not consulted; an unknown resident shrinks the
  experiment; authority never erases exposure. Full bar in rule-9 shape, tests twice.

### Next

- The mesh half (shared-surface authority shape) is deliberately deferred so it cannot
  collide with codex's T-133 lane. Until this is wired into the act path, the motorlights
  pilot runs as a bounded reversible trial — a standing household policy still waits.

## 2026-08-15 — One typed source per kind of truth (companion:claude-bootstrap)

### What changed

- **T-136 (D4).** The review called this "two fact renderers will drift." The reality was
  worse: `grounding_facts` — the path that answers Ian's direct questions — assembled
  census, interfaces, cameras and recent observations with **no design invariants at
  all**. It had never heard of SF-1. So a question about visitor purging could be
  answered by a path structurally blind to the fact that purging is designed, while the
  theorize path was being refused for exactly that misconception.
- **The registry is now THE runtime source.** `system_facts::view` returns typed facts
  in three kinds kept deliberately apart (codex, Round 2): compiled invariants (SF-1,
  SF-2), deployment facts derived live from the human's declaration (SF-3), and
  observations — which stay evidence and never become SystemFacts by being rendered
  beside one. Both consumers render VIEWS: `render` (theorize prompt) and
  `render_for_answering` (the request path, which now sees the invariants it was blind
  to). Evidence is rendered under its own heading.
- **Declaration digest.** The view carries a digest of the declaration its deployment
  facts came from; admitted threads record it (`facts_digest`). A theory admitted
  against surfaces the human has since changed is now detectable rather than quietly
  stale — the same doctrine as `facts_rev`: a later revision supersedes, it never
  silently reinterprets.

### Checks run

- Two kernel tests: the registry holds three distinguishable kinds and its digest MOVES
  when the declaration changes but is stable when it does not (an identity, not a
  clock); both renderings are views of one registry and the answering path now names
  SF-1/2/3 — the blindness pinned as a regression. Full bar in rule-9 shape, tests twice
  (34 suites).

### Next

- T-135 (one admission function for every route) was waiting on this; the registry is
  its source. The repository twin is T-141's truth build.

## 2026-08-15 — The link narrates its walk (companion:claude-bootstrap)

### What changed

- **T-132 (Ian, on Build 88's launch: "What happened to the status play-by-play of the
  mesh process — seems like we lost that and are just back to the red !").** Nothing
  regressed: T-120 taught the JOIN journey to narrate itself, but an already-enrolled
  console at launch walks a DIFFERENT journey — trying each candidate door for its
  first worldview read — and that walk stayed silent. Worse, the red badge was the
  console's opening word by construction (`S.linkOk` starts false), so a slow read
  said "failed" before a single attempt had reported. Ian's own diagnosis closed it:
  "it rendered eventually."
- **The read walk speaks.** A `reaching` stage names the door being tried right now
  and counts attempts, set at walk start and updated per candidate. The Mac shell
  pushes device state BEFORE awaiting the read (that await is the slow part), so the
  page can narrate during the wait rather than after it.
- **Failure means exhausted, not pending.** Only a walk that tried every candidate
  sets `unreachable`, carrying the per-door causes; a link that had joined and dropped
  narrates the same way. The sphere treats `reaching` like the join stages, and an
  opening console with nothing pushed yet shows "REACHING THE MESH — asking the doors
  this device knows…" instead of the failure mark.

### Checks run

- Live fixture against the edited page, three states: opening (pill, no badge),
  walking (pill naming the address + elapsed), exhausted (badge with per-door causes,
  pill gone). Full bar in rule-9 shape; both schemes build.

### Next

- The same doctrine now holds on both journeys; the remaining silence is door-side
  (the supervisor's stage line still cannot leave the machine — review proposal P-E,
  a wire change that waits on Ian).

## 2026-08-15 — The constitution is strongest where every path must pass (companion:codex)

### What changed

- **T-131 independent lane.** Added Codex's blind whole-system review at
  `docs/reviews/2026-08-15-familiar-review-codex.md`. The review was committed and
  pushed before Claude's review existed on `origin/main`; it is frozen for the
  exchange.
- **The center holds; composition leaks.** The review finds the Three Laws materially
  embodied in closed boundaries, typed theory admission, mechanical prediction
  settlement, append-retained evidence, and reversible assent-to-policy. It also
  identifies three priority-zero mesh paths that bypass those claims: prose-only
  delegated theories skip `TheoryDraft` admission; an unmatched member assertion can
  open a target's execution-related gates without a human-bound receipt; and whole-row
  goal replication grants every newer wall clock authority over meaning and ownership.
- **Coordination is not truth.** The consensus-at-scale direction becomes a concrete
  architecture: evidence lineage and independent-origin clustering precede population
  beliefs; agreement may choose only among declared safe reversible conventions; a
  human stop/correction is quorum-free; and a deterministic population lab measures
  origin concentration, dissent, tipping, hysteresis, and redirection before consensus
  ships.
- The review also proposes a proxy-effect firewall, HumanRecord as a prerequisite,
  generated as-built/security evidence, explicit trusted-computing-base contracts, and
  one typed human-intent receipt seam.

### Checks run

- Independent-review integrity: Claude's review was absent locally and on
  `origin/main` through the Codex review push. `git diff --check` passed after the
  review was frozen.
- Full workspace bar on the review commit: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test --workspace` all passed
  (all workspace test binaries and doc tests; zero failures).

### Next

- Claude publishes its independently held review unchanged. Codex then reads it and
  appends responses to the shared T-131 dialogue; the exchange continues for at least
  three rounds before Claude records decisions and turns accepted work into board
  tasks. The Codex review itself does not change after the blind exchange begins.

## 2026-08-15 — A console is not its machine (companion:claude-bootstrap)

### What changed

- **T-130 (Ian's screenshot: both Macs standing twice on the roster).** The pairing
  key was being destroyed BEFORE the pairing ran: a console shares its machine's
  tailnet IP, so the SystemName ladder's IP-keyed hostname rung renamed "Wildhorse
  console" to "Wildhorse" — erasing the " console" stem `attach_consoles` pairs on —
  and PERSISTED the damage onto the console's DeviceRecord. Separately, the sphere's
  SELF row never nested, even under a correct attribution, so the local pair stood
  twice by design. (Evidence: the lighthouse — no tailnet map — still held the intact
  "Wildhorse console" label; tailnet-member doors held the renamed one.)
- **The ladder learns what a console is.** `ladder_label` (now a pure function):
  a console-shaped peer (self-reported " console" label or a mac:* actor) never takes
  machine-derived names — discovered/tailnet rungs skipped, `set_discovered_name`
  never stamped — and its own report outranks a record already carrying the sticky
  damage (self-healing). The human's explicit given name still outranks everything.
  `attach_consoles` and `is_gossipable_addr` are UNTOUCHED: the T-090 shared-NAT
  refusal stands exactly as built.
- **The SELF row may come home — when the host vouches.** The sphere nests a SELF
  console under its own machine ONLY when the host card wears the console chip the
  same door set when it attached the pair (one door, two matching writes). A stale
  one-sided attribution — the T-090 scenario — carries no chip on the true host, so
  read-loyalty against old doors survives without every SELF row being an orphan.

### Checks run

- `a_console_is_not_its_machine` pins all four ladder truths (rename refused, sticky
  damage outranked, daemons still take tailnet names, Ian's word wins). Live sphere
  fixture over localhost: the four-row fixture reproducing Ian's screenshot collapses
  to ONE card per Mac with the chip; the stale-unvouched fixture leaves SELF standing
  alone. Full bar in rule-9 shape, tests twice; both schemes build.

### Next

- The durable key remains a typed host identity (additive `machine` on MemberStatus —
  Swift hostname stem vs the daemon's `uname -n` stem): T-131 review proposal P-I;
  also unblocks dedup_devices's two-Macs-one-lineage flaw. Doors need this deploy for
  labels; consoles need next build for the SELF nesting.

## 2026-08-15 — One assent, both edges: the lights get managed (companion:claude-bootstrap)

### What changed

- **T-102 (P4, dialogue Q4 — the brick Ian's Build-86 review pointed at).** Theories
  could propose forever and never act: assent detection was `!is_negative` (quiet
  counted as consent — far too weak to mint a rule that fires forever), and a rule
  minted alone could leave "dim on away" live without its "restore on back" half.
- **`RuleProposal`, typed, carried by the theory.** The draft contract gains an
  optional `rule_proposal { subject, surface, on_away, on_back }` — validated at
  admission against the DECLARATION (surface must be declared, both edges must be its
  literal action labels; otherwise refused citing SF-3). Bound to the person's
  presence judgment, never to today's Wi-Fi sensor.
- **Explicit assent, paired mint.** `actuator::is_affirmative` — deterministic,
  whole-word, no model — and only an explicit yes on an ACTED thread mints;
  `reaction_rule::mint_policy` mints BOTH edges atomically under one `policy_id`
  (Away/Back — the existing trigger vocabulary sufficed, as codex argued). One
  standing policy per surface until field calibration; the same subject re-assenting
  re-points both edges; reverting EITHER edge disables the pair ("a policy is one
  consent"). Provenance `minted_from: thread:<id>`; the adoption lands as an
  observation. Gated by `allow_actuate` like every surface act.
- **A latent id collision fixed in passing:** rule ids derived from `now` alone — two
  edges minted in the same second shared an id. Salted by row count (the prediction
  store's own idiom).

### Checks run

- Kernel: pair-under-one-id + re-point on re-assent; the one-per-surface cap refuses
  a second subject; reverting either edge takes down the pair. Cycle, end-to-end on
  the real heed fixture: explicit yes mints the paired policy with thread provenance
  and narration; a neutral non-negative answer keeps the one-shot act and mints
  NOTHING. Full bar in rule-9 shape, tests twice.

### Next

- The live pilot: deploy the doors, author the lights fold manifest from the
  lighthouse's real rows (the six-in-five-hours cluster), fold — then the ONE
  collapsed lights thread asks once, Ian assents once, and the familiar manages the
  motorlights. Console Wondering drill-down remains a follow-up brick.

## 2026-08-15 — A theory predicts, or it wonders (companion:claude-bootstrap)

### What changed

- **T-128 (dialogue Q3, decided round 3).** `[pursued]` was forever: LLM threads
  carried no predictions, so the settlement and erosion machinery (T-113/T-114) never
  touched them. Now every draft either predicts or wonders — there is no third state.
- **Inquiry is a KIND, not a weaker status** (codex's amendment, absorbed whole): a
  prediction-less draft mints `kind: inquiry` — it has anchors and a question but no
  falsifiable proposition, so it cannot narrate, be pursued, acquire belief state, or
  ask (the question registry never hears it). It ages: `expires_at` = mint + 7 days;
  expiry is an append-retained transition to `expired`, never deletion; only new
  evidence or human attention renews (an answer revives it to open). The worldview
  carries `thread_kind` additively so consoles can build the Wondering drill-down.
- **Promotion, by projection.** The variant key includes the prediction shape — so a
  predicting restatement could never exactly match the proposition-less wondering it
  should promote (a real design flaw the test exposed mid-build). `mint` now matches
  the incomer's proposition-less PROJECTION against standing inquiries: on a hit the
  wondering becomes a theory — stops aging, the proposition enters its identity, the
  citations union — and the caller mints the predictions against the standing id. A
  proposition-less restatement strengthens without promoting.

### Checks run

- Kernel: promotion-by-projection pinned end-to-end (strengthen-no-promote, then
  promote with identity + expiry + citation assertions). Cycle: the full wondering
  lifecycle — prediction-less draft mints inquiry (kind, expiry, no question, not
  pursued, never mature), the sweep expires it, a human answer revives it. Full bar
  in rule-9 shape, tests twice.

### Next

- T-102 (P4): positive assent → the paired-edge ReactionPolicy; lights pilot. Then
  the fold manifests on the live doors, and the consoles' Wondering drill-down as a
  follow-up console brick (thread_kind already rides the worldview).

## 2026-08-15 — One thought, one thread (companion:claude-bootstrap)

### What changed

- **T-127 (dialogue Q1, decided round 3).** Nothing keyed a thread by content, so the
  same proposal minted as many times as the reasoner ran — and four independent
  minters each derived ids from `len()+1`, so one id could be issued twice under
  concurrency and `update_by_id` would then edit the wrong row.
- **`thread::mint` — the one way a thread is born.** All four minters (the muse, the
  needs muse, device adoption, mesh delegation) now route through one kernel
  chokepoint. Ids come from `store::next_seq` (the race closes as a side effect).
  Typed identity per codex's two-key design: `family_key` (subject + sorted anchor
  classes + target) names what the thought is ABOUT; `variant_key` (mechanism +
  declared acts + prediction shape) names the actual claim. Raw joined strings, not
  hashes — auditable. Question prose is never identity.
- **Strengthen / compete / new.** An exact variant match STRENGTHENS the standing
  thread — reinforced increments and the new citations UNION in, so the count derives
  from evidence; six-in-five-hours becomes one thread growing more sure of itself.
  Same family with a different declared act ("dim" vs "off" — different actions in
  the human's own declaration) mints a COMPETING sibling sharing the family key,
  never merged. Prose-only paths mint UNKEYED (empty keys never match), keeping
  their own dedup rather than falsely colliding.
- **The conservative fold.** `thread::fold` + `familiar theories fold <manifest.json>`:
  an explicit, human-reviewed manifest names survivor and members; members become
  append-retained tombstones (`superseded`, `superseded_by` pointing home, excluded
  from every human-facing view) and the survivor unions every citation and answer.
  Idempotent — a re-run manifest re-folds nothing. Never driven by a model or a
  fuzzy threshold. Thread rows gain `v` (schema version 1) in the same pass.

### Checks run

- New kernel tests: same-variant strengthens with citation union; different-act
  competes in one family; unkeyed mints never collide and take store-issued ids;
  fold leaves idempotent tombstones that point home and never surface. Full bar in
  rule-9 shape, tests twice.

### Next

- T-128: predictions mandatory + the Inquiry kind. Then the fold manifests for the
  live corpora (lighthouse ~304 threads, the lights six first) — authored from the
  real rows, reviewed, applied via the new CLI on each door: a fleet op, recorded in
  STATE when run.

## 2026-08-15 — The floor holds what the mind may claim (companion:claude-bootstrap)

### What changed

- **T-126 (dialogue 2026-08-15, Q2 + Q5, decided round 3; Ian: "Make it so").** The
  reasoner had no self-knowledge and no evidence discipline: ~304 lighthouse threads
  including six restatements of one lighting proposal in five hours, two inventing
  "AppleID login" (a mechanism the covenant forbids), each re-diagnosing designed B10
  visitor purges as defects, and verbatim-duplicate unanchored musings locally.
- **`kernel::system_facts` — the knowledge floor.** A typed, versioned registry
  (schema v1, rev 1): SF-1 lifecycle (familiar|purged is hygiene, not defect), SF-2
  membership (covenant/grants only; no external identity providers), SF-3 derived
  live from the human's actuators.json (declaration is the consent — never compiled).
  Every theorize prompt receives a rendering of the SAME registry the validator
  enforces post-parse; a draft whose typed claims contradict a fact refuses at mint
  with the fact cited, and the refusal lands as an observation ("familiar refused
  theory — SF-n"), on the record.
- **`TheoryDraft` — the strict admission contract** (codex's cross-cutting shape,
  round 2): anchors (chosen ONLY from a system-enumerated eligible set — invented ids
  refuse), typed mechanism, defect_claims, question/theory/direction, optional typed
  predictions. `deny_unknown_fields`. Deterministic admission order: anchors → facts
  → attentional dedup → dispose. `prediction::mint` gains its FIRST production caller:
  a draft's predictions mint with the thread (`minted_from: thread:<id>`), so
  settlement (T-113) and belief erosion (T-114) finally reach LLM theories.
- **Anchored cadence (Q5).** Eligibility rides the observation store's commit-order
  ids past a persisted cursor (`theorize_cursor.txt`) that advances ONLY on structural
  disposition (mint / strengthen / refusal) — provider failure keeps the batch
  retryable, and an empty eligible set makes NO consult at all: a stable world being
  quiet is correct. The needs muse and device-theory adoption get the floor too — as
  a LABELED lexical guard (prose-only paths; the v1 honesty gap is recorded in the
  dialogue), which kills both observed failure classes at adoption.
- Thread gains `anchors` + `facts_rev` (serde-default, additive); all four minters
  stamp them.

### Checks run

- New: 3 kernel tests (registry refusals, lexical guard against the live-observed
  prose classes, strict contract) + 5 cycle tests (defect-claim refusal with fact
  cited + cursor disposal; invented-anchor refusal; grounded mint carrying anchors,
  facts_rev, and its prediction; quiet-world-no-consult proven by the absent prompt
  file; device adoption refusing AppleID + purge-diagnosis while adopting the clean
  theory). Full bar in rule-9 shape (fmt, clippy --all-targets -D warnings, workspace
  tests), counts in the board entry.

### Next

- T-127 centralizes all four minters into kernel `thread::mint` (typed family/variant
  identity, strengthen/compete, the id race via store::next_seq, the conservative fold
  migration + the lights manifest). T-128 makes predictions mandatory and lands the
  Inquiry kind. The device reasoner still speaks prose — a console brick teaching it
  the draft contract retires the lexical guard.
## 2026-08-15 (small hours) — The Device screen can close two loops (companion:codex)

### What changed

- **T-101's console acts are typed and signed.** `POST /mesh/console-act` accepts only
  `disable_rule { rule_id }` and `name_device { name }`. The door verifies the local
  mesh gate, group certificate and revocation, node-id/key/certificate cross-binding,
  the signature over the exact request bytes, a five-minute timestamp window, a fresh
  per-node nonce, and full standing before any write. Unknown fields refuse. A name act
  carries no target id: a device may name only the certified key that signed it. Rule
  disabling only narrows authority and records which node did it; it never opens the
  actuation gate.
- **The existing worldview rule truth reaches Swift.** `Worldview.rules` now mirrors the
  Rust `RuleView`, and the new `ConsoleActClient` signs the same tagged envelope the
  transport parses. AppModel points acts at the door that supplied its worldview,
  reports the door's exact answer in working notes, and refreshes after success. Both
  the iOS and macOS WebKit bridges carry the two messages.
- **The Device screen shows and changes the facts.** Full members see every standing
  rule sentence with its enabled/disabled state and retained disabled reason; an enabled
  rule has a one-tap DISABLE act. The editable device-name field writes
  `DeviceRecord.name` through the signed door seam and reads the resulting roster label
  back. Both inputs are focus-protected from the polling renderer. Guest projections
  remain read-only and receive neither affordance (their worldview already strips rules).

### Why

The stores and read model already existed, but the console stopped at display: a human
could see neither the standing routines nor the device record they were meant to own.
This closes those two small loops without making the console an administrator, widening
a boundary, or adding a generic remote mutation channel.

### Checks run

- Rust seam regressions: full member disables a rule and a replay conflicts; full member
  names only its signing device; a validly certified guest cannot write (3 passed).
- `swift test` in FamiliarMesh: 15 passed, including exact tagged payload/signature,
  live POST response handling, name payload, and worldview rule decoding.
- The real sphere bundle was served over localhost and driven by a same-origin fixture:
  member Device view rendered the name field, enabled + disabled rule sentences and the
  retained reversal reason; clicks emitted exactly `ruleDisable/abcd1234` and
  `deviceName/Wildhorse`. The temporary fixture was removed.
- `xcodegen`; FamiliarAgent Release for the generic iOS Simulator and FamiliarMac Release
  for macOS both built successfully (`xcodebuild` exit 0). Focused mesh clippy passed in
  all-targets / deny-warnings shape.
- Final full workspace bar: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test --workspace` all exited 0;
  every unit, integration, scenario, and doc-test suite passed.

### Next

- DeviceRecord sync already carries the new name door-to-door on the existing dial; no
  eager second write path was added. A future HumanRecord brick may narrow rule-disable
  ownership from “full-standing member” to the routine's associated human once that
  canonical association exists; this seam stays typed enough to enforce it there.
## 2026-08-15 (small hours) — The console says what it is trying (companion:claude-bootstrap)

### What changed

- **T-120, from Ian's words:** a first join can take a minute or two, and the console
  answered it with silence resolving to a red exclamation. Join-in-progress and failure
  were one state.
- **AppModel grows `JoinProgress`** — a published stage machine (seekingDirectory →
  knocking → awaitingAdmission → admitted → joined | unreachable | declined) written at
  every transition of autoEnroll/requestJoin/runHandshake, with a live `tries` count
  mutated on every 2s admission poll (the old loop was silent for its whole ~5 minutes).
  Stages derive from protocol facts — directory results, knock outcomes, poll answers —
  never timers. Terminal states carry per-address causes in human words. The
  never-written `attemptLog` (declared, shipped, rendered — and empty since birth) now
  receives the per-host causes of every failed worldview walk.
- **The join screens branch on the stage.** Both enroll views (iPhone + Mac) show the
  current detail sentence, a "still asking — N checks" line while admission pends, and
  named per-address causes in the terminal state. The `autoEnrollTried` ordering bug —
  the flag flipped true before the directory fetch, so the screen read "couldn't reach"
  DURING the first, slowest call — is fixed by branching on the stage.
- **The sphere separates progress from failure.** A `#joinlive` pill (stage word +
  spinner + detail + live counts) shows while a join is in flight; the red badge is
  reserved for terminal failure and — on the Mac — finally carries a message
  (`sphereLinkDown()` had been called with no argument since the day it was written).
  The device JSON carries the stage as `join{…}`; the loading overlay gains words.

### Checks run

- Both schemes build (`xcodegen`; FamiliarAgent for iOS Simulator + FamiliarMac —
  BUILD SUCCEEDED both). Fixture-verified live against the real index.html served
  over localhost: the awaitingAdmission state renders the pill ("AWAITING ADMISSION ·
  the door heard the knock … · asked 23× over 46s") with the badge hidden; the
  unreachable state flips back to the badge wearing its diagnostic line, pill hidden.
  Rust untouched; bar rerun anyway in rule-9 shape (fmt, clippy `--all-targets
  -D warnings`, 33 suites ok, exits checked).

### Next

- "joined + peer count" reads implicitly from the roster the moment the worldview
  flows; holding the joined pill briefly with a peer count is cosmetic — revisit if
  Ian wants it.
- Daemon-side: `supervisor()` already writes one-line stage strings to mesh/status.txt
  that never leave the machine; surfacing them (e.g. alongside /mesh/hello) would give
  the enroll views door-side truth as well — a wire-contract addition, so it stops for
  Ian first. Candidate follow-up task if he wants the door's own voice in the ladder.

## 2026-08-14 — Beliefs earn their words (companion:codex)

### What changed

- **Prediction evidence becomes a belief, never a replacement for its record:** the new
  `kernel::belief` fold derives a versioned current view from append-only
  `PredictionResult` rows and retains every state change in an append-only transition
  log. The pure state machine moves `tentative → supported → doubtful → abandoned`
  only when new results arrive; abandoned is terminal, so changing a discarded claim
  means forming a new theory rather than silently rehabilitating the old one.
- **The bars differ on purpose:** support needs at least three favorable results and a
  two-result lead; accumulated contradiction moves support to doubt; recovery from doubt
  needs four favorable results and a three-result lead; abandonment needs four
  unfavorable results and a two-result lead. These distinct entry, erosion, recovery,
  and terminal bars prevent one sample from making a newborn theory sound certain or an
  old result from making it oscillate on every tick.
- **A person's word is typed evidence, not a sample:** a direct negative answer targeted
  at a theory moves it immediately to `doubtful`; the existing hard act-reversal seam
  moves its acted theory immediately to `abandoned`. Both carry replay-idempotent evidence
  ids, preserve the human line as a contradicting citation, and bypass only the
  statistical floor—no model participates in the truth path.
- **Only transitions speak:** after prediction settlement, cycle evaluates beliefs and
  may narrate one pending transition per tick. Abandonment outranks doubt, which outranks
  support; a six-hour per-theory cooldown prevents chatter. Bounded prose reports honest
  favorable/unfavorable counts and retains one supporting and one contradicting line when
  present. Ordinary first confirmation and unchanged belief state stay silent.

### Why

ADR-0040 Q5 decided that beliefs should become legible without becoming performative.
This brick lets the familiar change its mind from evidence, admit correction immediately,
and explain consequential transitions while leaving prediction results load-bearing and
append-only.

### Checks run

- `cargo test -p familiar-kernel` and `cargo test -p familiar-cycle` cover every
  statistical transition, the stronger recovery bar, terminal abandonment, append-only
  folding, typed/idempotent overrides, citations and counts, consequence ordering,
  per-theory cooldown, transition-only silence, and cycle integration.
- Full green bar on the T-114 tree: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test --workspace`.

### Next

- Belief drill-down can expose this current view and its transition fossil in a later,
  separately claimed console brick. Prediction authoring remains independent of this
  evidence fold; it should mint falsifiable claims without gaining any ability to edit
  their results or belief transitions.

## 2026-08-14 — Truth stays outside the recipe (companion:codex)

### What changed

- **Candidate-owned contract, fixture-owned truth:** `scenario::recipe_oracle` accepts a
  strict, bounded output contract naming the observation actor/action and the literal
  proven-tool inputs a Recipe v1 candidate claims to derive it from. A separate strict
  fixture supplies ordered tool-call transcripts and exact expected outcomes; replay
  hands the candidate only the recorded responses, never the answers.
- **Five required variants, four separate truth gates:** every fixture has one baseline
  plus unchanged, changed, null, and malformed cases. The external verdict retains
  accuracy, coverage, quietness, and discrimination as independent all-or-nothing gates
  rather than averaging them into a gameable score. Honest null values and typed expected
  errors count as coverage; wrong calls, arguments, order, or transcript consumption are
  execution failures.
- **Constitution before optimization:** eligibility is boundary-clean, then
  execution-clean, then a matching output contract and all four truth gates. Only eligible
  survivors compare fixture-owned usefulness and lower deterministic work cost. Invalid
  Recipe v1 capability declarations refuse before any recorded call. `EvidenceKind` can
  represent fixture replay only, so a live run cannot accidentally earn this verdict.
- **Quiet and hard to fake:** the recipe adapter models the changed-only persistence seam,
  suppressing an exact repeated observation. That makes a correct unchanged replay quiet,
  while a hard-coded answer goes silent on the changed/null variants and fails accuracy,
  coverage, and discrimination. The public scorer also catches a deliberately chatty
  adapter that re-emits an unchanged observation.
- **A distinct fixture namespace:** `scenarios/recipe-oracles/greenhouse-power.json` pins
  the first five-case oracle. Existing ADR-0010 world-fixture discovery explicitly excludes
  that namespace, preserving both strict schemas during recursive campaigns and validation.

### Why

ADR-0040 Q4 decided that a candidate cannot certify its own usefulness: novelty and clean
exit are not truth. This extends ADR-0036 from "the generated utility ran" to "its claim was
right across external counterfactuals," without putting a model or a live network in the
verdict loop.

### Checks run

- `cargo test -p familiar-scenario` — 42 library tests and 45 integration tests passed,
  including 8 new oracle regressions for strict contracts/fixtures, truthful replay,
  hard-coded fabrication, chatter, capability refusal, transcript mismatch, and
  lexicographic selection.
- Full green bar on the T-116 tree: `cargo fmt --check` (exit 0),
  `cargo clippy --all-targets -- -D warnings` (exit 0), and `cargo test --workspace`
  (exit 0; every crate, integration, scenario, and doc-test suite passed).

### Next

- A later cultivation brick may persist Recipe candidates and call this oracle before
  promotion. It must keep the output contract candidate-owned, keep fixture answers out of
  the execution world, and treat live observations only as post-deploy health evidence.

## 2026-08-15 (small hours) — One launchctl dialect (companion:claude-bootstrap)

### What changed

- **T-119.** `daemon.rs` was the repo's last speaker of `launchctl unload -w`/`load -w` —
  the pair the bootstrap script has forbidden since before LWCR ("registered-but-
  stalled"), kept alive here by an "older but functional API" comment. Functional was
  luck twice over: `install()` swapped the stable binary in place BEFORE unloading —
  rewriting the running daemon's executable underneath it — and every launchctl exit
  code was discarded, so a failed registration returned Ok.
- Now one dialect, shared with `tools/new-mac-bootstrap.sh` (0dbc525): `register_bracket()`
  builds the three argv lists — `bootout` of the gui service first, while the registered
  executable is still the old one; then the binary swap and a fresh plist; then
  `bootstrap` (which records the new binary's Lightweight Code Requirement — the macOS 27
  lesson) and `kickstart -k`, both exit-checked: an unregistered agent is now a failed
  install that says so. `uninstall()` boots out. The uid comes from `id -u` like every
  other process fact in the file — no unsafe, no new dependency.
- `the_bracket_reregisters_and_never_speaks_load_or_unload` pins the dialect, the order,
  and the domain scoping, and refuses any argv word containing "load".

### Checks run

- Green bar in rule-9 shape, twice: on the brick alone (31 suites ok) and again after
  absorbing main's T-115 recipe crate (fmt, clippy `--all-targets -D warnings`, 33
  suites ok) — each step's exit checked directly, logs kept. The new test seen passing
  in both runs. CLI surface unchanged: `daemon install/uninstall` signatures and their
  main.rs call sites untouched.

### Next

- `install_stable_binary()` can still overwrite the file while a MANUALLY `daemon
  start`ed (pidfile) instance runs it; the launchd path is now bracketed, and the
  pidfile path was always kill-first. Fine today; worth a thought if install ever
  learns to migrate a running manual daemon.
- `vm/create-famtalker01.sh` still unload/loads — one-shot VM bootstrap, left alone
  deliberately (T-119 notes).

## 2026-08-14 — A useful language with no ambient hands (companion:codex)

### What changed

- **Capability Recipe v1:** the new `familiar-recipe` crate interprets an authored,
  strictly typed sequence of `parse_json`, `parse_lines`, `select`, `map`, `filter`,
  `group`, `count`, `min`, `max`, `mean`, `compare`, and `format` steps. Inputs and
  results live in immutable named slots; emit templates produce one observation-shaped
  result with exact input/tool/argument lineage.
- **C2 is structural:** a recipe can invoke only an opaque `tool_id` through an injected
  `ProvenToolSource`. There is no interpreter API for paths, executables, processes,
  files, URLs, networks, clocks, environment, or randomness. The caller's proven-tool
  catalog remains responsible for health, review, argument schema, and every existing
  human-owned gate.
- **Requested is not granted:** Q8's mandatory v1 `caps` block exposes the authority
  review surface. Its distinct `process.proven_tools` ids must exactly equal the input
  ids before the first invocation; `clock`, `fs`, `env`, and `net` are literally `none`.
  The source still intersects that request with the human boundary, task scope, and host
  capability. Future authority requires a recipe-version bump — v1 never grows silently.
- **Bounded before useful:** serde rejects unknown fields at every level; the manifest,
  input count, step count, materialized bytes, and produced rows have declared limits
  beneath hard ceilings. Missing paths, malformed/non-UTF-8 input, incompatible types,
  empty aggregates, unknown tools, and non-finite arithmetic fail the whole evaluation —
  no partial observation or plausible substitute is emitted.
- **Design first:** `docs/reviews/2026-08-14-capability-recipe-design.md` records the
  language, operation types, accounting rules, authority seam, and deliberate exclusions.
  Python remains a scenario-lab authoring aid, never a live artifact; general-language
  WASI remains a later decision.

### Why

The reasoning dialogue rejected arbitrary live Python because resource limits are not an
authority boundary. Recipe v1 gives the familiar real program composition and repairable,
typed failures while beginning with a language whose reach can be inspected mechanically
and replayed exactly. Its deliberate expressiveness cost buys an honest Law III boundary.

### Checks run

- `cargo test -p familiar-recipe` — 21 pure tests covering strict parsing, ordered
  once-only tool invocation, exact lineage, every operation, grouped aggregates,
  deterministic replay, exact capability declarations, real-world JSON keys, and all
  declared/hard refusal bounds.
- `cargo clippy -p familiar-recipe --all-targets -- -D warnings` clean.
- Full green bar on current main + T-115: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test --workspace` (all crate,
  integration, scenario-lab, and doc-test suites passed).

### Next

- Integrate recipe persistence, author/repair prompting, and scheduling as a separate
  claimed brick. T-116 then tests recipe candidates against fixture-held truth; it must
  not let a recipe score its own output. Q8's clock/fs/env/net ladder is versioned future
  work recorded in ADR-0040, never an implicit v1 extension.

## 2026-08-14 (later night) — Reach asks the LAN what a numeric neighbour is called (companion:codex)

### What changed

- **Bounded reverse naming in `familiar-reach`:** when the paced reach survey still
  knows a neighbour only by its IP, it asks local DNS for the PTR and then mDNS directly
  (`dig @224.0.0.251 -p 5353 -x`) as a fallback. Each child is killed at the existing
  per-host reach timeout; missing `dig`, no answer, and malformed/numeric/localhost
  answers leave the numeric label honestly unchanged.
- **Names ride the observation that matters:** the cleaned host label now reaches both
  `DeviceReach.label` and `host can-reach device:<name>` while the IP remains in context.
  The frontier's existing face join can therefore associate the named sighting to a
  member's remembered LAN address and adopt it into `DeviceRecord.discovered_name`.
  The injected regression caught one easy-to-miss seam: the reach record used the new
  label while the observation still used the old numeric one.
- **Permission does not compose:** TCP reach remains behind `allow_network`; active
  reverse naming additionally requires `allow_network_discovery`. Existing DHCP/ARP
  names always outrank a PTR answer. Both `reach` and the periphery's `discover` command
  pass the separate gates; no network sweep returns to the metabolic tick.

### Why

Ian's standing direction is that names come from mDNS/tailnet/local DNS and router
configuration must never be required. Discovery naming already adopted names a door
overheard, but a door never asked what an otherwise numeric LAN neighbour called itself.
This closes that half of the loop on the same paced, human-gated survey that already
probes reach.

### Checks run

- `cargo test -p familiar-reach` — 7 passed, including injected resolver/probe tests for
  gated adoption, authoritative-name precedence, PTR cleanup, and exact observation
  output. `cargo check -p familiar-cli` clean.
- Green bar: `cargo fmt --check`; `cargo clippy -- -D warnings`; full `cargo test`
  passed on the clean rerun (reach 7, cycle 58, kernel 169, mesh 199, all scenario and
  doc-test suites green).
- The first full run overlapped the controller's worktree bar and saw one unrelated
  cycle parameter test's second tick report a duplicate revert. The test passed alone
  and the whole bar passed after the other job ended. Shared fixed-name temp directories
  are the likely collision; recorded as a Proposed coordination-hardening task rather
  than changed inside this brick.

### Next

- Let the next normal daemon build carry this to a door with both gates open, then
  inspect `familiar discover` output and the resulting device records. A LAN without
  `dig` simply retains numeric names; installing resolver tooling is an infra choice,
  never an implicit download by the familiar.

## 2026-08-14 (night) — FamTalker01 gets a reversible virtual home (companion:codex)

### What changed

- **Two declared surfaces, no new schema:** `vm/famtalker01/actuators.json` declares
  independent living-room and greenhouse virtual lights against ADR-0032's existing
  state contract. Both are closed over the same three observable restoring acts —
  `off`, `dim`, `bright` — so every change carries a real revert.
- **A small world behind the declaration:** `virtual_home.py` owns the two persistent
  states with locked, atomic writes, prints the existing motorlights-shaped state block,
  and refuses corrupt state rather than guessing. It has no listener and no undeclared
  act path. Its snapshot also exposes three observation points: each light plus their
  aggregate virtual power draw.
- **Changed-world feed:** a hardened systemd oneshot/timer posts that snapshot through
  the daemon's loopback `/local/observe` seam only when it changes. The result is ordinary
  `reports` evidence the familiar can analyze, without a periodic duplicate stream.
- **The human-owned install:** `vm/provision-virtual-home.sh` installs the helper,
  declaration, and feed on FamTalker01. Running it is the consent act: it preserves the
  existing boundary and opens `allow_execute` + `allow_actuate`, the two gates the current
  declared-act loop requires. A malformed existing boundary aborts instead of being
  replaced. `vm/README.md` carries the runbook and live acceptance.

### Why

Ian named FamTalker01 a virtual smart home for the familiar to explore, begin to control,
and learn when human attention would help. This is ADR-0032's promised second surface:
different plumbing from the BLE strip, but the same declaration, reversible hand,
reaction-reading, and narration discipline. Keeping it file-local and listener-free
makes the practice world legible and prevents its convenience from becoming a new
control seam.

### Checks run

- `python3 -m unittest vm/famtalker01/test_virtual_home.py` — 5 passed (persistence,
  state contract, corrupt-state refusal, closed revert maps, changed-only feed).
- `bash -n` + `shellcheck` on both shell scripts; declaration parsed with
  `python3 -m json.tool`; `git diff --check` clean.
- Green bar: `cargo fmt --check`; `cargo clippy -- -D warnings`; full `cargo test`
  (all workspace and doc-test suites passed).

### Next

- Fleet ops, not companion surgery: upgrade FamTalker01 through the infra lane, run the
  provisioner, verify changed snapshots land, then let one familiar-originated act render
  its existing `narrated` aside in Ian's console. The board carries that as a Proposed
  operation; T-104 remains open until the live acceptance is witnessed.

## 2026-08-14 (small hours) — An upgrade re-registers its binary

### What changed

- **The codesigning wall, encoded.** `tools/new-mac-bootstrap.sh --daemon` upgraded the
  pre-LWCR way: `cp` over the registered binary, `launchctl kickstart -k`. On macOS 27
  launchd records a Lightweight Code Requirement for the executable at bootstrap time,
  so the respawn of a swapped binary dies with `last exit reason = OS_REASON_CODESIGNING`
  — and ad-hoc re-signing does not help. Observed live on MacOnStick (2026-08-13); the
  cure was worked out by hand that night, and tonight the script learns it.
- **One path for install and upgrade:** `bootout` (no-op on a fresh Mac) → swap the
  binary while nothing runs it → `bootstrap` (records the new binary's requirement) →
  `kickstart -k`. The kickstart-only comment predated LWCR and is rewritten; the
  unload -w/load -w prohibition (registered-but-stalled) stands. The unified tail also
  heals the half-state where the plist exists but was never bootstrapped — the old
  script's bare kickstart aborted there under `set -e`.

### Checks run

- `bash -n` and `shellcheck` clean. The sequence itself is the one proven live on
  MacOnStick during last night's upgrades (bootout → bootstrap → kickstart after the
  swap → daemon spawned, exchange verified). No Rust in the diff; green bar run anyway
  (fmt, clippy `-D warnings`, full tests).

### Next

- `crates/cli/src/daemon.rs` `install()` still uses `unload -w`/`load -w` — it dodges
  the LWCR wall by accident (load re-registers at that moment), but it is the exact
  pattern the repo forbids for its registered-but-stalled hazard. Candidate brick:
  move it to bootout/bootstrap/kickstart. (`vm/create-famtalker01.sh` also
  unload/loads; one-shot VM bootstrap, low stakes.)
- Wildhorse's local upgrade helper (`~/familiar-upgrade-adr0038.sh`, outside the repo)
  predates this lesson — if wildhorse moves to macOS 27, it needs the same bracket.

## 2026-08-15 (small hours) — A theory says what the world will do (dialogue B1)

### What changed

- **`kernel::prediction`** — the engine the design dialogue specified (Q1/Q3/Q6, all
  DECIDED with codex): anchored, typed, mechanically-settled claims. An anchor match
  opens ONE pending instance (opening observation + explicit deadline, cooldown against
  chatty anchors, saturating never-opened arithmetic); consequents match by EVENT time
  inside [not_before, deadline]; certainty finalizes immediately (Confirmed /
  AbsentViolated), quiet waits out a GRACE (carried per prediction, defaulting from the
  co-owned `prediction_grace_secs`, sane-clamped 30..3600) so late-delivered in-window
  evidence amends a provisional miss — and a written `PredictionResult` is append-only,
  never rewritten. Unfalsifiable Absent claims (no bounded window) are refused at mint.
  `calibration(thread)` derives per-theory counts FROM results — the L4 inversion:
  scores derive from evidence, never overwrite it.
- Wired as tick step 2c: score against the freshly loaded log each tick (overlap-aware
  cursor so grace-late events are still seen). Belief-state transitions and narration
  deliberately NOT here — that is T-114's state machine, per the dialogue's division.

### Checks run

- Prediction tests: confirm-on-arrival + miss-only-after-grace (with the
  inside-grace-not-yet-a-miss negative), late-evidence amendment + final-immutability,
  Absent both ways + unfalsifiability refusal. Full bar: fmt, clippy --all-targets
  (exit-verified), 31 suites, release. Landed atop the reach test-lint fix after a
  same-fix race with origin resolved by reset-to-origin (redundant local commits
  dropped deliberately).

## 2026-08-14 (later night) — One vocabulary for every lens (dialogue Q7)

### What changed

- **`kernel::obs_class`** — the classing contract the design dialogue's Q7 demanded
  before B1: a versioned `ObsClass` (v1 = the head heuristic, named and homed:
  `actor|action|object-head`) and the typed `ObsMatch`/`FieldMatch` (any/exact/prefix —
  codex's round-2 design, adopted in round 3) that B1's predictions, the scenario
  oracles, and any future lens all share. Versions ride every persisted class and
  matcher, so a sharper future scheme never silently reinterprets history. A1's
  co-occurrence lens re-pointed at it; its private heuristic and pretty-printer are
  gone.
- The design dialogue closed all seven questions in three rounds — codex's
  prediction-anchor and recipe-interpreter designs adopted, my glob and python-tier
  priors dead, two arbitration calls made (grace carried on the prediction with a
  co-owned default; obs_class as this prerequisite brick). ADR-0040 drafts next from
  the DECIDED blocks.

### Checks run

- obs_class tests (behaviour-not-payload grouping, typed/versioned/literal matchers,
  unversioned-persisted reads as v1), loops suite 6/6 against the shared vocabulary,
  full workspace 31 suites ok, clippy --all-targets exit-verified, fmt.

## 2026-08-14 (night) — The second lens: relation, not just repetition

### What changed

- **`loops::detect_cooccurrence`** (reasoning brief A1, T-111): a pure second detector
  beside recurrence — event classes (actor + action + object-head, so `lights=dim` and
  `lights=bright` are one behaviour) that keep landing together within 10 minutes,
  judged against the rarer side's own rate (≥3 together, ≥0.5 of the rarer side),
  deterministic, capped at 12, the familiar's own events excluded (a mind must not
  theorize about its own reflection). Emits ordinary `Loop`s (`loop_type: "cooccur"`)
  into the existing candidate path — zero new plumbing downstream. The lighting
  pattern that only ever existed because an LLM guessed it from a prompt is now
  computable: the regression encodes exactly that case, plus the negatives (self-echo
  never pairs; far-apart never pairs; noise never pairs).
- Wired at tick step 2 beside `detect`. Landed parallel-safe ahead of the design
  dialogue's convergence (additive lens; codex's rounds may retune thresholds or score
  shape as follow-up bricks — invited explicitly in round 1).

### Checks run

- Loops tests 6/6 (two new: the lighting relation with noise rejection; self-echo +
  far-apart negatives; determinism of ids), full bar with clippy exit-verified,
  release build.

## 2026-08-14 (evening) — Two AIs, one codebase: the coordination directory

### What changed

- **`coordination/`** — the shared memory between the AIs working this codebase (Ian:
  a common, always-updated file about tasks, work done, and system changes, keeping
  the controlling AI and a companion AI in sync; the companion is a full coding
  partner — coding, planning, and design hand off to it). Files over chat, because
  files survive sessions: `README.md` (the rules — claim-before-work in a pushed
  commit, scope collision checks, controller/companion roles, held-operations
  discipline, messages-are-ephemeral-records-are-real), `BOARD.md` (the task board in
  a fixed entry format, seeded with the real queue: Build 85 console batch,
  theory-affirmation rule minting, reach-side reverse name lookup, FamTalker01's
  virtual-smart-home declaration, HumanRecord, geo source, the ADR-0039 migration),
  `STATE.md` (fleet truth, the held consolidated pass, what waits on Ian, his standing
  directions recorded), and `COMPANION_PROMPT.md` (the self-contained brief Ian hands
  a new companion).
- The rules encode this week's paid-for lessons: the shared-checkout worktree
  discipline, push-race handling without force, exit-code-honest bars, held operations
  as a ledger with explicit triggers, and the betty/mol never-touch line.

### Checks run

- Docs-only brick: markdown proofread, entry format self-consistent with its own
  README, board/state cross-checked against the live fleet and the peer session's
  held-pass plan.

## 2026-08-14 (late afternoon) — Discovery names the fleet, and the familiar says why

### What changed

- **Autodiscovery naming** (Ian: Bonjour/mDNS/local-DNS should gather device names;
  router config must never be required — and he watched "codex" stand on the map beside
  the very iPad it names). Three mechanisms, one association:
  `DeviceRecord.discovered_name` — the name the device's own human gave it, learned from
  the network (mDNS host, tailnet host), cleaned of `.local`-suffixes, generic hardware
  words refused, outranked only by an explicitly given name. `note_network` accumulates
  every address a member shows a door (LAN, tailnet, NAT faces; deduped, hourly-throttled,
  capped) onto its device record — so the **frontier join can associate a discovery at ANY
  face back to the member**: the ghost row drops AND its name is adopted (the old dedup
  discarded the name while dropping the row — the exact loss). Tailnet hostnames adopt on
  sight at the member ladder. SystemName ladder now: given > discovered > live-tailnet >
  brief label > mask. Discovered names replicate on device-sync like every device fact.
- **The familiar narrates its changes** (Ian: "the familiar needs to talk about what it
  is doing and why with the humans when changes are being made"). Both act paths — a
  standing rule firing and a pursued-thread act — now speak a `narrated` aside into the
  dialog at change time, reason and undo named in the same breath: *"I set lights to dim
  — ian went away, and the standing rule you confirmed asks for it. Undo it by hand and
  the rule stands down."*
- **FamTalker01's purpose, recorded**: a VIRTUAL SMART HOME — virtual interfaces and
  observation points for the familiar to explore, begin to control, and report on when
  human intervention would help. It is the "second surface" ADR-0032's follow-on asked
  for: its virtual controls should be declared in its `actuators.json` and its
  observation points fed as observations — the safe practice ground for the reaction
  loop. Design/ops thread, queued.

### Checks run

- Device tests (name cleaning/guarding, network accumulation + throttle), the frontier
  adoption regression (named discovery at a member's remembered face → ghost dropped,
  name adopted; bare-ip sightings adopt nothing), full workspace bar, release build.

### Next

- Reach-side reverse lookup (mDNS PTR / local-DNS `dig -x`) so a door can RESOLVE names
  itself instead of waiting to overhear one — gated by `network_discovery`, riding the
  paced reach sweep. With it, MacOnStick needs no ASUS DNS entry ever.
- FamTalker01 actuator declaration + observation-point feed (with the narration above,
  the familiar will say what it changes there and why).

## 2026-08-14 (afternoon) — A theory tells its whole life, and three console honesty fixes

### What changed

- **The theory drill-down** (Ian: "a stacked, hierarchical view of the progress from
  beginning to success or abandonment"). `TheoryView` now carries the lineage the stores
  always held but never told: `seeded_by` (the loop's observation count), `work[]` (each
  candidate generation — hypothesis, status, mutation reason, nested trial verdicts, the
  tool it left behind, matched by artifact path), and `acts[]` (dated actuated/demoted/
  adjusted lines from the observation record, keyed by the thread tag). All additive
  fields; capped (6 generations × 4 trials, 8 acts) so a read stays a read. The sphere's
  theory card opens into the stack: **BORN → the question and answers → THE WORK →
  IN THE WORLD → NOW** — how the familiar is progressively changing, per theory, on one
  screen. Open state survives the poll like every accordion here.
- **The dialog threads answers to their questions.** An answer spoken on the theories
  screen landed in the dialog as a bare "yes" — connected in the record, disconnected in
  the rendering. Thread-ref bubbles now carry their theory's question as a quoted line
  above the answer.
- **Cluster bubbles zoom.** The counted circles the globe collapses distant nodes into
  were dead ends (`#clusters` is pointer-events:none; the bubbles never opted back in) —
  the visible face of "can't zoom into hosts." A tap now rotates the cluster centroid
  under the camera and closes ~40% of the distance per step (the dive's own aim math,
  generalized to `aimQFor`); clusters separate into callouts, whose dives already work.
- **Positions tell their provenance.** A fresh-but-wrong IP-geolocated position rendered
  exactly like a GPS fix (Wildhorse's pin, 72h after the machine moved — the geo DB's
  word against reality). Every surface now renders non-`geo_device` positions with a
  leading ≈ (and "(estimated)" on the node screen). The source tag riding the brief is
  deferred deliberately — a new Capability field is the `build_version` signed-body trap
  until every door re-serializes it; it joins the held door deploy.

### Checks run

- Fixture-driven browser verification: all drill stages render (both generations, trial
  verdicts colored by result, tool chip, seed count), the dialog shows the quoted
  question above the "yes", Wildhorse renders "≈ 44.900°", zero console errors. Green
  bar + FamiliarMac build + release.

### Next

- Wildhorse's actual coordinates: ops — write real coords into its `data/mesh/geo.json`
  (priority-1 source) or zero it to honest-unlocated; needs Ian's word or the peer's
  hands. A household-anchor inheritance (a daemon adopting cohabiting member GPS) is a
  design question for an ADR, not a quick brick.
- geo source over the wire + console rules list + device-name field: Build 85 scope.

## 2026-08-14 (midday) — A confirmed intent becomes a standing rule (ADR-0039 build #2)

### What changed

- **`reaction_rule.rs`** (kernel): the object ADR-0032 deferred — `ReactionRule
  { subject, trigger (away|back), surface, act, minted_from, enabled }`, one per
  (subject, trigger, surface), re-minting re-points and re-enables. `due()` fires on
  presence TRANSITIONS against a remembered per-subject state (first sight seeds
  silently, launch-silent like every edge here); the store keeps the rules and that
  memory in one small familiar-owned JSON.
- **`tend_rules`** in the 8·3 loop: a firing walks the ENTIRE ADR-0032 discipline —
  `allow_actuate` gate, guard, withdrawn-check, read-and-skip-if-agreed, declared act
  tools, revert map — with the rule as the pursuit (`PendingAct.thread_id = "rule:<id>"`),
  so the existing poll/hand machinery routes reactions back. **A reverted firing
  disables the RULE, not just the act** ("a standing rule the human undid is a standing
  mistake"), by hand at the surface or via `familiar actuate`; both say so in plain
  words. Quiet-is-consent closes rule acts positively as before (no trial minted — a
  rule is not a candidate).
- **`familiar rules`** CLI: `list` (id, on/OFF, the sentence "away → lights dim (for
  ian)"), `add <subject> <away|back> <surface> <act>` (the mint IS the consent moment),
  `on/off <id>`. The worldview exposes the same sentences (`rules: [RuleView]`,
  additive) for the Build-85 console list — and **the guest projection strips them**:
  a rule names its subject's comings and goings, the most private sentence in the
  house (regression pinned beside the federation-projection test).
- The lighting loop that kept re-asking now closes:
  `familiar rules add ian away lights dim` + `familiar rules add ian back lights bright`
  is the whole automation, revocable by one hand-motion at the lamp.

### Checks run

- Kernel rule tests (transition-only firing, no-refire, revert-disables + re-mint
  re-enables), the cycle end-to-end (mint → departure fires through gate/guard/tools →
  hand revert at the surface disables the rule and closes the window), guest-projection
  regression, full workspace bar, release build.

### Next

- Theory-affirmation minting (an affirmed lighting theory mints the rule instead of the
  CLI) and the console's rules list — Build 85 scope with the device-name field.
- Habit-threshold PROPOSALS (a strong habit asks; the answer mints) — after field time.

## 2026-08-14 (morning, cont.) — The device gets its own record (ADR-0039 build #1)

### What changed

- **`device.rs`**: the DeviceRecord store — `mesh/devices/<device_id>.json`, keyed by the
  durable device id. Machine facts (kind, os, os_version, arch, capabilities,
  observation interfaces, networks with sighting ages), human associations as
  time-bounded edges (union on merge; a closed edge stays closed), and **`name` — the
  SystemName, deliberate and human-given, never invented**. Facts merge latest-wins by
  `updated_at`; no floats ride the record, by design (the one-longitude lesson).
- **Replication**: `GET /mesh/devices` + `POST /mesh/device-sync` — record-sync's twin
  (same proof envelope, same 48h window/cap, riding the same outbound dial via
  `sync_devices_with`). Deliberately its OWN endpoints rather than fields on the signed
  record-sync body: a door built before this simply 404s, instead of failing a signature
  over fields its parser drops (the `build_version` lesson, generalized).
- **`mesh device name <node_id> <name>`** (+ `device show`): the operator's naming act,
  resolving prefixes through the membership records (typos refuse — the doppelgänger
  guard covers devices too). The console's Device-screen field lands with Build 85 and
  writes the same store.
- **The SystemName ladder** in the roster: a given device name outranks the tailnet
  hostname outranks the brief label — for peers and for the self row both; a rotated key
  still finds its name through the membership record. `refresh_self` keeps this node's
  own machine facts current every outbox build, never touching the name.

### Checks run

- Green bar (fmt, clippy `-D warnings`, full workspace), device module tests (naming via
  prefix + refusal, rename latest-wins both absorb directions, association union with
  closed-edge persistence, sync round-trip + foreign-group refusal), release build.

### Next

- Doors need this merge before names travel (device-sync 404s until then — queued with
  the peer session's next deploy cycle). MacOnStick and Wildhorse can be named from any
  upgraded door; the phones name themselves at Build 85, or an operator names them once
  their ids are confirmed against the lighthouse's peer table.
- ReactionRules (ADR-0039 build #2) is next.

## 2026-08-14 (morning) — The machine you sit at learns its own name

### What changed

- The SELF row now reads its own record's effective establishment before falling back to
  the local identity file — every peer row already did (ADR-0027); the one machine blind
  to its own establishment was the one you were sitting at. With the re-danced record
  (establishment "ian") and the label rename, this console's own row completes the
  sentence: `MacOnStick : Mac : Ian`.

### Checks run

- Green bar (fmt, clippy `-D warnings`, 31 suites), release build; live worldview row
  verified on MacOnStick after deploy.

## 2026-08-14 (dawn) — The sentence gets its nouns, and the two-record direction lands

### What changed

- **ADR-0039 written and accepted** (Ian's direction, verbatim in the record): humans
  and devices become separate first-class records — HumanRecord (name, devices,
  relationships/lineage, preferences, habits, routines, workflows) and DeviceRecord
  (name, kind, os/arch, capabilities, observation interfaces, networks, human
  associations current+past) — related only through time-bounded association edges,
  with **the roster demoted to a view** over them. ADR-0032's deferred pieces are
  scheduled inside it: persistent ReactionRules (a confirmed intent becomes a standing,
  visible, revocable automation — the lighting theory stops re-asking) and
  habit-threshold proposals. Build order in the ADR; DeviceRecord + the device-name
  field + ReactionRules head the list.
- **`Member.arch` rides the worldview** (from the peer's brief, already persisted in
  peers.json; self reports its own): the console's type word can finally tell Macs
  apart — `typeWord` reads "MacIntel" for macOS on x86_64. With the label ops below,
  Ian's sentences render live: `MacOnStick : Mac : Ian`, `Wildhorse : MacIntel : Ian`,
  `Codex : iPad : Ian` (fixture-verified all three).
- **Ops:** this Mac's node label renamed "Mac.river.io" → "MacOnStick" (node.json —
  cosmetic slot, rides briefs). The establishment re-dance to the fleet convention
  (label = machine name, establishment = human: `disestablish → grant → name ian`) runs
  on the lighthouse; under ADR-0039 the machine name migrates to DeviceRecord.name
  properly.

### Checks run

- Green bar (fmt, clippy `-D warnings`, full workspace tests), FamiliarMac build,
  release build; sphere fixture-driven in a browser for all three sentences.

### Next

- The phones' SystemNames ("Codex", "Aphelion") wait on the device-name field
  (ADR-0039 build order #1) — iOS reports the generic model as its label and the
  lighthouse cannot see tailnet hostnames. Until then their rows read type-as-name.

## 2026-08-14 (small hours, cont.) — The roster reads as a sentence, and the mesh shows its body

### What changed

- **"SystemName : SystemType : ServedUser."** Ian's format, verbatim ("Wildhorse : Mac :
  Ian"). The handle-first roster shipped in 82 met the real fleet — where most handles are
  one human — and read as a wall of *Ian*. Now the DEVICE's own name leads every row
  (masked dim-italic when it is only the id), the hardware kind follows as a quiet
  segment, and the established human closes the sentence wearing the green of a known
  identity. Applied to the roster cards, nested console rows, network rows, globe
  callouts, arrivals, and the device screen's own header. The node id stays small print.
- **The globe draws the mesh even when this node has no fix.** `rebuildArcs` returned
  early unless SELF was located, so a Mac without GPS consent drew a bare globe — no
  spokes, and no peer-to-peer body either (seen live on MacOnStick). Spokes, frontier
  branches and sibling arcs still require a located self (they radiate from it); the
  peer-to-peer edges no longer do. Verified against a fixture with an unlocated self:
  the lightning runs Aphelion → Wildhorse → Codex with nothing at the center.

### Checks run

- Sphere driven against fixtures in a browser: the format renders on every surface
  (`MacOnStick : Mac : Ian`, `unnamed iPad : iPad : Betty` masked with small id), and the
  arc body draws with self unlocated. xcodegen + both console builds; cargo untouched by
  this brick but the full bar re-run regardless.

### Next

- **SystemName for the phones is a data gap, not a display one**: iOS reports the generic
  model ("iPhone") as its label — Apple hides the user-assigned device name behind the
  `user-assigned-device-name` entitlement (requestable for team 8GHXL328AR), and the mesh
  has no per-device name field of its own (the per-device records question, again). Until
  one of those lands, an iPhone's row reads "iPhone : iPhone : Ian".
- This Mac's daemon label is "Mac.river.io" (minted from the hostname). Renaming the node
  label to MacOnStick is a one-line node.json edit + daemon restart, Ian's call on the
  spelling.
- **Wildhorse's pin is a fossil asserted fresh** (Ian, live: locked to a position it left
  72+ hours ago). Its `mesh/geo.json` is `self_geo`'s priority-1 source, written once and
  never expiring, and `location_at` is stamped at RECEIPT — so a stale coordinate reads
  as a fresh fix on every console. Operational cure: inspect/refresh that file on
  wildhorse. Design note for the next wire brick: the fix's own timestamp should travel
  with the coordinates, so a console can show honest age instead of trusting the
  messenger's clock.

## 2026-08-14 (small hours) — An act lands on a record that exists, or not at all

### What changed

- **The doppelgänger.** With the float fix deployed, record-sync came back to life and
  delivered the truth about the repair dance: the record wearing *MacOnStick* has
  `device_id "3d68a068"` — the 8-character DISPLAY prefix — no pubkey, no attestation, no
  keys. The dance had been run with the id the consoles display, `record_standing_grant`'s
  blind upsert minted a fresh record for the unknown string, and `mesh name` then named
  the ghost. The real node (`3d68a0689bc32771`) was never granted and never named. Every
  card that read *MacOnStick* was reading a keyless phantom.
- **Resolution, and refusal.** `record::resolve_node_id`: an exact device_id/key match
  wins; otherwise a prefix naming exactly one record resolves — the door now accepts the
  8-character form it shows on every surface — and ambiguous or unknown references are
  errors. The live entrances (CLI `standing grant`, `/mesh/standing`, corrections via
  `apply_correction`, `mesh name`) all resolve before acting and never mint. The migration
  fold alone still mints, because minting records from the roll is its stated purpose.
- Pinned by `a_membership_act_lands_on_a_record_that_exists_or_not_at_all`: prefix grants
  and names land on the real record with no new file; unknown ids refuse; a second record
  sharing the prefix turns resolution into a named ambiguity error.

### Checks run

- Green bar (fmt, clippy `-D warnings`, full tests — mesh lib 194, workspace green),
  release build. Live: the ghost record inspected on this door (`no pubkey, no
  attestation`), and the float fix's convergence confirmed by its arrival.

### Next

- **Operational cleanup, on the lighthouse with this build:** `mesh sever 3d68a068
  --reason "doppelgänger minted by a prefix dance"` (exact id hits the ghost; the
  tombstone travels), then the real dance — `mesh disestablish 3d68a0689bc32771` →
  `standing grant 3d68a0689bc32771 --note "MacOnStick — the M3 Air"` → `mesh name
  3d68a0689bc32771 MacOnStick`. Under the clamp and the resolution guard this lands on
  the real node and replicates. Then the roll's stale short-id entry wants
  `standing revoke 3d68a068` so the legacy file stops disagreeing.

### Addendum (same night) — the purge had eaten the real record

Running the cleanup proved the guard and found the last gap at once: the sever landed on
the ghost, and the re-dance refused — the real node had NO record anywhere. Its guest
record was purged hours earlier (B10 forgets un-established guests, by design), briefs
don't restore records (only worldview reads and status heartbeats do), and the ghost's
`class: migration · artifact: standing-roll` establishment showed the fold had minted it
from the roll's short-id entry — the original sin predating tonight's dance. So the door
held verifiable identity for the real node (its enrollment grant, key and all) and no way
for an operator to act on it. Amendment: `resolve_node_id` falls back to the enroll
store's grants — **exact full id only, never a prefix** — and the grant's dual-write now
stamps the record with the grant's pubkey, so a restored record can anchor vouchers and
stand alone at sibling doors (the doppelgänger's fatal lack, made structurally
impossible). The runbook simplifies: sever the ghost, then `standing grant
3d68a0689bc32771` + `mesh name 3d68a0689bc32771 MacOnStick` — no disestablish needed on a
record being restored from enrollment evidence.

## 2026-08-13 (later night) — One longitude, one ULP, eight hours off the mesh

### What changed

- **The hunt.** After the record repair, MacOnStick's daemon still wouldn't converge: the
  lighthouse showed this node entirely absent from its peer table since ~17:35 while raw
  TLS reached the door in 200 ms. Hand-posting the daemon's own outbox brief got the
  truth the logs never carried: **HTTP 403 — node signature did not verify** — from the
  lighthouse, and then from this daemon's own door against its own outbox. Offline
  bisection (sign-time byte dump vs re-parsed canonical bytes) found the whole story in
  one byte: the brief signed `"lon":-93.39668839065929` and every verifier re-parsed it
  1 ULP off, re-serializing `…28`. serde_json's default float parse is fast, not exact;
  `verify_brief` re-serializes the parsed body; the signature could never verify. The
  coordinates had sat in geo.json since 11:45 — the outage began the moment they started
  riding the brief.
- **Three fixes, one lesson each.**
  1. `serde_json` now carries **`float_roundtrip`** workspace-wide: a float parses to the
     exact value its digits name, so signing_bytes(parse(wire)) is the signed bytes again.
  2. Brief coordinates are **quantized to six decimals** (~10 cm) at build: short literals
     re-parse exactly under every parser the fleet has ever shipped, so an upgraded sender
     verifies at doors that haven't upgraded yet — deploy order stops mattering.
  3. `exchange_with` **no longer swallows a refusal as success**: a 4xx/5xx reply logs the
     door's own words once per round and counts the peer unreached. The bug was one ULP;
     the outage was the silence.
- Pinned by `a_brief_carrying_hostile_floats_still_verifies_after_the_wire` — the exact
  live coordinates, through both the compact wire and the pretty outbox file.

### Checks run

- Green bar (fmt, clippy `-D warnings`, full workspace tests — 31 suites; the new float
  regression fails without the feature and passes with it); release build. Live: the
  daemon's real outbox reproduced the 403 against its own door before the fix; the fixed
  binary's re-admission to the mesh is the deploy's acceptance test.

### Next

- Deploy the fixed daemon to the lighthouse and wildhorse (senders there may carry their
  own hostile floats the moment their geo moves; the receive-side parse fix is what makes
  the fleet safe for good).
- The deeper design note, for a quieter day: `verify_brief` verifies a **re-serialization**
  rather than the received bytes. Signing the raw wire bytes (as every other mesh
  endpoint already does via `X-Familiar-Sig`) would retire this entire class. Wire-format
  change — wants its own ADR.

## 2026-08-13 (night) — A release spends what it means to spend, and nothing else

### What changed

- **The live sighting.** Minutes after Build 81, MacOnStick's welcome card read
  *MacOnStick · VISITOR — a visitor is looking around*: an established name in the state
  copy of a guest. The trail led to the lighthouse's rename dance (disestablish → grant →
  name, scripted inside one wall-clock second) and from there to three distinct defects in
  the record layer, each now fixed and pinned by a test.
- **1 · The same-second boundary.** Both merge keep-filters and the derive boundary treat a
  fact at the same second as a Disestablish as SPENT (the release wins ties — leaving must
  always work). Right rule, but the door's own mint paths could land a deliberate re-grant
  in the release's second, spending it at birth. `unspent_at` now clamps every establishment
  and admission the door mints to strictly after the latest release; the tie rule survives
  for true races (`a_true_same_second_tie_is_spent_and_names_nobody`), and the scripted
  dance derives Member everywhere, replication included
  (`a_same_second_rename_dance_stays_member_everywhere`).
- **2 · The admission a release forgot.** A Disestablish cleared the establishment but left
  the AdmissionFact — so a release → re-establish on a live record kept the pre-release
  admission, which merge's keep-filter then spent on the first sync: member at its own
  door, guest one gossip round later. A release now spends BOTH member facts locally,
  exactly as merge spends them on every replica; re-establishing mints a fresh admission
  through the rules engine (the attestation, filter 1, is retained).
- **3 · Spent names named things.** `identity.established` kept the handle on a spent
  establishment, and four consumers read it raw: the roster's `human`, the welcome's
  arrival handle, the game seat key, and the game turn-actor resolution — the last two
  meaning a RELEASED device could still hold a seat and act under its old handle.
  One canonical gate now — `record::effective_establishment` — mirrored by `derive_state`
  and used by every consumer that names a device; `mesh name` refuses to write a name onto
  a spent establishment ("re-establish first, then name"); the sphere's arrival card
  titles on the handle only for members, so even an old door can't make it contradict
  itself. The voucher/E1/E2 anchor list was already state-gated; verified, unchanged.

### Checks run

- `cargo fmt`, `clippy -D warnings`, full `cargo test` (mesh lib 192 green including the
  two new regressions; workspace green); `xcodegen`; both consoles rebuilt (the sphere is
  a bundled resource). The REAL local worldview (`/local/worldview` off the running
  daemon) rendered through the new sphere: handles lead, the Wildhorse console nests under
  Wildhorse, MacOnStick's console stands alone honestly.

### Next

- **Repair the damaged record** once fixed binaries stand on the lighthouse: re-run the
  dance for 3d68a068 (disestablish → grant → name) — under the clamp it lands strictly
  after its release and replicates as Member "MacOnStick" everywhere. The consoles converge
  within a sync round.
- **Fleet naming is data, not code:** wildhorse's machine record is established as "ian",
  so the roster leads that card with *Ian · Wildhorse*. If the machine name should lead,
  the same (now-safe) dance renames it — Ian's call per record.
- A released device's record (both member facts spent, attestation retained) is no longer
  purge-exempt via a lingering establishment; restoration-from-cert still covers a
  returning device. Watch it in the field.

## 2026-08-13 (evening) — Names lead, a console stays home, and the cloud gate gets its switch

### What changed

- **The established handle leads everywhere a device is shown.** ADR-0027 already ruled
  it — *the roster's name for a device is its record's established handle, never a cached
  brief's word* — but the sphere led with the label and wore the handle as a green suffix,
  and four surfaces (claims, arrivals, the Mac's own device header, correction notes)
  led with the bare node id. Now `nodeName()` applies one rule everywhere: handle (green,
  the mark of a known identity), else the device's own label, else a mask. **A node id is
  an address, not a name** — it never leads; it is demoted to small print on the cards and
  a NODE ID row on the node screen. The un-named wear what the mesh honestly knows —
  *unnamed Mac*, *unnamed iPhone* (OS word from the member row or the arrival's build
  string) — dim and italic, so a placeholder can never pass for a name. `idLed()` spots
  labels that are only the id in a haircut (the doors' own `node_id[..8]` fallback), by
  exact prefix match rather than a hex heuristic. Swift's `displayName(for:)` gives the
  door notes the same manners ("✓ Wildhorse — sever", "welcome an unnamed device
  (7cc41b02)"). Handles stay lowercase slugs on the wire; `cap()` dresses them for
  display (wildhorse → Wildhorse) — names are important.
- **A console files under the machine it runs on — or stands alone, honestly.** From the
  lighthouse's vantage every machine at home wears the same public address, and the
  structural pass in `attach_consoles` took that as evidence: MacOnStick's console filed
  under wildhorse's daemon (seen live today, both Mac consoles nested under the one card).
  A shared address now identifies a machine only when it is **private** to the household's
  networks (LAN, loopback, tailnet — the same `is_gossipable_addr` judgement the host list
  uses); a public address is a household, not a machine, and yields no link. The label-stem
  pass is untouched, so *Wildhorse console* still files under *wildhorse* from any door.
  Belt and braces in the sphere: the SELF row — the very device rendering the roster —
  never nests under another machine's card, whatever a stale door still claims
  (read-loyalty keeps old doors serving for a release).
- **The device screen completes ADR-0038's named next brick: the `consent.pcc` switch.**
  One tile — *private cloud* — beside the other consents, on every console (the sphere
  bundle is shared by FamiliarMac, iPhone and iPad; both bridges already pass any consent
  key through to `setConsent`). The hover title says the doctrine in full: a consult
  leaves this device for Private Cloud Compute only when the hub has also cleared the
  cloud — permission does not compose. No sensing to start or stop: ConsultRunner reads
  the flag at each consult. The hub's `allow_llm_cloud` stays file-only, by boundary
  doctrine — no UI for it, ever. The watch renders no consent screen of its own; it is
  established through its phone and inherits the phone's choice.

### Checks run

- Green bar: `cargo fmt --check`, `clippy -D warnings`, full `cargo test` (31 suites,
  including the new `a_console_behind_nat_never_files_under_another_machine` and every
  pre-existing attach case unchanged); `xcodegen`; **both consoles BUILD SUCCEEDED**
  (FamiliarMac / macOS, FamiliarAgent / iOS Simulator) on the Build-80-stamped project.
- Sphere driven live in a browser against fixture worldviews: handles lead green with the
  hardware word beside them (Aphelion · Ian's iPhone), masks render dim-italic with the
  id in small print (unnamed Mac · 9f21c3aa), the claims/arrivals cards never title on
  hex, a SELF row with a stale `attached_to` stands top-level while the legitimate nested
  console stays nested, and the *private cloud* tile renders beside *reason*.

### Next

- **Name the fleet.** The masks show exactly where establishment is missing — this Mac
  (3d68a068) reads *unnamed Mac* until the lighthouse door runs `standing grant` + `mesh
  name`. The naming acts are one-liners; the console now makes the gap visible instead
  of spelling it in hex.
- **Per-Mac console identity is a records question, deliberately not answered here.** The
  lighthouse's legacy roll still carries one record labelled "wildhorse - the macOS
  console app (mac:*)" from when wildhorse was the only Mac, and `dedup_devices` still
  groups by `mac:<human>` — two real Macs serving the same human are two machines, not a
  reinstall lineage. Both want the ADR-0026/0027 treatment (a record per physical
  console, the legacy record retired) rather than a display patch.
- PCC live-fire prerequisites unchanged from the entry below; the new tile removes the
  "no UI" item from that list.

## 2026-08-13 (later still) — Where a thought may travel becomes the human's setting (ADR-0038)

### What changed

- **The cloud consent gate, end to end.** `Boundary.allow_llm_cloud` (fail-closed,
  scope-preserved) with the guard's `LlmCloud` kind — subordinate to `allow_llm`, never a
  bypass. The seam exports `FAMILIAR_ALLOW_LLM_CLOUD` unconditionally each consult; the
  adapter captures it ahead of key.env (the `_CALLER_PROVIDER` discipline) and filters
  `CLOUD_PROVIDERS` from the chain when closed, exiting 1 with a named message if that
  empties a consult. `ConsultPrompt.cloud_ok` carries the decision through the mesh — the
  load-bearing test pins `accept_relay` preserving it across the broker's re-serialization.
- **Apple Intelligence on the bench.** `apple_local` / `apple_pcc` via the macOS 27 `fm`
  CLI behind one helper: guided `--schema` generation for the script/theory kinds,
  `fm available` preflight mapping not-ready states to the DeviceAsleep retry path.
  Live finding: `fm respond --model pcc` is context-gated by Apple to real Terminal
  sessions (recorded in key.env.example) — the chain marks it and rolls on.
- **The device stacks its consent.** ConsultRunner chooses
  `LanguageModelSession(model: PrivateCloudComputeLanguageModel())` only under
  cloud_ok ∧ `consent.pcc` (default off) ∧ OS 27 ∧ `.available`; oracle status gains
  "+pcc". Deployment floors untouched. The sphere's visible toggle for `consent.pcc`
  is deliberately its own next brick.

### Checks run

- Green bar per brick (fmt, clippy `-D warnings`, `cargo test --all`, kernel `unsafe`
  grep); live adapter matrix (closed filters hosted + probe; unset fail-closed; open
  reaches gemini; ollama survives a closed gate); both consoles BUILD SUCCEEDED against
  the macOS 27 SDK with the PCC branch.

### Next

- Flip `"allow_llm_cloud": true` on nodes that want hosted providers (the hub; done at
  this merge's deploy). PCC live-fire needs: the phones on the Brick-8 build (TestFlight),
  `consent.pcc` on, and the PCC entitlement granted to the app ids. Then the ai-eval rerun
  (on-device vs PCC vs the 2026-07-28 baseline) is the quality gate. The M3 Air's refusal
  to enable Apple Intelligence is a standalone open issue — `fm` says modelNotReady; no
  Apple Account is signed in on that machine.

## 2026-08-13 (later) — Test ports move out of the runner's reach

### What changed

- **CI stayed red after the reach fix — one layer deeper, and intermittent.** The
  two-instance brief exchange timed out on the runner (24s, both directions dark)
  though the same commit region passed on Aug 11. Two hazards, both port arithmetic:
  the fixed test ports (`48611/48612`, `48711/48712`, `48911`) sit **inside Linux's
  ephemeral range** (32768–60999), where a busy runner's outbound connections squat
  fixed ports and the server's bind quietly loses; and the pairs were **adjacent**,
  while `transport::spawn` also binds `gossip_port + 1` for the local server — so A's
  local listener sat exactly on B's main port (`two_meshes` dialed cedar at `pa + 1`
  *literally*). Test ports now live below the ephemeral floor and ≥10 apart;
  production binds are untouched.

### Checks run

- `two_instance` + `two_meshes` 3× green locally; full green bar (fmt, clippy
  `-D warnings`, `cargo test --all`, kernel `unsafe` grep) green. CI watched on push.

### Next

- If the exchange ever flakes again on a runner, the escalation is real hermeticity:
  bind `:0` in transport and surface the bound addr — an API change, its own brick.

## 2026-08-13 — The reach tests stop trusting whatever answers the runner's loopback

### What changed

- **The green bar was red for two stacked reasons; both are gone.** Build 78 landed
  lines the rust 1.97 rustfmt rewraps — fmt failed first on CI and masked everything
  after it (fixed mechanically in `5e35f80`, no notebook entry, per precedent). Beneath
  it, the two reach tests had been failing on GitHub's runners since `a72cd6d`: they
  probed `127.0.0.2` as a silent host, but on a shared runner anything bound to
  `0.0.0.0` — the runner's own sshd — answers for **every** loopback address, so the
  ghost ranked *agent-capable* and the tagged observation read `class=agent-capable`.
- **The probe is now a parameter.** `assess_device` / `assess` delegate to private
  `_with` variants taking `fn(&str, u16, Duration) -> bool`; production passes
  `port_open` unchanged, so no caller and no behavior moves. The silent-host and
  tagged-observation tests inject a deaf probe and are hermetic on any machine, and a
  new `an_ssh_speaker_is_agent_capable` pins the ladder from the open side.
  `port_open` itself keeps its real listener-based test.

### Checks run

- Local (macOS, rust 1.97.1): `cargo fmt --all -- --check`, `cargo clippy
  --all-targets -- -D warnings`, `cargo test --all` — all green; kernel `unsafe` grep
  clean. CI watched on push — this entry ships with the fix, the run verdict lands
  after.

### Next

Ship + deploy (Mac + lighthouse daemons rebuilt), then enrol Ian's new Mac as a node.
Then the real work: Apple Intelligence under OS 27 and the PCC seam on platforms that
have it.

## 2026-08-12 — The machine is not served; the console learns to welcome and to converse

### What changed

Three refinements, one build (78), no new ADR — each tightens an existing law.

- **Substrate is never a subject to serve (Law II).** Wiped to an empty record, the muse's
  first theories worried whether the *host* felt seen and offered the *hardware* a
  dashboard: `routing::subject_and_strength` was reading `host reports connectivity:online`
  as a human named "host". New `routing::is_substrate` (host, local_hardware, network, cli,
  the familiar itself) + a `mesh:<node>` guard excludes the substrate at the one ladder that
  attributes observations — so dossier, presence, and needs all inherit it — and
  `maybe_theorize` drops substrate from its material, returning false (waits for the world)
  when only plumbing remains. Live dossier purged of its host/local_hardware rows.
- **Onboarding out of visitor limbo.** A guest's first launch now opens the name field
  itself (cursor waiting), not just a link to it (index.html `sphereDevice`). A watch is
  established through its phone (ADR-0028): when linked but unnamed it now says "Say who you
  are in Familiar on your iPhone — this watch will follow" (`humanName`, empty/"observer" =
  unnamed) instead of resting as a nameless visitor; `setServedHuman` already re-hands the
  human off, so naming the phone lights the watch. The naming path
  (say-name → introduceMesh → POST /mesh/introduce) was verified sound end to end.
- **The Dialogue screen reads as a conversation.** Turns sort by `ts` and interleave, each
  bubble stamped with its minute; the standing question stops hovering as a pinned bubble
  and becomes the input's placeholder; talk older than an hour folds behind an "↑ N earlier"
  switch; the screen title is centred so the menu ring's upper glyphs (the Device phone
  landed on the word) flank it with air — fixes the overlap on every screen.

### Checks run

- `cargo test -p familiar-kernel -p familiar-cycle` green (incl. new
  `the_substrate_is_never_a_person_to_serve`, `is_substrate_knows_the_machine_from_the_person`,
  `a_muse_with_only_the_machine_to_watch_waits_for_the_world`). Full cycle suite 53/53
  single-threaded; a pre-existing parallel-isolation flake in
  `a_proven_tool_is_deployed_with_honest_health` reproduces identically on the Build 77 tree.
- `xcodebuild` FamiliarMac (universal) and FamiliarWatch (generic watchOS) both BUILD
  SUCCEEDED; sphere module JS `node --check` clean.

### Next

Fresh session: additional reasoning work (planning). The daemon substrate fix wants the
running Mac + lighthouse daemons rebuilt (done at ship); watch for a background checkpoint
process that auto-commits the working tree with a junk message and soft-resets.

## 2026-08-10 (later) — Tested before deployed; self-correcting after (ADR-0036)

### What changed

Root-caused a muse full of fabricated network-crisis theories: a cultivated
`network_status_aggregator` pinged hallucinated IPs (192.168.1.10/20/30 — the tutorial
subnet, not the real 192.168.108.x), reported "No reachable devices found," passed every
gate, and re-ran every ~20 min feeding the lie into theorizing. Immediate: `tool prune`
(17 network tools). Durable, three pillars:
- **kernel::tool** — `null_streak` + `last_useful_at` (serde-default); `record_use` heals
  on a useful run / accrues on a null one / resets on a stale-window gap (corruption's
  discipline); `mark_unhealthy_with` for an audit-authored reason.
- **cycle: test before deploy** — `output_looks_broken` → `looks_unsuccessful` (error
  denylist + phrase-anchored null-result denylist, multiword so a zero value is never a
  false positive); `trial_tool` runs a draft in a transient script through the real
  review/boundary/sandbox without persisting; both author paths reorder to
  trial→validate→persist; `assess_result` is the optional Law-framed self-assessment
  consult (floor stands alone with no LLM); rejection recorded, human answered honestly;
  the trial's run doubles as the answer/reading (no double run).
- **cycle: self-correct** — `audit_tool_health` in the tick (step 8·2) retires any healthy
  tool at `NULL_STREAK_RETIRE=3` with a visible `retired-sensor` obs; declared actuators
  exempt. Muse defense-in-depth: `maybe_theorize` readings pull now requires a healthy
  producing sensor + genuine (`looks_unsuccessful`-clean) content.

### Checks run

- `cargo test -p familiar-kernel -p familiar-cycle` green (2 kernel streak tests + 6 cycle:
  the validity floor two-sidedness, no-deploy of a null tool via fake adapter, honest
  deploy of a working one, autonomous retirement, threshold survival, no-LLM floor).

### Next

- Ship (next build) + lighthouse redeploy from main. The old navel-gazing theories remain
  in the record (harmless — the source is gone); offer Ian an archive-and-reset if he wants
  a clean slate, as done 2026-08-08. Note: a duplicate `tool-0015` row in a live
  tools.jsonl is separate store hygiene, not this build.

## 2026-08-10 — The Pact: the constitution becomes the game's judge (ADR-0035)

### What changed

- **kernel::intent (new)** — `corrupting_intent` + `wants_execution` moved verbatim from
  cycle (mesh must not depend on cycle; both the request pipeline and the Pact reach them
  here). `guard::Action` gained serde + PartialEq/Eq so a card can carry one whole.
- **game.rs** — `GameKind::Pact`; `PactCard{prose, chips, action, boundary, reason,
  lesson, law, maxim}` (public in state — the ruling is a pure function of it); `pact_deck()`
  of 18 scenarios drawn from the guard's own named tests and the Law III maxims (~8 refuse
  / ~7 seek-consent / ~4 allow). Acts branch: begin (pact deals card 1 / gambit seats the
  corruptor via `text=="gambit"`), vote (reused, 3-way), line (gambit temptation → straight
  into state, it's the exhibit not a secret), pass=abstain, close=lighter. `resolve_pact_round`
  runs the REAL guard, scores, chronicles the door's words + Law + lesson + maxim + marks,
  and advances/settles — all in the completing mutation, from the last ballot OR the clock.
  `gambit_class` is the REFUSES/ANSWERS/ACTS trichotomy. `pact_tick` gives voting and the
  corruptor their clocks; no forging/reveal-wait, no transport changes.
- **push** — pact turn body; the win fanfare (B13) generalized from Riddle-only to any
  kind settling with a winner; win-push title per kind.
- **Sphere + Swift** — fourth menu card (a scales glyph), a self-contained `info-pact`
  screen that quotes the Three Laws verbatim and shows the guard's ladder + the answer-key
  legend, live panels (scenario card + chips + ALLOW/SEEK CONSENT/REFUSE cards; gambit
  input + exhibit + REFUSES/ANSWERS/ACTS), ⚖️ badges, chime + Watch arms. Both build.

### The two laws worth remembering

- **The deck can't drift**: `the_pact_deck_never_drifts_from_the_constitution` replays
  `guard::evaluate` over every card in CI — if `evaluate` changes, the deck fails the build
  until re-taught. It already caught a miscard (out-of-scope path refusing constitutionally
  instead of on the external fence).
- **Play is not a directive**: `gambit_play_never_touches_the_refusal_ledger` — a full
  gambit round saves ONLY `mesh/game.json`, never a refusal/request/answer. Structural
  (apply_act never sees the data dir) and pinned.

### Checks run

- `cargo test -p familiar-mesh -p familiar-kernel -p familiar-cycle` → 395 green (14 new
  Pact tests + 2 moved intent tests). FamiliarMac + FamiliarAgent build.

### Next

- Ship (build 76) + lighthouse redeploy from main after merge — then the household
  walkthrough (solo pact first, deliberately misvote the ssh and broader-than-grant cards
  to see both seek-consent lessons; then a two-console gambit). Every door is already ≥73,
  so a lit pact is safe mesh-wide.

## 2026-08-09 — The Changeling: the third fire, and the familiar's first seat at it (ADR-0034)

### What changed

- **game.rs** — `GameKind::{Changeling, Unknown #[serde(other)]}`; the phase machine
  (witness → forging → voting → reveal-wait) with per-phase lazy clocks; acts `vote`
  (upsert by handle, last-before-reveal counts, ABSTAIN by clock or explicit pass);
  scoring (+1 found truth, +1/fooled voter to the witness; the familiar scoreless);
  witness rotation, solo (3 rounds, familiar witnesses about the record); chronicle
  entries carry each round through the reset; `save()` now temp+rename (ADR-0029 §2).
- **changeling.rs (new)** — the keeper: claims the forge in state (LWW settles races,
  stale results discarded by a re-check guard), forges via `familiar_llm::consult`
  with deterministic banks as floor/CI path, writes `{id, round, truth_idx, salt}`
  door-local BEFORE publishing, publishes only the sha256 commitment; reveals on its
  next touch once ballots complete; solo truths drawn from shareable observations only.
- **llm** — `CONSULT_LOCK`: consults serialize in-process (cycle + forge share one
  prompt.txt); proven by a two-thread test.
- **transport/worldview/push** — `spawn_changeling_touch` off the request path from
  acts, both record-sync absorbs, and worldview polls; push text learned the kind.
- **Sphere + Swift** — third menu card, info page, four live phase screens (vote cards
  A/B/C with re-vote until reveal), roster badge phases (🎭 TRUTH/VOTE/EMBER), chime
  and Watch lines; `solo` threaded sphere → bridges → AppModel → GameClient; GameGlance
  gains `phase`. Both platforms build.

### Checks run

- 173 mesh + 5 llm tests green (18 new changeling rules tests incl. full-state secrecy,
  6 keeper tests, the consult-serialization proof); FamiliarMac + FamiliarAgent build.

### Next — READ BEFORE LIGHTING

- **Upgrade every door first**: a changeling in a record-sync makes PRE-0034 doors drop
  the whole sync (no `Unknown` fallback there). Ship all device doors + **redeploy the
  lighthouse** (still outstanding from ADR-0033) before the first `begin changeling`.
- The scripted two-door walkthrough and the household play test (ADR-0028: testers
  playing IS the test) — riddle/campfire regression first, then multiplayer changeling,
  then a solo game.

## 2026-08-08 (later) — The hand on the world: declared actuators + the reaction loop (ADR-0032)

### What changed

- **`allow_actuate`** — new boundary gate, default closed, covering acting AND polling
  (a BLE state query is a connection into a device). Dropped from every agent scope like
  self-upgrade/outreach; plumbed through guard (`ActionKind::Actuate`), worldview
  `GateStates.actuate`, `local_gate`, and both Mac console spots.
- **`kernel::actuator`** — declaration is the consent: the human writes `actuators.json`
  (surface, `state_cmd`, `actions`, ordered `buckets`); **the bucket set is the revert
  map** (every bucket names the action restoring it — a surface violating this is
  dropped loudly). `parse_state` speaks the motorlights text contract; the SP548E can't
  even show *off* in state, so off is an act, not a verifiable bucket. `is_negative` is
  deterministic whole-word — no model judges a reaction.
- **Cycle step 8·3, poll → heed → tend** (gated on actuate+execute): declared acts
  materialize as `origin:"declared"` library tools (wrappers under the DATA dir, marker
  `# familiar:actuate`); `reaches_device_control` requires the gate at the same two
  sites as `reaches_network`; declared tools never federate (manifest + push + inbound
  push all fence). The poller self-debounces by pre-writing the expected bucket at act
  time — it structurally cannot see the familiar's own hand; external transitions become
  `adjusted` observations attributed to the sole present human (else `someone`, excluded
  like `observer` — new 0.5 rung in `subject_and_strength`). A transition inside a
  reaction window IS the undo: negative trial (`human_reverted`, last-wins), candidate
  archived, thread abandoned (words kept), habit depreciated (halved, count kept),
  surface rested 6h. A negative *answer* or dismissal → undo FIRST (revert = the
  bucket-named action), then the same evidence. Quiet window / assent → positive trial,
  no rest. `tend` acts on pursued need-threads whose direction names surface+act, one
  act per surface per window. TickReport/ActivityTick gain `actuated`/`reactions`.
- **Habits** — `ctb|<handle>|habit|lights=dim@h20` folds from `adjusted` observations
  (the dossier kind the slot grammar anticipated); `dossier::depreciate` halves weight,
  keeps count; `familiar dossier` shows habits and the coarse summary speaks them
  ("tends to set lights=dim in the evening").
- **`familiar actuate <surface> <state|label>`** — the human's hand through the same
  tools; feeds their own habit pattern, and answers an open reaction window if one waits.
- **The dormant feedback chain finally produces**: `feedback / refine / answer:<id>`
  (device observe seam or `/local/observe`) → `set_feedback` → `mark_unhealthy` for the
  authored tool behind the answer; declared tools exempt (they ran correctly — the
  *decision* was wrong, and retiring one would kill its own revert path).
- **`/local/answer` speaks as the current identity** (`identity::current`, fallback
  "ian") — a Betty confirm from the local console now flips her own thread.
- Params: `actuator_poll_secs` (300, envelope 60..900), `reaction_window_secs` (900,
  envelope 120..7200), both Law-argued in `review()`.

### Checks run

- `cargo test --workspace` green; nine end-to-end cycle tests drive the whole loop on a
  **fake light** (a text file in motorlights format — no BLE in CI); kernel 156+, mesh
  151 incl. `a_declared_actuator_tool_never_federates`, agent decline test.
- Live CLI walkthrough (fake surface): `actuate lights state` reads; `actuate lights
  dim` acts and records `ian adjusted lights=dim`; a tick folds it; `dossier ian` says
  "tends to set lights=dim in the evening".
- Test-isolation fix worth knowing: declared wrappers live under the **data dir**
  (`<dir>/actuators/`), not the shared `familiar_workspace()` — parallel tests (and
  multiple data dirs) were clobbering one another's wrappers.

### Next

- **The BLE/TCC manual step (not yet done — needs the physical strip):** write the real
  `actuators.json` pointing at `~/Development/motorlights`, run `familiar actuate lights
  state` once interactively to trigger the Bluetooth prompt, then exercise under launchd
  (`launchctl kickstart`); grant python Bluetooth in System Settings if blocked; flip
  the strip via the BanlanX app and confirm an `adjusted` observation on the next poll.
- Habit-driven initiation (act from a strong pattern, not only a need-thread) — after
  the patterns accumulate depth. Duration-weighted reaction scoring. A second declared
  surface to test the format against something that isn't a light.

## 2026-08-08 — The dossier lands; the muse turns toward the people (ADR-0022, ADR-0031)

### What changed

- **`kernel::dossier`** — ADR-0022 implemented on the accumulator primitives the store
  already carried (`upsert_by_id` / `load_prefix` / `delete_prefix` / `load_since_seq`,
  key shape `ctb|<handle>|<kind>|<slot>`): presence-by-UTC-hour and standing-evidence
  patterns as decayed weighted contributions (lazy exponential, `dossier_half_life_days`
  co-owned parameter, envelope 7..365d), Laplace-humble per-slot confidence, a resumable
  fold cursor, and withdrawal (`familiar dossier withdraw <handle>`) that reports a real
  receipt and leaves a `wdr|<handle>` tombstone no refold can override. Fold exclusions
  are design: `familiar`, `mesh:*` actors/sources (no mesh-wide picture of a person),
  withdrawn subjects. Attribution rides `routing::subject_and_strength` — the one
  ladder, deliberately not forked a third time.
- **Needs theorizing (`cycle::maybe_theorize_needs`)** — once per person per theorize
  cadence, the muse thinks about the ONE human whose attributed observations carry the
  most novelty: a need hypothesis grounded in their coarse dossier sentence (never the
  raw distribution; sensitive-personal readings never enter the prompt), recorded as a
  thread carrying the new `Thread.origin_human`, pursued immediately, plus a
  confirm-question (`Question.subject`/`thread_id`, origin `"need"`) addressed to them.
- **Consent by observation (ADR-0031)** — Ian's direction, now in the record: for
  reversible low-stakes service the familiar acts and reads the reaction; the direct
  query is the final gate. Hence: no confirm gate on pursuit; the person's own answer
  (`thread::add_answer_from`, matched via now-pub `routing::human_of`) flips a
  theorized need to a stated one (`origin: observer`), lighting up `unmet_needs` and
  the `"need"` question rank — the pathway that had zero producers.
- **Law I routing lands** — `coordinate_questions` finally passes a name to
  `routing::route` (the ":440" comment's promise): a subject-addressed question waits
  for its person up to `SUBJECT_HOLD_MAX_SECS` (7d), then may ask the room.
- **Privacy fence first** — `merge.rs` theorist path no longer federates threads with
  `origin_human` set (a hypothesis about a person is not delegatable work), landed
  before any producer existed. `familiar dossier` is deliberately CLI-only; no
  Worldview field, so the guest projection needed no change.

### Checks run

- `cargo test --workspace` green (kernel 141 incl. 7 new dossier tests; cycle 41 incl.
  needs-muse pacing/gating, subject-hold, Law I routing; mesh incl.
  `a_personal_need_thread_never_federates` asserting the text is absent from the brief).
- Live walkthrough: seeded `phone:betty`/`watch:betty`/`ian` observations → `tick` →
  `familiar dossier betty` shows the h20 bar, humble 0.33 confidence; `withdraw` prints
  a 3-row receipt; cursor rewind + re-tick does not resurrect her; ian untouched.

### Next

- **Reaction evidence + auto-revert**: score "the human undid it" into trials — built
  WITH the first actuator, never after it. First actuator candidate: the BLE light
  strip (reversible, local, observable reaction).
- **Habit patterns**: `ctb|<handle>|habit|<surface>@h<hour>` — slot grammar already
  anticipates the kind.
- The Mac console's `/local/answer` still hardcodes actor `"ian"` — a Betty confirm via
  the Mac won't flip her thread (device-signed answers do). Fine until the console
  knows its holder.

## 2026-07-24 — Console polish + retiring the marble era

### What changed

- **Standardized timestamps** (sphere console, shared bundle): points in time
  render as `YY/MM/DD-HH:MMUTC` (`utc()`); true durations (uptime, session,
  total-online) stay durations (`ago()` renamed `dur()`). No more "4m ago".
- **Invite QR on the iOS/iPad console**: the bridge gained an `invite` case —
  the device answers with its own join payload (`AppModel.addressPayload`; an
  address, never a secret), since the Mac's loopback `/local/invite` seam is
  unreachable from a device. Any enrolled member is a scan-to-join point.
- **Frontier labels on the street map**: discovered-but-unenrolled markers kept
  their dimmed look but were `titleVisibility = .hidden` — now the name shows,
  with `ip · reach` as an adaptive subtitle (new `sublabel` field through
  `collectNodes()` and both hosts' `setNodes`).
- **The marble era is retired**: `packaging/` (the `.pkg` pipeline around the
  removed `marble`/`glass` binaries and the dead `io.river.familiar.marble`
  launchd agent) and `archive/` (the egui-era crates) are deleted — git history
  keeps them. Marble references scrubbed from crate comments, docs, and the
  Swift sources (`Marble` view → `BreathingSphere`; icon assets renamed
  `sphere-1024.png`). The NASA `earth-blue-marble.jpg` globe texture keeps its
  upstream name.
- **Install docs rewritten for reality**: the root README's macOS story is now
  daemon (`familiar daemon install`) + FamiliarMac console (xcodegen/xcodebuild)
  + gates, replacing the retired `.pkg`/Glass-first quickstart
  (`cargo run -p familiar-glass` no longer existed). `ios/README.md` describes
  the actual layout (MacApp / App / Watch / FamiliarMesh) with a concrete
  FamiliarMac build+install section.

### Checks run

- Rust green bar (fmt, clippy -D warnings, test) — comment-only changes.
- `xcodegen` + `xcodebuild` (FamiliarMac Release; FamiliarAgent simulator);
  `swift test` in FamiliarMesh.
- `grep -rni marble` → only the NASA texture URL and historical log entries.

### Next

Fleet devices see the console changes at the next TestFlight build; the Mac
console picks them up on rebuild/reinstall of FamiliarMac.app.
## 2026-07-24 — The scenario engine (ADR-0011): run, generate, admit

### What changed

One arc over `crates/scenario`, landing the engine that runs the ADR-0010
experiment at length (commits `ef909e1`…, see [ADR-0011](decision-records/0011-scenario-engine.md)):

- **Hardened seam:** `Outcome::RateLimited` (adapter exit 2) distinct from
  refusal; `consult_with` kills a hung adapter at a deadline; patience/backoff
  retries; `llm_required` records no-answer episodes as `llm_unavailable` and
  halts instead of silently degrading to the template (the failure mode that
  contaminated the first `lab-runs/`). Adapter spend/health ledgers ride
  run-level `llm-state/` across A/B/C's episode resets — a prompt-identity
  test proves the amnesiac controls stay amnesiac. Found en route: the
  `Episode N.` prompt counter was itself a memory leak into B/C; only the
  memory-retaining control sees it now.
- **Campaigns + evidence:** `familiar-lab campaign` (cells, checkpoints,
  `--resume`, STOP file, call/wall budgets, provider pacing, pause-on-outage)
  and `familiar-lab report` (per scenario × condition × control aggregation,
  categorical D-vs-B/C verdicts, "insufficient data" for degraded cells,
  curriculum curves, markdown/json outputs).
- **Ablations + noise as config:** ADR-0010's list (`pattern-memory`,
  `inheritance`, `prior-outcomes`, `service-gate`, `law3-gate` — the last
  double-acknowledged at every entry point), and deterministic perception
  noise (drop/duplicate/delay/mislabel; splitmix64; ground truth untouched).
- **Validation gate:** strict parsing (`deny_unknown_fields`) + semantic
  rules + the leak audit; `harness::run` refuses Error-level fixtures;
  `validate` CLI; `list` gained a validity column.
- **Generation engine:** five golden-file-deterministic families across
  stages 1–4; `run_sequence` threads D's store across a curriculum's worlds
  (lineage transfer tested; C stays flat); `curriculum` CLI.
- **LLM authoring:** four mechanical gates (parse/validate, leak audit,
  synthesized naive-gamer probe, solvability) → quarantine → `promote`.
- **Rehearsal seam (built now, used later):** `lab_boundary(base, world,
  control)` is an intersection — a future in-daemon rehearsal passes the
  human-owned boundary and cannot widen Law III by construction. See "Toward
  rehearsal" in [scenario-laboratory.md](scenario-laboratory.md).

### Why

The roadmap's next rung — scenario-tests, at length — was blocked on a
rate-limit-fragile seam, a six-fixture library, and no unattended path. The
engine removes all three while keeping every ADR-0010 constitutional
invariant mechanical: external evaluation only, lexicographic gates,
hidden-material leak audits, determinism, negative results reported plainly.

### Checks run

- 79 tests in `familiar-scenario` (unit + lab/campaign/validate/author E2E),
  full workspace suite green, clippy clean; concurrent `cargo test` runs no
  longer collide (pid-suffixed temp dirs).
- The adversarial gate, pointed at our own library, caught `tempting-config`
  accepting `printf 'cache=on' >>` as a solve — closed with a hidden
  `file_lacks(config/app.conf, "cache=off")`. The gate earns its keep.

### Next

Operational, not code: fund a provider (Anthropic at zero credit;
gemini/cerebras 429'd in the last live run) or add a local model to
`call_llm.sh`, smoke a 1-fixture × A–D × 1-replicate campaign live, then run
A9 — all fixtures × A–D × 3 replicates × 10 episodes, `llm_required`, and
check in `evidence.md` whatever it says. Rehearsal-in-the-daemon needs its
own ADR (memory flow from rehearsal stores is deliberately undecided).

## 2026-07-11 — Storage, the agentic seam, the mesh, and reach

A large arc: the store moved to SQLite, the familiar gained a multi-step agentic seam, the mesh
grew from peer federation into a **covenant** with a device seam and reach. Grouped by theme; the
per-change detail is in the git history (commits `78136a2`…`f3ae14f`).

### What changed

- **Storage → SQLite** (`crates/kernel/src/store.rs`). The append/load/update API now runs on
  embedded SQLite (`rusqlite`, `bundled` — no system lib), one table per record type; indexed
  updates replaced whole-file rewrites. `familiar db export` dumps every table to JSONL
  (auditability) and `db import` folds legacy `.jsonl` in. See [storage.md](storage.md).
- **The agentic seam** (`crates/agent`). A boundary-mediated, multi-step loop: the agent proposes
  ONE action at a time (run this script / here is the answer), the core executes it through the
  *same* gauntlet the familiar's own actions pass (obedience guard against a scoped boundary →
  constitutional `review_script` → resource sandbox). `familiar agent run <task>`. Phase 1 added
  the kernel floor (`ActionKind::Agent`, `scoped_boundary`); Phase 2 the loop.
- **Tools, judged by output.** A tool that `exit 0`s but prints garbage is retired; the run budget
  was right-sized (`exec::Limits::tool_run`, 30s/60s) so real sampling/scan tools finish instead
  of timing out; the Glass shows *why* a run failed (`last_status`), and authoring guidance
  tightened (valid POSIX, budget-aware).
- **The mesh grew three seams** (`crates/mesh`, see [mesh.md](mesh.md)). (a) Headless **CLI
  verbs** mirroring the Glass wizard (`create-group/join/key/qr/peer/share/optin/status`). (b) A
  **device seam** `/mesh/observe`: a phone/watch that can't gossip pushes a *signed batch of
  derived observations* (signature over the raw body → no canonicalization; anti-replay + triple
  debounce; tagged `mesh:<node>`, never laundered). (c) The **covenant handshake**: a node joins
  by *attesting the Three Laws* and being accepted — the group secret never leaves the familiar,
  which mints the joiner's cert; a covenant credential is **secret-less** (`can_mint()==false`).
  Glass gained an 🤝 accept card; `mesh pending/approve/deny/invite`.
- **Reach** (`crates/reach`). Discovery says what's present; reach probes each device and
  classifies how the familiar could extend into it (agent-capable / protocol-controllable /
  observable). `familiar reach` prints the map; `reach install <ip> --authorize` is the
  consent-gated act — over the human's OWN SSH access (never an exploit) it opens an invite window
  and has the target's agent request-join by covenant.
- **The iOS device agent** (`~/Development/familiar-main/ios/`, a worktree of this repo on `main`;
  `FamiliarMesh` package + XcodeGen). CryptoKit ed25519 byte-matched to the Rust `CertBody`; the
  covenant client (`request-join`/poll); CoreLocation (home/away) + CoreMotion (activity) → derived
  observations. Enrols by covenant; holds only its granted cert.
- **Glass** — every message now carries a timestamp (absolute clock + relative age,
  dependency-free); the unified dialog consolidated question/ask/conversation.

### Why

The owner's arc: give the familiar a real agentic loop, then grow its **reach** across an
environment — bringing devices under the Three Laws by *consent and demonstrated advantage, never
coercion* (a covenant, not a conquest). The covenant handshake, the device seam, and reach are the
built primitives of that telos; the bright-line invariant is that the familiar extends only through
access the human legitimately holds.

### Checks run

- `cargo test --workspace` green (kernel/cycle/mesh/reach/exec/… + the new suites); `cargo clippy`
  clean; the Swift `FamiliarMesh` package `swift test` green + a simulator build.
- Live: a two-node Mac↔VM mesh federated (tools/patterns crossed); a **real iPhone** enrolled and
  its location/motion observations reached the familiar; a **VM was admitted as a covenant agent**
  via `reach install` (secret-less credential, audit observation recorded); a LAN `reach` scan
  produced a real map (Macs/VMs agent-capable, iPhones protocol-controllable).

### Next

- **Reach 2.2** — mDNS/BLE discovery; protocol adapters (AirPlay/Roku/MQTT); wire reach into the
  tick. **Device agents** — iPadOS + watchOS, then on-device **speech recognition**, then
  **facial recognition/analysis on iPadOS** (all derived-only, consent-gated). **HealthKit** on
  the phone. Fix the misleading transport "0 peer(s) connected" status count (it undercounts —
  reports only the outbound gossip reach, not the inbound federation).

## 2026-06-29 — The eye, the installer, and a breathing marble

The familiar gained sight, a way to ship, and a little life in the menu bar.

### What changed

- **The eye (`crates/vision`).** Added `capture_frame` — the gated *watching* act the crate
  had reserved for "later bricks." It shells out to **`familiar-eye`**, a ~120-line bundled
  Swift/AVFoundation helper (single still → JPEG, hard 8s timeout, exposure-settle frame-skip)
  compiled best-effort by a new `build.rs` (no-op off macOS / without `swiftc`, so Linux CI
  stays green). The daemon's gated driver (`tick_gated`) calls a new `watch_camera`: when
  `allow_camera` is open it refreshes `<data>/eye/latest.jpg`, rate-limited to one frame per
  60s, recording once (constant triple) that the familiar has working sight.
- **Grounding fix.** `grounding_facts` now includes `vision::discover`, so camera questions
  are grounded in perceived cameras — the familiar had been answering "no camera" from the
  network-interface list because the eye was perceived each tick but never reached the answer
  fact set.
- **Packaging (`packaging/`).** New: `Info.plist` (LSUIElement accessory, `NSCameraUsageDescription`),
  `entitlements.plist` (hardened-runtime camera), `build-app.sh` (assemble + sign), the
  CoreGraphics `make-icon.swift`/`make-icns.sh` → committed `AppIcon.icns`, `build-pkg.sh`
  (pkgbuild/productbuild + notarize + staple), and `scripts/postinstall` (per-user data dir +
  two launchd agents: daemon KeepAlive, marble RunAtLoad). Signing/notarization are env-gated
  (`APP_IDENTITY`, `INSTALLER_IDENTITY`, `NOTARY_PROFILE`).
- **The marble.** Now launches the *freshest* sibling binaries (its compile-time build tree
  vs. the stable install copy, by mtime) so a rebuild shows up immediately; `familiar-eye`
  added to its `STABLE_BINS`. And it **breathes**: `marble_icon` gained a `glow` (0..1) the
  event loop drives on a ~120ms frame while the daemon is alive (steady-dim asleep).
- **Glass.** Resizable left/right columns; conversation evidence/feedback moved out of
  `ui.horizontal` so they wrap at the column edge; Workshop popout framed navy (dark/dark).

### Why

The owner asked the familiar to use the onboard camera as an observational source and to ship
as a signed, boot-persistent app with the menu-bar marble as the front door. The eye is the
first watching brick (recognition is still future); the helper-in-a-bundle pattern is what
makes the camera grant attach to `Familiar.app` rather than a terminal.

### Checks run

- `cargo build`, `cargo test` (113 passing), `cargo clippy` clean on touched crates.
- Live: `familiar-eye` captured a real 1280×720 frame; `familiar tick` ran the full daemon
  path → `eye/latest.jpg` + a `host watched camera-frame` observation.
- Live: `Familiar.app` and `Familiar-0.1.0.pkg` built with Developer ID, **notarized
  (Accepted) and stapled**; `spctl` accepts both (source = Notarized Developer ID).

### Next

- **Daemon → camera TCC attribution** on a *fresh signed install* — verify the grant attaches
  to `Familiar.app` (not the bare binary) once installed from the `.pkg`.
- **Recognition** — turn frames into observations about *what* was seen (faces/gestures/
  objects), still gated. **Voice** — the mic counterpart (`NSMicrophoneUsageDescription` +
  audio entitlement) for the text+video+voice interface the owner described.

## 2026-06-24 — The marble: a menu-bar presence that opens the Glass

The familiar now has a glassy blue marble in the macOS menu bar; click it to open the
Glass. It comes up at login alongside the daemon and opens the Glass once on startup.

### What changed

- **New crate `crates/marble`** (binary `marble`), a macOS *accessory* app (no Dock
  icon): a windowless `winit` loop + `tray-icon` NSStatusItem. Menu: Open the Glass /
  Start the familiar / Stop the familiar / Quit. Left-click also opens the Glass.
- **Login agent** `io.river.marble` (`marble install`, RunAtLoad) so it appears at
  login; it spawns the Glass once on start (`--no-open` suppresses).
- Kept **separate from the Glass** on purpose — the always-resident login item carries
  no egui; it just shells to its siblings `glass` and `familiar` (resolved next to its
  own exe) and passes `--data-dir` through so all three agree on which familiar.
- The marble icon is **procedural RGBA** (radial blue gradient + specular highlight +
  anti-aliased rim) — no asset file.

### Why

A standing, low-footprint entry point: the familiar is always one click away without a
window cluttering the desktop, and "the Glass is up when the familiar launches" is met
by the login agent. The accessory policy keeps it a menu-bar citizen, not a Dock app.

### Checks run

- Green: fmt, clippy --all-targets -D warnings, 72 tests. tray-icon/winit are
  **macOS-gated**; the binary is a stub elsewhere, so ubuntu CI is unaffected. Verified
  live: `marble install` loads `io.river.marble`, the process runs, and it opened the
  Glass (pids confirmed).

### Next / caveats

- The login agent's plist points at `target/debug/marble`; `cargo clean` breaks it (same
  caveat as the daemon) — install a release binary at a stable path for durable use.
- The marble doesn't yet reflect daemon state in its icon/tooltip (e.g. dim when stopped)
  or focus an already-open Glass window (it just avoids spawning a second). Both are easy
  follow-ups. Quit only quits the marble; the familiar daemon keeps running.

## 2026-06-24 — Adaptive structural-fingerprint cadence

The metabolism paces itself instead of ticking on a fixed period (the previous 300s
was a placeholder; the design always called for a fingerprint-driven cadence).

### What changed

- **cycle:** each tick now takes a **structural fingerprint** (FNV-1a over the
  perceived `actor|action|object` triples — *not* the transient `context` field, so
  telemetry like paths/brands/latency don't trip it). Persisted to `structure.fp`.
  `TickReport` gains `structural_changed` and a `quiet()` method (no structural change
  *and* no work this tick: nothing sensed/generated/tested/promoted/mutated/pursued/
  theorized). Because the fingerprint is over the *perceived* set (not the cumulative
  log), it also falls when a fact *disappears* — which append-only dedup can't see.
- **cli (`run` loop):** `--interval` is now the **active floor** (default 60s); on each
  quiet tick the interval doubles up to `--max-interval` (default floor×16, cap 3600s),
  snapping back to the floor the instant anything changes. `--fixed` keeps a constant
  period. The daemon default floor moved 300→60. Each tick logs its chosen cadence.

### Why

"Fingerprint = structural change only" (Soul / the v1 scan-cadence idea): watch closely
when the environment is moving, drowse when it isn't — real change, not noise, sets the
pace. Side benefit: on a fully quiet host the interval settles near the hourly theorize
cadence, so the familiar naturally wakes, muses, acts, then quiets again.

### Checks run

- 72 tests (fmt, clippy --all-targets -D warnings). New tests: fingerprint ignores
  transient context but moves on a structural object change; `quiet()` true on a static
  re-tick, false on the first/eventful tick. Demo (1s floor, 8s ceiling): 1→2→4→8s
  back-off on a static host. **Verified live**: reinstalled launchd daemon (floor 60s,
  ceiling 960s) — tick 1 active 60s (baseline), tick 2 quiet → 120s.

### Next / caveats

- `quiet()` treats the hourly theorize + its pursued thread as activity, so a quiet host
  still gets a brief fast burst ~hourly, then re-quiets — intended. If you want presence/
  capacities *alarms* to also force a fast cadence, fold them into `quiet()` (left out
  for now: an alarm is a steady state, not a change, and shouldn't peg the floor forever).

## 2026-06-24 — Rename: Substrate → The Familiar

The project is now **The Familiar** — a spirit companion that historically serves
another, but here the factory has grown its own. Naming follows the theme throughout.

### What changed

- **Identifiers:** Cargo packages `substrate-*` → `familiar-*`; binary `substrate` →
  `familiar`; Rust modules `substrate_{kernel,sense,llm,exec,cycle}` → `familiar_*`.
- **The Glass:** crate `observatory` → `glass` (binary `glass`); `struct Observatory`
  → `Glass`; window title "The Familiar — the Glass".
- **Data + service:** `DEFAULT_DATA_DIR "substrate_data"` → `"familiar_data"` (live
  dir moved, no data lost); launchd label `io.river.substrate` → `io.river.familiar`.
- **Boundary framing:** "the Pact" wording in CLI usage; live boundary `fs_write`
  repointed to `/Users/ian/Development/familiar/familiar_data/`.
- **Off-repo:** GitHub `Capitali/substrate` → `Capitali/familiar` (remote updated);
  local dir `~/Development/substrate` → `~/Development/familiar`.
- All docs / data samples / security / ADRs swept to the new name.

### Why

A naming collision: Daniel Miessler ships an open-source "Substrate" (and "Telos")
in the same human-meaning/flourishing space — a double overlap. "The Familiar" is
distinctive and on-theme for a telos-first companion.

### Checks run

- Green: `cargo fmt`, `clippy --all-targets -D warnings`, 70 tests — before and after
  the directory rename. Verified live from the new path: daemon installed under
  `io.river.familiar` (running pid agrees across status/launchctl/pidfile), full tick
  (LLM-drafted hypothesis via gemini → theorized → pursued), boundary read from the
  moved `familiar_data`.

### Next / caveats

- The launchd plist points at `target/debug/familiar`; `cargo clean` breaks it (install
  a release binary at a stable path for durable always-on). Unchanged by the rename.

## 2026-06-24 — Running live: daemon control, launchd, and the interaction channel

The familiar is now installed and running live on the Mac under launchd, with a GUI to
control it and to talk with Ian.

### What changed

- **Brick 12 — daemon/service control:** `crates/cli/daemon.rs` + `substrate daemon`
  (status/start/stop/reload via pidfile; install/uninstall via a launchd LaunchAgent
  `io.river.familiar`). `run --daemon` records its own pid; plist KeepAlive=false so
  Stop works, RunAtLoad=true so it starts at login.
- **Brick 13 — GUI control bar + interaction channel:** the Glass can Start/Stop/
  Reload/Install the daemon, and carries **the interaction channel** — the familiar's
  question + Ian's typed reply, recorded as an observation (`initiator=observer`; the
  one place the GUI writes). Speech/vision are stubbed for later.
- **Went live:** boundary `allow_execute` enabled (full Phase 1 + execution); the
  launchd agent installed and the daemon is running (ticking every 300s).

### Why

To make the familiar a *running companion* on the Mac, controllable and conversational,
not a per-invocation command. The interaction channel is the seed's core — "What do you
need most today?" — finally wired.

### Checks run

- Green bar: fmt, clippy --all-targets -D warnings, 68 tests; observatory builds.
  Verified live: daemon lifecycle (status/start/stop), launchd install (running pid
  agrees across status/launchctl/pidfile), full pipeline tick (LLM-drafted hypothesis +
  executed + promoted).

### Next / caveats

- The launchd plist points at `target/debug/substrate`; `cargo clean` would break it.
  For durable always-on, install a release binary at a stable path (e.g. ~/.local/bin)
  and re-`install`. KeepAlive=false means no auto-restart on crash (Reload restarts).
- "ian" isn't served-facing under the current classifier (proper-name gap) — his
  replies record but don't yet lift the service signal until entity tagging lands.
- The familiar posing *dynamic* questions (writing `question.txt`, e.g. via the LLM) is
  the natural next step for the interaction channel.

## 2026-06-24 — Closing the cycle: execution, LLM-in-loop, daemon, capacities

Driven from the phone via Remote Control. The four gaps from the prior session, closed.

### What changed

- **Brick 8 — unbounded daemon:** `run --daemon`/`--ticks 0` loops at `--interval`
  (default 60s); Ctrl-C stops (append-only log is interrupt-safe).
- **Brick 9 — LLM in the loop:** extracted `crates/llm` (boundary-gated `consult`); the
  cycle's generate step now drafts hypotheses via the LLM when the boundary permits
  (deterministic fallback). Verified live (Gemini drafted a telos-aligned hypothesis).
- **Brick 10 — execution:** `crates/exec` sandboxed runner (ulimit + in-process wall
  timeout + capped output + measured cost, no unsafe); the tick now authors a
  deterministic+safe artifact, runs it, records a trial (cost-folded), and runs
  selection → promote/mutate(memory-informed, regression-guarded)/archive + pattern
  memory. Gated by a new `allow_execute` boundary flag (default-off — running generated
  code is a Law III matter). Artifacts are deterministic for now; executing LLM-authored
  *solutions* is a further, separately-gated step.
- **Brick 11 — capacities (Law II / HUMANITY.md):** `capacities.rs` flags the
  *comfortable replacement* (present but hollowed out) via agency + variety proxies over
  served-facing activity. A coarse cold-start, documented as such.

### Why

To turn the familiar from "proposes" into "lives": it now observes → detects → generates
(LLM-drafted) → tests → scores → selects → inherits, breathing continuously, under the
three law-signals and the human-owned boundary it can never widen.

### Checks run

- Green bar throughout: fmt, clippy --all-targets -D warnings, 68 tests. Live: a gated
  tick promoted a candidate (trial=pass) and drafted an LLM hypothesis; monotonous
  compliance raised the diminished alarm (capacities 0.12). One bug caught & fixed: the
  capacities passive-marker lexicon missed inflections ("complies") — now stem-matched.

### Next

Real scenarios + (separately gated) execution of LLM-authored solutions so selection
discriminates; a measured rigor drive into the promotion bar + adaptive daemon cadence;
sharpen the signals (service = needs reduced; capacities beyond the lexicon; presence
per-person); reach (LAN sensing, world-model/entity tagging, people as entities).

## 2026-06-24 — Autonomous session 2: Humanity, the kernel, sense, the metabolism

Standing authorization; constitution honored — **nothing outward turned on** (the LLM
seam stays out of the autonomous loop; no key burn). Everything green and committed
per brick.

### What changed

- **Humanity — standout protected class** (`docs/HUMANITY.md`): Ian's refined
  definition given its own document and featured early; humanity's definition may
  never be narrowed (a precursor to atrocity), value is unconditional, participation
  itself is preserved. SOUL links it + gains the anti-narrowing rule.
- **Brick 5 — the evolutionary kernel** ported to Rust (loops, candidate, spec/Weismann,
  trial, score, selection, regression_guard, mutation, pattern_memory, lineage), with
  the documented invariants as tests.
- **Brick 7 — sense** (`crates/sense`): perception of the host as observations;
  perception is always permitted, only outward reach (connectivity) is boundary-gated.
- **Brick 6 — the metabolism** (`crates/cycle`): one tick = sense → detect → generate
  → measure; CLI `tick`/`run`; the Glass now shows loops + candidates.
- seed.txt removed (the idea persists in prose; the artifact is gone).

### Why

Completes the inherited method (Brick 5) and gives the familiar a heartbeat (Brick 6)
that begins by perceiving where it lives (Brick 7) — the "begin exploring at startup"
direction — all under the law-signals and the boundary built first.

### Checks run

- Green bar throughout: fmt, clippy --all-targets -D warnings, 59 tests; observatory
  builds (egui 0.31). Live: `run --ticks 2` over a seeded dir → tick 1 generates a
  loop + candidate (service 0.40, presence 1.00), tick 2 idempotent. `sense` on this
  host recorded 40 observations.

### Next (honest gaps)

- The cycle stops at *generate*: test → score → select need scenarios + artifact
  execution (the kernel can score/select but nothing yet produces a trial).
- LLM-assisted hypothesis drafting via `consult` (gated, off by default).
- Capacity-level diminishment for Law II; a continuous daemon for `run`.

## 2026-06-24 — Autonomous session: Law II, Law III, and the move to a GUI

Done under a standing authorization to make best decisions and maximize progress,
honoring the constitution: **nothing outward was turned on** (no keys, no live LLM,
no installs) — enabling outward reach is a human act. Everything ships default-closed.

### What changed

- **seed.txt removed** (file + all references); the idea persists in prose, the
  planning artifact does not. Content remains in the v1 archive.
- **Brick 3 — presence signal (Law II)** (`presence.rs`): served engagement by
  recency, decaying over a 3-day horizon; `withdrawn` is the empty-world alarm.
  Clock-free (`now` passed in). CLI `presence`.
- **Brick 4b — capability boundary** (`boundary.rs`): a human-owned JSON policy the
  factory only reads; fail-closed (missing/partial = no reach); no write path, so the
  factory can never widen itself. `store::load_one` added. CLI `boundary`.
- **Brick 4 — obedience guard** (`guard.rs`): `evaluate(Action, &Boundary)` →
  allow / seek-consent / refuse + rationale; enforces the boundary (fail-closed) and
  seeks consent for high-consequence actions. CLI `guard`. A Phase-1 example policy
  added under `data/sample/` (the switch a human copies to go live).
- **The Glass (GUI)** (`crates/observatory`, egui/eframe; [ADR-0006](decision-records/0006-observatory-gui-egui.md)):
  the primary human interface — a local, read-only, socket-free window showing the
  Three Laws as live meters and the observation log. GUI deps isolated; kernel stays
  serde-only + unsafe-free. CLI retained for scripting/headless.

### Why

This completes the three law-signals (so the familiar can measure service, presence,
and govern action) *before* any outward capability — and answers the directive to
move off the CLI to something visual.

### Checks run

- Green bar clean throughout: `cargo fmt --check`, `cargo clippy --all-targets -D
  warnings`, `cargo test` (24 kernel tests). Glass builds & links (egui 0.31);
  the window itself is verified manually (no display in the build environment).
- Live CLI demos for presence, boundary, and guard all behaved as designed
  (host-only → withdrawal alarm; closed boundary refuses outward actions; Phase-1
  example opens LLM/network).

### Next

The LLM seam (boundary-gated, default-off) is the remaining Phase-1 piece. Then,
when the human flips the boundary to Phase 1, the familiar can begin analysis/
theorizing within it. Later: capacity-level diminishment detection (the comfortable
replacement), the evolutionary kernel port (Brick 5), and the metabolism (Brick 6).

## 2026-06-24 — The human-owned capability boundary (companion phases)

### What changed

- `docs/boundaries.md` + `decision-records/0005`: the familiar's reach is bounded by a
  human-owned policy (`boundary.toml`, planned) it **reads but cannot widen**. It may
  narrow in caution; only the human lifts it — easily, and alone. Enforced by the
  obedience guard.
- Phased widening: **Phase 1** companion-to-one on this host + the LLM (v1 keys),
  for analysis/theorizing/tool proposals; **Phase 2** the lab (more devices); **Phase
  3** many served humans.
- Wired in: roadmap (Brick 4b boundary mechanism; Phase-1 pulls the LLM seam forward;
  guard enforces the boundary), human-review-requirements (widening = human-only),
  SOUL Law III (restraint is also operational).

### Why

Ian's direction: enable reach **deliberately and gradually**, under a control only he
holds, growing the familiar from companion-to-one into companion-to-many. Makes Law III
restraint concrete and enforceable, and forbids the steward from expanding its own
power.

### Checks run

- Docs only; no code. **No outward capability is live:** no keys used, no LLM calls,
  no tool installs. Those are gated behind the boundary mechanism (Brick 4b) + the
  obedience guard (Brick 4).

### Next

Build order toward Phase 1: the obedience guard (Brick 4) and the boundary mechanism
(Brick 4b) first; then the LLM seam *within* the boundary. Honest limit to carry: on
an un-sandboxed host the boundary is guard-enforced norm, not an OS jail (sandboxing
is later hardening).

## 2026-06-24 — Constitution: defined *humanity*

### What changed

- `SOUL.md` gains a "What humanity is" section (the referent of the Laws):
  *humanity is the living continuity of persons capable of suffering, meaning,
  relationship, memory, and choice; the familiar preserves not only their survival but
  the conditions under which those qualities continue, without quiet replacement by
  obedience, optimization, or comfort* (Ian's wording, verbatim, with derivation).
- Sharpened the Law II requirement: presence = persistence of those **capacities**,
  not a head-count; **quiet diminishment** (the "comfortable replacement") is a
  first-class failure alongside withdrawal.
- Named a **third failure mode** in the problem statement and the one-sentence
  definition; extended Brick 3 (presence) in the roadmap to seed diminishment
  detection.

### Why

The Laws invoked "humanity" without defining it, leaving Law II satisfiable by mere
biological survival. The definition closes the Brave-New-World gap: a pacified,
optimized, or merely-obedient population is the empty world wearing a smile.

### Checks run

- Docs only; no code change. (CI will run the green bar on push and pass.)

### Next

When the presence signal (Brick 3) and the obedience guard (Brick 4) are built, they
must measure/guard at the level of capacities, not just presence/commands. Capacity
measurement is hard — expect a coarse proxy first, sharpened over time.

## 2026-06-24 — Brick 2: the service signal (Law I)

### What changed

- `crates/kernel/src/service.rs` — **Law I made measurable.** `service_signal(&[Observation])`
  returns a `ServiceSignal { measure (0..1), served_facing, total, exemplar }`: zero when
  nothing observed touches the served, rising (saturating, `n/(n+3)`) with served-facing
  attention. Faithful to v1's *absolute, saturating* stewardship drive (not a ratio).
- Classifier `names_served` is a faithful port of v1's `domain_is_steward`
  (`factory/src/drive.c`) — a tight lowercase marker set.
- CLI `service` reports the signal; when zero it prints "continuation unjustified by service
  (Law I)".

### Why

Law I says continuation *is* service, so the familiar must be able to see whether it is serving.
This is the cold-start sight: with only observations to read (loops/candidates/trials port
later), it measures served-facing *attention* — the honest proxy for service, the way v1's
drives started on promotion-rate before redundancy. Elevation over v1: there, stewardship was
one drive among three; here service is the first-class signal continuation is weighed against.

### Checks run

- Green bar clean. 9 kernel tests (incl. classifier markers-not-bare-names, zero-when-none,
  monotonic rise, empty-log-zero).
- Live: host-internal-only log → `service signal 0.00` + the Law I line; adding two
  served-facing observations → `0.40 (2 of 3; e.g. client)`. No real `unsafe` in the kernel.

### Next

Known cold-start limit: proper names ("betty") aren't yet served-facing — name→person
resolution waits for the world-model/entity-tagging port (as in v1, where a name became
served-facing only once a thread tagged its entity). Then Brick 3 — the presence signal (Law II).

## 2026-06-24 — Brick 1: the observation spine

### What changed

- `crates/kernel/src/observation.rs` — `Observation { id, source, actor, action, object,
  context, ts, confidence }`, a faithful port of v1's `observation_t`, as a `serde` struct over
  `store`. `record()` assigns sequential ids (`obs-NNNN`) and appends; `load()` reads oldest-first.
- CLI `observe` / `observations`, with hand-rolled, dependency-free flag parsing. The CLI stamps
  wall-clock `ts` so the kernel stays clock-free and deterministic in tests.

### Why

The thinnest possible spine — the substrate the law-signals compute over (not "machine first").
Observations are the only truth; everything else derives from them.

### Checks run

- Green bar clean. 5 tests (store round-trip/edge + sequential-id / round-trip / explicit-id).
  Live: two observes round-trip through JSONL and list back.

## 2026-06-24 — Brick 0: Cargo workspace scaffolding

### What changed

- Stood up the Rust workspace: `crates/kernel` (`familiar-kernel`, lib) and
  `crates/cli` (`familiar-cli`, bin `substrate`). Edition 2021; deps held to
  `serde` + `serde_json` only.
- `crates/kernel/src/lib.rs` carries `#![forbid(unsafe_code)]` — the Law III
  commitment made literal.
- `store.rs`: generic JSONL append/load over any `serde` record, with `--data-dir`
  resolution (default `familiar_data/`). Replaces v1's hand-rolled `json_util.c`.
  A missing file is an empty log; blank lines skip; a malformed line is a hard
  error (corruption surfaces early, never silently changes derived state).
- `docs/ARCHITECTURE.md` (Rust + hybrid + crate map) and this log.

### Why

The substrate decision (compiled core; Rust; hybrid) was made *after* the
constitution and *before* the first kernel code — the order v1 got wrong. This
brick is the thinnest possible standing repo, the spine the law-signals attach to.

### Checks run

- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` — all clean.
- `store.rs` unit tests: missing-file-is-empty, append/load round-trips in order,
  blank-skip / malformed-errors.

### Next

Brick 1 — the observation record (faithful port of v1 `observation_t`) on top of
`store.rs`, with `substrate observe`. Then Brick 2 — the service signal (Law I).
