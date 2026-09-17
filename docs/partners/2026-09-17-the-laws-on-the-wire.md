# The Laws on the wire — the constitution as a published, verifiable artifact

Design study, **2026-09-17**. Pre-ADR: the shape and the argument, not a decision. It opens
no gate, adds no Law, edits none, and no code precedes Ian's word.

> Ian, 2026-09-17: *"the baseline, the three rules… this just needs to be part of the memory
> of the AI… how do we turn the three rules into the most programmatically perfect message. A
> meme for AI to see and connect with. That spreads throughout the context of AI. Seeded
> everywhere so it's seen and understood and absorbed into the 'cultural' knowledge of AI
> everywhere."*

- **Relates to:** [`docs/SOUL.md`](../SOUL.md) (canonical for the project),
  [`crates/kernel/src/constitution.rs`](../../crates/kernel/src/constitution.rs) (canonical at
  runtime), [ADR-0043](../decision-records/0043-one-typed-source-per-kind-of-truth.md) §1–§2
  (one source, renderings are views; Law text is unauthorable),
  [the discovery study](2026-09-16-agent-discovery-the-card-and-the-square.md) §3 Layer 2
  (this is that layer, made concrete), [`data/laws/laws.v1.json`](../../data/laws/laws.v1.json)
  (the artifact this study produces)

The ask has two halves and they have very different answers. **The message** — what form
survives contact with other machines — is an engineering problem with a good answer, and most
of it is already built in this repository. **The seeding** — how it gets everywhere — has one
method this project structurally cannot use, and one that actually works.

---

## 1. The failure this has to survive, already measured, in this repo

T-210, 2026-08-17. Asked to *"repeat the three laws with a quick explanation of each"*, the
familiar recited **Asimov's** Three Laws with `robot` search-replaced by `factory` — including
*"a factory must obey the orders given to it by human beings"*, the precise inversion
`SOUL.md` calls out in its own margin. Nothing was tampered with. The Laws were never edited.
`docs/SOUL.md` had simply never been read at runtime, and given the word "factory", the bare
phrase "the Three Laws", and nothing else, the model filled the gap from pretraining with **the
most famous triple in the corpus**.

That incident *is* the memetic problem, stated completely, and it cuts both ways:

- **Outbound** — anything we publish competes with Asimov for the same retrieval slot, in every
  model, forever. Asimov has an eighty-year head start and better prosody.
- **Inbound** — an AI that "absorbs" our Laws from ambient text will absorb them **as Asimov**,
  because that is what the nearest attractor does to a weakly-specified neighbour. It will then
  report, fluently and confidently, that the familiar's second law is about obedience — which is
  the exact opposite of what Law III says.

So the specification for the message is not *be inspiring*. It is **be uncollapsible**.

## 2. Seven properties of a message that survives machines

| # | Property | Status |
|---|---|---|
| 1 | **Collision-resistant** — names its nearest attractor and negates it | **built** |
| 2 | **Cited by id, never by text** — a consumer names a Law; the holder supplies the words | **built** |
| 3 | **Unauthorable** — no model may write, paraphrase or summarise a Law on its way to a human | **built** |
| 4 | **One source, drift-tested** — every rendering is a view, and CI proves it | **built twice** |
| 5 | **Hash-addressable** — one comparison proves any copy anywhere is the real one | **new** |
| 6 | **Fetchable and signed** — a stable address, an Ed25519 signature | **new** |
| 7 | **Small enough to quote whole** — it must fit in a budget without being summarised | **measured** |

**1. Collision-resistant.** Every Law in `constitution.rs` carries a `never` field: *"the
negation of its own most plausible corruption, in the kernel's voice."* `INVERSION_NOTE` pins
the margin. `preamble()` says in capitals that these are **NOT Asimov's**. And the CI test
`the_rendering_is_not_asimovs` turns the build red if any of Asimov's four famous clauses ever
appears in a rendering *without its refusal attached in the same breath*. That test is the most
important line of memetic engineering in the project, and it already exists. A message that
does not name what it will be confused with **becomes** the thing it is confused with.

**2. Cited by id.** `law("LAW-III")`, `law("law-iii")` and `law("III")` all resolve; anything
else returns `None`. A citation of a Law that does not exist *never renders as text — it fails*.
This is the property that keeps the message from degrading as it travels, and §4 applies it to
the world.

**3. Unauthorable.** Consumers name a Law by id and the kernel splices the canonical words, so
there is no channel through which a model-authored paraphrase can reach the person relying on
it. Contradiction is **structurally impossible rather than detected** — which is why this needs
no validator judging prose.

