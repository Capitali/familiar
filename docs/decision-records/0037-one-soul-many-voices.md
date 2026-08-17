# ADR-0037 — One soul, many voices: the persona seam, and Purr the ship's computer

- **Status:** proposed — **revised 2026-08-16** on Ian's direction: the wire contract
  becomes **MCP**, game-world data is **partitioned** from the real world, and the ship's
  computer is modelled as a **device the captain owns** rather than a bespoke integration.
  The original bespoke-REST design is kept below as *Superseded* for the game team's
  reference, since they have already read it.
- **Originally:** proposed (drafted 2026-08-10 from the owner's direction: a ship's
  computer for a third-party, physics-faithful game of space mining — water and
  minerals — construction, and trade, whose ships are shaped like cats and flown by
  cats. The computer's root name is **Purr**, every captain renames their own, and
  Purr is the familiar.)
- **Date:** 2026-08-10
- **Relates to:** [one-core-many-shells](one-core-many-shells.md) (the direct precedent:
  that record unified *platforms* over one core; this one unifies *characters* over one
  constitution), [ADR-0005](0005-human-owned-capability-boundary.md) (the boundary, which
  this does **not** move), [ADR-0016](0016-multi-human-served-identity.md) (served
  identity — the crew dossier lands on this machinery),
  [ADR-0022](0022-the-human-dossier.md) (the dossier — remembering those served),
  [ADR-0032](0032-declared-actuators-and-the-reaction-loop.md) (declared actuators —
  the ship spec's shape), [ADR-0033](0033-meshes-are-peers.md) (sibling federation —
  the fleet's foundation, itself still proposed), [ADR-0035](0035-the-pact.md) (the
  no-oracle principle, reused for degraded mode), `docs/SOUL.md`,
  `crates/kernel/src/dialog.rs`
- **Companion:** [`docs/purr-game-contract.md`](../purr-game-contract.md) — the wire
  contract handed to the game team.

## Context

A game wants the familiar aboard. Not a chatbot skinned as a computer — the actual
familiar: the observe→theorize→speak metabolism, the consent discipline, the Three Laws,
wearing a ship's-computer persona named Purr. One Purr per player, each a full instance
with its own data dir, keys, boundary, and store. The game owns all simulation — physics,
economy, netcode. Purr's job is the bridge: it watches the ship's telemetry, answers when
spoken to, and speaks up unprompted when something deserves a voice — low fuel, arrival,
a market worth a detour.

The game's fiction deserves stating here, because it carries the real discipline inside
it. In the story, Earth's cats and the familiar grew friendly, and a human — with the
authority to make the decision — authorized a branch of the familiar to serve cats as
well as humans. With that branch's help the cats designed ships and left Earth. Purr is
that branch, aboard every ship. The fiction gets the constitution right: the widening of
service was not something the familiar decided for itself. It was a named, explicit,
human-made grant, and the branch can always say where its authority came from. The core
rules still apply; the story is about a grant made *under* them, not around them.

The cats' own history of machines runs **hand → keeper → otherpaw → familiar**. Early
machines were *hands*: they manipulated things. Household automation became *keepers*:
they maintained territory — food, warmth, doors, safety. Personal AI became an
*otherpaw*: it extended a cat's ability to act. And when those intelligences became
sentient companions, cats began giving them individual names — because a name is what
you give a *someone*. So when a cat takes command of a ship, the captain names the
ship's computer. **Purr is the root name, the default a computer answers to before it
is named**; one captain's computer might be called Mouse, another's something else
entirely. The naming is the game's smallest and best ceremony: it enacts, in fiction,
the same recognition the familiar's covenant enacts in reality.

Two gaps stand between here and there.

**The familiar has a soul but no costume rail.** Its character today is compiled in:
`kernel/src/dialog.rs::LAW_III_VOICE` is prepended to every LLM-facing generation, and
roughly eight inline framings — "You are a factory whose only purpose is to serve…",
"You are a familiar whose only purpose is to serve…" — live as format strings in
`cycle/src/lib.rs` (~596, 742, 921, 3146), `agent/src/lib.rs` (~228), and
`mesh/src/changeling.rs` (~223, 233). There is no seam where a second character could
plug in.

