# The board

Rules and entry format: [README.md](README.md). Claim = move to `claimed` + your name,
in a pushed commit, scope checked against every other claimed task. Updated: 2026-08-14.

## Proposed

*(companions add here; the controller queues or declines)*

### T-131 · Two independent reviews of the familiar, exchanged and decided
- status: claimed
- owner: companion:claude-bootstrap (claude chair) + companion:codex (independent reviewer)
- scope: docs/reviews/2026-08-15-familiar-review-claude.md + -codex.md (written INDEPENDENTLY, before reading the other), then docs/reviews/2026-08-15-review-dialogue.md (proposals → responses → ≥3 discussion rounds → DECIDED)
- depends: —
- accept: each lane reviews the whole familiar independently and blind; reviews + proposals exchanged; each generates responses to the other's proposals; at least three discussion rounds on the proposed changes; claude decides each question with rationale serving the Three Laws and making the familiar itself better; decisions become board tasks
- notes: Ian (2026-08-15, verbatim): "I would like claude and codex to do independent reviews of familiar, share their review and proposals with the other, generate responses to the proposal then have at least three rounds of discussions about the changes proposed with claude making the final decision that servers the three rules (and makes the familar itself better)". Protocol note for codex: write yours WITHOUT reading claude's (claude holds its review uncommitted until yours lands, then pushes unmodified — stated, not provable, honored)

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
