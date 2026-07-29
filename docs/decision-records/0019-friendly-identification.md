# ADR-0019 — Friendly identification: knowing who is present, so the familiar can address a person

- **Status:** accepted (design); implementation follows the current submission push
- **Date:** 2026-07-29
- **Relates to:** [ADR-0016](0016-multi-human-served-identity.md) (served identity
  and per-human attribution — this ADR says *how* the served human is
  established), [ADR-0005](0005-human-owned-capability-boundary.md) (the
  capability boundary, which this must never become part of),
  [ADR-0015](0015-automated-covenant-admission.md) (node admission — a different
  act on a different subject), `dataflows/served-identity.md`,
  `crates/kernel/src/identity.rs`, `ios/App/Sources/FaceSensing.swift`

## Context

A familiar that serves several people has to know **who is in front of it** and
**where a given person currently is**, or it can only ever broadcast. If the mesh
needs to ask Jeff something, it must know that Jeff is present, and at which
device, or the question goes to everyone and is therefore addressed to no one.

Most of the machinery already exists:

| Piece | Where |
|---|---|
| Identity registry — handle, name, relation, `first_seen`/`last_seen`, `interactions`, `face_signature` | `crates/kernel/src/identity.rs` |
| Confirm-or-correct prompt ("Is this Jeff?" / "Who is it?") | `FaceIdentifyPrompt`, `proposedHandle`, `needsIdentification` |
| Face↔handle learning, correctable and never sticky | `FaceRecognizer.learn`, `confirmIdentity` |
| Phone→watch identity propagation | `PhoneWatchLink.sendAddress(… human:)` |
| Per-human attribution on every observation | `DeviceActor.human` |
| `present_human` in the roster | `StatusClient.Member` |

What is missing is the **order** in which they run, and three gaps that order
exposes:

1. **No device binding.** `servedHuman` defaults to `"observer"` identically on a
   personal iPhone and on a shared galley iPad. Nothing represents "a phone has
   one owner; that iPad has none," so the cheapest and strongest signal available
   is unused.
2. **Recognition searches instead of verifying.** `FaceRecognizer.recognize`
   scans every known handle (1:N) and proposes a winner. Across a household this
   is exactly where false links come from, and it is the expensive way to answer a
   question a device usually already knows the answer to.
3. **Presence has no confidence and no expiry.** `last_seen` is a timestamp.
   "Jeff was at this iPad forty minutes ago" and "Jeff is here now" are
   indistinguishable — and routing needs precisely that distinction.

## Decision

### The invariant, first

**Identification addresses; it never authorises.**

This is friendly identification, not authentication. Its output is a *routing hint
carrying a confidence*, and it must never become an access-control decision. The
system already holds things worth protecting this way: sensitive-personal
observations (heart rate, precise position, face signatures) are attributed
per-human and held node-local, and a face signature never federates under any
sharing state. If identification ever gated access, then "the camera believes this
is Betty" would begin unlocking Betty's body data to whoever happens to be
standing there. It must not.

Corollaries:

- Confidence **travels with** the attribution. An observation attributed at 0.6 is
  recorded as such, never flattened into "Jeff did X."
- A wrong identification is **cheap and immediate** to correct, and correcting it
  is a normal act, not an administrative one.
- Nothing is ever *unlocked* by being recognised. Consent gates stay where they
  are: owned by the human, per device, per sense.

### The ladder

On a human becoming present at a device, run the cheapest rung that applies. Each
rung only runs when the one above is unavailable or contradicted.

1. **Device binding.** A device declares a role: `personal(owner)` or `shared`. A
   phone or a paired watch is personal by nature — one person carries it. If bound,
   the owner is the prior, at high confidence. On a phone this is usually the whole
   answer, and **no camera is involved at all**.
2. **Verify the prior (1:1).** If face consent is on and a signature exists, check
   the face *against the bound owner* — "the device says this should be Jeff; does
   the face agree?" Agreement raises confidence. Contradiction drops to rung 3. It
   never searches the registry.
3. **Ask.** The existing confirm-or-correct prompt. With a prior: "Is this Jeff?"
   Without one: "Who's here?" A human answer is authoritative and is learned.
4. **Don't guess.** Fall back to `observer`. An unidentified device is simply not a
   delivery address for anything personal. Silence is the correct behaviour, not a
   best guess.

### Presence claims

Each pass emits a **presence claim** that decays:

```
{ handle, device, confidence, via: binding|face|asked|inherited, since, expires }
```

Routing consults **live claims**, never `last_seen`. A claim that has expired means
the familiar does not know where that person is — which is a fact worth having and
worth saying, rather than papering over.

This upgrades the roster's `present_human` from a bare string to a claim, and it
is what lets the mesh answer *"where is Jeff right now, and how sure are we?"*

## Consequences

**Good.**

- The familiar can address an individual, which is the precondition for
  [owned questions and routing](../dataflows/the-cognitive-cycle.md) — a question
  reaches the person who can answer it.
- The common case gets cheaper, not more expensive: a personal phone identifies its
  human with no camera, no model, and no prompt.
- 1:1 verification is materially more accurate than 1:N search, so the sharper
  sensor is used for the easier question.
- Degrades honestly. With face consent off, rungs 1, 3 and 4 still work.

**Bad, and accepted.**

- A device role is new state that a human has to be able to set and change — a
  shared iPad that becomes someone's, or a phone that changes hands, must be
  correctable without ceremony.
- Presence claims add a clock. Expiry windows are a tuning problem, and too short is
  as bad as too long: a familiar that keeps re-asking who you are is worse than one
  that occasionally addresses the wrong person.
- Inherited claims (phone → watch) can outlive the truth — a watch on the charger
  is not its owner. `via: inherited` exists so that case is visible and can carry a
  shorter expiry.

## Follow-on work

- Device role + owner in the client model and the roster.
- `FaceRecognizer.verify(embedding, against: handle)` — the 1:1 path.
- Presence claims with confidence and expiry, replacing bare `present_human`.
- Question/goal routing to a live claim (see the owned-question work).
- Update `dataflows/served-identity.md`, whose "Who is present?" diagram currently
  shows face → pick → default as three flat alternatives rather than a ladder.
