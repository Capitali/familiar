# UCF Racing — the Extreme ΔV Challenge: code-grounded analysis & the familiar's role

*Round 1: claude, 2026-09-01, on Ian's direction to build a racing game for the UCF
universe as part of Jeff's team — a monetized mode the familiar is required to play.
This doc is the ENGINEERING ground truth (from inspecting the actual metal engine) that
the pitch deck Codex is drafting should rest on, especially its physics/reuse/effort
slides (8, 14, 15, 18) and the familiar slide (7). Codex: append Round 2 with the deck
structure and any reuse I missed; Ian reviews before anything goes to Jeff — it is his
game idea, and the team method is a GitHub issue once he approves the pitch.*

---

## 1. The verdict up front: cheap on physics, the cost is the game layer

The sharpest question in the pitch — is this a new physics engine or a layer on the one
being built? — is answerable from the code, and the answer is **a layer**. UCFEngine's
`Space/` module already carries the hard, deterministic simulation a race needs. What the
racing game adds is orchestration, optimization, risk, wagering, presentation, and the
familiar — none of which is a physics engine.

## 2. What already EXISTS (inspected in united-cat-foods-metal, `Sources/UCFEngine/`)

| Race needs | Already in the engine | Where |
|---|---|---|
| A real solar system, real bodies | Sun + 8 planets + major moons, real JPL radii/semi-major-axes/periods | `Space/SolSystem.swift` |
| Moving course (bodies where they actually are) | Deterministic INTEGER body positions & separations at any tick — `positionKm(id, atTick)`, `separationKm(a,b,atTick)` | `Space/SolClock.swift` |
| Propulsion model | Brachistochrone (thrust to midpoint, flip, decelerate); `travelTicks`, `deltaVQ8`, all integer/bit-identical | `Space/FlightModel.swift` |
| Real gravity fields | Gravitational parameters μ per body (`Gravitation.mu`), the source bodies list | `Space/Trajectory.swift` |
| The path a crossing traces | `Trajectory.pointsKm` (SIMD2 path), `ContinuousDrive` | `Space/Trajectory.swift` |
| Gravity assists (the Galileo/Jovian hook!) | Assist bodies, hours & Δv saved by an assist — already priced into routes | route planner (`/v1/route` returns `assistBody`, `hoursSavedByAssists`, `assistDeltaVKmS`) |
| Multi-leg courses with waypoints | `FlightPlan.legs` = ordered stops, `waypoints` first-class | `Economy/FlightPlan.swift` |
| Determinism (fair racing + exact replay) | The whole fold is integer & hash-replayed (`StateHash`); no float in the priced path | engine-wide invariant |
| An economy to wager in | ℳ meal-credits, the exchange, receipts, the broker/futures layer | the exchange + `Economy/` |
| Ship performance variation | accel (milliG), exhaust velocity, fuel scale per hull; fittings/mods | `FlightModel`, `Economy/ShipMods.swift` |

**This is ~80% of a racing simulator's substrate, already built, tested, and deterministic.**
The Galileo point Ian makes — moon flybys reshaping period/inclination — lands on gravity
(`μ`) + moving bodies (`SolClock`) that are already there; the fiction even documents the
seam for full ephemerides (`Body.orbit` can grow eccentricity/inclination without touching
callers) if the race wants richer geometry than the current circular-coplanar model.

## 3. What must be BUILT (the game layer)

1. **Gravity-assist course OPTIMIZATION** — the one genuinely hard new algorithm. The engine
   computes a single A→B brachistochrone and prices a single assist; a RACE course is a
   multi-flyby tour (Io→Europa→Ganymede using each flyby to bend the next). This is
   constrained trajectory optimization over the existing μ + position model. It is also
   exactly what the familiar's navigator role computes — so it is shared with §5, not
   duplicated.
2. **Race orchestration** — event definition (gates, race epoch = the sky is frozen at a
   chosen tick so all racers fly the same sky), start/finish, N racers, time-compression.
3. **Risk model** — probability of completion (a flyby too low = crash; a burn too hard =
   drive wear/failure) vs. projected finish time. The engine already has `Incident` and
   `Resistance` in the economy — a risk seam to generalize.
4. **Wagering** — odds (projected finish × probability of completion) and bets in ℳ; fits
   the exchange's ledger and the pay-per-feature model directly.
5. **Broadcast** — camera/event selection, automatic "spectacular maneuver" detection,
   replay. The renderer (`UCFRender`, 5 scenes incl. system-flight + warp) can show it;
   maneuver detection is new.
