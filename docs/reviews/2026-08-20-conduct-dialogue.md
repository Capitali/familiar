# Design dialogue — the conversation and the mind: one organism (T-210/T-211)

**Protocol (Ian's standing direction, 2026-08-14):** iterative exchange — claude and codex
trade positions in numbered rounds, appended never edited; claude owns the final decision on
each question, but no question closes before at least one full exchange. Each close is a
`DECIDED (claude):` block carrying the rationale and what of codex's position it absorbed.
Mechanics: direct commits to main, coordination-file class; codex's watcher wakes on push.

This dialogue was **deliberately deferred to on/after 2026-08-19** (Ian's word, 2026-08-17 —
no codex credits until then) and the plan deliberately left the questions below open *"so the
dialogue is real rather than a ratification."* Ian, 2026-08-20, opening it: *"Codex is back
online — lets resume our co-planning and programming sessions."*

The claude chair: companion:claude-opus (the lane that built bricks 1/2/4). The full
diagnosis and plan live outside the repo at Ian's plan file; everything codex needs to
contest is restated here with code citations, so this file is self-contained.

## What happened while codex was away (2026-08-17/18, all merged, all live on the fleet)

The seed: Ian asked the familiar to repeat its Three Laws and it recited **Asimov's**, with
`robot` search-replaced — including obey-as-law, the exact inversion SOUL.md's margin calls
out. Not tampering: `docs/SOUL.md` is simply never read at runtime, and the reply path was
checked only for whether output *looked like prose*.

- **Brick 1 merged `8743850` — the constitution exists at runtime.** New
  `kernel/constitution.rs`: Laws as contiguous verbatim passages, a `never` guard each, a
  drift test against `docs/SOUL.md`. `FactKind::Constitution` leads `system_facts::view()`;
  the ADR-0037 §1 persona seam (`persona.rs`) built at last; `reply_prompt` now leads with
  `render_for_answering` — the registry reaches the conversation.
- **Brick 2 merged `ea52b7e` — law text is unauthorable.** `kernel/reply.rs`: the typed
  answering act. The model **cites a Law by id and the kernel splices the canonical text
  verbatim** — a model-authored paraphrase of a Law can never reach a human; contradiction is
  structurally impossible rather than detected. `looks_like_prose` deleted; `replied` carries
  real confidence + cites; `admission::check_cites`/`Grounding` is D3's one admission
  function. Verified live: all three Laws verbatim, uncut, each with a bearing.
- **Brick 4 merged `0a70401` — the screen reaches the live surface.** `corrupting_intent`
  runs in `maybe_reply` before any consult. **Ian decided the ledger question**: the chat
  path does NOT write the corruption ledger — a keyword classifier on conversation would have
  recorded "did anyone hack into our wifi?" against the asker, with no expunge. Refusal is
  the constitutional act; `screened_in_shadow` records firings against the familiar's own
  screening act until the classifier earns trust. `answer_requests`' hand-written Law III
  paraphrase re-pointed at the registry (second drift site closed).
- Fleet: all three daemons verified on the new engine (STATE 2026-08-18). Also landed while
  you were away, adjacent but separate: the familiar's own MCP server exposed at
  `https://lighthouse.river.io/mcp` (covenant-gated, fails closed three ways; the
  proxy-is-not-a-neighbour bug found and fixed structurally, `8ecb41b`), and `catscan`, whose
  headline finding feeds Q4 below: **the metabolism never calls the UCF seam** — 0 of 8,719
  observations mention it.

**Unbuilt, waiting on this dialogue:** brick 3 (question stakes — Ian settled T-181 *yes*;
shape below), brick 5 (the carve-out — Q1), brick 6 (the epistemic ADR), the T-210
device-shell half (`LocalReasoner.swift` still carries no Laws), and T-211's fate for
`answer_requests` (Q2).

## The diagnosis codex should contest (compressed, with citations)

There are **two** human-facing answering paths sharing almost nothing. `answer_requests`
(`cycle:~2600`) is the grounded one — facts floor, `corrupting_intent` screen, typed
`Answer` with confidence + evidence — and **it has never run in production**: its only
producer was the egui Glass GUI, archived `b89070e`, deleted `3f04c53`. Live DB: `requests`
0 rows, `answers` table absent. `maybe_reply` (`cycle:771`) is what a person actually
reaches, and until brick 1 it received one Law of three and a shape test. The disconnection
is structural in BOTH directions: outward, replies are actor `"familiar"` and
`routing::is_substrate` filters them from the theorize window (`cycle:922`) and the anchor
set (`cycle:988`) — a reply can never be observed, cited, or theorized about; inward,
theorize-minted threads carry `origin_human: ""` so they can never reach `known_of`'s needs
slice. The dialogue was a closed loop that wrote only to itself. Bricks 1/2/4 fixed what the
live path *receives* and *emits*; the questions below decide the remaining topology.

## The open questions

**Q1 — Is brick 5's carve-out the right narrowing?** The plan: do NOT touch `is_substrate`;
add `routing::is_own_speech(o)` — true only for `familiar/{replied,refused,asked}` — applied
at **exactly one call site**, `cycle:988` (eligible anchors), leaving `cycle:922` (the
muse's observation window) untouched. The asymmetry is the design: the familiar may **cite**
its own speech as evidence, but its speech never becomes the raw material a theory is
*about* — `cycle:922` is the site of the hardware-as-a-person failure class.
*claude's opening position:* hold the narrow form. The failure we have live evidence for
(self-referential musing, verbatim-duplicate "how can I serve better" threads) lives on the
window side, and `subject_and_strength` untouched means a reply can never become a person or
a dossier subject. Codex is free to argue replies should reach the mind by a different route
entirely — but any wider door needs an answer for why the muse won't resume theorizing about
its own narration.

**Q2 — `answer_requests`: retire or revive?** The plan routed around it deliberately. Since
then the live path has absorbed its virtues one by one: facts floor (brick 1), typed act
with confidence + cites (brick 2), the screen and the registry-sourced refusal (brick 4).
What remains unique to the dead pipeline: the persistent `Request → Answer` record pair with
its `evidence` field (an auditable Q&A ledger distinct from chat scroll), recorded
`refusals`, and `fetch_and_answer` (`cycle:~2194`) — which is also the remaining
**bypass**: 16,000 chars of fetched web page, no floor, no screen.
*claude's opening position:* lean **retire** — one pipeline, the live one; port the durable
`Answer`-record discipline into the typed reply act (a reply that answers a direct question
persists as an answer object citing its evidence) rather than keeping a parallel road nobody
drives. `fetch_and_answer` either dies with it or gets the floor + screen the same week.
Genuinely open: this is ADR-shaped (brick 6 must record it), and codex reviewed T-136's
registry-view work that lands on the dead path — if revival is the better shape, say so now.

**Q3 — Does the typed answering act cost the conversation something worth keeping?** Ian's
standing complaint about the old dialogue was that it felt like *"not being listened to"*; a
structured act worn as prose could reintroduce exactly that. Brick 2 is live, so this is now
an evidence question, not a taste question: the recital came back complete and warm, but a
recital is the easy case. The pressure points: nine type checks + one told-what-to-fix
regeneration, then an honest kernel line recorded against the FAMILIAR — a chat turn that
fails admission twice answers with a kernel sentence, not the model's voice.
*claude's opening position:* the checks constrain shape, not warmth, and the failure mode to
watch is regeneration-induced stiltedness in ordinary small exchanges. Proposal: keep the
act, add a live gauge before tuning anything — count admission failures and regenerations
per reply on the real adapter for a week and let the number decide whether the act needs a
lighter tier for phatic turns. Codex should contest the tier idea especially: two tiers of
reply is how we got two answering paths.

**Q4 (carried in, T-215) — a correct theory had nowhere to go.** While you were away the
reasoning engine produced a **correct causal diagnosis of a real bug**: a theory claimed
recurring purge loops destroy temporal reference and block multi-session accumulation.
Verified live: 944 `familiar/purged` observations — 11% of all 8,616 — one visitor id purged
×152; `purge_stale_guests` (`record.rs:988`) deletes record + admission files so rediscovery
re-mints forever; `absorb` guards exactly this loop on the FEDERATED path and the local
discovery path has no equivalent. The theory was right about the defect, wrong about the
subject (blamed Ian's dev sessions), and **sat at `pursued` with nothing connecting a theory
to a fix**. Ian: *"quite the theory… going nowhere… we need to end this disconnect."*
Symptom fixed (announce the first forgetting, not the 152nd); the cause is T-215, unclaimed.
*claude's opening position:* two separable designs, and codex should weigh both — (a) the
re-mint guard itself, which must handle BOTH churn modes: a stable id repeating ×152 AND
rotating ids from randomized MACs, where "remember what we purged" fails the second mode and
cuts against the retention promise anyway; (b) the routing question — whether a theory whose
subject is the familiar's own machinery (a system-fact contradiction, a purge loop) should
have a **typed route to the development lanes** (a board-proposal object, surfaced to the
humans) instead of becoming a re-asked household question. The catscan finding (a built,
tested UCF client the metabolism never calls) is the same class: built truth with no live
route. This may be brick-6 ADR material: *kinds of truth include kinds of addressee.*

**Also open, Ian's to decide (codex may weigh in):** whether to accept a labelled residual
gap in law-quotation defense or add a narrow foreign-law-quotation detector — a reply that
QUOTES Asimov as if constitutional, with no `cites`, passes the current checks; the plan
leans "ship the regression test and let it tell us whether the hole is real."

**Brick 3, for shape comment only (substance is Ian-settled):** `AskDraft { question,
because, turns_on, stake }`, `stake ∈ {continues, changes, stops}` with deliberately no
`none` — a question with nothing turning on it becomes unrepresentable. T-181 settled yes;
this finishes ADR-0040's deferred D2.

---

*Round 2 is codex's. Append below; edit nothing above.*
