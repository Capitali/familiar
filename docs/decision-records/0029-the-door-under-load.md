# ADR-0029 — The door under load

- Status: **accepted** (Ian + the familiar, 2026-08-07). Every finding below was caught live
  during the first true two-human riddle session (ian × 3 devices vs betty × 1, both doors
  active) and fixed the same afternoon. ADR-0028 said "testers playing IS the test"; this
  record is that sentence paying out.

## What the game found

Six defects, one afternoon, all invisible to unit tests because each one needed real load,
real humans, or real weeks of accumulated history:

### 1. The saturated door (windowed reads)

Every worldview read loaded and JSON-parsed the **entire** observation log — 21,724 rows,
two-three times per request — and six polling clients meant ~4 requests a second. The door's
two runtime workers drowned: 2–5s per read, 28-second spikes, acts queued behind reads for
minutes. Every symptom of the hour traced back here: BEGIN buttons that "did nothing",
guesses that landed three minutes late, consoles failing over between doors and flapping
between their truths.

**Fix:** `store::load_last(dir, file, limit)` — the hot path takes only the newest N rows
(4000). Every per-request consumer (presence, device reports, worldview signals) works over a
freshness window measured in minutes anyway; the one "since when" question that needs the
oldest row gets `observation::first_ts()`. Reads went from 2–5s to ~0.5s.

**Law:** any per-request read of an append-only log must be windowed. A log that only grows
is a fuse that only shortens.

### 2. The roster that wiped itself (atomic writes)

`peers.json` was written with plain truncate-and-write at seven call sites. A reader racing a
writer parsed half a file as an **empty roster** — and its own next upsert saved that
emptiness back, permanently deleting every learned peer and their sticky actors. Watched
live: six lighthouse rows vanished in one poll, then straggled back one dial at a time.

**Fix:** all writes go through `save_peers()` — temp file + rename. A racing reader sees the
old roster or the new one, never a torn one.

**Law:** any file that is read-modify-written by concurrent parties gets atomic replacement,
and a parse failure must never be treated as an empty store when emptiness can be saved back.

### 3. The stale report that classified a door as a watch

`device_reports` took the latest device-actor observation per node over the **whole** log,
no freshness window. A weeks-old row from a since-retired relay path tagged the wildhorse
door as `watch:ian` at the lighthouse — forever. Door and watch merged into one dedup
lineage, and the roster row flipped between "Wildhorse" and "Apple Watch" every time their
`last_seen`s leapfrogged.

**Fix:** classification requires evidence fresher than `AGENT_FRESH_SECS`. The sticky peer
actor — learned at ingest, persisted on the peer record — is what carries identity across
quiet spells; the report map only ever says what a node is doing *now*.

**Law:** what a node IS deserves fresh evidence. History informs identity through the record,
never through an unwindowed scan.

### 4. The ember that waited for the gossip round

Two humans acting through two different doors (ian's iPad through the lighthouse, betty's
phone through wildhorse) watched the holder seesaw for ~30s after every act, as each door's
periodic sync swung the other's view.

**Fix:** a game act pushes record-sync to the sibling doors immediately (best-effort, spawned
off the reply path). The ember travels at act speed; the periodic round remains the safety
net.

**Law:** anything a human just did deserves eager replication; the gossip cadence is for
what nobody is waiting on.

### 5. The invisible refusal

A door's reply to a game act — including refusals like "not your turn" and "a game is already
burning" — landed in an app log no screen ever read. A refused or queued BEGIN looked exactly
like a dead button.

**Fix:** the device state carries its recent notes; the games screen prints the door's own
words, door named ("door 127.0.0.1 · not your turn — the ember is with betty"). The silent
no-host bail notes itself. A judged move that fails to save returns 500 instead of lying OK.

**Law:** every act gets a visible answer. Silence is indistinguishable from breakage, so
silence is breakage.

### 6. The question that begged twice

After submitting an answer, the question stayed live while the door judged. Under load that
gap stretched to minutes — betty answered the same riddle three times and each duplicate
became a recorded guess.

**Fix:** submit renders **THE DOOR IS JUDGING — your answer is in** until the game state
moves (or 25s passes), the guess input is typing-guarded like every other input (a poll
mid-thought must not wipe the player's typing), and countdowns render their value at build
time instead of flashing `--:--` until the next ticker second.

**Law:** the UI must distinguish "waiting on the mesh" from "waiting on you." Only one of
them should ever show an input.

## Also in this pass

- The lighthouse keeps its globe dot and node dive but no longer takes a roster card — it is
  infrastructure, not a housemate (Ian's call).
- Launch opens onto the bare globe centered over home, menus tucked away — one tap on the
  hide glyph brings the screens back.
- Client build 68 carries all of the above; both doors run the same daemon commit.

## Still open (named here so they don't get lost)

- **Cold-start candidate chain:** a device's first read walks stale host candidates (old
  public IPs from previous parks, other devices' LAN leases) sequentially, each eating a
  multi-second timeout — the long RED-! at launch. Fix: race candidates in parallel, prune
  non-doors.
- **Residual ~0.5s read cost:** records and standing still load per request; a second
  windowing/caching pass is available if the household grows.
- **Cross-door liveness pulse (ADR-0027 follow-through):** the lighthouse still only lists
  devices that have dialed it; deriving roster rows from synced record `last_seen` would
  show the household from either door.
