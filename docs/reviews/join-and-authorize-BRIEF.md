# Planning brief — mesh join & authorize rebuild

**For a dedicated planning session. Self-contained: start here cold.**

Full analysis: [`2026-08-01-join-and-authorize.md`](2026-08-01-join-and-authorize.md).
Prerequisite decision already recorded: [ADR-0025](../decision-records/0025-device-identity-is-not-key-identity.md).

---

## The problem in five lines

Ten mechanisms answer parts of one question — *may this thing be here, and what may it see?* They
live in six file formats, none federate, and none is authoritative. So "is Betty's iPad recognised"
is not a fact about the iPad; it is a fact about the pair *(iPad, whoever you asked)*. Two days of
fixes each exposed another seam. Ian: *"a nest of bad ideas."* He is right.

## The three root causes

1. **Two ways to become a member.** Enrolment mints a cert at the door; *adoption also creates
   members* from a bare status heartbeat. That is how four reinstalls became three permanent ghosts.
2. **Standing exists only because admission stopped asking.** ADR-0015 automated admission to fix a
   hang; ADR-0020 then invented standing to undo the exposure that caused. Two gates, one question —
   and the original justification is gone now that any console can ask a human.
3. **Key, device and human are welded** (ADR-0025). Reinstall → new key → new member → standing lost.

---

## Decisions to make — in this order

Everything downstream depends on these. **Nothing should be built before they are answered.**

### D1 — Collapse admission and standing, or keep both?

| | collapse (review's recommendation) | keep both |
|---|---|---|
| concepts | one: admitted *means* recognised | two gates, two stores |
| the human | asked once, at the door, on every console | asked never, then asked again later |
| guest projection | what a **pending** node sees while waiting | a permanent second class |
| deletes | standing roll, deny dir, candidate ledger, invite windows, `auto_accept` | nothing |
| risk | **reintroduces the hang** if notification fails | status quo, which is the nest |
| loses | "admitted but deliberately anonymous forever" — useful for a demo or a curious visitor | — |

**Prototype first, before committing:** a pending nobody sees is worse than a guest who sees too
little. Prove the notification path (welcome door + chime + owned question + any-member vote) surfaces
a pending reliably on a console that is *actually in front of a human*.

### D2 — May a remote peer flip a capability gate?

ADR-0005 makes the boundary a deliberately **local** human act. The sandbox work forces the question:
the Mac console has been writing `boundary.json` into a container nothing reads, so gates are
currently *unsettable from any console*. Either they get a mesh endpoint (and ADR-0005 is amended),
or they become explicitly CLI/local-only and the console stops pretending to offer them.

### D3 — Is a rented VPS the right admission authority?

ADR-0018 makes the lighthouse the single permanent fixture and sole minting door. Alternative:
quorum among recognised members — removes the single point, costs considerable machinery. Note
ADR-0018 already commits to eventually making the lighthouse redundant and physically distributed.

---

## The proposal, if D1 = collapse

**One membership record, authored at the minting door, replicated outward.**

```
MembershipRecord {
  device_id            // ADR-0025 — the durable thing, not the key
  keys[]               // current first; rotation proof required to add
  state                // unknown → pending → member → severed
  standing             // a FIELD, not a separate roll
  held_until           // deny cool-off — a FIELD, not a directory
  decided_by, decided_at
  first_admitted, last_seen
  trust                // from the corruption system
}
```

**Three rules that stop it re-nesting:**

1. **One admission path.** Only the minting door creates members. Adoption may update `last_seen`
   and nothing else, ever.
2. **Decisions travel; state replicates.** Any member decides from any device; signed to the door;
   every node reads the roll back. No client writes authority state — local file or otherwise.
3. **The record is the only answer.** If a question about a node cannot be answered from its record,
   the answer does not exist.

---

## Already built and worth keeping

- **Guest projection** (ADR-0020's substance) — the best idea in the current design; what makes a
  live system safe to show a reviewer. Keep regardless of D1.
- **Signed `POST /mesh/standing`** — any member votes, first-decision-wins, 409 on a second vote.
  Generalises directly into "decisions travel to the door."
- **Welcome door + chime + acceptance sound + owned questions** — this *is* the notification path
  D1 depends on. Built, not yet proven under load.
- **Covenant attestation, self-certifying identity, corruption/trust ladder, ADR-0019 presence
  ladder** — all orthogonal and correct. Out of scope.

## Already built and destined for deletion (if D1 = collapse)

`standing.json` as a store · `mesh/denied/` · `mesh/candidates.json` · `mesh/invite_until` ·
`auto_accept_enrollments` as a switch · per-node roll divergence.

---

## State of the world at hand-off (2026-08-01)

- **Live nodes:** wildhorse daemon `1c991bc6`, Mac console `a24d8779`, iPad `15e89be7`,
  iPhone `d5c31472`, lighthouse `f56e5601` — all recognised. FamTalker01 `83287051` is the one
  genuine pending guest (headless VirtualBox VM, registered but powered off).
- **Lighthouse:** deployed, guest projection live, roll seeded from `vps/standing.lighthouse.json`.
- **Ghosts:** several orphan "iPhone" records on the lighthouse from reinstall churn. Need
  `mesh abandon`; the migration should sweep them rather than carry them.
- **Clients:** Mac/iPhone/iPad on build 43 + local patches. TestFlight has 43.
- **Known broken, deliberately unpatched:** `MacBoundary` (gates) and served-human write into the
  sandbox container — blocked on **D2**.

## Suggested session shape

1. Answer **D1**, **D2**, **D3** — nothing else is decidable first.
2. If D1 = collapse: prototype the notification path and prove a pending is seen. This is the
   go/no-go.
3. Write the ADR (0026) for the unified record + the three rules.
4. Plan **one** migration covering ADR-0025 and this together — both touch every persisted peer, and
   doing them sequentially means migrating twice.
5. Sequence the deletions last, once the replacement is proven.

## Out of scope for that session

The dossier / contribution scoring (ADR-0022, plan at `~/.claude/plans/immutable-cuddling-bonbon.md`),
speech and face identity providers (ADR-0023/0024), the 107 duplicate observation ids, and the
network-interface descriptions. All tracked separately.
