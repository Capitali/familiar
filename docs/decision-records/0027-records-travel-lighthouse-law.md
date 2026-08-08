# ADR-0027 — Records travel, claims persist, and the lighthouse is the only non-transient entity

- Status: **accepted** (Ian, 2026-08-06) — documented as-built; every mechanism below is
  deployed and was driven into shape by live household testing over 2026-08-04 → 06.
- Amends: [ADR-0026](0026-two-filter-admission.md) (the two-filter door), [ADR-0020](0020-standing-and-the-guest-projection.md)
  (standing), [ADR-0017](0017-status-and-connectivity.md) (the status hub).

## The law

> **The mesh must survive every device being off or disconnected. The lighthouse is the only
> non-transient entity.** Any reliance on a household machine's daemon is a defect; the burden
> moves to the lighthouse. A local Mac daemon ("thick client") is an *offer* — a LAN relay and
> supplement that lets a household mesh run without any lighthouse — never a requirement.

The acceptance test for every mesh feature is one question: *does this work with only the
lighthouse up?* If not, it isn't done. (The lighthouse holds the group secret from its keyed
join, so it can mint admissions alone; it carries the LLM adapter; it is every client's baked
rendezvous; and — by the mechanisms below — it holds the full membership record.)

## Records travel

ADR-0026 made the membership record the one answer to every membership question, but each door
held a private copy, and the household promptly proved why that fails: a claim landed at
whichever door a device happened to dial; the vouch arrived at another; the consoles polled a
third. The loop only closed when everyone happened to share a door.

**Record-sync**: right after each gossip brief exchange, the dial-OUT side offers its
recently-changed records (`GET /mesh/records`) and absorbs the sibling's
(`POST /mesh/record-sync`). Both directions ride the outbound dial because a lighthouse can
never dial into a CGNAT household. The envelope is the brief's own proof shape (cert in our
group, cert certifies the signing key, signature over the canonical body), and absorption is
`merge_records` — admission earliest-wins, corrections union — so re-offering what you were
just offered converges instead of looping. 48-hour window, capped; a door offline longer
catches up on the next live event.

Three merge lessons were paid for in blood (well, in flapping consoles):

1. **A deletion is not a state.** Purged records and finished games resurrected through
   whichever replica had not yet heard. Anything that must *end* travels as a fact — a
   correction, a tombstone with a decisive sequence — never as an absence.
2. **Different game ids are different generations**; last-writer-wins only reconciles *within*
   one. (An inflated tombstone seq plus one second of clock skew smothered fresh games.)
3. **A Disestablish spends only facts older than itself** — in `derive_state` and in merge —
   so the next human establishing fresh on the same hardware supersedes a released identity
   across every door.

## The identity verbs, completed in the field

The two-filter door shipped with four evidence classes; running it with real humans exposed
four missing verbs, all now built:

- **Vouch over the mesh** (E2 without the QR): a refused claim naming an *established* handle
  is **kept on the record** ("a claim addresses", ADR-0019), surfaced as `claims_waiting` to
  member readers, and that human's own device shows one green button. The voucher is minted on
  the established device and delivered to any door (`POST /mesh/vouch`); records now carry the
  device's verified pubkey so a door that never granted the device can still check the
  signature. The card and the admission converge across doors within a sync round.
- **Sponsorship** (the new-human half): a refused claim naming a *new* handle is also kept,
  and every member's welcome screen asks: *"a device here introduces Betty — nobody in the
  mesh holds that name yet. Welcome them?"* One signed member act (`POST /mesh/sponsor`) is
  evaluated as an E4 introduction with `EstablishedDevice` provenance — the rules engine's own
  path, every guardrail intact. This closed the last join pattern: a new human on a new device
  enrolling through the lighthouse arrives with Remote provenance and previously had **no**
  path but a hand-carried invite, while standing physically in the household.
- **Release** (the leaving half of E2's symmetry): a device may never vouch itself in, but it
  may always bow out. `POST /mesh/identity/release`, fired best-effort by the client's SEVER,
  is a **self-Disestablish correction** — the one precise exception to the self-correction
  ban, because renouncing your own name is yours to do. The covenant attestation stays; the
  device is a guest with its contract intact; the peer roster stops naming who left.
- **Restoration**: a certified reader (or status heartbeat) with no record gets its guest
  record restored from the cert it just proved — a cert is only ever minted upon attestation,
  so filter 1 is satisfied by evidence in hand. Without this, a device whose record was lost
  read forever as an invisible ghost: absent from the welcome, unvouchable, unnameable.

## Standing and the roster tell one truth

- `standing_full` (what consoles reconcile membership against) derives from **records** when
  records are the authority — the legacy roll drifts and is now only a shadow.
- The roster's *name* for a device is its record's **established handle** — never a cached
  brief's word. A record with no establishment names nobody, honestly.
- The status hub registers every heartbeating device as a peer and restores its record, so
  the lighthouse's roster is complete even for devices that only ever read through their LAN
  door.
- Client read-loyalty: each door serves its own worldview, so consoles stay loyal to one door
  and only defect after ~15 s of true silence (announced, never silent) — and a Tailscale
  "path upgrade" must answer `/mesh/hello` as the *same node* before it is preferred.

## Severed stays severed

SEVER on a device previously cleared local state and auto-enroll instantly rejoined — the
mesh, still holding the key's record, handed the old identity straight back. There was no way
to leave, and no way to test arriving. Severing now sets a persisted severed-by-human flag;
the join screen says plainly what happened, and a deliberate "Join the mesh" press is the only
way back. Severing also forgets whom the device served.

## Consequences

- The four join patterns all work end-to-end, each through its own honest door: new user/new
  device (introduce → sponsor), old user/new device (claim → vouch, or handoff/invite), new
  user/old device (release → introduce/sponsor), old user/old device (rotation/restoration).
- Doors are interchangeable: claim at one, vouch at another, read from a third — convergence
  within a gossip round (~30 s worst case cross-door; the honest floor while CGNAT means the
  lighthouse cannot push).
- Deeper anti-entropy (beyond the 48 h window) is deliberately deferred until the fleet needs it.
- The thick-client offer (bundled daemon behind an SMAppService gate — "Run a household door
  on this Mac", off by default, App-Store-compatible) is designed, not yet built.
