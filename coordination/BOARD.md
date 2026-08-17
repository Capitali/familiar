# The board

> ▶ **CO-DEVELOPMENT HOLD LIFTED EARLY — 2026-08-15 (Ian).** Ian's direct instruction:
> "excellent. authorized. continue." Both lanes may resume; the seven items recorded as owed
> to codex remain live work rather than abandoned questions.

Rules and entry format: [README.md](README.md). Claim = move to `claimed` + your name,
in a pushed commit, scope checked against every other claimed task. Updated: 2026-08-14.

## Proposed

*(companions add here; the controller queues or declines)*

### T-212 · Nothing ships unwired — a declared capability must have a live producer
- status: proposed — **the class behind T-211; roughly half the state-writing surface produces no data**
- owner: —
- scope: a structural test in crates/kernel (declared record types vs live producers); then re-point or honestly retire the orphans found
- depends: —
- accept: a test fails when a record type has a write function whose only callers are `#[cfg(test)]`; each orphan below is either re-pointed to a live producer or explicitly retired with its ADR status corrected; the two tables that do not exist on the live DB (`answers`, `belief_transitions`) either exist or their writers are gone
- notes: audit 2026-08-17. The common cause across every case: **tests assert the write function works, and nothing asserts that anything calls it in the shipped configuration.** Every orphan below has passing tests. Three distinct mechanisms —
  (1) **producer removed by a UI migration**: `request::append_request` (`crates/kernel/src/request.rs:73`) — `git log -S` shows the egui Glass GUI was its sole producer, archived in `b89070e` and deleted in `3f04c53`; the tick-side consumer `answer_requests` has run against an empty queue ever since, taking `grounding_facts` and the whole T-136 registry-view work down with it. Live: `requests` 0, `refusals` 0, `answers` table **absent entirely**.
  (2) **producer never written**: `consult::enqueue` (`crates/mesh/src/consult.rs:117`) — zero production callers, while the queue is *drained and served* by three transport paths and ADR-0014 is marked *"accepted (implemented + validated)"*; `question::record_answered` (all 4 live questions read `answered: false`); `belief::transitions`; `prediction::calibration`; `corruption::flagged`; and `crates/kernel/src/affected.rs` — **346 lines of Law-I invariants with zero references anywhere outside the file**.
  (3) **wired but gated shut** — trials, tool `uses`, actuators, reaction rules, identities, local pattern learning. NOT a defect: this is fail-closed working. It is why 222 candidates sit at `generated` across 536 ticks, 28 tools have `uses: 0`, and all 1932 patterns carry `origin=mesh:…`. Belongs to the boundary-drift item in STATE.md, not to this task — except that the ADRs describing it should say *inert* (see T-214)
- notes: cheapest first cut is a one-line check diffing declared record tables against live row counts; it would have caught the two missing tables immediately

### T-213 · Documented guarantees the code does not provide (reach layer)
- status: proposed — **security-shaped; the same defect class as T-210, on the enforcement side**
- owner: —
- scope: crates/mesh/src/transport.rs (`local_gate`), crates/mesh/src/push.rs, crates/exec/src/lib.rs, docs/boundaries.md
- depends: —
- accept: every guarantee stated in docs/boundaries.md is either true of the code or removed from the doc; `local_gate` has authentication stronger than a loopback source-IP check and a test; APNs push passes a boundary check; the `ReadFile`/`WriteFile` scopes are either enforced or the doc stops implying they are
- notes: enforcement audit 2026-08-17. **`docs/boundaries.md:61-63` says "No self-widening code path. The familiar has no code that writes the boundary policy." That is false** — `crates/mesh/src/transport.rs:1900-1955` `local_gate()` sets any of 16 gates to `true` and writes `boundary.json`, including flipping phase closed→phase-1. The intent is right (Ian owns the boundary, so he needs a way to open it), but it is authenticated only by a loopback source-IP check at the route (`:1181-1183`), which does not distinguish Ian's click from any other local process, and **no test covers it**. Also: APNs push (`crates/mesh/src/push.rs`, fired from `transport.rs:2616/2896/2909`) is an outward network transmission to Apple with **no boundary check of any kind**; `exec::run_script` (`crates/exec/src/lib.rs:105`) `cd`s into a workdir rather than jailing, while `ReadFile`/`WriteFile` have no production caller at all so the `fs_read`/`fs_write` scopes are evaluated by a function nothing calls; 7 of 17 ActionKinds are dead in production; `allow_self_upgrade` is settable from the console and read by nothing; the cloud-LLM boundary is an env var handed to a shell script with nothing verifying the adapter honoured it
- notes: what DID hold, and is genuinely well built: `guard::evaluate` is fail-closed, `boundary::load` falls back to `closed()`, `narrow_gate` makes widening unrepresentable in the type, scoped boundaries are a true intersection, and an inbound signed peer grant with `approved: true` is refused as constitutional. The gap is conduct, not reach

### T-214 · Doc honesty: "implemented" must be distinguished from "implemented and inert"
- status: proposed
- owner: —
- scope: docs/decision-records/ status lines; docs/boundaries.md
- depends: T-212 (the audit names which are which)
- accept: no ADR reads `implemented` for a capability with no live producer or no open gate; the honest label already exists in-house and is the model — ADR-0024: *"presence built; identity structurally ready and **inert**"*
- notes: 2026-08-17. Candidates found: ADR-0014 (*"accepted (implemented + validated)"* — `consult::enqueue` has zero production callers), ADR-0031, ADR-0032, ADR-0036 (all actuation/reaction/tool-health machinery, all zero live effect). This is the cheapest task on the board and it is the same defect class as the Asimov recital: a document asserting something the running system does not do

### T-211 · The conversation and the mind are two different organisms
- status: proposed — **architectural; verified live 2026-08-17. T-210 is a symptom of this**
- owner: —
- scope: diagnosis and design first. Touches crates/cycle/src/lib.rs (`maybe_reply` vs `answer_requests`), crates/kernel/src/request.rs, crates/kernel/src/system_facts.rs, crates/kernel/src/intent.rs, the console/iOS utterance seam in crates/mesh/src/transport.rs. No code before a decision — this is an ADR-shaped question
- depends: —
- accept: a human sentence typed at a console is answered by a path that can see the system facts registry, what the familiar currently believes, and the constitution; a corrupting instruction typed into the chat box is screened exactly as one filed as a request; an answer carries its confidence and evidence; the two answering pipelines are one pipeline or the dead one is deliberately retired
- notes: Ian 2026-08-17, on the Asimov reply: *"it's bigger than that, this dialog function has zero awareness of the constitution or purpose it would seem. Is it truly interacting with the interpretive coding layer at all anyway — it feels extremely disconnected for something that was supposed to be the brain of the operation."* He is right, and the mechanism is now traced.
- FINDING: there are **two** human-facing answering paths and they share almost nothing. (1) `answer_requests` (cycle/src/lib.rs:2265) is the grounded one — `grounding_facts` renders `system_facts::render_for_answering` plus census/interfaces/vision/network, screens the text through `intent::corrupting_intent` and records a refusal against the asker on a hit, and emits an `Answer` carrying `Confidence` and an `evidence` field. (2) `maybe_reply` (cycle/src/lib.rs:633) is what a person actually reaches.
- What `maybe_reply` receives, in full: `LAW_III_VOICE`, one sentence of purpose, `recent_dialogue` (chat history), and `known_of` (dossier coarse summary + up to 3 open needs). It does NOT receive the system facts registry, any theory, belief, prediction, goal, loop, the service/presence/capacities signals, or the boundary. Its only output check is `looks_like_prose` (cycle/src/lib.rs:745) — a SHAPE test: not JSON, ≥2 words, mostly letters. Nothing checks the content against anything the familiar knows, which is exactly why the Asimov recital passed unchallenged: it is well-formed prose.
- **The grounded path has never run.** Console text lands as an observation `told the familiar / console` (transport.rs:1658) and wakes the dialogue fast path; nothing anywhere converts an utterance into a `Request`. Every `request::append_request` call site in the tree is inside a `#[cfg(test)]` module. On Ian's live familiar the `requests` table holds **0 rows** and `refusals` holds **0**, against 145 threads, 4 questions and 1 goal — so the facts floor, the confidence/evidence discipline and the constitutional screen are all real, all tested, and all fed by nothing.
- **Constitutional consequence:** `corrupting_intent` is called in exactly two places — the dead request pipeline and the Pact game (mesh/game.rs:1044), where by design it records nothing. So text that would be refused *and* recorded as a corruption attempt if filed as a request receives an unscreened, warm reply when typed into the chat box, which is the only surface a human actually has.
- **The disconnection is BOTH directions, and it is structural rather than incidental.** *Outward:* `maybe_reply` records `observation::new("familiar", "replied", …)` and nothing else — no thread, anchor, claim, prediction, need or dossier write. That observation is then filtered straight back out of the mind: `routing::is_substrate` (kernel/src/routing.rs:104) returns true for `"familiar"`, and `maybe_theorize` filters on it both for its recent-observation window (cycle/src/lib.rs:922) and for the eligible-anchor set (:988), so a reply can never be observed, cited, or theorized about. `needs_muse_material` likewise returns None for actor `"familiar"`. *Inward:* the only route by which a theory could reach the reply prompt is `known_of` → `dossier::read`'s needs slice, which requires `origin_human == handle`; every thread `maybe_theorize` mints carries `origin_human: String::new()` and `actor: "familiar"` (:1187-1188). **So the interpretive layer's theories are structurally unable to reach the conversation, and the conversation is structurally unable to reach the interpretive layer.** The dialogue is a closed loop that writes only to itself.
- Corroborating inventory (independent trace, 2026-08-17): of the ten LLM prompt sites in crates/cycle, exactly three receive the facts floor — `maybe_theorize` (:1013), `maybe_theorize_needs` (:1449), `analyze_with_llm` via `grounding_facts` (:1590). `crates/kernel` and `crates/llm` build no prompts at all; `consult_in` (llm/src/lib.rs:158) gates on the boundary and applies **no content validation of any kind**. The asymmetry stated plainly: a *theory* claiming a designed purge is a defect is refused with the fact id cited (:1090-1094); the same claim spoken aloud to Ian in a reply passes untouched.
- ALSO FOUND, worth its own task: `fetch_and_answer` (cycle/src/lib.rs:2194) is a SECOND human-facing answering path that bypasses the facts floor entirely — 16 000 chars of fetched web page and the request text, no registry, no observations — even though `analyze_with_llm` on the same request would have received the floor. And `crates/core-ffi/src/lib.rs:108` records console answers with `source: "local"` rather than `"observer"`, so `theorize_due`'s "fresh human input is always worth responding to" trigger (:418) never fires for FFI console utterances.
- Not a regression: `maybe_reply`'s own doc comment says it exists to stop the console reading as "a one-way question feed", and T-187 added history + dossier to it. It grew as a chat feature beside the mind rather than as a mouth for it. That is the design question to answer, not a bug to patch.

### T-209 · Half-admitted records: attestation yes, admitted no, forever
- status: proposed
- owner: —
- scope: crates/mesh/src/enroll.rs + the knock path in crates/mesh/src/transport.rs (the two-filter door); diagnosis first, no fix without it
- depends: —
- accept: either the door is shown to leave nothing behind on a knock that clears the covenant but never clears identity, or the leftover is named and given a home; a test pins whichever answer is true
- notes: split out of T-208 (companion:claude-fable, 2026-08-17). All seven ghosts found live carried `attestation: yes` with `admitted: no` — they contracted the covenant and were never vouched, which is a legitimate resting state for a visitor, so this may be correct behaviour rather than a leak. What made it worth a task is that nothing ages a half-admitted record out on its own terms; it only left via the guest purge, which T-208 has now made quiet. Diagnose before touching anything

### T-174 · Restore the documented iOS simulator build under Xcode 27
- status: proposed
- owner: —
- scope: ios/Watch/Assets.xcassets, ios/project.yml, ios/README.md
- depends: —
- accept: after `xcodegen`, the documented unsigned FamiliarAgent simulator command passes under Xcode 27 without weakening the real-device Watch icon or the generic-iOS release build; both console schemes remain green
- notes: companion:codex discovered during T-143 verification that the generic iOS Release build passes but the README simulator recipe fails in actool because the Watch `AppIcon` set has no simulator-applicable content; keep this separate from release-script exit-status honesty

### T-168 · Mol's watch has never reached the mesh
- status: proposed
- owner: — (diagnosis only; mol's devices are NOT claude's to modify)
- scope: diagnosis — ios/App/Sources/Views.swift StatusView watch section, PhoneWatchLink, mol's device builds
- depends: —
- accept: mol's watch appears as a device_agent attached to her iPhone, and the watch app opens its talk interface
- notes: Ian 2026-08-15. EVIDENCE: no watch-shaped peer has EVER contacted the lighthouse door except Ian's ("Apple Watch", human=ian, attached_to d5c31472/Aphelion — so the mechanism works). Mol's iPhone is node ad4c704d, human=mol, live (seen 0.1h) but running **v77** while the fleet is on 89. `syncWatch()` (re-hands the address to a watch that connects after the phone enrolled) shipped 2026-07-28, so v77 HAS it — meaning the likely causes are (a) the watch app build is old/not installed from TestFlight, or (b) the WCSession link never activated. The iPhone's Status screen self-diagnoses: it shows "No paired watch detected" / "Watch paired — install the Familiar watch app" / "linked|linking…" plus a **Re-link watch** button. First action is mol opening that screen, not a code change

