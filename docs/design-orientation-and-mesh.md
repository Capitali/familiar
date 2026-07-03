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

### C — multi-nodal mesh (**implemented** — `crates/mesh`; full protocol in [mesh.md](mesh.md))
Built as `familiar-mesh`. The trust model **evolved** from this stub's original
"peers explicitly trusted, not discovered-and-trusted" to **gossip discovery + auto-merge of
cryptographically group-verified peers** — the human authorizes the *group*, and within it any
peer with a valid membership certificate is auto-trusted. The human still owns the gate; the
familiar never self-widens.
- **Node identity**: each node has a stable ed25519 keypair; `node_id` is a self-certifying
  fingerprint of its pubkey.
- **Group trust root**: a group ed25519 keypair. The join key *is* the group secret; holding it
  is membership (the power to mint a membership certificate binding a node to the group).
- **Transport**: native async HTTP over the Tailscale tailnet (the river.io fabric already uses
  it); gossip discovery via `tailscale status --json`, no central server.
- **`allow_mesh` gate**: fail-closed; local-only until a human opens it and enrolls a group.
- **Brief protocol**: exchange presence (counts, never names), a shareable-tool manifest
  (bodies fetched on demand), distilled patterns, and — only under per-human opt-in — scoped
  identity. Tools are the safe first thing to share.
- **Sharing policy**: tools + patterns shareable by default once `allow_mesh` is open and a
  group is enrolled; personal/identity data only under explicit per-human, per-group opt-in —
  never a blanket gossip of who-knows-whom. Peer tools still pass `review_script` + sandbox +
  `allow_execute` on use; peer observations are tagged `source="mesh"`, never laundered.

## Identity linking across modalities (future)

Identity awareness only grows in importance, and visual/auditory observation is what will
let the familiar maintain a *linked* identity — recognising the same person across sessions
and channels without asking each time.

- **The registry is the anchor.** `kernel::identity::Identity` is the durable record a person
  is linked *to*. It grows link/signature fields over time: a face embedding (from the now-
  authorized camera), a voice print, and behavioural/pattern signatures (rhythms, phrasing,
  schedule). Each is a *link* to an existing identity, not a new record — the familiar
  recognises "this is the person I already know as X".
- **Recognition feeds identity, identity feeds presence.** Face/voice/pattern recognition
  resolves *who is present* — sharpening Law II (presence judged against the actual served,
  the entity tagging the cold-start classifier lacked) and letting the familiar greet by name
  rather than re-ask.
- **Precise and consented, like names.** A new link is confirmed before it's kept (the same
  read-back discipline as the name), and biometric signals are strongly sensitive — gated like
  the camera (fail-closed, human-opened), never shared across mesh nodes without explicit
  scoping. Recognition must be correctable: a wrong link is fixable, never sticky.
- **Hooks in place:** the camera gate is open; the identity registry exists with a `relation`
  field and room for signatures; the confirm-before-keep flow is the model for confirming a
  link. The recognition pipelines (vision/audio embeddings) are the new work.

## Notes

- `std::env::consts::OS` / `ARCH` and the existing `sense` perception are the right basis for
  orientation; it is mostly framing + synthesis, not new sensing.
- Orientation should run once on first launch (a first-run marker), not every tick — the scan
  itself already runs each tick; what's first-run-only is the *synthesis + the opening move*.
