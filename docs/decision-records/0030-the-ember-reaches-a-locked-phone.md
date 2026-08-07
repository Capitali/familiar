# ADR-0030 — The ember reaches a locked phone

- Status: **accepted** (Ian, 2026-08-07 — "push build"). Server + client shipped the same
  day (daemon commit `3b90278`, client build 69).

## Problem

iOS suspends the console within seconds of the screen locking. ADR-0028's law — every
present device of the holder shows the ember — held only while apps were foregrounded; a
turn passed at the fire waited until the holder happened to look at their phone. The
watch chime (WCSession) covered a wrist with the phone app alive, and BGAppRefresh wakes
are minutes-coarse at the OS's whim. The missing piece is the platform's own channel:
APNs.

## Design

Three pieces, every one **best-effort by construction** — no config, no token, or no
reachable APNs mean no push, and the game itself never depends on it:

1. **Registration** — `POST /mesh/push-token`: a member device hands its door the APNs
   device token, signed like every member write (body signature, key-fingerprint identity,
   full standing). The door keeps one row per node in `mesh/push_tokens.json` (atomic
   replace, ADR-0029 §2's law). The device detects its own APNs environment from the
   embedded provisioning profile: development profile → `sandbox`; TestFlight/App Store
   (no embedded profile) → `production`.
2. **The sender** — a door carrying `mesh/apns.json` (`key_path` to Apple's `.p8`,
   `key_id`, `team_id`, `topic`) can push. The provider JWT is ES256 signed with `ring`
   (already in the tree under rustls); the HTTP/2 POST shells to `curl --http2`, which
   both doors ship, so the mesh's own hyper stays http1-only. Doors without the file
   simply never push.
3. **The trigger** — holder change, watched at all three game mutation sites: a local act,
   a synced game absorbed at the door, and the dial-out absorb. The cross-door arrival is
   exactly the moment the holder's phone is most likely locked. The push is a visible,
   time-sensitive alert — "🔥 the ember is yours" — to every registered device of the
   holder's *handle* (their devices resolved from membership records, ADR-0028's
   human-seat law extended to the pocket).

## Keys and environments

The Apple auth key lives outside the repo (`~/.appstoreconnect/private_keys/` on
wildhorse, `familiar_data/apns/` mode 600 on the lighthouse). One live lesson is recorded
here so it isn't relearned: an APNs key can be created **restricted to the development
environment**, and production sends then fail with `BadEnvironmentKeyInToken` — the
sandbox gateway accepting the key proves nothing about production. TestFlight pushes need
a key issued for both environments.

## What this deliberately is not

- Not a delivery guarantee: APNs may coalesce or drop; the lazy turn clock (ADR-0028)
  remains the referee.
- Not a data channel: the payload names the game kind, nothing else — worldview truth
  still travels the mesh, never Apple's pipe.
- Not the watch path: the watch chime stays on WCSession via the paired phone; a future
  standalone-watch push would be its own decision.
- Not silent-wake background sync (BGAppRefresh remains separate); a `content-available`
  wake and Live Activities are the named next steps if the alert path proves its worth.
