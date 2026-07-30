# The group secret: escrow and recovery

> **What this protects.** [ADR-0018](../docs/decision-records/0018-lighthouse-single-fixture.md)
> makes the lighthouse the only node that mints members. That is a deliberate concentration of
> authority, and it is only survivable because of what is written here. Without an escrow, losing
> the lighthouse means **no device can ever join this group again** — existing members keep working
> indefinitely, but the group can never grow, and a reinstalled phone can never come back.
>
> With an escrow, losing the lighthouse is an outage.

## What the escrow is

One 32-byte secret. It is not a backup of a machine; it is **the authority to admit members**.
Anyone holding it can mint a membership certificate that every node in the mesh will trust.

Treat it accordingly:

- **Offline.** Not in a repo, not in a password manager that syncs to a vendor cloud you don't
  control, not in the data dir of a running host other than the minting door.
- **Two copies, two places.** One is a single point of failure wearing a different hat.
- **Encrypted at rest**, with a passphrase you can actually reproduce in a year.
- It is **not** rotatable without re-admitting every member, so losing control of it is worse than
  losing it. See *Compromise*, below.

## Exporting (do this before you need it)

The escrow can only be taken from a node that still holds the secret. As of 2026-07-30 that is the
lighthouse and `wildhorse` — the second being an accident of history this procedure exists to end.
Prefer the **lighthouse**: it is the designated holder, and keeping `wildhorse` out of the recovery
story entirely is the point. `wildhorse` is a user device, the oldest hardware here and expected to
be replaced; nothing about the group's survival may depend on it. Its last act as a secret-holder is
to be reduced (below), after which replacing that machine is an ordinary re-join.

```sh
# On a node that can mint. Writes JSON to stdout; never leave it on disk unencrypted.
familiar mesh escrow-export | age -p > ~/group-escrow-$(date +%Y%m%d).age
```

The exported document carries `kind`, `group_id`, `label`, `group_pubkey`, `group_secret`,
`exported_at`. The `group_pubkey` is there so a restore can prove it reconstituted the **right**
group before writing anything.

`export_escrow` **fails** on a node holding a covenant credential. This is deliberate: an escrow
file containing an empty secret is worse than no file, because it looks like insurance.

## Verifying an escrow without using it

Check the secret derives the group's public key. If it does, that file will work.

```sh
age -d ~/group-escrow-20260730.age | python3 -c '
import json,sys,hashlib
d=json.load(sys.stdin)
print("kind    :", d["kind"])
print("group   :", d["label"], d["group_id"])
print("secret  :", "present" if d.get("group_secret") else "MISSING — THIS FILE IS USELESS")
'
```

For a full check, run the round trip in a scratch data dir rather than trusting the eye — the same
procedure the automated rehearsal performs (below).

## Restoring after losing the minting door

1. **Stand up a replacement node** and join it to the group by covenant, as any peer would. It gets
   its own key and its own membership. Do not try to restore onto a machine that is not already a
   member — restore grants minting authority; it does not create identity.
2. **Restore the secret onto it.**
   ```sh
   age -d ~/group-escrow-20260730.age | familiar mesh escrow-restore
   ```
3. **Point the mesh at it.** Clients carry a baked rendezvous host and pin
   (`AppModel.rendezvousHost` / `rendezvousPin`). A replacement lighthouse at a new address or with
   a new TLS key means **shipping a client update** — existing members fail over to learned hosts
   and pins, but a *fresh install* only knows what was baked in. Budget for this: it is the slowest
   part of the recovery, not the restore itself.

`restore_from_escrow` refuses three things rather than guessing: an unknown `kind`, an escrow for a
different `group_id`, and a secret that does not derive this group's public key. The third matters
most — a mismatched key would mint certs no existing member could verify, which is a worse outcome
than refusing to restore.

## Reducing a node to a covenant credential

Once the escrow exists **and has been verified**, a node that no longer needs to mint should stop
holding the secret:

```sh
familiar mesh reduce-to-covenant --yes   # strips group_secret; keeps identity + membership
```

`--yes` is required and is not ceremony. This is irreversible without the escrow, and until an
escrow exists a second node holding the secret *is* your redundancy — stripping it first makes the
group less recoverable, not more. Without the flag the command refuses and exits non-zero.

Restore refuses, without touching the existing credential, on: an escrow for a different group, a
secret that does not derive this group's public key, an unrecognised format version, and anything
that is not an escrow document at all. Verified through the CLI, not just the library.

## The custodian is transient too

[ADR-0018](../docs/decision-records/0018-lighthouse-single-fixture.md) says no device is permanent
and **no human is permanent**. An escrow held by exactly one person honours the first half and
quietly breaks the second: it moves the single point of failure from a rented VPS to a single pair
of hands.

This is not solved. What it needs is a succession story — a second custodian, a split secret
requiring two of three to reconstitute, or a sealed instruction that outlives its author. Until one
exists, write down **where the escrow is and how to decrypt it** somewhere a successor would
actually look, and treat that note as part of the escrow.

## Compromise

If the secret leaks, an attacker can mint members at will and every node will trust them. There is
no revocation of the secret itself, only of individual members
(`familiar mesh abandon <node_id>`). Recovery is a **new group**: create it, re-admit every device
by covenant, and abandon the old group id. Plan on losing federated history that lived under the old
group. This is the strongest argument for keeping the escrow offline and in few hands.

## The rehearsal

A procedure nobody has run is a hope. The full round trip — export, reduce to covenant, prove
minting then **fails**, restore, prove minting works again and that the resulting certificate
verifies under the original group key — runs as a test on every build:

```sh
cargo test -p familiar-mesh the_escrow_survives_losing_the_minting_door
cargo test -p familiar-mesh a_restore_refuses_the_wrong_group_and_the_wrong_secret
```

That covers the *mechanism*. It does not cover **your** escrow file, your passphrase, or your ability
to find either under pressure. Rehearse those by hand at least once, and again whenever the storage
changes.

## Status

| | |
|---|---|
| Mechanism implemented | ✅ `crates/mesh/src/group.rs` |
| Round trip rehearsed automatically | ✅ two tests, every build |
| CLI surface (`escrow-export` / `escrow-restore` / `reduce-to-covenant --yes`) | ✅ wired, procedure walked end to end 2026-07-30 |
| An escrow actually exported and stored | ❌ **not yet done** |
| `wildhorse` reduced to a covenant credential | ❌ blocked on the above |
| Succession plan for the custodian | ❌ owed — see above |
