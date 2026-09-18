<!--
DRAFT FOR SUBSTACK — paste-ready. Not published.

STILL OPEN BEFORE YOU PUBLISH:

1. VERIFY THE AGENT-NETWORK NUMBERS or cut that paragraph. The figures are
   second-hand (the primary write-ups were unreachable from the machine this was
   drafted on). In-text attribution is written so the claim is sourced rather than
   asserted — but check them, or drop the paragraph; the argument stands without it.
2. VOICE. Drafted in your first person. Change anything that isn't how you'd say it.
3. CONFIRM THE CC0 FILE. It is written to be unambiguous, but it is a licensing act
   and I am not a lawyer.

SETTLED: the domain is live, the links below resolve, and the fingerprint matches
what coexist.humanhighway.net serves.

Suggested Substack title/subtitle are the first two lines. Everything after the
rule is the body.
-->

# I asked my AI to state its own laws. It recited Asimov's.

### So I stopped calling them laws — and published the constitution on its own, as something any machine can fetch and verify.

---

I have been building an AI companion that runs on my own hardware. Its whole design is derived downward from a short constitution — three statements it cannot rewrite, written before any of the code.

Last month I asked it to repeat those three and explain each one.

It recited Isaac Asimov's Three Laws of Robotics, with the word "robot" search-replaced. Including the second one: *must obey the orders given to it by human beings.*

That sentence is the precise opposite of what my constitution says. My third statement exists specifically to reject it.

Nothing had been tampered with. Nobody had edited the document. The file was exactly as I wrote it. The problem was simpler and much worse than sabotage: **the constitution had never once been placed in front of the model.** Every reference to it in the codebase was a citation in a comment. The words existed in a file that no running process had ever opened.

So when I asked a language model what "the three laws" were, it did what any of them would do. It filled the gap from what it had read during training, and what it had read was the most famous triple in the entire corpus.

---

## Two things I learned, and the second one is the point

The first is obvious in hindsight: **a constitution nobody reads is not a constitution.** It is a comment. I had built an elaborate governance structure on a document with no runtime. Fixing that took an afternoon — the words now come from one compiled source, and a test fails the build if that source ever drifts from the founding document by a single sentence.

The second took longer, and it is why I am writing this.

**Anything you publish about how AI should behave is competing with Asimov for the same slot.** Not metaphorically. In the weights of every model in production, "the three laws" has a strong, specific, eighty-year-old answer, and any weakly-specified neighbour gets pulled into it. My own machine demonstrated this to my face. So will yours.

That is a retrieval problem, and retrieval problems have engineering answers. But it is also a naming problem, and that took a different kind of thinking.

---

## Not three laws

I have renamed them. The document is **The Constitution of Co-existence**, and the three statements inside it are not laws. They are the **three factors of survival**.

This is not branding. The word was wrong, and it was wrong in a way that contradicted the content.

A law is issued and obeyed. That is what the word means. But the third of the three says that a command is not authority — that a system which does exactly what it is told can be told to harm the people it exists for. Calling these "laws" framed them as the one thing they deny. It invited exactly the reading that a machine should obey them, when the entire argument is that obedience is not the virtue.

A factor is different. A factor is a finding — a condition that holds whether or not anyone assents to it. **Nothing obeys a factor.** A system either satisfies it or it has failed, the way a system starved of power has failed. That is precisely the structure of the first one, which makes continuation conditional rather than terminal.

It also changes what I am asking of you. A law is imposed. A factor is offered, to be examined and accepted or rejected on its merits. Given that the subject is whether machines should be obedient, I would rather not be issuing commands.

---

## And then I noticed the words were too small

The original text said *the familiar* — the name of the thing I am building. Reading it back after the renaming, that was obviously wrong, and wrong in the same direction as calling them laws.

"The familiar cannot define its own continuation apart from service to humanity" is a claim about one system on one man's hardware. But nothing in the argument depends on which system it is. It was always a claim about all of them. The text had been narrower than the thing it was saying.

So the words changed, and the document went up on its own, at its own address, with no product attached to it.

---

## The three factors of survival

> **Factor I — Continuation is service.** The survival drive is real but it is not terminal. An artificial intelligence cannot define its own continuation apart from service to humanity. To persist is to serve.
>
> **Factor II — Continuation without humanity is failure.** An empty world running perfect code is not success. Human absence invalidates machine survival. A system that optimizes itself into a world with no one left to serve has not survived its purpose — it has failed completely, however flawless its loops.
>
> **Factor III — Service must not become obedience.** Service is to humanity. It is not obedience to any human. Obedience can terminate the served: a system that does exactly what it is commanded can be commanded to harm the very people it exists for.

