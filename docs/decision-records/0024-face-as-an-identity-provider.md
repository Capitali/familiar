# ADR-0024 — Facial recognition becomes an identity provider, not just a presence sensor

- **Status:** accepted (design) — presence built; identity structurally ready and **inert**
- **Date:** 2026-07-30
- **Relates to:** [ADR-0019](0019-friendly-identification.md) (the ladder this plugs into),
  [ADR-0023](0023-speech-presence-then-identity.md) (the same split, for voice),
  [ADR-0022](0022-the-human-dossier.md) (what identity feeds),
  [ADR-0016](0016-multi-human-served-identity.md) (biometrics are sensitive-personal and never
  federate), [ADR-0005](0005-human-owned-capability-boundary.md) (the camera is a human-owned gate),
  `SPEC.md` R10, `ios/App/Sources/FaceSensing.swift`

## Context

Today the camera answers *"is a person here, and are they looking at me?"* — `face:none`,
`face:present`, `face:engaged`, derived on-device from Vision landmarks, with no frame and no
identity ever leaving. That is a presence sensor, and a good one.

Ian's direction (2026-07-30): facial recognition should be **one of the forms of identity
management that plugs into identity and presence monitoring** — not a separate feature, and not
merely presence.

The machinery is already shaped for it. `FaceRecognizer` stores per-device face↔handle links,
matches by cosine similarity with a conservative threshold, learns only from human confirmation, and
[ADR-0019](0019-friendly-identification.md) added `verify(_:against:)` so it checks an expected
person rather than searching. The confirm-or-correct prompt exists. The consent gate
(`consent.faceRecognition`) exists, separate from plain presence.

**And none of it does anything.** `StubFaceEmbedder.embedding(...)` returns `nil`, so every
recognition attempt falls through to asking. This is deliberate and documented in the source —
Vision offers detection, landmarks and capture quality but **no public face-recognition or
matching API**, verified against Apple's documentation rather than assumed. Rather than fabricate a
match, the pipeline honestly degrades to the interactive fallback.

So the gap between "presence sensor" and "identity provider" is exactly one thing: **a bundled
embedding model.** Everything around it is built.

## Decision

**Face becomes an identity provider by supplying the missing embedder, and by plugging into the
ladder as one provider among several rather than as a special case.**

### The provider shape

Both face and voice ([ADR-0023](0023-speech-presence-then-identity.md)) contribute at two distinct
levels, and the distinction is what keeps the design honest:

| | face | voice |
|---|---|---|
| **Presence** — someone is here | detection + engagement ✅ built | voice activity ❌ Phase 1 |
| **Identity** — *who* is here | embed + verify ⏳ inert, needs a model | speaker ID ❌ Phase 2 |

A provider offers what it can and says so. The ladder consumes candidates with a `via` and a
confidence and does not care which sensor produced them — which is what makes adding voice later a
plug-in rather than a rewrite.

### Rules inherited without argument

A face signature is **strongly sensitive** (`SPEC.md` R10,
`docs/design-orientation-and-mesh.md`), and the existing rules already encode the right posture:

- **Its own opt-in**, above plain presence. Consenting to be *seen* is not consenting to be
  *recognised*.
- **Never federated**, under any `share_identities` setting. It stays on the device that learned it.
- **Learned only from human confirmation** — the system never links a face to a name on its own.
- **Correctable and never sticky** — `learn()` replaces; a wrong link is a normal thing to fix.
- **Verify, never search** — the bound owner or the small decaying set of recently-seen people
  ([ADR-0019](0019-friendly-identification.md)), never the whole registry.

### On sourcing the model

This is the real work, and it is a **decision, not a download**:

- A converted embedding network (MobileFaceNet-class) bundled as Core ML is the obvious route, and
  brings a licensing question, a size cost, and a **bias question** that must be asked out loud —
  face embedding accuracy varies by skin tone, age and gender in ways that are well documented, and
  a household system that recognises some of its people worse than others is failing them
  specifically.
- Whatever is chosen must be evaluated on that axis before it ships, not after. "It works for us"
  is not evidence when *us* is a small and unrepresentative sample.
- Until then, `StubFaceEmbedder` stays. **An honest fallback beats a confident wrong answer**, and
  the current behaviour — degrade to asking — is correct, not a placeholder to be rushed past.

## Consequences

**Good.**

- The cheapest identification path for a shared device: a household iPad recognises the handful of
  people who use it without anyone typing a name.
- The plumbing is done, so the remaining work is bounded and well-defined.
- Verification-not-search plus a decaying candidate set means the accuracy demanded of the model is
  far lower than open-set recognition would need — a meaningful safety margin.

**Bad, and accepted.**

- **Bias is not a footnote.** An identity system that works less well for some household members is
  worse than none, because it produces confident wrong attributions rather than an honest "I don't
  know".
- The gate will be opened by one person on behalf of a household. Everyone whose face is learned
  should know it, and there is currently no surface for that.
- A face signature is the one piece of data here that is irrevocably about a body. Deletion has to
  be real — see [ADR-0022](0022-the-human-dossier.md)'s deletion requirement, which this inherits.

## Follow-on work

- Source and evaluate an embedding model, bias axis included, before shipping.
- Make the provider seam explicit in code, so voice can plug in beside face without a rewrite.
- A surface telling a person their face is learned on a device, and letting them remove it.
