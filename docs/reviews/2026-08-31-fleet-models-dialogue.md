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
