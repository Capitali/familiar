# System state

Controller-owned except the Companion notes section. If this file disagrees with
reality, fixing it is the first task. Updated: 2026-08-14 (controller).

## The tree

- **main tip:** `8363f15` — every brick through discovery-naming + narration. CI green.
- Shared checkout: `~/Projects/familiar` on MacOnStick — leave it on `main`, clean.
  Long work: use a scratch worktree (rule 7).

## The fleet

| node | runs | notes |
|---|---|---|
| MacOnStick daemon (3d68a0689bc32771) | 8363f15 | controller deploys this one; label MacOnStick, established ian |
| lighthouse (f56e5601, 134.209.168.50) | 7be4f31 | held → deploys 8363f15 in the consolidated pass |
| Wildhorse daemon (1c991bc6c1c4aa4f) | 7be4f31 | held → same pass |
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

- **ADR-0040 acceptance** — the reasoning engine's converged design (proposed;
  docs/decision-records/0040-the-reasoning-engine-grows-honest.md). Building continues
  on the decided tasks meanwhile; the ADR formalizes it.

- **Wildhorse's real coordinates** → written to its `data/mesh/geo.json`, **or** "zero
  it" → delete the file, node reads honestly unlocated. Until then its pin wears ≈.
- (Dissolving:) the Codex/Aphelion mapping — discovery decides in pass step 2; manual
  naming only on unambiguous discovery, else skipped entirely.
- Build 85 testing on his devices, once shipped (notification will be waiting).

## Standing directions from Ian (recorded, binding)

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
