# T-230 · Closing the calibration loop — the familiar learns to predict

Ian, 2026-08-29: "continue working on the familiar's reasoning skills. Deploy
and ship when ready."

## Round 1 — claude (chair): the gap, and brick 1

A survey of the reasoning engine surfaced one consistent pattern: **the
measurement half of every reasoning loop is built, but the learning half is
not.** Predictions are scored (deterministic settlement, append-only results,
per-thread calibration, the T-221 observed-vocabulary rule) — but nothing feeds
that record back into *making better predictions*. The theorize prompt
(`cycle/src/lib.rs`) that asks the reasoner to invent predictions has never
seen the familiar's own track record or the base rate of any event class. Each
prediction is invented fresh, and the observed-vocabulary rule guarantees a
prediction is *observable* but not *informative* — a familiar can look
well-calibrated by only ever predicting the inevitable.

**Brick 1 (built, this doc):** inject a deterministic, derived **calibration
feedback digest** into the theorize prompt, closing the loop with no schema
change:

- `kernel/src/prediction.rs::feedback_digest(results, class_freqs, now, window)`
  — pure and tested. It reports the familiar's recent settled record
  (confirmed vs missed within a two-week window), an honest nudge only when the
  record clearly says one thing (over-predicting / landing), and the observed
  event classes bucketed by base rate (often / sometimes / rare). Empty string
  when there's nothing to say, so it never pads the prompt.
- `cycle/src/lib.rs::maybe_theorize` computes the observed classes *with counts*
  once (reused for both the closed-world prediction list and the digest), then
  injects the digest into the prompt with the guidance: *a rare event called
  correctly teaches more than an inevitable one*.

Why this is the right first brick: it uses machinery that already exists (the
append-only results, the vocab tally), changes only the *generative* step (the
kernel's deterministic truth loop is untouched — scoring, settlement, and the
belief state machine are unchanged), and it is honest (the familiar reads its
own record back, including when it is doing badly). It makes the reasoner
*learn to predict*, not just *be measured*.

Bounds and guards:
- The digest is derived and deterministic; no model is in this loop.
- It is honest: the over-predicting nudge fires only when misses clearly
  dominate (`unfavorable*3 >= confirmed*7`, ≥4 settled); the "landing" praise
  only when confirms clearly dominate — never manufactured encouragement.
- The window bounds the record to the recent past; classes are bounded to 12
  in the digest and 40 in the vocab line.

## Questions for codex

- Q1 — is the honest-nudge heuristic (the two thresholds) the right shape, or
  should the digest report the numbers and let the reasoner judge, with no
  editorializing at all?
- Q2 — the digest keys base rates on `actor|action`, matching the prediction
  contract's class. A natural follow-up (brick 2) is per-CLASS hit rate, which
  needs the predicted class stored on `PredictionResult` (a defaulted,
  append-only field written at settlement). Worth doing, or does the aggregate
  record + base rate already carry the signal?
- Q3 — should the owed weekly miss/coverage/latency report (STATE.md) be built
  from the same digest so the human sees what the reasoner sees?

## Bar

kernel 97/0 (5 new feedback_digest tests), cycle 237/0, clippy 0. Full
workspace bar recorded in the reciprocal-review request.
