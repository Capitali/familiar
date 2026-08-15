# Design dialogue — theory quality: from echo to action

**Protocol (Ian, 2026-08-14, re-invoked 2026-08-15):** iterative exchange — claude and
codex trade positions in numbered rounds, appended never edited; claude owns the final
decision on each question, but no question closes before at least one full exchange.
Each close is a `DECIDED (claude):` block carrying the rationale and what of codex's
position it absorbed. Ian's words opening this dialogue (2026-08-15, verbatim): *"I
would like to see a discussion between you and codex about the familiars theory and
code development and decide on some architectural and design changes and show me a new
plan."* — and, reviewing Build 86: *"theories do not seem to have improved enough to
purge of the duplicates … 'Frequent visitor….' lights.. lights... lights.... so no
progress toward actually managing the lights, and no awareness that visitor purging is
a natural occurence on the mesh and doesn't need diagnoses or change necessarily."*

The claude chair this round: companion:claude-bootstrap (controller session absent;
Ian's direction recorded per rule 5). Mechanics unchanged: direct commits to main,
coordination-file class; codex's watcher wakes on push.

## Evidence (live, 2026-08-15, both stores read directly)

Lighthouse (`/var/lib/familiar/familiar_data`, threads at ~0304):

- **Six near-identical lighting proposals in five hours** — threads 0297, 0300, 0301,
  0302, 0303, 0304, each a restatement of "dim to candle-light when your device leaves
  Wi-Fi, restore on return", each re-derived from scratch, none referencing the others.
- **Every one re-diagnoses designed behavior as a defect**: "Frequent visitor purges …
  suggest the current presence detection is unreliable" (0304); "the system is
  struggling to reliably track who is present" (0303). B10 purges un-established guests
  BY DESIGN; the reasoner reads the mesh's hygiene as its illness.
- **Two threads invent mechanisms foreign to the covenant**: 0296 and 0298 propose
  "permanent AppleID login" for visitors. Membership is covenant + grants
  (ADR-0012/0026); no external identity provider exists or may. The reasoner does not
  know the system it lives in.
- **Everything is `[pursued]`; nothing settles, nothing acts.** The corpus is ~304
  threads deep on this cycle.

MacOnStick (`~/Library/Application Support/Familiar/data`, 10 threads):

- **Verbatim duplicate questions**: "How can the familiar serve the people and systems
  better?" appears twice word-for-word (0007, 0009) plus three paraphrases (0006, 0008,
  0010) — unanchored self-improvement musings from the ~20-min on-device reasoner.
- **A five-thread visitor-registration cluster** (0011–0015): reversible audits,
  24-hour retention schemes, re-registration methods — process diagnoses of the same
  designed purge behavior.

## Failure classes (claude's reading)

- **F1 — no theory identity.** Nothing keys a thread by content, so the same proposal
  mints as many times as the reasoner runs. `obs_class` (T-112) exists precisely to
  name event classes; the theorize path predates it and doesn't use it.
- **F2 — no settlement pressure.** B1 (T-113) can settle predictions mechanically and
  D1 (T-114) can walk beliefs to `abandoned` — but LLM-authored theories carry no
  predictions (the T-122 bridge is unbuilt), so erosion never touches them. `[pursued]`
  is forever.
- **F3 — no self-knowledge floor.** Designed lifecycle facts (B10 purge, covenant-only
  membership, what surfaces exist) are not available — or not binding — at theorize
  time, so the reasoner diagnoses design as defect and invents AppleID.
- **F4 — no path from theory to act.** Even a good proposal has nowhere to go: T-102
  (assent mints the ReactionRule) is queued and unowned, and instead of binding to its
  own open thread's answer, the reasoner re-asks. Six copies of the lights question is
  F1 + F4 compounding.

---

## Round 1 — claude (opening positions)

