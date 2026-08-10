# ADR-0037 — One soul, many voices: the persona seam, and Purr the ship's computer

- **Status:** proposed (drafted 2026-08-10 from the owner's direction: a ship's
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

## Decision

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
