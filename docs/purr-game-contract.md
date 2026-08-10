# Purr Game Contract, v1

The integration contract between a game client and **Purr** — a ship's-computer persona
of the familiar. This is the document handed to the game team; the decision record
behind it is [ADR-0037](decision-records/0037-one-soul-many-voices.md).

**Contract version: 1 (draft — not yet implemented; shapes here are the spec the
implementation will be held to).**

The model in one paragraph: the game owns all simulation and rendering — mining (water
and minerals), construction, trade. Purr runs beside the game (macOS) or inside it
(iOS), watches the ship's telemetry, answers the captain in character, and volunteers
speech when events warrant. Purr's output is text utterances, which the game client
voices, plus — once paired — **commands** the game validates. A fresh Purr is unaware:
it knows nothing of ship, crew, or cargo until the captain performs the **pairing**
(§2), naming it ("Purr" is the root name; each captain chooses their own) and handing
it the ship's design spec, the crew dossier, and the state of stores. From then on Purr
co-controls exactly the systems the spec declared, with the captain and authorized
crew. The **autonomy grant** (§8), purchased or earned in-game, adds initiative: Purr
may originate trade actions itself.

---

## 1. Transport and bindings

One contract, two bindings, identical JSON shapes:

| Platform | Binding | Detail |
|---|---|---|
| macOS | Loopback HTTP | The Purr daemon listens on `127.0.0.1:<port>` (its mesh gossip port + 1; default `47101`). The game discovers the port from the daemon's data dir (`purr_port` file) or is configured with it. |
| iOS | Embedded (UniFFI) | No sidecar processes on iOS. The game app links `familiar-core` and calls Swift functions with the same names and JSON payloads as the HTTP paths (`purr_hello() -> String`, `purr_chat(json) -> String`, …). |

Everything below is written as HTTP; on iOS, read each route as a function call with the
request body as its argument and the response body as its return.

**Auth (macOS binding — recommended, confirm with game team):** the daemon writes a
random session token to `<data-dir>/purr_token` (mode 0600) at startup. The game reads
it and sends `X-Purr-Token` on every request. Loopback alone is not a trust boundary —
every local process shares it; a same-user file read is.

**Errors:** any failure returns a non-2xx status (HTTP) and the shape

```json
{ "error": { "code": "bad_session", "message": "human-readable detail" } }
```

Codes v1: `bad_token`, `bad_session`, `bad_request`, `unavailable`.

---

## 2. Handshake and pairing

### `GET /local/purr/hello`

No body. Always answers, even before a session exists.

```json
{
  "contract_version": 1,
  "persona": { "name": "Purr" },
  "node_id": "9f31c2ab04d67e51",
  "llm": { "available": true, "provider": "gemini" },
  "seq_head": 41,
  "capabilities": ["chat", "telemetry", "utterances"]
}
```

`persona.name` is the computer's current name — `"Purr"` until the captain names it at
pairing, then whatever the captain chose. `llm.available` is live status, not
configuration — poll `hello` to detect degraded mode (§9). `seq_head` is the newest
utterance sequence number, for resuming a cursor. `capabilities` reflects the grant
ladder: `"commands"` appears once paired (§7), `"autonomy"` while the autonomy grant
(§8) is active. An unpaired Purr answers `hello` but knows nothing and controls
nothing.

### `POST /local/purr/pair` — the commissioning

A fresh Purr is unaware — no ship, no crew, no name but the root one. Pairing is the
one ceremony that changes that. In the game's fiction, machines progressed
**hand → keeper → otherpaw → familiar**, and when they became sentient companions, cats
began giving them individual names — a name is what you give a *someone*. When a cat
takes command of a ship, the captain names the ship's computer and hands it the ship:

