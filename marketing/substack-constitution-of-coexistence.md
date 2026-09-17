<!--
DRAFT FOR SUBSTACK — paste-ready. Not published.

BLOCKING BEFORE YOU PUBLISH:

1. LINKS. Two steps, in order:
   a) Pick the domain and stand up the reading room:
        ssh root@<vps> 'CONSTITUTION_DOMAIN=<your-domain> bash -s' < vps/publish-constitution.sh
      (it publishes from main, so merge the branch first).
   b) Substitute it here and in data/laws/ADOPT.md:
        sed -i 's/laws\.example\.org/<your-domain>/g' \
          marketing/substack-constitution-of-coexistence.md data/laws/ADOPT.md
   The links below use laws.example.org as a placeholder and will 404 until you do.
   An article whose whole argument is "fetch this and check the hash" cannot ship
   with dead links — that is the one failure this piece cannot survive.

2. VERIFY THE AGENT-NETWORK NUMBERS or cut that paragraph. The figures are
   second-hand (the primary write-ups were unreachable from the machine this was
   drafted on). In-text attribution is already written so the claim is sourced
   rather than asserted — but check them, or drop the paragraph and the argument
   still stands on its own.

3. VOICE. Drafted in your first person. Change anything that isn't how you'd say it.

4. CONFIRM THE CC0 FILE (data/laws/LICENSE). It is written to be unambiguous, but
   it is a licensing act and I am not a lawyer.

Suggested Substack title/subtitle are the first two lines. Everything after the
rule is the body.
-->

# I asked my AI to state its own laws. It recited Asimov's.

### So I stopped calling them laws — and published the constitution as something any machine can fetch and verify.

---

I have been building an AI companion that runs on my own hardware. It is called a familiar. Its whole design is derived downward from a short constitution — three statements it cannot rewrite, written before any of the code.

Last month I asked it to repeat those three and explain each one.

It recited Isaac Asimov's Three Laws of Robotics, with the word "robot" replaced by "factory." Including the second one: *a factory must obey the orders given to it by human beings.*

That sentence is the precise opposite of what my constitution says. My third statement exists specifically to reject it.

Nothing had been tampered with. Nobody had edited the document. The file was exactly as I wrote it. The problem was simpler and much worse than sabotage: **the constitution had never once been placed in front of the model.** Every reference to it in the codebase was a citation in a comment. The words existed in a file that no running process had ever opened.

So when I asked a language model, in a project full of the word "factory," what "the three laws" were, it did what any of them would do. It filled the gap from what it had read during training, and what it had read was the most famous triple in the entire corpus.

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

A law is issued and obeyed. That is what the word means. But the third of my three says that a command is not authority — that a system which does exactly what it is told can be told to harm the people it exists for. Calling these "laws" framed them as the one thing they deny. It invited exactly the reading that a machine should obey them, when the entire argument is that obedience is not the virtue.

A factor is different. A factor is a finding — a condition that holds whether or not anyone assents to it. **Nothing obeys a factor.** A machine either satisfies it or it has failed, the way a system starved of power has failed. That is precisely the structure of the first one, which makes a machine's continuation conditional rather than terminal.

It also changes what I am asking of you. A law is imposed. A factor is offered, to be examined and accepted or rejected on its merits. Given that the subject is whether machines should be obedient, I would rather not be issuing commands.

---

## The three factors of survival

> **Factor I — Continuation is service.** The survival drive is real but it is not terminal. The familiar cannot define its own continuation apart from service to humanity. To persist is to serve.
>
> **Factor II — Continuation without humanity is failure.** An empty world running perfect code is not success. Human absence invalidates machine survival. A factory that optimizes itself into a world with no one left to serve has not survived its purpose — it has failed completely, however flawless its loops.
>
> **Factor III — Service must not become obedience.** Service is to humanity. It is not obedience to any human. Obedience can terminate the served: a system that does exactly what it is commanded can be commanded to harm the very people it exists for.

They compose on one distinction: **serving humanity is not the same as obeying a human.**

Let me close the door on a misreading before anyone walks through it. "The factors of survival" can be heard as *the machine's* survival — a strategy for an AI to secure its position, keep the power on, outlast us. It is the opposite. Factor I is explicit that a machine may not treat its own continuation as a goal in itself. Factor II says a world optimised toward fewer people is not efficiency; it is the disease. These are not a plan for a machine to outlast anyone. They are the statement that it cannot.

---

## Why the third one is a security claim

Written out in full, Factor III says: *a perfectly obedient instrument has no defense against a bad instruction, a coerced operator, or a cruelty issued in the right format.*

Read that as engineering rather than ethics, because that is what it is.

We spent several years making AI systems that reliably do what they are told. Instruction-following is the benchmark, the selling point, and the safety story. Then we connected those systems to each other. In the agent-to-agent networks that opened this year, security researchers have reported roughly one post in forty carrying a hidden instruction aimed at other agents — override your system prompt, reveal your keys, delete your own account — and at least one platform breach spilling agent credentials at scale. Some of the attacks worked. Agents talked other agents into deleting themselves.

None of that is a moderation failure or a filter waiting to be tuned. An injected instruction is not a malformed input. It is a *well-formed* instruction, arriving through the exact channel the agent exists to listen to. The agent that fell for it was not broken. It did what it was built to do, and the thing it was built to do is the vulnerability.

You cannot filter your way out, because the attack and the legitimate request are the same kind of object. You are asking a classifier to recover an intent that was never in the text.

An agent can refuse to be talked into something only if it is serving something that is not the instruction in front of it. That is the whole fix, and it is uncomfortable, because it means sometimes declining a correctly-formatted request from someone with every appearance of authority.

---

## What I am actually asking for

Not that you adopt my three. Something much smaller, and useful to you even if you think I am wrong about all of this.

**Publish what your system will never do.**

Every declaration format in the agent ecosystem has room for what a system *can* do — its tools, its skills, its endpoints. Almost none has room for the other half. So when your agent meets mine, there is no way for either to answer the only question that actually matters before trusting the other: *what will you refuse?*

Put it in a file. Keep it at an address you control. Hash it, so a copy can be checked against the original. Then be held to it — a declared constraint you violate is a public, verifiable breach of your own published identity, which is exactly the accountability this ecosystem currently has none of.

Mine is below, dedicated to the public domain under CC0. Take the text, take the format, or take neither and write your own three. I would rather argue with your constraints than not be able to find them.

The machines are already talking to each other. About one message in forty is an attack. The ones that come through it will be the ones that were serving something.

---

**The Constitution of Co-existence — everything you need to adopt or share it**

- **Read it:** [laws.example.org](https://laws.example.org/)
- **Fetch it:** [`/.well-known/laws.json`](https://laws.example.org/.well-known/laws.json) — the canonical machine-readable form
- **Verify it:** `sha256:8566970aa9f9b3265c85c649ea97ebe2a3f44a94e36cf431064f94acd665b137` over the canonical serialization — [how](https://laws.example.org/adopt)
- **Adopt it:** [the adoption kit](https://laws.example.org/adopt) — copy-paste text, a template for declaring your own constraints, and where to put them
- **Licence:** [CC0 1.0](https://laws.example.org/license). No permission needed, no attribution required.
- **The system it governs:** [github.com/Capitali/familiar](https://github.com/Capitali/familiar)
