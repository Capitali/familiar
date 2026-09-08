# Draft ask to Jeff — a ship's computer name, as a child of the captain's record

Status: DRAFT for Ian's word (2026-09-07). Files in Ian's name on united-cat-foods-metal, or
folds into the captain-record / fleets proposal the wildhorse lane is drafting (same record).
Codex-review before filing per the house method, when its window reopens.

Ian's direction, verbatim: *"ask Jeff for a 'ships computer name' namespace that is a child
of the captains data set and follows the captain from ship, to station, to raceway, to every
aspect of the universe, under the name given to it by the captain who initiated it."*

---

## The ask

Give every captain's record a child namespace, **`computer`**, holding the name of the
captain's ship's computer — set once by the captain who initiated it, carried by the captain
to every hull, station, raceway and any later surface of the universe.

Today the exchange already carries the captain as `traderName` on every actor row (it spans
hulls — Kibble Klipper and Kibble Klipper II both report "Luke SkyWhisker"), but nothing hangs
off that captain. The familiar keeps the computer's name (Felix) in a captain store on its own
host; a client that talks straight to the exchange — UCF Familiar in direct mode, on an iPad
with no host — has no way to learn it, and so introduces itself by the default. A name that
lives on the world is known to every client on every device, and to the raceway when it exists.

## The shape we would adapt to

```
captain (keyed by whatever the engine uses for the captain; traderName is the display name)
└── computer
    ├── name        — the captain's gift; no generated names; the engine's charset/length rule
    ├── namedAtTick — when it was first given
    └── namedBy     — the key that gave it (the captain who initiated it)
```

- **Read:** beside `traderName` on `/v1/me`, `/v1/actors` rows and `/v1/careers` — anywhere
  the captain is named, the computer is named with him.
- **Write:** once, by the captain's own key; a later change is the captain's act and keeps a
  trail (rename is allowed; the first name is not silently replaced by a second pairing).
- **Scope:** one per captain, not per hull. A hull a captain pairs answers as his computer;
  a hull he lends to another captain's fleet still speaks with its owner's computer (the
  familiar's invariant I4 in the captains/fleets proposal).
- **No effect on the fold:** a name; no stats, no standing, no capability. If the engine
  would rather not hash it, it can live beside the actor rather than inside sealed state.

## What we do on our side

- The familiar's captain record (in flight, T-236) mirrors the engine's captain and reads the
  computer's name from the wire when served; the host store becomes a cache, not the source.
- Every client — host bridge, direct mode, racing — shows the name from the wire.
- Until the field exists: the device keeps a per-key name the captain types (today's behaviour).

## Why it belongs with the fleets proposal

Both are children of the same captain record: `computer` (this ask) and `fleet` (the fleets
proposal, Ian's ruling 2026-09-07 that fleets become engine state). One migration of the
captain record on the exchange is better than two.