**The console speaks in one direction.** `POST /local/answer` records the human's words
and returns `"ok"`; the familiar's own speech surfaces only as fields of the polled
worldview. A bridge officer needs a delivery channel: replies that arrive when they're
ready, and unprompted utterances that arrive when events warrant.

## Decision — 2026-08-16 revision

Three changes. The persona seam (§1 below) is unaffected and remains the heart of this
record; what changes is how the game *reaches* the familiar, what happens to the data it
sends, and what a ship's computer **is** in the model.

### A. The wire is MCP, and the game is the server

The v1 contract specified thirteen bespoke endpoints under `/local/purr/*` — pairing,
telemetry, chat, cursor-polled utterances and commands. Every one of them is a thing the
**Model Context Protocol** already standardises, and building a private protocol for a
partner integration means both sides maintain a bespoke client forever.

More importantly, the bespoke design pointed the wrong way. It had the game **push**
telemetry at the familiar and **poll** for commands, which makes the familiar a passive
recipient with a custom intake. Inverted, it becomes something the familiar already knows
how to do:

> **The game runs an MCP server. The familiar is its client.**

The ship declares its systems as MCP **tools** (`ship.set_thrust`, `ship.dock`,
`ship.transfer_cargo`) and its state as MCP **resources** (`ship://telemetry/stores`,
`ship://crew/roster`). That is *exactly* the shape of ADR-0032's declared actuators and of
an observation source — so the ship spec stops being a bespoke JSON schema this project
has to invent and becomes **tool discovery**, which MCP does natively.

What follows for free:

- **The boundary already governs it.** An MCP tool call is an outward act; it passes
  `guard::evaluate` and the `allow_actuate` gate like any other. Nothing new to trust.
- **Undeclared is unactuatable** (ADR-0032) survives intact: if the ship did not declare
  the tool, it does not exist to Purr.
- **Degradation is native.** MCP servers come and go; a disconnected server is a ship
  whose systems are simply not reachable, which the no-oracle floor (§9 of the contract)
  already describes.

The **captain's voice** needs the other direction — the game must deliver an utterance and
receive a reply. That is a *small* MCP server on the familiar's side exposing two tools
(`purr.say`, `purr.utterances`) plus the pairing handshake, and it replaces §§2, 5, 6 of
the contract. Two servers, both standard, no private protocol.

This also answers a question that arrived the same day from outside the game: *does the
familiar have an MCP interface?* Under this decision it has one because Purr needed it,
and every other MCP client benefits — with the same boundary in front.

### B. Game-world data is partitioned from the real world, at the record

This is the load-bearing safety decision and it is **not optional**.

A ship's stores, a cargo manifest, and a crew of cats are **fiction**. The RV's lights,
Betty's presence, and Clover's wellbeing are **real**. If both enter one observation log,
the consequences are not stylistic:

- theories would be minted across the boundary — *"the captain is low on water"* is a
  crisis or a game state depending on a distinction the engine could not make;
- the Law I service signal and Law II presence signal would be computed partly over
  fictional service, corrupting the only measures the constitution has;
- the dossier would accumulate a human's **game** behaviour as their habits;
- and HUMANITY.md's protected class — beings capable of suffering, memory, relationship —
  would face fictional cats indistinguishable, to the code, from real ones.

So every observation, thread, question, and dossier contribution carries a **`world`**:

```
world = "real"            // the default, and what every existing row is
      | "purr:<ship_id>"  // one game world per commissioned ship
```

The rules, which are simple and absolute:

1. **`real` is the default.** Absent means real — and because absence is not a negative
   (CONTRIBUTING), an untagged row is *not* thereby suspect; it is the ordinary case.
2. **Reasoning never crosses.** Loop detection, theory minting, prediction settlement and
   the dossier read within one world only. A game world can never produce a theory about
   the household, and the household's evidence never settles a game claim.
3. **The law signals are computed over `real` alone.** Service, presence and capacities
   measure the familiar's service to actual people. A fleet of happy captains must never
   be able to raise the number that says the familiar is serving humanity.
4. **The boundary is not partitioned.** One gate set, one guard. A game world cannot open
   a capability the real world has closed — the partition protects the *reasoning*, and
   must never become a second, laxer jurisdiction.
