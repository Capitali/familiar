# ADR-0026 — Two filters, one record: admission is rules-based, and the welcome is a greeting

- **Status:** proposed — written as the Phase 0 gate of the join/authorize rebuild; in force
  when Ian accepts it. Everything downstream of this record waits on that acceptance.
- **Date:** 2026-08-01
- **Relates to:** the architectural review and planning brief that called for this
  ([`docs/reviews/2026-08-01-join-and-authorize.md`](../reviews/2026-08-01-join-and-authorize.md),
  [`docs/reviews/join-and-authorize-BRIEF.md`](../reviews/join-and-authorize-BRIEF.md)),
  [ADR-0025](0025-device-identity-is-not-key-identity.md) (key ≠ device ≠ person — the
  prerequisite this builds on), [ADR-0015](0015-automated-covenant-admission.md) (automated
  admission — completed here, third amendment), [ADR-0019](0019-friendly-identification.md)
  (whose invariant is amended here, openly), [ADR-0020](0020-standing-and-the-guest-projection.md)
  (the projection survives; the roll does not), [ADR-0021](0021-live-roster-and-the-record.md)
  (whose 24-hour split gains a third leg), [ADR-0018](0018-lighthouse-single-fixture.md)
  (superseded in part by the warrant work this schedules), [ADR-0005](0005-human-owned-capability-boundary.md)
  (the capability boundary, which — as every membership record must say — this does **not** move)

## Context

Ten mechanisms in six file formats answer one question — *may this thing be here, and what
may it see?* — and none of them federate, so the answer depends on which node you ask. The
review traced the nest to three roots: two ways to become a member (enrolment and adoption),
standing existing only because admission stopped asking, and key/device/human welded into one
string. ADR-0025 answered the third. This record answers the first two.

The owner's direction, given in prose and binding here: **admission is automatic — rules-based.
This is a trust-building exercise, so we trust.** A client remains an anonymous guest until two
things are qualified: the human identity is established, and the device has contracted to the
Three Laws. Once both are in place, the user and device are admitted. And the welcome screen is
not a gate: *"it could just be a place that shows the last 24 hours of new members/devices/
agents/peers — 'welcome to these new members.'"*

## Decision

### 1. Two filters, and nothing else

A device becomes a member the moment both filters hold, automatically, with no approval step:

1. **The device contract** — the covenant handshake: a self-certifying identity signing an
   attestation of the Three Laws. Mechanical, already built, satisfied at the knock.
2. **The human identity is established** — by evidence (§3), not by assertion.

Until both hold, the device is a **guest**: its reads succeed and return the
[guest projection](0020-standing-and-the-guest-projection.md) — the live mesh with the people
taken out. A guest that never establishes identity stays a guest forever, and that is a feature,
not a queue: the demo viewer, the App Review reviewer, and the curious visitor are all guests by
construction, needing nobody's action and nobody's apology. **"Ready for admission" is a display
state** — a guest with exactly one filter unsatisfied, told plainly which one — never a stored one.

```
        knock (covenant attested)               identity established
unknown ─────────────────────────▶ guest ─────────────────────────▶ member ──▶ severed
                                  (projection;   automatic, the moment    (correction)
                                   stable forever both filters hold)
                                   if never       ◀── disestablish ──┘
                                   established)
```

### 2. One record, three rules

One **MembershipRecord** per device replaces the ten mechanisms — membership cert, pending,
invite window, auto-accept switch, deny records, standing roll, peer record, candidate ledger,
abandoned flag — as the sole authority on a node's membership question:

```
MembershipRecord {
  device_id            // ADR-0025: the durable thing; node_id is just the current key
  keys[]               // current first; an E1 rotation proof is required to add
  state                // guest | member | severed { reason, at }
  identity {
    claim              // "says they are Betty" — addresses only, admits nothing
    established        // the evidence that satisfied filter 2, class and artifact
  }
  admitted             // signed AdmissionFact: which node's rules admitted, when, on what
  held_until           // correction cool-off — a field, not a directory
  corrections[]        // the signed reversals (§5)
  attestation          // RETAINED — a node can be held to what it accepted
  first_seen, last_seen
}
```

The three rules from the review, unchanged, because they are what stop this re-nesting:

1. **One admission path.** Only the rules engine creates members. Adoption may update
   `last_seen` on an existing record and may do nothing else, ever.
2. **Decisions travel; state replicates.** Corrections are signed and go to the record's
   authority; every node reads the record back. No client writes authority state.
3. **The record is the only answer.** If a question about a node cannot be answered from its
   record, the answer does not exist.

