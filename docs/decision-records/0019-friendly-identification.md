# ADR-0019 — Friendly identification: knowing who is present, so the familiar can address a person

- **Status:** accepted — the ladder and presence claims are implemented client-side and carried on
  the wire (2026-07-29). What remains is listed under **Follow-on work**. **Amended 2026-08-01**:
  the invariant is restated by [ADR-0026](0026-two-filter-admission.md) — see the amendment at
  the end.
- **Date:** 2026-07-29
- **Relates to:** [ADR-0016](0016-multi-human-served-identity.md) (served identity
  and per-human attribution — this ADR says *how* the served human is
  established), [ADR-0005](0005-human-owned-capability-boundary.md) (the
  capability boundary, which this must never become part of),
  [ADR-0015](0015-automated-covenant-admission.md) (node admission — a different
  act on a different subject), `dataflows/served-identity.md`,
  `crates/kernel/src/identity.rs`, `ios/App/Sources/FaceSensing.swift`,
  [ADR-0022](0022-the-human-dossier.md) (what identification is ultimately *for* —
  remembering a person well enough to anticipate serving them),
  [ADR-0023](0023-speech-presence-then-identity.md) and
  [ADR-0024](0024-face-as-an-identity-provider.md) (the providers that feed the ladder)

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

## What is implemented

- **`DeviceRole`** (`personal(owner)` / `shared`), defaulted by hardware kind — a phone and a watch
  are personal, an iPad and a Mac are shared — and overridable by a human. A personal device that
  already knew who it served seeds its binding from that, so nobody states the same fact twice.
- **The ladder** (`Shared/Sources/Identification.swift`) as a pure, platform-free function: hand it
  what each rung knows and it returns a claim. 14 cases verified, including the one that matters —
  a contradicted binding falls to *unknown* rather than asserting the device's guess over the camera.
- **`PresenceClaim`** with `via`, `confidence`, `since` and `expires`, and per-rung lifetimes: a
  binding outlives a face sighting (the fact under it is durable), and an inherited claim is the
  shortest-lived (a watch on the charger is not its owner).
- **`FaceRecognizer.verify(_:against:)`** — the 1:1 path. `FaceSensing` now checks the prior instead
  of searching the registry, and reports agreement *or* contradiction as distinct outcomes.
- **The claim on the wire**: `present_via`, `present_since` and a new `present_confidence` on
  `MemberStatus`, so a 0.7 binding is never mistaken downstream for a human's own 1.0 answer. An
  expired claim reports nobody rather than the last person seen.
- **Attribution** follows the live claim, falling back to the persisted served human.

## Amendment, 2026-07-30 — the two gaps are closed

**Rung 2 now checks a scoped, decaying prior.** "Always ask on a shared device" was faithful to this
ADR and unusable on a galley iPad. Searching the whole registry was never the alternative — that is
where false links come from. The middle, and the one that fits
[ADR-0018](0018-lighthouse-single-fixture.md)'s transience principle, is a **bounded set that
expires**: the bound owner first, then the handful of people this device has actually seen in the
last fortnight, capped at three. Nobody is permanently "the person at this iPad"; the set reflects
who has been around and empties itself when they stop coming. Each candidate is still a 1:1 check
against someone we have a reason to expect — never a search.

A failure to match a *recent* face is deliberately **not** treated as contradicting a binding. It
means "someone else is here", and conflating the two would demote a perfectly good binding every
time a guest walked past the camera.

**A device's own claim now wins over the daemon's derivation.** The device is closer to the
evidence: it holds the binding, ran the 1:1 check, and heard the human answer. `classify()` reads
the live status directory and prefers a member's own claim, falling back to `derive_presence` for
clients too old to report one and for gossip peers that are not devices. This is safe because
`status::record` already enforces that a member may only place its *own* status — a device can only
ever speak for itself. Derived presence, which states no confidence, is scored by its evidence tier
(face 0.9, dialogue 0.85, motion 0.6, activity 0.4) so the two sources are comparable rather than
silently mixed. `Member.present_confidence` carries it, and the guest projection scrubs the
confidence and provenance along with the name.

## Follow-on work
- Surface `confidence`/`via` in the roster, so "PRESENT ian" reads with how it was established.
- Let a human set the device role and owner from the Device screen (today it is defaulted and set by
  answering the prompt).
- ~~Question/goal routing to a live claim~~ — **done.** `familiar_kernel::routing` derives who is
  present from the observation stream and addresses the open question to them; questions carry an
  `owner`, goals carry an `owner_human` that federates, and a question whose addressee walks out is
  re-addressed rather than left facing an empty chair. Ownership governs who is *asked*; a confirmed
  answer is an ordinary public observation.
- The daemon's own `derive_presence` has no binding tier — it reasons only from observations, so a
  device's claim and the daemon's derivation can still disagree. Reconciling them is unfinished.

---

## Amendment, 2026-08-01 — the invariant, restated for rules-based admission

This record's invariant was written in bold — *identification addresses; it never authorises* —
and [ADR-0026](0026-two-filter-admission.md) changes it, so the change is stated here rather
than left to be discovered. The invariant becomes:

> **A claim addresses; establishment admits.**

What is preserved, exactly as this record demanded: recognition alone never admits anything. A
face match, a voice match, a proposed handle — all of it remains a routing hint carrying a
confidence, cheap to correct, unlocking nothing. "The camera believes this is Betty" still opens
none of Betty's doors, and the sensitive-personal rules (a face signature never federates; a
guest sees no dossier) are untouched.

What changed: under ADR-0026, admission to the mesh is gated on the human identity being
**established by evidence** — a rotation proof, a device voucher, an invite token, or an
introduction made in the mesh's own space. Every establishing class is either cryptographic
continuity or a deliberate human act; the deliberate act moved from a third party approving at
a door to the arriving human (or their own hardware, or their inviter) producing the evidence.
Identification-as-claim keeps this record's contract in full. Establishment is a different act
with a different artifact, and it is the *only* identification-adjacent thing that authorises —
and only ever the one thing admission grants: membership, never capability (ADR-0005) and never
another person's data (ADR-0016).

The ladder above is unchanged and remains the machinery presence runs on; its `asked` rung is
also how an E4 introduction begins, which is why reviving the dead `confirmPresentHuman` path
is scheduled work rather than optional polish.
