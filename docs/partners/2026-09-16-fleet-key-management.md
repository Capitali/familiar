# Draft ask to Jeff — fleet key management: a captain's keys as a record he can read, name and revoke

Status: **FILED 2026-09-16 as ucf-exchange#55** (https://github.com/SpaceTrucker2196/ucf-exchange/issues/55) (Ian, 2026-09-16: "Fleet key management will need to be a thing in
the UCF world as well" and, after #54, "the captain should see all naming of fleet keys and meta tags
and have option update them across the fleet." The roster from `/v1/clients` shows him 17 keys in
his name, 10 of them revoked, minted from four apps.)

---

## The problem

I fly three hulls on PROD and I could not tell you tonight which key is on which device.
`/v1/clients` lists every key the exchange has ever minted for me — seventeen, ten of them
revoked, from four apps — but it names them by the trader label the app sent at enrolment
("Luke SkyWhisker", "Luke Sky-Whisker", "Biscuit") and the hull as it was called then. The
key that flies Kibble Klipper on wildhorse, the one my iPad enrolled KBC-03 with, the
co-pilot key I minted for KBC-04 and the read-only key the familiar's direct mode carries
are four rows that look alike. Nothing on the exchange knows which device holds which, what
each is FOR, or that four of the revoked ones were one afternoon's mistakes.

Now that the captain is a record (metal#86), keys have somewhere to belong.

## What exists today

- `POST /v1/enrol` (app key) mints a captain key; `POST /v1/copilot-keys` mints a delegated
  key under it; `DELETE /v1/copilot-keys/{keyId}` revokes one; `GET /v1/clients` lists the
  roster with `keyId`, `traderName`, `shipName`, `app`, `scopes`, `createdAt`,
  `lastSeenTick`, `revoked`, `actor` — no key material, and no owner beyond the label.
- The captain record: `captainId`, name, computer name, lineage, `hulls`.
- On my side, the familiar's `fleet pair` holds one key per hull in its store and `fleet
  status` says which pilot flies on which; UCF-Haul holds its own on each device.

## Proposals (in the order I'd try them)

**A. Keys hang off the captain, and the captain sees and sets every name on them.**
`GET /v1/captain/keys` (the captain's own key only): every key ever minted for this
`captainId` — captain keys and co-pilot keys — with the `clients` row's fields plus every
name and tag the wire keeps on a key: `traderName` (the handle a wall note signs with, #54),
`label` (free text, up to 32: "iPad", "wildhorse pilot KK", "read-only for the familiar"),
`hull` (the hull it is bound to), `app`, `mintedBy` (which key minted it), `lastSeenApp`.
`PATCH /v1/captain/keys/{keyId} {traderName?, label?, hull?}` sets any of them on one key;
`PATCH /v1/captain/keys {traderName}` sets the handle on EVERY key of the captain at once, so
"I am Luke SkyWhisker on all of my keys" is one request rather than one per device.
`DELETE /v1/captain/keys/{keyId}` revokes any of them (today only co-pilot keys can be
revoked by the captain; a lost captain key has to be reported to you). Revoking the key you
are calling with is refused.

**B. Mint from the record.** `POST /v1/captain/keys {kind: "captain" | "copilot", hull,
label, scopes}` — one door for both kinds, so a new device is enrolled BY the captain rather
than as a stranger who then has to be adopted (the adoption step metal#86 needed today).
A key minted this way is born under the captain record, named, and bound to a hull.

**C. The office page in UCF-Haul.** The Office shows the captain and the ship's computer
(#132); under them, the keys: label, hull, device, last seen, revoke. The familiar's bridge
shows the same list off the same route, and `fleet pair` sends the label it was given.

**D. Housekeeping the past.** A one-time `captains keys adopt` on the box that attaches
every existing key to the captain its `traderName` (case- and space-folded) resolves to, so
the seventeen rows in my name become one captain's list on day one.

## Recommendation

A and D together (the list is the whole problem; the labels are what make it usable), C as
soon as A exists, B when a second device enrols under a captain rather than beside one.

— Luke SkyWhisker (Ian), from wildhorse