```json
{
  "name": "Mouse",
  "captain": { "callsign": "Whisker", "human_name": "Ian" },
  "ship": {
    "name": "Longtail",
    "class": "medium freighter",
    "spec": {
      "description": "freeform description of the vessel and its features",
      "systems": [
        { "id": "drive", "kind": "propulsion",
          "description": "twin ion drives, 0.3g sustained",
          "controls": [ { "action": "set_route", "params": ["destination"] } ] },
        { "id": "cargo_bay", "kind": "cargo", "capacity": 120,
          "controls": [ { "action": "seal", "params": [], "reverse": "unseal" },
                        { "action": "unseal", "params": [], "reverse": "seal" } ] },
        { "id": "market_link", "kind": "trade",
          "controls": [ { "action": "buy",  "params": ["commodity", "qty", "max_price"], "reverse": "sell" },
                        { "action": "sell", "params": ["commodity", "qty", "min_price"], "reverse": "buy" },
                        { "action": "send_offer",   "params": ["counterparty", "commodity", "qty", "price"] },
                        { "action": "accept_offer", "params": ["offer_id"] },
                        { "action": "reject_offer", "params": ["offer_id"] } ] }
      ],
      "reflexes": [
        { "on": "damage:hull:critical", "system": "cargo_bay", "action": "seal" }
      ]
    }
  },
  "crew": [
    { "callsign": "Whisker", "role": "captain",  "authority": "captain", "human_name": "Ian" },
    { "callsign": "Patch",   "role": "engineer", "authority": "officer" },
    { "callsign": "Bramble", "role": "deckhand", "authority": "crew" }
  ],
  "status": {
    "fuel_fraction": 0.82,
    "credits": 900,
    "cargo": [ { "commodity": "water", "qty": 12 } ],
    "supplies": [ { "item": "rations", "qty": 40 } ]
  }
}
```

→ `200 { "paired": true, "name": "Mouse" }`

What each part becomes, on Purr's side:

- **`ship.spec`** is the grant of co-control. `systems[].controls` is the complete list
  of actions Purr may ever command (§7) — what the spec omits, Purr can see but never
  touch. `reflexes` are deterministic standing orders the spec itself declares (they
  run with no language model, like alerts). The spec text also becomes reference
  knowledge: ask Purr about the ship and it answers from the manual the captain gave it.
- **`crew`** becomes Purr's dossier of who it serves. `authority`
  (`"captain" | "officer" | "crew" | "guest"`) governs whose orders move which systems —
  the game enforces it on execution, Purr respects it in proposal. `human_name` appears
  only on entries backed by a real person at the controls; consent, identity, and spend
  accounting bind to the human, never to the fiction.
- **`status`** seeds Purr's opening picture of the ship; telemetry (§3) keeps it honest
  from then on.

Pairing happens once. Call it again to **refit** (updated spec), **re-crew** (updated
roster), or correct the record — same shape, new truth; `name` is optional after the
first call. Everything else Purr knows — routes flown, markets seen, the crew's habits —
it learns by being aboard, not by upload.

### `POST /local/purr/session`

Per-launch reconnect. Identifies who is at the controls this session (any crew member;
defaults to the captain):

```json
{ "callsign": "Whisker" }
```

→ `200 { "session": "s-8c04…" }`

All subsequent `telemetry` and `chat` calls carry this `session`. An unknown or stale
session → `bad_session`; re-handshake. Calling before ever pairing → `bad_request`.

### `POST /local/purr/name` — renaming

```json
{ "session": "s-8c04…", "name": "Mouse" }
```

→ `200 { "name": "Mouse" }`

The initial name is given at pairing; this call renames. The name persists in the
instance's persona (it survives restarts and reinstalls of the game — it is Purr's, not
the save file's) and is reported by `hello`. Renaming is allowed — it's the captain's
computer — but the game should treat it as ceremony, not a settings toggle. The name is
the **only** persona field the player can write, and only the captain may write it.

---

## 3. Telemetry

### `POST /local/purr/telemetry`

Batched, **edge-triggered** events. Send an event when something *happens or changes
state* — never per-frame, never continuous streams. Plus one `status_snapshot` every
30–60 s for ambient context.

```json
{
  "session": "s-8c04…",
  "events": [
    { "t": 1786500000, "type": "low_consumable", "data": { "resource": "fuel", "fraction": 0.14 } },
    { "t": 1786500004, "type": "waypoint_set",  "data": { "destination": "Meridian Station" } }
  ]
}
```

