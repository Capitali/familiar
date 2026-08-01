# App Review — reply, review notes, and TestFlight copy

Working copy of what we tell Apple and testers. Keep it here so the wording is versioned
alongside the build it describes.

---

## 1. Resolution Center reply — macOS build 41

**Status:** ready to send. **You must paste this yourself** — App Store Connect exposes no API
for Resolution Center; the thread lives only in the web UI and email.

Apple's question was *"Can you provide instructions on how to verify application functionality?"*
That is an information request, so it is answered in Resolution Center rather than by uploading
a new binary blindly.

**The honest position:** macOS build 41 cannot be verified by anyone, including us. The Mac app
renders its console unconditionally and never starts the enrolment it needs to join a mesh, so a
fresh install shows an empty window with no way forward. Sending "verification steps" for build 41
would be sending steps that do not work. The reply below says so plainly and points at the fix.

> Thank you for the note, and apologies for the unclear build.
>
> **What the app is.** Familiar is a client for a private, self-hosted mesh. A person runs the
> familiar service on their own hardware; this app joins that mesh as a peer and displays what it
> knows — a roster of the devices and people in the mesh, their positions on a globe, and a
> console for answering the questions the familiar asks. There is no account and no server of
> ours: the app talks only to the mesh the user owns.
>
> **Why build 41 could not be verified.** Investigating your question, we found a defect: the
> macOS build never initiates the enrolment step that joins a mesh, so a fresh install displays an
> empty console with no way to proceed. That is what you would have seen, and no instructions
> would have gotten past it. The iOS build of the same version does not have this defect.
>
> **What we have changed.** The macOS app now presents a join screen on first launch, discovers a
> reachable mesh automatically, and displays a confirmation code while it requests membership. We
> have also removed a network-policy error that silently blocked the app from reaching any mesh,
> and we now bundle all web assets in the app rather than loading them at runtime, so the console
> renders without a network connection.
>
> **Verification for the next build.** The next macOS build includes a demonstration mesh that the
> app connects to automatically on first launch, with no account, credentials, or hardware
> required. Launch the app and the join screen finds it, joins it, and opens the console with a
> populated roster and globe. Full steps are in the App Review notes attached to that build.
>
> We would rather submit a build you can actually exercise than ask you to work around a defect we
> have already found. The new build will follow shortly. Thank you for your patience.

**Do not** promise the demo mesh here unless the anonymising projection (task #5) ships with the
build. If that slips, cut the "Verification for the next build" paragraph and say the new build
follows with instructions.

---

## 2. App Review notes — to attach to the next macOS build

Keep these short and literal; reviewers follow them exactly.

```
Familiar is a client for a private, self-hosted mesh. There is no account system and no
service of ours to sign in to — the app joins a mesh the user runs on their own hardware.

To verify functionality, no credentials or hardware are needed:

1. Launch the app. The join screen appears and searches for a reachable mesh.
2. It finds the demonstration mesh automatically and displays a six-character
   confirmation code while it requests membership. Admission is automatic; this
   takes a few seconds.
3. The console opens. You should see:
   - a rotating globe with the mesh's nodes marked on it
   - a "Roster" panel listing the members of the mesh, each with its platform,
     version, and when it was last seen
   - the left and right glyph controls, which switch between roster, theories,
     activity, and device screens
4. Click any node's location marker to descend from orbit to street level. The
   globe rotates the node under the camera and hands off to Apple Maps.
5. The Familiar menu → "Push to Talk" is a microphone feature and will request
   microphone permission. It is optional; the app is fully functional without it.

The demonstration mesh contains synthetic data only. No real personal information
is shown to anyone who has not been explicitly invited to a private mesh.

Network: the app connects to mesh peers over TLS with certificate pinning. Peers are
self-signed and identified by public-key pin rather than by a certificate authority,
which is why the app declares NSAllowsArbitraryLoads — peer addresses cannot be
enumerated in advance as a domain exception list.
```

---

## 3. TestFlight "What to Test"

Currently **null** on iOS build 41 — testers see no guidance at all. Set via the App Store
Connect API (`PATCH /v1/betaBuildLocalizations/<id>`) or in the web UI.

### iOS build 41

```
Familiar joins a private mesh you run yourself and shows you what it knows.

On first launch it finds the mesh and asks to join — you'll see a short
confirmation code while it does. After that the console opens on a globe with
the mesh's devices marked on it.

Worth trying:
· The Roster — who and what is on the mesh, and when each was last seen.
· Tap a node's marker to dive from orbit down to street level.
· The Device screen — every sensor is off until you turn it on. Nothing is
  collected before you do.
· Answer a question the familiar asks you in the console.

Known in this build:
· "Present: unknown" against every member — the app does not yet identify who
  is using a device. That is the next piece of work.
· The globe can take a few seconds to appear on first launch while it loads.

Please report: anything that hangs, anything that says it sent data you did not
turn on, and anywhere the wording confuses more than it explains.
```

### macOS — for the next build

```
The Mac now joins the mesh as a peer in its own right, rather than needing a
familiar running on the same machine.

On first launch you'll get a join screen: it finds the mesh, shows a confirmation
code, and opens the console once it's admitted. If it can't find one, you can
paste an invite from another device — an invite carries an address only, never a
secret.

Fixed since the last build:
· The Mac could not join a mesh at all — it opened straight into an empty console.
· A network policy left over from an earlier design silently blocked every
  connection to the mesh.
· The globe swung to an arbitrary point before diving to street level, and the
  satellite view sat rotated against the map through the transition.
· The console needed an internet connection to draw itself. It no longer does.
```

---

## 4. The review story under ADR-0026 — the reviewer is a guest by construction

For builds that ship the two-filter admission
([ADR-0026](../docs/decision-records/0026-two-filter-admission.md)), the verification story
gets simpler and *more* honest, and the notes above should be rewritten around it when that
build is submitted:

- A reviewer who launches with nothing nearby **founds their own one-node mesh** and sees their
  own device's real local activity immediately — nothing synthetic, nothing of ours.
- A reviewer who visits the demonstration mesh reads its **guest projection**: the live system
  with the people taken out. They are a guest *by construction* — identity is established by
  evidence (a handoff, an invite, an introduction in the mesh's own space), none of which a
  remote reviewer can or should produce — so no one has to remember to keep them anonymous,
  and no one could forget to.
- The console tells them so in plain words: the covenant is accepted, identity is not
  established, and what each would take. "Admission pending" is a true statement about their
  state, not an apology for a broken screen.

The standing promise in §2 — *"No real personal information is shown to anyone who has not been
explicitly invited to a private mesh"* — stops depending on a hand-maintained roll and becomes
a property of the rules themselves. Keep that sentence; under ADR-0026 it is load-bearing and
true by construction.
