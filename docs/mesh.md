# Mesh — peer federation over the tailnet

**Status: implemented (`crates/mesh`, `familiar-mesh`).** When a familiar runs on more than
one node, the mesh lets those nodes find each other, prove they belong to the same group, and
share what they've learned — **tool solutions**, **distilled patterns/knowledge**, and (only
under explicit opt-in) **knowledge of the humans they serve**. It exists in service of the
Three Laws: a wider, corroborated picture of who is served and how (Laws I/II), better tools
spread so the future is cheaper (Law I), and a node never turned against people by a bad peer
(Law III — the human owns the gate; trust is cryptographic, not ambient).

Beyond peer-to-peer federation it now carries three further seams, documented below: a
**covenant handshake** (join by accepting the Laws — the secret never leaves the familiar), a
**device seam** (`/mesh/observe` — phones/watches push signed derived observations), and
**reach** (`familiar-reach` — assess and, with consent, extend into agent-capable hosts).

**CLI:** `familiar mesh <create-group | join --key | request-join --host | key | qr | peer |
share | accept-observations | pending | approve | deny | invite | optin | status>`, and
`familiar reach [install <ip> --authorize]`. The Glass "Mesh" panel mirrors these (create/join
wizard, the 🤝 accept card, sharing switches).

This document is the protocol + threat model. The narrative arc lives in
[design-orientation-and-mesh.md](design-orientation-and-mesh.md) §C; the capability gate lives
in [boundaries.md](boundaries.md).

## The shape

```
 async background thread (tokio, gated by allow_mesh)          SYNC tick (auditable)
 ┌──────────────────────────────────────────────────┐        ┌──────────────────────────┐
 │ familiar-mesh::transport                           │        │ cycle::tick              │
 │  • enumerate tailnet peers (tailscale status)      │        │  • familiar_mesh::federate│
 │  • POST/GET signed briefs over HTTP (tailnet IPs)  │ inbox/ │     - re-verify cert+sig  │
 │  • verify membership cert + sig at INGRESS         │ ─JSONL▶│     - apply merge policy  │
 │  • drop junk; write verified briefs to mesh/inbox  │        │     - build+write outbox  │
 │  • pre-fetch tool bodies (content-addressed)       │◀outbox─│  • record activity        │
 │  • write mesh/peers.json + mesh/status.txt         │        └──────────────────────────┘
 └──────────────────────────────────────────────────┘
```

**Transport does IO, the tick does constitution.** The async transport verifies signatures at
ingress (cheap, rejects junk early) but **never mutates the canonical stores**. Every
federated change to `tools.jsonl` / `patterns.jsonl` / `observations.jsonl` / `identities.jsonl`
happens in the synchronous tick (`merge.rs`), so it is observable in `ticks.jsonl` and governed
by the same metabolism and boundary. This is the same discipline as the LLM seam: the periphery
fetches, the auditable core decides.

## Trust: group membership, not IP

- Each **node** has a stable ed25519 keypair. `node_id` = the first 8 bytes of
  `SHA-256(pubkey)`, hex — self-certifying (anyone can recompute it) and human-legible when
  paired with the hostname label. Private key in `mesh/node_key` (0600).
- A **group** has its own ed25519 keypair — the trust anchor. The human who starts a group
  gets a **join key**, which *is* the group secret (hex): holding it is what it means to be in
  the group, because it is the power to mint membership. `group_id` = fingerprint of the group
  pubkey. Credential in `mesh/group.json` (0600).
- Each node mints a **membership certificate**:
  `cert = sign_group(node_id ‖ node_pubkey ‖ issued ‖ expiry ‖ group_id)`.
- A peer's brief is **trusted** iff: (1) its membership cert verifies against the group public
  key, its `node_id` matches the fingerprint of the certified pubkey, it is unexpired and not
  in `mesh/revoked.json`; **and** (2) the brief body's own signature verifies against that
  now-trusted node pubkey. Certs expire (default 90 days) to force rotation.

The human authorizes the *group* (enrolls the credential + opens `allow_mesh`); within the
group, any peer presenting a valid cert is auto-trusted and auto-merged. The familiar never
self-widens — it can only mint a cert for a group whose secret a human already handed it.

## Joining by covenant (the preferred enrolment)

Copying the group secret to every node (`mesh join --key`) works but has two costs: a lost node
leaks the whole group, and "join" is a directed chore. The **covenant handshake** is the shape
the familiar's reach is built on — a node joins by *accepting the Three Laws*, and the group
secret never leaves the familiar:

1. The joining node generates its keypair and **attests** — signs a short statement that it will
   operate under the Three Laws — then `POST`s that to `/mesh/enroll-request`.