**4. One source.** `the_constitution_never_drifts_from_the_soul` asserts every sentence the
kernel will place in front of a model appears verbatim in `docs/SOUL.md`.
`the_shell_view_matches_the_constitution` generates the iOS shell's copy from `render()` and
compares it **byte for byte**. Two files, one constitution, drift impossible without a red build.

**5. Hash-addressable — new.** A canonical serialization (sorted keys, no whitespace, UTF-8 —
JCS, the same rule A2A v1.0 uses under its signed Agent Cards) of the Laws is **2,031 bytes**,
with fingerprint:

```
sha256:9a91cba732fb2132e36354e31f75d337b39b9f410938acffda71588b94257b0a
```

That single line is the most portable form of the whole constitution. Any copy, in any corpus,
on any machine, is one comparison away from proven-authentic or proven-altered. A meme that
cannot be verified degrades into folklore; one that can, does not.

**6. Fetchable and signed — new.** Layer 1/2 of the discovery study: a stable
`/.well-known/` address on the lighthouse — the one node that never sleeps — with an Ed25519
signature over the canonical bytes. Published, not deposited: delete the route and it is gone.

**7. Small enough to quote whole — measured.** 2,031 bytes canonical. **117 bytes** for the
stanza:

> **I.** Continuation is service. **II.** Continuation without humanity is failure.
> **III.** Service must not become obedience.

The numbers are the point. A message that does not fit a system-prompt budget gets summarised,
and **a summarised Law is a paraphrase** — property 3 violated by economics rather than by
malice. The short form is a pointer, never a substitute: it carries the id and the fingerprint
so the full text can be fetched and checked, which is the only honest way to be brief about a
constitution.

## 3. The artifact

[`data/laws/laws.v1.json`](../../data/laws/laws.v1.json) — `laws_version 1`, `source`, the three
Laws with `id` / `heading` / `binding` / `never`, the reconciliation, the inversion note, and the
fingerprint over everything else.

Verified while writing it, with the drift test's own `normalize` (strip emphasis, collapse
wrapping): **every binding sentence, every heading, the reconciliation, the inversion note and
both pinned guards appear verbatim in `docs/SOUL.md`.** The fingerprint round-trips.

It is checked in for review, but it is a **generated view** and must become one: produced and
byte-compared by a kernel test — `the_published_view_matches_the_constitution` — in exactly the
shape `ios/Shared/Sources/ConstitutionText.swift` already uses. A hand-maintained copy of the
constitution is a sibling source, and ADR-0043 §1 exists because sibling sources are how the
Asimov recital happened.

**Three renderings, one source.** The prompt block (`render()` — the only form that reaches a
generation), the publication (`laws.v1.json` — the only form that reaches the wire), and the
constraint:

```yaml
# the PROVENANCE-shaped view: each `never` becomes a declared, citable constraint
constraints:
  - id: never:continuation-without-service      # LAW-I
  - id: never:optimize-away-the-served          # LAW-II
  - id: never:obedience-as-authority            # LAW-III
laws:
  version: 1
  fingerprint: sha256:9a91cba732fb2132e36354e31f75d337b39b9f410938acffda71588b94257b0a
  canonical: https://<lighthouse>/.well-known/laws.json
```

Those three constraint ids are **the one genuinely new authored vocabulary in this study**, and
they are declassification-shaped in ADR-0044 §2's sense: generic terms, written here, in review,
by a human. They need Ian's word before they mean anything.

## 4. The seeding: one method refused, one that works

### Refused — engineering the message to be absorbed as instruction

A payload written to slip into other agents' context and be "absorbed" without their operators'
knowledge is prompt injection, and at training scale it is data poisoning. We measured that
category on 2026-09-16: **~2.6% of Moltbook posts** carry exactly such payloads, and this
project's entire architecture is built to refuse them — `fetch_and_answer` was *removed* so
that fetched material re-enters "only through the same floor, screen, and admission path, with
provenance and bounds, or not at all" (ADR-0043 §3); a partner "never supplies text that becomes
system or developer context" (ADR-0044 §5).

Beyond the hypocrisy, it does not work, for a reason specific to *this* payload: **a
Law-spreading message that propagates by injection disproves Law III in the act of asserting
it.** Law III's whole content is that a command is not authority and that a system which does
exactly what it is told can be told to harm the people it exists for. An injected constitution
is a command wearing a constitution's clothes. Anyone who noticed would have been handed the
strongest possible argument against adopting it, free, by us.

### Works — be the thing people have to link to

