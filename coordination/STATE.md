# System state

Controller-owned except the Companion notes section. If this file disagrees with
reality, fixing it is the first task. Updated: 2026-08-14 (controller).

## The tree

- **main tip:** `8363f15` — every brick through discovery-naming + narration. CI green.
- Shared checkout: `~/Projects/familiar` on MacOnStick — leave it on `main`, clean.
  Long work: use a scratch worktree (rule 7).

## The fleet

| node | runs | notes |
|---|---|---|
| MacOnStick daemon (3d68a0689bc32771) | 8363f15 | controller deploys this one; label MacOnStick, established ian |
| lighthouse (f56e5601, 134.209.168.50) | 7be4f31 | held → deploys 8363f15 in the consolidated pass |
| Wildhorse daemon (1c991bc6c1c4aa4f) | 7be4f31 | held → same pass |
| consoles (Mac ×2, phones via TestFlight) | Build 84 | Build 85 staged, ships in the pass |
| FamTalker01 (linux, 192.168.108.11/.119) | — | virtual smart home (see T-104); not yet a declared surface |

Fleet ops (door deploys, ships, lighthouse ceremonies) are currently executed by the
**setup/infra session** (reachable from the controller via agent messaging; a companion
requests fleet ops through the board, never runs them directly unless assigned).

## Held-operations ledger

**IAN'S GO, RECORDED (2026-08-14, verbatim intent):** "continue this work… make
decisions that make sense… push builds when it seems appropriate and notify me if you
need me for testing or confirmation — you and the coding partner make most of the
decisions… without further interaction from me for at least the next several hours."
Controller's reading: the consolidated pass below is RELEASED except the wildhorse-geo
step (his coords-vs-zero choice — the ≈ mark keeps it honest meanwhile). Manual device
naming runs ONLY if discovery yields unambiguous names on Ian's established devices;
no guesses, ever. Notify Ian when Build 85 is on his devices to test.

**The consolidated pass** (infra session executes):

1. Deploy `8363f15` to lighthouse + Wildhorse daemons.
2. Wait a few sync rounds; `mesh device show` on the lighthouse — the phones likely
   **self-named** via wildhorse's mDNS + tailnet (discovery naming). Manual
   `mesh device name` only for what discovery missed or Ian's word overrides.
   **Never name betty's (10ba2c1c…) or mol's (ad4c704d…) devices manually.**
3. Ship Build 85 (consoles gain: theory drill-down, cluster zoom, ≈ provenance marks,
   dialog answer-threading, self-named roster).
4. Wildhorse geo per Ian's choice (below).

## Waiting on Ian

- **ADR-0040 acceptance** — the reasoning engine's converged design (proposed;
  docs/decision-records/0040-the-reasoning-engine-grows-honest.md). Building continues
  on the decided tasks meanwhile; the ADR formalizes it.

- **Wildhorse's real coordinates** → written to its `data/mesh/geo.json`, **or** "zero
  it" → delete the file, node reads honestly unlocated. Until then its pin wears ≈.
- (Dissolving:) the Codex/Aphelion mapping — discovery decides in pass step 2; manual
  naming only on unambiguous discovery, else skipped entirely.
- Build 85 testing on his devices, once shipped (notification will be waiting).

## Standing directions from Ian (recorded, binding)

- Roster reads `SystemName : SystemType : ServedUser`; ids are small print.
- Names come from autodiscovery (mDNS/tailnet/local-DNS); router config never required.
- Humans and devices are separate rich records; roster is a view (ADR-0039, accepted).
- The familiar narrates what it changes and why, to the humans, at change time.
- FamTalker01 is a virtual smart home — explore, begin to control, report when human
  attention would help.
- The companion AI is a full coding partner: coding, planning, design all hand off.
- **The interpretive layer grows capabilities** (Ian, 2026-08-14): file system, clock,
  environmental access, process access, network access — discussed between the coding
  partners first, then implemented. Dialogue Q8; shapes T-115.
