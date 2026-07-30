# ADR-0023 — Speech: presence first, identity as a separate plan

- **Status:** accepted (design) — presence half partially built, identity half not started
- **Date:** 2026-07-30
- **Relates to:** [ADR-0019](0019-friendly-identification.md) (the identification ladder speech plugs
  into), [ADR-0024](0024-face-as-an-identity-provider.md) (the same shape, for faces),
  [ADR-0022](0022-the-human-dossier.md) (what presence and identity feed),
  [ADR-0005](0005-human-owned-capability-boundary.md) (the microphone is a gate a human owns),
  [ADR-0016](0016-multi-human-served-identity.md) (biometrics are sensitive-personal and never
  federate), `SPEC.md` R10, `ios/App/Sources/VoiceSensing.swift`

## Context

ADR-0019's ladder identifies a human from a device binding, a face check, or by asking. Ian's
direction (2026-07-30) is that **presence, speech and facial recognition should be the primary,
passive means**, with asking as the last resort. Speech is currently the least developed of the
three and is not in the ladder at all.

What exists: `VoiceSensing` runs `SFSpeechRecognizer` with `requiresOnDeviceRecognition`, **push to
talk** — the human deliberately starts it — and emits the utterance as a `said` observation. The
routing module already scores a human speaking to the familiar at 0.9, the strongest ordinary
presence evidence short of a recognised face.

Two things are missing, and they are **different problems with different risk**, which is why this
ADR splits them.

## The constraint that shapes everything below

Apple ships **transcription** on-device. It ships **no public speaker-identification API** — there
is no supported way to ask "whose voice is this?". Identity from voice therefore requires a bundled
speaker-embedding model and a matching layer we own, exactly as face identity does
([ADR-0024](0024-face-as-an-identity-provider.md)).

So *hearing that someone spoke* and *knowing who spoke* are separated not by ambition but by what
the platform provides. Treating them as one feature would have hidden a model-sourcing project
inside a sensing change.

## Decision

**Speech becomes a presence provider now, and an identity provider later, as two separate pieces of
work with separate consent.**

### Phase 1 — presence, and the part that is not yet honest

Speech is strong presence evidence: a person speaking is present and *engaged*, which is more than
a motion beacon can say.

But the current signal is **push-to-talk**, and a human who pressed a button was never in doubt.
For speech to be a *passive* presence signal in the way Ian's direction requires, the microphone
would have to be listening when nobody asked it to — which is a different act, and the one people
most reasonably fear.

The decision is therefore:

- **Passive speech presence detects that a voice is present, not what it said.** On-device voice
  activity detection, yielding "a human is speaking here" with no transcript, no content, and
  nothing retained. This is the lightest possible footprint that still answers the presence question.
- **Transcription stays deliberate.** Turning "someone is speaking" into words remains push-to-talk
  or an explicit wake act. Ambient presence detection must never quietly become ambient
  transcription.
- **Its own gate, separate from `allow_microphone`.** A human who opened the mic for push-to-talk
  did not thereby consent to passive listening. A gate that already exists is not consent for a
  capability that did not exist when they opened it.

### Phase 2 — identity, later and separately

Speaker identification is a **biometric**, in the same class as a face signature: strongly sensitive
under `SPEC.md` R10 and `docs/design-orientation-and-mesh.md`. It therefore inherits, without
argument, every rule that already governs face signatures:

- its **own** opt-in, above passive presence and above transcription;
- **node-local**, never federated under any sharing setting;
- **correctable and never sticky** — a wrong voice link is replaced, not appended to;
- **verification, not search** ([ADR-0019](0019-friendly-identification.md)) — check the voice
  against the expected human or the small scoped set, never scan the registry.

It plugs into the ladder as a peer of the face check, not above it. When both are available and
disagree, the ladder's existing rule holds: **we know less than we thought**, and it falls to
asking rather than picking a winner.

**This phase does not start until Phase 1 is shipped and the model question is answered.**

## Consequences

**Good.**

- The familiar gains a passive presence signal that works in the dark, around corners, and without a
  camera — the cases where face detection is useless.
- Splitting the phases keeps a model-sourcing project from hiding inside a sensing change.
- Voice-activity-only presence is a genuinely cheap privacy posture: no words leave the moment.

**Bad, and accepted.**

- **Passive listening is the most alarming capability in the system to a reasonable person**, and no
  amount of "it's only VAD" changes how it feels. It must be visibly gated, visibly indicated while
  active, and off by default.
- Phase 2 depends on a speaker-embedding model we do not have and Apple does not provide. Until one
  is sourced, speech contributes presence strength and no identity — and the code must say so rather
  than appear to support it.
- Voice presence cannot distinguish a person in the room from a podcast. Some corroboration
  (a paired device present, a face, an interaction) will be needed before a voice alone should move
  a presence claim far.

## Follow-on work

- On-device VAD: which API, what it costs in battery, and how it is indicated while running.
- The separate gate, its wording, and its default (off).
- Phase 2 model sourcing — the same open question as `StubFaceEmbedder`.
- Decide the corroboration rule before a voice alone is allowed to establish presence.
