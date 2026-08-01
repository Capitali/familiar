# Architectural review — mesh join and authorize

**Asked for by Ian, 2026-08-01**, after two days in which every fix exposed another seam. He is
right that it has become a nest, and a fair share of the nest is recent and mine. This is a review,
not a defence.

---

## 1. What we have

Nine mechanisms currently answer some part of one question — *may this thing be here, and what may
it see?*

| mechanism | where | what it answers |
|---|---|---|
| membership cert | `mesh/granted/`, minted by `group.rs` | is it in the group at all |
| pending record | `mesh/pending/` | is it waiting for a human |
| invite window | `mesh/invite_until` | should we admit anyone right now |
| `auto_accept_enrollments` | `config.json` | should we admit *everyone* |
| deny record | `mesh/denied/` | is it in a retry cool-off |
| standing roll | `standing.json` | may it see names |
| peer record | `mesh/peers.json` | do we consider it a member locally |
| candidate ledger | `mesh/candidates.json` | has it persisted enough to adopt |
| corruption/trust | `refusals.jsonl` | is it behaving |
| abandoned flag | on the peer record | did a human retire it |

Ten, counting the flag. Six file formats. **None of them federate**, so every node holds its own
opinion, and the answer to "is Betty's iPad recognised?" is not a fact about the iPad — it is a fact
about the *pair* (iPad, whoever you asked).

## 2. What is actually wrong

Three root causes. Everything else is a symptom.

### 2a. There are two ways to become a member

Enrolment mints a membership certificate at the minting door. That is the designed path.

But **adoption also creates members** — `apply_status_freshness` sees an unknown node in the status
directory and writes a peer record. No certificate is checked. No door is consulted. That is a
second, informal admission path that bypasses enrolment entirely, and it is how four reinstalls
became three permanent ghost "iPhone" members.

I added ripening (a node must persist 10 minutes) which makes ghosts *rarer*. That is a mitigation,
not a fix. **The fix is that adoption must not be able to create a member at all.** A status
heartbeat is *evidence about* a member; it can never be *proof of* one.

### 2b. Standing exists only because admission stopped asking

This is the one worth sitting with.

- ADR-0009/0012 made admission a human act. It hung, because the pending landed on a headless VPS
  nobody looks at.
- ADR-0015 made admission automatic to fix the hang — explicitly reasoning that the human moves
  from the *door* to the *review loop*.
- ADR-0020 then had to invent **standing**, because automatic admission gave strangers the
  household's names, and the corruption system cannot catch a member whose only act is to read.

So standing is a second gate, bolted on to compensate for removing the first one. Two concepts,
two stores, two decisions, one question.

**And the original justification is gone.** The reason admission could not ask a human was that
there was no way to ask. We have since built exactly that: a welcome door on every console, an
owned question routed to whoever is present, a chime when someone arrives, and a signed
`/mesh/standing` vote any member can cast from any device. The hang ADR-0015 was written to fix is
now fixable directly.

### 2c. The layers are welded (ADR-0025)

`node_id` is a keypair fingerprint used as a device identity, with the human welded into the actor
string. Reinstall → new key → new member → standing lost. Already recorded; unchanged by this
review, and a prerequisite for it.

### Contributing: clients were allowed to write authority state

The Mac console wrote `standing.json`, `boundary.json` and `observer.txt` directly. Under App
Sandbox those writes landed in a container nothing reads, silently, for days. The sandbox exposed
it, but it was always wrong: **a peer must not write another node's authority state.** iOS could
never have done it at all, which is why the same button worked on one platform and not the other.

---

## 3. Proposal

### One record, one door, one lifecycle

Replace the ten mechanisms with **a single membership record**, authored at the minting door
(ADR-0018) and replicated outward. Every node caches it; no node invents it.

```
MembershipRecord {
  device_id            // ADR-0025: the durable thing, not the key
  keys[]               // current first; rotation proof required to add
  state                // see below
  standing             // guest | recognised          ← a FIELD, not a separate roll
  held_until           // deny cool-off               ← a FIELD, not a directory
  decided_by, decided_at
  first_admitted, last_seen
  trust                // from the corruption system
}
```

**One state machine**, replacing the intersection of eight stores:

```
unknown ──asks──▶ pending ──a member decides──▶ member ──▶ severed
                     │                            │
                     └────── denied (held) ───────┘
```

Orthogonal, and *attributes rather than states*: `stale` (not seen lately), `abandoned` (a human
retired it), `trust` (behavioural).

### Then collapse admission and standing into one decision

If a human is asked at the door — which we can now do — **there is no need for a second gate**.
"Admitted" means "recognised". Standing stops being a concept and becomes the answer to the single
admission question.

Concretely: revert `auto_accept_enrollments` to false, keep the pending state, and surface pendings
through the machinery built this week (welcome door, chime, owned question, any-member vote). The
guest *projection* stays — it is genuinely useful and is what makes a safe demo, a reviewer view,
and a visitor view possible — but it becomes the view a **pending** node gets while it waits, not a
permanent second class.

**What this deletes:** the standing roll as a separate store, the deny directory, the candidate
ledger, invite windows, `auto_accept` as a switch, and the whole class of "recognised here but not
there" divergence.

### Three rules that keep it from re-nesting

1. **One admission path.** Only the minting door creates members. Adoption may update `last_seen`
   on an existing record and may do nothing else, ever.
2. **Decisions travel; state replicates.** Any member may decide, from any device; the decision is
   signed and goes to the door; every node reads the roll back. No client writes authority state,
   local file or otherwise.
3. **The record is the only answer.** If a question about a node cannot be answered by reading its
   record, the answer does not exist. No inferring membership from a heartbeat, a peer file, or a
   directory listing.

---

## 4. Honest costs

- **It is a migration**, on top of ADR-0025's. Both touch every persisted peer, and they should be
  done together, once, not sequentially.
- **Reverting auto-admit reintroduces the hang** if the notification path fails. That risk is real
  and is the thing to prototype first: a pending that nobody sees is worse than a guest who sees
  too little. The mitigation is that the pending is now surfaced on *every* console rather than
  only on the VPS nobody looks at.
- **Federating the roll makes the door a hard dependency for joining.** Existing members are
  unaffected; new joins fail while it is down. ADR-0018 already accepted that, and the escrow makes
  it an outage rather than an ending.
- **Losing standing-as-a-separate-concept costs one capability**: today you can admit someone and
  deliberately keep them anonymous forever. Under the proposal that is expressible only by leaving
  them pending, which is honest but changes the semantics of the welcome list from "decide
  eventually" to "decide".

## 5. What I would keep, unchanged

- The **guest projection** itself (ADR-0020's substance) — it is the best idea in the current
  design and the reason a reviewer can be shown a live system safely.
- The **covenant attestation** and self-certifying identity — never in question.
- The **corruption/trust ladder** — it is orthogonal, behavioural, and correctly separate.
- The **presence/identification ladder** (ADR-0019) — a different question with a different subject.

## 6. Open questions for Ian

1. **Collapse admission and standing, or keep both?** The review argues collapse; the counter-case
   is that "admitted but anonymous" is a genuinely useful state for a demo or a curious visitor.
2. **May a remote peer flip a capability gate?** ADR-0005 makes the boundary a deliberately *local*
   human act. The sandbox work forces the question, and it should be answered before gates get a
   mesh endpoint.
3. **Is the lighthouse the right authority for admission**, given it is a rented VPS and everything
   else is transient? The alternative is quorum among recognised members, which removes the single
   point but is much more machinery.
