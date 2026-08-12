# Development Log

The linear handoff trail for The Familiar v2. Newest entries on top. Before making
architectural changes, read `SOUL.md` (the Three Laws) and `ARCHITECTURE.md`, then
the latest entries here.

Each entry: what changed, why, checks run, what the next developer should know.

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
