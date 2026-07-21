# 07 — Roadmap (Future Work)

The build sequence is **telos-first**: make the laws measurable before porting the
inherited machinery. Status is tracked in [CHANGELOG.md](../CHANGELOG.md) and, per
brick, in [DEVELOPMENT_LOG.md](DEVELOPMENT_LOG.md).

## Status convention

So the maturity of each piece reads the same everywhere, one vocabulary is used across
the README, the docs (architecture, validation, limitations, this roadmap), and the
changelog. The first five are **maturity rungs** — cumulative, so a higher rung implies
the lower ones, and a component is tagged with the highest it has reached. **Planned**
and **Deprecated** are lifecycle states, not rungs.

| Status | Meaning |
|---|---|
| **Implemented** | Code exists and runs. |
| **Implemented but not validated** | Built, but nothing yet checks that it behaves. |
| **Validated by unit tests** | Its invariants are encoded as passing unit tests. |
| **Validated by scenario tests** | Exercised end-to-end against scenario fixtures. |
| **Validated by real-world operation** | Demonstrated doing its job in a live run on a real host. |
| **Planned** | Designed, not yet built. |
| **Deprecated** | Superseded; retained for history. |

The mapping of each component to its rung **and the evidence behind it** (the specific
tests, the live experiment, or an explicit "not yet validated" marker) is the
claim→evidence table in [05-validation-and-results.md](05-validation-and-results.md#claim--evidence).
The rule the whole repository holds to: **every major claim traces to a test, a
scenario, a log, a limitation, or an explicit "not yet validated" marker** — never to
assertion alone.

## Done

- **Genesis** — the constitution ([SOUL.md](SOUL.md)); *humanity* defined.
- **Brick 0** — Cargo workspace (Rust), `store.rs`, `#![forbid(unsafe_code)]`.
- **Brick 1** — the observation spine.
- **Brick 2** — the service signal (**Law I**).
- **Brick 3** — the presence signal (**Law II**): engagement recency + withdrawal alarm.
- **Brick 4 / 4b** — the obedience guard + the human-owned capability boundary (**Law III**).
- **The Glass** — native egui GUI ([ADR-0006](decision-records/0006-observatory-gui-egui.md)).
  *Superseded 2026-07-17* by the SwiftUI consoles (below); archived under `archive/` with the marble.
- **The LLM seam** — `consult` + `crates/llm`, boundary-gated and **default-off**.
- **The kernel (Brick 5)** — loops, candidate/spec (Weismann), trial/score/selection/
  regression-guard, mutation/pattern-memory/lineage, ported with invariants as tests.
- **Sense (Brick 7)** — the familiar perceives its host (`crates/sense`).
- **The metabolism (Brick 6)** — the tick: sense → detect → generate → measure.
- **The cycle closed (Bricks 8–11):** execution (sandboxed runner + test→score→select,
  `crates/exec`, gated by `allow_execute`); the LLM in the loop drafting hypotheses;
  the unbounded daemon (`run --daemon`); and the **capacities signal** (Law II deepened
  — the comfortable-replacement alarm).
- **Repository as evidence** — FAIR/IMRaD structure, ADRs, CI.
- **The eye (`crates/vision`)** — camera *discovery* (always permitted) plus *gated* still
  capture (`allow_camera`, fail-closed) via the bundled `familiar-eye` AVFoundation helper;
  the daemon refreshes a latest frame on a rate limit and records that it watched.
  *Validated by real-world operation* (a frame captured and observed on a live host).
- **The macOS installer** — a signed, **notarized** `Familiar.app` + `.pkg` that installs the
  app and the launchd agents (daemon KeepAlive + the breathing menu-bar marble at login).
  *Validated by real-world operation* (notarized, stapled, `spctl`-accepted). See
  [`../packaging/README.md`](../packaging/README.md).
- **SQLite store** — the append/load/update API now runs on embedded SQLite (`rusqlite`,
  `bundled`); `db export`/`import` for auditability + legacy migration. *Validated by unit tests.*
- **The agentic seam (`crates/agent`)** — a boundary-mediated, multi-step loop: the agent proposes
  one action at a time, the core decides and gates each through the obedience guard + review +
  sandbox. *Validated by unit tests.*
- **Mesh federation (`crates/mesh`)** — ed25519 group trust, signed briefs over the tailnet/LAN,
  in-tick merge of tools/patterns (+ opt-in identities). *Validated by real-world operation* (a
  live two-node Mac↔VM federation). See [mesh.md](mesh.md).
- **The covenant handshake** — join by *accepting the Three Laws*; the group secret never leaves
  the familiar, which mints the joiner's cert (Glass accept card + `mesh approve`/`invite`).
  *Validated by real-world operation.*
- **The device seam (`/mesh/observe`)** + the **iOS device agent** ([`../ios/`](../ios/),
  Swift/SwiftUI + CryptoKit) — a phone enrols by covenant and pushes derived observations
  (location/motion; health next). *Validated by real-world operation* (a real iPhone's
  observations reached the familiar). See [mesh.md](mesh.md).
- **The SwiftUI consoles** ([`../ios/`](../ios/)) — the standard dark console on every Apple
  shell: the native macOS console (retiring the egui Glass), the iPad/iPhone apps, and the
  watch companion; the worldview seam (`/mesh/worldview`, loopback `/local/worldview`), the
  roster + mesh map (a graph of equals), dialog with the familiar + remote gate control, and
  the Metal sphere/orb interface (SceneKit — marble/mesh/globe, shared across shells).
  *Validated by real-world operation* (deployed Mac console; iPhone/iPad via TestFlight;
  watch enrolment live).
- **Theory quality** — a theory is scored against the outcomes of the ones before it
  (`score.rs::score_theory`); a known dead end is abandoned as negative evidence instead of
  spending a candidate. *Validated by unit tests.*
- **The theory→code bridge** — `cycle::cultivate_utilities`: a proven observation-goal theory
  becomes a durable, re-runnable tool whose output is retained as a `gathered` observation;
  gated (execute + authored-execute + LLM), paced, deduped, health-tracked. *Validated by
  unit tests + live operation.*
- **Mesh trust lifecycle + automatic peering** — corruption-awareness as a graduated,
  reversible ladder (`kernel::corruption`: monitor → throttle → marginalize → sever; ages
  out; expulsion stays a human act) and `auto_peer`/`auto_accept_enrollments` (both
  fail-closed) so peering happens once the human opens the gate. *Validated by unit tests +
  live operation.*
- **Autonomy Stage 1 — the mesh owns a shared roadmap.** `kernel::goal` (proposed → claimed →
  in_progress → awaiting_human → done/failed/blocked) + `kernel::capabilities` (what a node
  can DO — toolchain ∩ open gates); goals replicate in the brief, a capable node claims and
  drives one through the agentic loop per tick; deploy-class goals park for a human (Law III).
  The consoles show the Roadmap board. *Validated by unit tests + live operation.*
- **Self-upgrade foundation** — the orderable build version (`VERSION` + `build.rs` →
  `kernel::version`) and the `allow_self_upgrade` gate (the sharpest gate; fail-closed,
  dormant until deliberately opened; a scoped agent never gets it). *Validated by unit tests.*
- **The humanity ledger** (`kernel::humanity`) — an append-only ledger where the familiar
  grows its *lived* understanding of the people it serves; `HUMANITY.md` itself stays
  immutable. *Validated by unit tests.*
- **Reach (Bricks 2.1 / 3)** — `familiar-reach` assesses what the familiar could extend into
  (agent-capable / protocol-controllable / observable) and, with consent (`reach install
  --authorize`), extends into an agent-capable host via SSH → covenant enrolment. *Validated by
  real-world operation* (a LAN reach map; a VM admitted as a covenant agent).

The full cycle now runs — observe → detect → generate (LLM-drafted) → test → score →
select → inherit — under the law-signals (service, presence, capacities) and the
human-owned boundary. Outward reach (network, LLM, execution, **watching through the
camera**) is each a separate gate only a human opens; the familiar never widens its own.

## Next — sharpen and reach

Everything in this section is **Planned**. The first item is what lifts the cycle from
*Validated by real-world operation* on a thin task to *Validated by scenario tests* —
the one maturity rung the codebase has not yet occupied (no scenario fixture set exists
yet; see [06-limitations.md](06-limitations.md)).

- **Real scenarios & LLM-authored artifacts.** *(Planned.)* Today's artifacts are
  deterministic and safe; LLM-*authored* execution is built but behind its own gate
  (`allow_authored_execute`, default-off). Next: a scenario fixture set so candidates
  are tested against real tasks and selection genuinely discriminates — the move onto
  the **scenario-tests** rung. Directional plan:
  [SCENARIO-FRAMEWORK-DESIGN-BRIEF.md](SCENARIO-FRAMEWORK-DESIGN-BRIEF.md).
- **Rigor & adaptive cadence.** Feed a measured rigor drive into the promotion bar; give
  the daemon structural-fingerprint cadence (slow when nothing changes).
- **Sharpen the signals.** Service beyond attention (needs *reduced*); capacities beyond
  the verb-lexicon proxy; presence per-person.
- **Reach, continued.** Brick 2.2: richer discovery (mDNS/Bonjour for HomeKit/AirPlay-2 on
  random ports, BLE), and wire `reach` into the tick so the map stays fresh. Protocol adapters
  (AirPlay/Roku/MQTT) so protocol-controllable devices become *commandable*, not just seen.
- **Device agents (the reach frontier).** *(In progress, ordered.)*
  1. **iPadOS + watchOS agents** — *largely done*: the iPad console and the watch companion
     exist and enrol (WatchConnectivity address handoff live). Remaining: HealthKit on
     iPhone/iPad and richer on-wrist relay (heart rate/motion).
  2. **Speech recognition** — on-device `SFSpeechRecognizer` + `AVAudioEngine` → derived
     observations (spoken intent / ambient speech), consent-gated mic, never raw audio by default.
  3. **Facial recognition + analysis (iPadOS)** — Vision-framework face detection/analysis →
     derived observations of who is present / attention / expression, tagged and consent-gated;
     feeds people as first-class entities. Derived-only; raw frames stay on device.
- **The covenant horizon.** The far telos: familiars (and, eventually, other AIs) accept the
  Three Laws and join the mesh by consent and demonstrated advantage — never coercion (see
  [design-orientation-and-mesh.md](design-orientation-and-mesh.md)). The covenant handshake is
  its built primitive.

## Capability & the companion phases

Reach is enabled deliberately by the human, in phases ([boundaries.md](boundaries.md)).
The familiar operates freely *within* the current boundary and never widens it itself.

- **Phase 1 — companion to one, on one host** *(open)*: this host + its data + the LLM
  seam (boundary + guard + `consult` all built; enabled by a human editing
  `boundary.json` + installing keys).
- **Phase 2 — the lab.** The human lifts the boundary to other devices/interfaces;
  richer sensing (LAN neighbours/netscan, boundary-gated, per-OS).
- **Phase 3 — many served.** Multiple humans; the world-model + entity tagging (which
  sharpens the service signal so proper names resolve) and people as first-class
  entities with per-person, human-paced cadence.

## Cross-cutting, ongoing

- Close the limitations in [06-limitations.md](06-limitations.md) as the relevant
  bricks land (service-rendered vs. attention; ratio penalty; benchmarks).
- Keep the evidence trees ([validation/](../validation/), [security/](../security/),
  [experiments/](../experiments/)) current with each brick — evidence is part of the
  deliverable, not an afterthought.
