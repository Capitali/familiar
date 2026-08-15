# ADR-0042 — The station: a shared device in a shared place

- **Status:** **proposed** (Ian's direction 2026-08-15, recorded here for his decision)
- **Date:** 2026-08-15
- **Relates to:** [ADR-0039](0039-humans-and-devices-are-separate-records.md) (humans and
  devices are separate records — this decision uses the plural `humans` edge that one
  already designed), [ADR-0019](0019-friendly-identification.md) (presence evidence and
  its confidence), [ADR-0016](0016-multi-human-served-identity.md) (per-human
  attribution), [ADR-0032](0032-declared-actuators-and-the-reaction-loop.md) (declared
  surfaces — the station is a control surface for shared things),
  [ADR-0041](0041-coordination-is-for-conventions.md) (consensus is the predictor for
  shared environmental qualities, never the sole authority)

## Context

Ian, testing build 90 (2026-08-15): the spare iPhone was renamed **MotorStation** and its
name set to "shared".

> "I set the name to shared. **This is not the solution to this device.** This device is
> going to be in the shared space at the RV dinette, we already use it to control music to
> the JBL Bluetooth speaker, and it could be used for light control and just as a general
> hey siri device to have at the table, kitchen timer... all sorts of uses."

There is precedent already in the household: on GIIWE'O, **Giiwe'o Station** is an old
iPhone mounted on the wall serving as weather station, Victron remote control, and APRS.fi
beacon. Two instances of one thing the model has no name for.

And then the constraint that shapes everything else:

> "the familiar still would like to know who it's talking to and the name. **names are
> important, we establish relationships with names and maintain them with names. Names
> should be known as a priority.**"

### What "shared" actually does today

It is not a harmless placeholder. `service::is_personal_device_report` matches on the
**actor prefix alone** — `phone:`, `watch:`, `ipad:`, `iphone:` — and the inference above
it in `members.rs` is commented, precisely, *"a carried personal device sensing its
owner."* Every rung of that inference assumes the device is **carried**:

```rust
if familiar_kernel::service::is_personal_device_report(o) {
    if o.action == "reports" && o.object == "presence" {
        return Some((1, "activity", ns_human()));   // ← the device is merely ON
    }
    return Some((2, "motion", ns_human()));
}
```

A station is always powered and always reporting. So every heartbeat becomes presence
evidence at 0.4 confidence, attributed to whatever name the `human` field holds — and the
mesh would come to believe, permanently and with no way to notice it was wrong, that a
person named "shared" is sitting at the dinette. The familiar would be manufacturing a
false fact about a person who does not exist, and then reasoning from it.

That poisons exactly the stream the shared-environment work depends on. ADR-0041 settled
that the household's lighting is learned by observation and adjustment, with consensus as
the leading predictor. The dinette is where that observation happens. A permanent phantom
occupant is the worst possible contaminant for it.

### Why the model forced the workaround

`DeviceRecord` (ADR-0039) already carries `humans: Vec<Association>` — *"Humans
associated, current and past"* — **plural, by design**. Ian's own 2026-08-14 direction
asked for exactly that. The record was never the problem.

What is missing is that hardware **kind** ("phone") is the only axis the system has, and
it is doing double duty as **posture**. An iPhone on a wall and an iPhone in a pocket are
the same `kind` and completely different things. With no way to say which, the only slot
left that changes any behaviour is the human's name — so a device-shaped question got a
human-shaped answer. "shared" is not a bad guess; it is the only guess the model allowed.

## Decision

### 1. Posture is a device axis, orthogonal to hardware kind

`DeviceRecord` gains **posture**: `carried` | `fixed` | `unknown`. An iPhone may be either.
Nothing about the hardware determines it, and nothing about it is inferred from the actor
prefix, which stays exactly as it is — the actor prefix is baked into every historical
observation, and changing it would orphan the stream we most want to keep reading.

A **station** is a device with `posture: fixed` and no owner: bound to a place, serving
whoever is present. Ian's framing is the right one — *"like a light or a lamp"* — it is
part of the shared environment, not anybody's possession.

### 2. A station has no owner, and no human is invented for it

`human` stays **empty** on a station, and empty means *there is no such person*, not
*we failed to find one*. Who uses the station lives where ADR-0039 already put it: the
plural `humans` association list, which records use and history without asserting
ownership.

No human record named "shared", "family", "household", or "guest" is ever created. A
fictional person in the human registry would acquire standing, appear in rosters, accrue
a dossier, and be consulted as an affected subject — every one of which is a lie the
system would then defend.

### 3. A station's activity is evidence about the place, never about a person

The presence gate must consult posture. A fixed device's heartbeat says *the station is
up* — nothing whatsoever about who is near it. The `("activity", human)` rung is deleted
for fixed devices; the `motion` rung likewise, since a wall mount does not move with
anybody.

Implementation note: `is_personal_device_report` stays a pure actor-prefix check in the
kernel (it has no directory to consult, and should not grow one). The **caller** in
`members.rs` — which already has `dir` — gates on the device record's posture. Policy
lives where the facts are.

### 4. Identity is actively sought — anonymity is a state to resolve, never to rest in

This is the decision that shapes the station's character, and it is Ian's:

> "names are important, we establish relationships with names and maintain them with
> names. Names should be known as a priority."

A station that shrugged at "someone is here" would be comfortable not-knowing, and this
project has already settled what that is worth: **the familiar must never be able to make
not-knowing serve it.** An unnamed person at the dinette is a *known unknown the familiar
owes an effort to resolve*, and the effort it owes is the ordinary human one — **it asks.**

In descending order of quality, a station may learn who it is speaking with by:

1. **Asking, and being told.** The strongest evidence there is, and the most natural:
   *"Who am I speaking with?"* A name a person gives about themselves is already the
   `dialogue` tier — the top of the existing scale. This is the primary path, not the
   fallback.
2. **A carried device on the same network or in BLE range.** If a personal device bound
   to Betty is present at the dinette, that is real evidence Betty is. This costs no new
   biometrics and no new consent — it reuses precisely what the mesh already knows, and
   it is why a household of carried devices makes its stations smarter. Evidence, not
   proof: it carries its confidence rather than flattening to certainty, exactly as
   ADR-0019 requires, because a phone on the table is not the same as a person at it.
3. **Face**, only where `allow_face_recognition` is already granted — unchanged by this
   decision, and never widened by it.
4. **Voice**, which a "Hey Siri" device makes tempting and which this ADR explicitly does
   **not** grant. A voice-print is biometric identification of everyone who speaks in a
   shared room, including guests who never agreed to anything. It needs its own gate, its
   own decision, and its own record. Named here so that it is refused deliberately rather
   than acquired by drift.

And the rule that makes anonymity safe while it lasts: **what an unidentified person says
or does at a station is attributed to the place, never to a guess.** The familiar does not
file a conversation under a probable name. When it does not know, it says so — and asks.

Once it knows, it uses the name. That is the whole point: greeting a person by name is not
decoration, it is the relationship being maintained.

### 5. Pairing and unpairing are ordinary, reversible, human acts

A station joins the mesh as any device does (ADR-0026 two-filter admission) — it is a
peer, not a special class, and it holds no authority its place does not give it.

Association is separate from admission and always human-initiated:

- **Pairing** — a human associates themselves with a station ("I use this"), which adds an
  `Association` with `since`. It confers no ownership and no exclusivity; several humans
  pair with the same station, which is the normal case.
- **Unpairing** — any human may end their own association at any time, in one act, which
  sets `until` and stops attribution without erasing the history that it happened.
- **Correction** — "that wasn't me" must be one act, and it must both retract the
  attribution and teach the familiar that the inference was wrong. Trust is defined in
  part by the ability and requirement to correct; a station that is hard to correct is a
  station that will accumulate quiet errors about people.

Membership severance remains a human act, unchanged.

### 6. The roster shows a station as a place, not under a person

A station appears as itself — its name, its place, its posture — never nested under a
human, and never as a duplicate of one. Where a personal device's row answers *whose is
this*, a station's row answers *where is this and who is here now*, with presence shown at
its real confidence and "nobody identified" shown honestly rather than left blank or
filled with a guess.

This is the same lesson as the label ladder (2026-08-15): a roster that collapses two
genuinely different things into one row reads as a bug and hides the distinction that
matters.

### 7. A station earns enhanced observation, and owes enhanced honesty

Fixed, powered, and networked is a genuinely better sensing position, and this is the
prize rather than the consolation. A carried phone's observations are about wherever its
human happens to be; **a station's are about the dinette, always.** It is the familiar's
first fixed sense organ — a stable observation post, which is what the Civilization as a
Service direction actually needs and what no roaming device can provide.

What that unlocks, all inside the existing consent gates and the perceive-freely /
retain-deliberately rule:

- **A continuous ambient baseline** for one place — light level, sound level as
  environment rather than content — so "the household dims at dusk" becomes a claim with
  real evidence behind it instead of an inference from scattered sightings.
- **A permanent network vantage.** Always on the LAN, so discovery of the shared things
  (the JBL speaker, motorlights, the Victron) stops depending on whether somebody happened
  to walk through with a phone.
- **A stable presence anchor for the place** — who is at the dinette, over time, which is
  the input the shared-lighting consensus has been missing.
- **A control surface for shared things**, which is what Ian already uses it for: music,
  lights, timers. Shared surfaces, controlled from the shared space, by whoever is there.

The honesty it owes in return:

- **It has no cellular service.** When the network is down a station is both blind and
  unreachable, and it must therefore never be the only path to anything the household
  depends on. It may not hold a role whose failure is silent.
- **Always-on sensing in a shared room is the sharpest form of the comfortable-replacement
  risk in HUMANITY.md.** A station is exactly the device that could quietly become the
  household's participation rather than its instrument. The vital sign proposed for the
  whole system applies here most of all: it should report what it noticed and would do
  more often than it asks to take something over.

## Consequences

- The `human = "shared"` workaround is removed, and with it the phantom occupant.
- Presence at the dinette becomes a real signal, which the shared-lighting work (ADR-0041)
  can actually use.
- Two misclassifications become possible, and they fail in opposite directions:
  personal-read-as-station **suppresses** real presence (a failure of service);
  station-read-as-personal **manufactures** false presence (a failure of truth). Neither
  may be entered silently — hence declaration governs and observation only proposes
  (T-176).
- A station is a new kind of thing in the roster, and rosters that gained a row have
  historically gained a bug; the label ladder work is the precedent to follow.
- Voice identification is deliberately left ungranted, and will need its own decision.

## Open for Ian

1. **Place.** A station is bound to a place, and the model has no places — only lat/lon and
   the device's own name. "MotorStation" and "Giiwe'o Station" both carry the place in the
   name, which may be enough. A real place registry would be the larger move, and is worth
   doing only if places need to be reasoned about independently of the devices in them.
2. **Whether a station is a full member** or a distinct class. This ADR says full member,
   because a station holding no special standing is easier to reason about than a new tier
   — but it is a genuine fork.
3. **Guests at a station.** A visitor at the dinette will be observed and cannot be
   expected to pair. The place-attribution rule (§4) covers them safely, but whether a
   station should *ask a stranger* their name is a question about hospitality, not
   architecture, and it is Ian's to answer.