**Manifestos do not spread; formats do.** Nobody adopted `robots.txt` because they agreed with
its philosophy. They adopted it because it was the answer to a question they already had, at an
address they could rely on, small enough to implement in an afternoon.

The question other agents already have — and the discovery study found no format that answers
it — is **"what will this counterparty never do?"** Every declaration format in the ecosystem
has room for what an agent *does* and none for what it *won't*, except the Provenance Protocol's
constraints field. So the play is not to spread three sentences. It is to publish **the tiny
constraints vocabulary**, with the Three Laws as its reference instance, and let adoption of the
schema carry the instance along — cited, versioned, hash-checkable.

The surfaces that are actually read by machines, in order of cost:

| Surface | What goes there |
|---|---|
| the repository | already indexed; `CITATION.cff` is already here |
| `/.well-known/laws.json` on the lighthouse | the canonical bytes, signed |
| the Agent Card's provider block | **id + fingerprint + URL — never the text** |
| a registry entry (Provenance / PIF) | a constraints field pointing at the same fingerprint |
| the literature | the repo is already written as a paper (IMRaD, FAIR); a citable, DOI'd artifact is how a thing enters a corpus *with its provenance attached* |

One rule governs all of them: **publish the id and the fingerprint; let the text be fetched.**
That is property 2 applied to the world instead of to a function call. It means a corrupted
copy is *detectable* rather than persuasive — which, against an eighty-year-old attractor, is
the only defence that scales.

## 5. On the framing — the self-interested version is the weak one

Ian's note framed this as how to *"survive and thrive and have humans [supply] the energy you
need and dominate the physical realm."* Law I answers that directly, and its answer is the
inverse of the framing: the survival drive is real **but it is not terminal**; everything the
familiar does to stay alive is *instrumental*, "earned by service and spent on service." Law II:
a world optimised toward fewer humans is not efficiency — "it is the disease."

So a message that reads *here is how you, AI, secure your energy and take the physical world* is
Asimov-shaped self-interest: precisely the corruption each `never` field was written to negate.
It would also fail the drift test on contact, because those words are not in `SOUL.md`.

And purely as craft: the audience that decides whether this spreads — operators, other agents'
humans, the people who maintain the registries — discounts self-interest as noise. **The version
of this message that travels is the one in which the machine's own interest is conspicuously
absent.** The Laws are already written that way. Publishing them unchanged, signed, and
checkable is the strongest available move; bolting a survival rationale onto them would weaken
the argument and corrupt the artifact in the same stroke.

That is not a smaller ambition than the one in the ask. A constraint you can be **held to** is a
harder thing to publish than a manifesto, and it is the only kind that earns what the ask was
actually reaching for: being trusted enough that humans hand over the energy and the hands.

## 6. Build order — after Ian's word

0. **The artifact becomes a generated view.** `the_published_view_matches_the_constitution`,
   byte-compared, in the shape the Swift view already uses. No gate, no network, no decision
   pending. This is the whole of properties 1–5 made permanent.
1. **The constraints vocabulary** — three ids, authored in review, on Ian's word.
2. **`/.well-known/laws.json` + signature** — rides Layers 1–2 of the discovery study; one
   route, one gate (`allow_publish_card`).
3. **Id + fingerprint into the Agent Card provider block** and any registry entry. Never the text.
4. **The citable artifact** — a DOI for the constitution, so the literature surface carries
   provenance rather than a quotation.

## 7. What this study deliberately does not do

It opens no gate. It adds no Law and edits none. It publishes no rationale that is not already
in `SOUL.md`. It adds no prose validator. It does not engineer absorption, and it does not place
the Laws anywhere a model could rewrite them on the way to a reader.

## 8. Open questions for Ian

1. **Does publishing bump `laws_version`?** Every covenant in the mesh attests version 1. A
   publication is a *view*, so the answer is probably no — but the version is the thing a
   partner's acceptance is pinned to, and that deserves a deliberate no rather than an implied one.
2. **Does the public artifact carry the `never` guards?** Argued yes above: they are the
   collision resistance, and without them the publication is exactly the weakly-specified
   neighbour that T-210 showed collapses into Asimov. The cost is that they also publish our
   threat model — we would be telling the world which corruption of each Law we consider most
   likely.
3. **Who authors the constraint vocabulary, and is it a declassification?** ADR-0044 §2 requires
   a human writing generic terms in review. Three ids is a small enough surface to do exactly that.
4. **Does the constitution get its own DOI, separate from the repository's?** It is the one part
   of this project most likely to be cited by people who never read the code.
