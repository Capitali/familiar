# ADR-0039 — Humans and devices are separate records; the roster is a view

- **Status:** **accepted** (Ian, 2026-08-14 — direction given in prose and binding here;
  implementation phased below)
- **Date:** 2026-08-14
- **Relates to:** [ADR-0022](0022-the-human-dossier.md) (the dossier — the HumanRecord
  grows from it and inherits every constraint), [ADR-0025](0025-device-identity-is-not-key-identity.md)
  (key ≠ device ≠ person — this record finishes what that one started),
  [ADR-0026](0026-two-filter-admission.md) / [ADR-0027](0027-records-travel-lighthouse-law.md)
  (membership records — untouched in scope, narrowed in meaning),
  [ADR-0032](0032-declared-actuators-and-the-reaction-loop.md) (whose deferred follow-on —
  habit-driven initiation — is scheduled here, alongside standing reaction rules),
  [ADR-0016](0016-multi-human-served-identity.md) (per-human attribution),
  [ADR-0019](0019-friendly-identification.md) (identification addresses)

## Context

One field has been doing two jobs. The membership record's establishment handle is the
system's only name slot, so the fleet used it both ways at once: the phones establish as
their *human* ("ian"), while the Macs were named as *machines* ("MacOnStick",
"wildhorse"). 2026-08-13 spent a whole night on the consequences — a roster that read as
a wall of one name, a rename dance that could mint ghosts, and a display format
("SystemName : SystemType : ServedUser") that the data model could not actually feed,
because *device name* and *served human* are two facts and the model stores one.

Meanwhile ADR-0022 built the human's patterns (presence, habits, needs) as a dossier —
but the human still has no first-class record of their own; identity, dossier kinds, and
establishment references are scattered. And ADR-0032 built the act loop but deliberately
deferred the object a *confirmed* automation would live in: the familiar can act once
from a pursued thread, and can only re-ask when the human's intent is standing
("dim while I'm away" — the lighting theory that kept asking, 2026-08-14).

Ian's direction (2026-08-14), which is the decision:

> ADR-0032 needs to build the persistent reaction rule, and habits. The data model needs
> to be capable of a full set of data for humans and for devices; devices and humans are
> separate objects that can be related to each other. So a human needs its own record:
> name, devices associated with, relationships and lineage, preferences and habits,
> routines and workflows. A device needs its own record: the device name and type,
> capabilities, observation interfaces, networks, humans that have associated to the
> device current and past. A human needs a rich data record, and a device needs a rich
> data record. The roster is simply views into those data sets.

## Decision

### 1. Two records, related, never conflated

**HumanRecord** — one per human the mesh serves. Grows from the identity registry and
the ADR-0022 dossier rather than beside them:

```
HumanRecord {
  handle                    // the identity slug ("ian") — the establishment reference
  name                      // how they are addressed ("Ian")
  devices[]                 // associations: { device_id, since, until? } — current and past
  relationships[]           // to other humans: kin, household, lineage — declared, not inferred
  preferences               // stated wants, with provenance (said, confirmed, inferred-and-affirmed)
  habits                    // the ADR-0032 dossier patterns (ctb|handle|habit|…) — same store, same bounds
  routines[]                // standing reaction rules this human confirmed (§3)
  workflows[]               // multi-step routines, same consent shape, executed stepwise
  first_seen, interactions  // carried over from the identity registry
}
```

**DeviceRecord** — one per device, machine-facts only:

```
DeviceRecord {
  device_id                 // ADR-0025's durable identity (keys live on the membership record)
  name                      // the SystemName ("MacOnStick", "Codex") — deliberate, human-given
  kind                      // phone | tablet | watch | mac | linux | vps | hub …
  os, os_version, arch      // the SystemType facts ("macOS", "x86_64" → MacIntel)
  capabilities[]            // what it can do (brief capability + declared actuator surfaces)
  observation_interfaces[]  // what it can sense: camera, mic, GPS, BLE, network survey …
  networks[]                // where it lives: tailnet, LANs seen, addresses — with last_seen
  humans[]                  // associations: { handle, since, until? } — current and past
}
```