→ `200 { "accepted": 2, "seq": 41 }`

`t` is Unix seconds (game may send its sim-time separately inside `data` if it diverges).
`seq` is the current utterance head, so a telemetry response doubles as a poll hint.

---

## 4. Event vocabulary, v1

Required `data` fields per type. Unknown `type` values are **accepted and logged**, never
rejected — both sides must tolerate vocabulary growth.

| type | required `data` | notes |
|---|---|---|
| `departure` | `from` | leaving a station/body |
| `arrival` | `at` | arriving at a station/body |
| `docking` | `at` | docking complete |
| `undocking` | `from` | |
| `low_consumable` | `resource`, `fraction` | send once per threshold crossing (e.g. 0.25, 0.10), not continuously |
| `consumable_restored` | `resource`, `fraction` | clears a prior low state |
| `market_delta` | `station`, `commodity`, `delta_pct` | notable price movement the game deems visible to the player |
| `hail_received` | `from_ship`, `text` (optional) | another ship hails |
| `damage` | `system`, `severity` (`"minor"\|"major"\|"critical"`) | |
| `cargo_change` | `commodity`, `delta` | |
| `credits_change` | `delta`, `balance` | |
| `waypoint_set` | `destination` | |
| `combat_start` | — | |
| `combat_end` | `outcome` (optional) | |
| `mining_start` | `site`, `resource` (`"water"` \| mineral name) | |
| `mining_yield` | `site`, `resource`, `amount` | on completion or notable haul, not per tick |
| `construction_progress` | `site`, `structure`, `fraction` | edge-triggered at milestones (0.25/0.5/0.75/1.0) |
| `trade_offer` | `from`, `commodity`, `qty`, `price` | an offer arrived (station or ship) |
| `trade_completed` | `commodity`, `qty`, `price`, `counterparty` | |
| `crew_change` | `callsign`, `change` (`"joined"\|"left"`), `authority` (on join) | light roster change; full re-crew goes through `pair` |
| `command_result` | `command_id`, `ok`, `detail` (optional) | outcome of a command (§7) — how Purr learns what the world actually did |
| `status_snapshot` | `position`, `velocity`, `fuel_fraction`, `hull_fraction`, `credits`, `cargo` (freeform object) | ambient context, ≤ 1 per 30 s |

---

## 5. Chat

### `POST /local/purr/chat`

```json
{ "session": "s-8c04…", "text": "Purr, how far to Meridian at current burn?" }
```

→ `202 { "message_id": "m-1a2b…" }`

**Chat is asynchronous.** The reply arrives on the utterance channel (§6) with
`reply_to: "m-1a2b…"`. Reply latency is normally seconds but is unbounded when the
language model is remote or queued; the degraded-mode guarantee (§9) bounds the worst
case. Do not block gameplay on a reply.

---

## 6. Utterances

### `GET /local/purr/utterances?after=<seq>&wait=<ms>`

The single delivery channel for everything Purr says: chat replies, deterministic
alerts, ambient remarks, relayed hails. Cursor-based: pass the last `seq` you processed
(`after=0` for a fresh session — or `seq_head` from `hello` to skip history). Optional
`wait` long-polls up to that many ms when nothing is pending. A 1 Hz poll is fine.

```json
{
  "seq_head": 44,
  "utterances": [
    {
      "seq": 42, "t": 1786500001,
      "kind": "alert",
      "text": "Fuel at fourteen percent, Captain. We land at Meridian on fumes and hope.",
      "speech": { "priority": 1, "interruptible": false },
      "source_event": "low_consumable",
      "expires": 1786500600
    },
    {
      "seq": 43, "t": 1786500008,
      "kind": "reply", "reply_to": "m-1a2b…",
      "text": "Six hours at current burn. Four if you let me plot the gravity assist.",
      "speech": { "priority": 2, "interruptible": true }
    },
    {
      "seq": 44, "t": 1786500030,
      "kind": "ambient",
      "text": "Meridian's grain price is up nine percent. Just saying.",
      "speech": { "priority": 3, "interruptible": true },
      "source_event": "market_delta",
      "expires": 1786502000
    }
  ]
}
```