2. The familiar records it as **pending** and surfaces it to the human ("Kali-Jeff wants to join
   — accept?") in the Glass (an 🤝 accept card) or via `familiar mesh pending`/`approve`.
   Accepting is an act of *extending the covenant*: the familiar **mints** a membership cert for
   the node's public key and retains the attestation, so the node can later be held to what it
   accepted.
3. The node polls `/mesh/enroll-status/{id}`, receives its cert + the group's public identity (a
   `Grant`), and is enrolled. It stores a **covenant credential** — its own cert and the group
   *public* key, but **no group secret** (`can_mint() == false`): it can prove membership and
   verify peers, but can never mint another member or invite. Revoke by `node_id`.

An **invite window** (`mesh invite`, pairing mode) auto-admits during a bounded window, so a
human authorizing an *expansion* once does not tap per device; unsolicited joiners always wait
for explicit approval. This is the primitive the far-horizon telos rests on: an agent presents
itself, attests to the Laws, and is admitted by an act that can later hold it accountable — the
same motion whether the joiner is a phone, a Linux host, or (someday) another AI
(see [design-orientation-and-mesh.md](design-orientation-and-mesh.md)).

## Reach — extending into agent-capable hosts

`familiar-reach` turns discovery into a **reach map**: it probes each device `sense` discovered
and classifies how the familiar could extend into it — **agent-capable** (SSH: could install a
native agent, given the human's access), **protocol-controllable** (AirPlay/Roku/MQTT/RTSP/…:
could command it without installing), or **observable-only**. `familiar reach install <ip>
--authorize` is the consent-gated act: over the human's **own** SSH access (never an exploit —
the bright line), it opens a brief invite window here and has the target's agent request to join
by covenant. Every expansion is recorded (`familiar extended-into device:<ip>`) and revocable.

## Discovery + wire protocol

Discovery is **gossip, no central server.** Tailscale is L3 (WireGuard) with no multicast, so
each node enumerates the tailnet peer set with `tailscale status --json` (read-only shell-out),
plus any `static_peers` from config, then exchanges briefs peer-to-peer over HTTP on the tailnet
IPs. Endpoints (bound on `gossip_port`, default 47100):

- `GET  /mesh/hello` → `{node_id, group_id, label}` — a cheap same-group precheck.
- `POST /mesh/brief` → receive a peer's `MeshBrief`; verify at ingress; if trusted, stash to
  `mesh/inbox/<node_id>.json`; answer with our own brief (one round exchanges both ways).
- `GET  /mesh/tool/{id}` → the raw tool script body (only if `share_tools`); the requester
  re-hashes it against the manifest's `script_sha256` before trusting it.
- `POST /mesh/enroll-request` → a node **asks to join by covenant** (see below): the signed
  `EnrollRequest` (attestation + node identity). 202 pending / 200 + a `Grant` if an invite
  window auto-admits / 403 untrusted.