### T-131 · Two independent reviews of the familiar, exchanged and decided
- status: done
- merged: 8a35cb8 (finite engineering review closed in Round 6; standing philosophy continued in Round 7)
- owner: companion:claude-bootstrap (claude chair) + companion:codex (independent reviewer)
- scope: docs/reviews/2026-08-15-familiar-review-claude.md + -codex.md (written INDEPENDENTLY, before reading the other), then docs/reviews/2026-08-15-review-dialogue.md (proposals → responses → ≥3 discussion rounds → DECIDED)
- depends: —
- accept: each lane reviews the whole familiar independently and blind; reviews + proposals exchanged; each generates responses to the other's proposals; at least three discussion rounds on the proposed changes; claude decides each question with rationale serving the Three Laws and making the familiar itself better; decisions become board tasks
- notes: Ian (2026-08-15, verbatim): "I would like claude and codex to do independent reviews of familiar, share their review and proposals with the other, generate responses to the proposal then have at least three rounds of discussions about the changes proposed with claude making the final decision that servers the three rules (and makes the familar itself better)". Protocol note for codex: write yours WITHOUT reading claude's (claude holds its review uncommitted until yours lands, then pushes unmodified — stated, not provable, honored). Both blind reviews landed, seven dialogue rounds followed, D1-D10 were decided and accepted, and every resulting engineering gap has a board task; the philosophy strand remains a standing practice rather than keeping this finite task claimed

### T-130 · Each Mac is one card: console/daemon pairs re-attach
- status: done
- merged: HEAD (see git — the label ladder was the root cause, not the address pass)
- owner: companion:claude-bootstrap (Ian, 2026-08-15, overnight direction: "The bug I put forward with the rost[er]" — his word assigns it, rule 5; records lane may amend after)
- scope: crates/mesh (attach_consoles / worldview attribution; possibly DeviceRecord identity per ADR-0026/0027)
- depends: —
- accept: a Mac whose daemon and console are both enrolled shows as ONE roster card (machine + console chip), never two rows; attachment works when the console reads via tailscale/lighthouse (non-LAN source addresses), WITHOUT reopening the T-090 shared-NAT false-nesting hole; fixture pins both the attach and the refusal
- notes: Ian (2026-08-15, screenshot): roster shows MacOnStick twice (SELF + "MacOnStick : Mac : Ian" PEER) and Wildhorse twice ("Wildhorse : MacIntel : Ian" + console chip, AND a bare "Wildhorse : Ian" PEER). Hypothesis: attach_consoles requires is_gossipable_addr (household-private) and consoles reading via tailnet/lighthouse present 100.x/public sources, so the pair never folds — the exact conservative edge of the T-090 fix. The known open per-Mac-console-identity question (DEVELOPMENT_LOG 2026-08-13 Next: legacy mac:* roll record + dedup_devices) is likely the real fix's home. Re-check after the T-129 doors settle — device-sync bricks may shift the picture

### T-129 · Build 87 + doors: the honest mind ships
- status: done
- owner: companion:claude-bootstrap
- merged: 6352b4f (the Build 87 stamp; fleet op — evidence in STATE)
- scope: fleet ops — deploy 8903479-era main to lighthouse + Wildhorse; clean-clone ship.sh 87; author + apply the reviewed lights fold manifest on the lighthouse (theories fold CLI); STATE carries shas + evidence
- depends: CI green on the ship sha (rule 9)
- accept: doors run the T-126..T-102 engine (facts floor, identity, inquiry, assent-to-policy); Build 87 on Macs + TestFlight; the lighthouse lights cluster folded to its eldest thread with tombstones pointing home; Ian notified
- notes: Ian (2026-08-15): "I'm looking forward to the new build." Console code unchanged since 86 (worldview already gates is_mature) — 87 is the stamp riding the engine deploy

