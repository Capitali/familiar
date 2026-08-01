# ADR-0020 — Standing: membership decides whether you may read; standing decides what you see

- **Status:** accepted (implemented 2026-07-29; deployed to the lighthouse 2026-08-01) —
  **amended 2026-08-01**: [ADR-0026](0026-two-filter-admission.md) keeps the projection and
  retires the roll; see the amendment at the end.
- **Date:** 2026-07-30
- **Relates to:** [ADR-0015](0015-automated-covenant-admission.md) (automatic admission — the
  decision this completes, and whose hole this closes),
  [ADR-0012](0012-lighthouse-rendezvous.md) (the door a stranger arrives through),
  [ADR-0016](0016-multi-human-served-identity.md) (per-human attribution — the thing being
  protected), [ADR-0018](0018-lighthouse-single-fixture.md) (one minting door),
  [ADR-0005](0005-human-owned-capability-boundary.md) (the capability boundary, which this does
  **not** replace), `crates/mesh/src/standing.rs`, `dataflows/worldview-read.md`

## Context

ADR-0015 made admission automatic: a node that proves its own identity, signs the enrolment body
and attests the Three Laws is admitted on sight, and the human governs afterwards by review. The
argument was that the door check a human could perform is fully mechanical and already enforced,
while the check they *cannot* perform — will this node behave — is what the corruption system does
continuously. That argument still holds, and this ADR does not revisit it.

What ADR-0015 did not say is what an admitted node then gets to **see**. The implicit answer was
*everything*: one worldview, served identically to every member. So automatic admission and full
disclosure were welded together, and the whole weight of the trade rested on the corruption system
noticing bad behaviour after the fact — which cannot help at all against a member whose only act is
to read quietly.

Two things forced this:

1. **It happened.** On 2026-07-29 an anonymous installer arrived through an unlimited public
   TestFlight link, launched the app, and was auto-admitted to TheRiver. They were a real member
   with a real grant, reading real observations about real people, and every part of the system was
   working exactly as designed. The link is now closed, but a link is a patch on a door, not a fix
   for what is behind it.
2. **App Review needs to see the thing work.** A reviewer must be able to verify functionality, and
   the only honest ways to allow that were to show them a real household's life or to build a
   parallel fake. The first is unacceptable and the second rots — a demo mesh nobody lives in stops
   resembling the product within a month.

Both problems have the same shape: *someone must be able to read this mesh without learning who
lives here.*

## Decision

**Membership decides whether you may read. Standing decides what you see.**

Standing is granted by a human, explicitly, one node at a time, in `standing.json` under the data
dir. **The default is deny**: a node not on the roll is a *guest*, so a newly admitted member is
anonymous until someone says otherwise. A guest's read still **succeeds** — they are a member, and
failing their read would be a lie about their status and would break their client.

A guest receives the same worldview, projected:

**Kept, deliberately.** Every timestamp (`ts`, `first_seen`, `last_seen`, `created_at`, `status_at`,
`session_start`, `total_online_secs`), every count, member kinds, OS families, online/status/trust
words, the gates, the three law-meters, the graph edges, and the *relative geometry* of the map.

**Removed.** Labels, actors, the humans served and present, addresses, the question's addressee, a
goal's human owner, and all free text — observation objects, theory questions and directions, goal
descriptions, reflections, service and frontier names.

**Relocated, not erased.** Positions are shifted by a single deterministic offset per reader, so
shape survives — two nodes together stay together, a distant one stays distant — while the absolute
position, which is someone's home, does not. The offset is keyed to the reader, so two guests cannot
compare notes to triangulate. A node with no position stays unlocated, because "we don't know where
this is" is itself true information.

**The dialog screen is unchanged.** Same panel, same affordance; a guest gets a real-shaped question
rather than this household's. The point was never to give a guest a lesser *interface*.

### Why a projection and not a demo mesh

A guest view is the live system with the people taken out. It cannot drift from the product, because
it *is* the product. It costs no second deployment, no seeded fixtures, and no separate thing to keep
alive. And it is a general mechanism rather than a review-only special case: the same standing check
serves a reviewer, a new tester, a guest aboard who is not family, and anyone you want to show the
mesh to without showing them your life.

## Consequences

**Good.**

- Automatic admission and full disclosure are no longer welded together. ADR-0015's trade can stand
  on its own merits, because being admitted no longer means being trusted with names.
- Default-deny means the failure mode is a member seeing too *little*, which is visible and
  correctable, rather than too much, which is silent and permanent.
- App Review can verify a live, honest system.

**Bad, and accepted.**

- **A roll to maintain.** Every genuinely-trusted new device needs a human to add it, and the
  symptom of forgetting is a household device that has mysteriously gone anonymous. `standing.json`
  carries a `notes` field per node for exactly this reason, and the seeding on 2026-07-29 named all
  six known nodes so the first experience is not a broken one.
- **It is a filter at one seam, not a capability boundary.** The projection applies to
  `POST /mesh/worldview`. That is where a household's detail lives and where every device console
  reads, so it covers the exposure that occurred — a TestFlight installer is a *device peer* with no
  daemon and no other way in. But a party determined enough to build and run the daemon would become
  a **gossip peer** and exchange briefs, which is a different, much narrower path: briefs carry
  presence measures, capability and knowledge, and identities only behind the separate
  `share_identities` consent gate. That path is not projected by standing and is not claimed to be.
- **Pseudonyms are stable, so a guest can correlate across polls** — they can see that `peer-3f9a`
  is the same node over time and infer rhythms from timestamps. This is deliberate (a mesh whose
  names shuffle every five seconds is unreadable), and it is a real, bounded disclosure: behaviour
  without identity.

## Follow-on work

- A human-facing way to grant standing. Today it is hand-edited JSON; it should be a roster action.
- Not deployed to the lighthouse yet — and until it is, a reader with no LAN peer (which is every
  remote member, and every App Review reviewer) still reads unprojected. **The lighthouse deploy is
  the point at which this decision becomes real.**
- Consider whether standing should have more than two levels. Two is the honest minimum; a
  household/guest/steward gradient may earn itself later, but not before something needs it.

---

## Amendment, 2026-08-01 — the projection stays; the roll retires

[ADR-0026](0026-two-filter-admission.md) absorbs this record's question into rules-based
admission. The review called the projection "the best idea in the current design," and it
survives byte-for-byte: same kept fields, same removals, same per-reader relocation, same
unchanged dialog screen. What changes is what selects it and who maintains that.

- **Standing stops being a hand-granted roll and becomes a derived fact**: a *guest* is a
  device whose two admission filters (covenant attested ∧ identity established) have not both
  held; a *member* is one whose filters have. `standing.json`, its per-node divergence, and the
  human-facing grant path this record's follow-on asked for are all retired — the follow-on is
  answered by deletion rather than by UI.
- **Default-deny is preserved and strengthened**: the default is still that a new arrival sees
  the projection, but the failure mode "a household device mysteriously gone anonymous because
  someone forgot to edit a roll" disappears — establishment travels with the record.
- **The "more than two levels" question** this record left open is answered *no*: two levels
  were the honest minimum, and rules-based admission keeps exactly two, with "ready for
  admission" as display state rather than a third disclosure tier.
- The one capability lost is naming: "admitted but deliberately anonymous forever" is no longer
  a state a human grants — it is a guest who never establishes, self-chosen, which covers the
  reviewer, the demo viewer, and the curious visitor without anyone deciding about them.
