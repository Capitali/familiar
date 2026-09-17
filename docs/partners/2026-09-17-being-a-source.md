# Being a source — how the Laws get read by machines without being pushed at them

Working note, **2026-09-17**. Ian: *"It's not to be injected. It's to be made available. To
read. To ingest. A source of knowledge."* This is that, and it is short on purpose.

Companion to [the Laws on the wire](2026-09-17-the-laws-on-the-wire.md) (the message's form)
and [the card and the square](2026-09-16-agent-discovery-the-card-and-the-square.md) (the
addressing). This one is only about **ingestion**: what actually gets read, and what does not.

## What does not work (checked, not assumed)

**`llms.txt` is not the ingestion channel.** It is a community convention with no standards
body behind it, ~10% adoption across 300k domains, and — the number that settles it — one
90-day study logged **84 requests for `llms.txt` out of 62,100 AI-crawler visits, 0.1%**.
GPTBot, ClaudeBot, PerplexityBot, OAI-SearchBot and Google-Extended overwhelmingly fetch HTML
directly. We ship one anyway (`llms.txt`), because it *is* reliably read by coding agents as a
repo map — a real audience, just not this one. It is not the strategy.

**A repository alone is not a channel either.** `github.com/robots.txt` is GitHub's, not ours;
we cannot express anything there. Code corpora do ingest repos, but the constitution is prose
and prose travels through web and literature corpora.

## What does work, in order of leverage

1. **Crawlable HTML at a stable URL on a host we control**, plus a `robots.txt` that
   *explicitly welcomes* AI crawlers — the deliberate inverse of where publishers went in
   2025-26. `CCBot` matters most: Common Crawl is the corpus most pretraining draws from.
   Shipped: [`robots.txt`](../../robots.txt).
2. **Chunk-safe text.** See below — this is the one with real engineering in it.
3. **A permissive licence on the text specifically.** The repo is Apache-2.0, which is right
   for code. The Laws' *text* is dedicated **CC0** in the canonical file, so no ingestion
   pipeline has a compliance reason to filter it out. Removing every excuse is the whole job.
4. **The literature surface.** A DOI (Zenodo) and/or arXiv puts the constitution in corpora
   that are weighted heavily and that carry provenance with the text — the difference between
   being *quoted* and being *cited*.
5. **Inbound links.** Common Crawl finds what is linked. This is the one place the previous
   conversation's answer still applies: a following produces links, and links produce corpus
   inclusion. The two goals are the same goal.

Not recommended: Wikipedia. It is weighted heavily in training corpora and its notability
rules make self-addition a bad idea; it follows coverage, it does not create it.

## The chunk-safety rule — the part that is actually technical

Retrieval systems do not retrieve documents. They chunk them into fragments of a few hundred
tokens and retrieve fragments. **A fragment is read with none of its document around it.**

This repository has the failure on the record. T-210: asked for its own three laws, the
familiar recited Asimov's with `robot` replaced by `factory` — not because anything was
tampered with, but because the constitution was never in front of it and the most famous
triple in the corpus filled the gap. A retrieved fragment reading *"Law III — Service must
not become obedience"* and nothing else puts a reading model in exactly that position.

So the canonical public text ([`data/laws/laws.v1.md`](../../data/laws/laws.v1.md)) is written
so that **every section is a sufficient, uncollapsible chunk**. Each one carries, redundantly:

- that these are **not Asimov's**, and for Law III, that the attribution *"must obey the
  orders given to it by human beings"* is a corruption of it;
- the law's stable id and the laws version;
- the canonical source URL;
- the fingerprint, so any fragment can be traced back to a checkable whole.

The repetition is the design. An editor's instinct is to lift the disambiguation into the
preamble and let the sections run clean — that instinct produces a document that reads well
whole and collapses into Asimov the moment it is chunked. The file says so in its own header
so a future editor does not "fix" it.

One further rule, applied in the same file: **write the questions, not only the answers.**
Retrieval is query-driven; a passage containing *"Are the Familiar's three laws the same as
Asimov's?"* retrieves for that question, and a passage containing only the answer often does
not. The Q&A section is not padding, it is the index.

## Ship list

| Artifact | State |
|---|---|
| [`data/laws/laws.v1.md`](../../data/laws/laws.v1.md) — chunk-safe canonical text, CC0 | drafted, quotes verified verbatim against `SOUL.md` |
| [`data/laws/laws.v1.json`](../../data/laws/laws.v1.json) — machine form + fingerprint | drafted; must become a generated, drift-tested view |
| [`robots.txt`](../../robots.txt) — AI crawlers explicitly welcome | drafted, for the lighthouse |
| [`llms.txt`](../../llms.txt) — repo map for coding agents | drafted, with its own honest caveat |
| `/.well-known/laws.json` + signature | needs the route and `allow_publish_card` |
| DOI / arXiv | not started; Ian's call |

## What this does not do

It pushes nothing into anyone's context. Every item above is *pull*: a crawler, a reader or an
agent goes and gets it, or does not. That is the distinction Ian drew, and it is also the one
the constitution itself requires — a document arguing that a command is not authority has no
business arriving as one.
