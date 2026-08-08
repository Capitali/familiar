# ADR-0015 — Automated covenant admission: the process is the consent

- **Status:** accepted (in force on TheRiver) — **amended 2026-07-30** after the policy was
  exercised by a stranger for the first time, and **again 2026-08-01** when
  [ADR-0026](0026-two-filter-admission.md) completed the design this record started; see the
  amendments at the end.
- **Date:** 2026-07-27
- **Relates to:** [ADR-0009](0009-sovereign-mesh-transport.md) (the covenant
  transport), [ADR-0012](0012-lighthouse-rendezvous.md) (the lighthouse as the
  primary door — this is what completes it),
  [ADR-0005](0005-human-owned-capability-boundary.md) (which said admission is a
  human act — this ADR revises *when* that act happens), the corruption-awareness
  trust system (`crates/kernel/src/corruption.rs`),
  [ADR-0020](0020-standing-and-the-guest-projection.md) (what an admitted node may
  **see** — a question this record left implicit, and which ADR-0020 answers)

## Context

ADR-0009/0012 made admission a per-join human act: an enrolling device filed a
`Pending` and a steward ran `mesh approve <code>`. In practice this hangs. When
the lighthouse is the primary door (ADR-0012), the pending is filed **on the
lighthouse** — a headless VPS the human never looks at — so a join waits forever
with no surface to approve it from. Timed invite windows (`mesh invite`) are a
race: the window expires before the human gets to the device, or opens a blanket
auto-admit anyway. The per-join gate was friction without added safety: the
human, tapping "approve" on a six-character code, was not actually verifying
anything a machine hadn't already verified.

## Decision

Admission is automated. `auto_accept_enrollments = true` is the standing policy
on every mint-capable door (the lighthouse and wildhorse). A node is admitted the
moment it:

1. presents a **self-certifying identity** — its `node_id` is the fingerprint of
   the public key it presents (forgery-proof), and
2. **signs** the enrollment body with that key, and
3. **attests the Three Laws** — a non-empty covenant statement, signed.

No per-join human approval. No timed window. The human consents **once, to the
process** — this ADR — rather than to each join. That consent is real: the Three
Laws covenant is the contract every node signs, and the human authored and
accepted the admission policy itself.

The human stays in control, but *after* admission rather than *before*:

- **Roster review** — the worldview/roster shows every member with its
  provenance: `first_seen` (when it joined), `present_human`, trust standing, and
  attached sub-devices (ADR-0012 roster work). A human can see who is on the mesh
  and when they arrived.
- **Anti-corruption + rule checks** — the corruption-awareness trust system
  monitors every admitted member continuously and can throttle → marginalize →
  sever a node whose behaviour slips, with no human action. The Three Laws
  boundary gates reject any directive that violates the covenant, per node,
  every cycle. This is the real gate, and it is *behavioural and ongoing*, not a
  one-time yes/no at the door.
- **Revoke** — `familiar mesh abandon <node_id>` removes a member; any fresh
  contact revives only if re-admitted under policy.

The security argument: the door check a human could perform (does this node hold
its key? did it sign? did it accept the Laws?) is fully mechanical and already
enforced. What a human *cannot* do at the door — judge whether a node will behave
— is exactly what the corruption system does continuously afterward. So moving
the human out of the join loop and into the *review* loop loses no safety and
removes the hang.

## Consequences

- `auto_accept_enrollments` stays **false by default in code** — a conservative
  default for other deployments and for the authority-proxy posture (a headless
  node that deliberately routes each join to a human). TheRiver opts in
  explicitly; the switch is a deliberate, recorded human decision, not implied by
  `allow_mesh`.
- The `Pending`/`approve`/`deny`/`invite` machinery remains for deployments that
  keep the human at the door; it is simply not on the critical path for TheRiver.
