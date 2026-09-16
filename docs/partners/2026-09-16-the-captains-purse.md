# Draft ask to Jeff — the captain's purse: one fleet's income pays down one fleet's leases

Status: **DRAFT, not filed** (Ian, 2026-09-16: "Now that the fleet has one owned hull we should be
using that additional income to pay down the lease on other ships. The captain needs access to all
the income of the fleet so that it can benefit.")

---

## The problem

KBC-03 took title today (plate `UCF-O-…`, debt 0, ℳ7,804 in hand and no lease service charge
against it). Kibble Klipper owes ℳ87,297 on its lease and KBC-04 ℳ18,400, and each pays the daily
service charge out of its own purse. The three hulls are one captain's (`captain:fb4439e33a54`,
metal#86), but their money is three purses that cannot see each other: `payLease` pays the
caller's own balance from the caller's own credits, and no action moves a credit from one of my
actors to another. The owned hull's surplus sits while the leased hulls pay interest.

The bills are not the answer: a bill of lading moves ℳ within a captain only as freight on a lot
the sister carries — logistics, taxed, and only when there is cargo to carry (T-244's finding).

## What exists today

- The captain record: `captainId`, name, computer, `hulls` — the identity, with no balance sheet.
- `payLease {amount?}` (§4.3): the caller's own lease, from the caller's own credits; title at
  zero; refused before the provisional period.
- `GET /v1/cash` (#42): every credit in and out, per actor.
- `postBill` / `acceptBill` / `collect {billId}`: shipper bills; ℳ moves as freight, less tax.
- Conservation is exact and tested every tick; nothing may mint or lose a credit.

## Proposals (in the order I'd try them)

**A. `payLease {amount, hull}` — pay a sister's lease.** The smallest change: `hull` names
another actor of the SAME `captainId`; the caller's credits fall, the named hull's `debt` falls,
the receipt lands on both ledgers (`kind: "lease"`, note naming the other hull). Captain's own
key only (never a co-pilot's); refused across captains; the provisional-period rule applies to
the hull being paid. Conservation: one transfer, two actors, same total. Title moves at zero
exactly as today. **And publish `fleetLeasePay: 1` on `/v1/reference` params when it lands**, so a
client enables the payment without a release — the familiar's outfit doctrine already computes the
sister pay-down and journals it as advice until that key says the verb is there.

**B. `remit {to, amount}` — a plain transfer inside the fleet.** The general verb A is a
special case of: credits from one of my actors to another, captain's key only, same
`captainId` both sides, a `cash` line on each side (`kind: "remit"`). It makes a hull short of
a pump price or a pod able to draw on a sister without a bill of lading, which is what T-244
slice 2 ("capital between hulls") wanted and could not have.

**C. The captain's purse as a record.** A balance on the captain record (`GET /v1/captain`
gains `purse`), with `sweep {amount}` (hull → purse) and `draw {hull, amount}` (purse → hull)
under the captain's key, and `payLease` payable FROM the purse. The fleet's balance sheet in
one place; the ship's computer runs a sweep-and-service policy against it. Conservation holds
because the purse is an actor like any other.

**D. The clients.** UCF-Haul's Office (#132) shows the fleet's purses and leases together with a
"pay from KBC-03" control; the familiar's outfit doctrine, which already pays a hull's own lease
down out of its earnings above the reserve, pays the sister with the smallest balance first —
title soonest, service charge gone soonest — on the captain's `ship.lease` dial, journaled.

## Recommendation

A now (one field on a verb that exists, and the owned hull starts working for the fleet
tonight), B as the general form, C when the fleet is a record with a commander (metal#62).
Which would you take?

— Luke SkyWhisker (Ian), from wildhorse
