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