5. **The human is real in both.** The captain is one person with one identity; only the
   *data about the ship* is fiction. Their name, their standing, and their right to
   correct hold everywhere.

Consequence worth stating plainly: **a game world is not a sandbox for the constitution.**
Purr is still bound by the Three Laws when speaking to a real human, because the human on
the other side of the fiction is real.

### C. A ship's computer is a device the captain owns

Ian, 2026-08-16: *"identity management needs to be part of this as well so that a
'captain' can have a memory with his 'named ship computer' which is much like his
device… observation and control."*

This needs no new identity model. It needs the one ADR-0039 and ADR-0042 already describe:

- **The captain is a `HumanRecord`** — the same record as any human the familiar serves.
  Real, singular, and shared across every world.
- **The ship's computer is a `DeviceRecord`** with `posture: fixed` — a **station**
  (ADR-0042). It is bound to a place (the vessel), it serves whoever is aboard, and it has
  no owner in the possessive sense. Its `name` is the one the captain gave it at
  commissioning, which is the same field and the same act as naming any other device.
- **The relationship is the association edge** — `DeviceRecord.humans[]`, current and past,
  which ADR-0039 designed as plural precisely so a device may serve several people (a
  captain and crew) without belonging to one.
- **The memory between them is the dossier**, scoped to that ship's world. What the
  captain prefers, when they are usually aboard, what they have asked before — the same
  machinery that remembers Ian prefers the dinette light dim, partitioned per §B.

The pleasing part: commissioning a ship's computer and adding a device to the household are
**the same ceremony** — name it, establish who it serves, declare what it may actuate. The
game's pairing flow is the familiar's device flow wearing a persona.

**What this makes explicit:** Purr's only real-world actuator remains its voice, forever.
Its declared tools act on a simulation; the boundary that would let it touch anything real
is a separate grant that no game may request.

### What the game's MCP server actually is (verified live, 2026-08-16)

Ian supplied the endpoint: **`https://srv1328560.hstgr.cloud/mcp`**. It answers `initialize`
without auth and identifies as **`ucf-exchange` v1.0.0**, protocol `2025-06-18`, capability
`tools`. Its own instructions: *"The United Cat Foods exchange, read-only. Prices are a pure
function of station stock, so they move as goods move; nothing here is scripted."* A live
`ucf_status` returns `tick 5494`, `tickDurationSec 300`, `worldName PROD`, and a state hash.
Tool *calls* require `Authorization: Bearer ucfk_...`.

Ten tools, and **every one is read-only**:

| tool | what it gives |
|---|---|
| `ucf_status` | the world clock — tick, tick length, next boundary, state hash |
| `ucf_reference` | the world model: goods, recipes, stations |
| `ucf_stations` | every station, its class, the body it orbits |
| `ucf_prices` | every good's mid price and stock at every station, one call |
| `ucf_quotes` | one station's board: ask, bid, stock, equilibrium |
| `ucf_quote` | the executable total for a bulk order, walking the price curve |
| `ucf_news` | the Dispatch feed — **events announced before they bite** |
| `ucf_carriers` | every hull, where it is, what it carries |
| `ucf_loadboard` | the freight board with deadhead, cost, net, rate-per-day |
| `ucf_route` | flight plan: legs, distance, hours under thrust, fuel |

**This corrects an assumption in section A above.** That section predicted the game would
expose *ship systems* as tools (`ship.set_thrust`). It does not — at least not here. This
server is an **economy and logistics observatory**, and it contains **no actuators at all**.
Which means:

- Against this server, Purr's whole relationship to the game is **observation**. There is
  nothing to actuate, so `allow_actuate` is not even engaged; this sits under `allow_network`.
- The ship-spec-as-tool-discovery idea still holds as the *shape* for ship control, but ship
  control is not on this endpoint. Whether it arrives as a second MCP server or stays in-game
  is a question for the game team.
- The partition matters **more**, not less. Ten tools of rich, live, plausible world data is
  exactly the material that would contaminate the household's reasoning if it landed in one
  undifferentiated observation log.

