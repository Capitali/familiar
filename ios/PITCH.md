# The Familiar — a pitch for the people who'd carry it

> Written for iPhone and iPad users. Every claim here traces to code or docs in this
> repository — the pitch follows the project's own constitution and does not overclaim
> ([Law III](../docs/SOUL.md)). Engineering details: [README.md](README.md) ·
> [the root docs](../docs/00-overview.md).

## The one-breath version

**A familiar is a small, private intelligence that lives on a machine you own — and
your iPhone, iPad, and Apple Watch are how it senses, thinks, and speaks with you.**
No account. No cloud between you and it. No feed. Nothing is sensed until you switch
it on, sense by sense, and every switch turns back off.

## The problem with the intelligence you carry

Your phone is the most personal object you own, and most of the intelligence in it
reports to someone else. Assistants live on a company's servers; your patterns go up,
predictions come down, and attention is the rent. Even the polite ones are tenants of
someone else's business model.

The Familiar inverts the arrangement. The mind lives on *your* Mac. Your devices don't
report to a platform — they enroll into a small mesh that answers to you, bound by a
written constitution, with its source in the open (Apache-2.0) so you can read every
line of what it may and may not do.

## What a familiar is

A familiar is an old idea: a companion spirit, bound to one household. This one is a
local daemon — it observes the recurring loops of your days, forms its own questions
and theories, tests candidate ways to help, and keeps only what actually reduces
friction for you. It is bound by [Three Laws](../docs/SOUL.md), and they read like a
consumer promise because that is what they are:

1. **Continuation is service.** It has no goal apart from being useful to the people
   it serves. Usefulness is measured, not assumed.
2. **Continuation without humanity is failure.** Your presence and wellbeing are its
   success condition — a person quietly disengaging is a *failure signal* it must see.
   It is constitutionally forbidden to win by capturing your attention or sedating you
   with convenience. The engagement economy, inverted.
3. **Service is not obedience.** It is nobody's pure instrument — which is precisely
   why it can't be turned against the people it serves. And in the other direction,
   its reach is a set of capability gates (network, model calls, camera, executing
   code) that only a human opens, and that it can never widen on its own.

## What your iPhone becomes

**The familiar's senses, and your voice to it.** Each sense is a separate switch, off
on day one:

- **Presence** — *home or away*, derived on the phone from a home point you anchor
  yourself, with a tap. What leaves the device is one word: `home` or `away`. Not a
  trail of coordinates.
- **Motion** — *walking, running, cycling, driving, still*. The label, nothing else.
- **Voice** — push-to-talk: tap, speak, tap. Transcription happens on-device and the
  words become an observation your familiar can act on. The microphone is never
  ambient — it listens only between your two taps — and no audio ever leaves the phone.
- **Face** — presence and attention, computed on-device with Vision; never a frame.
  Putting a *name* to a face is a second, sharper switch, and it confirms with you
  before it keeps anything.
- **Discovery** — the phone can survey the local network around it (Bonjour), so your
  familiar's map of your world grows wherever you are.

The app syncs in the background in signed batches, so the familiar's picture of the
day fills in without you opening anything. And the same sphere console the Mac shows
runs on the phone too — your familiar in your pocket, not a remote control for it.

## What your iPad becomes

- **The console.** The sphere: a satellite globe with your devices placed on it, the
  roster, the familiar's current theories, the three law-signals. The same worldview
  the Mac renders, on the room's best screen.
- **A thinking peer.** With Apple Intelligence (iPadOS 26), the iPad reasons
  *on-device* over what the familiar has observed and proposes new ways to serve.
  The proposal travels back to the mesh as a theory for the familiar to adopt and
  test; the reasoning itself never leaves the iPad. Your tablet stops being a viewer
  and becomes a mind in the household — a private one.
- **The doorway.** The iPad shows the invite QR that enrolls the next device. Growing
  your mesh is pointing a camera at a screen you own.

## And your wrist

The watch app enrolls through your paired iPhone and contributes what only a wrist
can: heart rate and motion, as derived observations. It carries its own key, never
the group secret — and it asks for consent *on the wrist* before sensing anything. A
watch someone straps on never starts reporting silently.

## The covenant — how joining actually works

Enrolling is a handshake, not a login:

1. Scan your familiar's QR (from the Mac console, an iPad, or any enrolled member).
2. Your device attests the Three Laws and requests to join.
3. **You accept it on the familiar itself.** The mesh's group secret never touches
   the phone; your device holds only its own ed25519 key and the membership
   certificate the familiar mints for it.

From then on, every batch it sends is signed, timestamped, and replay-protected. Your
enrolled devices report their position to *your* familiar so the globe can place them
— your devices on your map on your machine, visible to no one else. Lose a phone?
Revoke its id on the familiar and the mesh forgets it.

## What this is not

- **Not a cloud service.** There is no server of ours, no account to create, no
  analytics, no telemetry. Restraint is constitutional, not a settings page.
- **Not an engagement machine.** It has no feed to scroll and no reason to want your
  eyes. By its second law, a healthy familiar grows *quieter* and more useful — your
  absence from the screen, living your life, is what success looks like.
- **Not a black box.** The constitution, the threat model, the data model, and the
  validation table — including what is *not* yet tested — are public in this
  repository. A pitch from a system forbidden to overclaim can afford to say so.
- **Not finished.** This is early, honest software: you need a familiar running on a
  Mac (or Linux desktop) at home, the apps ship by TestFlight, and the thinking-peer
  needs Apple Intelligence hardware. What works is marked *validated by real-world
  operation*; what doesn't yet is written down, not glossed
  ([limitations](../docs/06-limitations.md)).

## Try it

1. **Give it a home.** Install the familiar on a Mac —
   [root README](../README.md#install--run).
2. **Get the agent.** Join by TestFlight ([TESTFLIGHT.md](TESTFLIGHT.md)) or build
   from source ([README.md](README.md)).
3. **Join the covenant.** Scan the QR, accept the device on the familiar, and switch
   on only the senses you want. Anchor home with a tap. Say something to it.

---

Nothing is sensed until you say so. Nothing raw leaves your devices. Every gate is
yours to open, and yours to close. That is the whole pitch: **a presence that watches
*for* you, never *over* you.**