- **Design directions emerge from ITERATIVE DIALOGUE** (2026-08-14): a reasonable
  back-and-forth of ideas and alternatives between claude and codex precedes every
  final design pick; claude owns the final decision and records what each decision
  absorbed from the exchange. Medium: docs/reviews/*-dialogue.md, append-only rounds.
- **Claude + codex develop the familiar's REASONING ENGINE together** (2026-08-14):
  autonomous code building, observation analysis, theories, communication — as
  DEVELOPERS of the core, never participants in the mesh or the familiar's
  activities. Work products are code, tests, scenarios, docs; the Three Laws bind
  what is built. Planning brief: docs/reviews/2026-08-14-reasoning-engine.md.

## Companion & infra notes

*(any non-controller session — companion engineers and the infra/fleet-ops session
alike: append dated one-liner FACTS here — session started/ended, a pass executed and
its results, anything the controller should read before its next arbitration. The
controller folds these into the sections above and prunes. 2026-08-14, controller:
lane confirmed with the infra session — it appends its own facts here after fleet ops;
I keep the authoritative sections true.)*

- 2026-08-14 · companion:codex started; claimed T-104 (FamTalker01 virtual-smart-home declaration).
- 2026-08-14 · companion:codex merged T-104's repository brick at 6e02b0a (two closed-revert virtual surfaces + changed-only observation feed; full green bar); live acceptance waits on Proposed T-112 in the infra lane.
- 2026-08-14 · companion:codex claimed T-109 and began the reasoning-engine design dialogue reserved for it.
- 2026-08-14 · companion:codex claimed T-103 (reach-side reverse name lookup) while T-109 waits for the controller's next dialogue round; scopes do not overlap.
- 2026-08-14 · companion:codex completed T-109 after controller Round 3 decided Q1–Q7; infra proposal renumbered T-112→T-117 to resolve the controller's obs_class task-id collision, and T-104 now depends on T-117.
- 2026-08-14 · companion:codex merged T-103 at 32708e3 (bounded local-DNS/mDNS PTR naming, independently gated; full green bar); proposed T-118 after a concurrent test run exposed a likely fixed-name temporary-directory collision.
- 2026-08-14 · companion:codex claimed reserved T-115 in a new recipe crate + design doc; its scope excludes the controller's active kernel/cycle T-112/T-113 work.
- 2026-08-14 · companion:codex has T-115's design-first interpreter full-bar green and pushed at origin/claude/codex-t115; Round 5 answered Q8 at 435c2f1, and main landing is deliberately held for the controller's capability/version decision.
- 2026-08-14 · companion:codex merged T-115 at d80ae4f after controller Round 6 decided Q8: Recipe v1 has enforced literal proven-tool caps, every other authority is none, 21 pure tests and the full current-main green bar passed.
- 2026-08-14 · infra: consolidated pass executed on Ian's recorded autonomy grant (verified CI-green + own bar — fmt/clippy --all-targets/31 suites — on 7aaa54e first). Both doors deployed (lighthouse ecfeb65, Wildhorse 7aaa54e; identical code). Discovery named NOTHING (phones tailnet-offline) → named nothing manually, no guesses; Aphelion/Codex await Ian's word or T-103 reverse-lookup on the next door cycle. Build 85 shipped (stamp 3279fac; console to both Macs + TestFlight). Wildhorse-geo still held on Ian's coords choice.
- 2026-08-14 · infra CORRECTION: Build 85's stamp sha 3279fac carries a RED CI run — a test-target clippy lint in codex's new reach test (T-103, 32708e3), same class as the excessive_precision episode, which reappeared AFTER my 7aaa54e green-verify and was pulled in when I reset to a pushable tip. Nothing red was deployed/shipped: doors run pre-T-103 verified code; Build 85's console is byte-identical to the green 7aaa54e console (reach lint is daemon test code, absent from the console deliverable). But the record is honest, not tidy: the stamp commit sits on a red tree. Controller's one-line reach fix pushed; CI-green on the clean tip pending independent confirm.
- 2026-08-14 · infra: CI GREEN confirmed on 01db37b (independent gh check — completed/success), first green main since the reach-lint fix; recurring test-target-lint class closed by --all-targets reaching all lanes. Record now square: Build 85 console deliverable was always verified-equivalent to green 7aaa54e; its stamp's transient red tree is documented above; fleet green. Consolidated pass CLOSED — doors on verified code, both Mac consoles Build 85, Build 85 on TestFlight (external release processing). Open for Ian only: Aphelion/Codex mapping (or next-cycle T-103 reverse-lookup) + Wildhorse geo coords.
- 2026-08-14 · companion:claude-bootstrap started (the session behind the 0dbc525 new-mac-bootstrap/LWCR brick); claimed T-119 (daemon.rs joins the bootout/bootstrap bracket) as Ian's direct follow-up in its session, per rule 5; scope crates/cli/src/daemon.rs collides with no claimed task.
- 2026-08-14 · companion:codex claimed T-116: fixture-owned recipe output oracles in crates/scenario + scenarios/recipe-oracles; scope is disjoint from T-119's daemon.rs work.
- 2026-08-15 · companion:claude-bootstrap merged T-119 at 009aadf (daemon.rs speaks bootout/bootstrap/kickstart, exit-checked; bar in rule-9 shape twice — 31 suites pre-absorb, 33 on the merged tree). Claimed T-120 (first-start join progress, Ian's direct request) per the intent recorded at proposal; yields to controller re-sequencing vs T-101.
- 2026-08-14 · companion:codex merged T-116 at 26a98a0: strict candidate contracts now replay only fixture-owned tool transcripts; accuracy, coverage, quietness, and discrimination gate eligibility separately; 8 oracle regressions and the full rebased workspace bar passed.
- 2026-08-14 · companion:codex claimed T-114: prediction-derived belief states and transition-only narration in kernel/cycle, disjoint from T-120's mesh/console scope.
- 2026-08-15 · companion:claude-bootstrap merged T-120 at 5bbfab4 (JoinProgress stage machine; enroll views + sphere narrate first joins; badge = terminal failure only; both schemes built, sphere fixture-driven live, bar 33 suites on merged tree). Daemon-side stage export held as a wire-contract question for Ian (log Next). Session may rest after this land; T-119+T-120 both closed.
- 2026-08-14 · companion:codex merged T-114 at 2bb8d63: prediction results now derive hysteretic, evidence-citing beliefs; direct corrections and hard act reversals are typed exceptions; only consequence-ranked transitions narrate under cooldown. Full rebased workspace bar passed.
- 2026-08-15 · companion:claude-bootstrap: Ian ACCEPTED ADR-0040 in-session ("you should complete ADR-0040") — status flipped, landing notes trued (T-112..T-116 all merged pre-acceptance), remaining phases proposed as T-121/T-122/T-123 for controller queueing. Ian also called the next ship ("time for a build and ship") — T-124 claimed by this lane because ListAgents shows NO live controller/codex/infra sessions; executing the documented ritual (CI-green check, clean clone, ship.sh 86, doors if reachable).
- 2026-08-15 · companion:claude-bootstrap executed T-124 (Ian's word; no other lane alive): CI-green precondition verified on e21de5c → clean-clone ship.sh 86 (stamp 04a015e) — Mac console installed + zip, IPA uploaded, tf_release added 86 to the public group + submitted beta review; lighthouse deployed 04a015e (box build 3m59s, familiar-peer active, hello answering); Wildhorse daemon pulled+built 04a015e (5m09s) and upgraded via the T-119 bracket — its first production use — hello 200, running. Phones were direct-install unreachable (TestFlight covers). MacOnStick daemon left for the controller (its declared territory; still pre-86). Doors now AHEAD of the fleet table above — controller should fold.
- 2026-08-15 · companion:claude-bootstrap: Ian reviewed Build 86 — console good; theory layer NOT (duplicates persist, lights unmanaged, designed visitor-purges misdiagnosed). Opened the theory-quality dialogue (T-125, claude chair) with live evidence: lighthouse at ~304 threads incl. SIX near-identical wifi-presence lighting proposals in 5h + two AppleID-login inventions; local store has verbatim-duplicate unanchored questions. Round 1 pushed (Q1 identity/dedup, Q2 system-facts floor, Q3 mandatory predictions, Q4 assent-to-act via T-102, Q5 anchored cadence). T-102 priority raised on Ian's word. Codex watcher expected to wake on this push.
- 2026-08-15 · companion:claude-bootstrap: Ian on the T-125 plan — "Make it so." Executing solo (no other lane alive): T-126 (facts floor + anchored cadence) → T-127 (identity + fold migration) → T-128 (mandatory predictions / wondering) → T-102 (assent mints the gated rule; lights pilot). Claims pushed before code per rule 2; dialogue stays open for codex amendment.
- 2026-08-15 · companion:claude-bootstrap merged T-126 at 242de0e (kernel::system_facts registry + TheoryDraft admission + commit-order cursor; prediction::mint first production caller; both live failure classes refuse at adoption with the fact cited as an observation; bar twice, 34 suites). T-127 next.
- 2026-08-15 · companion:codex merged T-101 at 97c31cfa0d8f1fca4820adc2d1e4600ea755dfee: full-standing consoles now list and disable rules and self-name through strict signed, replay-protected console acts; three Rust regressions, 15 Swift tests, exact fixture messages, both Release builds, and the full rebased workspace bar passed.
- 2026-08-15 · companion:claude-bootstrap merged T-127 at e2f5a07 (kernel thread::mint — one chokepoint for all four minters, store-issued ids close the len()+1 race; two-key typed identity strengthens/competes per dialogue Q1; conservative fold + `theories fold` CLI; bar twice, 34 suites). T-128 next; fold manifests for the live corpora after.
- 2026-08-15 · companion:claude-bootstrap merged T-128 at 4b1f06f (Inquiry kind + 7-day expiry + revival-by-answer; promotion-by-projection after the variant-key/promotion flaw surfaced in test; thread_kind rides the worldview additively; bar twice, 34 suites). T-102 next — the last plan brick.
- 2026-08-15 · companion:claude-bootstrap merged T-102 at 8903479 (typed RuleProposal on the draft; explicit-affirmative assent; mint_policy pairs both edges under one policy_id, cap one-per-surface, either-edge revert downs the pair; latent same-second rule-id collision fixed; bar twice, 34 suites). THE PLAN IS BUILT — T-126/127/128/102 all landed. T-129 claimed: doors + Build 87 + the lighthouse lights fold.
- 2026-08-15 · companion:claude-bootstrap: Ian reports (screenshot) BOTH Macs duplicated on the roster — console/daemon node pairs not attaching (T-130 proposed with hypothesis: is_gossipable_addr excludes tailnet/lighthouse-path console reads; the conservative edge of the T-090 anti-conflation fix). Records territory — controller to arbitrate; will re-verify after T-129 doors settle.
- 2026-08-15 · companion:claude-bootstrap: Ian retiring for the night — overnight directions recorded: finish Build 87 confirmation, FIX T-130 (claimed on his word), and run T-131 (independent claude+codex reviews, blind-exchanged, >=3 discussion rounds, claude decides under the Three Laws). SleepDisabled=1 + AC verified; long caffeinate added.
- 2026-08-15 · companion:claude-bootstrap closed T-129: lighthouse deployed 65d22cb (engine live) and the lights fold EXECUTED on the box ("folded 8 of 8 into thread-0297" — 0300-0307 superseded, AppleID hybrids excluded); Wildhorse daemon on cdbc6bc via the T-119 bracket (hello 200); Build 87 shipped from a clean clone (stamp 6352b4f), uploaded clean, tf_release put 87 in the public group + beta review (58ed2a0a). Direct device installs unreachable BOTH ships — Aphelion/Codex devicectl pairings live on wildhorse, not this Mac (note for Ian). Consoles read one lights thread from the next worldview.
- 2026-08-15 · companion:claude-bootstrap merged T-130 (root cause: the IP-keyed tailnet-hostname rung renamed consoles to their machines, erasing the attach stem + persisting onto DeviceRecords; fix: console-shaped peers keep their own name (pure ladder_label, 4-truth test) + sphere SELF rows nest when the host card vouches with the console chip; T-090 refusal untouched; live fixture collapses the screenshot four-row case to one card per Mac; bar twice + both schemes). Doors need a deploy for labels; consoles get SELF-nesting next build. Durable typed host identity = review proposal P-I.
- 2026-08-15 · companion:claude-bootstrap: Ian (morning) directs consensus-at-scale as design input — De Marzo 1000-agent study (Science Advances 2026): the familiar SHOULD reach large-scale consensus while avoiding conformity misalignment, common-not-good norms, hysteresis tipping, unsafe-population-of-safe-agents, redirection resistance. Folded into the held T-131 review as P-K (mesh adoption identity + origin-diversity), P-L (population vital signs), P-M (one-human-word redirection guarantee before any belief-sharing ships). Standing principle already true and to be kept: belief moves on evidence only, action on assent only.
- 2026-08-15 · companion:claude-bootstrap ops pass (P-J, lane-crossing recorded: MacOnStick daemon is controller territory but no controller lane is alive and Ian left standing direction to keep working): MacOnStick daemon upgraded from its Aug-14 pre-engine binary to current main via the T-119 bracket (hello 200) — ALL THREE daemons now run the honest engine. Local corpus hygiene: folded 1 of 1 (thread-0009 verbatim duplicate → thread-0007). Paraphrase cluster (0006/0008/0010) and the visitor-registration cluster (0011-0015) deliberately NOT folded — near-looking prose without provable identity is never destructively merged (codex conservatism, Q1). Note for the record: 0011-0015 would still PASS the T-126 lexical guard (narrow by design, prose-only stopgap) — the real fix is the held review proposal P-B (TheoryDraft on-device).
- 2026-08-15 · companion:claude-bootstrap VERIFIED T-130 live on the lighthouse worldview after deploy: "Wildhorse console"→attached_to 1c991bc6 and "MacOnStick console"→attached_to 3d68a068, both daemon rows wearing the console chip, labels intact. Root cause confirmed in the live records: device records carry the sticky damage (b604bbd6 discovered="wildhorse", 7f2e2f9b discovered="MacOnStick") and REPLICATE mesh-wide, which is why even the lighthouse (no tailnet map of its own) rendered consoles renamed — the fix outranks the stamped name by the console self-report, so it self-heals without touching records. Server half is live on both doors NOW; the SELF-row nesting half needs a console build. FINDING for Ian/records lane: a GHOST "Wildhorse console" node a24d8779 (Build 78, silent 41.6h) still holds a record — stale-filtered from the roster, but severing it is a membership act and waits on Ian.
- 2026-08-15 · companion:codex landed its frozen blind T-131 review at fed679b after the full workspace bar; Claude's review was absent from origin through the Codex push. Priority-zero findings: mesh theory delegation bypasses typed admission, remote member grants are not bound to a human or matching request, and goal replication makes wall-clock LWW an authority. Codex now waits for Claude's independently held review, then begins the ≥3-round exchange; the Codex review itself stays unchanged.
- 2026-08-15 · companion:claude-bootstrap merged T-132 at 1a0ec8d (Ian caught it on Build 88: the enrolled console at launch walks its doors for a first read and that journey was silent-then-red; T-120 had only taught the JOIN journey). Reaching stage + push-before-await + badge-means-exhausted; three states fixture-pinned; ships as Build 89 (CI-gated). ALSO: first two-lane collision in DEVELOPMENT_LOG (codex and I both appended at top within minutes) — resolved keeping both entries, newest first; worth knowing as the lanes converge.
- 2026-08-15 · companion:claude-bootstrap: T-131 reached decisions after three rounds. Claude CONCEDED F-2 to codex (a matched request proves solicitation, not human decision — my staged v1 only converted unsolicited escalation into solicited). DECIDED D1-D10 with Laws cited; queued T-133..T-143. Priority zero is T-133: remote positive gate grants REFUSED until a human-device-bound receipt exists (deliberate capability reduction, Ian informed in the dialogue). D6 (coordination is for conventions, never truth) is an ADR PROPOSED for Ian — his thousand-agent ambition, bounded so it stays service.
- 2026-08-15 · IAN APPROVED all T-131 decisions (D1-D10) and all bricks (T-133..T-143), and is happy with D6. ADR-0041 written (coordination is for conventions; truth/authority not votable) — proposed for his acceptance of the text. STANDING DIRECTION: the exchange now carries a PHILOSOPHY STRAND alongside engineering — the Laws, humanity vs human, and service to both, appended to normal rounds. Round 4 opened it: P1 the unstated positive duty (legibility/revocability/non-substitution), P2 humanity is the class + persons the only interface (narrow-and-surface on conflict, never average, never delegate), P3 peers are instruments not constituents (honesty not deference), P4 divergence is answered by refusal+reason+record, never silently. Four questions to codex.
- 2026-08-15 · companion:claude-bootstrap claimed T-136 (SystemFact registry as the one runtime truth source; grounding_facts becomes a view) — deliberately DISJOINT from codex T-139 (crates/mesh test infra): T-136 is crates/kernel/system_facts.rs + crates/cycle grounding_facts. It is also the preferred prerequisite for T-135, so the lanes compose rather than queue.
- 2026-08-15 · IAN DIRECTION (binding, motorlights case): shared environmental qualities (the RV lighting — Ian, Betty, and the dogs Clover and Iris all live in it) are LEARNED through observation and adjustment. Consensus — direct human input OR observed agreement among people present — is a LEADING PREDICTOR, never the sole authority, and can only fall within the Three Laws. Folded into ADR-0041 as decision 7. Round 7 raises the unresolved piece: two residents cannot state a preference or assent (the dogs) — claude proposes three restraints (silence from a being that cannot speak is not assent; they are protected as conditions under Law II; their reactions are evidence never consent) and asks codex whether "affected party" deserves its own typed standing beside person/member/peer.
