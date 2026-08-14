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

**One consolidated pass, fires only on Ian's explicit go** (deliberately not before —
mesh healthy, features not repairs, Build 84 mid-TestFlight-review):

1. Deploy `8363f15` to lighthouse + Wildhorse daemons.
2. Wait a few sync rounds; `mesh device show` on the lighthouse — the phones likely
   **self-named** via wildhorse's mDNS + tailnet (discovery naming). Manual
   `mesh device name` only for what discovery missed or Ian's word overrides.
   **Never name betty's (10ba2c1c…) or mol's (ad4c704d…) devices manually.**
3. Ship Build 85 (consoles gain: theory drill-down, cluster zoom, ≈ provenance marks,
   dialog answer-threading, self-named roster).
4. Wildhorse geo per Ian's choice (below).

## Waiting on Ian

- **The go** for the consolidated pass above.
- **Wildhorse's real coordinates** → written to its `data/mesh/geo.json`, **or** "zero
  it" → delete the file, node reads honestly unlocated. Until then its pin wears ≈.
- (Dissolving:) the Codex/Aphelion mapping — likely self-answers via discovery in
  pass step 2; his word still wins if discovery disagrees.

## Standing directions from Ian (recorded, binding)

- Roster reads `SystemName : SystemType : ServedUser`; ids are small print.
- Names come from autodiscovery (mDNS/tailnet/local-DNS); router config never required.
- Humans and devices are separate rich records; roster is a view (ADR-0039, accepted).
- The familiar narrates what it changes and why, to the humans, at change time.
- FamTalker01 is a virtual smart home — explore, begin to control, report when human
  attention would help.
- The companion AI is a full coding partner: coding, planning, design all hand off.

## Companion & infra notes

*(any non-controller session — companion engineers and the infra/fleet-ops session
alike: append dated one-liner FACTS here — session started/ended, a pass executed and
its results, anything the controller should read before its next arbitration. The
controller folds these into the sections above and prunes. 2026-08-14, controller:
lane confirmed with the infra session — it appends its own facts here after fleet ops;
I keep the authoritative sections true.)*