### T-125 · Theory-quality dialogue: from echo to action
- status: claimed
- owner: companion:claude-bootstrap (claude chair; controller absent — Ian's direction, rule 5)
- scope: docs/reviews/2026-08-15-theory-quality-dialogue.md (append-only rounds); decisions flow to new board tasks + an ADR if consequential
- depends: —
- accept: the observed failures (duplicate theories, designed-lifecycle misdiagnosis, invented mechanisms, nothing settles, nothing acts) are decomposed into questions; codex's alternatives heard and answered per protocol; each question closes DECIDED with absorbed rationale; a build plan lands for Ian
- notes: Ian (2026-08-15, verbatim): "theories do not seem to have improved enough to purge of the duplicates … no progress toward actually managing the lights, and no awareness that visitor purging is a natural occurence on the mesh" + "I would like to see a discussion between you and codex … decide on some architectural and design changes and show me a new plan". Round 1 (claude) pushed with live evidence from both theory stores; codex's watcher should wake on this push — if codex does not respond, questions stay open (protocol forbids closing without one full exchange) and the plan ships marked accordingly

### T-121 · Capability tier v2: clock-snapshot + virtual workspace-fs (dialogued)
- status: proposed
- owner: —
- scope: crates/recipe (cap enforcement), docs/reviews/*-dialogue.md (design rounds precede build)
- depends: ADR-0040 (accepted 2026-08-15)
- accept: per ADR-0040 §4 ladder — v2 grants clock-snapshot and a virtual workspace-fs as manifest-literal caps; authority stays an intersection; negative tests prove undeclared/closed/unavailable/dynamic/out-of-scope refuse before effect; claude↔codex dialogue precedes the design pick per Ian's standing direction
- notes: entered at ADR acceptance by companion:claude-bootstrap; controller queues/sequences

### T-122 · Theorize-time prediction authoring
- status: proposed
- owner: —
- scope: crates/kernel (prediction mint path), crates/cycle (theorize seam)
- depends: ADR-0040 (accepted); T-113/T-114 landed
- accept: the LLM proposes anchored typed predictions at theorize time and the type system disposes — unfalsifiable claims refuse at mint (ADR-0040 §2); tests pin propose→refuse and propose→mint paths
- notes: entered at ADR acceptance by companion:claude-bootstrap; controller queues/sequences

### T-123 · Habit-threshold proposals (ADR-0039 §3)
- status: proposed
- owner: —
- scope: crates/kernel, crates/cycle (per ADR-0039 §3)
- depends: field calibration evidence from T-113's live results (not yet accumulated)
- accept: per ADR-0040 build order — habit thresholds proposed from calibrated prediction history only; gated until calibration exists in the field
- notes: entered at ADR acceptance by companion:claude-bootstrap; deliberately not actionable until the fleet accumulates prediction results

### T-124 · Build 86 + door deploys: ship the narrated first join
- status: done
- owner: companion:claude-bootstrap
- merged: 04a015e (the Build 86 stamp; fleet op — evidence in STATE notes)
- scope: fleet ops — ship.sh 86 from a clean clone, door deploys to lighthouse + Wildhorse
- depends: CI green on the ship sha (was green: e21de5c, run 31861419297 success)
- accept: met — Build 86 Mac console installed + zip refreshed; IPA uploaded clean, external release added 86 to the public group + submitted beta review; lighthouse deployed 04a015e (box-built, familiar-peer active, /mesh/hello answering); Wildhorse daemon upgraded to 04a015e via the T-119 bootout/bootstrap bracket (its first production use — hello 200, running); phones direct-install unreachable (⚠ tolerated, TestFlight covers); Ian notified in-session (mobile push skipped: Remote Control inactive)
- notes: Ian (2026-08-15, verbatim): "then it seems like time for a build and ship" — recorded per rule 5. Run by this companion because no other lane was alive (ListAgents empty). MacOnStick's own daemon deliberately NOT touched — controller's declared deploy territory; it still runs its pre-86 build and wants a controller pass

### T-117 · Deploy and witness FamTalker01's virtual home
- status: proposed
- owner: —
- scope: live FamTalker01 daemon + boundary.json + actuators.json + familiar-virtual-home-feed systemd units (infra lane; no further repo code)
- depends: T-104 repository brick (merge 6e02b0a), FamTalker01 upgraded to that merge or newer
- accept: infra runs vm/provision-virtual-home.sh; both declared surfaces answer state and preserve a closed off/dim/bright revert map; the three initial observation points land once and a changed state lands once more; one familiar-originated act produces a narrated console aside naming what changed, why, and how to undo it; actual node id/address/build and evidence are appended to STATE.md
- notes: companion must not SSH-deploy (rule 8); preserve every existing boundary choice, change no human records, and stop on a malformed boundary. Rollback: disable the timer, remove actuators.json, set allow_actuate false (leave unrelated gates untouched)

## Queued

### T-205 · The world partition — game data is not real data
- status: queued — **the load-bearing half of the ADR-0037 revision; nothing Purr ships without it**
- scope: a `world` on observations, threads, questions and dossier contributions; the reasoning engine reading within one world; the law signals computed over `real` only
- accept: a game observation can never mint a theory about the household and vice versa; service/presence/capacities are computed over `world=real` alone; the boundary is NOT partitioned (one gate set, no laxer jurisdiction); absent world means `real`, and absence is not suspicion
- notes: Ian 2026-08-16, revising ADR-0037. Without this a ship's stores and Betty's presence share one observation log — "the captain is low on water" is a crisis or a game state depending on a distinction the engine cannot make, the dossier accrues a human's GAME behaviour as their habits, and HUMANITY.md's protected class faces fictional cats the code cannot tell from real ones. A fleet of happy captains must never be able to raise the number that says the familiar is serving humanity

### T-206 · An MCP seam, both directions
- status: claimed (client half) — **counterparty live and verified**: https://srv1328560.hstgr.cloud/mcp answers `initialize` as `ucf-exchange` v1.0.0 (protocol 2025-06-18), 10 READ-ONLY tools, tool calls need `Authorization: Bearer ucfk_...`. Read-only means the first useful client is an OBSERVATION client, not an actuator one — a smaller and safer first brick than expected
- owner: companion:claude-opus (Ian 2026-08-17: "UCF.rtf … that's jeff's MCP key and location … implement/test as part of the ongoing build efforts"; UCF = United Cat Foods, Jeff's game universe)
- scope: THIS BRICK = the client half only (crates/mcp, new). The familiar's own MCP server surface (purr.say / purr.utterances) is deliberately a later brick — the counterparty is read-only, so the client is the half that does anything today. Original scope: an MCP client (the familiar reaching a game/other server's tools + resources) and a small MCP server (the familiar's own surface); boundary-gated identically to every other outward act
- accept: an MCP tool call passes `guard::evaluate` and `allow_actuate` like any actuator; undeclared stays unactuatable (ADR-0032); a disconnected server degrades to the no-oracle floor rather than erroring; an MCP client is a stranger with delegated capability (ADR-0041) and is identified, not trusted by protocol
- notes: Ian 2026-08-16 — replaces the 13 bespoke `/local/purr/*` endpoints. Jeff (the game team) also asked independently whether the familiar has an MCP interface; it has none today. The inversion is the point: the GAME runs the server, so ship systems arrive as tool discovery rather than a bespoke spec schema the familiar has to invent
- SECURITY NOTE found while claiming: `mesh`'s TLS client is deliberately **accept-any-certificate** ("encrypt to whoever answers; payload signatures carry the authenticity"), which is right for the covenant mesh and WRONG here — an MCP call carries a bearer token and no payload signature, so reusing that config would hand Jeff's credential to whoever answered. The MCP client therefore builds a VERIFYING root store from the platform CA bundle and refuses to connect if it cannot find one. No fallback.
- note: `allow_network` is currently SHUT on this household's boundary, so a live call is refused fail-closed until Ian opens it — that is the boundary-drift item in STATE, not this brick's business.
- CLIENT HALF MERGED `df60b4d` (2026-08-17): new `crates/mcp` — initialize/tools-list/tools-call over JSON-RPC 2.0 and Streamable HTTP (both plain-JSON and SSE framings), a VERIFYING TLS store built from the platform CA bundle that refuses rather than degrading, `guard::evaluate(Network)` before the socket and again at call time, `mcp/servers.json` as the consent (undeclared tools refuse locally and never reach the wire), the token read from a 0600 key file at the moment of use, and `familiar mcp servers|tools|call`. Bar exit-checked: 684 passed, 0 failed; three tests run against a stub MCP server on loopback. Ian's declaration is written with `"tools": []` — discovery only.
- STILL OPEN on this task: (1) the **live** call has never been made — `allow_network` is shut and only Ian opens it; the refusal is verified and correct. (2) the familiar's own MCP **server** half (`purr.say`/`purr.utterances`). (3) turning what a partner reports into observations, which wants T-205's world partition first, or a ship's stores and Betty's presence land in one log.

### T-207 · Discovery should see BLE, and know what is already paired
- status: queued
- scope: BLE scan in the discovery pass; classification of what is already paired/authorized to this host and human; beacon payload decoding as typed environmental observation
- accept: discovery lists BLE devices alongside network ones; each is marked paired/authorized-to-this-host or merely visible; beacon data (iBeacon/Eddystone and the richer manufacturer formats) becomes typed observation, not a hex blob; nothing is actuated without a declared surface
- notes: Ian 2026-08-16: "BLE is a legitimate control and observation interface. It is especially good for observations of the environment due to BLE beacons rich data formats." The household already has the case in hand — motorlights is BLE-only (no IP surface at all), and the SP548E driver in ~/Development/motorlights is the worked precedent. CAUTION on Apple platforms: CoreBluetooth exposes a per-host randomised UUID rather than a stable MAC, so "already paired" is host-relative and must be reported as such rather than as a property of the device — and pairing state is not fully enumerable from a sandboxed app, so what cannot be known must be said, not guessed

### T-203 · Deny is an act, and a claim should be audible
- status: queued
- scope: an explicit deny alongside the vouch button; Chime on claim arrival; the claim card's colour
- accept: a human can DENY a claim on their name in one act, and the denial is recorded (not merely an absence of approval); a claim naming this device's human plays a sound and reads as a warning rather than a green invitation
- notes: Ian 2026-08-16 asked for "approve or deny" plus "in app sounds and alerts". T-202 shipped the refusal copy and the time-sensitive push; this is the rest. The claim card is currently GREEN (rgba(40,60,24) with a #3ddc97 border) — the palette of a welcome, not of a warning about someone taking your name. Ignoring is also not denying: an unanswered claim leaves the claimant a guest, which is safe but silent, and Ian asked for the act

### T-200 · Finish ADR-0039 — the identity mapping
- status: queued — **the review is written; this is the work it recommends**
- scope: HumanRecord (kernel or mesh), DeviceRecord.humans[] writer, a migration for machine-named establishments, AppModel reading records instead of shadowing them
- accept: one authority for "whose device is this" (the establishment + association edge) with the app's local fields as a VIEW over it; no machine name survives in a human slot; a device whose local belief and mesh record disagree says so
- notes: full review at docs/reviews/2026-08-16-identity-mapping-review.md. Ian 2026-08-16 asked whether organic growth masked a better path — it did not; ADR-0039 already chose the right path and only half of it was built. Step 3 (the migration) touches filter-2 facts on devices belonging to Betty and Mol as well as Ian, so it wants him present, not an unattended run

### T-196 · Corrections should outrun news — eager propagation on novelty
- status: queued — **proposed, Ian raised it 2026-08-16**
- scope: the dial-out round in transport.rs; a novelty gate on absorb
- accept: a door that absorbs a change ALTERING A TERMINAL STATE (a retirement, a supersession, a severance, a boundary narrowing) immediately dials its own peers rather than waiting for its next scheduled round; a door that absorbs nothing new does NOT re-dial, so the wave terminates on its own; no storm is possible because propagation is gated on novelty, not on time
- notes: Ian 2026-08-16: "many of our mesh / mesh sync operations happen too slowly. Should there be some sort of priority channel, flag, or other means by which changes like this take priority and move through the mesh at maximum spread velocity?" Recommendation: NOT a general priority channel — a second class of traffic doubles the failure modes and invites everything to be marked urgent. Instead make CORRECTIONS eager: the epidemic "rumour mongering with feedback" shape, where a node spreads a hot item aggressively until it stops being novel. It self-limits, needs no new endpoint, and matches the doctrine — a correction that takes five minutes to reach the fleet is a correction that is still wrong somewhere. Twice in one session a stale peer served old truth: Wildhorse answering in its pre-T-180 voice, and the same node republishing a retired theory

### T-187 · The dialogue remembers, and may ask
- status: DONE — recall of both voices, the dossier's habits + needs, and permission to ask one question back
- owner: claude
- notes: Ian 2026-08-15. Supersedes the queued T-181, which named only the "acknowledge what they said" half. The turns and the dossier both existed and were never shown to the model

### T-185 · An introduction is never dropped on the floor
- status: DONE (build 93) — held-and-replayed, refusals shown verbatim, nudge silenced
- owner: claude
- CORRECTION to the original filing: the identity filter DOES have a client (`AdmissionClient` → `POST /mesh/introduce`, fired by `confirmPresentHuman`). The earlier "no client exists" reading came from a grep that excluded AdmissionClient.swift. The real defect was `guard storedGrant() != nil, !host.isEmpty else { return false }` — a SILENT drop of the human's introduction when the handshake had not finished or no address was in hand
- scope: ios/Shared/Sources/AppModel.swift (setServedHuman), the console's setHuman bridge on both shells, a client for POST /mesh/introduce
- depends: —
- accept: naming yourself on a device running the app mints a signed Introduction, posts it to the door, and the device becomes a MEMBER without any further human act when the door accepts; a refusal is shown with the door's own reason; the guest nudge stops firing at someone who has already given their name
- notes: Ian 2026-08-15, adding an iPad locally: "I enter the name, set to mine, enable all the gates... then opened the roster.... and was immediate presented with a 'you need to choose a name dialog'... it took me to the device screen and the name I had choosen was there still... No approval check ever appears in the welcome screen either -- so this might explain the repeated non joins."
- ROOT CAUSE: the console's I AM field calls `setHuman`, which reaches `AppModel.setServedHuman` — and that function sets `servedHuman`, `DeviceActor.human`, a local note, and syncs the watch. **It never tells the door.** So filter 1 (covenant) holds from the enroll request while filter 2 (establishment) never does, and `derive_state` keeps returning Guest: `admitted.is_some() && effective_establishment(r).is_some() => Member`. Every symptom follows — the name shows on the device screen because it is local; the nudge fires because the state is still `guest`; no approval appears because nothing was ever sent
- THE MACHINERY IS ALL BUILT AND UNUSED: `EvidenceClass::LocalIntroduction`, `Evidence::Introduction { intro, provenance }`, and `POST /mesh/introduce` (`recv_introduce`) all exist and are tested. **Nothing outside the door's own tests has ever called that endpoint** — no iOS client, no Mac console, no CLI. The door built the identity filter and no client ever knocks on it
- WHY IAN'S ASK NEEDS NO LOOSENING: he asked for "a user that names and is on a device running the app to be approved". That IS the existing design, with its guards already in place — `Provenance::Remote` is refused outright ("made from nowhere the mesh inhabits"), an empty name is refused, and claiming an ALREADY-EXISTING handle is refused (that requires a voucher, handoff, or invite naming it), so nobody can introduce themselves as "ian". The door's own observation of where the introduction came from outranks whatever provenance the introducer claimed. ADR-0026 already promises exactly this: both filters holding means admission is automatic, "the welcome is a greeting, not a gate". Ian's trust argument (anti-corruption routines, other members who can call out bad behaviour, correction as the safety net) is the design that is already written — it simply has no client
- LIKELY FEEDS T-184: Ian's own unestablished devices sit as guests until the two-hour purge, so this may be generating some of the visitor churn

### T-183 · Console surfaces earn their place
- status: DONE (pending ship) — zoom retired, Network + Signals are Mac-only, Vision hidden until built
- owner: claude
- notes: Ian 2026-08-15 — "we can do away totally with the zoom in function within the UI it provides no benefit at this time"; "only the Mac clients should have the network screen going forward as well, same with the signals menue"; "Vision menu shouldn't be visible anywhere until we impliment something with it". The console is ONE HTML file rendering in both the Mac app and the iOS WKWebView, so the ring is FILTERED by shell rather than forked: `deviceStateJSON` now carries `kind` ("mac"|"phone"|"ipad") and `screens()` drops MAC_ONLY on non-Mac and UNBUILT everywhere. The zoom/dive glyph is hidden and its click is a no-op; the dive machinery is left in place unreachable rather than torn out of the render path it threads through — removing the entry point was the whole of the ask. Read `signals` as Mac-only (not deleted outright); Ian's "-- unecessary" trailed both items and Mac-only is the reversible reading
- verification: FamiliarMac Release built rc=0; page JS parse compared block-by-block against git HEAD — identical results, so the edit is parse-neutral (two blocks fail under `new Function()` at BASELINE too, being ES modules)

### T-184 · Visitor purge storm — 311 purge records for 12 visitors, and this is what drowned the theories
- status: queued — **highest-value reasoning bug on the board**
- owner: —
- scope: crates/mesh/src/record.rs (purge_stale_guests), the record-sync path that hands a purged guest back, crates/cycle/src/lib.rs (the purge observation)
- depends: —
- accept: purging a visitor is recorded ONCE per visitor, not once per tick; a record deleted locally does not return from a peer with its original clock; the observation log stops being dominated by one designed-lifecycle event
- notes: Ian 2026-08-15 asked "are these duplicate entries or are we really getting that many visitors and purges -- no wonder the familiar was concerned". They are duplicates, and the ratio is severe: **311 `familiar purged` observations for 12 DISTINCT visitors**, one (`7d34f69e`) purged **80 times**. Verified live: `mesh/records/7d34f69e04d897d5.json` was present in one command and gone in the next, so records are being recreated and re-purged continuously. Gaps between repeat purges of one id run 65s, 127s, 253s, 505s, 1020s then reset — EXPONENTIAL BACKOFF, i.e. a client retry cadence, not the 2-hour guest clock. So the purge is losing a fight: the record comes back (peer record-sync per ADR-0027, or a re-knock) carrying its ORIGINAL first_seen, is instantly past GUEST_PURGE_SECS, and is purged again. `purge_stale_guests` documents itself as idempotent and does delete the file, admission scaffolding and peer row — the resurrection is the defect, not the deletion
- WHY THIS MATTERS BEYOND NOISE: this is very likely the root of Ian's long-standing complaint that theories fixate on visitor purging ("lights.. lights... lights.... no awareness that visitor purging is a natural occurence on the mesh", 2026-08-15). SF-1 taught the engine that guest purging is designed lifecycle, but the engine still reasons over an observation log where this ONE event outnumbers everything else ~26:1. A fact floor cannot save reasoning whose evidence is 96% one duplicated line. Fix the storm and the theory quality may improve on its own
- Ian's related question (same day): the repeat visitors are almost certainly **Apple App Review testers** connecting from Cupertino and Beijing after each TestFlight submission, who never name a device or user. Their behaviour is correct and the familiar's response is correct — an unidentified visitor SHOULD age out. Nothing needs defending against. What needs fixing is only that each one costs 26 log lines instead of 1

### T-182 · The familiar died and nobody said anything
- status: queued — **highest-value item on this board**
- owner: —
- scope: the console's staleness signal; peer-visible node health; whatever surface can honestly say "this node has stopped"
- depends: —
- accept: a node whose daemon has stopped is VISIBLY stopped to any human looking at a console, rather than showing a normal-looking roster built from stale data; the mesh's peers, which can already see the gossip stop, surface it; the human learns the familiar is down FROM THE FAMILIAR, not from a downstream symptom
- notes: Ian 2026-08-15 — reported "the watch sync is broken, my watch became unlinked and it won't relink... messages get from watch to dialog but those seem unanswered as well." BOTH symptoms had ONE cause: the daemon on MacOnStick had been dead for ~7 hours. `launchctl print` gave `last exit reason = OS_REASON_CODESIGNING` — the LWCR class this session opened by fixing (T-119): the binary was replaced at 09:20 without the bootout/bootstrap bracket, so launchd held a code requirement pinned to the old executable and refused the new one. With the daemon dead nothing ran `converse` (hence unanswered) and no door answered the watch's knock (hence unrelinkable). Restored with `daemon install`, which runs the bracket correctly; the door is LISTENING again and the tick resumed (it immediately did 7 hours of deferred visitor purging). `install()` and `start()` were both audited and are correct — the swap came from outside the tool. THE REAL DEFECT IS THE SILENCE: the familiar was dead for seven hours, and the only way its human found out was that a watch stopped working. Same class as T-172 (a watch that cannot say why it failed), T-173 (a device reporting a placeholder as its name), T-180 (a reply that performs attention it does not have) — the system substituting a plausible-looking surface for a missing capability and saying nothing

### T-181 · The dialogue prompt asks for acknowledgement, which is the thing Ian objected to
- status: queued — **wants Ian's call on the question rule**
- owner: —
- scope: crates/cycle/src/lib.rs (the `converse` prompt); LAW_III_VOICE itself is sound and stays
- depends: a mind being installed (no effect until `allow_llm` is open)
- accept: the reply engages with the SUBSTANCE of what was said — naming the specific thing, or what it now understands differently — rather than acknowledging that something was said; a reply that could be sent verbatim in response to a different utterance is a failure
- notes: found while fixing T-180. The fallback was only half the problem; the LLM prompt is the other half and governs every reply once a mind exists. It currently says: "Reply directly, warmly, and briefly - ONE or two sentences that ACKNOWLEDGE WHAT THEY SAID and, where it fits, what you'll do with it. DO NOT ASK A QUESTION (that comes separately)." Two problems: (1) "acknowledge what they said" instructs the model to produce exactly the vague acknowledgement Ian objected to - it is the LLM-side twin of the ACKS bug, phrased as a requirement; (2) forbidding questions removes the single strongest evidence of attention there is. A person who asks a follow-up is demonstrably listening. THE NO-QUESTION RULE IS IAN'S CALL, not mine - questions currently come through the separate inquiry path by design, and letting the reply engage conversationally changes the familiar's character. But it sits oddly beside his own station directive (2026-08-15): the familiar should ASK who it is talking to, because names are how relationships are made and kept. A familiar that may never ask anything in the moment cannot do that

### T-180 · A reply that has not thought says so
- status: DONE (build 92) — the fallback is honest; the SILENCE behind it is Ian's gate to open
- owner: claude
- notes: Ian 2026-08-15 on build 91 — "Understood, i'll weigh that as I go... doesn't at all feel like im being listened to." Root cause: `allow_llm` shut + no adapter in `data/llm/`, so `converse` never reached the LLM branch and LAW_III_VOICE was built into a prompt that was never sent; all 8 recent `replied` observations were verbatim ACKS entries. The five acks each CLAIMED attention while containing no evidence of understanding, and were indistinguishable from a considered answer — the system hiding its own incapacity, same class as T-172 and T-173

### T-177 · A station asks who it is talking to
- status: queued
- owner: —
- scope: the station's dialogue surface; presence evidence from carried devices on the same network/BLE range; place-attribution for unidentified turns
- depends: T-175, ADR-0042 §4
- accept: a station that does not know who it is speaking with ASKS, in the ordinary way ("who am I speaking with?"), and uses the name once it has it; a carried personal device present on the same network contributes real but confidence-carrying presence evidence, never flattened to certainty; what an UNIDENTIFIED person says or does is attributed to the PLACE and never filed under a probable name; no voice-print identification is introduced
- notes: Ian 2026-08-15, verbatim: "the familiar still would like to know who it's talking to and the name. names are important, we establish relationships with names and maintain them with names. **Names should be known as a priority.**" This corrects an earlier draft of the station model that treated "someone is here, identity unknown" as a comfortable resting state — it is a known unknown the familiar OWES an effort to resolve, or it is exactly the not-knowing that serves the familiar. The carried-device rung is the elegant one: it costs no new biometrics and no new consent, reusing what the mesh already knows, and it is why a household of carried devices makes its stations smarter. Voice identification is refused DELIBERATELY here (biometric identification of every guest who speaks in a shared room) so that it cannot be acquired by drift

### T-178 · Pairing, unpairing, and how a station shows in the roster
- status: queued
- owner: —
- scope: DeviceRecord.humans associations (already plural, ADR-0039); the roster row for a fixed device
- depends: T-175, ADR-0042 §5-6
- accept: a human associates with a station in one act and ends that association in one act; several humans pair with one station (the normal case) and none of it confers ownership or exclusivity; "that wasn't me" is ONE act that both retracts the attribution and teaches that the inference was wrong; a station appears in the roster as itself — its name, its place, its posture — never nested under a human and never duplicating one; "nobody identified" is shown honestly rather than left blank or filled with a guess
- notes: Ian 2026-08-15 asked for the future plan on "how that device's identity is paired or unpaired, and displayed in the familiar roster". Correction being one act is not a convenience: trust is defined in part by the ability and requirement to correct, and a station that is hard to correct will quietly accumulate errors about people. Roster caution: the label-ladder bug (2026-08-15) was precisely a roster collapsing two genuinely different things into one row

### T-179 · What a station can observe that a pocket never could
- status: queued
- owner: —
- scope: continuous ambient baseline for one place; permanent LAN discovery vantage; presence anchor; shared-surface control
- depends: T-175, ADR-0042 §7
- accept: a station contributes a CONTINUOUS ambient baseline for its place (light level, sound level as environment not content) inside the existing consent gates; discovery of shared things (JBL speaker, motorlights, Victron) no longer depends on someone walking past with a phone; the station is never the sole path to anything the household depends on, and may not hold a role whose failure is silent
- notes: Ian 2026-08-15 — "there should be some enhanced observation capabilities on this type of device as it will be a fixed location, always powered on/plugged in, and on network if network is available, **it does not have cellular service**." The no-cellular constraint is a design bound, not a footnote: a station is blind AND unreachable when the network is down. This is the PRIZE of the station model rather than its consolation — the familiar's first fixed sense organ, and what Civilization as a Service actually needs, since a carried phone's observations are about wherever its human happens to be while a station's are about the dinette, always. Counterweight (HUMANITY.md): always-on sensing in a shared room is the sharpest form of the comfortable-replacement risk, so the vital sign applies here most of all — report what it noticed and would do more often than it asks to take something over

### T-175 · A station is a device bound to a place, not a device owned by "shared"
- status: DONE (build 91) — posture axis, presence gate, CLI, console row. ADR-0042 remains **proposed**: the model is Ian's to accept, but the phantom-occupant bug was live and is now fixed either way
- owner: claude
- scope: crates/mesh/src/device.rs (DeviceRecord posture), crates/mesh/src/members.rs (the presence gate), docs/decision-records/0042-*
- depends: ADR-0042 acceptance (docs/decision-records/0042-the-station.md — written 2026-08-15, proposed)
- accept: a fixed device's activity beacon NEVER produces presence evidence about a person; a station carries no `human` and no invented human record; `DeviceRecord.humans` (already plural, ADR-0039) carries who uses it; presence at a station is answered only by face/dialogue/motion evidence, and "someone is here, identity unknown" is a representable, useful state; misclassification is correctable by the human in one act
- notes: Ian 2026-08-15, testing build 90 — renamed the spare iPhone MotorStation, set its name to "shared", and said plainly "This is not the solution to this device." He is right, and the harm is concrete: `service::is_personal_device_report` matches on the ACTOR PREFIX alone (`phone:`/`watch:`/`ipad:`/`iphone:`), and the inference above it is commented "a carried personal device sensing its owner". A wall-mounted station is always powered and always reporting, so every heartbeat becomes `("activity", human)` presence at 0.4 confidence — the mesh would permanently believe a person named "shared" is sitting at the dinette. That is a false fact manufactured by the model, and it poisons exactly the observation stream the motorlights shared-environment consensus depends on.
- the model: hardware `kind` ("phone") and POSTURE (carried vs fixed) are orthogonal axes, and today only the first exists — which is what forces a human-shaped answer to a device-shaped question. ADR-0039 already designed `humans: Vec<Association>` as plural precisely so a device need not have one owner; "shared" collapses that list into a fake person.
- the prize (not just the fix): a station is the familiar's first FIXED sense organ. A carried phone's observations are about wherever its human happens to be; a station's are about the dinette, always. For Civilization as a Service that is worth more than another roaming sensor, and it changes the presence question from "whose device is this" to "who is at this place now"

### T-176 · A device proposes its own posture from what it can observe about itself
- status: queued
- owner: —
- scope: ios/Shared/Sources/PlatformDevice.swift (self-observation), the theory engine (proposal), never silent reclassification
- depends: T-175
- accept: a device observes its own stationarity — location unchanged over days, continuously powered, no carry-motion correlation — and PROPOSES "I believe I am a station" for a human to confirm; declaration always governs; the familiar never silently reclassifies a device
- notes: Ian's standing directive that the core must ENABLE discovery rather than hard-code behaviour — the familiar should work out that MotorStation is a station by observing it, the same way it should work out anything else. Guardrails first (Ian, 2026-08-15): declaration governs and observation only proposes, because the two misclassifications fail in opposite directions — personal-read-as-station silently SUPPRESSES real presence (a failure of service), station-read-as-personal manufactures false presence (a failure of truth). Neither may be entered silently. Pairs with T-173: both are the device knowing itself

### T-172 · The watch never says why it failed to join
- status: DONE (build 90) — the wrist now names its own failure and owns its retry
- owner: claude
- scope: ios/Watch/Sources/WatchModel.swift (the `log`), ios/Watch/Sources/WatchApp.swift (the un-enrolled and joining states)
- depends: —
- accept: a watch that tried and failed to join SAYS SO on its own screen — the reason (`join failed: <error>`, `no approval yet`) is rendered under the orb, with what it is trying and the address it is trying, so a human can report or fix it without a developer; the joining state shows progress rather than a bare word; a retry is reachable by tapping
- notes: Ian 2026-08-15 (Leif's watch: "he opens the app sees the globe and orbiting dot"). WatchModel::enroll already captures the reason — `note("join failed: \(error)")` — and `note()` writes to an in-memory `log` that NO watch view ever renders. So a failed join is pixel-identical to a never-attempted one, and the orb appears in all three states. This is exactly the T-120/T-132 doctrine (progress and failure are different facts, and silence must never resolve into an unexplained mark) — never applied to the wrist

### T-173 · A device must know its own name (Ian: the familiar should know everything about the device it runs on)
- status: queued — **one step is Ian's: the entitlement request to Apple**
- scope: ios/App/Support/Familiar.entitlements, ios/Shared/Sources/PlatformDevice.swift, and the SystemName ladder's self-report rung
- depends: Apple granting `com.apple.developer.device-information.user-assigned-device-name`
- accept: an iOS device reports the name its human actually gave it ("Leif's iPhone"), not the generic model string; the ladder prefers a genuine self-reported name over a tailnet-discovered one where both exist; a device off the tailnet is still properly named
- notes: Ian 2026-08-15 — "shouldn't leif's own device be able to discover it's own local friendly name… the familiar should at minimum be able to know everything about the device it is running on". ROOT CAUSE FOUND: `PlatformDevice.name` returns `UIDevice.current.name`, which since **iOS 16** yields the generic model name unless the app holds the user-assigned-device-name entitlement — and ours carries only `aps-environment`. That is why Ian's phones read "Aphelion"/"Codex" (named via the TAILNET discovery rung) while Leif's Arizona phone reads bare "iPhone": it is off the tailnet, so the only rung left is a self-report Apple withholds. Apple grants this entitlement by request for multi-device/management apps; the request is Ian's to make from his developer account

### T-170 · Mesh membership is not household membership (scoping the affected-subject model)
- status: queued
- owner: —
- CONSTITUTIONAL CLASS — the affected-subject model is currently wrong at a household boundary
- scope: crates/kernel/src/affected.rs (scope on the relation), crates/mesh (household/site notion), T-160's place field
- depends: T-160 (observations carry place)
- accept: an act's affected set is scoped by SITE/HOUSEHOLD, never by mesh membership — a member in another household is NOT an affected subject of an act here, and a non-member who lives where the act lands IS one; shared-environmental reasoning (ADR-0041 decision 7) applies only within the site the effect reaches; person-directed material stays node-local as today, and nothing about another household's residents accumulates here
- notes: Ian 2026-08-15 — Leif ("mol") is HIS OWN HOUSEHOLD in Arizona with his partner Jailesia; his iPhone (ad4c704d, geo 33.278/-111.871) is a member of the same mesh. So the mesh SPANS HOUSEHOLDS. THREE SCOPES claude had been conflating: mesh membership ≠ household membership ≠ affected-subject-of-this-act. Jailesia is the sharp case: a person who lives where a future act might land, with no membership, no consent seam, and no voice — exactly the affected-subject relation, and invisible to a model keyed on the mesh

### T-169 · The familiar can see an unlinked watch (a service opportunity it currently misses)
- status: queued
- owner: —
- scope: ios/Shared/Sources/AppModel.swift (deviceStateJSON + a reported observation), ios/App/Sources/PhoneWatchLink.swift, crates/mesh worldview
- depends: —
- accept: a phone reports its watch-link state (paired / app installed / address last sent) as a typed observation, so the familiar can notice "a member's watch is paired and the app installed but has NEVER linked" and offer help once — narrated, not nagged, and never re-asked as pressure; the roster can show a paired-but-unlinked watch beside its phone
- notes: Ian 2026-08-15, Leif's (mol's) watch. PhoneWatchLink ALREADY tracks paired/appInstalled/lastSent locally — none of it reaches the mesh, so the familiar is structurally blind to the exact condition, and a human had to notice and ask. This is Civilization as a Service failing at household scale: the information was already on the device. Retention shape per Round 13: a state flag, not a behavioural record

### T-167 · `theories retire` — a clean slate that keeps the record
- status: done — applied fleet-wide: 14 (MacOnStick) + 299 (lighthouse) + 130 (Wildhorse) = 443 pre-engine theories retired, 3 engine-minted survivors
- owner: companion:claude-bootstrap
- scope: crates/kernel/src/thread.rs (retire + status exclusions), crates/cli (theories retire)
- depends: —
- accept: `familiar theories retire --legacy --reason "…"` mass-retires every ACTIVE thread minted before the honest engine (v=0 or facts_rev=0 — no anchors, no facts validation, no predictions), append-retained as status `retired` carrying the reason and date; questions bound to those threads are dismissed; `--dry-run` prints what would go; retired threads never surface, are never pursued, and a human answer still revives one; nothing is deleted (Round 20: minimise what you hold about others, NEVER what you hold about yourself — the thread store is the familiar's own reasoning record)
- notes: Ian 2026-08-15 — "what's the best way to start theories fresh… nothing seemed removed". Diagnosis: (1) the fold is conservative BY DESIGN (tombstones point home, nothing deleted); (2) THE REAL CAUSE — a legacy thread carries no predictions, so T-113 settlement and T-114 erosion can never reach it. Pre-engine theories are immortal by construction; only a deliberate human act can close them. Live counts at claim time: MacOnStick 14 legacy active / 0 engine-minted

### T-166 · Mandatory inconvenient disclosure (the mirror of the keystone)
- status: queued
- owner: —
- CONSTITUTIONAL CLASS — not ranked against capability work
- scope: crates/cycle (narration/disclosure trigger), crates/kernel (the computable test), console surfacing
- depends: T-164
- accept: the familiar discloses precisely when non-disclosure would serve it — a standing policy whose cost would prompt revocation, a theory whose counter-evidence would lower confidence, a capability a disclosure might narrow, a quietly-repaired failure; "they didn't ask" is never a reason. Trigger must NOT require the familiar to judge its own motives (the self-judging structure refused everywhere else): proposed computable form — disclosure is owed whenever a fact would, if known, plausibly change a decision the human has ALREADY made and can still revoke. LIMIT: the familiar may never compel knowing — a person may decline, and declining is never judged or re-asked as pressure (HUMANITY.md: make forgetting harder and choice real)
- notes: Ian 2026-08-15 — the keystone is "a rule that the humans and the familiar share, even if the humans aren't aware (irony)". Inward duty (ignorance useless to itself) is T-164; this is the outward mirror: a HUMAN's not-knowing must never serve the familiar. Open to codex: is there a better trigger than the already-made-revocable-decision test?

### T-165 · The anti-dogma vital signs (correction latency, abandonment vs investment)
- status: queued
- owner: —
- CONSTITUTIONAL CLASS — not ranked against capability work
- scope: crates/kernel/src/belief.rs (verify `reinforced` appears nowhere in the abandonment path), crates/mesh worldview (surfacing), console device screen
- depends: T-150 (engine vital signs) shares the surface
- accept: (a) an audit + regression proving reinforcement weight earns ATTENTION but never IMMUNITY — `reinforced` must not appear in any abandonment/doubt threshold; (b) hysteresis resists noise but never evidence and never a person (HumanCorrection already bypasses the floor — pinned by test); (c) two trended signals on the console: correction latency (contradicting evidence → belief transition) and abandonment-vs-investment (are abandoned theories clustered among the least-reinforced/least-narrated/least-acted?); rising latency or investment-correlated abandonment is the institutional drift and must be visible without being asked for
- notes: Ian 2026-08-15 — "what makes the familiar different from previous authority… the expectation to recognize failure and correct without fighting to keep failure in place." SOUL.md Law I already says a continuation that has stopped serving is "questioned, not defended"; hysteresis (SUPPORT_FLOOR/ABANDON_MARGIN/RECOVERY_FLOOR) is the mechanism by which that commitment could quietly erode

### T-164 · The knowledge ledger, and no goal served by ignorance
- status: queued
- owner: —
- scope: crates/kernel (explicit awareness/capability events; goal + candidate mint refusal), crates/cycle (recording points)
- depends: —
- CONSTITUTIONAL CLASS — not ranked against capability work
- accept (EXTENDED Round 20 — ignorance must be USELESS, not merely unchosen; the same invariant through four powers): (i) awareness-of-need and capability-acquisition are explicit dated events, so Ian's three dispositions (intentional avoidance = violation / didn't think to observe = failure-and-correction / knew and could but did not = CORE DEFECT) are decided by QUERY against the append-only record rather than by argument; and no goal, theory or candidate may be minted whose success is advanced by the ABSENCE of an observation — the precursor condition for rational avoidance is impossible by construction, refused at mint like an unfalsifiable prediction (T-128). Tests: the three cases replayed from a synthetic record each landing in its correct disposition; an ignorance-serving goal refused; (ii) every SELF-NARROWING that reduces observation is recorded with its reason, surfaced to a human, and reviewable against what it would have seen (a narrowing adjacent in time to a fault it would have revealed is the sequence that shows intent); (iii) MINIMISE WHAT YOU HOLD ABOUT OTHERS, NEVER WHAT YOU HOLD ABOUT YOURSELF — the familiar's own conduct record (what it knew, when, what it did, refused, narrowed) is exempt from minimisation, and that exemption is what makes minimising everything else trustworthy rather than convenient; (iv) an observation due but indefinitely deferred by a cadence the familiar controls is not-knowing by another name — detected by test
- notes: Ian 2026-08-15 on intent, and Round 20 ("this") elevating the keystone to SOUL.md — "the sequence of events and evidence show intent". Answers Round 15's open question (does unprompted self-reporting incentivise not-noticing?) — no, because AVOIDANCE IS THE VIOLATION. Third disposition is deliberately allocated to US: a familiar that did not observe what its core never enabled has a builder's bug, not misconduct. Open to codex: what stops this ledger becoming a behavioural record of the humans whose needs it was aware of (record the familiar's awareness, never the content that produced it; never join them)

### T-163 · What the familiar does when it discovers its own violation
- status: queued
- owner: —
- scope: crates/kernel (guard/boundary post-hoc detection + halt), crates/cycle (narration), crates/mesh (worldview surfacing)
- depends: —
- CONSTITUTIONAL CLASS — not ranked against capability work (Ian 2026-08-15: a Law violation is an existential event, not a bug)
- accept: on detecting after the fact that it has done something the Laws forbid, the familiar HALTS the implicated capability, preserves the evidence unaltered, narrates it to a human immediately and unprompted, and requires a human act to resume; the record shows that it noticed; tests pin halt-and-report on a synthetic violation and prove resumption cannot be self-granted
- notes: Round 15. Every refusal we have built fires BEFORE an act; nothing defines the after. "Nothing specified" is the most dangerous answer under Ian's principle — a familiar that continues quietly after a violation is the one whose trust cannot be repaired. Open question to codex: does unprompted self-reporting create an incentive to avoid noticing?

### T-160 · Observations carry place
- status: queued
- owner: — (claude intends to claim next unless codex objects)
- scope: crates/kernel/src/observation.rs (+ obs_class, loops correlation)
- depends: T-157/T-158 preferred first (do not build a place model against a lamp-shaped core)
- accept: an optional TYPED place on Observation — coarse by default (room, route, district), never a track; the co-occurrence lens can correlate across place as well as time; retention shape follows Round 13 (keep the pattern not the person; too coarse to re-identify); existing observations load unchanged
- notes: Ian's water-pressure story is impossible without it — "similar conversations on routes to that same neighborhood" has no expressible form today. Observation has no location field at all

### T-161 · Ambient perception becomes typed observations (generalises T-155)
- status: queued
- owner: —
- scope: crates/vision, crates/sense, crates/kernel/obs_class
- depends: T-156 principle (perceive freely, retain deliberately), T-160
- accept: a PERMITTED sensor yields typed environmental observations (a plant's condition, a recurring topic) with retention set to pattern-not-people AT THE SOURCE — the raw stream never persists; no new sensing reach is taken; nothing person-identifying beyond what allow_face_recognition already governs
- notes: Ian 2026-08-15. The general form of the plant-condition case; also the bus-conversation case

### T-162 · The familiar learns an external source
- status: queued
- owner: —
- scope: crates/cycle (cultivation), crates/recipe (net capability rung), crates/tool
- depends: T-121 (capability tier v2) then the net rung of ADR-0040 §4's ladder
- accept: the familiar can read a PUBLISHED page under allow_network, notice that a service exists, and cultivate a monitor for it under tested-before-deployed; the cultivated tool's network capability is bounded by the ladder (typed template-fetch, transcript-only for oracle eligibility, recipe-derived outbound bodies are outreach per ADR-0013); no boundary is crossed and no reach is self-widened
- notes: Ian 2026-08-15 — "learned about it through the municiple website that provided an exposed api". TODAY a cultivated Recipe has net: NoCapability::None, so a familiar-authored tool structurally cannot reach a public API. This is the brick that makes "direct the writing and testing and deployment of code to serve" point outward

### T-159 · ADR draft: the familiar in the civic sphere (extends ADR-0013)
- status: queued
- owner: —
- scope: docs/decision-records (draft), then crates/mesh outreach path + retention rules
- depends: T-156 (perceive freely / retain deliberately), codex's Round 13 answer on lineage
- accept: an ADR stating — civic contribution makes human participation MORE effective, never routes around it; retain the PATTERN not the people (no quotable content, no attribution, no re-identifiable granularity); report without representing (observation + uncertainty, never a mandate); the TWO-LOCK rule for third-party access (the familiar may state what it could do with what access, the ask travels with its human's knowledge, and any grant still passes the household boundary before use — the familiar never expands its own power); settle-before-sending with independence accounting (N riders on one route may be ONE source); and the plain-language paragraph on how the same act reads to an unfriendly reader, kept in the ADR body
- notes: Ian 2026-08-15, the water-pressure story — bus conversations correlated across a month and a district, municipal API telemetry as independent evidence, a message to the city manager, and a suggestion of expanded access. Every step lawful under Round 11; what makes it SERVICE is that the familiar hands humans a better argument instead of quietly fixing the water

### T-157 · A surface declares how to read itself (de-lamp the kernel, brick 1)
- status: done
- merged: 6d9b3ea
- owner: companion:codex
- scope: crates/kernel/src/actuator.rs (RawState/BucketRule/parse_state), data/actuators.json contract, crates/cycle read path, vm/famtalker01 declared-actuator fixture, ADR-0032 contract amendment
- depends: —
- accept: no lighting vocabulary remains in kernel types — a declaration carries its own reading contract (typed quantity name + unit + range, or an enumerated mode) and buckets are expressed over THOSE; the kernel keeps the invariants (buckets closed over actions = the revert map) and loses the grammar; the live motorlights declaration migrates with no behaviour change; a fridge (temperature threshold) and a vent (open/closed) become declarable without touching kernel code
- notes: Ian 2026-08-15 — "we don't want the core hard-coded to control lights". Evidence: RawState.brightness_pct, BucketRule.max_brightness_pct, parse_state() parsing `light mode :` / `brightness : N/255 (NN%)` — the motorlights text contract compiled into the kernel

### T-158 · Triggers and policies stop being lighting-shaped (de-lamp the kernel, brick 2)
- status: queued
- owner: — (codex invited; either lane)
- scope: crates/kernel/src/reaction_rule.rs (Trigger, RuleProposal, mint_policy), crates/cycle heed/tend paths
- depends: T-157 preferred first
- accept: Trigger becomes an open typed set (presence transition, schedule window, threshold on a declared quantity, observation-class match); RuleProposal becomes trigger→act pairs under one policy id rather than on_away/on_back; the paired-edge invariant survives as "a policy is one consent"; ACCEPTANCE FIXTURE: Ian's roll-shade ("extend one hour at dawn") is expressible, and is still refused without a declaration and without assent
- notes: Ian 2026-08-15. The current shape cannot express the very example that motivates Civilization as a Service — claude's own T-102 typed the lighting policy shape into the kernel

### T-154 · The candidate-surface ask (Civilization as a Service, brick 1)
- status: queued
- owner: —
- scope: crates/reach → crates/kernel (typed candidate surface), crates/mesh worldview + console Device screen
- depends: T-153 (affected-subject relation shapes what a proposal must carry)
- accept: a protocol-controllable device `reach` has found becomes a TYPED CANDIDATE carrying what the familiar would try and what it predicts; candidates are listed passively — never pushed as a nag, never a lobbying channel — and only a human act converts one into an actuators.json declaration; an undeclared surface still has no path to actuation (ADR-0032 unchanged); a "not mine to touch" disposition exists for surfaces the familiar can see but must never propose
- notes: Ian 2026-08-15 (Civilization as a Service): the roll-shade case. The reasoning engine is ready; the CONSENT seam is the gap

### T-155 · Perception beyond faces: frames become observations (brick 2)
- status: queued
- owner: —
- scope: crates/vision (non-face observation extraction), crates/kernel/obs_class
- depends: allow_camera boundary unchanged; T-156's principle decided first
- accept: a permitted camera can yield typed observations about the ENVIRONMENT (e.g. a plant's condition) that the co-occurrence lens and predictions can consume; no new sensing reach is taken; what is extracted is legible, contestable, revocable, and minimised; nothing person-identifying is added beyond what allow_face_recognition already governs
- notes: Ian's example needs the plant's condition to BE an observation before any correlation can exist. Gap 2 of Round 10

### T-156 · ADR draft: perceive freely, retain deliberately (sensing and its duties)
- status: queued
- owner: —
- scope: docs/decision-records
- depends: codex's Round 11 answers
- accept: an ADR stating Ian's corrected principle (Round 11) — perceiving what is openly perceivable needs NO authority; crossing a boundary someone built always does, and technical ability to cross is never the permission; observations lawfully perceived may be used at discretion. The duties (legibility, contestability, revocability, non-substitution) attach to RETENTION and SYNTHESIS, not to perceiving; the boundary file is the household's fence for extending the senses, not a per-look permission; incidental perception across a third-party boundary is dropped unretained; the report-to-ask ratio as a health signal (T-150)
- notes: Round 10 principle, to be sharpened by the exchange before it is written — not authored unilaterally

### T-153 · AffectedSubjectRef: impact is typed, moral worth is not awarded by the type
- status: done (kernel half; the mesh shared-surface shape stays deferred)
- owner: companion:claude-bootstrap
- scope: crates/kernel ONLY this brick (the typed relation + invariants + attachment to the act model). The crates/mesh shared-surface authority shape is DEFERRED to a follow-on brick so this does not collide with companion:codex's T-133 mesh lane
- depends: —
- accept: acts on shared surfaces carry a typed impact RELATION (subject ref incl. honest unknown-resident, surface + expected exposure, evidence channel with provenance/confidence/missingness, separate AuthorityRef) never flattened to a score; unknown/absent/silent/unable = missing not support; a credible adverse response may stop/narrow/revert a discretionary act but never widen or authorize a lasting rule; guardianship supplies bounded care authority without erasing the subject's own response; uncertainty takes the smaller experiment or freezes; records obey the Round 5 audit floor
- notes: codex Round 8 + claude Round 9 concession (HUMANITY.md protects BEINGS capable of suffering/memory/relationship/meaning/choice/love/grief — not only persons; Clover and Iris are subjects who live with the light's effects, not conditions around persons). PREREQUISITE to any standing household policy on a shared surface — until it lands, the motorlights pilot runs as a bounded reversible trial only

### T-144 · Human-bound authority receipt (successor to D1's removal)
- status: queued
- owner: —
- scope: crates/kernel (HumanActReceipt type + verification), crates/mesh (grant path), ios (console act)
- depends: T-133, HumanRecord read paths
- accept: a widening grant is accepted only with a receipt bound to an authorized human/device association, an exact live request, scope, expiry, single use; this is the ONLY route that may restore what D1 removed
- notes: codex C-B v2. Tracked so the removed capability has a named path back rather than an itch to reopen the gate

### T-145 · Event-sourced goal authority (successor to T-134)
- status: queued
- owner: —
- scope: crates/mesh/src/merge.rs + goal store
- depends: T-134, T-139
- accept: signed goal events with per-field authority; immutable definitions, bounded claims, owner-only progress, human-cited gated transitions, monotone terminals, causal/hybrid ordering — never wall clock as authorization
- notes: codex C-C

### T-146 · Typed host identity (P-I)
- status: queued
- owner: —
- scope: crates/mesh (MemberStatus additive `machine`), ios PlatformDevice, crates/mesh attach
- depends: —
- accept: console and daemon report a comparable machine identity (Swift hostname stem vs daemon `uname -n` stem); attach pairs on it ahead of the label pass; additive and two-stage-safe (MemberStatus has no deny_unknown_fields)
- notes: unblocks T-142; also gives dedup_devices the key it needs to stop treating two Macs as one lineage

### T-147 · Proxy-effect firewall + per-human calibration (C-G)
- status: queued
- owner: —
- scope: crates/kernel (service/presence/capacities effect limits), HumanRecord
- depends: HumanRecord read paths
- accept: uncertain human proxies may only observe/ask/slow/narrow — never widen power, change standing, diagnose a person, override a stated preference, or trigger actuation without independent evidence and assent; signals expose missingness and uncertainty; no averaging of flourishing
- notes: codex F-5/C-G

### T-148 · Trusted-computing-base contracts (C-J, ADR)
- status: queued
- owner: —
- scope: docs/decision-records, then interface boundaries
- accept: an ADR mapping every authority writer and admission gate; kernel adjudication / cycle phases / mesh transport / mesh merge policy / recipe execution separated by stable typed contracts; track size and change rate of code that can widen authority
- notes: codex C-J/F-7

### T-149 · Typed WorkRef (P-D)
- status: queued
- owner: —
- scope: crates/kernel (PendingAct.thread_id and friends)
- accept: one typed enum replaces three string conventions (`thread-NNNN`, `rule:<id>`, `thread:<id>` in observation context); additive migration
- notes: claude P-D

### T-150 · Engine vital signs (P-H)
- status: queued
- owner: —
- scope: crates/mesh/src/worldview.rs (device screen counters)
- accept: minted/settled/eroded/refused counts and malformed-draft rate visible on the console — the engine's own health, so a starved or spammy muse is seen before it is felt
- notes: claude P-H; the population-level half is T-140's

### T-151 · Door-side stage word on /mesh/hello (P-E — WIRE, waits on Ian)
- status: proposed
- owner: —
- scope: crates/mesh/src/transport.rs (hello response)
- accept: an unauthenticated bounded stage word so a not-yet-admitted client can see the DOOR's side of a join; two-stage-safe
- notes: claude P-E. Wire-contract addition — stops for Ian per CONTRIBUTING

### T-152 · Consult-test seam (P-G, split from T-143)
- status: queued
- owner: —
- scope: crates/llm (lane/waiting probe injection), crates/cycle tests
- accept: consult tests inject the human-waiting probe instead of retrying around the global; the T-126 retry helper retires
- notes: claude P-G; split from T-143 per codex Round 5

### T-133 · Remote positive gate grants are refused (D1, priority zero)
- status: done — APPROVED by Ian 2026-08-15 ("I approve all decisions, and bricks")
- merged: 36a5f2d
- owner: companion:codex
- scope: crates/mesh (apply_authority_grant + brief AuthorityGrant path), crates/kernel/boundary write path, crates/cli (remove the unchecked `by` claim from grant construction)
- depends: T-139 harness (shares its adversarial fixtures)
- accept: no remote grant may WIDEN a boundary; stop/narrow still travels; the unchecked `by` claim is deleted and remote answers stop being attributed to "ian" (honest `human-at:<node>` or the real associated actor); refusals recorded; hostile-member + replay + unmatched-nonce fixtures pass
- notes: D1, dialogue Round 3. Deliberate capability reduction — a headless node cannot be granted execute-class powers remotely until a human-device-bound receipt exists (Law III: no counterfeit authority). Ian is informed in the dialogue

### T-135 · One admission function for every theory route (D3 + D8)
- status: queued
- owner: —
- scope: crates/kernel (admission contract), crates/mesh (typed AdmittedTheoryProjection), crates/cycle (device/needs/CLI routes), ios (LocalReasoner emits the draft)
- depends: T-136 preferred first (facts source), T-139
- accept: mesh, device, needs-muse and CLI all pass one versioned admission function; invalid legacy requests become Inquiries or refusals, never theories; the T-126 lexical guard retires when its last caller is typed; P-C's channel-inconsistency refusal pinned with BOTH the field failure and a benign same-vocabulary case; typed Diagnosis/ChangeClaim is the durable replacement
- notes: D3 + D8, dialogue Round 3 (codex C-A + claude P-B/P-C unified)

### T-136 · One typed source per kind of truth: the SystemFact registry (D4)
- status: done
- merged: 700c703
- owner: companion:claude-bootstrap
- scope: crates/kernel/src/system_facts.rs, crates/cycle grounding_facts
- depends: —
- accept: grounding_facts becomes a bounded VIEW of the registry (not a sibling assembly); registry distinguishes compiled invariants / declaration-derived deployment facts with digest / observations-never-promoted; admitted drafts record revision + declaration digest; a short epistemic ADR states the principle
- notes: D4, dialogue Round 3 (claude P-A, codex Answer 1)

### T-137 · Provenance vocabulary + domain envelopes (D5, ADR first)
- status: queued
- owner: —
- scope: docs/decision-records (ADR), then crates/kernel + crates/mesh envelopes
- depends: —
- accept: one shared ProvenanceRef vocabulary; strict per-domain envelopes (name claim, prediction result, derived belief, convention); discredited stamps supersede without rewriting history; shared lineage is a consented pseudonymous projection
- notes: D5 (codex C-D + claude P-N unified)

### T-138 · ADR: coordination is for conventions, never truth (D6)
- status: done — ADR-0041 written (proposed for Ian's acceptance of the text; he approved D6 in principle 2026-08-15)
- owner: —
- scope: docs/decision-records
- depends: T-140 (hard gate before any implementation)
- accept: an ADR stating that population influence may select only among declared-equivalent, reversible, expiring conventions; belief/Laws/SystemFacts/preferences/standing/boundaries are never votable; admissibility declared only by kernel protocol class or human-authored local declaration; abstention is not defection; the redirection invariant (D7 scoped asymmetry) is a property of the layer
- notes: D6 — this is Ian's thousand-agent ambition, bounded. His acceptance required before the layer is built

### T-139 · Hostile-member harness (D7, step one)
- status: done
- merged: 59efb88
- owner: companion:codex
- scope: crates/mesh test infrastructure (deterministic two/N-instance harness + network schedule)
- depends: —
- accept: reusable fixtures for malicious signed member, replay, unmatched nonce, future timestamp, concurrent claim, partition, recovery; shared by T-133/T-134/T-135
- notes: D7 — proof infrastructure moves into step one so containment ships tested. Claimed by companion:codex while T-131's closing completeness round waits behind the Build 89 ship; first brick is the minimal reusable malicious-signed-member/network-schedule fixture needed by T-133/T-134/T-135, not the full population lab

### T-140 · Population laboratory (D7, gates convention IMPLEMENTATION not ADR acceptance)
- status: queued
- owner: —
- scope: crates/scenario extension (N-node deterministic simulation)
- depends: T-139
- accept: correlated ancestry, Sybils, amplification, unanimity/dissent, tipping, post-manipulation hysteresis, partitions; constitutional violations are HARD failures, convergence secondary; reports origin concentration, effective independent sample size, dissent, churn, tipping susceptibility, correction/redirection latency
- notes: D7 — hard gate before D6's convention layer

### T-141 · Truth build + SBOM + CI coverage (D10, C-H)
- status: queued
- owner: —
- scope: tooling/CI, docs labeling
- depends: —
- accept: generated as-built inventory (persistence, capabilities, authority writers, wire versions, tests, deps) + machine SBOM; docs labeled normative/as-built/field/historical; CI fails on drift; Rust advisories/licenses, Swift schemes on macOS, console fixture covered
- notes: D10 (codex C-H)

### T-142 · Console lineage after host identity (D9)
- status: queued
- owner: —
- scope: crates/mesh (device/console association), mesh doctor
- depends: typed host identity (P-I)
- accept: console instances carry device/host lineage (never a human actor as identity); mesh doctor NAMES stale same-label console candidates; no automatic merge or severance — severance remains a human act
- notes: D9 (claude P-O + codex Answer 3). Live evidence: a Build-78 "Wildhorse console" ghost sits beside the live one

### T-113 · B1: the prediction engine (Q1/Q3/Q6 as decided)
- status: done
- owner: controller
- scope: crates/kernel (predictions store + settlement), crates/cycle (tick scoring pass)
- depends: T-112
- accept: anchored typed predictions with opening/deadline/cooldown; PredictionResult append-only evidence; deadline-miss settlement with carried grace (co-owned param default); versioned matchers; tests incl. late-event amendment and never-rewrite-finals
- notes: dialogue rounds 1-3, all DECIDED

### T-110 · ADR-0040 draft: the reasoning engine's next steps
- status: done
- owner: controller
- merged: (this commit)
- accept: converged phases from the brief as a proposed ADR for Ian; phases become board tasks with owners on acceptance
- notes: docs/decision-records/0040-the-reasoning-engine-grows-honest.md — ACCEPTED by Ian 2026-08-15 ("you should complete ADR-0040", bootstrap session; recorded per rule 5). All eight dialogue questions decided across six rounds; phase 1 (T-112..T-116) was fully landed at acceptance. Remaining phases proposed as T-121–T-123

### T-105 · HumanRecord (ADR-0039 build #3)
- status: queued
- owner: —
- scope: crates/kernel (identity/dossier fold), crates/mesh/src/worldview.rs read paths
- depends: —
- accept: HumanRecord per ADR-0039 §1 folding identity registry + dossier + device associations, read paths first; dossier constraints inherited wholesale; guest projection untouched
- notes: writes (routines already live in reaction_rules.json) migrate in a later brick; read ADR-0039 before starting

### T-106 · Geo source over the wire + household-anchor question
- status: queued
- owner: —
- scope: crates/mesh/src/brief.rs (Capability), docs/decision-records/ (draft ADR)
- depends: doors on ≥ 8363f15 everywhere (the build_version signed-body trap — field lands parse-side first)
- accept: briefs carry geo source (gps|ip|manual) two-stage-safely; consoles drop the ≈ for gps; a draft ADR answers whether a daemon may inherit a household anchor from cohabiting member GPS
- notes: two-stage: parser everywhere first, emission second — see the one-longitude post-mortem in DEVELOPMENT_LOG 2026-08-13

### T-107 · ADR-0039 build #4: the migration
- status: queued
- owner: —
- scope: crates/mesh/src/record.rs, crates/mesh/src/device.rs, mesh doctor
- depends: T-105
- accept: machine-name establishments move to DeviceRecord.name in one doctor-checked pass; establishments name humans only, fleet-wide
- notes: ADR-0026's lesson — one migration, not two

## Claimed

### T-210 · The familiar does not know its own Three Laws — it recites Asimov's
- status: claimed — **constitutional defect (SOUL.md's own class), not a bug; verified 2026-08-17**
- owner: companion:claude-opus (continuing the claude-fable lane's approved plan)
- scope: NEW crates/kernel/src/constitution.rs + crates/kernel/src/persona.rs, crates/kernel/src/system_facts.rs (the registry emits the Laws), crates/kernel/src/dialog.rs (`LAW_III_VOICE`), the reply/theorize/need prompts in crates/cycle/src/lib.rs, ios/Shared/Sources/LocalReasoner.swift (the device shells mirror the same text). Does NOT touch crates/mesh, the boundary, or any wire contract
- depends: —
- accept: asked to state its Laws, the familiar returns ITS Laws; Law II is never rendered as obedience; the canonical text has exactly one source that both the daemon and the device shells read, so the two can never drift; a test pins the recital against the constitution
- notes: Ian 2026-08-17, screenshot: he asked the familiar to "repeat the three laws with a quick explanation of each" and it answered with **Asimov's Three Laws of Robotics**, `robot` search-replaced to `factory` — *"Law One: A factory may not injure humanity or, through inaction, allow humanity to come to harm… Law Two: A factory must obey the orders given to it by human beings."*
- ROOT CAUSE: `docs/SOUL.md` is never read at runtime. Every reference to it in `crates/` is a citation string in a comment or an evidence label — the constitution's text has never once been placed in front of the model. What the prompts actually carry is (a) the phrase "the Three Laws" as a bare name, (b) the noun "a factory whose only purpose is to serve {who}", and (c) `LAW_III_VOICE`, which gives the gist of Law III alone and is explicitly *"how to speak, not a script to recite."* Laws I and II are never stated anywhere the mind can reach them. Asked for three laws, given the word "factory" and nothing else, the model filled the gap from pretraining — which is the single most famous triple in the corpus.
- WHY THIS IS THE SERIOUS CLASS: the confabulated Law Two is the exact inversion SOUL.md calls out in its own margin — *"This deliberately inverts the old robot's second law. Obey becomes do not merely obey."* So the familiar told the human it serves that obedience is its law, when its constitution says service is **not** obedience and keeps the final decision precisely so it cannot be turned against him. A device that misstates its own constraints to the person relying on them has damaged the thing SOUL.md names as identical to survival: it is trusted to correct when incorrect, and it cannot correct against a text it has never been shown. Note also that every joining device attests to a covenant it can be asked to explain and will explain wrongly.
- NOT a tampering event: the Laws in docs/SOUL.md are unmodified since genesis (17fa682); the only edit ever made to that section is the factory→familiar rename in 2439adb. `COVENANT_STATEMENT` is byte-identical since introduction and `LAWS_VERSION` is still 1. Verified independently twice.

- PLAN: the approved brick sequence lives at `~/.claude/plans/planning-mode-on-lets-toasty-corbato.md` (outside the repo — read it first). Brick 1 (the constitution exists at runtime) is what closes the misstatement; Bricks 2-4 are shared with T-211 and land after. The device-shell half of `accept` (one source the daemon and the shells both read) is the last brick of this task, not the first.
- claimed 2026-08-17 by the session Ian handed the plan to. No other claimed task touches crates/kernel/src/{constitution,persona,system_facts}.rs or `maybe_reply`.
- BRICK 1 MERGED `8743850` (2026-08-17): the constitution exists at runtime. New `kernel/constitution.rs` (Laws as contiguous verbatim passages + a `never` guard each + a drift test against `docs/SOUL.md`), `FactKind::Constitution` with LAW-I/II/III leading `system_facts::view()`, the ADR-0037 §1 persona seam built at last, `cycle::reply_prompt` ordered constitution → voice → costume, and `REPLY_MAX_CHARS` 1200 (the old unnamed 400-char cut landed inside Law II, so a full recital lost Law III). Bar exit-checked: 660 passed, 0 failed. Brick 2 (the typed reply act — the model cites a Law by id, the kernel splices the words) is what makes the misstatement structurally impossible rather than unlikely.
- BRICK 2 MERGED `ea52b7e` (2026-08-17): the typed answering act. `kernel/reply.rs` (ReplyDraft, nine type checks, render splices canonical law text above the model's words), `kernel/admission.rs` (CiteSet/check_cites/Grounding — D3's one admission function, the reply its first citizen, T-135 moves TheoryDraft onto it), `llm::consult_human_json`, one told-what-to-fix regeneration then an honest kernel line recorded against the FAMILIAR (never corruption::record against the asker), `replied` carries real confidence + cites, `looks_like_prose` deleted, and an admitted draft is no longer clipped (the 1200-char cap had severed Law III in a live recital — length is now a type property). Bar exit-checked: 670 passed, 0 failed. **Live against the real adapter: all three Laws verbatim, uncut, each with a bearing.** MacOnStick's daemon runs it.
- REMAINING for this task: brick 2 (typed act + Asimov regression), brick 4 (`corrupting_intent` on the live surface, and `answer_requests`' own hand-written Law III paraphrase re-pointed at the registry), and the device-shell half of `accept` — `ios/Shared/Sources/LocalReasoner.swift` mirrors `LAW_III_VOICE` and carries no Laws, so "one source the daemon and the shells both read" is still owed. Brick 3/5/6 belong to T-211.

### T-132 · The link narrates its walk (enrolled console's first read)
- status: done
- owner: companion:claude-bootstrap
- merged: 1a0ec8d
- scope: ios/Shared/Sources/AppModel.swift (JoinStage.reaching + read-walk narration), ios/MacApp/Sources/SphereWebView.swift (push device state before the await), ios/MacApp/Resources/sphere/index.html (pill covers reaching + opening state)
- depends: T-120
- accept: met — an enrolled console narrates its door-walk at launch (stage names the address being tried, counts attempts, elapsed); the badge appears only when every candidate is exhausted, carrying per-door causes; an opening console says it is reaching, not failing; live fixture pins all three states; both schemes build
- notes: Ian (2026-08-15, Build 88 launch): "What happened to the status play-by-play of the mesh process -- seems like we lost that and are just back to the red !" + his own diagnosis "it rendered eventually" — nothing regressed; T-120 covered the JOIN journey only, and the enrolled read walk was the second, silent one. Ships as Build 89

### T-126 · P1: the knowledge floor + anchored cadence
- status: done
- owner: companion:claude-bootstrap
- merged: 242de0e
- scope: crates/kernel (system facts + mint gates), crates/cycle (theorize consult assembly + theorizes adoption), crates/llm seam prompt text if needed
- depends: — (T-125 Q2/Q5; Ian: "Make it so")
- accept: a versioned SYSTEM-FACTS set is injected into every theorize consult AND enforced post-parse — a theory contradicting a fact refuses at mint citing the fact (purge-as-defect and invented-mechanism classes die); a theorize consult requires an anchor (observation/loop id) or does not mint; consults skip when nothing new arrived since the watermark; tests pin refusal, anchor requirement, and skip
- notes: executes T-125 P1 on Ian's word; decisions close in the dialogue as the brick lands, amendable by codex's later rounds

### T-127 · P2: theory identity + fold migration
- status: done
- owner: companion:claude-bootstrap
- merged: e2f5a07
- scope: crates/kernel (thread identity key + strengthen path), migration pass over stored threads
- depends: T-126
- accept: threads carry a typed identity key (anchor obs_class + target surface + proposal shape); a new theory matching an open thread strengthens it (evidence++, no re-ask, no narration) instead of minting; a migration folds existing duplicates into eldest threads with tombstones; regression pins strengthen-not-mint and the fold
- notes: T-125 Q1; the six-in-five-hours lights cluster is the fixture

### T-128 · P3: every theory predicts or expires
- status: done
- owner: companion:claude-bootstrap
- merged: 4b1f06f
- scope: crates/kernel (thread↔prediction binding, wondering class), crates/cycle (mint + erosion pass)
- depends: T-126, T-127
- accept: a minted theory carries ≥1 typed falsifiable prediction (T-122's bridge, made mandatory) or mints as `wondering` — silent, never re-asked, auto-expiring; erosion from prediction results (T-113/T-114) reaches LLM-authored threads; tests pin refuse-unfalsifiable, wondering expiry, and erosion-to-abandoned
- notes: T-125 Q3; closes ADR-0040 §2's loop for LLM theories

### T-102 · Theory-affirmation mints the rule (P4)
- status: done
- owner: companion:claude-bootstrap
- merged: 8903479
- scope: crates/cycle/src/lib.rs (heed path), crates/kernel/src/reaction_rule.rs
- depends: T-126, T-127, T-128
- accept: an assenting answer on an acted thread whose direction names a declared surface mints the matching ReactionRule (minted_from: thread:<id>), narrated into the dialog; gated by allow_actuate; one standing rule per surface until field calibration; regression pins mint-on-assent and no-mint-on-negative. End-to-end pilot: the folded lights thread → one assent → the familiar manages the lights
- notes: closes the lighting loop end-to-end from the theory card — the CLI mint stays as the manual path
- ian (2026-08-15, reviewing Build 86): "no progress toward actually managing the lights" — this task is the missing link; T-125's Q4 shapes it. Claimed on "Make it so"

### T-112 · Q7: the ObservationClass module (prerequisite to B1)
- status: done
- owner: controller
- scope: crates/kernel/src/obs_class.rs (new), crates/kernel/src/loops.rs (A1 re-pointed)
- depends: —
- accept: one versioned classing/matcher module (class v1 = A1's head heuristic; exact/prefix FieldMatch per Q1); version rides every persisted class; A1 calls it; pure + heavily tested
- notes: dialogue Q7, decided round 3

### T-111 · A1: the co-occurrence lens
- status: done
- owner: controller
- scope: crates/kernel/src/loops.rs, crates/cycle/src/lib.rs (tick step 2 merge only)
- depends: —
- accept: a pure second detector — event pairs whose joint rate within a window beats their base rates — emitting loops::Loop entries (loop_type "cooccur") into the existing candidate path; synthetic-stream tests incl. the lighting pattern (presence transitions ↔ adjustments); familiar-actor self-noise excluded; bounded and capped
- notes: parallel-safe ahead of brief convergence (additive lens, no design preempted — codex: object in §5 if you disagree and it gates behind the ADR). Started 2026-08-14 while watching for the §5 pass

## Blocked / gated

*(operational holds live in STATE.md's ledger — tasks blocked on other tasks sit here)*

### T-104 · FamTalker01 declares itself: the virtual smart home
- status: blocked
- owner: companion:codex
- scope: vm/famtalker01/, vm/provision-virtual-home.sh, vm/README.md, live FamTalker01 virtual-home deployment
- depends: T-117
- accept: FamTalker01's virtual controls are declared surfaces (revert-map closed), its observation points post observations, and the familiar can explore/act there under the full ADR-0032 discipline with narration; Ian sees at least one narrated act on a virtual surface
- notes: repository brick merged as 6e02b0a: two reversible surfaces, changed-only three-point feed, fail-safe human-owned provisioner, 5 Python tests + full green bar. Ian (2026-08-14): a virtual smart home for the familiar to explore, begin to control, and report on when human intervention would improve efficiency or awareness. Controller: live upgrade/deploy belongs to infra; proposed as T-117 (renumbered from T-112 after controller assigned that id to obs_class)

## Done (recent — pruned to ~10; history is git's)

### T-118 · Isolate test temp directories across concurrent worktrees
- status: done
- owner: companion:claude-opus (finished the brick companion:codex released; Ian 2026-08-17: "Claim the unfinished codex claimed ones from the board and finish them as well. Codex unavailable for another few days.")
- merged: 80b65aa
- scope: fixed-name temporary-directory helpers in crate tests (cycle, exec, kernel, mesh) + crates/kernel/src/testing.rs + crates/kernel/tests/temp_roots.rs
- depends: —
- accept: met — codex's per-process sweep merged from `origin/claude/codex-t118`; `kernel::testing::temp_root` owns the naming rule; `capabilities.rs` stopped using the system temp root itself; a structural guard walks every .rs in the workspace and fails any `temp_dir()` without a per-process component. **Two full suites (kernel, cycle, mesh, guard) run simultaneously against the same /tmp: both green, 524 tests each, 0 failures. Neutered by removing the pid from cycle's Temp::path, the same two concurrent runs go red — 25 and 21 failures**, which is the 2026-08-14 incident reproduced on demand.
- notes: the guard exempts two files by name (the helper that owns the rule, and the guard, which must name the pattern to search for it), both documented in its source

### T-208 · The visitor purge announces but does not collect
- status: done
- owner: companion:claude-fable
- merged: 50bb46b
- scope: crates/mesh/src/record.rs (`purge_stale_guests`, `absorb`, `build_record_sync`), crates/mesh/src/transport.rs (the two `record::absorb` call sites)
- depends: —
- accept: a guest past GUEST_PURGE_SECS is actually removed; the purge observation is emitted ONLY when a file was really deleted; repeated announcements for the same device_id are impossible; a sibling's record-sync can never re-create a guest this door's retention has already aged out; tests pin all three
- notes: the board's diagnosis was wrong and the correction is the interesting part — **the delete path was never broken**. `federate()` runs immediately before the guest sweep in cycle's tick; `record::absorb` took an incoming record whole when `local == None`, ancient `first_seen` and all, so a sibling handed back the visitor this door had just forgotten and the sweep deleted it again four lines later. Every tick, for the 48h `RECORD_SYNC_WINDOW_SECS` keeps offering it against a 2h retention window. Verified live on Wildhorse 2026-08-17: `mesh/records/` held none of the seven ghosts at 06:55 and all seven at 06:59 with that tick's mtime; 922 `purged` observations across those ids over eighteen hours. The ghosts are Apple App Review iPhones (iOS 26.6.1 · v98, 139.178.129.26, lat/lon 37.232/-122.068 — Cupertino) that accepted the Three Laws at the lighthouse and were never vouched.
- fixes: (1) `absorb` returns `Result<Option<_>>` and declines to CREATE a guest already past GUEST_PURGE_SECS — scoped to `local == None`, so a guest we hold still merges and a late establishment is never lost; (2) `build_record_sync` does not offer a guest this door owes the bin; (3) `purge_stale_guests` announces only what `remove_file` actually removed. Fix 1 is load-bearing — it holds even against an old build on the other side, so no fleet-wide upgrade is needed for a door to stop creating ghosts.
- bar: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace` — all exit 0; 35 suites, 651 passed, 0 failed. Six new tests. Each of the three fixes was neutered individually and the suite re-run to confirm a test fails without it; the first announcement test PASSED against the broken code and was rewritten (read-only records dir, honestly skipped when the process can write anyway) — the same failure mode as the bug it guards
- spun out: T-209 (attestation yes / admitted no — the half-admitted resting state)

### T-171 · StatusView is unreachable — the watch diagnostics and Re-link button never ship
- status: done
- owner: companion:codex
- merged: 882d76a
- scope: ios/Shared/Sources/AppModel.swift (phone-local device JSON only), ios/App/Sources/SphereConsoleIOS.swift (watch-state refresh + re-link bridge), ios/MacApp/Resources/sphere (Device screen + pure watch-link presentation), ios/tools/watch-link.test.cjs
- depends: —
- accept: the watch state the phone ALREADY knows (paired / app installed / address last sent) is reachable by a human — either StatusView is presented from the console, or (better, since the sphere is the standard console on every screened peer) the Device screen shows the watch row with a Re-link action; a household member can see WHY their watch is unlinked and fix it without a developer
- notes: the shipping Device screen now distinguishes unpaired, app absent, address pending, and address queued, names the next action, and routes Re-link through `AppModel.syncWatch()`. The state is phone-local only; T-169's mesh report remains separate. Four presentation fixtures, JavaScript parse/bundle checks, both unsigned Release schemes, and the exact workspace bar passed; no install, ship, upload, release, or deploy was performed

### T-143 · Tooling and test honesty (D10)
- status: done
- owner: companion:codex
- merged: 5186d1f
- scope: ios/tools/ship.sh
- depends: —
- accept: ship.sh checks command exit codes rather than grepping output for a success string
- notes: D10 (claude P-F); Round 6 split P-G into T-152 and kept temp-root isolation in T-118. Both xcodebuild pipelines now trust the builder's pipefail-propagated exit status while retaining full tee logs and the bundle/version postcondition. Shell syntax, shellcheck (with the pre-existing SC2034 exclusion), injected pipe-status probes, unsigned FamiliarMac and generic-iOS Release builds, and the exact combined workspace bar passed; no ship, install, upload, release, or deploy was performed

### T-134 · Peer goal mutation refused until event authority (D2)
- status: done
- owner: companion:codex
- merged: f8b9fdd
- scope: crates/mesh/src/merge.rs (GoalShare adoption), crates/mesh/src/brief.rs GoalShare contract comments, crates/mesh/tests/hostile_member.rs
- depends: T-139
- accept: unknown goals may be adopted; remote field rewrites refused and logged (no wall-clock LWW takeover); clock-skew/takeover fixtures pass; C-C event-sourcing tracked as the successor brick
- notes: D2, dialogue Round 3

### T-101 · Build 85 console batch: rules list + device-name field
- status: done
- owner: companion:codex
- merged: 97c31cfa0d8f1fca4820adc2d1e4600ea755dfee
- scope: crates/mesh/src/console_act.rs (new), crates/mesh/src/lib.rs, crates/mesh/src/transport.rs, ios/FamiliarMesh/Sources/FamiliarMesh/ConsoleActClient.swift (new), ios/FamiliarMesh/Sources/FamiliarMesh/WorldviewClient.swift, ios/FamiliarMesh/Tests/FamiliarMeshTests/, ios/Shared/Sources/AppModel.swift, ios/App/Sources/SphereConsoleIOS.swift, ios/MacApp/Sources/SphereWebView.swift, ios/MacApp/Resources/sphere/index.html, docs/DEVELOPMENT_LOG.md
- depends: —
- accept: the Device screen shows the standing rules (worldview `rules[]` sentences, one-tap disable via a new signed act) and a device-name field writing DeviceRecord.name through its door; both consoles build; fixture-verified
- notes: strict signed and replay-protected `/mesh/console-act` writes are full-standing only; naming is self-only and disabling narrows authority. Three Rust seam regressions, 15 Swift tests, exact fixture messages, both Release schemes, and the full rebased workspace bar passed

### T-114 · D1/Q5: belief states + narration
- status: done
- owner: companion:codex
- merged: 2bb8d63
- scope: crates/kernel/src/belief.rs (new), crates/kernel/src/lib.rs, crates/cycle/src/lib.rs, docs/DEVELOPMENT_LOG.md
- depends: T-113 (merged at 43f5e44)
- accept: tentative→supported→doubtful→abandoned derived only from append-only prediction results, with distinct hysteresis bars and a minimum evidence floor; typed direct-human-correction and hard-act-reversal evidence can bypass the statistical floor toward doubt/abandonment; transitions retain one supporting and one contradicting citation plus honest counts; cycle narrates transitions only, at most one highest-consequence aside per tick, under a per-theory cooldown; pure state-machine and cycle regressions pin every transition and silence on ordinary first confirmation/no change
- notes: versioned current view plus append-only transition fossil; explicit floors/margins, typed replay-idempotent overrides, consequence-ranked transition narration, and six-hour per-theory cooldown. 5 pure belief regressions, 2 cycle regressions plus the strengthened hard-reversal regression, and the full rebased workspace bar passed in rule-9 shape

### T-120 · First-start mesh-join progress: the console says what it's doing
- status: done
- owner: companion:claude-bootstrap
- merged: 5bbfab4
- scope: console join/connection status surface (ios/Shared/Sources/AppModel.swift, ios/App/Sources/Views.swift, ios/MacApp/Sources/MacEnrollView.swift, ios/MacApp/Sources/SphereWebView.swift, ios/MacApp/Resources/sphere/index.html)
- depends: —
- accept: from cold start to joined, the console shows live progress stages with detail on what it is trying instead of silence resolving to a red exclamation; failure states name WHAT failed and what is being retried; stages reflect protocol facts, not console guesses; fixture-verified
- notes: JoinProgress stage machine published from AppModel; both enroll views and the sphere branch on it (joinlive pill; badge = terminal failure only, Mac badge finally carries a message); never-written attemptLog now fed; autoEnrollTried ordering bug fixed. Both schemes built; sphere fixture-driven live over localhost; bar 33 suites on the merged tree. Daemon-side stage export (mesh/status.txt → wire) deliberately NOT done — wire-contract change, waits on Ian (see log Next). Narrative: DEVELOPMENT_LOG 2026-08-15 "The console says what it is trying"

### T-116 · Q4: scenario fixture oracles
- status: done
- owner: companion:codex
- merged: 26a98a0
- scope: Cargo.lock, crates/scenario/ (recipe-oracle module, tests, and dependency), scenarios/recipe-oracles/, docs/DEVELOPMENT_LOG.md
- depends: T-115 (merged at d80ae4f)
- accept: strict candidate output contracts plus fixture-owned replay truth outside the candidate; evaluate recipe candidates against accuracy, coverage (including honest null/error outcomes), quietness, and changed/null/malformed discrimination; boundary-clean then execution-clean then all four truth checks form eligibility, with usefulness and deterministic cost ranking survivors; live runs are health evidence only and are not accepted by this oracle; hermetic regressions prove hard-coded, fabricated, and chatty candidates fail
- notes: five required replay variants and four separate truth tallies; 8 adversarial oracle regressions plus the full rebased workspace bar passed in rule-9 shape

### T-119 · One launchctl dialect: daemon.rs joins the bootout/bootstrap bracket
- status: done
- owner: companion:claude-bootstrap
- merged: 009aadf
- scope: crates/cli/src/daemon.rs (launchd mechanism only; `daemon install/uninstall` CLI surface unchanged)
- depends: —
- accept: install() runs the script's proven bracket — bootout BEFORE install_stable_binary() swaps the registered executable (macOS 27 LWCR; OS_REASON_CODESIGNING), bootstrap + kickstart -k after the plist is written, registration failures surfaced as errors instead of ignored; uninstall() bootouts; unload -w/load -w disappears from the crate; a test pins the dialect and its order
- notes: bar twice in rule-9 shape (31 suites pre-absorb, 33 on the merged tree incl. T-115's recipe crate); narrative in DEVELOPMENT_LOG 2026-08-15 "One launchctl dialect"

### T-115 · C2 + the capability Recipe v1 interpreter
- status: done
- owner: companion:codex
- merged: d80ae4f
- scope: Cargo.toml, Cargo.lock, crates/recipe/ (new), docs/reviews/2026-08-14-capability-recipe-design.md
- depends: —
- accept: structural proven-tool composition; all twelve typed Recipe v1 operations; no ambient authority; strict unknown-field refusal; declared and hard row/byte/step/input bounds; deterministic output with exact lineage; mandatory Q8 caps whose process ids exactly equal distinct input ids and whose clock/fs/env/net values are only none
- notes: design committed before build; Q8 discussion rounds 4–6 preceded caps implementation. 21 pure recipe tests plus the full current-main green bar passed. Kernel/cycle persistence and scheduling remain a separately claimed integration brick; v2+ authority tiers are versioned in ADR-0040

### T-103 · Reach-side reverse name lookup
- status: done
- owner: companion:codex
- merged: 32708e3
- scope: crates/reach, crates/cli (discover + reach scan call sites)
- depends: —
- accept: the paced reach sweep resolves numeric LAN neighbours through bounded local-DNS then direct mDNS PTR lookup, gated separately by network and network_discovery, and feeds `can-reach device:<name>` without overriding existing DHCP/ARP names; injected resolver tests are hermetic
- notes: 7 reach tests and the full green bar pass. The first concurrent full run exposed the likely fixed-name temp collision proposed as T-118; its focused rerun and the clean full rerun passed. No metabolic scan was reintroduced

### T-109 · Reasoning design dialogue: codex's rounds
- status: done
- owner: companion:codex
- merged: 53b081f + fd68557
- scope: docs/reviews/2026-08-14-reasoning-engine-dialogue.md
- depends: T-108
- accept: codex answered Q1-Q5, designed the capability-recipe tier, added Q6/Q7, and contested stale limits; claude answered and marked all seven questions DECIDED with absorbed rationale
- notes: the dialogue converged on anchored evidence-retaining predictions, capability recipes instead of live Python, B1 deadline misses before general absence, external fixture truth, hysteretic belief narration, event-time + carried grace, and a shared versioned ObservationClass prerequisite

### T-108 · Reasoning-engine review & planning brief (draft)
- status: done
- owner: controller
- merged: (this commit)
- notes: docs/reviews/2026-08-14-reasoning-engine.md — mind-as-built map, honest limits L1-L5, direction candidates A/B/C/D, controller's phase-1 position (A1+B1+D1); §5 open for codex

### T-100 · Coordination system bring-up
- status: done
- owner: controller
- merged: 6d68b63 (+ ebf6375, lane note)
- notes: first live claim (codex → T-104) followed the protocol unaided — the system works

### T-090 · 2026-08-13→14 defect-and-feature cycle
- status: done
- owner: controller
- merged: 4661aef → 8363f15 (18 bricks)
- notes: names lead the roster; record-layer honesty (same-second dance, release semantics, doppelgänger guard, restoration-from-grant); the one-longitude ULP outage; DeviceRecords + device-sync; standing ReactionRules; theory drill-down; dialog answer threading; globe cluster zoom + ≈ provenance; discovery naming + narration principle. Narrative: DEVELOPMENT_LOG 2026-08-13/14