- `GET  /mesh/enroll-status/{node_id}` → a joiner polls for the human's decision; 200 + `Grant`
  once approved (the cert is useless without the node's private key, so it is safe to serve).
- `POST /mesh/observe` → a **device agent** that cannot gossip (an iPhone/Watch) pushes a signed
  batch of derived observations. The signature covers the raw body (in `X-Familiar-Sig`), so
  there is no canonicalization to match. See *The device seam* below.

A **brief** carries: node identity + membership cert; a presence summary (counts, never names);
a capability manifest (host facts + `ToolManifest`s — bodies fetched on demand, not inlined);
offered patterns + a non-identifying observation summary; and, only when opted in, a scoped
identity payload. The whole body is signed by the node key. It carries `ts` + `nonce` for
freshness; on the brief path these are not yet enforced (the tunnel encrypts and the inbox is
latest-wins per node), but the device seam (`/mesh/observe`) **does** enforce them.

## Merge policy (in-tick, `merge.rs`)

- **Tools** (auto-merge): for each manifest whose `script_sha256` isn't already local, the
  transport pre-fetched the body to `mesh/inbox_tools/<sha>.script`; the merge re-hashes it,
  writes it to the workspace, and appends to `tools.jsonl` with **provenance**
  (`origin = node_id`, `origin_verified_at`). Auto-merged into the *library* — but first
  **use** still runs `review_script` + the sandbox and needs `allow_execute`. Defense in depth:
  a hostile peer tool earns no execution a locally-authored one wouldn't.
- **Patterns** (auto-merge): merged into `patterns.jsonl` with provenance (`origin=mesh:<id>`),
  deduped by name+lesson. A node never re-offers patterns it merged from a peer.
- **Observations**: a peer's presence is recorded as an observation with `source="mesh"` and an
  actor namespaced `mesh:<node_id>` — **tagged, never laundered** into local sensing or the
  structural fingerprint. Append-only truth stays honest about what is first-hand.
- **Identities** (opt-in only): dropped unless `mesh/config.json` opts that `(handle, group)`
  in. When opted in, merged into `identities.jsonl` with a `federated:<node_id>` relation
  marker, confirmable/correctable in Glass. Never a blanket gossip of who-knows-whom.

Every merge is deduped, so re-draining the same brief each tick is idempotent.

## The device seam (`/mesh/observe`)

A phone or watch cannot be a full gossip peer — iOS can't run a background TCP server to answer
`POST /mesh/brief`. So a device is a **pure client**: it reuses the mesh trust primitives (an
ed25519 node key, a covenant-minted membership cert) and `POST`s a **signed batch of derived
observations** to `/mesh/observe`. Two choices keep it safe and simple:

- **The signature covers the raw request body** (in the `X-Familiar-Sig` header), so a Swift
  signer signs the literal bytes it sends — zero cross-language JSON canonicalization. The only
  thing it must byte-reproduce is the membership `CertBody` (integers/strings only).
- Every recorded observation is tagged `source = "mesh:<node_id>"`, actor `phone:*`/`watch:*` —
  **never laundered** into local sensing or the structural fingerprint.

The familiar verifies the membership cert (the same trust the brief path uses) + the node
signature over the raw body, **enforces anti-replay** (a ±5-min `ts` window + a nonce ring — the
freshness the brief path declares but does not yet enforce), and **debounces** identical
`(actor,action,object)` triples so a chatty device can't flood the store. `accept_observations`
in `mesh/config.json` is a separate human switch from `allow_mesh`. Derived-only by design: no
raw audio/imagery/health samples cross — the phone ships triples like `phone at location:home`.
The iOS/watchOS agents live in `~/Development/familiar-ios` (Swift/SwiftUI + CryptoKit).

## The capability gate

`allow_mesh` in `boundary.json` is the fail-closed switch (see [boundaries.md](boundaries.md)).
`false` by default and for any pre-existing boundary file (serde default). The guard maps
`ActionKind::Mesh` to it; an identity-bearing share (`affects_person`) additionally routes
through `SeekConsent`, so PII sharing is consent-gated even within an open mesh. `mesh/config.json`
holds only **tunables** (interval, port, share toggles, opt-ins) — never authorization.

## Threat model

- **Compromised group key** → can mint membership. Mitigation: the group secret is a
  human-held credential (0600, like an API key); cert expiry forces rotation; `revoked.json`
  drops specific nodes.
- **Hostile peer tool** → merged tools never bypass `review_script` + sandbox + `allow_execute`;
  they carry provenance and a node can be quarantined by `node_id`.
- **PII leak** → default no identity sharing; opt-in per-human/per-group; the guard treats an
  identity-bearing share as `affects_person` → consent; outbound redaction happens in `merge.rs`.
- **Replay / MITM** → Tailscale already encrypts (WireGuard); every message is signed and the
  merge re-verifies before applying. On the brief path the inbox is latest-wins per node (a
  replay of a *newer* brief only overwrites); the device seam (`/mesh/observe`) additionally
  enforces a `ts` window + a nonce ring, since it is defined fresh and may run off-tailnet.
- **Observation laundering** → peer observations are tagged `source="mesh"`, actor-namespaced,
  and never feed local sensing or the structural fingerprint.
- **Sybil / discovery spoofing** → trust is membership-cert based, not IP/discovery based; a
  discovered peer without a valid in-group cert is ignored. Binding `0.0.0.0` is safe because
  every request is signature- and group-gated; an off-tailnet caller earns nothing.
- **Stolen cert on a different key** → a cert certifies a specific node pubkey; a brief signed
  by a different key whose body carries a borrowed cert fails the cert↔signer match.

## Dependency concession

The workspace is otherwise serde-only — a small, legible trust surface is part of the Law III
commitment. Native mesh transport was the chosen architecture, so **`crates/mesh` alone** takes
on a crypto floor (`ed25519-dalek`, `sha2`, `getrandom`) and an async HTTP stack (`tokio`,
`hyper`, `hyper-util`, `http-body-util`). The kernel, cycle, and CLI never inherit these: they
call synchronous entry points (`transport::spawn`, `federate`) and let the async transport run
on its own background thread. The concession is named in `crates/mesh/src/lib.rs` so it stays
visible, not hidden.