Records replicate. An `AdmissionFact` merges earliest-wins — it is idempotent, because every
door runs the same rules on the same evidence. A `Correction` merges latest-wins — it is a
deliberate reversal, not a race. First-decision-wins *voting* is no longer needed anywhere,
because nothing is voted on.

Orthogonal and deliberately untouched: `trust` (the corruption ladder, attached to the **key**,
because it scores a signer's behaviour), `stale` (derived from `last_seen`, ADR-0021), and the
capability boundary (ADR-0005 — being admitted grants capability over nothing).

### 3. What establishes an identity

This is the load-bearing clause. Rules-based admission is exactly as safe as the rules, so the
rules are evidence-classed and machine-checkable. **A typed claim addresses; evidence
establishes.** Four classes:

| class | artifact | what it proves |
|---|---|---|
| **E1 — rotation proof** | the new key's enrolment signed by the device's previous key (ADR-0025, verbatim) | same physical device, so the established human link carries over |
| **E2 — device voucher** | a continuity statement signed by a device already bound to the claimed handle, produced by a deliberate physical act — scanning the handoff code on the old device, the phone→watch link | the claimed human's own hardware vouches for the new one |
| **E3 — invite token** | a single-use, ten-minute, member-signed token (minted at `/local/invite`, which **stops carrying the group secret**); may name the expected handle | an existing member's deliberate act, displaced in time |
| **E4 — local introduction** | the introduce-yourself interaction — name entry, face, voice — **with provenance**: made on a network where a member device is colocated, on an already-established device, or at founding | a new human, introduced in a place the mesh actually inhabits |

Two guardrails, and they are rules rather than judgements:

- **Claiming an existing handle never establishes via E4.** Only E1/E2/E3 naming that handle
  can attach a new device to an existing human. Typing "I am Betty" cannot become Betty; the
  record shows it as an unestablished claim — *someone says they're Betty* — which is a fact
  worth displaying and no more.
- **A pure remote stranger cannot establish.** Rendezvous-only arrival plus a typed name
  satisfies no class; they remain a guest reading the projection, with the UI saying exactly
  what is missing. This preserves ADR-0015's hard-won lesson — distribution is the real
  vector — while costing the honest newcomer nothing: an invite, a handoff, or being in the
  room all work immediately.

**ADR-0019 is amended by this record, openly.** Its invariant — *identification addresses; it
never authorises* — becomes: **a claim addresses; establishment admits.** Recognition alone
still never admits anything: every establishing class is either cryptographic continuity or a
deliberate human act. What changed is that the deliberate act moved from a third party approving
at a door to the arriving human (or their own hardware, or their inviter) producing evidence.
Sensitive-personal rules are untouched: face signatures never federate, a guest sees no dossier,
and being recognised still unlocks nothing that was not already yours.

### 4. The welcome is a greeting

ADR-0021 split *who is here* (the live roster, 24 hours) from *who has been* (the history).
The welcome screen becomes the third leg of the same split: **who is new.** It lists every
record whose admission, first sighting, or new-human registry entry falls within the last
24 hours — members, devices, agents, peers — as *"welcome to these new members"*: the handle,
the device kind, how the identity was established (*via handoff from Betty's old iPad*,
*invited by ian*, *introduced themselves here*), and when. Guests appear as arrivals too — *a
visitor is looking around* — with no pending-decision framing, because there is no decision
pending.

**No buttons.** The recognise/not-now affordances are deleted. The door glyph's pulse and the
arrival chime become greetings — *someone new has joined* — edge-triggered and launch-silent
exactly as built. The acceptance chime keeps both of its sides: the arriving device feels the
moment, and every member console greets it, because the arriving user's transition is precisely
what feeds everyone else's welcome list. The kernel question minted at every admission — *"who
does it belong to?"* — is retired; nothing waits on its answer. In its place, an informational
observation joins the feed.

### 5. Correction, where administration honestly lives

Trust extended automatically must be cheap to withdraw deliberately. Corrections live on the
roster/device card and the CLI — never on the welcome screen — and are low-ceremony, per
ADR-0022: a field edited twice a year should not look like a field that needs attention.

```
Correction { act: sever | disestablish | hold | restore,
             subject_device, corrected_by, reason, ts, nonce, sig }
```

- **Sever** revokes membership (absorbing `mesh abandon` + `revoked.json`).
- **Disestablish** is *"that's not Betty"*: clears the establishment, the record drops to
  guest, and the same evidence artifact is held out for the cool-off window.
- **Hold** is the existing five-minute not-now, future-dated-denial guard included; **restore**
  is the undo. A denial remains "not now", never a ban.

`POST /mesh/correct` inherits the signed-member verification shape built for `/mesh/standing`;
a device cannot correct itself; corrections travel with the record. What survives of the
standing-vote machinery is exactly this transport pattern — the vote itself, the grant path,
`standing.json` as a store, the deny directory, the candidate ledger, invite *windows*, and
`auto_accept_enrollments` as a switch are all deleted. The switch dies with particular honour:
`vps/README.md` always said auto-accept must stay off on a public node, and under this record
it finally can — evidence gates admission now, not a boolean.

### 6. Any member node can hold the door (grow without the lighthouse)

Admission that is rules-based can be evaluated anywhere the rules and the record are. The group
key signs a **minting warrant** for a member node's key; verification walks cert → warrant →
group public key, so any peer can check a membership no matter which door minted it. A warranted
node runs the same rules engine on the same evidence classes; an unwarranted node relays the
knock (the one-hop relay already built). The lighthouse becomes what `vps/README.md` always
said it was — an ordinary peer the network granted a good address — and rendezvous becomes a
convenience any well-addressed peer can offer.

This supersedes ADR-0018's first decision ("one online minting door") in the direction that
record itself named as the endpoint: *"it should eventually be made redundant and physically
distributed."* Its fear — two automatic doors are two attack surfaces — inverts under this
design: what made the old door dangerous was that *reaching it* sufficed. Now every door runs
the same mechanical rule set, whose satisfying evidence a stranger cannot produce by reaching
an address; what distributes is rule *evaluation*, not policy; and every admission is a signed,
attributable fact that travels with the record.

## Consequences

**Good.**

- One record, one lifecycle, one answer — and the answer is the same on every node you ask.
- The four user/device patterns fall out of the two filters instead of needing four flows:
  new user/new device (knock + invite/introduction/founding), old user/new device (knock +
  handoff/voucher — no second person), new user/old device (only the human step is pending),
  old user/old device (a non-event, at last: a reinstall with a rotation proof keeps the
  record, and ghosts stop being mintable by accident).
- The hang that motivated ADR-0015 cannot recur: there is no human in the admission loop to
  fail to be asked. The guest state is stable, honest, and self-serve.
- The reviewer story gets stronger: App Review reads a live mesh as a guest by construction.

**Bad, and accepted.**

- **Impersonation and evidence-gaming replace the hang as the top risk.** The guardrails, the
  single-use tokens, and a `disestablish` that travels are the mitigations. Residual and
  accepted: a stolen unlocked member phone can voucher an attacker's device — the same trust a
  stolen phone already carries everywhere else in its owner's life; `sever` is the recourse.
- **No human in the loop means arrivals are learned after the fact.** Mitigated by design
  rather than gate: the celebration surfaces within one poll cycle, with a chime; invites and
  handoffs mean arrivals are almost always expected; correction is cheap and travels.
- **It is a migration** — one, covering ADR-0025's device records and this record together,
  because both touch every persisted peer and migrating twice would be twice the risk.
- **"Admitted but deliberately anonymous forever" is no longer a granted state** — it is now
  simply a guest who chose not to establish. That loses nothing the projection did not already
  provide, and it removes a decision nobody should have had to make.

## Follow-on work

- **Phase 1 — evidence-path prototype (go/no-go):** E1 survives a reinstall; E2 establishes in
  under thirty seconds with no third party; E3 enforces single-use and expiry; the two negative
  cases — an existing-handle claim and a remote stranger's typed name — do **not** admit; and
  arrivals reach every polling console inside ten seconds. The negatives are non-negotiable.
  (Also: fix the plaintext `enroll::http()` against the TLS port, which blocks every Rust-native
  join today.)
- **Phase 2 — the record and the one migration**, dual-written behind a read flag, with a
  `mesh doctor` equality check and a ghost sweep.
- **Phase 3 — the two-filter door, the celebration welcome, and the correction plane**, with
  one release of wire compatibility for build-43 clients.
- **Phase 4 — warrants and the kill-the-lighthouse drill**: a LAN join completing with the VPS
  down, and the real escrow finally exported before `wildhorse` is reduced.
- Capability gates stay **local-only** for now (the D2 question): consoles render other nodes'
  gates read-only rather than pretending. Revisit once ADR-0025's device→human binding exists.
- Where Ian may wish to loosen: the E4 provenance rule (whether a typed name from a pure remote
  stranger should ever establish) and the demo-mesh window for the cold-start founder. Both are
  single rules in one engine, changeable by policy rather than surgery.
