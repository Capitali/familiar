# ADR-0015 — Automated covenant admission: the process is the consent

- **Status:** accepted (in force on TheRiver)
- **Date:** 2026-07-27
- **Relates to:** [ADR-0009](0009-sovereign-mesh-transport.md) (the covenant
  transport), [ADR-0012](0012-lighthouse-rendezvous.md) (the lighthouse as the
  primary door — this is what completes it),
  [ADR-0005](0005-human-owned-capability-boundary.md) (which said admission is a
  human act — this ADR revises *when* that act happens), the corruption-awareness
  trust system (`crates/kernel/src/corruption.rs`)

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
