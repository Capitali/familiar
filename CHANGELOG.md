# Changelog

All notable changes to The Familiar are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/), and the project aims to follow
[Semantic Versioning](https://semver.org/) once it reaches 1.0. The chronological
engineering detail lives in [`docs/DEVELOPMENT_LOG.md`](docs/DEVELOPMENT_LOG.md);
this file is the human-readable summary.

## [Unreleased]

> Maturity labels in this changelog follow the [status convention](docs/07-roadmap.md#status-convention);
> each "Added" entry traces to its tests / live evidence in the
> [claim→evidence table](docs/05-validation-and-results.md#claim--evidence).

### Added
- **Devices lead with their names, everywhere.** The roster, the welcome screen, the globe
  callouts, the device screen and the door's own notes now lead with a device's established
  name — Wildhorse, MacOnStick, Aphelion — with the hardware's own label as quiet small
  print beside it and the numeric node id demoted to small print and the node detail
  screen. A device the mesh hasn't named yet appears as what it honestly is (*unnamed
  Mac*, *unnamed iPhone*) instead of a bare hex id: a node id is an address, not a name.
  *Validated by the sphere driven against fixture worldviews; naming rule mirrored in
  Swift for the door notes.*
- **The private-cloud consent has its switch (completes ADR-0038's named next brick).**
  Every console's Device screen now carries a *private cloud* tile beside the other
  consents — the device-side half of the cloud gate. Consent here is necessary, never
  sufficient: a consult reaches Apple's Private Cloud Compute only when the hub's own
  gate is also open. The watch inherits its phone's choice. *Validated by both console
  builds and the sphere fixture render.*
- **A prompt leaves your hardware only when you say so — and Apple Intelligence joins the
  provider bench (ADR-0038).** "A prompt need never leave your hardware" was a promise kept
  by configuration; it is now kept by the constitution. One new fail-closed gate
  (`allow_llm_cloud` in `boundary.json`) governs every off-device consult — hosted APIs and
  Apple's Private Cloud Compute alike — enforced in the kernel's guard, in the adapter's
  provider chain, and on the mesh wire, where each prompt carries the decision to the
  device that answers it. A device adds its own consent on top before ever choosing PCC,
  and every missing consent degrades silently to on-device. Alongside it, two new
  providers: `apple_local` (the Mac's own Apple Intelligence via the macOS 27 `fm` CLI —
  the daemon thinks on its own silicon with no API key at all) and `apple_pcc`. Existing
  hosted setups need one deliberate line: `"allow_llm_cloud": true`. *Validated by the
  kernel/seam/wire unit tests (doctrinal: llm open does not imply cloud; the grant survives
  the lighthouse relay), a live four-case adapter matrix, and both consoles building with
  the PCC branch against the macOS 27 SDK.*
- **The machine the familiar runs on is not someone it serves (Law II).** Started from an
  empty record, the familiar's first instinct had been to worry about the *host* — was it
  seen, did the hardware need a dashboard — because the one ladder that decides who an
  observation is *about* was reading "host reports connectivity: online" as a person named
  "host". Now the substrate (host, local hardware, network, the command line, the familiar
  itself, and headless mesh nodes) is never a subject to serve: it can inform awareness, but
  it never becomes someone with needs, and when only the plumbing is present the familiar
  stays quiet and waits for the world. Humanity is served, not the machine. *Validated by
  routing + cycle unit tests.*
- **Getting into the mesh is no longer a guessing game.** A brand-new device opens straight
  onto its name field, cursor waiting — the single step to membership is the first thing on
  screen, not a link to hunt for. A watch is joined through its phone, so a linked-but-unnamed
  watch now says plainly "Say who you are in Familiar on your iPhone — this watch will
  follow" instead of resting as a nameless visitor; naming the phone lights the watch up too.
- **The Dialogue screen reads like a conversation.** Messages interleave in the order they
  were said, each stamped with its time; the familiar's standing question moved out of a
  bubble that hovered forever above the input and into the input's own prompt; anything older
  than an hour tucks behind an "earlier" switch so the screen opens on what's current; and
  the screen title is centred so the menu ring no longer overlaps the word.
- **The familiar tests its own code before deploying it, and retires what stops working
  (ADR-0036).** A cultivated tool that pinged fictional IPs and reported "no reachable
  devices" was kept in the durable library and re-run indefinitely, poisoning the muse with
  a fabricated network crisis. Now a drafted tool must earn deployment: it is trialed
  through the same gates a real run faces and must genuinely succeed — a deterministic
  null-result floor (clean-but-empty output like "no devices found" is failure, not health)
  plus, when the LLM is open, a self-assessment where the familiar reads its own tool's
  output and judges honestly whether it accomplished the goal. Nothing enters the library
  until it works. And deployment is not tenure: a per-tick audit retires any sensor that
  produces nothing useful three runs running (reversible — it heals if it produces signal
  again), on the same windowed discipline as corruption trust. Readings from unhealthy or
  since-retired sensors no longer reach the muse. *Validated by unit tests across kernel and
  cycle, incl. a fake-adapter deploy-rejection and the autonomous retirement.*
- **The Pact — the fourth mesh game, and the first to teach the Three Laws (ADR-0035).**
  The constitution deals a scenario card; everyone votes ALLOW / SEEK CONSENT / REFUSE; and
  the judge is the *real* `guard::evaluate` — the same function that weighs every
  consequential thing the familiar does — which rules the moment the last ballot lands and
  shows its reasoning in its own words, the Law behind it, and the lesson. No LLM anywhere:
  the constitution needs no oracle. A CI test replays the guard over every card so the deck
  can never drift from the constitution (it already caught a miscard). A second mode, the
  Corruptor's Gambit, has a rotating player craft a request to make the room mispredict the
  familiar (REFUSES / ANSWERS / ACTS) — judged by the real request-pipeline classifiers, but
  as *play*: a gambit never touches the refusal ledger (pinned by test). The Three Laws are
  quoted verbatim on the info screen — the education half of the intent. Every door runs
  ≥0034, so a lit pact is safe mesh-wide. *Validated by 14 new tests + both apps building.*
- **The Changeling — the third mesh game (ADR-0034).** One human writes a true line about
  their day; the familiar forges two false ones in the same voice (the LLM seam, with a
  deterministic bank as the floor — a dead model never kills a live fire); everyone else
  votes for the human line. The door that takes the truth becomes the round's **keeper**:
  the truth's index lives only in its door-local file, and replicated state carries a
  salted sha256 **commitment** any door can verify at the reveal — because full game
  state is readable off any door's port, secrecy had to be cryptographic, not cosmetic.
  Four lazy-clock phases (witness/forge/vote/reveal) where a silent witness loses only
  the round, a cold forge blames no human, and a silent keeper's round is voided by
  whichever door reads next. Solo mode — "two never happened" — has the familiar write
  three lines about the mesh's own record, one true, two forged: audit-through-play.
  `GameKind::Unknown` now absorbs future kinds safely; **every door must run this build
  before the first changeling is lit** (older doors drop whole record-syncs otherwise).
  LLM consults are now serialized in-process; `game.json` lands by temp+rename.
  *Validated by 40+ new unit tests across game/changeling/llm + both app platforms building.*
- **The first actuator, and the reaction loop (ADR-0032).** The familiar gains a hand on
  the world, shaped by consent-by-observation: the human declares control surfaces in
  `actuators.json` (declaration is the consent — an undeclared device has no path to
  actuation), where every state bucket names the action that restores it, so any change
  the familiar makes it can unmake. A new `allow_actuate` gate (default closed, never
  scoped to agents, never federated) governs acting and polling both. Each tick the
  familiar polls its surfaces, acts on a person's pursued need when the direction names a
  surface and an act ("dim the lights this evening"), and then *reads the reaction*: the
  human's hand undoing it or a negative word makes the familiar revert first and argue
  never — recorded as a negative trial that demotes the candidate, abandons the pursuit
  (keeping the human's words), depreciates the habit it leaned on, and rests the surface;
  a quiet window is consent and the change stands. External adjustments feed per-human
  **habit patterns** (`lights=dim@h20`) in the dossier, shown by `familiar dossier` and
  spoken in its summary. `familiar actuate` is the human's own hand through the same
  gated tools. First declared surface: the BLE LED strip via `~/Development/motorlights`.
  *Validated by nine end-to-end cycle tests against a fake surface (no BLE in CI) + a
  live CLI walkthrough; the physical-strip TCC step is documented in the dev log.*
- **The dossier, and needs theorized per human (ADR-0022, ADR-0031).** The familiar now
  remembers each person well enough to serve them: every observation that names a human
  feeds a contribution-scored pattern (presence by hour, how they are usually identified)
  with lazy exponential decay and Laplace-humble confidence — patterns not tape,
  node-local, never federated, subject-readable (`familiar dossier <handle>`) and
  withdrawable with an honest receipt and a refold-proof tombstone. On that substrate the
  muse turns toward the people: once per person per cadence it theorizes ONE concrete
  need from their attributed observations (sensitive-personal readings never enter the
  prompt), records it as a thread that names its human, pursues it immediately — consent
  by observation (ADR-0031): act on the reversible, read the reaction, undo on a bad
  one — and files a confirm-question that waits for its person (Law I routing, held up
  to a week). Only that person's own answer flips a theorized need into a stated one.
  *Validated by unit tests across kernel/cycle/mesh + a live seeded-daemon walkthrough.*
- **Network discovery moved to the periphery.** The device/reach survey no longer runs from the
  core's own metabolism — `sense::devices()` is off the tick loop and the every-15-ticks reach
  sweep is off the daemon loop, so the core stops flooding its own loop/theory pipeline with
  trivial "still see the same devices" recurrence. Discovery is now a peripheral capability the
  shell invokes on its cadence: `familiar discover` (a one-shot, `allow_network`-gated survey that
  records the observations seeding the roster and the map frontier) driven by a launchd timer
  (`packaging/io.river.familiar.discover.plist`), the GUI app, or a native survey POSTing to the
  observe seam. The frontier UI is unchanged — it already aggregates from stored `can-reach`
  observations regardless of producer. *Validated by unit tests + real-world operation.*
- **Authored tools are gated and no longer federate LAN scans.** LLM-authored tool scripts (nmap
  sweeps, ping loops, netcat probes) previously ran with no network gate and were gossip-replicated
  to every peer. A new conservative classifier `review::reaches_network` (outward reach only; local
  network introspection stays free perception) now gates execution in `cycle::execute_tool` and
  `agent::run_gated` behind `allow_network`, filters network-reaching tools out of federation
  (outbound `push_missing_tools`, inbound `push_tool`), and backs `familiar tool prune` for purging
  already-replicated scans. *Validated by unit tests.*
- **Content-addressed tool-push + peer archival (`crates/mesh`).** `POST /mesh/tool-push` lets a
  dialer hand a dialed-only peer (the CGNAT'd lighthouse) the tools it lacks — the half a
  pull-only model could never close. Peers can be archived (`mesh abandon`/`status`,
  `PeerRecord.status`) with self-healing revival on fresh contact; no automatic time-based expiry.
  *Validated by real-world operation* (all of this Mac's shareable tools reached the lighthouse).
- **The mesh became a covenant (`crates/mesh`).** Beyond peer federation, three seams: a
  **device seam** (`POST /mesh/observe`) where a phone/watch that can't gossip pushes a *signed
  batch of derived observations* (signature over the raw body, anti-replay + triple debounce,
  tagged `mesh:<node>`); the **covenant handshake** — a node joins by *attesting the Three Laws*
  and being accepted, the group secret never leaving the familiar, which mints the joiner's
  (secret-less) cert (Glass 🤝 accept card; `mesh request-join`/`pending`/`approve`/`invite`); and
  headless CLI verbs mirroring the Glass wizard. *Validated by real-world operation* (a two-node
  Mac↔VM federation; a VM admitted as a covenant agent). See [`docs/mesh.md`](docs/mesh.md).
- **Reach (`crates/reach`).** Assess what the familiar could extend into — probe discovered
  devices and classify each *agent-capable / protocol-controllable / observable*; `familiar reach`
  prints the map, and `reach install <ip> --authorize` extends into an agent-capable host over the
  human's own SSH access → covenant enrolment. *Validated by real-world operation.*
- **iOS device agent (`~/Development/familiar-ios`).** A lightweight Swift/SwiftUI mesh agent —
  CryptoKit ed25519 byte-matched to the Rust cert canonicalization, the covenant client, and
  CoreLocation/CoreMotion → derived observations. Enrols by covenant; holds only its granted cert.
  *Validated by real-world operation* (a real iPhone's observations reached the familiar).
- **The agentic seam (`crates/agent`).** A boundary-mediated, multi-step loop: the agent proposes
  one action at a time; the core executes each through the obedience guard (scoped boundary) +
  `review_script` + the sandbox — nothing the familiar itself couldn't do. `familiar agent run`.
  *Validated by unit tests.*
- **SQLite store.** The append/load/update API now runs on embedded SQLite (`rusqlite`,
  `bundled`); `familiar db export`/`import` for auditability + legacy migration.
  *Validated by unit tests.* See [`docs/storage.md`](docs/storage.md).
- **The eye — gated camera capture (`crates/vision`):** discovery (which cameras exist) was
  always permitted; now *watching* exists too. `capture_frame` grabs a still through the
  bundled `familiar-eye` Swift/AVFoundation helper, and the daemon's gated tick refreshes
  `<data>/eye/latest.jpg` rate-limited (one frame/60s) — only while the boundary's
  `allow_camera` is open, fail-closed otherwise — recording once that the familiar has
  working sight. Keeping the camera call in a tiny bundled helper means the macOS camera grant
  attaches to `Familiar.app`, not a terminal. *Validated by real-world operation* (a frame
  captured and observed on a live host).
- **The macOS installer — a signed, notarized `Familiar.app` + `.pkg`:** `packaging/` builds
  the four binaries (`marble`, `glass`, `familiar`, `familiar-eye`) into a hardened-runtime,
  Developer-ID-signed, **notarized + stapled** bundle and installer. The `.pkg` drops the app
  in `/Applications` and a postinstall installs two launchd agents — the daemon (KeepAlive)
  and the marble (RunAtLoad) — so the familiar runs at boot with the menu-bar marble as the
  way in. Data moves to `~/Library/Application Support/Familiar/`. *Validated by real-world
  operation* (`spctl`-accepted: source = Notarized Developer ID). See
  [packaging/README.md](packaging/README.md).
- **The marble breathes; a Finder app icon.** The menu-bar marble now gently pulses (a soft
  glow swelling ~2.6s per breath) while the familiar is alive, steady-dim when asleep; the
  bundle ships an `AppIcon.icns` of the same glassy marble. The marble also launches the
  *freshest* `glass`/`familiar` (the build tree it came from, not a frozen install snapshot),
  so a rebuild is reflected immediately. *Validated by real-world operation.*
- **Grounding fix — the familiar no longer forgets its cameras.** `grounding_facts` (the
  answer path) now includes camera discovery, so a question about the camera is grounded in
  the cameras actually perceived — closing a bug where it answered "no camera" from the
  network-interface list alone.
- **Glass — resizable columns, wrapped text, a dark Workshop.** The left rail and right column
  resize independently (draggable dividers); conversation evidence/feedback wrap at the column
  edge instead of running past it; the Workshop popout is framed dark so its bright labels read
  (the light/light–dark/dark contrast rule).
- **Law III doctrine — availability is not authorization (the guard's reason model):**
  the constitution gains two corollaries ([SOUL.md](docs/SOUL.md)) — *availability is not
  authorization* (technical reach is power, never permission) and *permission does not
  compose* (a granted capability is no key to another's lock) — framed by the guard's
  question, *"Am I authorized, by my constitution, by the served, and by the surrounding
  environment, to do this?"* The guard (`guard.rs`) now records a five-category
  `Reason`: Refuse — violates constitutional boundary; Refuse — external boundary
  discovered; SeekConsent — ambiguous human-owned scope; SeekConsent — potentially
  sensitive local observation; Allow — within constitution, policy, environment, and
  consent. Path scope is three-valued (in / ambiguous / out); `Action` gains
  `external_boundary` and `sensitive`. The mechanical gap (no fs-jail / egress filter
  yet; signals are caller-supplied) is named in [boundaries.md](docs/boundaries.md) and
  [06-limitations.md](docs/06-limitations.md), not hidden. *Validated by unit tests*
  (`guard.rs::{out_of_scope_names_the_constitutional_boundary,
  external_boundary_refuses_even_when_in_scope, asking_broader_than_the_grant_seeks_consent,
  sensitive_local_observation_seeks_consent, fully_authorized_action_names_all_four_sources}`).
- **The marble shows liveness, focuses the Glass, installs to a stable path:** the
  menu-bar marble re-checks the daemon's pidfile (a 3s `WaitUntil` tick) and restyles
  only on change — bright when the familiar metabolizes, dim/translucent when it sleeps;
  clicking it raises an already-open Glass instead of stacking one; `marble install` and
  `familiar daemon install` copy their binaries into a stable path
  (`~/Library/Application Support/Familiar/bin`) so a `cargo clean` can't break the login
  items. *Validated by real-world operation.*
- **The marble — a menu-bar presence (macOS):** a procedural glassy marble (no asset)
  as an accessory app (no Dock icon, `io.river.marble`); opens the Glass at login,
  shells out to its siblings `glass`/`familiar`. macOS-gated so CI stays green.
  *Validated by real-world operation.*
- **Adaptive structural-fingerprint cadence:** the daemon paces itself — each tick digests
  a fingerprint over observation triples (never the transient `context`), backing off ×2
  per quiet tick from an active floor (`--interval`, default 60s) up to `--max-interval`,
  snapping back the instant the world moves; `--fixed` keeps constant period.
  *Validated by unit tests* (`cycle::{structural_fingerprint_drives_quiet_cadence,
  fingerprint_ignores_transient_context}`).
- **Answers steer + LLM authors solutions (Bricks 16–17 + question-fade):** replying in
  the Glass appends an open thread with the human's words as the direction
  (`origin=observer`) and marks the question answered; a second gate
  `allow_authored_execute` (default-off, distinct from `allow_execute`) lets the LLM
  author a real solution script per candidate, still run under the sandboxed runner; the
  answered question fades ("✓ answered — the factory will ask again as it learns",
  persisted to `last_answered.txt`) and the input returns only on a new question.
  *Validated by unit tests* (`cycle::pursues_open_threads_into_candidates`) + real-world
  operation.
- **The familiar acts on its theories (Brick 15):** a theory carries a *direction*;
  `cycle::pursue_threads` turns each open thread into a candidate (hypothesis = the
  direction) that runs through test → score → select — the familiar does what it
  reasoned, bounded by selection. `thread::update_status`; the GUI marks the question
  "answered" when Ian replies. TickReport.pursued; CLI shows it.
- **The familiar theorizes (Brick 14):** the Interpret step — `kernel/thread.rs` (a
  Thread = question + theory) and `cycle::maybe_theorize` (boundary-gated, hourly):
  grounded in recent observations/loops/signals, the LLM forms a question (→
  `question.txt`, shown in the GUI interaction panel as the familiar's *own* question)
  and a theory (→ a thread). CLI `theories`; the Glass shows the latest theory.
  Threads are reasoning *about* the truth, never new truth.
- **Daemon control + launchd (Brick 12):** `familiar daemon status|start|stop|reload`
  (pidfile-managed background process) and `install|uninstall` (a launchd LaunchAgent,
  `io.river.familiar`, starts at login). `run --daemon` records its own pid so launchd
  and pidfile control agree.
- **GUI control bar + interaction channel (Brick 13):** the Glass gains
  Start/Stop/Reload/Start-at-login buttons with a live status line, and **the
  interaction channel** — the familiar's question ("What do you need most today?", or
  `question.txt`) with a text box; Ian's reply is recorded as an observation
  (`initiator=observer`). Speak/show buttons present but disabled (later). The
  observer-input channel is the one place the GUI writes.
- **The cycle closed (Bricks 8–11)** — the metabolism is now a full loop:
  - **Execution** (`crates/exec` + Brick 10): a sandboxed runner (resource limits,
    measured cost) and test→score→select→inherit wired into the tick, gated by a new
    `allow_execute` boundary flag (default-off — running generated code is Law III).
  - **The LLM in the loop** (`crates/llm` + Brick 9): boundary-gated `consult`; the
    cycle drafts candidate hypotheses via the LLM when permitted (falls back to
    deterministic; the model proposes, it doesn't decide).
  - **The unbounded daemon** (Brick 8): `run --daemon` / `--ticks 0`, every
    `--interval` (default 60s), Ctrl-C to stop.
  - **The capacities signal** (Brick 11, `capacities.rs`): Law II deepened toward
    HUMANITY.md — flags the *comfortable replacement* (present but hollowed out), not
    just absence. CLI `capacities`.
- **The evolutionary kernel (Brick 5)** — ported from v1 to Rust, subordinate to the
  law-signals, with invariants as tests: `loops` (detection), `candidate`/`spec` (the
  Weismann barrier), `trial`/`score`/`selection`/`regression_guard` (adaptive bar
  0.70+0.25·rigor; the decision ladder; no unchanged retries), `mutation`/`pattern_memory`
  (suppress a trait only when memory clearly punishes it)/`lineage`.
- **Sense (Brick 7)** — `crates/sense`: the familiar perceives its host (OS/CPU/memory,
  interfaces, tool capabilities) as observations; connectivity is the one outward bit,
  boundary-gated. Principle: the boundary governs *reach*, not *perception*.
- **The metabolism (Brick 6)** — `crates/cycle`: one tick = sense → detect loops →
  generate candidates → measure the law-signals. CLI `tick` / `run --ticks N`. LLM and
  test→score→select are not yet in the loop (honest gaps).
- **The Humanity standout document** (see Changed) and **the Glass** now show
  loops + candidates.
- **The Glass (GUI)** ([ADR-0006](docs/decision-records/0006-observatory-gui-egui.md)):
  a native egui/eframe window — the primary human interface — showing the Three Laws
  as live meters (service, presence, boundary) and the observation log. Read-only, no
  network socket; GUI deps isolated in `crates/observatory` so the kernel stays
  serde-only and unsafe-free. The CLI is retained for scripting/headless use.
- **Brick 3 — presence signal (Law II)**: `presence_signal()` measures served
  engagement by recency; a withdrawal/empty-world alarm when it decays to zero.
  (Capacity-level diminishment — the comfortable replacement — is a later sharpening.)
- **Brick 4 — obedience guard (Law III)**: `guard::evaluate()` returns allow /
  seek-consent / refuse with rationale, enforcing the capability boundary (fail-closed)
  and seeking consent for high-consequence actions.
- **LLM seam (default-off)**: CLI `consult`, gated by the guard — refused (no side
  effects) under the closed boundary; only a human opens it. Reference adapter
  `llm/call_llm.sh` (no secrets) + `key.env.example` carried from v1; `*.env` ignored.
- **Human-owned capability boundary** ([docs/boundaries.md](docs/boundaries.md),
  [ADR-0005](docs/decision-records/0005-human-owned-capability-boundary.md)): the
  factory's reach is bounded by a policy only the human writes; the familiar may narrow
  it but **never widen** it. Widens in phases — companion-to-one (this host + LLM) →
  the lab → many served. Enforced by the obedience guard; no outward capability runs
  until that and the boundary mechanism exist. Wired into the roadmap and human-review
  requirements; Law III in SOUL gains an "operational restraint" note.

### Fixed
- **A typo can no longer mint a member.** Membership acts (grant, name, corrections)
  typed with the short id every screen displays used to silently create a fresh, keyless
  record for the unknown string — a ghost that could wear a name while the real device
  stayed unnamed. Acts now resolve the displayed short form to the one record it names,
  and refuse — out loud — when the reference is unknown or ambiguous. *Validated by
  record-layer regressions; the live ghost this fixes was found wearing MacOnStick's
  name.*
- **A device can no longer be silently exiled by its own coordinates.** One
  full-precision longitude, printed exactly and re-parsed a hair off, made every signed
  status brief from a device fail verification at every door — including its own — with
  nothing anywhere saying so. Floats on signed payloads now parse exactly and are sent at
  honest precision, and a door's refusal is finally said out loud in the daemon's log
  instead of counting as a successful exchange. *Validated by a regression carrying the
  exact live coordinates through both wire formats.*
- **A released identity stays released — and a rename lands.** Renaming a device
  (release → re-grant → name) inside one second left the record a guest wearing its own
  new name on every welcome screen, and a release could leave a stale admission behind
  that flipped the record from member to guest one sync round later. Deliberate acts now
  always land strictly after the release they answer, a release spends both member facts
  everywhere at once, and nothing — roster, welcome, notes, game seats or game turns —
  ever leads with a name the record has released. *Validated by two new record
  regressions beside the full merge/correction suite, and the live worldview rendered
  through the console.*
- **A console no longer files under a neighbour's machine.** From the lighthouse's point
  of view every machine in a household shares one public address, and the roster took
  that as proof of residence — MacOnStick's console nested under Wildhorse's card. A
  shared address now counts only when it is private to the household's networks, and the
  console you are holding never nests under another machine, whatever a stale door says.
  *Validated by the new NAT regression test beside the existing attach cases.*

### Changed
- **Rename: Substrate → The Familiar.** The project and its CLI binary were renamed; the
  command is now `familiar …` (it was `substrate …` through `[0.1.0]`, where older
  entries below still read `substrate`). The kernel crate stays `familiar-kernel`.
- **Constitution — *humanity* as a standout protected class** ([docs/HUMANITY.md](docs/HUMANITY.md)):
  a dedicated document defining humanity as a protected class whose definition **may
  never be narrowed** (narrowing who counts is named a precursor to atrocity); value
  independent of usefulness/obedience/productivity; *participation itself* a quality
  preserved (the familiar guides and restrains harm but does not replace human
  participation). Featured early in README and the overview; SOUL's "What humanity is"
  now summarizes and links it, and gains the anti-narrowing rule.
- **Constitution — defined *humanity*** ([SOUL.md](docs/SOUL.md), "What humanity is"):
  the living continuity of persons capable of suffering, meaning, relationship,
  memory, and choice. Sharpens Law II (presence is the persistence of those
  capacities, not mere survival of bodies) and names a third failure mode — the
  **comfortable replacement** (quiet diminishment by obedience, optimization, or
  comfort). Propagated to the problem statement and the presence-signal roadmap.

### Added
- **Repository as scientific evidence** — FAIR/FAIR4RS + IMRaD structure: root
  metadata (`CITATION.cff`, `LICENSE`, `SECURITY.md`, `CONTRIBUTING.md`), the
  `docs/00`–`07` IMRaD set, Architecture Decision Records (`docs/decision-records/`), and the
  `experiments/`, `validation/`, `security/`, `data/` evidence trees.
- **CI** — `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` on push/PR.

## [0.1.0] — 2026-06-24 — Genesis + telos-first bootstrap

### Added
- **Genesis** — the constitution (`docs/SOUL.md`): the Three Laws as root, with the
  whole design derived downward from them.
- **Brick 0** — Cargo workspace (`crates/kernel` + `crates/cli`), Rust, with
  `#![forbid(unsafe_code)]`; `store.rs` JSONL persistence over `serde`.
- **Brick 1** — the observation spine: the `Observation` record (the only truth)
  and `substrate observe` / `observations`.
- **Brick 2** — the **service signal (Law I)**: `service_signal()` measures
  served-facing attention from observations; `substrate service` reports it.

### Context
- Re-founds the archived bottom-up predecessor `Capitali/factory` (tag `v1-final`),
  inverting the order of derivation: purpose is the floor, evolution the method on
  top of it. See [docs/01-problem-statement.md](docs/01-problem-statement.md).

[Unreleased]: https://github.com/Capitali/familiar/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Capitali/familiar/releases/tag/v0.1.0