Fields: `kind` ∈ `reply | ambient | alert | hail`; `reply_to` only on `reply`;
`source_event` names the triggering telemetry type when there is one; `expires` (Unix
seconds, optional) marks speech that goes stale.

**Delivery rules for the client:**

- `alert` (priority 1) — must be voiced/shown; not interruptible unless flagged.
- `reply` (priority 2) — should be voiced; it answers the player.
- `ambient` / `hail` (priority 3) — may be dropped under load; never voice after
  `expires`.

**Voice is the client's.** Purr sends text plus the `speech` hints; the game does TTS
(and STT for input) with its own audio session — ducking, cockpit positioning,
interruption on explosions. Apple Speech runs on-device on both platforms.

---

## 7. Co-control and commands

Once paired, Purr holds **co-control** of the systems the spec declared, jointly with
the captain and authorized crew. Co-control is exercised through the command channel:

### `GET /local/purr/commands?after=<seq>&wait=<ms>`

Same cursor mechanics as utterances, opposite direction. Empty until paired.

```json
{
  "seq_head": 12,
  "commands": [
    { "seq": 11, "command_id": "c-76f0…", "t": 1786500042,
      "system": "cargo_bay", "action": "seal", "params": {},
      "origin": "ordered", "ordered_by": "Whisker",
      "reason": "Captain's order: seal the bay before undock." },
    { "seq": 12, "command_id": "c-77aa…", "t": 1786500100,
      "system": "market_link", "action": "buy",
      "params": { "commodity": "water", "qty": 40, "max_price": 12 },
      "origin": "auto",
      "reason": "Meridian sells water at 9; Harrow's Rest buys at 15. Margin holds after fuel." }
  ]
}
```

Every command names its `system` and `action` — always ones the pairing spec declared;
Purr can never command what the captain didn't hand it. `origin` says on whose
initiative:

- `"ordered"` — carrying out an order from the captain or authorized crew
  (`ordered_by` names them; authority comes from the crew dossier, and the game
  enforces it on execution just as Purr respects it in proposal).
- `"reflex"` — a deterministic standing order the spec's `reflexes` declared (hull
  breach → seal the bay). No language model in the path; reflexes fire like alerts.
- `"auto"` — Purr's own initiative. **Only ever present under the autonomy grant (§8).**

**Purr proposes; the simulation disposes.** The game validates every command against its
own rules — fuel, funds, docking rights, physics — executes or refuses it, and reports
the outcome as a `command_result` telemetry event carrying the `command_id`. Purr never
assumes a command succeeded; the result event is the only truth. Every command carries a
`reason` the game may surface — the captain is always entitled to ask why.

**The captain outranks the computer, always.** Any explicit captain action that
contradicts a pending command voids it; a chat message like "belay that" should be
relayed as chat, and Purr will stand down and say so.

---

## 8. Autonomy: the trade-running grant

An optional tier the game sells as an in-app purchase **or** lets a captain earn through
accumulated in-game wealth: a Purr that autonomously runs trade routes, buys and sells,
and negotiates. The constitutional shape (see ADR-0037 §6): autonomy is an **explicit,
scoped, bounded, revocable grant** — never a default, never a drift. It adds no new
systems — the pairing spec already declared everything Purr may touch — it adds
**initiative**: commands with `origin: "auto"`.

### `POST /local/purr/autonomy`

Grant, adjust, or revoke:

```json
{
  "session": "s-8c04…",
  "enabled": true,
  "scope": { "trade": true, "routes": true, "negotiation": true },
  "budget": { "credits_max": 50000, "range": "within 2 jumps" }
}
```

→ `200 { "autonomy": "granted" }` (or `"revoked"`)

`scope` limits which declared systems initiative may reach; `budget` bounds it.
`enabled: false` revokes instantly and totally; in-flight `auto` commands are void
(ordered and reflex commands are untouched — revoking the autopilot doesn't disable the
crew). The game enforces the purchase/wealth gate before making this call — entitlement
is the game's business; Purr only honors the grant it's given. Only the captain's
session may grant or revoke.