The two relate **only** through the association edges, each time-bounded — "current"
is an open `until`, history is closed ones. Nothing else may hold a human fact on a
device object or a device fact on a human object.

### 2. The roster is a view

Every console surface — roster cards, callouts, arrivals, network rows, the map — is a
**projection of these records**, never a store. The roster sentence is the canonical
projection: `DeviceRecord.name : DeviceRecord type-word : HumanRecord.name` of the
current association ("Codex : iPad : Ian", "Wildhorse : MacIntel : Ian"). A device with
no current association shows its machine facts and no human; a device with no name yet
wears the honest mask (never the hex). The membership record keeps answering the one
question it was built for — *may this thing be here, and what may it see* (ADR-0026) —
and stops moonlighting as a name registry: establishment binds a device to a
**HumanRecord by handle**, and the machine-name squat on establishment handles ends by
migration (§4).

### 3. Reaction rules and habits (ADR-0032, completed)

A confirmed automation becomes an object:

```
ReactionRule {
  id, subject               // whose rule (HumanRecord.routines carries it)
  trigger                   // presence transition, schedule window, habit threshold
  surface, act              // a DECLARED actuator surface and one of its acts (ADR-0032)
  minted_from               // the theory/thread + the affirming answer — provenance is consent
  enabled, last_fired, outcomes[]
}
```

- **Minting**: an affirmed theory (or an explicit human instruction) that names a
  declared surface mints a rule — the SeekConsent moment is the mint, once, not per act.
- **Execution**: tend (cycle 8·3) evaluates rules on their triggers; every firing is
  still guard-weighed, gated by `allow_actuate`, one act per surface per window,
  reaction-read exactly as ADR-0032 built — a reverted act *disables the rule* and says
  so, it does not merely rest.
- **Habit-driven initiation** (0032's deferred follow-on): a habit slot crossing the
  strength threshold may *propose* a rule — never silently become one. The proposal is a
  question; the answer mints or declines. The asking loop ends because a standing intent
  finally has a place to live.
- **Visibility**: rules list on the console beside consents — named in plain words
  ("away → lights 25%, back → 50%"), one tap to disable, disable travels like a
  correction.

### 4. Privacy, federation, migration

- The HumanRecord inherits ADR-0022 **wholesale**: derived not tape, bounded,
  node-local, subject-readable, withdrawable, projected away from guests; relationships
  and lineage are *declared* facts, never inferred ones. It does not federate beyond
  what the dossier already allows (nothing).
- The DeviceRecord's machine facts are mesh facts and replicate like records do
  (ADR-0027 sync); its association edges name handles only.
- **Migration** (one, per ADR-0026's lesson): existing establishments whose handle names
  a machine ("MacOnStick") move that name to `DeviceRecord.name` and re-point the
  establishment at the owning human; establishments already naming humans stand. The
  device-name field the consoles grew (Device screen) writes `DeviceRecord.name` from
  day one.

## Consequences

**Good.** The one-field-two-jobs era ends: renames stop being identity surgery; the
roster sentence is computable for every device; a confirmed intent becomes a standing,
visible, revocable automation instead of a recurring question; the human's scattered
facts get one owned home.

**Bad, and accepted.**
- Two more record kinds to sync, migrate, and keep honest — paid once, against the
  nightly cost of the conflation (2026-08-13 was that cost, itemized).
- Association history ("humans past") is genuinely sensitive; it lives under dossier
  rules and the guest projection, and pruning it is the subject's right.
- Reaction rules are real-world power on a timer; every mitigation ADR-0032 built
  (declared surfaces, reversibility, one-per-window, rest, disable-on-revert) applies
  unchanged, and rules add named provenance on top.

## Build order

1. **DeviceRecord + the name field + views** — the roster sentence completes for the
   phones (device-name entry on the Device screen; tailnet hostnames as suggestions).
2. **ReactionRule store + tend integration + console list** — the lighting loop closes.
3. **HumanRecord** folding identity registry + dossier + associations (read paths first,
   then writes).
4. **The migration** — machine-name establishments → DeviceRecord.name, one pass,
   doctor-checked, with ADR-0026's dual-write discipline.
5. Habit-threshold proposals (after rules have field time).
