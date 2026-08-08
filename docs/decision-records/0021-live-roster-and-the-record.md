# ADR-0021 — The roster answers who is here; the record answers who has been

- **Status:** accepted (implemented in the console 2026-07-29) — **amended 2026-08-01**: the
  split gains a third view; see the amendment at the end.
- **Date:** 2026-07-30
- **Relates to:** [ADR-0017](0017-federated-status-and-connectivity.md) (the status directory and
  its 5-minute TTL — the mechanism this rests on and the source of the second confusion below),
  [ADR-0018](0018-lighthouse-single-fixture.md) (the lighthouse as the only place the full
  membership roll exists), [ADR-0019](0019-friendly-identification.md) (presence claims, which
  answer *who* is here rather than *what* is),
  [ADR-0020](0020-standing-and-the-guest-projection.md) (a guest sees this same split),
  `crates/mesh/src/transport.rs` (`apply_status_freshness`), the sphere console's roster screen

## Context

The roster was answering two questions at once and therefore answering neither well.

`FamTalker01` sat in the live roster between two members seen seconds ago, having last spoken
**25 hours** earlier. Nothing about the panel said so at a glance; you had to read the SEEN cell of
every card to work out which rows meant anything. A roster you have to audit before you can trust it
is not a roster — and the failure is quiet, because it degrades exactly as the mesh grows and old
devices accumulate.

Underneath that was a second, sharper confusion: **"not here right now" and "not a member" looked
identical.** Two different mechanisms produced it:

1. `status::directory` is TTL-filtered to five minutes, by design (ADR-0017 — presence should go
   dark quickly). So the federated view only ever names the currently-live.
2. `apply_status_freshness` only ever *bumped* peers it already held, and silently dropped every
   directory row it did not recognise. Since the lighthouse is the only minting door (ADR-0018), in
   practice that meant every remote member. A tester on the far side of the country could be
   enrolled, live and heartbeating, and simply not exist in any other node's roster.

Together those meant a member who was merely asleep and a member who had never existed were the same
non-event. You could not ask "who is on my mesh?" from any device you own.

## Decision

**Split the two questions and answer each one properly.**

- **The live roster answers "who is here."** A member not seen for **24 hours** leaves it.
- **A history answers "who has been."** Behind a button on the roster screen, a condensed table:
  last seen date and time (UTC, to the minute), role, OS and version, cumulative time in the mesh,
  the human served, position, and how it was reaching the mesh. The button appears only when there
  *is* a history — an always-present empty toggle is furniture.
- **A member returns to the live roster automatically** on any fresh contact. Nothing is archived,
  moved or forgotten; this is a view, not a state transition.
- **Unknown members are adopted from the status directory**, so the record has something to record.
  Only when demonstrably ours: matching `group_ref`, never our own node, and never a row carrying no
  `group_ref` (an older node we cannot attribute). `first_seen` is stamped from *our* first sighting,
  not the lighthouse's, because that is the only thing this node can honestly attest to.

### Why 24 hours

It is a judgement, not a derivation. It wants to be far longer than the status TTL (5 minutes — a
phone in a pocket must not vanish from the roster) and far shorter than the withdrawal horizon
(3 days — by which point the question is whether someone has left, not whether they are in the room).
A day is the interval at which "I haven't seen that thing since yesterday" becomes true in ordinary
speech. It is one constant, `STALE_SECS`, and moving it is cheap.

## Consequences

**Good.**

- The live roster can be trusted at a glance, which is the only way a roster is useful.
- The history is *more* informative than the old mixed list — it carries precise last-seen times,
  versions and cumulative presence that the card layout had no room for.
- The two mechanisms compose: adoption gives the record content, and the split gives it somewhere
  to live.

**Bad, and accepted.**

- **A peer's history is only what that peer has witnessed.** A node that never met a member has no
  record of them, and there is still no federated membership *roll* — the full list of who has ever
  been admitted exists only on the minting door (ADR-0018). So this answers "who has been here, as
  far as I know", not "who belongs to this mesh". Those are different questions and only the first is
  now answerable from a device.
- **A member seen once, briefly, and never again looks the same as a long-standing one who left.**
  `first_seen`, `total_online_secs` and the version columns are what distinguish them, and reading
  that distinction is on the human.
- **The 24-hour line will occasionally be wrong** — a boat device that reports twice a week is
  permanently in the history despite being a healthy, expected member. If that pattern becomes
  common the answer is a per-member expected cadence, not a longer global constant.

## Follow-on work

- Publish the membership roll from the minting door so "who belongs" becomes answerable from a peer,
  and the history can distinguish *departed* from *never met*.
- Consider a per-member cadence so an intermittent-by-design node is not treated as stale.

---

## Amendment, 2026-08-01 — the split gains a third view: who is new

This record split one confused question into two honest ones — *who is here* (the live roster)
and *who has been* (the history). [ADR-0026](0026-two-filter-admission.md) adds the third leg of
the same split: **who is new**. The welcome screen becomes an arrivals view over the same
24-hour judgement (`STALE_SECS`, one constant, shared): every record whose admission, first
sighting, or new-human registry entry falls within the last day, rendered as a greeting —
*"welcome to these new members"* — with the handle, the device kind, how the identity was
established, and when. It carries no buttons and frames no decision, because under ADR-0026
there is no decision pending at the door.

The follow-on above — publish the membership roll so "who belongs" becomes answerable from a
peer — is delivered by ADR-0026's replicated record rather than by a separate publication: the
record travels, so the history can finally distinguish *departed* from *never met*.