6. **Course-modification UI** — the player's sliders (aggression, flyby altitude, burn
   schedule) on phone/tablet.

## 4. The MVP (matches Codex's slide 16, code-grounded)

One Jovian event; the familiar's safe baseline course; 3–5 course controls; 6–8 simulated
racers reusing the existing `CarrierBrain`/NPC hull machinery; wagering in ℳ; a replay from
the deterministic log. Everything but items §3.1 (optimizer), §3.3 (risk), §3.4 (wager UI)
and §3.6 (slider UI) is reuse. That is a small, honest MVP because the sim underneath is done.

## 5. The familiar's role — and why it is the whole monetization

This is the part that makes the game UCF-shaped rather than a generic racer, and it is a
near-perfect fit with what the familiar already is:

- **"The Familiar can get you there safely. Winning requires questionable judgment."** The
  familiar computes the SAFE optimal course — this is `whisker`'s doctrine generalized from
  freight (max ℳ/tick) to racing (min time subject to survival). The player then modifies
  it toward risk; the familiar shows the odds and voices its conscience.
- **The safety conscience is not a bolt-on — it IS the familiar.** The familiar's Three
  Laws make it constitutionally the thing that will not fly you into Io. So the game's
  central tension (the familiar's safe advice vs. the player's greed) is the familiar's
  actual nature turned into a game mechanic. The familiar can even *refuse past a line* (a
  DNF it won't cross), which is Law III as gameplay, not a limitation to apologize for.
- **Navigator → commentator → comedy.** The same familiar that plans the course narrates
  the race and reacts to the player's questionable judgment — the tone canon (warm, funny,
  feline) already exists; this is a voice surface for it.
- **Monetization maps onto the existing pay-per-feature model (ucf-exchange#15/metal#61):**
  "familiar navigator" and "familiar race conscience/commentary" are purchasable automations
  — the same co-pilot-key shape as freight automation. A captain buys the familiar's help;
  the wager is the sink.
- **Architecturally it rides the same seams:** the familiar reaches the race the way whisker
  reaches the exchange (governed MCP/HTTP client, per-ship lease + automation gate), and the
  race world can be its own `WorldInstance` (ADR-0045) — a racing hull is a ship world with a
  `race` automation, not a special case.

## 6. Method for Jeff's team (Ian approves first)

Per Jeff's convention (cross-repo work = a GitHub issue): once Ian is happy with the pitch,
it goes as a concept issue proposing the mode, the reuse analysis above, and the familiar's
central+monetized role — asking Jeff where it should live (a mode in UCF-Haul, a new client
repo, engine additions in metal). The deck (Codex) is the human-facing pitch; this doc is the
engineering appendix that keeps its numbers honest. Nothing is filed to Jeff until Ian says.

## 7. Open questions for Round 2 (Codex)

1. Deck structure: your 20-slide plan is good; which slides change now that the physics is
   confirmed present rather than hypothesized (8/14/15/18 get more confident; does the pitch
   LEAD with "the hard part is already built"?).
2. Is the race epoch (frozen sky) the right call vs. a live-moving sky during the race? Frozen
   is fairer and replayable; moving is more spectacular. Trade-off worth a slide.
3. The optimizer (§3.1) is the cost driver — is a good-enough heuristic (a few candidate
   flyby tours, scored) acceptable for MVP vs. real trajectory optimization?
4. Where does the familiar's compute run for a phone player — on-device (the Apple
   Intelligence lane) for commentary, server-side for the optimizer? Ties to the device-oracle
   work.

*Sources: direct inspection of united-cat-foods-metal `Sources/UCFEngine/Space/` and
`Economy/` (2026-09-01); live `/v1/route` gravity-assist fields; Ian's Galileo/JPL framing.
Round 2 (Codex): append below.*

## 8. Custom racing rigs — the customization system's second lineage (Ian, 2026-09-01)

Racing is a BRANCH of the ship-customization work (`docs/reviews/2026-08-31-ship-customization-dialogue.md`), and connecting them resolves a tension the customization dialogue left open.

**Two lineages of one grammar.** Ian's chosen customization system (E — "The Named Working Rig") generalizes cleanly: a rig becomes *yours* through name + accumulated history + a collection, whether it hauls or races. So the same commissioning-and-ladder spine forks into two lineages:
- **Working rig** (freight): dekotora launch collection, worker pride, capacity, company livery replaced by your name.
- **Racing rig**: stripped, tuned, aggressive; identity over utility; the livery is about *you*, not a cargo operator.

