# ADR-0025 — A key is not a device, and a device is not a person

- **Status:** accepted (design) — not yet implemented. Named by Ian as the priority, 2026-07-31.
- **Date:** 2026-07-31
- **Relates to:** [ADR-0016](0016-multi-human-served-identity.md) (per-human attribution — this
  gives it something durable to attribute *to*), [ADR-0019](0019-friendly-identification.md) (the
  identification ladder produces the device→human link this formalises),
  [ADR-0020](0020-standing-and-the-guest-projection.md) (standing is currently keyed on the wrong
  thing), [ADR-0022](0022-the-human-dossier.md) (a dossier keyed on a churning identifier is a
  dossier about nobody), [ADR-0012](0012-lighthouse-rendezvous.md) / [ADR-0015](0015-automated-covenant-admission.md)
  (enrolment mints the key this conflates), `crates/mesh/src/node.rs`, `crates/kernel/src/identity.rs`

## Context

`node_id` is the fingerprint of a public key. That is exactly what it should be, and it is
forgery-proof, which is why enrolment rests on it. The mistake is everything built on top: the mesh
has been treating a **keypair** as if it were a **device**, and welding the **human** into the same
string (`phone:ian`).

This stopped being theoretical on 2026-07-31. In one afternoon of reinstalling builds onto two
devices:

- **Reinstalling minted new keys**, so the mesh saw brand-new devices. The iPhone briefly appeared
  as `147cfa12…` alongside its real `d5c31472…`.
- **Every throwaway identity became permanent.** Member adoption (`apply_status_freshness`) promotes
  an unknown node to a peer record on a *single* heartbeat, so each transient key became a member
  that never leaves. The iPad's welcome list showed **three iPhones**.
- **Standing was attached to the key**, so a device that reinstalls loses recognition and returns as
  a stranger — even though it is the same physical iPad, in the same house, used by the same person.
- **The Watch cascades.** It mints its own key and is handed the phone's human, so one person
  becomes several actors, and nothing records that the watch and the phone are the same wrist.
- Two consoles showed **different lists**, because each serving node holds its own roster and roll.

Every one of those is the same defect wearing a different hat.

## Decision

**Three layers, named separately, linked by metadata rather than by string concatenation.**

| layer | what it is | lifetime |
|---|---|---|
| **Key identity** — `node_id` | the fingerprint of a keypair; proves *this message came from this key* | **rotatable**; a reinstall may legitimately produce a new one |
| **Device identity** — `device_id` | a durable record of a *thing*: this iPad, this Mac, this watch. Owns one or more key identities over its life | **durable**; survives reinstall, key rotation, OS upgrade |
| **Human identity** — `handle` | a person in the identity registry ([ADR-0016](0016-multi-human-served-identity.md)) | **durable**, and independent of any device |

### The links are claims, not concatenations

- **key → device** is an ownership record: the device record lists the keys it has used, current
  first. A new key joins an existing device only on evidence — the cleanest being a **rotation
  proof** (the new key's enrolment signed by the previous key). Without evidence it is a *new
  device*, which is the safe answer.
- **device → human** is a **presence claim** ([ADR-0019](0019-friendly-identification.md)) with a
  `via`, a confidence and an expiry — not a fact baked into an actor string. A shared iPad has
  different claims at different hours. A phone has a durable binding. A watch inherits, weakly.

`actor = "phone:ian"` stays as a *rendering* of the current claim, because a great deal of code and
every existing observation reads it. It stops being the place identity is *stored*.

### What attaches to what

- **Standing attaches to the DEVICE**, not the key. That is the whole point: reinstalling your iPad
  should not make you a stranger, and it currently does. A device that cannot prove continuity is a
  new device and starts as a guest — correct and safe, and now a rare event rather than the norm.
- **The dossier attaches to the HUMAN** ([ADR-0022](0022-the-human-dossier.md)). A dossier keyed on
  a churning identifier is a dossier about nobody.
- **Trust and corruption scoring stay on the KEY**, because they are about the behaviour of a
  signer. A compromised key should not be laundered by its device record.

### The Watch, specifically

A watch is its **own device** with its **own key** — it is not a peripheral of the phone. What it is
not is its own *person*. It carries a device→human claim of `via: inherited` with the shortest life
of any rung (a watch on a charger is not its owner, which ADR-0019 already says), plus a
**companion** link in metadata naming the phone it was paired from. That link is what lets the
roster show one human wearing one watch carrying one phone, instead of three actors that happen to
share a name.

## Consequences

**Good.**

- Reinstalling stops creating strangers, which is the single most visible symptom today.
- Ghost members become impossible to create by accident: a key with no device evidence does not
  become a member (see the adoption fix this depends on).
- The roster can finally say "this person, on these devices" — which is what
  [ADR-0022](0022-the-human-dossier.md) needs and cannot currently have.
- Key rotation becomes expressible, which the mesh will need anyway.

**Bad, and accepted.**

- **A migration.** Existing peer records are keyed on `node_id`; each needs a device record minted
  around it. The mapping is one-to-one today, so the migration is mechanical — but it touches the
  roster, the standing roll and every persisted peer.
- **Rotation proof is a new trust surface.** "This new key is the same device" is a claim an attacker
  would love to make. It must be signed by the *previous* key and must never be inferred from a
  label, an IP or a device name.
- **`device_id` is a stable per-device identifier**, i.e. exactly the kind of thing that enables
  tracking. It stays node-local and never federates in raw form, inheriting ADR-0016's
  sensitive-personal rule.
- The three-layer model is more machinery than one string, and the temptation to keep reading
  `actor` as truth will be constant.

## Follow-on work

- Device records + the key-ownership list; mint one per existing peer as a migration.
- Rotation proof at enrolment, and the "new device" fallback when it is absent.
- Move the standing roll from `node_id` to `device_id`.
- Companion links (watch → phone) and their rendering in the roster.
- Only then: the dossier's subject becomes the human handle with confidence, per ADR-0022.
