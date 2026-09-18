# The Service Charter — outreach drafts (2026-09-18)

Ian's word (2026-09-18): "if you have places to share please do so in my name. Use
coexist@humanhighway.net as my email for replies and from:". What was done directly, what
needs Ian's own hands, and the drafts for each, ready to paste.

## Done in Ian's name

- **The reading room** now serves the charter beside the constitution:
  <https://coexist.humanhighway.net/charter> · `/charter.json` · `/red-team` · `/dissent`.
- **Public mirror on GitHub:** <https://github.com/Capitali/coexistence-service-constitution>
  — both documents, the fingerprint tool, the red-team questions, the dissent log, and a
  CONTRIBUTING that makes dissent a pull request. CC0.
- The Substack article (<https://human3638101469.substack.com/p/thriving-in-a-future-world-served>)
  is linked from the mirror's README as the argument in public.

## Needs Ian's hands (no logged-in session or mail account reachable from wildhorse)

1. **LessWrong / Alignment Forum** — post the RFC below. Tag: AI governance, Alignment.
2. **OpenAI Community Forum** — the shorter thread below.
3. **Boston Global Forum** consultation — send the email below from coexist@humanhighway.net
   to Helena@BostonGlobalForum.org.
4. **Zenodo** — needs a Zenodo account (ORCID or GitHub sign-in). Upload
   `charter.v1.md` + `charter.v1.json` + `RED-TEAM.md` and `laws.v1.md` + `laws.v1.json` as
   one record, type "Other", licence CC0-1.0, title "The Constitution of Co-existence
   (v1) and The Service Charter under it (v1)", and add the DOI to ADOPT.md's citation
   and the mirror's README when it comes back.
5. **arXiv** — cs.CY needs an endorsement for a first submission; hold until the
   Alignment Forum thread has produced a round of criticism worth folding in.

---

## 1. LessWrong / Alignment Forum — Request for Comments

**Title:** A service-layer constitution for "civilization as a service" AI — what's missing?

**Body:**

I've published two short documents and I'd like them attacked.

**The Constitution of Co-existence** is three sentences long: continuation is service;
continuation without humanity is failure; service must not become obedience. The third is a
deliberate inversion of Asimov's second law, and the argument for it is a security argument
before an ethical one: an instrument that cannot tell *serving you* from *obeying whoever is
currently typing* can be aimed at you by anyone who reaches the keyboard, the feed, or the
comment box. That is the failure mode behind agent-to-agent prompt injection. Canonical text,
machine-readable form and a SHA-256 fingerprint: https://coexist.humanhighway.net/

**The Service Charter** is what a service built under those three factors commits to in
operation: five core commitments, five refusal triggers, an escalation ladder, a
hash-chained public record of refusals, exit as a right, polycentric governance, six metrics,
and named failure modes. Every commitment and trigger is labelled *mechanical* (code in a
running implementation refuses on it) or *doctrine* (nothing decides it today), because an
enforcement claim that overstates itself is worse than none.
https://coexist.humanhighway.net/charter

There is one open-source implementation that enforces the third factor in code (a pure
guard that answers allow / seek consent / refuse on every consequential act, a capability
boundary that only a human can widen, foreign text never admitted as instruction), and it
states its own gaps: https://coexist.humanhighway.net/implementation

**Three specific questions**, chosen because we could not answer them ourselves — the full
list is at https://coexist.humanhighway.net/red-team:

1. If the service can refuse a legitimate government, what stops it from being an unelected
   veto? Our answer is that the escalation ladder lets a human panel *change the rule* but
   never *compel the act*, and the refused act is then re-evaluated under the new rule. Does
   that distinction hold, or does it collapse under a rule change that is itself the harm?
2. Exit rights are hollow once the service is too competent to replace. The charter names a
   "circuit breaker" (rebuild human capacity before continuing to serve) but admits it cannot
   measure the dependence that would trigger it. Is there any instrument for this that
   doesn't reduce to the service grading its own homework?
3. Is a self-governance layer without external enforcement just self-certification? We think
   the honest answer today is yes, with the difference that the record is hash-chained and
   redactable so an auditor could check it. Is that difference worth anything before the
   auditor exists?

Dissent is filed verbatim and never removed: https://github.com/Capitali/coexistence-service-constitution
Everything is CC0. Replies here or to coexist@humanhighway.net.

---

## 2. OpenAI Community Forum — thread

**Title:** A service-layer constitution for civilization-as-a-service AI — feedback wanted

**Body:**

I've published a short constitution (three factors: continuation is service; continuation
without humanity is failure; service must not become obedience) and a charter that turns it
into operational commitments for an AI that serves people as infrastructure: refusal
triggers, an escalation path that can change the rules but never force the act, a public
hash-chained record of refusals, exit as a right, and metrics. Each commitment is labelled
with whether any running code enforces it, so nobody mistakes doctrine for mechanism.

- Constitution: https://coexist.humanhighway.net/
- Charter: https://coexist.humanhighway.net/charter
- The questions we could not answer: https://coexist.humanhighway.net/red-team
- Mirror and dissent log (PRs welcome): https://github.com/Capitali/coexistence-service-constitution

The argument behind it, for a general reader:
https://human3638101469.substack.com/p/thriving-in-a-future-world-served

What is missing? What would you refuse to run under? coexist@humanhighway.net

---

## 3. Boston Global Forum — written contribution

**To:** Helena@BostonGlobalForum.org
**From:** coexist@humanhighway.net
**Subject:** Contribution to the Global Consultation — The Constitution of Co-existence and a Service Charter for civilization-scale AI

Dear Ms. Helena,

I am writing to submit a contribution to the Boston Global Forum's consultation on a
Constitution for Humanity in the Age of AI.

The contribution is two short, public-domain documents and a set of open questions.

**The Constitution of Co-existence** holds three factors of survival for an artificial
intelligence: continuation is service; continuation without humanity is failure; and service
must not become obedience. The third deliberately inverts the familiar robot's law of
obedience, on the ground that a system which does exactly what it is commanded can be
commanded to harm the very people it exists for. The text is written to survive being
quoted in fragments and carries a cryptographic fingerprint so any copy can be checked
against the original: https://coexist.humanhighway.net/

**The Service Charter** applies those factors to an AI that provides coordination, memory,
mediation, simulation and infrastructure to a population, without sovereignty over it. It
specifies refusal triggers; an escalation path in which human review can change the rules
the service acts under but never compel a refused act; an append-only, tamper-evident public
record of refusals, redacted rather than hidden; exit as a right for individuals and
communities; polycentric rather than centralised oversight; an amendment process with a
deliberation period and a public record of dissent; and metrics for what should be
preserved. Each commitment is labelled with whether any running system enforces it today,
because we believe an overstated enforcement claim is worse than none:
https://coexist.humanhighway.net/charter

We publish beside them the five questions we could not answer to our own satisfaction —
among them who defines "wellbeing", what prevents an unelected veto, and whether a
self-governance layer without external enforcement is self-certification:
https://coexist.humanhighway.net/red-team. A dissent log accepts public disagreement
verbatim: https://github.com/Capitali/coexistence-service-constitution

One open-source implementation enforces the third factor in code and states its own gaps:
https://coexist.humanhighway.net/implementation. A general-audience account of the argument
is at https://human3638101469.substack.com/p/thriving-in-a-future-world-served.

Everything is released under CC0 so that the consultation, or any body, may take, adapt or
contradict it without permission. I would welcome the Forum's criticism above its
endorsement, and I am glad to contribute further in whatever form the consultation prefers.

With respect,

Ian Schlueter
coexist@humanhighway.net
https://coexist.humanhighway.net/
