# Review — membership, establishment, device and human mapping

**Asked by Ian, 2026-08-16:** *"Really give the membership, mesh establishment, and device and
user registration process and mapping is all correct and makes sense the way we've implemented
— the organic growth of this slowly may have masked a better architectural decision path."*

**Verdict: the architecture is right and unfinished.** ADR-0039 (2026-08-14) diagnosed this
exact problem and specified the fix. Roughly half of it was built. Every symptom Ian hit today
comes from the unbuilt half — not from a wrong decision, and not from something that needs
redesigning.

## The evidence

This Mac's own membership record, read live:

```
3d68a068   established='MacOnStick'   admitted=yes    ← a MACHINE name
b604bbd6   established='ian'          admitted=yes
10ba2c17   established='betty'        admitted=yes
d5c31472   established='ian'          admitted=yes
```

`Establishment.handle` is filter 2 — *who this device serves*. On the phones it holds a human
(`ian`, `betty`). On the Mac it holds a computer (`MacOnStick`). One field, two meanings,
still, today.

ADR-0039 wrote this down verbatim as the thing it was ending:

> One field has been doing two jobs. The membership record's establishment handle is the
> system's only name slot, so the fleet used it both ways at once: the phones establish as
> their *human* ("ian"), while the Macs were named as *machines* ("MacOnStick", "wildhorse").

So Ian's Mac reads `Mine — Ian's` and simultaneously *unnamed*: the local UI knows an owner,
the mesh record names a machine, and no human-facing surface can find a person.

## What was designed vs what exists

| ADR-0039 called for | State |
|---|---|
| `DeviceRecord` — device facts, its own record | **built** (`crates/mesh/src/device.rs`) |
| `HumanRecord` — one per human: handle, devices[], relationships, preferences, habits, routines | **never built.** Its only trace in the codebase is a comment in `reaction_rule.rs`: *"until that record exists they live here"* |
| `DeviceRecord.humans[]` — the association edge, current and past | **structure exists, has no writer.** `merge()` reconciles it and a test asserts on it; nothing anywhere ever creates one |
| The roster as a *view* over the two records | partial — it reads devices, but there is no human record to read |
| Migration of machine-named establishments | **not done** — hence `MacOnStick` above |

## The consequence: seven notions of "whose device is this"

Counted across the tree, with nothing reconciling them:

| Notion | Where it lives | Authority |
|---|---|---|
| `Establishment.handle` | membership record | the mesh's filter-2 fact — **the only durable one** |
| `servedHuman` | `AppModel`, local | what this app attributes reports to |
| `deviceOwner` | `AppModel`, local | who the human said owns it |
| `deviceRole` (personal/shared) | `AppModel`, local | how it is used |
| `DeviceRecord.humans[]` | device record | designed for exactly this; unwritten |
| `brief.human` | the mesh brief | first identity opted into the group — a *third* rule |
| `present_human` | derived | who is here **now** — correctly separate, not part of the problem |

The app never reads the mesh's establishment at all. `AppModel` has no reference to it. So the
console's `Mine — Ian's` and the record's `MacOnStick` cannot disagree in any way the system is
able to notice — there is no comparison to fail.

## Why this is organic growth, precisely

Each of these was individually reasonable when added. `servedHuman` came from ADR-0016
(per-human attribution on a shared device). `deviceOwner`/`deviceRole` came from the shared-iPad
case. `brief.human` came from the consent gate on identity sharing. `Establishment` came from
two-filter admission. None was wrong; nobody ever went back and said *which one is the truth,
and what are the others?* ADR-0039 answered that — and the answer was never finished, so the
question is still open in the code.

## Recommendation — finish ADR-0039, do not redesign

1. **Build `HumanRecord`**, as specified. It is the missing half, and the thing that makes
   "who is this person, and which devices are theirs" answerable at all.
2. **Write the `humans[]` edge.** The moment a device establishes, an association exists; the
   structure and its merge already work. This alone gives the plural, time-bounded relation the
   station model (ADR-0042) already leans on.
3. **Migrate machine-named establishments.** `MacOnStick` and `wildhorse` are device names
   living in the human slot. The device record now has a proper home for them
   (`DeviceRecord.name`), which it did not when they were written. This is a data migration,
   not a schema change — and it is what makes Ian's Mac stop reading as unnamed.
4. **Make the app read the record, not shadow it.** `servedHuman` and `deviceOwner` should be a
   *view* over the establishment + association, with local state only as a pre-confirmation
   staging area (which is what T-198's `namePending` already is, in miniature).
5. **Make a contradiction visible.** Two sources that cannot disagree in public will disagree in
   private forever. If local belief and the mesh record differ, the Device screen should say so
   — the same discipline as every other honest-failure fix this week.

Nothing here needs a new ADR. It needs ADR-0039 finished, in the order above; steps 3 and 5 are
small and fix the reported symptom, while 1 and 2 are the structural work.

## What this does not change

`present_human` (who is here now, ADR-0019) is a genuinely different question from who a device
serves, and its separation is correct. The two-filter admission itself (ADR-0026) is sound —
today's registration failures were UI silence (T-185, T-198), not a flaw in the filters.