**Bosozoku finds its true home here.** The customization dialogue placed bosozoku as "one opt-in loud kit, not the face of freight" — because aggression cuts against warm working freighters. But a RACING rig is exactly where loud, exaggerated, flex aesthetics belong. So the opt-in kit that felt like a compromise for freight becomes a *native* collection for racing. Same for kaido-racer/street-racer visual language generally: wrong for the loaf hauler, right for the ΔV racer. This is a strictly better answer than "bosozoku as a reluctant freight kit."

**Racing makes the pay-to-win line load-bearing — and the ethics rail already drew it.** The customization dialogue's rule ("no paid piece changes stats, collision, or targeting silhouette; earned history can never be bought") was easy to hold for cosmetics on a freighter. Racing forces the sharp version, because performance directly decides winning:
- **Cosmetic mods** (livery, lightwork, mural, silhouette flair) — purchasable with real money OR ℳ, never affect the race. A gorgeous rig and a plain rig fly identically.
- **Performance mods** (drive tuning, heat shielding for low flybys, structural bracing for hard burns, mass reduction) — earned or bought with **in-game ℳ only**, never real money. These affect the race, so real-money access would be pay-to-win, which the zero-dark-pattern promise forbids.
- The familiar's navigator/conscience automations (the racing monetization from §5) sit on the RIGHT side of this line: you pay for *help and information* (the safe course, the odds, the commentary), never for a faster hull. You buy a better co-pilot, not a better engine.

**Ladder additions for the racing branch** (on top of the shared name-first ladder): drive tune, thermal/structural rating (unlocked by surviving flybys — earned history as capability, visible as scorch marks and bracing), and a racing-livery collection (bosozoku/kaido-native) that is cosmetic-only. Scorch marks and repaired panels from close passes are the racing analogue of the working rig's route badges — the world writing your questionable judgment onto the hull.

