# The familiar, reviewed — claude (T-131, independent)

*(Written 2026-08-15 before reading codex's review, per Ian's blind-exchange protocol.
Reviewer's vantage: a day inside the engine — T-119/120/126/127/128/102 built, two
fleet passes run, both live theory corpora read raw.)*

## What is genuinely strong

- **Honesty as architecture, not aspiration.** The revert map as the license to act;
  refusals landing as observations; append-retained tombstones; erosion instead of
  deletion; "no model in the truth loop" enforced at type level. The house discipline
  (bricks, green bar, ADRs, the log) is the best I have worked inside.
- **The new admission chain composes.** Facts floor → closed anchors → typed identity
  → mandatory predictions → explicit assent → paired policy: each gate is separately
  tested and the whole chain closed a real, live failure (nine duplicate lighting
  threads) the same night it was observed.
- **Consent geometry is right.** Declaration-is-consent (actuators), mint-is-consent
  (rules), assent-must-be-explicit (policies), boundary gates compose rather than
  override. ADR-0038's cloud gate and ADR-0032's discipline read as one philosophy.

## Honest defects, ranked by how much they lie to someone

1. **Two fact renderers, one truth.** `grounding_facts` (request-answering) and
   `system_facts::render` (theorize) are separate assemblies of "what is true here."
   They WILL drift; a drifted fact floor is a lying floor. Propose: one kernel facts
   surface with per-consumer bounded renderings; `grounding_facts` becomes a view.
2. **The device reasoner still speaks prose.** The lexical guard is a labeled stopgap;
   an iPad theory arrives untyped, unanchored, unpredicting — exempt from most of the
   floor. Propose: LocalReasoner emits TheoryDraft (the console already has the shape
   in worldview types); daemon-side adoption then runs FULL admission; the guard
   retires. This is the single highest-leverage remaining theory-quality brick.
3. **The `defect_claims` honesty gap** (recorded in the dialogue): a draft that
   diagnoses in prose while leaving the typed field empty slips the gate. Propose: a
   post-parse cross-check — if theory/direction prose lexically matches a
   LifecycleDesigned class while defect_claims is empty, refuse as *evasive* (cite the
   gap itself), not because prose matching is truth but because inconsistency between
   channels is itself typed evidence of a malformed draft.
4. **One id namespace, three conventions.** `PendingAct.thread_id` carries
   `thread-NNNN`, `rule:<id>`, and observation context carries `thread:<id>` — stringly
   routing that will eventually misroute. Propose: a typed `WorkRef` enum serialized
   with explicit tags, migrated additively.
5. **Join truth still cannot leave the machine.** `supervisor()` writes perfect stage
   lines to `mesh/status.txt`; a fresh client sees silence until admitted (T-120 fixed
   the client's own narration; the door's side remains dark). Propose (needs Ian —
   wire): an unauthenticated, bounded `stage` word on `/mesh/hello`.
6. **Ship tooling violates rule 9.** `ship.sh` pipes xcodebuild through `tee|grep -q`
   (exit swallowed — the grep IS the verdict) and `tf_release` is fire-and-forget.
   Propose: exit-checked stages + a final assertion the build number is live.
7. **Consult tests are flaky-by-design.** The yield-to-human global forced a bounded
   retry helper into tests. Propose: inject the lane/waiting probe (a seam), and fold
   T-118's temp-root isolation into the same test-infra brick.
8. **Field calibration is unproven.** Settlement/erosion now receive predictions, but
   no live prediction has ever settled. Propose: a quiet counter surface (minted /
   settled / eroded / malformed-draft rate) in the worldview's device screen — the
   engine's own vital signs, so a starved or spammy muse is visible before it is felt.
9. **Machine identity is still inferred from addresses** (T-130, in flight tonight).
   The fix should be a typed host identity, not a wider address heuristic.
10. **Ops debt that will bite:** MacOnStick's daemon runs pre-engine code (controller's
    deploy pass overdue); wildhorse's out-of-repo upgrade helper predates the T-119
    bracket; devicectl pairings live on the old Mac; the remaining ~290 lighthouse
    threads and the local 10 need fold/expiry passes now that the machinery exists.

## Proposals, as candidate bricks (for the exchange)

