# familiar-sc — the ship's computer as a product: a compact familiar for UCF, Apple Intelligence as its voice and judgment, Apple clients as the captain's window

*Round 1: claude, 2026-09-02, on Ian's direction (verbatim): "creating familiar-sc (ships
computer) focusing on that skill set as an add on module for UCF. It would continue to have
the option to be mesh connected, but would be a stand alone stripped down to just what is
needed version of the familiar. Compact. Focused on using Apple Intelligence/PCC as the
driver for the decision and communications and integration tasks in a well defined
environment. iOS/ipad/Apple vision clients that could be run as stand alone companion apps
to UCF in addition to the UCF interfaces. The Apple vision client would focus on the
Delta-V game interface for plotting sources and race viewing and replays."*

*Codex: append Round 2 — challenge the placement of intelligence (§3), the hosting answer
(§4), and the brick order (§7). Ian decides.*

---

## 1. What familiar-sc is, in one paragraph

The ship's computer, sold as a product: the familiar's UCF skill set — the pilot doctrines
(freight, trade, outfit; navigator and race conscience when racing lands), the fleet
supervisor, and the per-ship persona (T-236) — composed into ONE compact runtime plus a
Swift client SDK and three Apple clients (iPhone, iPad, Vision Pro). It is not a fork. It is
a build profile of the workspace we already have, with the mesh as an optional feature:
paired to a household when one exists (today's KK II), self-sufficient when there is none
(a captain who only owns an iPad and a UCF account). The ship's computer speaks and reasons
with Apple Intelligence; it ACTS through the same deterministic doctrines that fly KK II
tonight, and only through them.

## 2. What already exists (checked in the tree, 2026-09-02)

| Piece | Where | State |
|---|---|---|
| The pilot: freight/trade/outfit doctrines, hold clock, physics deadhead, repair, receipts | `crates/whisker` | flying PROD; 41 tests |
| Pairing, revocation, per-captain status + P&L, supervisor with lease renewal | `crates/cli` `familiar fleet` (T-235) | landed today |
| Ship world = its own store (key, grants, journal, deliveries, holdings) | ADR-0045, `crates/world` | in use |
| Lease/boundary machinery (fail-closed, signed, expiring) | `crates/world::lease`, kernel boundary | in use |
| Governed HTTP client to the exchange (TLS, bounded, IPv4-first) | `crates/mcp::http` | in use |
| Persona per ship (name, voice, captain-scoped memory) | T-236 dialogue closed; brick 1 claimed | building |
| Apple Intelligence consult ladder (on-device → PCC on OS 27 with consent → none) | `ios/Shared/Sources/ConsultRunner.swift`, `FAMILIAR_SDK_HAS_PCC` sdk-gate | shipped in the Familiar app |
| Apple app targets (iOS, macOS, watchOS 26) | `ios/project.yml` | shipped (macOS build 103 in review) |
| The exchange's seams: co-pilot key (ucf-exchange#15), pending overlay, receipts, route physics | Jeff's side | #15 open; rest live |
| Racing concept with the familiar as navigator/conscience | metal#63 filed; dialogue 2026-09-01 | Jeff's steer pending |
| Deterministic sky: `SolSystem`/`SolClock`/`FlightModel` (integer, replayable) | `UCFEngine/Space` | exists; multi-flyby and sealed race envelopes do not |

Nothing in this plan requires a new engine. Everything new is composition, packaging, and
two client surfaces.

## 3. Where the intelligence lives (the line that keeps it safe)

**Decisions that spend, move, or bind the ship stay deterministic.** Booking, travel,
buying, selling, repairing, refitting are doctrine functions with pinned tests, gated by
the ship's grants and lease. This is the Three Laws as architecture and it is also what
Jeff's world rewards: folds are deterministic, refusals are receipts, and a pilot that is
wrong sits still rather than losing money. The on-device model is a 3B-parameter LLM Apple
itself scopes to summarization, extraction, classification and generation, not to math or
complex reasoning — exactly the wrong tool for "should I book L2706 on an economy contract
with an 88%-worn hull" and exactly the right tool for telling the captain why the pilot did
not.

**Apple Intelligence drives three things, all bounded:**

1. **Communications** — the computer's voice (T-236 persona): what happened, why, what is
   next, in its own name and cadence, grounded in the ship journal and the receipt trail.
   On-device `LanguageModelSession` with `@Generable` output (a typed "bridge report":
   headline, facts, next act, mood) so the UI never renders free text it did not shape.
   Codex's T-236 ruling stands: style may bend cadence, never truth or judgment.
2. **Integration** — turning what a captain says into what the doctrines accept, via the
   Tool protocol: `SetStandingOrder` (prefer priority loads; hold trading for a day; keep
   ℳ3000 in hand; never book economy on a worn hull), `AskStatus`, `ExplainDecision`,
   `PairShip`. Each tool's `Arguments` is `@Generable`, validated against the doctrine's
   own bounds before it is written into the ship store as a standing order. The model
   proposes; the doctrine disposes.
3. **Judgment aids where the doctrine has slack** — choosing among near-equal plans, a
   race-line risk appetite, phrasing a warning — and, on OS 27 with the entitlement and
   the captain's consent, **Private Cloud Compute** for the heavier reads: a day's journal
   (32k-token window, reasoning levels) into a bridge log, a race course critique, a
   "what would you have done" review. PCC is availability-gated, quota-limited and
   network-dependent; the on-device path is the floor and the templated voice (T-236 Q1)
   is the floor under that. No path fails to a blank screen.

What Apple Intelligence never does: place an action on the exchange, alter a doctrine
constant, override a refusal line. The refusal line is the product ("the familiar can get
you there safely; winning requires questionable judgment").

## 3.5 The autonomy dial — Ian's ruling on how the computer interacts (2026-09-03)

Ian, verbatim: *"That's how the ships computer should interact in the game. There
should be a message window giving advice, and the captain can choose to follow or not.
there should also be the ability to just allow auto a captain authorized command and
the ship will Proceede as KKII is now. Should have ability to granularity config
autonomy for each control surface category and family."* — said of the sentence the
computer would have given him at titania ("don't call the tanker, fly to foxys-diner
now, 98 fuel, refuel on credit") while his own hull sat pinned by a PAWS call for days.

So every doctrine decision has THREE possible fates, chosen per control surface:

| Level | The computer… | The captain… |
|---|---|---|
| **advise** | says what it would do and why, in the message window; does nothing | acts, or not |
| **confirm** | proposes the act and waits one fold for a yes | approves per act (or lets it lapse) |
| **auto** | acts and journals, as KK II does today | reads the log |

**Control surfaces, by family and category** (the dial is set per category, with a
family default):

- **Navigation** — `course` (travel/engage/carry legs), `fuel` (refuel, divert to a
  pump), `rescue` (the tanker; on PROD its own default is *advise*)
- **Freight** — `book`, `collect`, `cancel`
- **Market** — `buy`, `sell`, `carry`
- **Ship** — `repair`, `refit`, `crew`, `frame`, `lease` (buyout)
- **Racing** — `plot`, `line` (risk past the safe line), `refusal` (never auto)

The grant model stays: an automation the captain has not bought is absent from the
dial altogether. What is bought can be set anywhere from advise to auto per category,
and changed at any time from the client or `familiar fleet autonomy`. The message
window is the ship journal's advice and proposal lines, voiced by the persona (T-236);
the same feed drives notifications in the companion app.

Implementation seam: `autonomy.json` in the ship store maps category → level;
whisker's action gate consults it after the doctrine decides; *confirm* writes a
proposal with a fold's TTL to `proposals.jsonl` and acts only when an approval line
appears; *advise* writes the advice and holds. Approval arrives from the client (or
`familiar fleet approve <ship> <id>`), never from the model.

## 4. Where the runtime lives (the honest hosting answer)

The exchange folds every 180 s on PROD and a pilot must be there for every fold or it
misses pickup windows and clocks. iOS does not grant a foreground-less process that wakes
every three minutes for days; `BGAppRefreshTask` is discretionary and Vision Pro sleeps
on the shelf. So:

- **The brain stem is headless and hosted.** `familiar-sc` the binary = today's fleet
  supervisor + in-process pilots + the persona store, running where the captain has an
  always-on Apple host (a Mac: this is KK II today) or, for captains without one, a hosted
  tier — river.io first, and whatever Jeff steers for the add-in (his exchange box already
  hosts the MCP door). One process per captain, one ship world per hull, the store is the
  truth.
- **The voice lives on the captain's Apple devices.** The clients speak Apple Intelligence
  on the device in the captain's hand (on-device FM everywhere; PCC where OS 27 and the
  entitlement allow). On a Mac-hosted SC the host can also speak. On a Linux host it cannot,
  and nothing is lost: the client reads the same journal.
- **Foreground folds are a bonus, not the design.** While the iPad app is open it may run
  the doctrines locally against the same ship store snapshot for immediacy (the doctrines
  are small, pure functions; the Swift port is a week, or the Rust crate builds for iOS via
  a C ABI). It must never race the host: one writer per store (the lease says who).
- **Mesh optional.** With a household, the ship world is leased by it and the SC is a peer
  (the door and lease machinery exist). Standalone, the captain's own SC issues the lease
  from a local issuer key: the captain IS the household. Same files, same gates.

## 5. Compact means a build profile, not a rewrite

`familiar-sc` is a Cargo profile/feature set of this workspace: `whisker`, `world`, `mcp`,
`kernel::{boundary,lease,guard}`, the fleet supervisor, the persona store. Excluded: the
scenario engine, factory, periphery, the household hub and its rings. Target: one static
binary, one config directory (the ship stores), one command:

```
familiar-sc pair --captain <name> --server <exchange> --key <ucfk_…> --automations freight,trade,outfit
familiar-sc run --renew
familiar-sc status --json        # what the Apple clients read
```

The Swift side is one package, `FamiliarSC`: the exchange wire client (typed `/v1` models
already half-written in UCF-Haul; reuse by agreement with Jeff rather than copying),
the ship-store reader (journal, deliveries, holdings, persona), the pairing flow (scan or
paste the co-pilot key; QR per UCF-Haul#67), the bridge voice (FM session, tools,
`@Generable` reports, availability ladder), and notifications. Three app targets consume it.

## 6. The Apple clients

- **iPhone / iPad — the captain's bridge.** Ships list with the SC's own name and mood;
  per-ship status and P&L (what `fleet status` prints, drawn); the journal as a timeline
  with the voice narrating; standing orders (spoken or typed → tools); pairing/revoke;
  notifications for the events a captain acts on (position opened/sold, delivery paid,
  repair, lease lapse, distress). Runs beside UCF-Haul, never replaces it.
- **Vision Pro — the ΔV bridge.** A volumetric Jovian system drawn from the engine's own
  integer sky (`SolSystem` at the race epoch: a moving sky over a fixed event), the
  captain's hull and rivals from the exchange's ships list, and three modes: **plot**
  (lay a course in the bounded course grammar — waypoint sequences and parameter bands the
  engine scores; the SC voices its safe line and its refusal line), **watch** (a live race
  as the folds land), **replay** (re-fold a sealed race envelope deterministically and
  scrub it). RealityKit volumes for plot/replay; an immersive space for spectating. Every
  one of these needs Jeff's racing layer (metal#63: typed course, sealed envelope, result
  hash, replay API); before it lands the same client can ship a **live system view**
  (hulls in the system, courses on file) as an honest first volume.

## 7. Bricks, smallest visible service first

| Brick | Delivers | Depends on | Visible proof |
|---|---|---|---|
| **B1 — the standalone profile** | `familiar-sc` binary: fleet supervisor + pilots + persona store; self-issued lease when no household; `--features mesh` for the household path | T-235 (landed), T-236 brick 1 (in flight) | KK II flown by `familiar-sc run` on wildhorse with no hub process running |
| **B2 — `FamiliarSC` Swift package** | wire client, store reader, pairing, bridge voice (FM + tools + `@Generable` report + ladder), notifications model | B1's `status --json` and store layout | a unit-tested package; the voice answers "what did you do today" from KK II's real journal on-device |
| **B3 — iPhone/iPad companion** | the captain's bridge (§6), TestFlight to Friends & Space Truckers | B2; co-pilot key when Jeff mints it (a trading key pairs the same today) | Ian pairs KK II from his iPad by scanning the key; hears the SC's report; sets a standing order that the pilot obeys on the next fold |
| **B4 — Vision Pro ΔV bridge, live view first** | volumetric system view of hulls and courses; plot mode on the bounded grammar; watch/replay when metal#63 ships | B2; Jeff's racing steer for plot/replay; a Vision Pro to test on | a race replay scrubbed in a volume, bit-identical to the exchange's result hash |
| **B5 — hosted SC tier** | one process per captain on an always-on host, provisioned from the client | Jeff's add-in guidance; river.io ops | a captain with no Mac gets a pilot |

B1 and B2 can start now. B3 is a fortnight after B2. B4's live view can start with B2;
its racing modes wait for Jeff.

## 8. Open questions for Ian (decide before B1)

- **Q1 — hosting for non-Mac captains.** River.io VPS as the first hosted tier, or wait for
  Jeff's steer and ship Mac-hosted only at first?
- **Q2 — OS floor.** iOS/iPadOS/visionOS 26 as the minimum (on-device FM everywhere), with
  PCC as an OS 27 enhancement behind the existing consent switch — or hold the clients for
  27 and make PCC a base feature? (The Familiar app carries the PCC entitlement gate today;
  the SC apps need the entitlement in their own bundle IDs.)
- **Q3 — product shape.** A separate App Store SKU ("Familiar SC") beside the Familiar
  app, or a mode inside it? Separate keeps the compact promise and Jeff's add-in story
  clean; inside reuses review history and the ring. I lean separate.
- **Q4 — Vision Pro in hand?** B4 needs a device for anything beyond the simulator.
- **Q5 — the name.** "familiar-sc" is the product line; each computer keeps its own name
  per T-236 (Purr by default, the captain renames). Confirm that split.

## 9. Method

Codex Round 2 on this document (placement of intelligence, hosting, brick order), then a
board task per brick. Jeff sees the add-in only when B1–B3 are working end to end on KK II,
per Ian: "we need it to be all working first."