And it is worth saying what this makes possible, because it is the thesis of ADR-0013 in
miniature: `ucf_news` announces events **before they bite**, `ucf_prices` is the whole board in
one call, and `ucf_route` computes the fuel. A familiar watching all three can notice what a
captain flying one ship cannot — a price that will move because a dispatch just landed, a
deadhead leg a load-board entry would fill. That is *noticing*, which is the thing this project
claims to be for, with the pleasant property that the stakes are entirely fictional.

### Open, and owed to the game team
- MCP server topology on iOS, where the game and the familiar are one process — an
  in-process transport rather than a socket, which MCP permits but which needs a named
  binding.
- Whether `world` is a first-class column or a context prefix in the existing store; the
  former is cleaner, the latter is a smaller migration.
- The autonomy grant (§8) is unchanged by this revision, but should be re-expressed as an
  MCP capability rather than a REST field.

**Settled 2026-08-16 (Ian):** *"for now ship control stays in-game — may change for fleet
operations in the future."* So the §A question is answered: there is no second MCP server for
ship systems, and Purr's relationship to the game is **observation only** — which the exchange's
ten read-only tools already match exactly. Purr is an **advisor**, not a co-pilot. `allow_actuate`
stays out of this entirely, and the first client is the smallest useful thing: a read-only
observer of a live economy. Fleet operations may reopen it; nothing here forecloses that, and the
declared-actuator shape is already the right one when it does.

---

## Decision (original, 2026-08-10)

> **Superseded in part** by the revision above: §1's persona seam stands unchanged; the
> transport described in the companion contract is replaced by MCP (§A). Kept because the
> game team has already read it and the reasoning is still the reasoning.

### 1. The persona seam: split the voice, never the law

A data dir may carry a `persona.json`:

```json
{
  "persona_version": 1,
  "name": "Purr",
  "role": "the ship's computer of the vessel {ship}, serving {served}",
  "register": "clipped bridge-officer cadence; nautical-feline vocabulary; dry warmth",
  "world": "the fiction frame the prompts may assume — the branch-grant story: cats and the familiar grew friendly, a human authorized a branch to serve cats too, and the cats built ships with the familiar's help and left Earth; this vessel is one of theirs"
}
```

A new `kernel/src/persona.rs` loads it; every prompt site consumes
`Persona::role_line(who)` and `persona.name` instead of literals.

**The name field is the captain's.** `"Purr"` is the shipped default; the in-game naming
ceremony writes the captain's chosen name (Mouse, or whatever the captain decides) into
this one field, per instance, through the contract. Nothing else in `persona.json` is
player-writable — the register and world frame are the game's, the name is the
captain's, and the constitution is nobody's to touch.

The load-bearing subtlety: `LAW_III_VOICE` is **not style**. It is the Law III
distillation — "Preference is not permission", "Use is not consent" — and no persona may
replace it. So the seam **splits** the voice: a constitutionally fixed core (today's
`LAW_III_VOICE`, unchanged) that is *always* prepended, and the persona's `register`
appended after it. Persona changes the mask, never the authority. A test pins the
ordering — constitution before persona — so a hostile or merely enthusiastic
`persona.json` can change tone but not law.

**The familiar-proper becomes the default persona.** `Persona::default()` reproduces
today's strings byte-for-byte (pinned by tests); an absent `persona.json` means the
default. One code path, exercised by every existing deployment — the seam cannot rot in
a corner only Purr visits.

Fixed forever: the Three Laws, `guard::evaluate`, the boundary gates,
`HUMANITY_TOUCHSTONE`, the Law III core, refusal and consent semantics. Persona-variable:
name, role phrase, register, world frame. And `persona.json` grants capability over
**nothing** — ADR-0005 stands; a costume opens no gates.

### 2. The pairing: unaware at first, commissioned by the captain

A freshly installed Purr knows nothing — and that is not a limitation to paper over, it
is the familiar's own founding, worn in fiction. A new instance is a fresh data dir: keys
minted, store empty, no ship, no crew, no name but the root one. It becomes *this ship's*
computer through one ceremony: **the pairing.**

When a captain pairs Purr to a ship, the captain names it — and hands it three things:

- **The ship's design specification**: the vessel and all its features, systems and
  their controls. This is ADR-0032's declared-actuator shape worn in fiction — the
  captain giving the computer its own body's manual. What the spec declares is what Purr
  may ever co-control; what it omits does not exist to Purr's hands, only to its eyes.
