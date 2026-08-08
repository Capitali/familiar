# ADR-0028 — The mesh games, and the law of the fire

- Status: **accepted** (Ian, 2026-08-04; human-turn law 2026-08-06). Shipped: Riddle of the
  Mesh and The Campfire, both live across the household.

## Why games

A game is not a toy bolted onto the mesh; it is the mesh, exercised on purpose. Every act is a
signed member write; the turn travels between devices like the worldview does; the state
replicates door-to-door on the record-sync channel; and the judge is the familiar itself,
running deterministic rules a human can read. The game screens say — prominently — which seams
each game tests, because that is the point: **testers playing IS the test.** (The first two
days of play found eleven real defects, several of them deep replication semantics no unit
test had reached.)

## The law of the fire

> **Games are played between humans.** A seat is an *established handle*. Every present device
> of that human shows the ember; any one of them may answer. Devices serving nobody, daemons,
> and watches hold no seat — doors referee, sensors sense, neither takes a turn.

The door resolves the acting device to its established handle before any move is judged; an
unestablished device is refused with the reason spelled out ("the fire knows humans —
establish who you are before playing").

## The two games

- **Riddle of the Mesh** — the familiar draws from its own deterministic bank (the cards teach
  the system: keys, signatures, record-sync, guests). The door judges guesses with articles,
  case and punctuation forgiven; **the answer never travels to any console**. First correct
  guess wins. One guess or a pass per turn.
- **The Campfire** — the familiar speaks the opening line; each holder adds ONE line (240
  chars) and passes the ember to a chosen human or lets the circle turn. Closes at 24 lines or
  by its lighter's hand. The familiar then names the **line of the story**: the line with the
  most words no other line used — a novelty prize whose rule is one sentence long, and the
  familiar's own opening can never take its own prize. (An LLM judge may someday have
  opinions; the deterministic rule is the floor, not the ceiling.)

## Shared rules

- One game at a time — the household's hearth has one fire.
- Turn clocks are enforced **lazily** by whichever door reads next (the mesh has no game
  loop, only readers and actors). Expiry = a strike and the ember moves on; two strikes =
  eliminated to spectator; a collapsed field goes to the last human standing.
- State replicates by last-writer-wins *within* a game id; a different id is a different
  **generation** and the later-lit fire wins outright — a finished game's tombstone can never
  smother a fresh one (ADR-0027's merge lesson, learned here first).
- The ember is visible everywhere it should be: a pulsing roster badge with a live countdown
  on every device of the holder (and the machines those devices nest under), a warm games
  glyph, and a chime on arrival — the badge is a door; tapping it lands in the game.
