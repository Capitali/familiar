# Design: first-launch orientation → identity-first interfacing → multi-nodal mesh

**Status: design / roadmap.** Captures Ian's intended arc so it can be built incrementally
without re-discovery. One idea at two scales: the familiar *orients itself* on arrival, then
*begins serving identity-first*; and eventually a node orients toward its *peers* too.

## The arc

1. **Clean install → orientation.** The familiar scans everything it can reach to learn
   *where it is*, *who the potential human observers are*, *how the system is used*, and
   *any history of use*.
2. **Begin interfacing, identity first.** Grounded in what it learned, it starts the
   relationship by learning who it serves — the name (shipped: `kernel::identity`, the
   cycle's name-ask, the Glass confirm/keep flow).
3. **(Future) multi-nodal mesh.** When the familiar runs on more than one node, orientation
   also means announcing its presence to peers (over Tailscale) and receiving their briefs:
   each peer's existence, its human observers, its serving queue + priority, and its library
   of self-written tools/assets that could be shared.

## What already exists (build on, don't rebuild)

- `sense::census / interfaces / capabilities` + `vision::discover` — scan host facts, network
  interfaces, installed tools, and cameras every tick. The environment-scan half is largely
  here; orientation is mostly *synthesis + a first-run framing* over these.
- `kernel::identity` — the retained registry (handle, verbatim name, relation, first/last
  seen, interaction count) + the current-observer pointer. The "begin with identity" landing.
- `tool` / `tools.jsonl` + the workspace scripts — the per-node "library of self-written
  code" that is the natural unit to share across nodes.
- The boundary (Law III) — the human-owned capability gates; the model for every new reach.

## Constitutional lines (must hold)

- **Orientation is reach.** Identifying observers and "history of use" reads personal data.
  Low-sensitivity hints (system username, `git config user.name/email`, hostname) are fine;
  shell history / recent files / login history are sensitive and stay inside the granted
  `fs_read` scope. Discovery-vs-use is the same line we drew for the camera: knowing a thing
  exists is perception; reading its contents is gated.
- **Mesh is transmission.** Node-to-node briefs are outward transmission — the exfiltration
  surface Law III guards. Sharing *tools* is benign; sharing *observers* across nodes is
  sensitive and needs scoping + consent, so a node never leaks one person's data to another.
  Gate it behind a new fail-closed capability (`allow_mesh`), human-opened, never self-widened.

## Bricks

### A — first-launch identity hints (buildable now)
Make the name-ask *informed*. On first run, gather low-sensitivity identity hints — system
username, `git config user.name` / `user.email`, hostname — and present them in the name-ask:
"I see I'm running as 'ian' on 'wildhorse' — shall I call you Ian, or something else?" Realizes
"begin interfacing, querying based on its learnings, beginning with identity." Small, ties to
the shipped identity feature, low Law III cost.
- Hooks: `sense` (add an `identity_hints(now)` perception), the cycle's name-ask step, the
  Glass name panel (prefill / offer the hint as a one-click answer, still confirmed).

### B — deeper orientation (usage + history, gated)
A first-run orientation pass that synthesizes the scan into a usage picture and candidate
observers (e.g. from accounts, ownership, activity). Strictly within `fs_read`; sensitive
sources stay off unless granted. Feeds richer first questions.

### C — multi-nodal mesh (future, its own brick)
- **Node identity**: each node has a stable id + presence record.
- **Transport**: Tailscale tailnet (the river.io fabric already uses it).
- **`allow_mesh` gate**: fail-closed; local-only until a human opens it; peers are explicitly
  trusted, not discovered-and-trusted.
- **Brief protocol**: on greet, exchange — presence, observers (scoped/consented),
  serving queue + priority, and a manifest of shareable tools/assets. Tools are the safe
  first thing to share (a node can offer a capability it authored to a peer that lacks it).
- **Sharing policy**: tools shareable by default once `allow_mesh` is open; observer/personal
  data shared only under explicit scoping — never a blanket gossip of who-knows-whom.

## Notes

- `std::env::consts::OS` / `ARCH` and the existing `sense` perception are the right basis for
  orientation; it is mostly framing + synthesis, not new sensing.
- Orientation should run once on first launch (a first-run marker), not every tick — the scan
  itself already runs each tick; what's first-run-only is the *synthesis + the opening move*.