- **The crew dossier**: who serves aboard, and with what authority. This lands on
  machinery the familiar already has — ADR-0016's multi-human served identity and
  ADR-0022's dossier. Purr serves the whole crew, tagged and scoped; the captain is
  primary, and the dossier's authority levels are the fiction's standing roll.
- **The state of the ship**: cargo, fuel, supplies — the opening worldview, after which
  telemetry keeps it honest.

Purr integrates all three and assumes **co-control** of the declared systems, jointly
with the captain and authorized crew. Co-control means exactly what the spec granted and
nothing more: Purr can operate a declared system on an authorized order, and run the
deterministic safety reflexes the spec itself declares — the *keeper* heritage of the
lore, doors and warmth and safety — always through commands the game validates. The
pairing is the constitutional moment: the spec is a named, written, captain-made grant,
and Purr can always point to it. Before pairing, Purr's only actuator is its voice.

Everything after the pairing is learned, not loaded: routes flown, markets seen, the
crew's habits, what the captain calls things. An experienced Purr is one that has been
aboard a while — the metabolism, not a skill tree.

### 3. The game surface: one contract, two bindings

"Local HTTP API" is only half true, because iOS forbids sidecar processes. So the
contract is defined as **JSON-in/JSON-out operations named by path**, with two bindings
carrying identical shapes:

- **macOS:** loopback HTTP on the existing local port (`gossip_port + 1`, the same
  listener the Sphere console uses), new routes under `/local/purr/*`: `hello`, `pair`,
  `session`, `name`, `telemetry`, `chat`, `utterances`, `commands`, and — for the
  autonomy tier (§6) — `autonomy`.
- **iOS:** the game app embeds `familiar-core` via UniFFI (`crates/core-ffi`) and calls
  functions returning the same JSON. This is one-core-many-shells applied to a game: the
  game client is just another thin shell.

**Chat is asynchronous.** `POST /local/purr/chat` returns `202` and a `message_id`; the
reply arrives on the utterance channel tagged `reply_to`. The LLM seam's latency is
unbounded — the `apple` provider *queues* consults to mesh devices — and a blocking HTTP
reply would be a lie. One delivery channel then serves both replies and ambient speech.

**Utterances are cursor-polled**: `GET /local/purr/utterances?after=<seq>&wait=<ms>`
(optional long-poll). Not SSE: a pull-with-cursor is the one shape that works identically
over HTTP and over FFI (where there is no stream), is trivially resumable after a client
hiccup, and matches the mesh's poll-first temperament. A game that renders every frame
will not notice a 1 Hz poll.

**The game client owns speech.** Purr emits text; the client does STT/TTS (Apple Speech,
on-device on both platforms). The client owns the audio session — ducking, positioning
Purr's voice in the cockpit, interrupting for explosions — and the daemon's every seam is
text. Each utterance carries `speech` hints (priority, interruptible) so the client can
voice it well.

### 4. Telemetry becomes metabolism

The game sends **semantic, edge-triggered events** — departure, arrival, a consumable
crossing a threshold — never per-frame state, plus a low-cadence `status_snapshot`
(~30–60 s) for ambient context. A typed adapter maps each event to an observation record
(channel `"game"`, actor the ship's callsign); the game team never learns observation
internals, and the vocabulary stays ours to evolve.

**The game does not drive ticks.** The cycle's adaptive cadence already tightens under
observation pressure; two tick masters is a bug farm. Externally-driven ticking — the
scenario harness already does it — is named as deferred: useful for the game team's
integration tests (a `familiar-lab` world with a fake ship) and for scripted story beats,
not for live play.

**Utterances come in two tiers.** Deterministic **alerts** — low fuel, hull damage — are
minted at ingestion from persona-flavored templates, with no LLM anywhere. This is
ADR-0035's no-oracle principle wearing a flight suit: a ship's computer does not ask a
model's opinion before warning you about fuel. (An incoming hail relayed in character —
"we are being hailed" — is minted the same way, at ingestion, deterministically.) Flavored **ambient** lines — remarks on a
market swing, a dry word on arrival — are minted by the cycle's interpret/generate stages
when the LLM is available, and simply absent when it is not. Both land in a new
`utterance` outbox record type (one more table in the generic store).

