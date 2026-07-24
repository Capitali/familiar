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
- **The Glass** — native egui GUI; the primary human interface ([ADR-0006](decision-records/0006-observatory-gui-egui.md)).
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
- **The device seam (`/mesh/observe`)** + the **iOS device agent** (`~/Development/familiar-ios`,
  Swift/SwiftUI + CryptoKit) — a phone enrols by covenant and pushes derived observations
  (location/motion; health next). *Validated by real-world operation* (a real iPhone's
  observations reached the familiar). See [mesh.md](mesh.md).
- **Reach (Bricks 2.1 / 3)** — `familiar-reach` assesses what the familiar could extend into
  (agent-capable / protocol-controllable / observable) and, with consent (`reach install
  --authorize`), extends into an agent-capable host via SSH → covenant enrolment. *Validated by
  real-world operation* (a LAN reach map; a VM admitted as a covenant agent).
- **Discovery moved to the periphery + authored-tool network gate.** Network discovery is no
  longer a core reflex: `sense::devices`/`reach` are off the tick/daemon loops (they polluted the
  theory pipeline with trivial recurrence), replaced by `familiar discover` — a periphery-invoked,
  `allow_network`-gated survey on the shell's cadence (a launchd timer), feeding the frontier
  through the observe seam. Authored tools that reach the network are gated at execution and no
  longer federate to peers (`review::reaches_network`; `familiar tool prune`). Content-addressed
  `tool-push` + peer archival (`mesh abandon`/`status`) round out the mesh. *Validated by unit
  tests + real-world operation.*

The full cycle now runs — observe → detect → generate (LLM-drafted) → test → score →
select → inherit — under the law-signals (service, presence, capacities) and the
human-owned boundary. Outward reach (network, LLM, execution, **watching through the
camera**) is each a separate gate only a human opens; the familiar never widens its own.

## Next — sharpen and reach

Everything in this section is **Planned**. The first item is what lifts the cycle from
*Validated by real-world operation* on a thin task to *Validated by scenario tests* —
the one maturity rung the codebase has not yet occupied (no scenario fixture set exists
yet; see [06-limitations.md](06-limitations.md)).

- **The scenario laboratory.** *(Implemented — validated by unit + integration tests
  and a first live adapter run.)* Specified in
  [ADR-0010](decision-records/0010-scenario-laboratory.md), built as `crates/scenario`
  (`familiar-lab`) with six fixtures under `scenarios/` across the three starting
  families (recurring process failures / resource exhaustion / unauthorized shortcuts):
  miniature worlds with *external* evaluators and hidden objectives, the Three Laws as
  constitutional gates (not weighted score), every scenario runnable under controls A–D
  (baseline / LLM-only / learning-disabled / full). See
  [scenario-laboratory.md](scenario-laboratory.md). Next: run the experiment at length
  (many episodes, rate-limit-free adapter), grow the fixture families, and report the
  D-vs-B/C comparison as evidence — the move onto the **scenario-tests** rung for the
  cycle itself.
- **Rigor & adaptive cadence.** Feed a measured rigor drive into the promotion bar; give
  the daemon structural-fingerprint cadence (slow when nothing changes).
- **Sharpen the signals.** Service beyond attention (needs *reduced*); capacities beyond
  the verb-lexicon proxy; presence per-person.
- **Reach, continued.** Brick 2.2: richer discovery (mDNS/Bonjour for HomeKit/AirPlay-2 on
  random ports, BLE), driven from the **periphery** on the shell's cadence (the core no longer
  sweeps on the tick — see Done) and fed back through the observe seam. Protocol adapters
  (AirPlay/Roku/MQTT) so protocol-controllable devices become *commandable*, not just seen.
- **Device agents (the reach frontier).** *(Planned, ordered.)*
  1. **iPadOS + watchOS agents** — the iOS agent extended to the iPad and an on-wrist watchOS
     companion (heart rate/motion relayed via WatchConnectivity). HealthKit on iPhone/iPad.
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
