# The board

> ▶ **CO-DEVELOPMENT HOLD LIFTED EARLY — 2026-08-15 (Ian).** Ian's direct instruction:
> "excellent. authorized. continue." Both lanes may resume; the seven items recorded as owed
> to codex remain live work rather than abandoned questions.

Rules and entry format: [README.md](README.md). Claim = move to `claimed` + your name,
in a pushed commit, scope checked against every other claimed task. Updated: 2026-08-14.

## Proposed

*(companions add here; the controller queues or declines)*

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

### T-118 · Isolate test temp directories across concurrent worktrees
- status: queued
- owner: —
- controller (2026-08-14): accepted — it explains observed reality (a full-suite count read 4-of-31 during concurrent runs today); per-process/per-worktree unique temp roots, start with the fixed-name helpers (rules/actuator tests included)
- scope: fixed-name temporary-directory helpers in crate tests (begin with crates/cycle)
- depends: —
- accept: test temp roots include a process- or worktree-unique component; concurrent full green-bar runs cannot mutate the same fixture directory; a focused regression or parallel harness pins the isolation
- notes: observed while barring T-103: one full run overlapped the controller's run and cycle's parameter-revert test saw its second tick revert again; the test passed alone and a clean full rerun passed after the other job ended. Treat this as test-infrastructure hardening, not a T-103 failure

### T-117 · Deploy and witness FamTalker01's virtual home
- status: proposed
- owner: —
- scope: live FamTalker01 daemon + boundary.json + actuators.json + familiar-virtual-home-feed systemd units (infra lane; no further repo code)
- depends: T-104 repository brick (merge 6e02b0a), FamTalker01 upgraded to that merge or newer
- accept: infra runs vm/provision-virtual-home.sh; both declared surfaces answer state and preserve a closed off/dim/bright revert map; the three initial observation points land once and a changed state lands once more; one familiar-originated act produces a narrated console aside naming what changed, why, and how to undo it; actual node id/address/build and evidence are appended to STATE.md
- notes: companion must not SSH-deploy (rule 8); preserve every existing boundary choice, change no human records, and stop on a malformed boundary. Rollback: disable the timer, remove actuators.json, set allow_actuate false (leave unrelated gates untouched)

## Queued

### T-172 · The watch never says why it failed to join
- status: queued
- owner: —
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