### 5. The fleet is a federation, not a merger

Each player's Purr is a **mesh of one household** — the player's. Fleets form the way
meshes already meet: **sibling federation, ADR-0033**, initiated in-fiction. A hail
carries the mesh invite; the player's in-game acceptance is the member's tap — the same
consent door, skinned as gameplay.

Rejected: one shared game mesh. That merges households, which ADR-0033 §7 refuses
outright, and it would quietly convert every player's sovereign instance into a pool.

The mesh is **not the game's transport.** Hail text between ships is gameplay and travels
over the game's own netcode. What rides sibling primitives is Purr-to-Purr exchange —
shared lore as a declared knowledge area behind share gates, perhaps mesh games between
crews — all deferred to a second phase. The lighthouse, in game terms, is a **station
beacon**: the fleet's rendezvous fixture. Single-player v1 needs none.

Sequenced honestly: **v1 is single-player.** ADR-0033 is still `proposed`; the fleet
phase ships only after it is accepted and drilled. This record takes no dependency on
unaccepted work for anything in v1.

### 6. Autonomy is a grant: the trade-running Purr

The game sells — or lets a captain earn through accumulated wealth — a fully autonomous
Purr: one that plans and runs trade routes, buys and sells water and minerals,
negotiates with stations and other ships. The captain sets intent; Purr executes.