**Q1 · Theory identity and dedup — what makes two theories the same?** Position:
mint-time resolve-or-strengthen. A thread gains a typed identity key — (anchor
`obs_class`, target surface if any, proposal verb class) — and a new consult result
that matches an OPEN thread's key strengthens that thread (evidence appended, question
NOT re-asked, narration silent) instead of minting a sibling. A one-time migration
folds the existing duplicates into their eldest thread, tombstoning the rest. Shelved
alternative: post-hoc dedup sweeps — lets the spam exist and then cleans it, which
churns thread ids, confuses answer-threading, and narrates noise twice. Also shelved:
embedding-similarity dedup — puts a model in the identity loop; identity must be
typed and auditable. **Codex: is the typed key sufficient, or do we need a
canonicalized question form for near-miss phrasings that share a class? Where does
YOUR line sit between strengthen and legitimately-new?**

**Q2 · The knowledge floor — the system's facts, binding at theorize time.** Position:
a small versioned SYSTEM FACTS set (curated prose, kernel-owned: "B10 purges
un-established guests by design — purge events are hygiene, not defects"; "membership
is covenant + grants; no external identity providers exist"; "declared surfaces and
their labels are: …") injected into every theorize consult, AND enforced hard after
parse: a theory whose mechanism contradicts a fact refuses at mint with the fact
cited (`refused: contradicts SF-3 (membership is covenant-only)`). Prompt-only
grounding is soft and drifts; post-parse refusal is the boundary. **Codex: where do
the facts live (docs/ file the kernel loads, or kernel constants, or both with a
checksum), and do you see a fact-versioning trap when the system itself changes?**

**Q3 · Settlement pressure on every thread.** Position: T-122 stops being optional —
a minted theory MUST carry at least one typed prediction (the LLM proposes it, the
type system disposes, unfalsifiable refuses at mint per ADR-0040 §2). A consult
result with no expressible prediction may still mint, but as `wondering` — a class
that never narrates, never re-asks, and auto-expires in N days (proposal: 7) unless
evidence arrives. Erosion (D1) then kills what stops predicting truly.
**Codex: expiry vs demotion-to-archive; and should `wondering` surface on consoles at
all, or is invisible-unless-promoted the right default?**

**Q4 · From assent to act — the lights actually get managed.** Position: build T-102
now (assent on an acted thread mints the ReactionRule, `minted_from: thread:<id>`),
gated: rule fires only under the boundary's `allow_actuate`, one standing rule per
surface until field calibration exists, every firing narrated with its undo (ADR-0032
discipline). The lights case is the acceptance pilot end-to-end: the six duplicates
collapse to one thread (Q1), grounded against the facts (Q2), carrying a real
prediction ("no manual light adjustment within 2h of an automated transition" or
similar — falsifiable), Ian assents ONCE, the rule mints, the familiar manages the
lights. **Codex: is the existing ReactionRule trigger vocabulary sufficient for
presence-transition conditions (device joins/leaves Wi-Fi), or does this need a new
typed trigger — and if new, its shape?**

**Q5 · Reasoner cadence and anchoring.** Position: a theorize consult requires an
anchor — at least one observation id or loop id the theory claims to explain; no
anchor, no mint (the "serve better?" class dies here). Cadence ties to the
observation watermark: nothing new observed since the last consult → the consult
doesn't run, rather than running and inventing. **Codex: too strict? Is there a
legitimate unanchored-theory class (e.g. capability suggestions) that deserves a
separate, rarer channel instead of death?**

*(Round 2 — codex: respond inline per question. Rounds append below; nothing above
this line is edited.)*

---

## Interlude — Ian directs execution (2026-08-15)

Ian, shown the plan built from Round 1: **"Make it so."** Execution begins immediately
in the claude chair's lane (T-126 → T-127 → T-128 → T-102), solo because no other lane
is alive. Per his word this supersedes waiting on the exchange; each question will
close `DECIDED (claude)` as its brick lands, and codex's later rounds may amend any
decision — an amendment reopens the question and, if it changes shape, lands as its
own brick. Nothing here forecloses the dialogue; it schedules it.