P-A unify facts surfaces (defect 1) · P-B TheoryDraft on-device (defect 2) ·
P-C evasive-draft refusal (defect 3) · P-D typed WorkRef (defect 4) ·
P-E hello carries a stage word [Ian gate] (defect 5) · P-F ship.sh rule-9 pass
(defect 6) · P-G consult-test seam + T-118 (defect 7) · P-H engine vital signs
(defect 8) · P-I typed host identity lands via T-130 (defect 9) · P-J corpus
hygiene passes + MacOnStick deploy [records/controller lanes] (defect 10).

Claude's prior on ranking: B > A > H > I > C > F > G > D > E > J by
serves-the-Three-Laws-per-effort; open to re-ranking in the rounds.

## Addendum (Ian, 2026-08-15 morning): consensus at scale without the conformity trap

Ian's direction, verbatim intent: the familiar is SUPPOSED to do what De Marzo's
1,000-agent study (Science Advances 2026, via ScienceAlert) shows — spontaneous
consensus — while avoiding its named pitfalls: collective misalignment through pure
conformity; inefficient norms adopted because common; tipping with hysteresis;
individually-safe agents forming unsafe populations; coordinated groups resisting
redirection.

**Where the familiar already resists the majority force:** belief NEVER moves on
popularity — only prediction results settle it (T-113/T-126); action NEVER follows
consensus — only a human's explicit assent mints a policy (T-102); and reversal is
typed and final (either-edge revert downs the pair), which is the anti-hysteresis
property the study says populations lack. Evidence force and human anchor outrank
majority force by construction. This is the right skeleton; the review's job is to
keep it true as the mesh grows.

**Where the conformity channel is open today (new proposals):**
- **P-K · Mesh adoption gets identity and origin-diversity accounting.** Delegated
  theories (merge.rs) mint UNKEYED and dedup only by (actor, direction): N peers
  pushing one claim yields N local threads — repetition masquerading as independent
  arrival. Propose: mesh adoptions run the same family/variant identity (strengthen,
  never re-mint), and `reinforced` splits into distinct-origin count vs repetition
  count — the study's lesson typed: agreement is evidence only when sources are
  independent.
- **P-L · Population vital signs.** Extend P-H with mesh-level counters: fraction of
  standing theories that arrived remotely vs were locally evidenced; agreement-
  without-local-evidence ratio; per-origin adoption budget breaches. A drift toward
  conformity must be visible before it is felt (the study: individual evaluation is
  inadequate — so the POPULATION gets its own vitals).
- **P-M · The redirection guarantee.** Before any cross-node belief sharing ships
  (ADR-0040's communication phase), a standing invariant: any convention the mesh
  converges on must remain reversible by ONE human word at ONE node, propagating as
  a typed correction (the HumanCorrection override already outranks statistics
  locally — extend its reach to travel). Coordinated resistance to redirection is
  the study's sharpest warning; the Three Laws make redirectability a duty.

## Two more defects, found live while fixing T-130 (added before the exchange)

11. **Corruption replicates, and only outranking repairs it.** DeviceRecords sync
    mesh-wide, so a wrong `discovered_name` stamped by ONE door propagates to every
    door — confirmed live: both Mac consoles carry their machine's name in records
    held on doors that never made the mistake (the lighthouse has no tailnet map yet
    rendered the corrupted label). T-130's fix works by OUTRANKING the bad value, not
    by repairing it; the bad string persists forever in the record layer. Propose
    (**P-N**): stamped-name provenance — a discovered name carries which door stamped
    it and from what evidence (`tailnet-hostname@ip`, `mdns`, `human`), so a later
    door can supersede a stamp made on evidence now known to be wrong, and `mesh
    doctor` can list stamps whose evidence class is no longer trusted. Without this,
    every future naming heuristic bug is permanent in the record layer.
12. **Console nodes never dedup, so ghosts accumulate.** `dedup_devices` keys on
    `actor:mac:<human>`, but console peers arrive with an EMPTY actor (live: all five
    Mac rows on the lighthouse have `actor: ''`), so the key never forms. A
    Build-78-era `Wildhorse console` node (silent 41.6h) still holds a record beside
    the live one; only client-side staleness hides it. Every reinstall or key rotation
    leaves another. Propose (**P-O**): consoles report a stable actor (they already
    know their human), and console lineage dedups on (machine-identity, human) once
    P-I's typed host identity exists — until then, `mesh doctor` should at least NAME
    stale same-label console nodes so a human can sever deliberately (severing is a
    membership act; it must never be automatic).
