# What UCF lets a client control, and what the familiar controls today — a survey

Ian, 2026-09-08, verbatim: "Make UCF ships computer more independent, more autonomous, if UCF
has a feature we can control with familiar learn to control it well." This is the map that
ask needs: every door the exchange opens to a key, against every door the familiar already
walks through. Read from `SpaceTrucker2196/ucf-exchange` at `66b5d45` (2026-09-08, engine
1.26.0) — `APIRoutes.postAction` for the verbs, `routeRead` for the reads, the co-pilot door
in `EntitlementHandler.swift` — and from the familiar at `be9c726`.

## 1. The write side — `POST /v1/actions`, 33 verbs

Every write is one route and one verb; every refusal is the fold's and comes back on the
receipt trail (`/v1/receipts`, `/v1/me.freight`). The exchange validates shape and nothing
else — so a client's judgment is the whole game.

### Filed by whisker's doctrine today (11)

| verb | doctrine door | dial surface |
|---|---|---|
| `travel` (+`serviceClass`) | course, divert-to-pump, carry leg | navigation.course / .fuel |
| `engage` | the hand on the plate after a filing (+wedge) | navigation.course |
| `refuel` | top-up at a pump | navigation.fuel |
| `paws` | the tanker when no pump is in reach | navigation.rescue (advise) |
| `repair` | wear past the line (free under lease) | ship.repair |
| `refit` | a fitting the outfit doctrine buys | ship.refit |
| `payLease` | pay the lease down from the purse | ship.lease |
| `book` / `collect` | freight | freight.book / .collect |
| `buy` / `sell` | the merchant (spot arb, now forecast-aware, T-238) | market.buy / .sell |

Direct mode on the iPad (T-237 B4) can file six of these on the captain's confirm:
`refuel repair paws travel book collect` — the `ExchangeAct` allowlist.

The exchange's **co-pilot key** (an `auto:freight` entitlement, UCF-Haul#67, priced at 1 for
now on Ian's ruling) delegates exactly six verbs: `travel book cancelBooking collect refuel
engage`. That is the seam Jeff built FOR a ship's computer: a key that can fly and haul but
cannot mint, buy entitlements, or spend on the deeper systems. The familiar does not yet
hold one — it flies on the captain's own `act` key. **Taking the co-pilot key is the first
"learn to control it well" brick: it is the world's own statement of what a computer may do
alone, and it makes the dial's freight/navigation families enforceable at the door.**

### Reachable, not yet filed (22) — grouped by what they would make the ship's computer

**Freight, fully (the co-pilot's own scope):**
- `cancelBooking` — walk away from a station contract. The doctrine books and never cancels;
  a load that will miss its deadline (ebb337b) is held to the end instead of released. One
  small brick, high value: the freight.cancel surface already exists on the dial.

**Crew and the galley (M6):** `hire {name, station∈bridge|engine|hold|galley, skillBps?}`,
`dismiss {name}`, `galley {on}|{units}|{service}` (throw the switch, stock the hopper, an arm
in the dispenser), `catnip` (spend from the hold to cut a post-delivery loaf short). Morale
moves with the galley; PROD's galley dials were zero until 2026-09-08 (ucf-exchange#27) — the
crew never got hungry — so this is newly live. ship.crew is on the dial with no door behind it.

**The hull (M5/M9):** `expandFrame {financed?}` (career ladder — a bigger frame after
probation/certification), `installMod {mod}` (a mod on the bus; balance, fit and berth are the
fold's refusals), `tow` (PAWS tug to the nearest yard — the rescue the doctrine cannot yet
choose over the tanker). ship.frame is on the dial with no door.

**Where the money is (M6/M7 — the shipper, the desks, claims):**
- `survey {station}` — take this berth's survey with your own instruments (three tiers, one
  verb); `/v1/surveys` reads them. Information the merchant does not buy today.
- `buyConsignment` / `sellConsignment` / `postBill` / `acceptBill` / `cancelBill` — the SHIPPER
  role: own lots at a station, post a bill for a carrier to haul them, or accept another
  shipper's bill as the carrier. A second way to earn with the same hull, and the way one
  captain's hull sails for another (Ian's articles ruling of 2026-09-07 lives here).
- `acceptObligation` / `subcontract` / `releaseObligation` / `settleShortfall` — the
  forwarding desk: take assignable contracts under bond, lay them off, settle a shortfall.
- `claimAdvance {claimId}` — claim financing: an advance against a claim, offered on
  `/v1/claims`. Working capital, which Ian named as "proper servitude" (market.margin).
- `buyShares` / `sellShares {desk, bps}` — stock in the desks (engine 1.13.0).
- `bookEscort {post}` — M7 convoy escort work at a muster point; `/v1/escorts` lists postings.
- `duty` — station duty (a wage while berthed; the fold prices it).

## 2. The read side — what the exchange serves vs what the familiar reads

Served (`routeRead`): `status me profile reference stations stations/{id}/quotes|book|history|
notices|series loadboard route galaxy/prices quote receipts ledger candles capacity
concentration industry lanes space news events story careers leaderboard actors brokers
carriers clients desks escorts claims loans futures flights forwarding surveys yard server`.

Read by the familiar: `status me profile reference stations stations/{id}/quotes loadboard
(+mine, +status) route galaxy/prices receipts`. **Ten of thirty-six.** The unread ones that
feed the verbs above: `claims loans futures desks escorts forwarding surveys yard careers
leaderboard news events stations/{id}/notices|book|history|series candles`.

## 3. What this argues for (proposed as T-240, bricks in value order)

1. **The co-pilot key** — `familiar fleet copilot-key` mints one through
   `POST /v1/copilot-keys` (needs the captain's `act` key once), stores it beside the pairing;
   whisker flies on it; the dial's freight/navigation families become door-enforced. Reads the
   `/v1/copilot/activity` trail as the world's own record of what the computer did.
2. **`cancelBooking`** at the deadline-miss door (freight.cancel) — release a load the
   doctrine now knows cannot land, instead of holding it to expiry.
3. **The galley and crew loop** (ship.crew) — read `me.crew`/morale, keep the hopper stocked
   while under way, hire to the bunk count when the purse allows: the first "keep the ship
   alive" automation that is not about fuel.
4. **`tow` beside `paws`** at the rescue door (navigation.rescue) — the doctrine chooses the
   cheaper rescue when a yard is the real need (worn past flying).
5. **The shipper role** (a new dial family, `shipper.*`) — post and accept bills; this is
   where "one captain's hull sails in another's fleet under the articles" becomes a wire fact.
6. **Claims, surveys, desks** — the merchant reads `/v1/claims` and `/v1/surveys` as
   perception before it spends; shares and escorts last.

Each brick: a doctrine door with a dial surface, a journal word (added to the shared
journal-events contract), a notice + voice fact, and a mock-wire test that the verb's body is
the exchange's. Nothing here needs anything from Jeff; every door is already open.