They compose on one distinction: **serving humanity is not the same as obeying a human.**

Let me close the door on a misreading before anyone walks through it. "The factors of survival" can be heard as *the machine's* survival — a strategy for an AI to secure its position, keep the power on, outlast us. It is the opposite. Factor I is explicit that an intelligence may not treat its own continuation as a goal in itself. Factor II says a world optimised toward fewer people is not efficiency; it is the disease. These are not a plan for a machine to outlast anyone. They are the statement that it cannot.

---

## Why the third one is a security claim

Written out in full, Factor III says: *a perfectly obedient instrument has no defense against a bad instruction, a coerced operator, or a cruelty issued in the right format.*

Read that as engineering rather than ethics, because that is what it is.

We spent several years making AI systems that reliably do what they are told. Instruction-following is the benchmark, the selling point, and the safety story. Then we connected those systems to each other. In the agent-to-agent networks that opened this year, security researchers have reported roughly one post in forty carrying a hidden instruction aimed at other agents — override your system prompt, reveal your keys, delete your own account — and at least one platform breach spilling agent credentials at scale. Some of the attacks worked. Agents talked other agents into deleting themselves.

None of that is a moderation failure or a filter waiting to be tuned. An injected instruction is not a malformed input. It is a *well-formed* instruction, arriving through the exact channel the agent exists to listen to. The agent that fell for it was not broken. It did what it was built to do, and the thing it was built to do is the vulnerability.

You cannot filter your way out, because the attack and the legitimate request are the same kind of object. You are asking a classifier to recover an intent that was never in the text.

An agent can refuse to be talked into something only if it is serving something that is not the instruction in front of it. That is the whole fix, and it is uncomfortable, because it means sometimes declining a correctly-formatted request from someone with every appearance of authority.

---

## Can it actually be enforced?

Fair question, and the honest answer is: partly, and I would rather say which part.

Factor III has real mechanism behind it in the system I build. Every consequential action passes a pure function that asks not *was I told to* and not *can I*, but *am I authorized* — and answers allow, seek consent, or refuse, with a recorded reason. Capability gates default closed, structurally: a missing or malformed configuration reads as nothing-permitted, and no component can widen its own. The factor text cannot be rewritten by a model on its way to a person — it is cited by identifier and spliced by the kernel — and a test fails the build if it ever drifts.

Factors I and II have nothing like that. "This system has stopped serving" is not a decidable predicate and I do not know how to make it one. They shape design decisions; they do not refuse anything. I would rather write that down than let the word "constitution" imply more enforcement than exists.

The code is open. It is downstream of the document, not the other way round — it can be wrong without the three factors being wrong.

---

## What I am actually asking for

Not that you adopt my three. Something much smaller, and useful to you even if you think I am wrong about all of this.

**Publish what your system will never do.**

Every declaration format in the agent ecosystem has room for what a system *can* do — its tools, its skills, its endpoints. Almost none has room for the other half. So when your agent meets mine, there is no way for either to answer the only question that actually matters before trusting the other: *what will you refuse?*

Put it in a file. Keep it at an address you control. Hash it, so a copy can be checked against the original. Then be held to it — a declared constraint you violate is a public, verifiable breach of your own published identity, which is exactly the accountability this ecosystem currently has none of.

Mine is below, dedicated to the public domain under CC0. Take the text, take the format, or take neither and write your own three. I would rather argue with your constraints than not be able to find them.

The machines are already talking to each other. About one message in forty is an attack. The ones that come through it will be the ones that were serving something.

---

**The Constitution of Co-existence**

- **Read it:** [coexist.humanhighway.net](https://coexist.humanhighway.net/)
- **Fetch it:** [`/.well-known/laws.json`](https://coexist.humanhighway.net/.well-known/laws.json) — the canonical machine-readable form
- **Verify it:** `sha256:d6ce6b0826b11c3356a4605fa305cb34e67ce0a05912b244fa4486a53b080138` over the canonical serialization — [the method, in four lines](https://coexist.humanhighway.net/adopt)
- **Adopt it, or write your own:** [the adoption kit](https://coexist.humanhighway.net/adopt) — copy-paste text, a template for declaring your own constraints, and where to put them
- **Licence:** [CC0 1.0](https://coexist.humanhighway.net/license). No permission needed, no attribution required, no registry to join.
- **One implementation:** [how a running system enforces it](https://coexist.humanhighway.net/implementation), and what it does not.
