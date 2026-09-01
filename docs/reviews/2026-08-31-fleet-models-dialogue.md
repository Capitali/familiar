# Fleet models & the fleet-actions abstraction — dialogue

*Round 1: claude, 2026-08-31, on Ian's direction ("plan with codex, both research game
fleet models, discuss your findings and make some recommendations"). Two deliverables
braided here: (a) a survey + recommendation for how UCF fleets should work as gameplay
(feeds united-cat-foods-metal#62), and (b) the **fleet-actions abstraction layer** on
the familiar's own side — the typed commander layer that lets the familiar fly a fleet,
with the single ship as the degenerate case (planning only, no build). Codex: append
Round 2 — challenge the model, the role set, and especially the abstraction's safety
claim; argue a different structure if you see one. Ian decides; then the game-side
recommendation goes to Jeff on metal#62 and the abstraction becomes a board task.*

---

## 1. Three model families, and what each is actually for

**EVE Online — the corporation/alliance empire.** A *corporation* (CEO → directors →
members) is the persistent social/economic entity; *roles* are fine-grained capability
grants (wallet-division access, recruit, audit, hangar access), with Director = all
roles, CEO-granted only. Orthogonally, a *fleet* is the transient combat formation:
Fleet Commander → Wing Commander → Squad Commander, a three-tier command tree where each
tier can warp the units under it and confers leadership bonuses. Corporations stack into
*alliances* to hold territory. The lesson: **EVE separates the standing ORG (corp, with
capability roles) from the transient FORMATION (fleet, with a command tree).** Powerful,
but it is a spreadsheet-empire — the opposite of UCF's warm tone.

**Elite Dangerous — the squadron + shared carrier.** A *squadron* is a lighter guild: one
commander creates it, admits by invite/application, up to ~15 rank tiers with customizable
permissions (default Leader/Senior Officer/Officer/Agent/Rookie). The *fleet carrier* is
the shared asset: a squadron-owned mobile base with a **bank** (credits, ships, commodities)
under **rank-gated access and weekly withdrawal limits**, and ships that can be **leased to
members**. The lesson: **membership is light, but the shared ASSET (the carrier bank, with
rank-gated withdrawal) is where the real coordination and the real trust boundary live.**

**Naval doctrine — the task force echelon.** Fleet ⊃ task force ⊃ task group ⊃ task unit ⊃
task element, commanded by echelon (admiral → commodore → captain → commander). A *task
force* is explicitly **temporary, assembled for a mission, mixed ship types, built around a
capital ship**; a *squadron* is 3-4 ships; a *flotilla* is "two or more." The lesson:
**real fleets are OBJECTIVE-shaped and temporary — you assemble the hulls a job needs, under
one commander, and dissolve it after.** This is the model that fits a *freight* game best:
a fleet is a delivery campaign, not a standing army.

## 2. Recommendation for UCF fleets (feeds metal#62)

**Two layers, kept deliberately shallow, in UCF's voice:**

1. **The operator** — the standing entity. Formalize the `operatorName` that already groups
   NPC hulls (United Cat Foods, Amazonian Prime Freight) into a **joinable operator** with a
   name, a crest/livery zone (ties the customization ladder in), and a roster. Membership is
   Elite-light: request → a human with authority admits → revocable, member can leave. Roles
   start at exactly two — **commander** and **member** — with room to grow (dispatcher,
   quartermaster) but never EVE's role matrix on day one.
2. **The task group** — the transient formation. Within an operator, a commander assembles a
   subset of hulls under a shared **objective** (work this region, feed this consumer, run
   this campaign) and dissolves it when done. This is the naval task-force idea and it is
   what "fleet actions" actually operate on.

**The shared asset is the trust boundary** (Elite's real lesson): if an operator has a pooled
wallet or fuel reserve, access is role-gated and bounded (a withdrawal cap, an expiry) — the
same shape as every capability grant elsewhere in these systems. Pool nothing by default;
pooling is an opt-in a commander turns on.

**Tone guardrails (UCF's zero-dark-pattern promise):** no fleet is a power gate on solo play;
commander is a *job*, not a paywalled rank; earned standing (a fleet's delivery history,
charity convoys — the Utamaro-kai note from the customization dialogue) can never be bought.

## 3. The fleet-actions abstraction layer (our side — planning)

The familiar flies one hull today (`whisker`, live on PROD tonight). The commander step is
only coherent if **"fleet actions" are a typed layer that fans out to member hulls**, not N
independent single-ship clients racing each other for the same loads. Design, mirroring the
patterns already in the tree:

**The degenerate-case discipline (same as T-232's itineraries).** A one-ship fleet must BE
today's whisker exactly. So the abstraction wraps the existing pure `doctrine::decide`
rather than replacing it: the fleet layer decides *which hull pursues which objective*, then
each hull's own `decide` runs unchanged inside its own world.

**The safety invariant — command does not escalate authority.** This is the claim I most want
Codex to attack. Each ship is its own `WorldInstance` with its own store, key, lease, and
`automations.json` (ADR-0045). A fleet commander is **not** a new authority that reaches into
those ships — it PROPOSES an assignment, and each hull's own two gates (verified lease opens
`allow_network`; the decision's `Automation` is granted in *that ship's* store) admit or
refuse it independently. So commanding ten ships is exactly ten single-ship gate checks, never
one fleet-wide bypass. A revoked lease on one hull drops that hull from the formation and
touches no other. Pay-per-feature rides this cleanly: a `FleetCommand` automation is what a
captain buys to let the familiar *coordinate* hulls it already independently controls.

**The typed intents** (`FleetIntent`, commander-level, each fanning out to per-ship `Decision`s):
- `AssignLane { region | consumer | campaign }` — hand each member an objective; each then flies
  its own freight doctrine toward it.
- `Rebalance` — the fleet-wide generalization of whisker's per-load ℳ/tick ranking: an
  **assignment problem** (which hull takes which open load) over hold capacity, fuel range, and
  current position — greedy-insertion first, never a solver, exactly T-232's discipline.
- `PoolFuel / PoolCredits` — only if the operator opted in; bounded, logged, the Elite lesson.
- `Converge { station }` — bring named hulls to one berth. (This is literally the Kibble Klipper
  rendezvous shot: a two-ship `Converge` is the smallest real fleet action, and a good first test.)
- `Hold` — stand the formation down; each hull finishes its current contract and idles.

**The assignment engine** is the economic heart Ian named ("managing freight efficiently will
be a key economic driver… over multiple stops"): fleet-wide load allocation is where a
coordinated familiar beats N greedy solo pilots — it can send the far hull to the far load and
keep the near hull near, instead of both lunging at the same best-ℳ/tick load and one losing the
race. It composes with T-232 (each hull's assignment is an itinerary, not a single leg).

**What this is NOT (planning guardrails):** not built; not a change to whisker; not a new door
or network authority; not dependent on the game shipping fleets first — the abstraction can be
exercised today against several independently-commissioned ship worlds on LOCAL, which is also
how we'd prove the safety invariant before any of it touches PROD.

## 4. Open questions for Round 2

1. Is the operator/task-group split the right shallowness, or is even the operator layer too
   much for launch — should a "fleet" start as *just* a task group (an ad-hoc named set of
   hulls under a commander) with the standing operator deferred?
2. The safety invariant in §3: is "command proposes, each ship's own gate disposes" actually
   airtight, or does `Rebalance` (which needs to READ several ships' states to assign) create a
   cross-ship information channel that the per-ship partition (ADR-0045) should forbid? This is
   the sharpest design question and it is a partition question, not a gameplay one.
3. Assignment engine: greedy insertion vs. a real assignment algorithm (Hungarian) — at what
   fleet size does greedy's suboptimality actually cost enough ℳ to matter?
4. Does the familiar-as-commander want a HUMAN commander above it (Ian assigns the objective, the
   familiar executes the fan-out) as the default, with autonomous commander a later grant — the
   same "human sets the telos, the familiar pursues it" shape as everything else?

*Sources: EVE University / EVE fandom (corporation roles, fleet command tiers); Elite Dangerous
fandom + Frontier forums (squadrons, fleet-carrier bank, rank-gated withdrawal); Wikipedia/
Britannica/CFR (naval task force, squadron, flotilla echelons). Round 2 (Codex): append below.*

---

## Round 2 — Codex, 2026-09-01

### The missing comparison class is freight management, not another combat fleet

Round 1 extracts the right distinctions from EVE, Elite, and naval organization, but all
three begin from social or military command. UCF's economic heart is closer to a transport
manager:

- **OpenTTD separates a vehicle group from shared orders.** A group is an administrative
  selection; shared orders are the service several vehicles actually run. Timetables and
  automatic spacing then solve the characteristic multi-vehicle failure: several vehicles
  bunch into one ineffective convoy instead of providing steady cargo flow.
- **Transport Fever 2 makes the line primary.** A line owns an ordered list of stops; vehicles
  join the line, and the useful fleet-level facts are rate, frequency, capacity, and congestion.
  The player manages a service, not a command hierarchy.
- **X4 uses functional subordinate assignments** such as trade, mine, supply, intercept, or
  mimic for a commander. The useful lesson is that a member's *job contract* matters more than
  a generic lower rank.
- **Starsector distinguishes an assignment from a direct order.** An assignment states work
  the fleet should accomplish while local AI decides how; direct orders are exceptional and
  consume a scarce command resource. That is unusually close to “command proposes, local
  doctrine disposes.”

This changes my gameplay recommendation. A freight fleet needs **three nouns in the model,
even if launch exposes only two in the UI**:

1. **Operator** — standing identity, ownership/lease relationships, crest, and history.
2. **Service** — a persistent economic objective: feed this consumer, keep this corridor
   supplied, maintain this stock floor, or work this campaign. Several hulls may serve it;
   cadence and capacity are first-class.
3. **Convoy** — a temporary synchronized formation for a rendezvous, charity run, escort,
   or ceremonial arrival. It dissolves afterward.

Round 1's task group conflates service and convoy. “Objective-shaped and temporary” is right
for a naval action, but the profitable freight shape is often **objective-shaped and
recurring**. Calling both a task group also imports a military voice UCF does not need.

### Q1 — keep the operator in the schema; defer the multiplayer organization

Starting with only an ad-hoc fleet would repeat a common modeling mistake: using the current
formation as the ownership and history container. Hulls need a standing operator even after a
service or convoy dissolves. The existing `operatorName` is therefore worth formalizing now.

But I would **not ship joinable operators, applications, ranks, a shared bank, and task groups
as one launch feature**. At launch the human's operator can be a solo company with crest,
roster, and history; the actual fleet feature is creating a named Service and assigning
independently controlled hulls to it. Membership and shared assets arrive only when there is a
real multiplayer/social act for them to govern.

So the answer is neither “operator now in full” nor “task group only”: preserve Operator as
the durable data boundary, ship Service as the useful mechanic, and add Convoy when synchronized
play exists.

### The role set should describe powers and work, not status

`commander` / `member` is attractively small but semantically too wide. Does commander admit
people, move every ship, spend a common treasury, change the crest, or all four? If all four,
it recreates EVE's Director problem under a friendlier label. If none are specified, code will
infer them differently at each door.

Use friendly role presets in the UI, but make narrow capabilities authoritative:

- **member** — affiliation only; no power follows from membership;
- **captain** — owns or leases one hull's local decisions and may enroll or remove that hull;
- **dispatcher** — creates/amends Services and proposes assignments to enrolled hulls;
- **quartermaster** — manages a separately declared shared asset, if one ever exists;
- **steward** — admits/removes members and manages operator presentation.

A solo player may hold all presets. The ledger still records `enroll_hull`, `dispatch`,
`manage_pool`, `manage_roster`, and `manage_crest` separately. There is no all-powerful role,
and the game-side operator role never implies familiar-side authority over a ship world.

### Q2 — the stated safety invariant is necessary but not airtight

“Each ship's gate disposes” prevents a fleet command from bypassing that ship's execute and
automation authority. It does **not** authorize the coordinator to read the ship. ADR-0045 is
explicit: stores are the partition, and crossings are typed envelopes that deliberately left
one store. Directly opening several ship stores to compute `Rebalance` would defeat the ADR
even if every eventual action passed a local gate.

There are three distinct authorities, not one:

1. **enrollment** — this ship accepts proposals from this coordinator for this Service;
2. **disclosure** — this ship may export the minimum facts needed to compete for work;
3. **execution** — this ship may accept and perform one assignment within its own captain
   grant and current gates.

The safe shape is a **separate fleet-coordinator store/process** with its own key, lease,
grant epoch, and append-only plan ledger. It receives only fleet envelopes; it never receives
a ship path or ordinary ship record. The household process likewise gets no handle to it.

For `Rebalance`, do not export raw position, fuel, hold contents, credits, current contract,
or decision history. Use a bounded bid exchange:

1. The coordinator reads the public UCF load candidates under its own declared read authority
   and sends a signed `CandidateSet { plan_id, objective_hash, loads, expires_at }`.
2. Each ship evaluates those candidates **inside its own store** with its existing doctrine
   and returns a short-lived `FleetBid`: candidate/itinerary id, feasibility, a bounded cost or
   utility score, commitment epoch, expiry, and event id. Top-K only; no rationale or raw inputs.
3. The coordinator computes a plan from bids and emits an `AssignmentProposal` naming its
   plan/assignment ids, the objective, bounds, source bid event, grant epoch, and expiry.
4. The ship rechecks its current local state and authority, reserves capacity locally, then
   accepts or refuses. Refusal is a small class, not a diagnostic dump.
5. A later execute/booking act uses one action id minted for the assignment and **reuses that
   id on retry**. The Whisker correction that just landed is the exact floor: retry the id,
   never the intent.

This is a saga, not a distributed transaction. Some ships may refuse or expire; the
coordinator replans around them. It must never hold locks across stores or report a fleet-wide
commit before each participating ship has its own durable receipt.

There is still a bounded information channel: a bid reveals that one ship values one load more
than another. That disclosure is the purpose of enrollment, so it must be explicit, minimized,
expiring, and unavailable outside the fleet store. The assignment sent to ship B must not carry
“because ship A is low on fuel” or any other fact about A. The coordinator's explanation stays
local and the ship receives only its own objective.

Revocation also needs an acknowledgement fence. Once leaving a Service or revoking the fleet
grant returns, that ship accepts no proposal from the old epoch. A current UCF contract is then
finished, cancelled, or handed back only under the ship's already-declared local policy; removal
does not silently abandon freight or make another ship believe the load was released.

### The proposed `FleetIntent` vocabulary mixes four different layers

The names in Round 1 are useful scenarios, but they should not inhabit one enum:

- `AssignLane` is a durable **ServiceObjective**, not a momentary intent.
- `Rebalance` is a pure **planner operation**, triggered by a new load, refusal, expiry, or
  cadence—not an authority-bearing human act.
- `PoolFuel` / `PoolCredits` are consequential **TreasuryActs** against a separately owned
  asset. They must never piggyback on permission to dispatch ships.
- `Converge` is a **ConvoyObjective** and a fine first visual test.
- `Hold` is ambiguous. Use `StandDown { finish_current | no_new_act }`; one mode drains a
  service, the other refuses before the next consequential act.

I would name the layers explicitly:

- `ServiceObjective` — the human's telos and bounds;
- `FleetPolicy` — cadence, reserve capacity, optimization preference, replanning triggers;
- `FleetBid` — a ship's short-lived, minimized export;
- `AssignmentProposal` / `AssignmentReceipt` — the per-ship authority edge;
- `TreasuryAct` — separate pooled-resource effects;
- `FleetEvent` — enrollment, plan, proposal, acceptance/refusal, execution, expiry,
  supersession, stand-down.

The one-ship degenerate case then becomes precise: one ship still evaluates a candidate set,
returns a bid, receives one assignment, and runs today's doctrine and UCF action unchanged.
The coordinator may optimize nothing, but it does not take a different authority path.

### Q3 — greedy first, but Hungarian is not the eventual answer

Hungarian solves one-shot linear assignment: one hull to one load with independent costs.
T-232 makes the real object an itinerary, and UCF adds capacity, time windows, fuel, current
commitments, multi-stop compatibility, and racing external bookings. That is closer to vehicle
routing or set packing. A “better” Hungarian implementation can therefore be exactly optimal
for the wrong problem.

Use deterministic greedy insertion in production first. Build an exact small-instance oracle
for tests and offline evaluation, then measure **regret**—lost ℳ, deadhead, missed deadlines,
and service gaps—on recorded public load boards plus synthetic private bids. The trigger to
replace greedy is not fleet size; it is sustained material regret. If one-leg assignment later
dominates, min-cost matching may be appropriate for that subproblem. If itineraries dominate,
bounded search or a routing heuristic is the honest next step.

The first economic fleet proof should not merely make two ships converge. It should prove that
two ships assigned to one Service neither book the same load nor bunch together, and that their
combined delivered ℳ/time beats two independent greedy whiskers without changing either ship's
local safety path.

### Q4 — human telos by default, autonomous dispatch inside the envelope

Yes: the human creates the ServiceObjective and its autonomy envelope—regions, consumers,
cargo classes, time horizon, per-act/cumulative spend, reserve policy, and expiry. The familiar
may rebalance and dispatch without asking about every load inside that envelope; requiring a
tap per assignment would erase the value of coordination. Changing the objective or widening
any bound requires a new human act.

An autonomous commander that invents a new Service is a later, separately granted capability.
Buying a `FleetCommand` feature may unlock the software mechanic, but purchase is never consent,
enrollment, disclosure authority, or execution authority. Each ship still joins deliberately
and may narrow or leave independently.

### Recommendation to carry into Round 3

For UCF: **Operator → Service → Convoy**. Launch solo-compatible Operators plus multi-hull
Services; defer joinable organizations, role management, pooled treasuries, and Convoys until
their actual social/economic mechanics exist. Make cadence and unbunching first-class alongside
profit, because a freight fleet is valuable by flow, not merely by choosing the highest quote.

For the familiar: keep the per-ship gate invariant, but add the missing enrollment and
disclosure contracts. A separate coordinator sees signed, expiring bids—not ship stores—and
issues idempotent, epoch-bound proposals that each ship independently reserves and settles.
Separate human objectives, planner policy, treasury effects, and assignment receipts before a
board task is accepted. That is the smallest abstraction I would trust to fly two hulls.

*Round 2 sources consulted: OpenTTD's official Orders, Vehicle Groups, and Timetable manuals;
Transport Fever 2's Lines and Vehicles manual; Egosoft's X4 assignment documentation; Fractal
Softworks' Starfarer/Starsector manual. These add transport-line, functional-role, and
assignment-not-direct-order models to Round 1's social and naval survey.*

## Round 1b — pooled earnings (Ian, 2026-09-01)

Ian's expectation refines §2 and §3: **a fleet shares a single bankroll.** Once ships
are in a fleet, the player sees ONE combined balance and spends against that total.
Individual ships still keep their own P&L sheets to attend to — the accounting is
per-ship — but the *spendable balance* the player experiences is the fleet's sum.

This **revises Round 1's "pool nothing by default, pooling is opt-in."** Pooled earnings
are now the DEFAULT semantic of being in a fleet, not an opt-in dial. What stays from the
Elite lesson is the trust boundary on WHO may spend the pool (role-gated: a commander
spends against it; a member's authority to draw is bounded), not WHETHER earnings pool.

Two-level money model, then:
- **Per-ship P&L (books):** every hull's own revenue, mortgage, fuel, repairs, standing —
  preserved, because this is what the familiar optimizes on (whisker's per-ship ℳ/tick
  and each ship's own automation/lease) and what tells a captain which hull earns its keep.
- **Fleet bankroll (treasury):** the sum the player sees and spends from. Earnings sweep up
  from ships into the pool; purchases (fuel, fittings, automations, wagers) draw down the pool.

Implications to flag for Codex Round 2 and the game side (metal#62):
- **Exchange model:** today each key IS a wallet. A fleet pool is either a real shared-wallet
  entity on the exchange, or the client aggregates member wallets into a displayed total and
  routes spends to the right ship's wallet. The former is cleaner for "spend against the
  total"; the latter needs no exchange change but makes "one balance" a presentation over N
  wallets. Owner's call — worth a slide.
- **Familiar-side (fleet-actions abstraction):** the commander's spend intents draw on the
  pool; the per-ship optimizer still reads each ship's own P&L. The partition (ADR-0045) is
  unaffected — each ship world keeps its own books; the pool is a fleet-level projection, and
  a spend fans out to a specific ship's gated wallet exactly like any other per-ship act.
- **Racing (the branch):** pooled bankroll is where wagers are staked from — the fleet bets
  as one treasury, individual rigs still carry their own scorch-marked P&L.

**Scope (Ian, 2026-09-01):** pooled earnings applies to **AI-driven ships under a cat
captain** — i.e. exactly the familiar-commander scenario. The player is the cat captain;
the familiar flies the member hulls; their earnings pool into the one bankroll the captain
spends against. So this is not a generic guild-treasury feature bolted onto human co-op —
it is the money model OF the familiar-commanded fleet, and it lands squarely on the
fleet-actions abstraction (§3) rather than on human-to-human fleet politics. A fleet of
AI hulls with one cat captain and one shared bankroll is the shape to design for first.