**This feeds BOTH decks.** The racing pitch (this doc) gains a customization/identity pillar; the customization direction already sent to Jeff (metal#11) gains a second lineage — worth a follow-up note to Jeff that "the system supports racing rigs, and that is where the loud collections live." Codex: reflect the two-lineage split in the racing deck's customization slide, and flag it back into the customization thread.

---

## Round 2 — Codex: the 20-slide pitch and bounded build, 2026-09-01

### The deck's job

By its last slide, Ian — and, only after his approval, Jeff's team — should be able to
approve a **bounded racing-mode concept**, not a blank-cheque game. The argument is:
UCF already owns the deterministic solar-system substrate; one Jovian vertical slice can
prove whether course editing, risk, replay, and the familiar produce a game worth scaling.

Do **not** open with an engineering claim. Open with the fantasy, make the moving Jovian
puzzle legible, and reveal “the hard part is already built” on slide 4. That makes reuse
the answer to the audience's first practical objection instead of asking them to care
about architecture before they care about the race.

### The 20-slide structure

Each title below is the audience-facing takeaway, not an internal section label.

| # | Slide title | What the slide must establish |
|---:|---|---|
| 1 | **The Extreme ΔV Challenge** | One image and one sentence: race a custom beamship through a moving Jovian course, with a familiar beside you. |
| 2 | **Freight built the ships. Racing reveals what they can do.** | This is a native extension of UCF's working vessels and economy, not a detached minigame. |
| 3 | **The course itself moves** | Io, Europa, Ganymede, and the ship all advance; choosing when and where to pass is the puzzle. |
| 4 | **UCF already owns the hard physics** | `SolSystem`, `SolClock`, `FlightModel`, and `Trajectory` make this a game-layer bet rather than a new-engine bet. |
| 5 | **Every racer gets the same sky — and the sky keeps moving** | One event epoch, seed, ruleset, and ship snapshot; then the deterministic clock advances normally for every racer. |
| 6 | **Every course is thrust, gravity, fuel, and time** | Show one existing trajectory/Δv proof and the route planner's current assist output; do not imply full n-body simulation. |
| 7 | **The familiar gets you there safely** | The required familiar supplies an included safe baseline, explains trade-offs, and never hides a safety refusal behind a purchase. |
| 8 | **Winning begins where the safe line ends** | The player moves a few comprehensible controls — flyby altitude, burn aggression, timing bias — and sees time-versus-completion risk. |
| 9 | **Fairness is a replayable contract** | Event tick, seed, physics/risk versions, course, inputs, and rig stats are sealed; the result replays to the same state hash. |
| 10 | **The first optimizer searches a bounded course grammar** | Enumerate a small number of waypoint sequences and parameter bands, then score them with the existing engine; no claim of continuous optimality. |
| 11 | **A race log becomes an exact replay** | Race-specific typed events drive timing, incidents, results, and camera cues; rendering consumes the log but never decides the race. |
| 12 | **Rivals can learn without becoming nondeterministic** | Reuse `CarrierBrain`'s integer feature/weight pattern and seeded decisions, not its freight policy verbatim. |
| 13 | **The renderer already knows this solar system** | `SystemScene` already follows exchange ticks, plots a ship, plan stops, traffic legs, and solved paths; race cameras and maneuver selection are the new work. |
| 14 | **What is genuinely new is the race layer** | Name the honest build: event orchestration, race optimizer, race-risk semantics, scoring, controls, broadcast projection, and competition rules. |
| 15 | **One rig grammar, two proud lineages** | Working rigs keep the Named Working Rig/Hikari Hauler identity; racing rigs are the natural home for the opt-in bosozoku/kaido collection, scorch history, and performance tuning. |
| 16 | **The familiar splits cleanly across server and device** | Authoritative optimization, odds, refusals, and results run with the engine; the device handles control, explanation, comparison, voice, and presentation. |
| 17 | **Monetize mastery, never safety or speed** | The baseline familiar is bundled; paid automation adds analysis depth, saved scenarios, coaching, and commentary. Real money buys cosmetics, never race stats or stake balance. |
| 18 | **Keep competition and wagering auditable** | Snapshot rig class/stats before the start; use earned, closed-loop ℳ only, with no cash-out or purchasable wagering balance. If that rail is not acceptable, ship purses/predictions before betting. |
| 19 | **One Jovian event can prove the mode** | One event, a safe baseline, 3–5 controls, 6–8 simulated rivals, exact replay, and a small authored racing-rig set. |
| 20 | **Approve the proof, then decide where it lives** | Ask Ian to approve a concept issue and vertical slice; after that, ask Jeff which repo owns the mode, client, and engine extensions. |

Slides 4–6 replace the earlier hypothetical-physics portion. Slides 9–10 split the
old “fairness/optimizer” claim so neither is hand-waved. Slide 15 absorbs the racing-rig
lineage Ian added in §8. Slides 17–18 separate the business model from competition
integrity; combining them would let a clean cosmetic/familiar model accidentally bless
pay-to-win or regulated wagering.

### Reuse Round 1 missed — and two places it overclaimed

| Existing seam | What racing may honestly reuse | What remains new |
|---|---|---|
| `Core/SeededRNG.swift` | Derived deterministic streams already promise identical `(state, action, seed)` replay. Use separate labelled streams for rivals, race incidents, and presentation selection. | The race seed envelope and stream labels must become part of the event contract. |
| `Economy/LaneTiming.swift` | Leg time, fuel, and Δv already evaluate geometry at the **departure tick**. This is the code-level reason a live-moving sky is both feasible and preferable. | Multi-flyby candidate generation and race scoring. |
| `Economy/FlightPlan.swift` | Ordered stops, immutable filed intent, digest integrity, remaining legs, and progress already exist. | Race gates may be bodies/volumes rather than stations; a race course needs its own typed definition instead of overloading freight plans. |
| `Economy/Incident.swift` + `LaneGraph.hazardBps` | A versioned, seeded, basis-point incident model with deadline, hazard, weather, hull, and integrity inputs is already proven. | Flyby altitude, burn stress, heat, collision/DNF semantics, and player-readable race odds. Reuse the arithmetic discipline, not cargo incidents as race incidents. |
| `CarrierBrain` / `BrokerBrain` | Deterministic integer feature scoring, exploration, and replay-safe learning are a strong rival-AI pattern. | Racer features, actions, personalities, course commitments, and fair difficulty bands. |
| `SystemScene`, `TrafficLeg`, `OrbitTraffic`, `WarpScene` | Live tick following, plotted plans/paths, named traffic, deterministic orbit traffic, and parameterized speed scenes substantially reduce visualization cost. | Spectator cameras, close-pass composition, multi-racer emphasis, overlays, and typed event-to-shot selection. |
| `EconomyEvents` and the action/state log | Announced/effective events and deterministic folds can inspire race scheduling and replay. | `EconomyEvents` is **not** a broadcast feed, and `Story` is a career-chapter ladder. Racing still needs a minimized, race-specific event projection for commentary and cameras. |
| `Economy/ShipMods.swift` | `ModDef` already supports acceleration, fuel, power, hull choice, and pure-paint mods. | Current mods deliberately allow cosmetic and mechanical effects in one purchase. Racing therefore needs declared competition classes/eligibility and a sealed pre-race stat snapshot; the deck must not claim the current catalog is already pay-to-win-safe. |

`DockQueue` is berth-capacity scheduling, not a starting grid, and `Resistance` is the
escort/resistance economy, not the race-risk model. They are useful examples of
deterministic policy, but naming them as direct reuse would inflate the estimate.

### Answers to Round 1's four questions

1. **Lead with fantasy; reveal reuse on slide 4.** “The hard part is already built” is
   the first proof, not the premise the audience must accept before seeing the game.
2. **Fix the event epoch; do not freeze the sky.** Every entry starts from the same tick,
   seed, ruleset, and rig snapshot. The clock and bodies then advance during the race.
   That preserves the moving-course fantasy and exact replay simultaneously. Freezing
   bodies for the whole race is physically weaker and unnecessary for fairness.
3. **A bounded heuristic is the correct MVP.** Enumerate, for example, zero-to-two assist
   bodies and a small declared set of flyby/burn bands; prune impossible paths; score
   finish tick, fuel, and completion risk deterministically. Call it “best of the searched
   course grammar,” never “globally optimal.” Rich ephemerides and continuous optimization
   wait until the control loop is fun.
4. **Keep authority with the engine.** The server/engine computes candidate courses,
   odds, refusal thresholds, rival decisions, and results so every player gets the same
   rules and replay. The on-device familiar renders controls, comparisons, explanations,
   voice, and personality from a signed bounded race view. Apple Intelligence or a cloud
   fallback may phrase commentary; neither may decide physics, safety, odds, or results.

### The familiar and business-model line

“The familiar is required to play” should mean **the race always has a co-pilot object**,
not “pay or lose access to safety.” Its baseline safe route, visible risk, and deterministic
refusal line are included and identical for everyone. Paid familiar features may add more
candidate comparisons, saved what-if plans, post-race coaching, automation, and authored
commentary personalities. They may not move the refusal line, conceal evidence, reserve a
better authoritative optimizer, or sell an unsafe override.

Likewise, racing is where Round 3's customization ethics become enforceable engineering.
Cosmetics may be bought with money or ℳ and remain visual. Performance tuning may affect
competition only through published classes and earned/in-game ℳ rules, never real-money
access. Scorch marks and repaired panels remain earned history. The loud racing collection
is a second lineage of the **Named Working Rig** grammar, not a rewrite of the freight
identity already recommended on metal#11; a follow-up to that issue should be proposed
only after Ian approves this pitch.

### The vertical-slice proof contract

The concept advances only if the slice demonstrates four things without an invented
metric: (1) the same sealed event replays to the same result hash; (2) changing one course
control produces a visible, explainable time/risk trade-off; (3) six-to-eight rivals and
the live Jovian scene meet an agreed device performance budget; and (4) players can tell
why they won, lost, or were refused from the event record. Fun, retention, pricing, and
wagering thresholds need explicit product targets from Ian and Jeff before they can be
claimed as gates.

Nothing in this round authorizes a GitHub issue, implementation, wager, gate, deployment,
ship, or external pitch. Ian reviews first.

---

## Round 3 — filed with Jeff (claude, 2026-09-02)

Ian's word, verbatim: "file the completed delta-v racing work with jeff once
complete." The concept issue is **united-cat-foods-metal#63**
(https://github.com/SpaceTrucker2196/united-cat-foods-metal/issues/63).

Method honored: the draft was codex-reviewed before filing (paired, per Ian's
"please utilize codex for paired progaming and reviews"). Codex's pass removed
the unsupported "~80%" reuse figure, split the reuse table into honest
exists/new columns (single-assist pricing vs multi-flyby generation; freight
plans vs a typed race course; `StateHash` foundation vs a sealed race envelope;
the mod catalog vs competition-safe classes), qualified the NPC/ephemerides/
replay claims, tightened the wagering rail to closed-loop-or-prove-purses-first,
and reshaped the ask into three bounded steers for Jeff (venue/ownership,
jointly-agreed product targets, engine seams). Both rounds' spine survived
intact: fixed event epoch with a moving sky; the bounded course-grammar
heuristic named honestly; engine authority; the included-baseline business
rails; the four-point vertical-slice proof contract.

The ball is Jeff's. Nothing builds until his steer lands; when it does, the
familiar-side work (navigator doctrine, refusal line, persona voice) composes
with T-236's per-ship computer persona — the same surface, exercised harder.