- **Follow-up — deferred, sequenced behind facial recognition (Ian, 2026-07-27):**
  making roster review *actionable* (revoke/sever, a "newly joined, not yet
  reviewed" flag) is an **administrative action**, and administrative actions must
  eventually become **authenticated** — the operator proves they are an authorized
  human before the action takes effect. The authenticator is **facial
  recognition**; the surface is a **voice/audio interface** ("familiar, remove
  that node" → face-verify → act). So this does *not* get built as an unauthenticated
  console tap now. It moves **downstream of the facial-recognition effort** (on the
  roadmap; see the multi-human identity work) and lands as part of an authenticated
  admin layer over voice. Until then, review is via the roster and revoke is via the
  CLI (`mesh abandon`) — deliberately, so no un-authenticated one-tap revoke exists
  in the shipped console. The automated-admission policy above stands on its own in
  the meantime; this only concerns the *human-control* half becoming convenient
  *and* authenticated together, not one before the other.
- Reversible: `mesh auto-accept off` restores per-join approval instantly (read
  live per request; no restart).

---

## Amendment, 2026-07-30 — tested in the wild, and found incomplete rather than wrong

On **2026-07-29** this decision was exercised by a real stranger for the first time. An anonymous
installer arrived through an unlimited public TestFlight link, launched the app, and was admitted to
TheRiver. They were a genuine member with a genuine grant, reading real observations about real
people, and **every component behaved exactly as specified here**.

### What that did and did not test

It did not test the admission *logic*, which was never in question. The joining node was
self-certifying, signed its enrolment body, and attested the Three Laws — it satisfied every check
this ADR says is sufficient, because those checks are sufficient *for what they check*. Nothing
failed.

What it exposed is that this record answered **"may this node join?"** and left
**"what may a joined node see?"** entirely unstated — so the implicit answer was *everything*.
Admission and full disclosure were welded together without either being argued for.

That matters because of the specific shape of the safety argument above. The trade was: move the
human out of the door and into the review loop, because the corruption system watches behaviour
continuously and can throttle → marginalize → sever. **That defence cannot engage against a member
whose only act is to read quietly.** Reading is not misbehaviour, produces no signal, and decays no
trust score. The one thing the post-hoc gate cannot catch is precisely what happened.

### What changed as a result

[ADR-0020](0020-standing-and-the-guest-projection.md) supplies the missing half: membership decides
whether you may read; **standing** decides what you see, is granted by hand, and is **denied by
default**. Admission stays automatic. Disclosure no longer rides along with it.

With that, the argument in this record holds as written — but it holds because the thing it was
implicitly promising (full access on admission) is no longer what admission grants. The decision is
**amended, not reversed**.

### Two corrections to the text above

- *"every mint-capable door (the lighthouse and wildhorse)"* is now **the lighthouse alone**.
  [ADR-0018](0018-lighthouse-single-fixture.md) reduced minting to one door; `wildhorse` still holds
  a secret as of this writing, and removing it is tracked there.
- *"The human consents once, to the process"* remains true, but the process they consented to has
  changed shape and is now the more defensible one: automatic admission **plus** default-deny
  standing. A human who agreed to the former was not thereby agreeing to hand their household's
  names to anyone who could reach the door.

### The lesson that is not about the door

The vector was **distribution**, not admission. An unlimited public TestFlight link is a standing
invitation to be admitted, and no door policy compensates for handing out the address. That link is
now closed. Worth stating plainly because the instinct after an event like this is to tighten the
gate that did not fail.

---

## Third amendment, 2026-08-01 — the automation extends to the whole admission

[ADR-0026](0026-two-filter-admission.md) finishes what this record began. The first amendment
split admission from disclosure and left disclosure to a hand-granted standing roll — a second
gate, bolted on to compensate for removing the first one. ADR-0026 removes the bolt-on by making
the whole admission **rules-based**: a device is admitted the moment the covenant is attested
*and* the human identity is **established by evidence** — cryptographic continuity, a member's
deliberate act displaced in time, or an introduction made in the mesh's own space. Until then it
is a guest reading the projection, which remains the floor this record's first amendment laid.

Three corrections to the machinery named above:

- `auto_accept_enrollments` no longer exists as a switch — in either direction. Evidence gates
  admission now, not a boolean, so there is nothing to toggle and no per-deployment posture to
  record.
- The `Pending`/`approve`/`deny`/`invite` machinery this record kept "for deployments that keep
  the human at the door" is retired with it. What survives of "deny" is the correction plane's
  *hold* — the same five-minute not-now, now a field on the record.
- The deferred follow-up above (authenticated admin actions sequenced behind facial recognition)
  is unchanged in spirit but now applies to **corrections** (sever / disestablish / hold), which
  are the only administrative acts left at the door. Until it lands, corrections live on the
  roster card and the CLI, exactly as un-ceremonious as this record wanted revocation to be.

The consent argument holds in its strongest form yet: the human consents once, to a process
whose every admission is a signed, attributable fact produced by rules the human authored — and
what the human governs afterwards is no longer a roll to remember but a record that answers.