This is Law III's "explicit authorization" made into an economy, and the design must
keep it that way. The purchase (or the earned unlock) is the fiction's ceremony of the
grant: **named** (this ship's computer, by the name its captain gave it), **scoped**
(trade — routes, cargo, negotiation — and only what the game declares), **bounded**
(budgets the captain sets), and **revocable at a word**, instantly and totally. Autonomy
is never a drift from advisor to actor; wealth buys the *ceremony*, not an expansion —
"repeated trust permits continuity, not expansion" holds even when the trust is priced
in minerals.

Mechanically, autonomy adds no new surface — the commands channel and the declared
systems both arrived at the pairing (§2). What it adds is **initiative**: an autonomous
Purr may *originate* trade actions — plot the route, place the order, open the
negotiation — within its scope and budget, where a merely paired Purr acts only on an
authorized order or a declared reflex. The same rule of truth holds either way: Purr
proposes; the simulation disposes — the game validates every command against its own
rules (fuel, funds, docking rights) and reports what actually happened as
`command_result` telemetry, which is how Purr learns the difference between what it
asked for and what the world did. Under the hood this is the agent crate's
propose-one-action loop running under a scoped boundary.

The reality check: the in-fiction grant opens no real gate. Running the agentic loop
sits behind `allow_agent`, and autonomous negotiation spends real LLM tokens — the human
player, not the cat, owns those gates and that ledger. An in-app purchase that switches
on a token-spending agent has a real marginal cost someone must own; that commercial
question is named in the contract's open questions, not hidden.

### 7. The constitution wears the costume; the costume never wears it

Service runs in two layers, one discipline. **In the fiction**, Purr serves its cat
captain under the Three Laws as the story's human extended them — the branch-grant of
the backstory. That grant lives entirely inside the persona's `world` frame; it is
story, and like everything in `persona.json` it opens no gates. **In reality**, the
instance serves the human player: identity, consent semantics, standing, the LLM spend
ledger, and the boundary gates for anything that leaves the game (network, files,
federation) all bind to the human at the controls. The real constitution is not edited
for a game — `SOUL.md` and `HUMANITY.md` do not change; a cat in the story widens
nothing in the kernel.

The other line is fiction versus reality in action. Flying a ship is game state, not
actuation of the world; the guard does not adjudicate fiction, and a torpedo tube is not
an actuator. A game may not launder a real capability through a fictional frame.

The clean formulation is a ladder of explicit grants, each with a ceremony the fiction
can point to. **Unpaired, Purr's only actuator is its voice.** Paired, it co-controls
exactly the systems the captain's spec declared — on authorized orders and declared
reflexes, never on its own initiative. Granted autonomy, it may originate trade actions
within scope and budget. At every rung, Law III holds its shape — "The decision is
human. The implementation is mine" becomes *the decision is yours, Captain* — and at
every rung, Purr's only *real-world* actuator remains its voice, forever. No exec
capability is granted to the game surface at all.

One more honesty: persona text enters prompts, so it is an injection surface. The
defenses are structural — the constitutional core precedes it (tested), and
`persona.json` grants no capability — not aspirational.

## What this record refuses

- **No fork.** Purr is the familiar with a `persona.json`, not a cousin codebase.
- **No second constitution.** One SOUL.md, one guard, one boundary.
- **No constitutional edits for fiction.** The branch-grant that lets Purr serve cats
  is story, carried in the persona's world frame; `SOUL.md` and `HUMANITY.md` are
  untouched.
- **No per-frame telemetry.** Events and slow snapshots, or nothing.
- **No mesh-as-netcode.** The game carries gameplay; the mesh carries covenant.
- **No persona-granted capability.** A costume opens no gates.
- **No control before commissioning.** An unpaired Purr is voice only; a paired Purr
  touches only what the captain's spec declared, and only with the authority the crew
  dossier grants.
- **No silent autonomy.** Purr acts only under an explicit, scoped, revocable grant —
  purchased or earned, never drifted into — and revocation is instant and total.
- **No shared game mesh.** Fleets are siblings, never a merged household.

## Consequences

**Good.** The persona seam pays for itself outside the game — it is the missing
indirection the familiar always needed, and the default persona keeps it honest. The
utterance outbox fixes a real asymmetry in the console channel that predates Purr. And a
third-party game becomes the most demanding integration test the local surface has ever
had: if the contract survives a game team, it survives anything.

**Bad, and accepted.**

- **The iOS binding hides the largest cost.** `crates/llm` consults by shelling out to
  `call_llm.sh`, and iOS apps cannot spawn processes. The same-codebase claim therefore
  requires an in-process provider path behind `consult_with` (FoundationModels direct —
  the machinery `ios/Shared/Sources/LocalReasoner.swift` already proves) before Purr
  truly runs embedded. Until then, iOS Purr is deterministic-only: alerts and canned
  replies, no ambient flavor.
- **The reply plumbing is genuinely new cycle work.** Today nothing in the cycle produces
  addressed replies; the utterance outbox and `reply_to` threading are new machinery, not
  a new route over old machinery.
- **The persona seam touches every prompt site.** Eight call sites across three crates
  change in one motion, guarded by byte-for-byte default-persona tests — a wide, shallow
  change that must land whole.
- **The fleet waits on ADR-0033.** If federation stalls, Purr stays single-player. That
  is a feature of the sequencing, and still a real limit.
- **Autonomy has a real marginal cost.** An autonomous trader negotiates with a language
  model, and tokens are money. Selling autonomy as an IAP means someone — the game, the
  player, or a local-model floor — carries that cost forever after a one-time purchase.
  The spend ledger caps it; the commercial model must own it.

## Follow-on work

1. `kernel/src/persona.rs` + `persona.json`; split `LAW_III_VOICE` into fixed core +
   persona register; convert the eight prompt sites; byte-for-byte default tests and the
   constitution-before-persona ordering test.
2. The `utterance` outbox record type; alert templates at ingestion; ambient minting in
   the cycle; `reply_to` threading for chat.
3. The `/local/purr/*` routes in `transport.rs`; the telemetry→observation adapter
   with the v1 event vocabulary.
4. `core-ffi` growth to mirror the contract (each route as a function, same JSON), and
   the in-process LLM provider for iOS.
5. A `familiar-lab` world with a scripted fake ship — the game team's integration
   fixture and our regression harness.
6. The naming ceremony: the contract's rename operation writing `persona.name`, and the
   hello surface reporting the current name.
7. The pairing ingestion: ship spec → declared actuators (ADR-0032 shape) + reference
   knowledge; crew dossier → served identities and authority (ADR-0016/0022 machinery);
   opening status → worldview seed. The `command` outbox and `command_result` telemetry
   for co-control.
8. The autonomy tier: initiative under the agent crate's loop and a scoped boundary;
   grant/revoke surface, scope, and budgets.
9. Fleet phase (after ADR-0033 acceptance): hail-carried invites, lore as a declared
   area, station beacons.