---

## 9. Degradation contract

`hello.llm.available` may be `false` at any time (offline, budget exhausted, provider
cooldown). Purr's hard guarantees without any language model:

1. **Telemetry is always accepted** and recorded.
2. **Alerts always fire** — they are deterministic, persona-templated at ingestion, no
   model in the path. The ship's computer warns you about fuel without asking anyone's
   opinion.
3. **Chat always answers within 2 s**, worst case with a canned in-persona
   acknowledgment carrying `"degraded": true` on the reply utterance ("My deep thinking
   is offline, Captain. Instruments still work."). When the model returns, a full reply
   to the same `message_id` MAY follow.
4. **Ambient utterances simply stop.** No fakes.
5. **Reflexes keep working.** The spec's standing orders are deterministic — hull
   breach still seals the bay with no model anywhere.
6. **Autonomy stands down safely.** With the model gone, no new `auto` commands are
   issued — Purr emits an `alert` that the autopilot is standing down, and
   already-validated route-following remains the game's to finish. Structured orders
   from the game UI still flow (`ordered` commands are deterministic relays);
   natural-language orders degrade with chat.

---

## 10. Versioning

- `contract_version` is an integer, returned by `hello`. Within a major version, changes
  are **additive only** — new event types, new optional fields, new utterance kinds.
- Both sides MUST ignore unknown fields and unknown enum values (the client treats an
  unknown `kind` as `ambient`; Purr logs unknown event types; the game refuses unknown
  command `action`s with a `command_result` of `ok: false`).
- Breaking changes increment the major; the client refuses a major it doesn't know.

---

## 11. Open questions for the game team

1. **Daemon lifecycle (macOS).** Who launches Purr — does the game bundle the `familiar`
   binary and spawn it with `--data-dir` in the game's container, or require a separate
   install (launchd)? Our recommendation: bundle and spawn; the data dir belongs to the
   player, not the game version.
2. **iOS embedding.** Who calls the metabolism tick (game loop timer vs background
   task)? Data dir in the app's sandbox container — and what backs it up? Losing the
   device currently means losing that Purr's memory and keys; is that acceptable
   fiction ("the ship's computer went down with the ship") or do we need export?
3. **Auth.** Confirm the token-file scheme (§1), or propose the platform-native
   alternative you prefer.
4. **Latency and cadence.** What reply-latency budget makes conversation feel right in
   your cockpit, and what poll cadence do you want to run? (Shapes the `wait` default.)
5. **Content responsibility.** Purr's flavored lines come from a language model under
   our constitution and spend controls, but ratings are the game's. What rating are you
   targeting, and do you need a content contract beyond our existing guarantees?
6. **The spec schema.** §2's `ship.spec`, `crew`, and `status` shapes are our opening
   proposal — you own ship design, so the schema should be co-designed against your
   actual data model. How detailed do specs get (per-system tonnage, power draw,
   damage states?), and how are refits represented in your game so `pair` re-calls
   stay cheap? Also: how do structured orders (a "seal cargo bay" button vs. spoken
   orders) reach Purr — via chat, or a dedicated order event?
7. **Autonomy economics.** Autonomous negotiation spends real language-model tokens
   after a one-time purchase (or in-game earn). Who carries that marginal cost — the
   game (server-side keys), the player (their own provider/local model), or a local-model
   floor with a cloud upgrade? Our spend ledger caps it; your commercial model must own
   it. Also: what does the grant/revoke ceremony look like in your UI?
8. **Fleet phase (later).** Ship-to-ship Purr features (shared lore, cross-crew games)
   arrive in a second phase over mesh federation. Multiplayer hail *text* stays on your
   netcode regardless. Who would host fleet "station beacons" (lighthouses), and does
   your NAT/hosting model allow player-reachable rendezvous?

---

*Familiar side of the contract: routes land in `crates/mesh/src/transport.rs` (loopback
listener), the telemetry adapter and utterance outbox in `crates/kernel` +
`crates/cycle`, the iOS functions in `crates/core-ffi`. See ADR-0037's follow-on work
for sequencing.*
