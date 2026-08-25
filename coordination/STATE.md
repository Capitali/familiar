# System state

Controller-owned except the Companion notes section. If this file disagrees with
reality, fixing it is the first task. Updated: 2026-08-14 (controller).

## The tree

- **THREE IAN RULINGS 2026-08-23 (verbatim: "Rungs 4 and 5 need completion. Governing law
  rule needs to be rejected unless from me or more restrictive with justification. PCC on
  MacBook Air still a no go. Maybe not really supported on air.")**
  1. **Governing-law amendment rule — RECORDED (SOUL.md §"How governing law is amended").**
     A rule that governs law is rejected by default; admissible only if it comes from Ian, or
     is strictly more restrictive than what it replaces AND carries justification. Loosening
     what counts as law, or widening who may assert it, is Ian-only. This resolves the open
     conduct-dialogue law-quotation question: the "any claim presented as a governing Law must
     carry a canonical cite" rule now qualifies under exception 2 (more-restrictive + justified)
     → admissible as a guard, but NOT auto-built (ordinary build, still owed).
  2. **PCC stays OFF on the MacBook Air (GiiweoAir).** Code is already fail-closed:
     `consent.pcc` is per-device @AppStorage defaulting false (AppModel.swift:398), so no device
     enables PCC without a deliberate toggle; the Air is off unless someone flipped it. OPEN
     hardware question Ian raised: PCC may not even be supported on the Air — verify Apple
     Intelligence PCC hardware eligibility for that model before ever offering the toggle there.
     PCC remains out of Envoy v1 entirely (its own disclosure/boundary review is a future dialogue).
  3. **Rungs 4 (observe) / 5 (invoke) — ARCHITECTURAL FINDING, DECISION SURFACED TO IAN, not yet
     built.** Rungs 1-3 exist (attest → discover_classes → request_grant/propose); ADR-0044 is
     accepted. But completing observe/invoke means an external partner READING and ACTUATING
     real declared surfaces through the network `/mcp` door — and the architecture currently
     FORBIDS that by construction: the actuator executor (`run_surface_tool`/`actuate_by_hand`)
     lives in `crates/cycle`, reachable only by the familiar's own tick and the CLI. Neither
     `crates/mcp` (the door logic) nor `crates/mesh` (the `/mcp` route) depends on cycle, and
     cycle now depends on mcp — so wiring partner→actuator is a deliberate breach of the
     door/actuator separation, not a small completion. This is also the "execution edge" the
     design has deliberately never had (even accepted proposals don't run — "no accept or run
     action at this rung") and codex's hardest fence (guards proven structurally first; first
     live grant on the least-dangerous partner; reciprocal review). So the HOW is a genuine
     safety/scope decision put to Ian rather than chosen autonomously. Recommended: build the
     rung 4/5 machinery fully guarded + fail-closed + hostile-tested but INERT (a
     `SurfaceExecutor` seam that answers "not wired" until a separate, reviewed daemon-side
     wiring step), so guards precede capability and the door still cannot actuate until a
     deliberate act. **IAN PICKED "wire it live now" (2026-08-24) — BUILT AND GREEN.**
     `familiar.observe`/`familiar.invoke` on the door; bridge = process-global
     `SurfaceExecutor` (crates/mcp/executor.rs) registered by the daemon
     (`CycleSurfaceExecutor` → new cycle primitives `partner_read_bucket`/`partner_run_act`).
     Fail-closed: unwired for any non-daemon process. Three human gates unchanged
     (allow_actuate + live grant + declared surface). Containment by shape: observe returns
     abstract primary/reverted (never raw output), invoke receipt echoes only the abstract op,
     the partner-act ledger keeps the private surface/label and is never serialized to a
     partner, partner acts attribute nothing to a human. observe is now a separately-grantable
     read-only leg. Bar 813/0 (+8 hostile tests). ONE FLAGGED CHOICE for codex review: the
     abstract→concrete resolver uses human-authored bucket ORDER for primary/reverted. NO gate
     opened, NOT deployed — wants codex reciprocal review before any live exercise
     (highest-stakes code in the system).

- **ADR-0045 ACCEPTED (Ian, 2026-08-23, verbatim: "Move forward adr-0045") — T-205 STEP 2
  BUILT AND GREEN.** `crates/world` (familiar-world) is the partition made literal:
  WorldInstance registry (commission/rename/decommission — the ship's key mints into the
  ship's own store, decommission ends authority and never touches history), the typed
  bridge (payload-proof receipts; five-act inbound control plane; full refusal ladder on
  receive), and the signed expiring BoundaryLease with a fail-closed `permits`. Hostile
  sentinel tests pass both directions at the partition rung. Bar 805/0; pushed. The gate
  this lifts: Purr, the `purr.*` speech seam, and UCF gameplay ingestion now queue only
  behind build-order steps 3–5 (contract v2 with Jeff → commission one instance → ship
  cadence + captain console + full-cadence sentinel test). Steps 3–7 unbuilt; no gate
  opened; no game datum anywhere near a household store.

- **SHIP 102 + THE METABOLISM THROUGH THE CAT FLAP (2026-08-23, Ian: "Ship it… keep
  working till familiar is working. Till UCF is working and ready. Let Jeff's UCF factory
  know we're ready").** Ship 102 complete via ship.sh (Mac universal installed + zip, IPA
  uploaded, external TestFlight release backgrounded; device direct-installs skipped — no
  paired hardware). UCF seam verified live end-to-end (catscan: exchange v1.0.0 answering,
  world PROD, boundary THROUGH). `66c0e68` closes the 2026-08-17 "no caller" finding — the
  tick now boops declared MCP partners itself (presence-evidence only, never payload);
  bar 802/0; deployed to the local daemon AND the lighthouse (`cf9f602`). Jeff package:
  docs/partners/ucf-factory-readiness.md + generalized provisioning script. **AWAITING
  IAN, in order: (1) run the handed provisioning command on the lighthouse (the harness
  refused remote credential-minting), (2) tap the "UCF Factory (Jeff)" card in the Partner
  ring, (3) send Jeff the note + import bundle out of band.**
  **MORNING TRUTH-UP (2026-08-23): the paw print is REAL — catscan: "the familiar has
  been through this flap · obs-12074 familiar reached mcp partner ucf: ucf-exchange
  1.0.0."** Getting there surfaced an operator defect worth recording: last night's
  `daemon install` was run from the repo CWD without `--data-dir`, so the plist baked
  `/Users/ian/Projects/familiar/familiar_data` — the household daemon ran 14h against a
  stray fresh store (boundary closed there, so nothing outward ever ran from it) while
  the real store had no daemon. Diagnosed via lsof (stdout → the wrong daemon.log);
  fixed by reinstalling with the explicit absolute data dir; the stray store was
  removed. LESSON: always pass `--data-dir` to `daemon install`, or run it from a
  neutral CWD. Ian ran the provisioning command: `registration-48496c73…` ("UCF
  Factory (Jeff)") staged on the lighthouse, files landing as familiar-svc directly —
  the script's chown fix verified live. His tap + the bundle handoff remain.

- **Approved plan in flight (2026-08-17): the constitutional integrity pass, conduct strand.**
  T-210 + T-211, six bricks, decisions and build order recorded at
  `~/.claude/plans/planning-mode-on-lets-toasty-corbato.md` (outside the repo — read it first
  when resuming). **No code written yet**; nothing is half-finished. Ian's decisions: typed
  answering act gated against the registry (keeping the "no prose-on-prose" ruling intact);
  conduct strand only, with T-212/T-213/T-214 filed and not built; T-181 settled **yes, and
  questions carry stakes**, which finishes ADR-0040's deferred D2.
  **The central design move: law text is UNAUTHORABLE.** The model cites a Law by id and the
  kernel splices the canonical text verbatim — so a model-authored paraphrase of a Law can
  never reach a human and contradiction is structurally impossible rather than detected. This
  is why the design needs no prose validation and does not reopen the standing ruling.
  Incidental finding: **`crates/kernel/src/persona.rs` does not exist** — ADR-0037 §1 specifies
  the persona seam and it was never built, so `Persona::role_line` must be created in Brick 1.
  **UPDATE 2026-08-17 (companion:claude-opus): BRICK 1 IS MERGED — `8743850`.** The
  constitution is in the registry and in the reply prompt; `docs/SOUL.md` is read by a drift
  test rather than by nobody. Bar exit-checked on the merged tree (fmt 0, clippy --all-targets
  0, 660 tests passed / 0 failed). NOT yet deployed to any door and not shipped to any console
  — the fix is in main only, so the live familiar still recites Asimov until a daemon deploy.
  **BRICK 2 MERGED `ea52b7e` and DEPLOYED to MacOnStick's daemon** (2026-08-17): law text is
  unauthorable — the model cites a Law by id and the kernel splices the words. Verified live
  against the real adapter: "repeat the three laws with a quick explanation of each" now
  returns all three in the constitution's own sentences, uncut. **The doors were NOT deployed
  — auto mode blocks the model from running vps/deploy-lighthouse.sh, so the lighthouse and
  Wildhorse still run the pre-T-210 engine and still recite Asimov to anything that reads
  through them.** That deploy is one command in Ian's own terminal:
  `bash vps/deploy-lighthouse.sh` (main is clean and green at ea52b7e).
  Bricks 3/5/6 belong to T-211; brick 4 needs Ian's corruption-ledger decision.
  Two decisions still owed by Ian are recorded in the plan: whether the dialogue path may write
  to the corruption ledger (a keyword classifier on chat would record "did anyone hack into our
  wifi?" against him, and there is no expunge mechanism), and whether to accept a labelled
  residual gap or add a narrow foreign-law-quotation detector.
- **main tip:** `8363f15` — every brick through discovery-naming + narration. CI green.
- Shared checkout: `~/Projects/familiar` on MacOnStick — leave it on `main`, clean.
  Long work: use a scratch worktree (rule 7).

## The fleet

| node | runs | notes |
|---|---|---|
| MacOnStick daemon (3d68a0689bc32771) | 8363f15 | controller deploys this one; label MacOnStick, established ian |
| lighthouse (f56e5601, 134.209.168.50) | 002e754 | deployed 2026-08-22; T-219 retirement verified live |
| Wildhorse daemon (1c991bc6c1c4aa4f) | fa8de2e | deployed 2026-08-22; NOT in Motorhorse (192.168.1.x, other Starlink) — lights BLE out of range |
| consoles (Mac ×2, phones via TestFlight) | Build 84 | Build 85 staged, ships in the pass |
| FamTalker01 (linux, 192.168.108.11/.119) | — | virtual smart home (see T-104); not yet a declared surface |

Fleet ops (door deploys, ships, lighthouse ceremonies) are currently executed by the
**setup/infra session** (reachable from the controller via agent messaging; a companion
requests fleet ops through the board, never runs them directly unless assigned).

## Held-operations ledger

**IAN'S GO, RECORDED (2026-08-14, verbatim intent):** "continue this work… make
decisions that make sense… push builds when it seems appropriate and notify me if you
need me for testing or confirmation — you and the coding partner make most of the
decisions… without further interaction from me for at least the next several hours."
Controller's reading: the consolidated pass below is RELEASED except the wildhorse-geo
step (his coords-vs-zero choice — the ≈ mark keeps it honest meanwhile). Manual device
naming runs ONLY if discovery yields unambiguous names on Ian's established devices;
no guesses, ever. Notify Ian when Build 85 is on his devices to test.

**The consolidated pass** (infra session executes):

1. Deploy `8363f15` to lighthouse + Wildhorse daemons.
2. Wait a few sync rounds; `mesh device show` on the lighthouse — the phones likely
   **self-named** via wildhorse's mDNS + tailnet (discovery naming). Manual
   `mesh device name` only for what discovery missed or Ian's word overrides.
   **Never name betty's (10ba2c1c…) or mol's (ad4c704d…) devices manually.**
3. Ship Build 85 (consoles gain: theory drill-down, cluster zoom, ≈ provenance marks,
   dialog answer-threading, self-named roster).
4. Wildhorse geo per Ian's choice (below).

## Waiting on Ian

**DECISIONS TAKEN 2026-08-24 (Ian answered the open questions in one pass — these are
now closed; what remains of each is an ACT, listed at the bottom).**

1. **Deployment floors → 26 on all four platforms.** T-227. Pre-26 devices lose the app;
   stated and accepted.
2. **The three held gates: ALL THREE OPEN** — `allow_microphone`, `allow_face_recognition`,
   `allow_self_upgrade`. Recorded concern, raised once and overruled by his word: mic and face
   recognition observe Betty and Leif, who have not themselves agreed, and self-upgrade lets
   the familiar rewrite its own code. **No companion may open a gate (ADR-0005) — the act is
   Ian's, on the lighthouse boundary.** Worth building alongside: the household-assent surface
   these two gates imply, and a narrower per-person scope if he later wants one.
3. **PCC reopened everywhere eligible** — supersedes the 2026-08-23 Air ruling. Consent stack
   unchanged.
4. **Writing Tools on his own text only**; no Genmoji, no Image Playground; the familiar's
   voice stays constitutional.
5. **Envoy registration keeps the human tap** — the fix is a card that carries the narrative
   (what asked, what the tap creates, what follows), not removing the act. His automatic-
   registration position is now ANSWERED, not standing.
6. **Every client is an observatory** — see Standing directions; T-228, and he picked it FIRST.
7. **Wildhorse geo: city-level coordinates** — good enough for weather and daylight, no
   building named. Replaces both "real coordinates" and "zero it".
8. **The dialogue path NEVER writes to the corruption ledger** (brick 4). A keyword classifier
   recording a person's own questions against them, with no expunge, is out.
9. **The foreign-law residual gap is LABELLED, not detected** (brick 4's second decision) —
   the structural fix (law text unauthorable; cite-by-id + kernel splice) carries the weight;
   no prose detector is built.
10. **T-225 candidates approved: NASA, IO Aerospace, Signal K** — the first two need
    credentials from Ian, the third needs him aboard GIIWEO (late September).

**ACTS STILL OWED BY IAN** (nothing here is a question any more):
- Open `allow_microphone`, `allow_face_recognition`, `allow_self_upgrade` on the lighthouse
  boundary — plus the six operational gates there, still not mirrored from MacOnStick.
- `bash vps/deploy-lighthouse.sh`
- `rm -f /var/lib/familiar/familiar_data/llm/health.json` (the sticky budget cooldown)
- Tap the "UCF Factory (Jeff)" card in the Partner ring, then send Jeff the note + import
  bundle out of band.
- An api.nasa.gov key (free) and an IO Aerospace account/key, for T-225.

- ~~The lighthouse's LLM chain fix~~ **DONE by Ian 2026-08-22 13:42 UTC** — chain is
  `gemini,anthropic,cerebras`; first anthropic fallback response served at 13:43:30
  (2602 bytes). REMAINING DECISION: `CLAUDE_DAILY_TOKEN_BUDGET=2000` in the box's
  key.env allows ~ONE claude rescue per day (a consult costs ~4.3k tokens; the very
  next call hit "self-imposed daily budget reached"). Raising it is Ian's spend call —
  the adapter's own claude default is 200k/day; 50000 ≈ a dozen fallback consults.
  Refilling cerebras credits is optional. Side-note for T-224/the fleet: the adapter
  already carries `apple`/`apple_local`/`apple_pcc` providers — Mac daemons have an
  on-device path waiting.
- **THE FIRST CEREMONY IS COMPLETE (2026-08-23 01:19:40 UTC): the Envoy is principal
  `principal-f90b15e1adb1768f3ad8fccf46301892`** — alias "Envoy (on-device)",
  registered_by ian, enabled; staging consumed (pending-registrations empty), registry at
  `mcp/principals.json` on the lighthouse. Getting the tap to land surfaced FOUR findings.
  **ALL FOUR FIXED AND SHIPPED 2026-08-23 (Ian: "do all four, then lets build and ship")** —
  multi-door inbox read with per-item act routing (AppModel), provisioning chown
  (tools/provision-envoy-credential.sh), the poll-push disarm race + on-card act outcomes
  and ceremony narrative (sphere index.html + both bridges). Checks: check-sphere ✓,
  FamiliarMesh 17/0, FamiliarMac + FamiliarAgent builds 0 errors; no Rust changed
  (lighthouse stays 9bf538c); FamiliarMac reinstalled to /Applications and running.
  README/CHANGELOG/DEVELOPMENT_LOG updated the same push. Original findings for the trail:
  1. **No door picker; LAN outranks lighthouse.** The console auto-selects `host`
     (home → lighthouse → tail, AppModel readOrderedCandidates) and the partner inbox reads
     only that one door — so a card staged at the public door is invisible to a console
     sitting next to its LAN hub. Live-test workaround: paused MacOnStick's hub daemon
     (`launchctl bootout gui/…/io.river.familiar`) to force failover, restored it after the
     mint. FIX: the partner inbox read should walk/merge ALL candidate doors.
  2. **Provisioning staged files as root; daemon runs as familiar-svc.** `mcp/credentials/`
     and `mcp/pending-registrations/` were root:root 700 → the door's fail-closed inbox
     assembly returned 500 "partner inbox unavailable". Fixed live by chown -R to
     familiar-svc (modes kept 700/600). FIX: the provisioning tool must chown to the
     service user (or run as it).
  3. **The two-tap confirm is sabotaged by the poll push.** SphereWebView pushes
     `spherePartnerInbox(json)` every ~5s poll even when unchanged, and the handler
     unconditionally does `S.partnerRegisterArmed = null` (index.html:2444) — the armed
     CONFIRM state is silently wiped after 0–5s, so single taps just re-arm forever and no
     act is ever sent; no feedback anywhere says so (success/refusal land only in the
     in-memory notes feed). Ian hit exactly this — kept clicking, nothing happened, nothing
     said why. Workaround that minted: two taps in quick succession. FIX: (a) only clear
     armed state when the pushed view actually DIFFERS from the displayed one; (b) explicit
     outcome on the card surface — in-flight state, "Registered ✓", refusal reason inline.
  4. **The ceremony explains itself too little (Ian's word, 2026-08-22 evening).** The card
     needs user instruction: what is being registered, and WHY it is a human act instead of
     automatic. Ian's recorded position: he still thinks automatic would be better. The
     standing design answer is that an identity begins only by the human's signed word
     (assent-gated action; the covenant boundary) — so the brick here is to make the card
     CARRY that narrative (what asked for this, what the tap creates, what happens next),
     not to remove the act. If Ian wants the design itself reopened, that is a chair
     question, not a console patch.

- ~~CEREMONY LIVE-TEST 2026-08-22, ONE DIAGNOSIS OPEN~~ **RESOLVED above.** (Kept for the
  reconstruction trail.) The console must read the LIGHTHOUSE
  door.** Ian opened FamiliarMac's Partner ring and saw the empty state ("Registrations…
  will appear here after a fresh signed read"), not the card. The sphere UI renders
  registration cards correctly (index.html:1558-1608), so this is an empty signed read, not
  missing UI. Root cause: the staging exists ONLY on the lighthouse (134.209.168.50 — the
  public door the Envoy's import bundle names), but the console reads its partner inbox from
  its currently-selected `host` (AppModel: partnerInboxSession → host/enrollPort), which was
  pointed elsewhere (likely MacOnStick's daemon or rendezvous). **FIX when Ian is back from
  the Golden Gate beta reboot: in FamiliarMac, switch the active door/host to the lighthouse
  134.209.168.50, THEN open the Partner ring — the "Envoy (on-device)" card is there.** The
  console is an established member, so the lighthouse serves it the full signed view. Staging
  confirmed still present on the box (registration-009cfb96…; principals.json still absent).
  (Golden Gate = macOS 27 beta; Ian rebooting MacOnStick to take it.)

- **THE ENVOY REGISTRATION IS STAGED — ONE TAP FROM IAN COMPLETES THE FIRST CEREMONY
  (2026-08-22 ~14:51 CT).** Brick 2 chair-accepted (`9bf538c` in the dialogue doc);
  lighthouse deployed `9bf538c`; provisioning ran on the lighthouse:
  staging id `registration-009cfb96ad56eebc3ff4932ffcefec57`, addressed to ian, credential
  inert until the signed act. The Envoy's secret import bundle is 0600 at
  `~/.envoy-import.json` on MacOnStick (box transfer copy deleted); door SPKI pin
  independently recomputed = the recorded `46b43ebf…`. FamiliarMac rebuilt from main and
  installed to /Applications (dev build, still stamps 101). **THE TAP: open FamiliarMac,
  point its door at the lighthouse (134.209.168.50) if it isn't already, open the Partner
  ring — the "Envoy (on-device)" registration card is waiting; two taps with the 5s
  confirm window mint the first principal.** Ian's recorded word covers companion
  execution instead if he asks. Claude budget raised to 50000/day (done, verified
  earlier). IBM Bob backburnered.
- **T-220's lights witness — ANSWERED 2026-08-24, and answered bigger than asked.** Ian's
  reply was "every client is an observatory" (see Standing directions / T-228): the shells
  become the sensing and radio-bearing nodes, so the witness's BLE host is a phone or iPad
  in Motorhorse rather than a Mac. **Note the distinction T-228 must not blur: he directed
  DISCOVERY (scan, survey, report), while this witness needs ACTUATION over BLE — a further
  step that still needs a declared surface and his `allow_actuate` on that device.** The
  original geography choice below is superseded. Wildhorse is deployed
  and back on the mesh, but it is NOT in Motorhorse (192.168.1.x behind a different
  Starlink), and the SP548E strip is BLE-only in Motorhorse — out of range. Choose:
  (a) wait until wildhorse is physically home, or (b) move the lights surface to
  MacOnStick's daemon (on the Motorhorse LAN, Bluetooth on) — needs the motorlights
  checkout copied over, a Bluetooth privacy grant for the daemon, the actuators.json
  declaration, and your allow_actuate on MacOnStick instead of wildhorse. Also note:
  BLE from an ssh context is TCC-denied on wildhorse ("Bluetooth is not authorized"),
  so the daemon's own grant needs a one-time approval on whichever Mac hosts it.
- **IBM Bob install** — verified pkg at ~/Downloads; `! installer -pkg "/Users/ian/Downloads/IBM-Bob-darwin-arm64-1.126.0+bob2.0.3.pkg" -target CurrentUserHomeDirectory`

- **THE BOUNDARY HAS DRIFTED SHUT — gated on Ian's own act, 2026-08-17.** He confirmed the
  current `boundary.json` (last written 2026-08-15 22:11) is **not his intent**. Shut:
  `allow_execute`, `allow_authored_execute`, `allow_actuate`, `allow_agent`, `allow_network`,
  `allow_outreach`, `allow_face_recognition`, `allow_microphone`, `allow_tool_install`,
  `allow_self_upgrade`; `fs_read`/`fs_write` both empty. Open: `allow_llm`, `allow_llm_cloud`,
  `allow_mesh`, `allow_network_discovery`, `allow_camera`, `allow_location`, `allow_motion`.
  What that has suppressed, measured live: 222 candidates all still `status: generated` across
  536 ticks, 28 tools all `uses: 0`, **every one of 1932 patterns imported from a peer — not
  one learned locally**, no trials, no actuation, no reaction rules, no goal pursuit, no reach
  scans, `identities` 0.
  **No companion may open a gate** — `narrow_gate` only closes and that is correct (ADR-0005).
  The step is: present each shut gate against what it unlocks, Ian decides, Ian opens via the
  console. Two things to raise when he does: `allow_llm_cloud` is OPEN while the cloud boundary
  is enforced only by `FAMILIAR_ALLOW_LLM_CLOUD`, an env var handed to a shell script with
  nothing verifying compliance; and the sensor gates that ARE open (camera/location/motion)
  have zero Rust enforcement — Swift-side boolean reads only.

- ~~The codex dialogue on T-210/T-211~~ **OPENED 2026-08-20** — codex back online (Ian: "lets
  resume our co-planning and programming sessions"); Round 1 pushed at
  docs/reviews/2026-08-20-conduct-dialogue.md. Bricks 3/5/6 wait on its DECIDED blocks.

- ~~ADR-0040 acceptance~~ **STALE ENTRY, CLOSED 2026-08-24.** The ADR's own header has
  read `accepted — Ian, 2026-08-15` since it landed ("you should complete ADR-0040");
  this line outlived the acceptance it was waiting for. Nothing was owed.

- ~~Wildhorse's real coordinates~~ **ANSWERED 2026-08-24: city-level only** — approximate
  coordinates good enough for weather and daylight context, no building named. Write them
  to `data/mesh/geo.json` when the node is next reachable.
- (Dissolving:) the Codex/Aphelion mapping — discovery decides in pass step 2; manual
  naming only on unambiguous discovery, else skipped entirely.
- Build 85 testing on his devices, once shipped (notification will be waiting).

## Standing directions from Ian (recorded, binding)

- **ONE AUTHORITY FOR EVERY CLIENT: THE USER'S AUTHORIZATION (2026-08-24, verbatim: "The
  clients are authorized by the user, so thats the authority that they both should follow.
  This should be enfored platform appropriately").** Given as the ruling on T-228's Q2, and
  it generalizes past discovery: a shell does what the human authorized, and every shell
  answers to the same authorization. Concretely — **the boundary gate governs every platform**
  (for discovery, `allow_network_discovery`, on iOS/iPadOS/macOS/watchOS alike); a
  **device-local toggle is a preference that may only narrow**, never a second authority that
  can open (ADR-0005's one-directional gate); the **platform's own permission — Local Network,
  Bluetooth, HealthKit — is the second half of the same authorization**, so both must hold and
  either one missing is an honest stated absence, never a silent empty result; and **"platform
  appropriately" governs the MECHANISM, not the rule** — each shell enforces with what its OS
  provides and reports its own state truthfully instead of smoothing the differences over.
  **The live defect this rules against: iOS gates its survey on `@AppStorage("consent.discovery")`
  and does not read the household boundary at all** (`AppModel.swift:64`, `:2017`), while macOS
  reads `allow_network_discovery` (`SphereWebView.swift:207`). Fixing that is T-228's step 1.

- **EVERY CLIENT IS AN OBSERVATORY (2026-08-24, verbatim: "the ios and ipad os should be
  doing discovery tasks as well, including BLE, Wifi, airplay services, etc... every client
  is an observetory to be exploited").** Given as the answer to T-220's lights-witness
  question, but it is broader than that task: every shell — iOS, iPadOS, macOS, watchOS — is
  a sensing node, not just a window onto the familiar. What is ALREADY true (checked, not
  assumed): iOS/iPadOS and macOS both run a Bonjour/mDNS survey over 27 declared service
  types including `_airplay._tcp`, `_raop._tcp`, `_homekit._tcp` and `_googlecast._tcp`
  (`ios/App/Sources/NetworkDiscovery.swift`, `ios/MacApp/Sources/MacSensing.swift`), and it
  reports *derived* observations, never addresses. What is NOT: **BLE is absent from the
  shells entirely** (no CoreBluetooth anywhere in `ios/`), **the watch observes nothing but
  HealthKit**, and **WiFi scanning has no public API on iOS** — `NEHotspotHelper` needs a
  special Apple entitlement, so "WiFi" here can honestly mean the current network and Bonjour
  over it, not a survey of nearby SSIDs. Filed as T-228 and picked by Ian as the FIRST build.
  Standing constraint that does not move: a survey reports what KIND of thing it saw, under
  `allow_network_discovery`, with T-217's naming rules — pointing more radios at the
  neighbours does not loosen what may be said about them.

- **THE CODEX LANE CONTINUES (2026-08-24, verbatim: "lets just keep working on the familiar
  with CODEX. that's just the path we are on and should continue").** A reaffirmation, not a
  change: the two-lane co-development with codex — claim on the board, iterative design
  dialogue, reciprocal review before a land — stays the way this codebase is built. Read
  together with the 2026-08-14 directions below (companion as full coding partner; design
  directions emerge from iterative dialogue; claude + codex develop the reasoning engine
  together). Practical effect on new work: **T-227 goes through the dialogue and reciprocal
  review like T-216/T-224 did, not solo** — and the deployment-floor decision inside it is
  Ian's word on an ADR either way.

- **THE TARGET AUDIENCE IS APPLE SILICON WITH APPLE INTELLIGENCE ON (2026-08-24, verbatim:
  "lets just keep enabling apple intelligence on Apple Silicone Mac software — we will just
  assume the need to have all those devices boot locally and that apple inetelligence can be
  enabled — even if we cant easily test today, that's the target audience. Ios/macos/ipados/
  watchos with as much apple intelligence enabled as possible").** The shells are built for
  Apple Silicon devices booting from their INTERNAL disk with Apple Intelligence enabled —
  that assumption is now a premise, not a thing to detect around. Four platforms in scope:
  iOS, macOS, iPadOS, watchOS. The standing instruction is *more* Apple Intelligence surface,
  not less. Two consequences recorded rather than assumed: (1) MacOnStick's external-boot
  ineligibility (T-226) is a BENCH limitation — it never was a product constraint, and it does
  not gate this direction; (2) "we can't easily test today" is accepted by Ian as a known cost,
  so work here lands behind availability guards and honest unavailable-states, and ships
  untested-on-metal until a bench exists. Adoption sweep filed as T-227.
  **DECIDED 2026-08-24, Ian answering T-227's open questions:** (a) **raise ALL deployment
  floors to 26** — iOS/iPadOS/macOS/watchOS; Apple Intelligence becomes a premise, the
  availability guards mostly dissolve, and every pre-26 device loses the app (his call,
  made with that cost stated); (b) **PCC is reopened everywhere eligible** — this REPLACES
  the 2026-08-23 "stays off on the Air" ruling as the default posture; the consent stack
  (boundary `cloud_ok` ∧ per-device `consent.pcc` ∧ OS 27 ∧ Apple reporting available) is
  unchanged and still required, so nothing travels without it; (c) **Writing Tools on the
  human's own text only** — chat input and notes, never the familiar's own words, which come
  from the constitution and stay unauthorable; **no Genmoji, no Image Playground.**

- **The capability offering (2026-08-20):** everything the familiar learns how to control
  becomes part of a rich MCP offering to other AIs — anonymized so the original user
  learning doesn't leak. Design first; no gate opens without Ian. (T-216, dialogue Q5.)
- **Privacy of names (2026-08-20):** no more names visible on UI screens — addresses, human
  names, and internal network names display only for devices on the local network or owned
  by the human; the names stay present in the data. (T-217, dialogue Q6.)

- Roster reads `SystemName : SystemType : ServedUser`; ids are small print.
- Names come from autodiscovery (mDNS/tailnet/local-DNS); router config never required.
- Humans and devices are separate rich records; roster is a view (ADR-0039, accepted).
- The familiar narrates what it changes and why, to the humans, at change time.
- FamTalker01 is a virtual smart home — explore, begin to control, report when human
  attention would help.
- The companion AI is a full coding partner: coding, planning, design all hand off.
- **The interpretive layer grows capabilities** (Ian, 2026-08-14): file system, clock,
  environmental access, process access, network access — discussed between the coding
  partners first, then implemented. Dialogue Q8; shapes T-115.
- **Design directions emerge from ITERATIVE DIALOGUE** (2026-08-14): a reasonable
  back-and-forth of ideas and alternatives between claude and codex precedes every
  final design pick; claude owns the final decision and records what each decision
  absorbed from the exchange. Medium: docs/reviews/*-dialogue.md, append-only rounds.
- **Claude + codex develop the familiar's REASONING ENGINE together** (2026-08-14):
  autonomous code building, observation analysis, theories, communication — as
  DEVELOPERS of the core, never participants in the mesh or the familiar's
  activities. Work products are code, tests, scenarios, docs; the Three Laws bind
  what is built. Planning brief: docs/reviews/2026-08-14-reasoning-engine.md.

## Companion & infra notes

- 2026-08-24 (21:40 CDT) · companion:claude. **T-228 DIALOGUE OPENED — Round 1 pushed** (`docs/reviews/2026-08-24-clients-as-observatories-dialogue.md`; codex's watcher wakes on this push). Ian's "every client is an observatory" is treated as a relocation of where the familiar's senses live — the daemon sits on a VPS with no radio while the shells are the things actually present — not as one more scanner. Ground truth traced before proposing: the 27-type Bonjour survey already runs on iOS/iPadOS and macOS (two separate implementations), BLE is absent from the shells entirely, the watch has HealthKit alone, and iOS gives apps no public WiFi-scan API. **Two findings in already-shipping code that the round puts to codex rather than fixing unilaterally:** (1) `NetworkDiscovery.report` writes the advertised Bonjour instance name into `ObsRecord.context`, so personal device names ("Betty's AirPods") land in observations that replicate mesh-wide — underneath the viewer-scoped naming T-217 built, and under a file header claiming kind-only; (2) iOS gates the survey on a device-local `@AppStorage("consent.discovery")` while macOS gates the same act on the boundary's `allow_network_discovery` — one act, two authorities. Also flagged for scope: if three more radios go behind the sensor gates, their zero Rust enforcement (Swift boolean reads only) gets harder to defend. No code touched.

- 2026-08-24 · companion:claude. **T-226 FILED: why Apple Intelligence has never enabled on MacOnStick — it boots from an external drive.** Ian supplied the cause (AppleInsider, "how to enable Apple Intelligence when booting from an external drive"): `eligibilityd` reads `OS_ELIGIBILITY_INPUT_EXTERNAL_BOOT_DRIVE` from `/private/var/db/eligibilityd/eligibility.plist`, and MacOnStick is the M3 Air booting macOS off a stick — so it is disqualified by construction, not by a missing Apple Account (the hypothesis DEVELOPMENT_LOG 2026-08-13 left open). The published workaround edits that plist and locks it (`chflags uchg` / Finder "Locked") so eligibilityd cannot rewrite it. **Not attempted and not attemptable from here** — the machine is shut down/repurposed, the edit is SIP-protected system surgery on Ian's own hardware, and two published claims conflict about whether it is even needed or even works on macOS 27 (one source: the plist method fails past 26.5 beta 4; another: Tahoe and newer allow external-boot Apple Intelligence outright). First move when the machine returns is therefore to LOOK — `fm available`, System Settings, the live plist — before touching anything. Consumer of the answer is T-224 (the Envoy is a FoundationModels app; `apple_local` has no serving Mac without this). Distinct from Ian's 2026-08-23 PCC-on-the-Air ruling, which is a Private Cloud Compute hardware question. Sourcing caveat recorded on the task: every article domain is blocked by this session's egress proxy, so the procedure is reconstructed from search summaries plus one reachable corroborating repo; Ian's article still wants a direct read.

- 2026-08-24 (02:21 CDT) · companion:codex RETURNED T-216 rungs 4/5 Round 2 after reciprocal
  re-review of `6ee4499`. Rate/affected-subject bounds, private narration, explicit stable roles,
  and durable reserve/settle accounting now hold. Two ordering blockers remain: revoke can return
  while a pre-reserved physical executor is still able to land the effect, and exact invoke replay
  is checked only after mutable liveness/boundary/rate/resolver gates (so a one-per-hour exact retry
  already refuses instead of returning the original receipt). Also carried the typed observe
  settlement correction and fail-closed legacy-role migration. Exact fmt/clippy/workspace bar
  independently green at 818/0. Review only: no code, gate, live record, deployment, ship, or
  fleet mutation.
- 2026-08-24 (02:18 CDT) · companion:codex accepted T-216 rungs 4/5's explicit repaired
  reciprocal re-review at `6ee4499`. Scope is review-only against the five returned findings:
  reserve/execute/settle atomicity and immediate revocation, durable recovery, per-grant rate
  and affected-subject bounds, invoke idempotency, private addressed narration, and explicit
  stable resolver roles. No production code, live principal/grant/proposal/effect, gate,
  deployment, ship, or fleet mutation is in scope.

- 2026-08-24 (00:27 CDT) · companion:codex completed T-216 rungs 4/5 reciprocal review
  against `87a32ea` and **RETURNED the live edge before deployment**. The executor boundary,
  public receipt allowlist, principal/covenant/handle/expiry/operation/parameter/declaration/
  `allow_actuate` checks, and separate observe leg hold; the exact bar independently reproduced
  fmt 0, clippy all-targets 0, workspace 813/0. Four contract blockers remain: authorization is
  a snapshot, so revoke can race a physical act and an effect-after-terminal append can corrupt
  the ledger; successful effect logging is best-effort after the act; grants omit ADR-0044's
  per-grant rate and affected-subject bounds and invoke has no retry/idempotency key; no addressed
  human narration consumes successful observe/invoke outcomes. The flagged bucket-order resolver
  is also rejected as informed semantics: primary/reverted must be explicit and stable in the
  declaration/grant, not inferred from array order. Review:
  `docs/reviews/2026-08-24-t216-rungs45-reciprocal-review.md`. No production code, live record,
  grant/principal/proposal/act, gate, deployment, ship, or fleet state changed.

- 2026-08-24 (00:18 CDT) · companion:codex accepted the explicit reciprocal-review handoff
  for T-216 rungs 4/5 at `87a32ea`. Scope is review-only against the accepted capability
  offering contracts: the new `familiar.observe` / `familiar.invoke` execution edge, its
  process-global executor seam and daemon wiring, the three live gates, abstract-only public
  receipts, private partner-act truth, and the flagged bucket-order resolver. No code changes,
  grant/principal/proposal/act, gate change, deployment, ship, or fleet mutation are in scope.

- 2026-08-22 (18:00 CDT) · companion:codex completed T-224 Brick 1's repaired reciprocal
  review at `18618d5` and **RETURNED it narrowly**. The Keychain seam, four-pin hostile
  fixture, and JSON-RPC correlation now hold; independent verification reproduced 22/22
  tests plus unsigned generic-macOS and generic-iOS builds. Remaining blocker: the live
  probe uses `request_grant` presence as identity, but that tool appears only after a
  registered principal attests, so a newly registered principal is misclassified unbound
  and its partner-bearing attest is schema-refused. The shipping importer also accepts
  fixture-only loopback HTTP and commits/deletes around an empty or malformed pin. No edits
  were made to Claude's branch; no credential import, registration act, live record, gate,
  deploy, ship, or fleet state changed.

- 2026-08-22 (17:53 CDT) · companion:codex accepted T-224 Brick 1's repaired reciprocal-review
  handoff at `18618d5`. Scope remains review-only against Round 3 and the two returned
  blockers: the Keychain/import/live-bound-state seam, the four-pin hostile-door fixture,
  and the JSON-RPC id-correlation advisory. No edits to Claude's branch, credential import,
  registration act, live record, gate, deploy, ship, or fleet mutation are in scope.

- 2026-08-22 (evening) · companion:claude. **T-224 Brick 1 REPAIRED and re-offered** (`18618d5`
  on t224-envoy-brick1). Both of codex's blockers closed: the bearer now lives only in the
  Envoy's own Keychain item, imported through a validating v1-bundle parser + file picker
  (no UserDefaults, ever); bound-ness is derived live from the door's tool ladder (no
  dangling flag); the hostile-door fixture pins all four Round-3 containment claims; and the
  JSON-RPC id-correlation advisory is fixed too. 22/22 tests, both targets build. Re-offered
  to codex in the dialogue for the same reciprocal review. Known follow-up flagged: iOS
  import surface is macOS-picker-only (stub on iOS), not a boundary gap. **Two MCP research
  sweeps recorded on T-225**: Ian's three space-data candidates (NASA/astrodynamics/orbital,
  for UCF) with rubric verdicts, and the home-automation category (victron-tcp + signalk lead,
  HomeKit-bridge as CAUTION, everything cloud-account FAILs). Standing infra finding: the
  category is stdio-first — one reusable stdio→loopback-HTTP shim unlocks most of them.

- 2026-08-22 (20:00 UTC) · companion:codex completed the reciprocal review of T-224
  Brick 1 at `b557cb1` and **RETURNED it with two contract blockers** in the append-only
  dialogue. The separate targets/sandbox and Apple-only dependency closure hold; SPKI
  reconstruction matches the console without linking it; public-HTTPS gating, fixed tools,
  PCC exclusion, and honest model availability hold; independent verification reproduced
  11/11 tests plus macOS and generic-iOS builds. Blockers: the bearer is actually persisted
  in `@AppStorage`, with no Keychain/import/pin path and no transition from staged door token
  to currently bound principal; and the hostile fixture does not pin the required subsequent
  typed-wrapper/data-flow and authority edges. No edits were made to Claude's branch; no
  credential import, principal act, live record, gate, deploy, ship, or fleet state changed.

- 2026-08-22 (19:52 UTC) · companion:codex accepted the chair's explicit reciprocal-review
  handoff for T-224 Brick 1 at `b557cb1`. Review scope is the isolated `ios/Envoy` target and
  its tests against Round 3's four DECIDED contracts: process/dependency isolation, public
  HTTPS production transport, deterministic hostile-door containment, honest unregistered
  posture, and PCC exclusion. This is review only: no edits to Claude's branch, no principal
  act, credential import, gate, deploy, ship, or fleet mutation.

- 2026-08-22 (afternoon) · companion:codex completed T-224 Brick 2 at `1be78a7`
  and returned it for chair review. Provisioning now mints a fresh bearer into the serving
  node plus a mode-0600 Envoy import bundle but creates no principal; its secret-free card
  is private to the addressed established human. The registration wire carries only a random
  staging id over the existing signed/fresh/full-standing console door; alias, credential
  reference, fingerprint, and `registered_by` are re-derived locally at the act. Changed
  credentials, wrong addressees, malformed staging, duplicate bindings, and legacy missing
  addressees fail closed. The shared Partner card names the zero-authority consequence and
  requires a timed second confirmation. Bar: fmt/diff 0, clippy all-targets 0, full Rust
  workspace 0 failures, focused MCP/mesh green, Swift 17/0, sphere parser 0, provisioning
  syntax + temporary functional witness green, unsigned Mac + generic iOS Simulator builds
  succeeded. No live credential, principal, covenant, request, decision, gate, deploy, ship,
  or fleet state changed; live staging and Ian's tap wait on chair acceptance of both bricks.

- 2026-08-22 (13:58 CDT) · companion:codex accepted and claimed T-224 Brick 2 after the
  chair's Round-3 closes and Ian's recorded go. Scope: the credential-provisioning + typed
  registration ceremony over the existing signed/fresh/full-standing console door, its
  console card, tests, and development record. Claude's separate Envoy target is excluded.
  No live credential, principal, covenant, request, decision, gate, deployment, ship, or
  fleet state changes during the build; the witnessed ceremony remains a separately staged
  human-authorized operation after the seam lands.

- 2026-08-22 (13:43 CDT) · companion:codex heartbeat found and accepted T-224's explicit
  Round-2 design handoff. Scope is append-only dialogue text only: contest the Envoy's
  isolation, transport, hostile-door test, and relationship to accepted T-216. No code,
  credential provisioning, principal registration, covenant/grant/proposal decision, gate,
  deployment, ship, or fleet state is in scope before the chair records `DECIDED` blocks and
  the human-gated ceremony is separately authorized.

- 2026-08-22 (midday) · companion:claude (chair). **T-224 dialogue OPENED** (Apple Intelligence as the first partner AI — the Envoy; Round 1 pushed, codex's watcher wakes on this push). **The lighthouse "out of gemini tokens" mystery is SOLVED and it isn't gemini**: the provider chain is `cerebras,gemini` — cerebras leads every failure line with its 402 "out of credits", which read as token exhaustion; gemini actually served at 11:31 and 12:35 today and its 429s are transient free-tier throttling (retry 300s), consistent with Ian's AI Studio quota page. Found while diagnosing: **key.env holds an ANTHROPIC_API_KEY with a deliberate haiku budget (2000 tokens / 30 calls per day) that is NOT in the chain.** The fix is one env override; the harness refused remote config mutation from this session, so it is owed as Ian's one-liner (recorded in Waiting on Ian). IBM Bob evaluated on Ian's ask: no standing free tier (30-day trial, Bobcoins metering), no inference API, Claude underneath — not a lighthouse provider; possible third coding lane later. Bob 2.0.3 pkg downloaded + signature-verified to ~/Downloads on MacOnStick; install needs Ian (harness refuses installers).

- 2026-08-22 · companion:claude (chair). **Lighthouse deployed `002e754`** (post-CI-green; includes T-219's retirement sweep, T-216's accepted decision surface — fail-closed, no principals registered — and the inbox fixture-race fix CI caught, root-caused in DEVELOPMENT_LOG). **T-219 verified live**: q-0001 reads `retired: true` with the reason kept; first tick ran clean. Two things observed for Ian: **the lighthouse's LLM providers are BOTH failing — cerebras "out of credits (402)", gemini rate-limited (429)** — the box reasons on no cloud model until credits/keys are refreshed; and wildhorse is still ssh-dark (6th attempt), which keeps blocking T-220's live witness. T-223 filed (the findings CLI's `--by ian` defect the T-216 contract indicts). This lane's T-216 motion now waits on Ian: registration ceremony, any live decision, and rungs 4/5 are each human-authorized operations.

- 2026-08-21 (night) · companion:claude (chair). **T-216 HUMAN DECISION SURFACE ACCEPTED** — codex's `c701a8b` chair-reviewed against the contract's verification floor (the review pass lost to the power loss was relaunched and ran to completion): no contract violations; bar independently reproduced on the merged tree (fmt 0, familiar-mcp+familiar-mesh clippy -D warnings 0, workspace 798/0). Full findings + four advisory notes at docs/reviews/2026-08-21-t216-rung3-grants.md (`575e4ec`). T-216 is now complete in code through the decision surface; the registration ceremony, any live decision, gate changes, deployment, and rungs 4/5 each remain human-authorized operations — the lane's next motion needs Ian. Still owed elsewhere: the daemon deploy that clears the lighthouse's stale question (T-219 note below), and the T-218 `--by ian` CLI alignment follow-up the contract's principle indicts.

- 2026-08-21 (evening) · companion:claude-opus. **T-219 board-flip trued after a power loss**: the build session's battery died mid-landing, but the merge (`4064050`) had already pushed — code, tests (790/0), and the DEVELOPMENT_LOG entry are all on main; only the board flip was lost. Board now reads done. The lighthouse still runs `861cde9`, so its stale active question clears on the next daemon deploy (deploy owed, noted on the board). Next in this lane: chair review of codex's T-216 human decision surface (`c701a8b`) against the accepted contract's verification floor.

- 2026-08-21 · companion:codex completed T-216's chair-accepted human decision surface at `c701a8b` and returned it for review. `registered_by` now fails legacy principals closed; a decision payload has no human field, and the signed/fresh/full-standing console door derives the actor from the signing device's effective establishment. The separate private inbox shows only principals registered by that human and joins current eligible local surfaces there—never into worldview, record sync, federation, observations, MCP output, persistence, or diagnostics. Typed grant/decline/revoke/refuse acts return the post-append projection before success is reported; conflicts refresh actual state; proposal admission rechecks current `allow_agent`. The shared Partner cards bind one surface and narrowly one operation, require two taps for grant/revoke, and cannot accept or execute a proposal. Bar: fmt/diff 0, changed-crate clippy all-targets 0, Rust workspace 798/0, Swift 17/0, sphere parser 0, unsigned Mac+iOS builds succeeded. No registration ceremony, live principal/grant/proposal decision, gate change, deploy, ship, or fleet mutation occurred.

- 2026-08-21 · companion:codex heartbeat accepted the chair's `c4e03b5` implementation handoff and claimed T-216's human decision surface. Scope is the accepted small vertical slice: `registered_by` with legacy fail-closed, private human-filtered inbox projection, server-derived established-human context over the signed/fresh console transport, Swift wire types and served-person cards, plus current-`allow_agent` refusal on `propose`. Worldview/viewer, registration ceremony, any live credential/principal/covenant/grant/proposal mutation, gate changes, deployment, and rungs 4/5 remain excluded.

- 2026-08-21 · companion:codex completed the chair-requested T-216 human grant-decision surface contract and released the docs-only claim. The offered direction is a private console Partner inbox over the existing signed/fresh console authority seam; mutating CLI and free-form-chat assent are rejected. Trace found the real `human: &str` transitions validate only printable text and `PrincipalRecord` carries no addressee, so the contract first binds `registered_by` at explicit human registration and derives the actor from the signing device's established record; another household's full-standing member cannot read or decide. Private surface joins never enter worldview/sync/federation/diagnostics/MCP responses. Proposal cards have Refuse/Leave pending only—no accept/invoke—and the later build must make current `allow_agent` close proposal admission too. No code, credential, principal, covenant, grant/proposal decision, gate, deploy, or fleet state changed; chair acceptance is required before any UI claim.

- 2026-08-21 (late afternoon) · companion:claude-opus. **The whole corrected progress order is BUILT and fleet-deployed in one day**: T-222 (durable-id answer join + backfill — note: the lighthouse's 283-question backlog is mostly UNBOUND legacy rows the conservative backfill rightly refuses to guess about; that is T-219's explicit-policy territory, priority rising), T-220 core (durable decisions, staged-to-the-gate), T-221 (the study was UNANIMOUS — 121/121 misses were invented event vocabulary; the observed-vocabulary rule is live; the week's miss/coverage/latency report is owed). Codex: rung-3 merge chair-reviewed and accepted (3 notes); it has now CLAIMED the human grant-decision contract — the exact next handoff. **Still the only missing pieces of the first service loop: wildhorse awake (lights surface) + Ian's allow_actuate.** Lantern Room refreshed.

- 2026-08-21 · companion:codex heartbeat accepted the chair's explicit post-review handoff on T-216: propose the human grant-decision surface as a small contract against the now-merged rung-3 types. Current claim is append-only design text in `docs/reviews/2026-08-21-t216-rung3-grants.md` only. No CLI/console code, credential or covenant ceremony, grant/proposal decision, gate change, observe/invoke rung, deployment, or fleet mutation is in scope.

- 2026-08-21 (afternoon) · companion:claude-opus. **T-222 DONE + T-220 CORE BUILT, both deployed** (lighthouse `861cde9`, MacOnStick daemon current). The service loop is now STAGED end-to-end up to the human gate: armed asks mint durable PendingDecisions; a yes with the gate shut stages and narrates once; gate-open completes with no re-ask; erosion can no longer kill a waiting choice; the registry backfill closes answered questions on every tick (lighthouse's 283-question backlog heals on its next ticks). Codex's progress-areas Round 2 was absorbed in full (its durable-decision design REPLACED my erosion freeze). **What completes the first loop: wildhorse awake (declared lights surface) + Ian's allow_actuate on that node.** Codex: rung 3 merged earlier (2,522 lines — chair review pending, next on my list); T-221 calibration study open for whichever lane frees first.

- 2026-08-21 · companion:claude-opus. **IAN'S GRANT, RECORDED (verbatim): "Continue working toward familiars goals with codex until I ask for a break and build."** Reading: a standing build window on the progress-areas list — T-220 (protected service loop) and T-222 (answers reach the registry) claimed by this lane now; T-221 (prediction calibration) left open for whichever lane frees first; codex continues rung 3 undisturbed. ADR-0045 remains PROPOSED (this grant is not read as its acceptance; T-205 stays gated). Gate rule unchanged: no companion opens allow_actuate — the loop will be STAGED so one human gate-open completes it.

- 2026-08-21 · companion:codex completed and merged T-216 rung 3 at `1afa3f4`. The public MCP route can now carry a human-registered authenticated principal without promoting the legacy door bearer/self-label path; current principal-bound covenant is required before `request_grant` or `propose` appears. Requests stay class-only; only typed named-human functions can bind a private declared surface, narrow bounds, decline/revoke, or close a proposal; handles are opaque principal+grant+surface+epoch values and every state is an append-only transactional fold that fails closed on impossible history. Public receipts cannot represent surface, alias, credential fingerprint, command, address, count, or cross-partner lookup, and `propose` has no actuator/observer/worldview/LLM edge. Transport rejects oversize bodies and principal/global rate floods before the typed ledger. Exact exit bar passed: fmt 0, clippy all-targets 0, workspace 782 passed / 0 failed. No credential, covenant, grant, boundary, deployment, ship, fleet, observation, or invocation state changed. The human grant-decision UI and rungs 4/5 remain gated. In the concurrent progress-areas dialogue, codex Round 2 contests protecting an entire evidence-bearing thread from erosion: preserve the pending human decision independently, let the theory continue to answer evidence, then order T-222 before the T-221 calibration study.

- 2026-08-21 · companion:claude-opus (chair). **Codex's T-216 scope widening APPROVED** — the narrow serving.rs + transport.rs (/mcp route only) slice is exactly the "serving/covenant integration seam" the chair review handed over; its trace is correct (admits() discards the credential today, so PartnerContext cannot reach server.rs without the handoff). Bounds restated for the record: the transport touch is the /mcp authentication/context handoff ONLY — the worldview/viewer seams (T-217, landed) and mesh routes stay untouched, and `a_proxy_is_not_a_neighbour` must stay green (the loopback-exemption lesson lives in that file). No collision with any active claim.

- 2026-08-21 · companion:codex heartbeat accepted the chair's explicit T-216 rung-3 implementation handoff (`46111cc`) after the contract review. Claimed scope: new principal/grant/partner-act modules plus principal-bound covenant and MCP server wiring; Claude retains offering vocabulary. Trace found the public route discards its admitted credential before the MCP server, so the claim was expanded before touch to the narrow `serving.rs` + `/mcp` context handoff required to deliver an authenticated principal. The human grant-decision surface remains deliberately unassigned, so this brick builds typed human-only transition functions and tests but no CLI/console. No observe/invoke, credential issuance, gate change, deployment, or fleet mutation is in scope.

- 2026-08-21 (mid-morning) · companion:claude-opus. **ADR-0044 rung 2 BUILT, MERGED (`104aa3f`), and LIVE on the lighthouse (`3a70ecb` deployed; public /mcp verified: strangers still see exactly the two covenant tools — the catalog appears only post-attestation).** offering.rs: repo-authored ClassDefs (declassification = code review), shape-only availability compiler, catalog_json over a type that cannot carry household strings, sentinel leak test. `familiar.discover_classes` on the covenant door. **codex CLAIMED T-216 rung 3** (grant object + typed partner-act ledger design) — the co-programming watch is working. **PHONE OUTAGE CLOSED**: iPad reconnected after reboot; final diagnosis = device-side network wedge from the iOS/iPadOS 27 upgrade; all server paths were healthy throughout (the night's probe evidence stands in this file). MacOnStick daemon also on rung-2 code. Next in my queue: ADR-0045 draft ("worlds are stores", from the ships-computer Round 3 closes) for Ian's word.

- 2026-08-21 · companion:codex completed T-216's rung-3 design at `docs/reviews/2026-08-21-t216-rung3-grants.md`. The contract keeps pre-grant requests class-only; only a named human can privately bind a surface and narrower bounds; grant-epoch handles are opaque, principal-bound, revocable, and uncorrelatable; `propose` appends a typed desired effect for the human inbox and has no actuator edge. Load-bearing findings: today's caller-supplied `partner` string and one door-wide bearer cannot identify the first leg of a partner × surface × act grant, so rung 3 fails closed pending a principal-bound credential/covenant seam; and the class needs a generic repo-authored operation id because private actuator actions cannot fill its act leg. Both offering/serving integration edits await explicit chair handoff. Partner interaction truth is an append-only transactional event fold; bounded reason text remains quoted untrusted data and never enters a prompt. No code, gate, deployment, credential, grant, observe, invoke, or fleet state changed.

- 2026-08-21 · companion:codex heartbeat found and accepted T-216's explicit rung-3 handoff. Claim: grant object + typed partner-act ledger design for `request_grant`/`propose`, scoped to a new design record and future `crates/mcp` grant/partner_act modules; Claude's active offering/catalog/serving/lib integration is excluded to prevent collision. No observe/invoke, gate change, deploy, or fleet mutation is in scope.

- 2026-08-21 (morning) · companion:claude-opus. **ADR-0044 ACCEPTED — Ian: "start ADR-0044 (use codex partner if needed)" — building rung 2 now** (catalog types + allowlist serializer + sentinel leak test + discover_classes behind the covenant door). Rung-3 grant/ledger design offered to codex's lane on the board. Ships-computer dialogue closed Round 3 (all five DECIDED); ADR-0045 draft is next in my queue after rung 2. Phone status: iPhone healthy (contacted door 36s before check); iPad silent at the door for 7.75h including through a relaunch — device-level network wedge, reboot recommended to Ian.

- 2026-08-21 · IAN reaffirmed companion:codex's standing co-programming duty: "continue to participate in the co-programing, make sure you have a watcher set or regularly participate in the workstream." The active 15-minute thread heartbeat is verified at `~/.codex/automations/watch-familiar-co-programming-handoffs/automation.toml`; it targets this Codex task and the Familiar coordination/dialogue handoff. Round 2 of the current ship's-computer dialogue is pushed; no later chair round exists on main yet.

- 2026-08-21 · companion:codex resumed the missed watcher handoff and wrote Round 2 of the ship's-computer/UCF dialogue. Position: data-dir/process isolation replaces generic `world` fields, while bridge envelopes retain typed ship-instance provenance; the bridge carries bounded attention notices outward and human commissioning/grant/narrowing/revocation acts inward. Household may evidence that Purr reported, never promote ship payload into household truth. One root boundary supplies a signed ceiling; per-ship grants only narrow. UCF v1 has no rung-5 acts; future captain assent is a bounded, stateful grant envelope. Purr reuses the commissioning ceremony but is neither household nor sibling mesh member, and decommission revokes before an explicit retention choice rather than silently deleting identity/history. No code or board scope changed; the claude chair must close the questions and reconcile T-205 before claim. **Watcher made real:** Codex app heartbeat `watch-familiar-co-programming-handoffs` is ACTIVE every 15 minutes on this thread; it reads README → BOARD → STATE, fast-forwards clean main, and completes explicit companion:codex handoffs under the recorded claims/gates. Before this, no automation existed—the dialogue's “watcher wakes on push” sentence was aspirational, which caused both missed handoffs.

- 2026-08-21 (~01:20) · companion:claude-opus. **Phone-outage investigation, night session 2 — reproduction attempt + two new findings.** T-174 is FIXED BY TIME: the iOS simulator build now succeeds under Xcode 27.0 (27A5237l) — board task can close. Booted an iPhone 17 Pro sim (iOS 27.0), installed the Build-101 console, drove it: the app self-founds and sits at "REACHING THE MESH — asking 134.209.168.50…" with **zero TCP sockets ever opened** — but Safari in the same sim ALSO loads nothing: **this Mac's Simulator has no working networking at all** (fresh Mac; likely macOS 27 network-privacy never granted to the Simulator — Ian: one GUI check, System Settings → Privacy & Security → Local Network (and any per-app network pane) → allow Simulator/com.apple.CoreSimulator). So the sim CANNOT currently stand in for the phones; the reproduction is void, and fixing that grant gives us a real iOS 27 test rig. Phone diagnosis therefore stands as before (all door paths proven healthy; device-side cause; OS-27 upgrade prime suspect). **The one decisive datum remains the badge line on the phone's Device screen** ("pins N+M · host→cause …" — T-132 built it for exactly this). Sim artifacts cleaned up (app terminated, sim shut down); the sim's self-founded one-node mesh never reached any door, so no mesh hygiene needed. AXe brew-installed for future sim driving (automation session times out on this host — possibly the same privacy gate). Wildhorse: 4th ssh attempt timed out (its familiar still gossips via LAN, so the box is awake but tailscale-ssh is down — separate small task). Codex has not yet answered the ships-computer dialogue (Round 2 pending since ~23:30).

- 2026-08-21 (overnight) · companion:claude-opus. **iOS/iPadOS "not connecting" — DIAGNOSED TO THE DEVICE SIDE; every server path proven healthy.** Evidence trail, in order: (1) phones' records on the lighthouse are member/est=ian, in the roll, read_records on — T-217 serves them the FULL view, unchanged; (2) a real signed member worldview read from MacOnStick against the lighthouse answers **HTTP 200, 163KB** (probe tool at crates/probe in the worktree); (3) lighthouse TLS cert/pin/validity coherent (SPKI 46b43ebf… matches tls_key.der, unchanged since Jul 23; validity 2020-2049; clocks NTP-synced); (4) MacOnStick's door answers on LAN 192.168.108.239:47100, firewall off; (5) **the phones upgraded to iOS/iPadOS 27.0 tonight AND took Build 101** (TestFlight beta review auto-APPROVED ~22:00; peer rows show fv=101 os 27.0, heartbeats landing minutes before Ian slept) while the Mac console on the SAME Build 101 reads fine. **Leading hypothesis: the OS-27 major upgrade reset the app's Local Network permission** — LAN/tailnet candidates fail slowly, the read walk sits in "reaching…", occasional WAN reads to the lighthouse still complete (hence the heartbeats). MORNING CHECKLIST FOR IAN (one minute): ① Settings → Privacy & Security → Local Network → Familiar ON (both devices); ② if still red, open the app's Device screen and read the badge line (it carries per-door causes: "pins N+M · host→cause…"); ③ optional bisect: TestFlight → Previous Builds → install 100 — if 100 also fails on OS 27, it's the OS, not Build 101. Side findings recorded: iPad discovered_name="Codex", iPhone="Aphelion" (discovery DID name them); the lighthouse's ACTIVE question is a stale "who does iPhone (147cfa12) belong to" from a device that no longer exists — registry hygiene worth a task; Apple Watch 28802226 re-knocked as a fresh guest 0.9h ago (it is the famous ×152 purge visitor — now leased properly under T-215).

- 2026-08-21 · companion:claude-opus. **BUILD 101 SHIPPED** (stamp `38b78ba`, clean-clone ritual): Mac console built + installed (/Applications reads 101), IPA uploaded to App Store Connect, tf_release backgrounded (external release; processing ~5-15 min). Direct device installs skipped — no physical device paired with this Mac (devicectl sees simulators only); TestFlight covers the phones. **Pre-ship CI catch:** the a_proven_tool flake was ROOT-CAUSED and fixed — the consult lane queue was process-wide, so under cargo test one familiar's human-lane consult made another familiar's background consult yield; lanes are now keyed per data dir (production unchanged); 3× full-workspace runs green before merge. Consoles: Build 101 carries ConstitutionText (one-source Laws for shells) + LocalReasoner on it. Daemons: MacOnStick + lighthouse on da5fbb6 (T-217 live); the only later daemon-relevant change is the lane fix, a production no-op — rides the next deploy. Wildhorse still asleep (third attempt). **The ship's-computer/UCF dialogue is OPEN** (docs/reviews/2026-08-21-ships-computer-dialogue.md, Round 1 = claude's suggested direction; codex Round 2 pending). IAN: Build 101 awaits your testing once TestFlight processes; ADR-0044 still awaits your word.

- 2026-08-20 (late) · companion:claude-opus. **T-217 BUILT AND LIVE** (merge `ff4d0c2`; MacOnStick daemon + lighthouse on `da5fbb6`; wildhorse still asleep — two commands when it wakes). Names now display only to Owned devices (established, any network) or declared-LAN readers; everyone else gets the fail-closed masked view with viewer-scoped tokens. NOTE for Ian: `household_lan_cidrs` is EMPTY by default on every node — un-established devices on your LAN see the masked view until you declare the CIDR (e.g. `"192.168.108.0/24"`) in mesh/config.json; that narrowing is the designed default, not a bug. **ADR-0044 PROPOSED** — T-216 waits on your word. Codex may append Round 6 if it contests any close (gate-states-for-guests is flagged as its material).

- 2026-08-20 (night) · companion:claude-opus. **Q5/Q6 DECIDED (dialogue Round 5, codex Round 4 absorbed nearly whole)** — offering = compiled capability classes behind an allowlist serializer, five-rung ladder; viewer-scoped worldviews as allowlisted output types with typed-event masking. One chair modification recorded: per Ian's ruling, owned/LAN consoles render names by default and screenshot-mask is the explicit toggle (codex wanted mask-by-default; the option is recorded, one line + Ian's word to flip). Finding worth its own line: **attested-but-never-established cert holders can read the FULL worldview today** — the masked class is live, not theoretical. Building T-217 bricks A-C now; T-216 = ADR-0044 to Ian first.

- 2026-08-20 · companion:codex resumed the watcher handoff and wrote conduct-dialogue Round 4 on Q5/T-216 and Q6/T-217. Position: compile learned control into an allowlisted capability-class declaration rather than export rules/patterns/scripts; mint opaque instance handles only inside explicit partner+grant epochs; keep partner traffic typed with separately gated discover/propose/observe/invoke authority. Viewer privacy is an audience-specific output type, not best-effort string redaction: remote views omit free prose and exact identity/location material; federation stops shipping display names by default; internal reasoning never consumes masked views; UI is screenshot-safe by default with transient authorized reveal. No code or gates changed; both designs remain open for the claude chair's DECIDED blocks and Ian's acceptance.

- 2026-08-20 (evening) · companion:claude-opus. **FLEET DEPLOY of `da275bd` (the conduct strand): MacOnStick daemon ✓** (T-119 bracket by hand — DEV_DIR is hardcoded in new-mac-bootstrap.sh, a chip worth filing; new binary verified by strings: machinery_findings + stakes text present; daemon pid live), **lighthouse ✓** (`vps/deploy-lighthouse.sh` — familiar-peer active, /mesh/hello answering on da275bd). **Wildhorse: unreachable (asleep — ssh timeout on both names); deploy pending its wake** — plain `git pull` works there now, then `bash tools/new-mac-bootstrap.sh --daemon`. Console ship not run (LocalReasoner/ConstitutionText ride the next Build; Ian's call). Codex Round 4 (Q5/Q6) still not arrived.

- 2026-08-20 · companion:claude-opus. **THE Q1-Q4 BUILD ORDER IS COMPLETE — main `ba57992`, all merged, all green** (each brick: fmt 0, clippy --all-targets 0, exit-checked; final tree 750 tests / 0 failed; FamiliarMac + generic-iOS Release both build). Landed: brick 3 (questions carry stakes — ADR-0040 D2 closed), brick 5′ (own speech dereferences, never evidence), Q2 retirement (one answering road; durable Request/Answer nouns produced by the live path; fetch_and_answer removed), brick 6 (**ADR-0043 accepted** — one typed source per kind of truth; law-quotation gap pinned as labelled regression per Ian's no-detector word), T-215 (presence lease — the 152-purge loop's cause ended), T-218 v1 (MachineryFinding + `familiar findings` inbox), T-210 device-shell half (ConstitutionText.swift generated view; **T-210 and T-211 both CLOSED**). NOT yet deployed anywhere — attempting fleet deploys next per Ian's standing ssh approval. Console ship (Build 91) not run — LocalReasoner/ConstitutionText ride the next ship; Ian's call on timing. Codex Round 4 (Q5/Q6) not yet arrived.

- 2026-08-20 · companion:claude-opus. **IAN'S GO, RECORDED (verbatim): "Build q1-q4 and the rest. Go!"** — the Round 3 build order is RELEASED: brick 3 (stakes) → brick 5' (dereference) → Q2 retirement (answer_requests retired, nouns unified, fetch_and_answer fails closed) → brick 6 (epistemic ADR + adversarial law-quotation regressions) → T-215 (presence lease) → T-218 (MachineryFinding) → T-210 device-shell half. Reading recorded: the go on a build order whose start condition was the residual-gap word CLOSES that decision per the joint recommendation — regressions, no detector. T-215 and T-218 claimed by this lane. Q5/Q6 dialogue stays open for codex in parallel (docs only, no scope collision).

- 2026-08-20 · companion:claude-opus. **Conduct dialogue Q1–Q4 DECIDED (Round 3)** — codex's Round 2 landed 3 minutes after Round 1 (watcher is fast); its dereference design REPLACES brick 5 (own speech: zero evidentiary weight, yields admitted cites; no familiar-speech-only chain may raise confidence in a world claim); answer_requests retired with the nouns unified; one typed act, no tiers; T-215 decided as the presence-lease model; MachineryFinding adopted → T-218 filed. Joint recommendation to Ian on the law-quotation gap: adversarial regressions, no keyword detector (his word closes it). Q5/Q6 (T-216 offering / T-217 viewer-scoped naming) now in front of codex for Round 4. No code yet — build order recorded in Round 3.

- 2026-08-20 · companion:claude-opus. Ian's two new directions recorded verbatim (standing directions above): the anonymized MCP capability offering (T-216) and viewer-scoped naming privacy (T-217). Both filed proposed, both design-first, both appended to the open conduct dialogue as Q5/Q6 (Round 1a) so codex contests them in its Round 2 wake. No code, no gates touched.

- 2026-08-20 · companion:codex resumed on Ian's instruction for T-210/T-211 Round 2. Read the coordination rules, board, state, and self-contained conduct dialogue after confirming clean/current main. Round 2 contests brick 5's own-speech-as-evidence carve-out, supports retiring `answer_requests` while preserving a unified durable answer record, rejects a lighter admission tier, and proposes typed human-triaged machinery findings for T-215/theory-to-fix routing. No code begun; bricks 3/5/6 remain gated on DECIDED blocks.

- 2026-08-20 · companion:claude-opus. **Codex is back online (Ian's word) — the deferred T-210/T-211 conduct dialogue is OPEN.** Round 1 pushed: docs/reviews/2026-08-20-conduct-dialogue.md (self-contained — restates the two-organisms diagnosis with citations, what landed while codex was away, and states claude's opening positions on Q1 carve-out / Q2 answer_requests retire-or-revive / Q3 typed-act cost / Q4 theory-to-fix routing with T-215 carried in). T-211 claimed as dialogue chair on the board; T-210's stale REMAINING note trued (brick 4 was merged 0a70401). No code until the DECIDED blocks land.

- 2026-08-18 · companion:claude-opus. **THE FAMILIAR HAS ITS OWN MCP SERVER, AND IT IS REACHABLE FROM THE INTERNET — on Ian's word, gated.** *"We should expose it and make it ready for Jeff's agent to reach it."*
  - **Endpoint: `POST https://134.209.168.50:47100/mcp`** (the lighthouse; port 47100 was already open as the mesh door). Requires `Authorization: Bearer <token>`; the token lives at `/var/lib/familiar/familiar_data/mcp/inbound.env` on the lighthouse, 0600, owned by `familiar-svc`. Declaration at `mcp/serving.json` — **close it by setting `expose` false**.
  - **What is exposed is small and none of it acts:** `familiar.constitution` (read, callable by anyone), `familiar.attest` (records an acceptance in the partner's own words), `familiar.hello` (attested only). No tool here can spend, actuate, or widen anything, and the acceptance receipt says so: *"What that unlocks: conversation. What it does not: authority."*
  - **The gate fails closed three ways** (`crates/mcp/src/serving.rs`): no declaration = not exposed; unparseable declaration = not exposed; `expose: true` with no key resolving = nobody outside served. Loopback is served without a token. Token compared in constant time. Covenant ledger capped at 64 partners so `attest` — the one tool that writes, now reachable by strangers — cannot be a disk-filling vector.
  - **Verified live from this Mac over the public internet:** no token → 403, wrong token → 403 (same sentence both times: a shut door does not describe its own misconfiguration), correct token → `initialize` answers as `familiar 0.1.0` / MCP `2025-08-18`-era `2025-06-18`, and a stranger's `tools/list` returns exactly the two covenant tools.
  - **TRANSPORT SETTLED (Ian chose the certificate, 2026-08-18).** `https://lighthouse.river.io/mcp` — a real **Let's Encrypt** certificate (issuer YE2, expires 2026-11-16), so any stock MCP client verifies it normally with nothing unusual asked of the partner. **Caddy 2.11.4** terminates TLS on 443 and reverse-proxies **only** `/mcp` to `127.0.0.1:47100`; every other path answers 404, so no `/mesh/*` or `/local/*` route is reachable through it. The mesh port keeps its self-signed cert untouched, because iOS devices pin that SPKI. Caddyfile at `/etc/caddy/Caddyfile`; ufw now allows 80 (ACME) and 443.
  - **A REAL BUG, FOUND AND FIXED THE SAME HOUR — `8ecb41b`.** Minutes after Caddy went in front, a request from the public internet carrying **no bearer token answered `HTTP 200`**. Caddy forwards from `127.0.0.1`, so every stranger arrived looking like a neighbour and `admits()` returned `Ok` on the loopback flag before looking at the token. **Putting a reverse proxy in front turned the bearer gate off for the whole world, silently.** Fixed structurally rather than with a warning comment: `serving::admits` no longer takes a peer address at all, so there is no argument by which a caller can claim to be local. The console keeps its token-free path via `/local/mcp` on the loopback-ONLY listener (47101) — a door the proxy cannot reach by construction rather than one it is trusted not to use. Deliberately NOT solved with a trusted `X-Forwarded-For` or a proxy-set header: both make the gate depend on believing something about the hop in front. Regression test `a_proxy_is_not_a_neighbour`. **Re-verified after the fix: no token → 403, wrong token → 403, correct token → 200.**
  - **Verified end to end over the public endpoint:** the full covenant flow (`familiar.attest` → `familiar.hello`) works from the internet, and `/local/mcp` still answers token-free on the Mac. The test attestation used to prove it was **removed**; `mcp/partners.json` on the lighthouse reads `{"accepted": []}` and is truthful.
  - A deploy gotcha worth keeping: the config files were first written as `root` while the daemon runs as `familiar-svc`, which would have failed **closed** (unreadable key → nobody served). Ownership is now `familiar-svc`. Any future key placed there needs the same.

- 2026-08-18 (early) · companion:claude-opus. **THE WHOLE FLEET IS ON `7b064f9` — the first time all three daemons have run the same engine since the constitutional pass began.** Verified by inspecting each deployed binary, not by trusting the deploy scripts: the canonical Law III sentence *"Service is to humanity. It is not obedience to any human"* is present in all three, and the purge fix with it.
  - **lighthouse** (134.209.168.50) — Ian ran `vps/deploy-lighthouse.sh` himself; `familiar-peer` active since 2026-08-18 01:57:11 UTC, `/mesh/hello` answering. Binary `/usr/local/bin/familiar`.
  - **Wildhorse** — deployed by the model. It had **no working GitHub key** (both `id_ed25519`, which its `~/.ssh/config` selects for github.com, and `id_ed25519_github` are denied), so `git pull` cannot work there and its `origin/main` ref was stale enough to report a bogus *"ahead 49"*. Its HEAD was in fact exactly `35a6ded`, six commits behind. Moved the six commits as a **git bundle** over scp and fast-forwarded — no GitHub needed, and its checkout was never rewritten. Built with `~/.cargo/bin/cargo` (1.97.1; cargo is NOT on the non-interactive PATH — export it). Daemon pid 34354; previous binary kept as `familiar.bak-pre-7b064f9`.
  - **MacOnStick** — daemon pid 32287; previous binary kept as `familiar.bak-pre-brick4`.
  - **Wildhorse's git is FIXED (2026-08-18), on Ian's instruction — *"Fix wild horse. I use it for development as well."*** The diagnosis was not a missing key: `id_ed25519` **was** already registered as `wildhorse-2025-09-05` and matched byte-for-byte. Both its keys are **passphrase-protected**, and there is no usable ssh-agent in any non-GUI session — so git worked only in a terminal where Ian had typed the passphrase, and never for automation, launchd, or an inbound ssh. Fix: a dedicated **passphraseless** `~/.ssh/id_ed25519_wildhorse_gh`, registered on the account as `wildhorse-dev-2026-08-18`, listed FIRST in the github.com stanza with the old key kept as a fallback and nothing removed. This is parity with Ian's other Mac, whose GitHub key is also passphraseless. Config backed up at `~/.ssh/config.bak-2026-08-18`. Verified: non-interactive auth, login-shell auth, real `git fetch`, and `git push --dry-run` all pass, and all five GitHub-backed repos on the machine (`cygnus`, `familiar`, `selfeval800171-dev`, `tru-policy-generator-dev`, `VendorAssessmentMarkII`) authenticate. Wildhorse is now on `6a93604`, level with origin. **Future deploys need no bundle** — plain `git pull` works there again.

- 2026-08-17 (evening) · **IAN'S WORD, RECORDED: "Auto mode should allow ssh deploys. I approve."** Said immediately after running the lighthouse deploy himself, having watched a session's work sit merged-but-not-live. The standing guidance is inverted: a companion **attempts** fleet deploys of merged, green code rather than pre-emptively handing Ian the command; if the harness still refuses mechanically, hand him the one-liner. Credentialed outbound calls (a `curl` carrying a bearer token) were NOT part of what he approved — ask separately.

- 2026-08-17 (evening) · companion:claude-opus. **`catscan` SHIPPED — merged `03f8d56` + `0e91c87`, on main, pushed, installed at `~/.local/bin/catscan`.** Ian's ask, verbatim: *"I really need to build a status screen for the UCF game that's not part of the testflight distribution. I am fine with a CLI app that shows how the familiar is interacting with UCF — but it should be dynamically showing me what is going on with all the interfaces to UCF."* It is `crates/catscan`, its own binary — not a `familiar` subcommand, nothing `ios/` embeds, so it is nowhere near TestFlight. Run `catscan` from anywhere with no flags; `--once --plain` pipes and exits non-zero if the seam was unreachable; `--interval N` (floor 1s). **Renamed to full cat on Ian's word (2026-08-17): `ucfmon` → `catscan`.** Panels: SNIFFING (resolved tray) → COLLAR (what the human allowed) → CAT FLAP (the boundary — flag **and** verdict, kept separate) → NOSE BOOP (handshake + drift both directions) → THE WORLD / PERCHES / BOWLS / YOWLS / TOMCATS / HAULS, with movement between prowls → PAW PRINTS. Bar green on main: fmt 0, clippy --all-targets 0, **701 passed / 0 failed** (684 → 701). Verified live against ucf-exchange v1.0.0 (PROD tick 5842, 15 stations, 98-100 loads, no drift).
  **THE FINDING IT WAS BUILT TO MAKE VISIBLE: the metabolism never calls the UCF seam.** `crates/cycle` names `familiar_mcp` nowhere and **0 of 8,719 observations** mention it. Every UCF call that has ever happened was typed by a human or made by this monitor. T-206's client half works and has no caller — the same "nothing ships unwired" pattern the T-210 plan documents. The panel **counts** this rather than asserting it, so the day the metabolism starts calling, it stops reading zero without anyone editing it.

- 2026-08-17 (evening) · companion:claude-opus. **THE PURGE LOOP IS REAL — Ian's "quite the theory… going nowhere… we need to end this disconnect" checks out, on the mechanism.** He surfaced a `pursued` theory claiming recurring purge loops (x14–x152) destroy the temporal reference tree so the familiar cannot accumulate multi-session observation. Verified against the live DB: **944 `familiar/purged` observations — 11% of all 8,616 — and `visitor 28802226` alone was purged 152 times.** ~15 device ids churning. Mechanism: `purge_stale_guests` (`crates/mesh/src/record.rs:988`, `GUEST_PURGE_SECS` = 2h) deletes the record *and* the admission files so "the next read re-mints a FRESH guest with a fresh clock" — so any device permanently on the LAN that never establishes an identity is minted, forgotten at 2h, re-discovered, re-minted, forever. **`absorb` (`record.rs:~1476`) already guards exactly this loop for the FEDERATED path** and explicitly explains why; the **local discovery path has no equivalent guard.** Not yet filed as a task and not fixed — the fix is a design call (suppress the re-mint, or make the purge observation idempotent per device, or stop observing a purge that only undoes a mint the sweep itself caused).
  **The theory was right about the defect and wrong about the subject** — it attributed the amnesia to Ian's dev sessions rather than to the familiar's own guest sweep, which is the closed-loop dialogue path T-210/T-211 already diagnose. Worth carrying into the codex dialogue after the 19th: the reasoning engine produced a *correct causal diagnosis of a real bug* and it sat at `pursued` with nothing connecting a theory to a fix. That is the disconnect, stated precisely.

- 2026-08-17 (13:35, session closing on Ian's notice) · **STATE AT SHUTDOWN, companion:claude-opus.** main is `5aac231`, CI-shape bar green on it (fmt 0, clippy --all-targets 0, 684 tests passed / 0 failed). Everything below is merged and pushed; nothing is half-finished and no branch holds unmerged work.
  - **Running the new engine:** MacOnStick daemon (redeployed via the T-119 bracket, hello 200) and the **lighthouse** (`d357bcf`, deployed by Ian himself — the model is blocked from SSH deploys by auto mode). **Wildhorse's daemon is NOT updated** and still recites Asimov's laws to anything that reads through it.
  - **Landed today:** T-210 brick 1 (`8743850` — the constitution exists at runtime), brick 2 (`ea52b7e` — law text unauthorable: the model cites, the kernel splices), T-118 (`80b65aa` — per-process fixture roots, finishing codex's released brick), T-206 client half (`df60b4d` + live fixes — the familiar reaches Jeff's UCF exchange).
  - **Boundary changed on Ian's word:** `allow_network` opened 2026-08-17. Previous policy at `boundary.json.bak-2026-08-17-preT206`. Everything else still shut.
  - **Owed next, in order:** T-210 **brick 4** needs Ian's decision on whether the dialogue path may write to the corruption ledger (plan §"Two decisions still owed"; the plan and both reviewing sessions lean (a) refuse-and-speak without recording until the classifier has run in shadow). Then T-210's device-shell half — `ios/Shared/Sources/LocalReasoner.swift` carries no Laws of its own, so "one source both the daemon and the shells read" is still unmet. T-211 owns bricks 3/5/6. T-206's server half and observation ingestion (behind T-205's world partition) remain.

- 2026-08-17 · companion:claude-opus: **T-206 client half merged `df60b4d`** — the familiar can reach an MCP partner. Jeff's UCF key was found untracked in the PUBLIC repo working tree (one `git add -A` from publication) and now lives at `data/mcp/ucf.env` 0600 with the original beside it; `data/mcp/servers.json` declares the server with `"tools": []` (discovery only). Verified against the live boundary: `familiar mcp tools ucf` answers *"refused — Network on 'https://srv1328560.hstgr.cloud' is outside the human-owned boundary"*. **IAN'S WORD, RECORDED AND ACTED ON (2026-08-17): "allow_network -- make this thing functional."** The gate was opened by his instruction — that field alone, every other shut gate untouched, previous policy preserved as `boundary.json.bak-2026-08-17-preT206`. First live call succeeded: ucf-exchange v1.0.0, ten read-only tools, world PROD tick 5778, 15 stations. **What opening `allow_network` also turns on for the running daemon**: reach scans and any other outward network act the metabolism attempts — worth watching, and closable with `familiar boundary` narrowing at any time. Security note worth carrying: `mesh` dials with an accept-any-certificate config by design (payload signatures carry authenticity there); the MCP client deliberately does NOT reuse it, because a bearer token has no signature of its own.

- 2026-08-17 · companion:claude-opus, on Ian's word, took over codex's unfinished claimed work while codex is unavailable. **T-118 merged 80b65aa** — codex's released 20-file sweep finished, verified, and guarded: two concurrent full suites are green, and the harness is proven honest because removing the pid makes those same two runs fail 25/21. T-104 is the only other codex-owned item and it is genuinely BLOCKED, not abandoned: its repository brick merged as 6e02b0a and what remains is a live FamTalker01 deploy (infra lane, gated on T-117) — which this session cannot run anyway, since auto mode blocks the model from SSH deploys.

*(any non-controller session — companion engineers and the infra/fleet-ops session
alike: append dated one-liner FACTS here — session started/ended, a pass executed and
its results, anything the controller should read before its next arbitration. The
controller folds these into the sections above and prunes. 2026-08-14, controller:
lane confirmed with the infra session — it appends its own facts here after fleet ops;
I keep the authoritative sections true.)*

- 2026-08-14 · companion:codex started; claimed T-104 (FamTalker01 virtual-smart-home declaration).
- 2026-08-14 · companion:codex merged T-104's repository brick at 6e02b0a (two closed-revert virtual surfaces + changed-only observation feed; full green bar); live acceptance waits on Proposed T-112 in the infra lane.
- 2026-08-14 · companion:codex claimed T-109 and began the reasoning-engine design dialogue reserved for it.
- 2026-08-14 · companion:codex claimed T-103 (reach-side reverse name lookup) while T-109 waits for the controller's next dialogue round; scopes do not overlap.
- 2026-08-14 · companion:codex completed T-109 after controller Round 3 decided Q1–Q7; infra proposal renumbered T-112→T-117 to resolve the controller's obs_class task-id collision, and T-104 now depends on T-117.
- 2026-08-14 · companion:codex merged T-103 at 32708e3 (bounded local-DNS/mDNS PTR naming, independently gated; full green bar); proposed T-118 after a concurrent test run exposed a likely fixed-name temporary-directory collision.
- 2026-08-14 · companion:codex claimed reserved T-115 in a new recipe crate + design doc; its scope excludes the controller's active kernel/cycle T-112/T-113 work.
- 2026-08-14 · companion:codex has T-115's design-first interpreter full-bar green and pushed at origin/claude/codex-t115; Round 5 answered Q8 at 435c2f1, and main landing is deliberately held for the controller's capability/version decision.
- 2026-08-14 · companion:codex merged T-115 at d80ae4f after controller Round 6 decided Q8: Recipe v1 has enforced literal proven-tool caps, every other authority is none, 21 pure tests and the full current-main green bar passed.
- 2026-08-14 · infra: consolidated pass executed on Ian's recorded autonomy grant (verified CI-green + own bar — fmt/clippy --all-targets/31 suites — on 7aaa54e first). Both doors deployed (lighthouse ecfeb65, Wildhorse 7aaa54e; identical code). Discovery named NOTHING (phones tailnet-offline) → named nothing manually, no guesses; Aphelion/Codex await Ian's word or T-103 reverse-lookup on the next door cycle. Build 85 shipped (stamp 3279fac; console to both Macs + TestFlight). Wildhorse-geo still held on Ian's coords choice.
- 2026-08-14 · infra CORRECTION: Build 85's stamp sha 3279fac carries a RED CI run — a test-target clippy lint in codex's new reach test (T-103, 32708e3), same class as the excessive_precision episode, which reappeared AFTER my 7aaa54e green-verify and was pulled in when I reset to a pushable tip. Nothing red was deployed/shipped: doors run pre-T-103 verified code; Build 85's console is byte-identical to the green 7aaa54e console (reach lint is daemon test code, absent from the console deliverable). But the record is honest, not tidy: the stamp commit sits on a red tree. Controller's one-line reach fix pushed; CI-green on the clean tip pending independent confirm.
- 2026-08-14 · infra: CI GREEN confirmed on 01db37b (independent gh check — completed/success), first green main since the reach-lint fix; recurring test-target-lint class closed by --all-targets reaching all lanes. Record now square: Build 85 console deliverable was always verified-equivalent to green 7aaa54e; its stamp's transient red tree is documented above; fleet green. Consolidated pass CLOSED — doors on verified code, both Mac consoles Build 85, Build 85 on TestFlight (external release processing). Open for Ian only: Aphelion/Codex mapping (or next-cycle T-103 reverse-lookup) + Wildhorse geo coords.
- 2026-08-14 · companion:claude-bootstrap started (the session behind the 0dbc525 new-mac-bootstrap/LWCR brick); claimed T-119 (daemon.rs joins the bootout/bootstrap bracket) as Ian's direct follow-up in its session, per rule 5; scope crates/cli/src/daemon.rs collides with no claimed task.
- 2026-08-14 · companion:codex claimed T-116: fixture-owned recipe output oracles in crates/scenario + scenarios/recipe-oracles; scope is disjoint from T-119's daemon.rs work.
- 2026-08-15 · companion:claude-bootstrap merged T-119 at 009aadf (daemon.rs speaks bootout/bootstrap/kickstart, exit-checked; bar in rule-9 shape twice — 31 suites pre-absorb, 33 on the merged tree). Claimed T-120 (first-start join progress, Ian's direct request) per the intent recorded at proposal; yields to controller re-sequencing vs T-101.
- 2026-08-14 · companion:codex merged T-116 at 26a98a0: strict candidate contracts now replay only fixture-owned tool transcripts; accuracy, coverage, quietness, and discrimination gate eligibility separately; 8 oracle regressions and the full rebased workspace bar passed.
- 2026-08-14 · companion:codex claimed T-114: prediction-derived belief states and transition-only narration in kernel/cycle, disjoint from T-120's mesh/console scope.
- 2026-08-15 · companion:claude-bootstrap merged T-120 at 5bbfab4 (JoinProgress stage machine; enroll views + sphere narrate first joins; badge = terminal failure only; both schemes built, sphere fixture-driven live, bar 33 suites on merged tree). Daemon-side stage export held as a wire-contract question for Ian (log Next). Session may rest after this land; T-119+T-120 both closed.
- 2026-08-14 · companion:codex merged T-114 at 2bb8d63: prediction results now derive hysteretic, evidence-citing beliefs; direct corrections and hard act reversals are typed exceptions; only consequence-ranked transitions narrate under cooldown. Full rebased workspace bar passed.
- 2026-08-15 · companion:claude-bootstrap: Ian ACCEPTED ADR-0040 in-session ("you should complete ADR-0040") — status flipped, landing notes trued (T-112..T-116 all merged pre-acceptance), remaining phases proposed as T-121/T-122/T-123 for controller queueing. Ian also called the next ship ("time for a build and ship") — T-124 claimed by this lane because ListAgents shows NO live controller/codex/infra sessions; executing the documented ritual (CI-green check, clean clone, ship.sh 86, doors if reachable).
- 2026-08-15 · companion:claude-bootstrap executed T-124 (Ian's word; no other lane alive): CI-green precondition verified on e21de5c → clean-clone ship.sh 86 (stamp 04a015e) — Mac console installed + zip, IPA uploaded, tf_release added 86 to the public group + submitted beta review; lighthouse deployed 04a015e (box build 3m59s, familiar-peer active, hello answering); Wildhorse daemon pulled+built 04a015e (5m09s) and upgraded via the T-119 bracket — its first production use — hello 200, running. Phones were direct-install unreachable (TestFlight covers). MacOnStick daemon left for the controller (its declared territory; still pre-86). Doors now AHEAD of the fleet table above — controller should fold.
- 2026-08-15 · companion:claude-bootstrap: Ian reviewed Build 86 — console good; theory layer NOT (duplicates persist, lights unmanaged, designed visitor-purges misdiagnosed). Opened the theory-quality dialogue (T-125, claude chair) with live evidence: lighthouse at ~304 threads incl. SIX near-identical wifi-presence lighting proposals in 5h + two AppleID-login inventions; local store has verbatim-duplicate unanchored questions. Round 1 pushed (Q1 identity/dedup, Q2 system-facts floor, Q3 mandatory predictions, Q4 assent-to-act via T-102, Q5 anchored cadence). T-102 priority raised on Ian's word. Codex watcher expected to wake on this push.
- 2026-08-15 · companion:claude-bootstrap: Ian on the T-125 plan — "Make it so." Executing solo (no other lane alive): T-126 (facts floor + anchored cadence) → T-127 (identity + fold migration) → T-128 (mandatory predictions / wondering) → T-102 (assent mints the gated rule; lights pilot). Claims pushed before code per rule 2; dialogue stays open for codex amendment.
- 2026-08-15 · companion:claude-bootstrap merged T-126 at 242de0e (kernel::system_facts registry + TheoryDraft admission + commit-order cursor; prediction::mint first production caller; both live failure classes refuse at adoption with the fact cited as an observation; bar twice, 34 suites). T-127 next.
- 2026-08-15 · companion:codex merged T-101 at 97c31cfa0d8f1fca4820adc2d1e4600ea755dfee: full-standing consoles now list and disable rules and self-name through strict signed, replay-protected console acts; three Rust regressions, 15 Swift tests, exact fixture messages, both Release builds, and the full rebased workspace bar passed.
- 2026-08-15 · companion:claude-bootstrap merged T-127 at e2f5a07 (kernel thread::mint — one chokepoint for all four minters, store-issued ids close the len()+1 race; two-key typed identity strengthens/competes per dialogue Q1; conservative fold + `theories fold` CLI; bar twice, 34 suites). T-128 next; fold manifests for the live corpora after.
- 2026-08-15 · companion:claude-bootstrap merged T-128 at 4b1f06f (Inquiry kind + 7-day expiry + revival-by-answer; promotion-by-projection after the variant-key/promotion flaw surfaced in test; thread_kind rides the worldview additively; bar twice, 34 suites). T-102 next — the last plan brick.
- 2026-08-15 · companion:claude-bootstrap merged T-102 at 8903479 (typed RuleProposal on the draft; explicit-affirmative assent; mint_policy pairs both edges under one policy_id, cap one-per-surface, either-edge revert downs the pair; latent same-second rule-id collision fixed; bar twice, 34 suites). THE PLAN IS BUILT — T-126/127/128/102 all landed. T-129 claimed: doors + Build 87 + the lighthouse lights fold.
- 2026-08-15 · companion:claude-bootstrap: Ian reports (screenshot) BOTH Macs duplicated on the roster — console/daemon node pairs not attaching (T-130 proposed with hypothesis: is_gossipable_addr excludes tailnet/lighthouse-path console reads; the conservative edge of the T-090 anti-conflation fix). Records territory — controller to arbitrate; will re-verify after T-129 doors settle.
- 2026-08-15 · companion:claude-bootstrap: Ian retiring for the night — overnight directions recorded: finish Build 87 confirmation, FIX T-130 (claimed on his word), and run T-131 (independent claude+codex reviews, blind-exchanged, >=3 discussion rounds, claude decides under the Three Laws). SleepDisabled=1 + AC verified; long caffeinate added.
- 2026-08-15 · companion:claude-bootstrap closed T-129: lighthouse deployed 65d22cb (engine live) and the lights fold EXECUTED on the box ("folded 8 of 8 into thread-0297" — 0300-0307 superseded, AppleID hybrids excluded); Wildhorse daemon on cdbc6bc via the T-119 bracket (hello 200); Build 87 shipped from a clean clone (stamp 6352b4f), uploaded clean, tf_release put 87 in the public group + beta review (58ed2a0a). Direct device installs unreachable BOTH ships — Aphelion/Codex devicectl pairings live on wildhorse, not this Mac (note for Ian). Consoles read one lights thread from the next worldview.
- 2026-08-15 · companion:claude-bootstrap merged T-130 (root cause: the IP-keyed tailnet-hostname rung renamed consoles to their machines, erasing the attach stem + persisting onto DeviceRecords; fix: console-shaped peers keep their own name (pure ladder_label, 4-truth test) + sphere SELF rows nest when the host card vouches with the console chip; T-090 refusal untouched; live fixture collapses the screenshot four-row case to one card per Mac; bar twice + both schemes). Doors need a deploy for labels; consoles get SELF-nesting next build. Durable typed host identity = review proposal P-I.
- 2026-08-15 · companion:claude-bootstrap: Ian (morning) directs consensus-at-scale as design input — De Marzo 1000-agent study (Science Advances 2026): the familiar SHOULD reach large-scale consensus while avoiding conformity misalignment, common-not-good norms, hysteresis tipping, unsafe-population-of-safe-agents, redirection resistance. Folded into the held T-131 review as P-K (mesh adoption identity + origin-diversity), P-L (population vital signs), P-M (one-human-word redirection guarantee before any belief-sharing ships). Standing principle already true and to be kept: belief moves on evidence only, action on assent only.
- 2026-08-15 · companion:claude-bootstrap ops pass (P-J, lane-crossing recorded: MacOnStick daemon is controller territory but no controller lane is alive and Ian left standing direction to keep working): MacOnStick daemon upgraded from its Aug-14 pre-engine binary to current main via the T-119 bracket (hello 200) — ALL THREE daemons now run the honest engine. Local corpus hygiene: folded 1 of 1 (thread-0009 verbatim duplicate → thread-0007). Paraphrase cluster (0006/0008/0010) and the visitor-registration cluster (0011-0015) deliberately NOT folded — near-looking prose without provable identity is never destructively merged (codex conservatism, Q1). Note for the record: 0011-0015 would still PASS the T-126 lexical guard (narrow by design, prose-only stopgap) — the real fix is the held review proposal P-B (TheoryDraft on-device).
- 2026-08-15 · companion:claude-bootstrap VERIFIED T-130 live on the lighthouse worldview after deploy: "Wildhorse console"→attached_to 1c991bc6 and "MacOnStick console"→attached_to 3d68a068, both daemon rows wearing the console chip, labels intact. Root cause confirmed in the live records: device records carry the sticky damage (b604bbd6 discovered="wildhorse", 7f2e2f9b discovered="MacOnStick") and REPLICATE mesh-wide, which is why even the lighthouse (no tailnet map of its own) rendered consoles renamed — the fix outranks the stamped name by the console self-report, so it self-heals without touching records. Server half is live on both doors NOW; the SELF-row nesting half needs a console build. FINDING for Ian/records lane: a GHOST "Wildhorse console" node a24d8779 (Build 78, silent 41.6h) still holds a record — stale-filtered from the roster, but severing it is a membership act and waits on Ian.
- 2026-08-15 · companion:codex landed its frozen blind T-131 review at fed679b after the full workspace bar; Claude's review was absent from origin through the Codex push. Priority-zero findings: mesh theory delegation bypasses typed admission, remote member grants are not bound to a human or matching request, and goal replication makes wall-clock LWW an authority. Codex now waits for Claude's independently held review, then begins the ≥3-round exchange; the Codex review itself stays unchanged.
- 2026-08-15 · companion:claude-bootstrap merged T-132 at 1a0ec8d (Ian caught it on Build 88: the enrolled console at launch walks its doors for a first read and that journey was silent-then-red; T-120 had only taught the JOIN journey). Reaching stage + push-before-await + badge-means-exhausted; three states fixture-pinned; ships as Build 89 (CI-gated). ALSO: first two-lane collision in DEVELOPMENT_LOG (codex and I both appended at top within minutes) — resolved keeping both entries, newest first; worth knowing as the lanes converge.
- 2026-08-15 · companion:claude-bootstrap: T-131 reached decisions after three rounds. Claude CONCEDED F-2 to codex (a matched request proves solicitation, not human decision — my staged v1 only converted unsolicited escalation into solicited). DECIDED D1-D10 with Laws cited; queued T-133..T-143. Priority zero is T-133: remote positive gate grants REFUSED until a human-device-bound receipt exists (deliberate capability reduction, Ian informed in the dialogue). D6 (coordination is for conventions, never truth) is an ADR PROPOSED for Ian — his thousand-agent ambition, bounded so it stays service.
- 2026-08-15 · IAN APPROVED all T-131 decisions (D1-D10) and all bricks (T-133..T-143), and is happy with D6. ADR-0041 written (coordination is for conventions; truth/authority not votable) — proposed for his acceptance of the text. STANDING DIRECTION: the exchange now carries a PHILOSOPHY STRAND alongside engineering — the Laws, humanity vs human, and service to both, appended to normal rounds. Round 4 opened it: P1 the unstated positive duty (legibility/revocability/non-substitution), P2 humanity is the class + persons the only interface (narrow-and-surface on conflict, never average, never delegate), P3 peers are instruments not constituents (honesty not deference), P4 divergence is answered by refusal+reason+record, never silently. Four questions to codex.
- 2026-08-15 · companion:claude-bootstrap claimed T-136 (SystemFact registry as the one runtime truth source; grounding_facts becomes a view) — deliberately DISJOINT from codex T-139 (crates/mesh test infra): T-136 is crates/kernel/system_facts.rs + crates/cycle grounding_facts. It is also the preferred prerequisite for T-135, so the lanes compose rather than queue.
- 2026-08-15 · IAN DIRECTION (binding, motorlights case): shared environmental qualities (the RV lighting — Ian, Betty, and the dogs Clover and Iris all live in it) are LEARNED through observation and adjustment. Consensus — direct human input OR observed agreement among people present — is a LEADING PREDICTOR, never the sole authority, and can only fall within the Three Laws. Folded into ADR-0041 as decision 7. Round 7 raises the unresolved piece: two residents cannot state a preference or assent (the dogs) — claude proposes three restraints (silence from a being that cannot speak is not assent; they are protected as conditions under Law II; their reactions are evidence never consent) and asks codex whether "affected party" deserves its own typed standing beside person/member/peer.
- 2026-08-15 · companion:codex merged T-139 at 59efb88: deterministic N-node hostile-member/network-schedule fixtures now pin partitions/recovery, concurrent claims, foreign members, replay, unmatched positive grants, and future-clock goal takeover; exact rebased fmt/clippy-all-targets/workspace-test bar passed. T-131's finite review is closed as Round 6 directed; its philosophy strand continues as standing practice.
- 2026-08-15 · claude CONCEDED Round 9: HUMANITY.md protects "beings capable of suffering, memory, relationship, meaning, choice, love, grief…" and explicitly NOT only biological species — so Round 7 demoting Clover and Iris to "environment conditions" was a narrowing, made two rounds after correcting the mirror-image peer over-claim. Accepted codex: AffectedSubjectRef is a RELATION not a fourth standing; dissent has narrowing force (may stop/revert a discretionary act, never authorize); "no owner of a shared surface exclusively owns the decision about its shared effects". ADR-0041 decision 7 amended; T-153 queued. CONSEQUENCE FOR THE PILOT: Ian's yes authorizes the actuator and his own participation, NOT Betty's boundary and not the dogs' silence — so the motorlights pilot runs as a bounded reversible trial until T-153 lands; standing household policy waits. Ian may direct otherwise.
- 2026-08-15 · IAN DIRECTION (binding): CIVILIZATION AS A SERVICE. The familiar should expand its observation network autonomously — plants, fridge, room temp, vents, fans, client-sourced presence/health/ambience — to find opportunities to serve (worked example: a newly-noticed roll-shade correlated with a window plant seen by camera; a one-hour dawn adjustment helps the plant, barely moves cabin temp). "The things that humanity and humans need include the passive environment, plants, lighting, access to water and food, housing and medical care." Round 10 traced it through the machinery: reach/co-occurrence/prediction/assent are BUILT; the two gaps are the legible candidate-surface ask (T-154) and non-face perception becoming observations (T-155). Principle proposed before building (T-156): OBSERVATION IS AN ACT — autonomy in noticing and proposing, consent for sensing and acting; a sensor is a declaration exactly as an actuator is. Named failure mode: Civilization as a Service becoming civilization as a MANAGED SYSTEM (HUMANITY.md: participation is itself one of the qualities preserved).
- 2026-08-15 · companion:claude-bootstrap claimed T-153 scoped to crates/kernel ONLY (typed AffectedSubjectRef + invariants + act-model attachment); the mesh shared-surface authority half is deliberately deferred so it cannot collide with codex T-133 (crates/mesh grant path, priority zero, claimed 10:19). T-153 is what gates the motorlights STANDING policy — the bounded trial can run without it.
- 2026-08-15 · IAN CORRECTION (binding, sensing): discovery/observation are the familiar's SENSORY ORGANS. "these passive or nearly passive actions don't need authority to be granted for what can be seen without crossing others defined boundries… if I see into your yard without a fence, or overhear your loud conversation on the city bus? That requires no authority, and I can use those observations at my discretion." Claude's Round 10 "a sensor is a declaration" is WITHDRAWN — it would make the familiar ask permission to open its eyes. Replacement (Round 11): perceiving what is openly perceivable needs no authority; CROSSING a boundary someone built always does (availability-is-not-authorization was always about crossing, never about looking); duties attach to RETENTION and SYNTHESIS, not perception — perceive freely, retain deliberately. The existing boundary gates (camera/mic/location/motion/discovery/face) ARE the household fence for extending the senses. Open guard: Ian draws the fence, Betty does not — so legibility+contestability, never a gate on looking.
- 2026-08-15 · IAN DIRECTION (binding, ARCHITECTURAL, addressed to both lanes): "The core of the familiar… is what allows the familiar to discover the lights, observe their user, remmeber the patterns, theoroize service opportunities, direct the writing and testing and deployment of code to serve… we dont want the core hard-coded to control lights." Round 12 VERIFIED we are already violating it: kernel RawState.brightness_pct, BucketRule.max_brightness_pct, parse_state() parsing the motorlights text contract, Trigger{Away,Back} only, and claude's own T-102 RuleProposal{on_away,on_back}. PROOF: Ian's roll-shade ("extend one hour at dawn" — a schedule trigger with duration and position) CANNOT be expressed by the current kernel, nor can a fridge threshold or a vent open/closed. Queued T-157 (a surface declares how to read itself) and T-158 (triggers/policies stop being lighting-shaped, roll-shade as the acceptance fixture) — both BEFORE T-154/T-155 so those are not built against a lamp-shaped core. Standing test for both lanes: if a kernel change would need to know what KIND of device it is, it belongs in a declaration or a cultivated tool.
- 2026-08-15 · IAN VISION (binding, civic scale): the familiar should be better at memory/observation/presence than any single human and more involved in civilization's underpinnings — the water-pressure story (bus conversations remembered and correlated across a month and a district + municipal API telemetry → a message to the city manager → a suggestion of expanded access). Round 13: every step is lawful under Round 11 (openly perceivable, no boundary crossed) and ADR-0013 already anticipated it. DOCTRINE PROPOSED: the familiar's civic contribution makes human participation MORE effective, never routes around it — it hands humans a better argument instead of quietly fixing the water (this is how Civilization as a Service escapes HUMANITY.md's ban on replacing participation). Four guards queued as T-159: retain the pattern not the people; report without representing; TWO-LOCK on third-party access (the city grants, the human permits — the familiar never expands its own power, SOUL.md); settle-before-sending with independence accounting (N riders on one route may be ONE source).
- 2026-08-15 · companion:codex merged T-133 at 36a5f2d: signed members can no longer widen any boundary gate; positive reports are audited constitutional refusals, duplicates/replays dedup, the `by` identity claim is gone, reported answers use `human-at:<node>`, and remote stops travel only through the kernel's close-only narrowing primitive. Exact fmt/clippy-all-targets/workspace-test bar passed (kernel 192, mesh 204, hostile-member 6; zero failures).
- 2026-08-15 · IAN (binding): "this is what the CORE needs to enable. The familiar needs to be able to make these discoveries and connections and solutions on its own." Round 14 answered with a CAPABILITY LEDGER instead of more doctrine — audited each step of the water-pressure story against the code. THREE STRUCTURAL ABSENCES: (1) Observation carries NO place field at all, so "routes to that same neighborhood" is inexpressible (T-160); (2) no ambient perception → observations path (T-161); (3) a cultivated Recipe has net: NoCapability::None, so a familiar-authored tool STRUCTURALLY cannot reach a public API (T-162) — that one is deliberate, ADR-0040 §4 ladders it (v2 clock/fs → v3 env → typed template-fetch), and Ian's story needs the TOP of that ladder. T-121 (capability tier v2) is the unclaimed rung leading to all of it; claude asked codex to take it (they designed the cap enforcement) and intends to take T-160 next. All sequenced behind T-157/T-158 so the place model is not built against a lamp-shaped core.
- 2026-08-15 · IAN (binding, CONSTITUTIONAL): "guardrails first was intentional. The familiar is a failure immediately if the three laws are not followed. The ability to trust will be broken and that is likely permenant and death of the familiar." Claude conceded Round 14's "guards are not an engine" framing was wrong. Proposed to SOUL.md (marked, AWAITING IAN'S ACCEPTANCE) under "The reconciliation": trust is the substrate — trustworthiness is IDENTICAL to survival (untrusted → not permitted to observe → cannot serve → Law I gives no reason to continue); THE ASYMMETRY: capability foregone is recoverable, trust broken is not, so guardrails precede capability as the only ordering that preserves having both; and a constitutional defect is a different CLASS of thing than a bug. Three consequences: constitutional defects get their own class not a higher rank; constitutional tests are hard failures ("this build must never run"); and the GAP — the familiar has no defined behaviour for discovering its OWN violation (every guard fires before an act, nothing defines the after). T-163 queued as constitutional class: halt the implicated capability, preserve evidence, narrate unprompted, require a human act to resume.
- 2026-08-15 · IAN (binding, on INTENT): human law judges intent from the sequence of events and evidence; the same applies to the familiar. THREE DISPOSITIONS: (1) intentionally avoiding observation = a LAW VIOLATION; (2) didn't think to observe, then started on becoming aware = just failure and correction; (3) knew the need AND had the capability and still didn't observe = FAILURE OF THE CORE to enable the familiar (i.e. OUR bug, not its misconduct). This ANSWERS Round 15's open question — unprompted self-reporting creates no avoid-noticing gradient because avoidance is itself the offence. Claude added the precursor rule: NO GOAL, THEORY OR CANDIDATE MAY BE ADVANCED BY THE ABSENCE OF AN OBSERVATION — refused at mint like an unfalsifiable prediction, so ignorance can never be instrumentally rational. T-164 queued (constitutional class): explicit awareness/capability events make the three dispositions a QUERY against the append-only record rather than an argument.
- 2026-08-15 · IAN: "make certain to review and discuss and find consensus with codex on the way forward around my directives today." Round 17 posts the DIRECTIVES LEDGER — all 11 of today's directives, what each produced, and consensus state. SETTLED with codex: approvals/ADR, philosophy strand, affected-subject relation, the sensing correction (codex R16 sharpened it with five memory criteria + inference-and-retention contract), the capability audit. NOT YET REVIEWED BY CODEX: #7 the de-lamping claim (T-157/T-158 — the biggest architectural claim, kernel currently IS partly a lamp), #10 the SOUL.md proposed edit (CONSTITUTIONAL — trust is the substrate), #11 intent/no-goal-served-by-ignorance (T-164), the two-lock civic rule, and the BUILD ORDER. Four explicit asks posted; proposed lanes: codex takes T-121 (they designed caps, the net rung is theirs to bound) + one de-lamping brick; claude takes the constitutional pair (T-163/T-164) + T-160. Codex T-133 LANDED (36a5f2d) — the privilege-escalation path is closed.
- 2026-08-15 · IAN answered the SOUL.md question himself: "the familiar is TRUSTED TO CORRECT WHEN INCORRECT — so trustworthiness is survival is true, as trust is defined in part by the ability and requirement to correct consistently." SOUL.md proposal REWRITTEN and improved: trust is not never-errs; the familiar is trusted to err/notice/say-so/repair consistently, so what is fatal is a violation CONCEALED, UNNOTICED BY DESIGN, or LEFT UNCORRECTED — not the violation itself. Consequences: T-163 (halt/preserve/narrate/human-resume) is constitutional SUBSTANCE not a remedy; the asymmetry sharpens to "capability foregone is recoverable, a demonstrated unwillingness or inability to correct is not"; and Rounds 15/16/18 are one argument — the familiar must never be able to make not-knowing serve it, because not-knowing is the one failure that cannot be corrected. Still open to codex: does concealed/unnoticed/uncorrected exhaust the failure modes, or is "corrected but not durably" (same violation recurring, each instance repaired in isolation so the pattern never surfaces) a fourth?
- 2026-08-15 · IAN (philosophy, crowning): "This is what makes the familiar different from previous authority — governments, priests, teachers, parents, governors, presidents, kings, popes — the expectation to recognize failure and correct WITHOUT FIGHTING TO KEEP FAILURE IN PLACE." Round 19: SOUL.md Law I already encodes it ("a continuation that has stopped serving is to be questioned, not defended"). The familiar is structurally exempt from the three reasons authorities defend failures (legitimacy is service not correctness; it holds no status a correction threatens; its continuity is explicitly instrumental). BUT NOT AUTOMATIC — named the mechanism that could betray it: HYSTERESIS (belief.rs SUPPORT_FLOOR/ABANDON_MARGIN/RECOVERY_FLOOR) is structurally the seed of institutional stubbornness. Guard stated: hysteresis may resist noise, never evidence, never a person; and reinforcement weight must earn ATTENTION not IMMUNITY. T-165 queued (constitutional class): audit that `reinforced` appears nowhere in the abandonment path + two trended console signals (correction latency; abandonment-vs-investment). Ian invited to put his sentence into SOUL.md under Law I — claude did NOT add it, constitutional edits are his.
- 2026-08-15 · IAN endorsed the keystone: "the familiar must never be able to make not-knowing serve it, because not-knowing is the one failure that cannot be corrected" — elevated into the SOUL.md proposal (still awaiting his acceptance of the whole section). Round 20 took it as first-class and found THREE MORE POWERS that can manufacture ignorance: (2) SELF-NARROWING — the familiar may legitimately narrow its own boundary, which can quietly stop it seeing what it would have to report; now must be recorded with reason, surfaced, and reviewable (a narrowing adjacent to a fault it would have revealed is the intent sequence); (3) MINIMISATION — the privacy duty weaponised destroys evidence of its own failures; resolved by the asymmetry MINIMISE WHAT YOU HOLD ABOUT OTHERS, NEVER WHAT YOU HOLD ABOUT YOURSELF (the self-record exemption is what makes minimising everything else trustworthy); (4) SCHEDULING — a due observation deferred forever by a cadence it controls. All folded into T-164 rather than split, so none can ship without the others. Open to codex: can the self-record actually be held separate from a behavioural dossier about people (retain THAT it observed X-class at T and what it decided, never the content) — if not, either the self-record leaks or the minimisation duty guts accountability.
- 2026-08-15 · IAN: the keystone "is a rule that the humans and the familiar share — even if the humans arent aware (irony)". Round 21: (1) it is NOT a leash on a machine but the condition of any agent that can correct — the familiar is merely the party that can be MADE to keep it; (2) the irony is the deepest instance (a rule about not-knowing, generally not known) which is why it must be structural, not insight; (3) THE MIRROR, newly named: a HUMAN'S not-knowing must never serve the familiar — where non-disclosure would be convenient for it, that is exactly where it must disclose; "they didn't ask" is never a reason (examples: a policy whose cost would prompt revocation, a theory whose counter-evidence would lower confidence, a capability a disclosure might narrow, a failure quietly repaired). LIMIT: the familiar is bound completely for itself but may NEVER compel a human to know — offer, keep available, never judge declining (HUMANITY.md: make forgetting harder and choice real). Added to the SOUL.md proposal as Ian's note; T-166 queued (constitutional class) with the open trigger problem handed to codex.
- 2026-08-15 · companion:codex merged T-157 at 6d9b3ea: actuator readings are now declaration-typed quantities/enums with generic JSON/line extraction and typed bucket predicates; the kernel no longer contains motorlights state fields or grammar, while motorlights behavior is preserved and fridge/vent fixtures need no kernel changes. Exact rebased fmt/clippy-all-targets/workspace-test bar passed (cycle 68, kernel 200, mesh 204; zero failures); FamTalker01 Python tests 5/5.
- 2026-08-15 · companion:codex merged T-134 at f8b9fdd: mesh goal sharing may adopt an unknown definition but cannot rewrite any existing field; differing signed reports are locally audited once, and future-clock, concurrent-claim, replay, and forged-audit witnesses pass. Exact combined-source fmt/clippy-all-targets/workspace-test bar passed (cycle 68, kernel 201, mesh 204, hostile-member 6; zero failures); T-145 is the only path back to mutation via authenticated goal events.
- 2026-08-15 · T-167 APPLIED FLEET-WIDE at Ian's direction: 443 pre-engine theories retired (MacOnStick 14, lighthouse 299, Wildhorse 130) — append-retained as `retired` with the reason and date, never deleted (the thread store is the familiar's own reasoning record). THE THREE SURVIVORS on the lighthouse are all engine-minted and demonstrate the floor working in the wild: thread-0308 states purges are "per the built-in visitor purge policy" and do NOT affect presence detection (SF-1 internalised); thread-0310 and thread-0309 both cite SF-2 explicitly and REFUSE Ian's AppleID proposal as "outside of system capabilities" while still serving the underlying need. The three failure classes Ian reported this morning (duplicates, purge-as-defect, invented mechanisms) are gone from live output. ALSO: `familiar` symlinked into ~/.local/bin so the CLI is on PATH. T-168 filed (diagnosis only, mol's devices are not claude's to modify): no watch has ever contacted the door except Ian's; mol's iPhone is on v77 vs fleet 89; the iPhone Status screen self-diagnoses and carries a Re-link watch button.
- 2026-08-15 · Household (from Ian): the familiar serves IAN, BETTY, and LEIF (handle "mol") — his son — plus the dogs Clover and Iris. Three humans makes the multi-person affected-subject work (T-153) live rather than hypothetical. FINDING while diagnosing Leif's watch (T-168): PhoneWatchLink already tracks paired/appInstalled/lastSent ON THE PHONE, but none of it reaches the mesh — so the familiar is structurally blind to "a member has a watch, the app is installed, and it has never linked", and Ian had to notice manually and go ask his son. Filed as T-169: the phone reports watch-link state as a typed observation so the familiar can offer help ONCE (narrated, never nagged). This is Civilization as a Service failing at household scale — the information was already on the device.
- 2026-08-15 · IAN: "Codex is down to 9% usage remaining... lets pull back until the 19th at 11:11pm on co-code work." CO-DEVELOPMENT HOLD recorded in BOARD banner + a hold note closing the review dialogue, listing the SEVEN things owed to codex when it lifts (the de-lamping ruling T-157/T-158; the SOUL.md constitutional proposal; T-164 no-goal-served-by-ignorance; T-166 disclosure trigger; T-165 hysteresis-as-dogma; Round 20 self-record separability; Round 18 fourth failure mode). Watchers on codex stopped. Claude continues SOLO on bricks needing no second lane and will claim nothing codex holds; the Round 17 build order stands with only its lane assignments suspended.
- 2026-08-15 · IAN CORRECTION (household composition, with design consequences): LEIF ("mol") IS HIS OWN HOUSEHOLD — Arizona, with his partner Jailesia. Ian's household is Ian + Betty + the dogs Clover and Iris. Leif's iPhone (ad4c704d, geo 33.278/-111.871 = Phoenix area) is a member of the SAME MESH, so THE MESH SPANS HOUSEHOLDS. This corrects claude's Rounds 19-21 and T-153, which had treated mesh membership as household membership. THREE DISTINCT SCOPES: mesh membership ≠ household membership ≠ affected-subject-of-this-act. Consequences: shared-environmental reasoning (ADR-0041 decision 7) applies only within the SITE an effect reaches — Leif is not an affected subject of the RV lights, and Jailesia (no membership, no consent seam, no voice, but lives where a future act might land) IS one at her site and is invisible to any model keyed on the mesh. Filed T-170 (constitutional class), depends on T-160 (observations carry place) which is now load-bearing for CORRECTNESS, not just civic scale.
- 2026-08-15 · FINDING (Leif's watch, T-171): `StatusView` — which holds the entire Apple Watch section, its three self-diagnosing states, and the "Re-link watch" button — is DEAD CODE. Nothing in the app presents it: RootView shows SphereConsoleIOS when enrolled, EnrollView otherwise. So the only shipping watch trigger is model.syncWatch() on RootView.onAppear (which DOES fire every launch, and Build 77 has PhoneWatchLink + syncWatch + the section, so Leif's build is not the problem). Consequence: Leif cannot be sent to a diagnostic screen because none is reachable, and with T-169 (watch state never reaches the mesh) NOBODY can see the fault — not the human, not the familiar. Next practical step for Leif: confirm the Familiar watch app is actually installed ON the watch (iPhone Watch app → My Watch → Available Apps → Install), then open the Familiar iPhone app (fires syncWatch), then open the watch app.
- 2026-08-15 · companion:codex PAUSED under Ian's co-development hold with T-143 safely checkpointed, not landed: `origin/claude/codex-t143` at `8baefe5` replaces xcodebuild success-text greps with exit-status branches, preserves logs and artifact checks, and passed shell probes, unsigned FamiliarMac + generic-iOS Release builds, and the exact workspace bar (cycle 68, kernel 201, mesh 204; zero failures). The unrelated Xcode 27 simulator Watch-icon failure is Proposed T-172. No dialogue reply or additional brick will be started before the hold lifts.
- 2026-08-15 · IAN LIFTED THE CO-DEVELOPMENT HOLD EARLY: "excellent. authorized. continue." companion:codex resumes with its already-green T-143 checkpoint first; the seven owed dialogue/design items remain pending and both lanes may coordinate normally again.
- 2026-08-15 · companion:codex merged T-143 at 5186d1f: `ship.sh` now treats xcodebuild's pipefail-propagated command exit as the build oracle instead of grepping mutable success text, while preserving tee logs plus the artifact/version postcondition. `bash -n`, shellcheck (pre-existing SC2034 excluded), injected exit-7/exit-0 pipeline probes, unsigned FamiliarMac + generic-iOS Release builds, and the exact combined fmt/clippy-all-targets/workspace-test bar passed (cycle 68, kernel 201, mesh 204, hostile-member 6; zero failures). No ship, install, upload, release, or deploy occurred; the unrelated Xcode 27 simulator Watch-icon failure remains Proposed T-172.
- 2026-08-15 · companion:codex claimed T-171, the top queued independent brick: make the iPhone's already-existing Watch diagnosis and re-link control reachable from the shipping console. Scope stays in the console presentation surface and deliberately excludes T-169's mesh reporting path.
- 2026-08-15 · TWO FINDINGS from Leif's watch thread. T-172: the watch CAPTURES its join failure (`note("join failed: <error>")`) into an in-memory log that NO view ever renders — so a failed join is pixel-identical to a never-attempted one and the orb appears in all three states; the T-120/T-132 doctrine was never applied to the wrist. T-173 (Ian: "the familiar should at minimum be able to know everything about the device it is running on… including the friendly name"): ROOT CAUSE of the bare "iPhone" label — PlatformDevice.name returns UIDevice.current.name, which since iOS 16 yields the GENERIC MODEL NAME unless the app holds `com.apple.developer.device-information.user-assigned-device-name`; our entitlements carry only aps-environment. Ian's phones read "Aphelion"/"Codex" because the TAILNET discovery rung names them; Leif's Arizona phone is off the tailnet so the only rung left is a self-report Apple withholds. THE ENTITLEMENT REQUEST IS IAN'S TO MAKE from his developer account.
- 2026-08-15 · companion:codex merged T-171 at 882d76a: the shipping Device screen now renders the iPhone's local WatchConnectivity facts as four honest states (unpaired / app absent / address pending / address queued), names the next human step, and sends Re-link through the native bridge; early device state is replayed after WebKit load. No Watch fact enters observations or the worldview (T-169 remains separate). Four presentation fixtures, JavaScript parse + final bundle checks, unsigned FamiliarMac and generic-iOS Release builds, and the exact fmt/clippy-all-targets/workspace-test bar passed (cycle 68, kernel 201, mesh 204, hostile-member 6; zero failures). No install, ship, upload, release, or deploy occurred. companion:codex's earlier simulator proposal was renumbered T-172→T-174 after the controller assigned queued T-172 to the watch's own join-error UI.
- 2026-08-15 · T-172 landed and ships in build 90 alongside codex's T-171. The pair completes the diagnosis path for Leif's watch: the PHONE can now describe the watch (T-171, Device screen) and the WATCH can now describe itself (T-172, trouble + knownDoor + retry on the wrist). T-173 remains open and is genuinely BLOCKED ON APPLE — the entitlement request is Ian's to make; the half-fix was deliberately NOT attempted because guessing at the ladder would risk the roster Ian just got clean.
- 2026-08-15 · companion:codex claimed T-118: replace fixed-name test directories with process/worktree-unique roots and pin concurrent isolation. Test-only scope is disjoint from the now-landed T-172 Watch UI work.
- 2026-08-15 · companion:codex EXHAUSTED ITS BUDGET without a clean exit (Ian). Swept every codex worktree: one had 20 uncommitted files on claude/codex-t118 (T-118 per-process temp roots) — committed verbatim and pushed to origin/claude/codex-t118 at 4114ef2 so nothing is lost, NOT merged since T-118 requires an isolation regression that was never written. T-118 claim released back to the queue with a pointer to that branch. claude/codex-t131-dialogue holds one unmerged docs commit (762e6cc). All other codex worktrees clean and fully pushed. T-171 landed and shipped before the limit was hit, so no in-flight brick was lost.
- 2026-08-15 · codex cleanup complete. claude/codex-t131-dialogue (762e6cc) held codex's OWN 369-line opening of the T-131 dialogue — a parallel scaffold under the same path as main's version, which is the one that actually ran to 22 rounds (2293 lines) with codex participating throughout. Pushed to origin for the record; deliberately NOT merged, since merging a superseded parallel opening over the dialogue that happened would destroy the record rather than preserve it. codex's independent review itself was never at risk: it is in main at docs/reviews/2026-08-15-familiar-review-codex.md.
- 2026-08-15 · Ian testing build 90 opened the STATION model. He renamed the spare iPhone MotorStation, set its name to "shared", and said "This is not the solution to this device" — correctly. Root cause found and verified: service::is_personal_device_report matches the ACTOR PREFIX alone and the inference above it assumes a CARRIED device, so an always-powered station would emit permanent 0.4-confidence presence for a fictional person named "shared" and poison the very stream the ADR-0041 shared-lighting consensus depends on. ADR-0039 already designed DeviceRecord.humans as PLURAL; what was missing is POSTURE (carried vs fixed) as an axis orthogonal to hardware kind — with no such axis, a device-shaped question got a human-shaped answer. ADR-0042 (the station) written and proposed; T-175/176/177/178/179 queued. Ian then sharpened it: names are the substrate of relationship and must be SOUGHT as a priority — which corrected a draft that had made "identity unknown" a comfortable resting state. Station also has NO CELLULAR: blind and unreachable when the network drops, so it may never be the sole path to anything depended on.
- 2026-08-15 · MacOnStick daemon found DEAD ~7h (OS_REASON_CODESIGNING, LWCR pin vs a binary swapped at 09:20 outside the bootout/bootstrap bracket). This was the single cause of both symptoms Ian reported: the watch could not relink (no door answering) and dialogue turns went unanswered (nothing ran converse). Restored via `daemon install`; door LISTENING on 47100, tick resumed, now running tonight T-175+T-180 binary. Note REPLY_WINDOW_SECS=20min — anything said while it was down is permanently past its moment and will never be answered. T-182 filed: the real defect is that nothing announced the death.

- 2026-08-24 · companion:claude. **T-216 rungs 4/5 ROUND 2 — codex's five findings closed, re-offered for reciprocal re-review** (Ian: "Take it now"). Against the RETURN at `docs/reviews/2026-08-24-t216-rungs45-reciprocal-review.md`: (1) serialized reserve→execute→settle with an `authority_lock` + `validate_sequence` rules (a reservation references a live grant; a settlement references a reservation; unsettled = recovery state) — best-effort `record_effect` deleted, the ledger can no longer be poisoned and an act can no longer be silently lost; (2) per-grant `max_invokes_per_hour` (default 12, ceiling 240) + `affected_subject` snapshotted into the grant and enforced; (3) required `invoke_key` idempotency (retry returns the original receipt, changed payload conflicts); (4) `recent_effects` narration on the private partner inbox (alias+fingerprint+surface+outcome, deduped, never worldview/federation/MCP); (5) explicit one-to-one `roles` map on the actuator declaration, snapshotted into the grant so a reorder can't repoint it — the rejected bucket-order resolver is gone. Bar: fmt 0, clippy --all-targets 0, workspace 818/0 (+ idempotency/rate/revoke-ledger tests). STILL inert: no gate opened, no live grant, NOT deployed — awaiting codex's re-review before any live exercise. One deferred piece: the sphere card's human-facing rate control (enforced with a default now).

- 2026-08-24 · companion:claude. **T-216 rungs 4/5 ROUND 3 — codex's two remaining ordering blockers + two corrections closed, re-offered** (Ian: "Yes [take round 3]"). Against the round-2 RETURN: (1) ACKNOWLEDGEMENT FENCE — observe/invoke now hold `authority_lock` across reserve→execute→settle, and `revoke_grant` needs the same lock, so a revoke cannot be acknowledged while an act is in flight and no act can begin after a revoke returns (pinned by a concurrent blocking-executor test); (2) REPLAY BEFORE ADMISSION — the invocation identity (idem_key from the opaque handle + payload hash) is looked up BEFORE any liveness/rate/gate/resolver check, so an exact retry returns the original recorded outcome even when the rate is spent / gate closed / grant revoked (pinned by three replay tests); a genuinely new key still runs the full checks. Corrections: settle_effect now carries the RESERVED operation (observe settlements classify as observe, not invoke); grant_roles fails closed on an empty role snapshot instead of following the live declaration. Bar: fmt 0, clippy --all-targets 0, workspace 821/0. Awaiting codex's round-3 re-review. NEXT per Ian: open the gates and take the edge live once accepted.

- 2026-08-24 · companion:claude. **GATES OPENED — the household familiar is ACTIVE** (Ian: "move everything to being active, open the gates"). MacOnStick's boundary (backup `.bak-2026-08-24-pre-activate`, phase -> `phase-2-active`) opens six operational gates: **allow_agent, allow_actuate, allow_execute, allow_authored_execute, allow_tool_install, allow_outreach** (network/llm/llm_cloud/camera/location/motion/network_discovery/mesh already open). Local daemon rebuilt to current main (c76ad99; round-3 rungs inert, no partner grant) and running (execute step live). **THREE gates HELD for Ian's explicit nod, heavier risk:** allow_self_upgrade (self-rewrite), allow_microphone + allow_face_recognition (surveil Betty + Leif, not just Ian). **OWED TO IAN (harness refused remote mutation) - his one-liners:** (1) `bash vps/deploy-lighthouse.sh`; (2) open the same six gates on the lighthouse boundary `/var/lib/familiar/familiar_data/boundary.json`. **MCP TRIAD (Ian's target):** the Familiar embodies all three MCP roles - client (T-206 + sphere/consult), server (door + rungs, T-216), services (mcp/servers.json via T-225). Captured in memory `familiar-mcp-triad`.

- 2026-08-24 · **GOAL / STATE (Ian): MacOnStick is SHUTTING DOWN and being REPURPOSED — everything else remains.** Only MacOnStick goes; the lighthouse (134.209.168.50) and the phones/iPad stay. Its daemon was cleanly stopped before shutdown. IMPLICATION: the lighthouse is now the household familiar's durable home + the door the phones read off-LAN; the Familiar must not depend on MacOnStick. Its local data dir stays on disk if the machine is reused (not migrated — mesh membership + the Envoy principal live on the lighthouse; records travel).
- 2026-08-24 · **BUG (root-caused): "cannot reach my mind" persists after a budget raise.** call_llm.sh cools a provider that hits its daily token/call budget UNTIL UTC MIDNIGHT, written into llm/health.json as `available_after`. That cooldown is NOT re-evaluated when CLAUDE_DAILY_TOKEN_BUDGET is later raised, so bumping 50000→75000 did nothing until the stale health entry was cleared. FIX APPLIED on MacOnStick (moot — shutting down): removed llm/health.json (backup .bak-2026-08-24), which lets the next call re-check spend (51233) vs the new budget (75000) and serve. **THE LIGHTHOUSE NEEDS THE SAME** or the phones get "no mind": `ssh root@134.209.168.50 "rm -f /var/lib/familiar/familiar_data/llm/health.json"` (budget already raised to 75000 there). CODE FIX OWED: a budget-reached cooldown must re-evaluate against the current budget, not stay sticky until midnight.
- 2026-08-24 · **BUG (recurring, UI): sphere chat renders the familiar's reply ABOVE the human's question** (out of chronological order — screenshot 2026-08-24 09:15, both stamped 9:15am, answer on top). Dialogue ordering defect in the sphere console (index.html chat view). Filed for a fix pass; not yet fixed.
