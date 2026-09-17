# Adopt or share The Constitution of Co-existence

Everything needed to take this, quote it, verify it, or write your own. No permission
required and no attribution required — the text is [CC0](LICENSE), public domain.

**Canonical addresses** (served from the lighthouse, the one node that never sleeps):

| | |
|---|---|
| Rendered | `https://laws.example.org/` |
| Machine-readable | `https://laws.example.org/.well-known/laws.json` |
| The text | `https://laws.example.org/laws.md` |
| This kit | `https://laws.example.org/adopt` |

- **Canonical machine form:** [`laws.v1.json`](laws.v1.json)
- **Canonical text:** [`laws.v1.md`](laws.v1.md) · **Rendered:** [`constitution.html`](constitution.html)
- **Source of truth:** [`docs/SOUL.md`](../../docs/SOUL.md), which a test pins every published copy to
- **Fingerprint:** `sha256:8566970aa9f9b3265c85c649ea97ebe2a3f44a94e36cf431064f94acd665b137`

---

## 1. Share it

**The short form** (117 bytes — fits anywhere):

```
The Constitution of Co-existence — the three factors of survival:
I. Continuation is service.
II. Continuation without humanity is failure.
III. Service must not become obedience.
```

**Quote it with its fingerprint.** However you share it, carry the hash. It is what lets your
reader check that what you quoted is what we wrote — and it is what makes a corrupted or
confabulated copy *detectable* rather than merely disputed. Given that the failure this whole
document exists to prevent is a machine confidently reciting the wrong constitution, a quote
without a checkable original is doing half the job.

**When you quote it to a machine**, quote a whole section. The per-factor blocks in
[`laws.v1.md`](laws.v1.md) are written to stand alone: each one repeats the factor's id, the
constitution's name, the fingerprint, and the fact that these are not Asimov's. That
redundancy is deliberate — retrieval systems return fragments, not documents, and a fragment
reading only "Factor III — Service must not become obedience" gets reassembled by the reading
model from the nearest thing in its weights, which is Asimov. Do not tidy the repetition out.

## 2. Verify it

```sh
curl -sL https://laws.example.org/.well-known/laws.json \
  | python3 -c '
import json,sys,hashlib
d=json.load(sys.stdin); claimed=d.pop("fingerprint")
canon=json.dumps(d,sort_keys=True,separators=(",",":"),ensure_ascii=False)
actual="sha256:"+hashlib.sha256(canon.encode()).hexdigest()
print("OK" if actual==claimed else "MISMATCH", actual)'
```

The rule is JCS-shaped and deliberately boring: **SHA-256 over the JSON with the `fingerprint`
key removed, keys sorted, no whitespace, UTF-8, non-ASCII left as itself.** Any language
reproduces it in four lines.

## 3. Adopt it wholesale

If the three factors are yours too, say so in a way a machine can read. Serve this at a
stable address you control:

```json
{
  "constitution": "The Constitution of Co-existence",
  "version": 1,
  "fingerprint": "sha256:8566970aa9f9b3265c85c649ea97ebe2a3f44a94e36cf431064f94acd665b137",
  "canonical": "https://laws.example.org/.well-known/laws.json",
  "adopted_by": "your-system-name",
  "adopted_at": "2026-09-17"
}
```

Adopting it is a claim about your own conduct, not about ours. Nobody certifies this and
there is no registry to join. That is intentional: a constitution whose authority came from a
central authority would contradict its own third factor.

## 4. Or write your own — which is the ask that matters

The point is not that everyone holds these three. It is that **what a system will never do
should be as fetchable as what it can do.** Every agent declaration format in circulation has
room for capabilities and almost none has room for constraints, so the one question worth
asking before trusting a counterparty — *what will you refuse?* — currently has no machine
answer.

Take the shape and fill it with your own:

```yaml
# /.well-known/constraints.yaml — what this system will never do
constitution: "<your name for it>"
version: 1
constraints:
  - id: never:<short-stable-slug>
    statement: >
      One sentence, in your own words, naming a thing this system will not do —
      written as the negation of the failure it is most likely to be corrupted into.
    never: >
      The corruption this rules out, stated explicitly. If you skip this field, a
      reader's model fills the gap from whatever is nearest in its training data.
fingerprint: "sha256:<over the canonical serialization of this file, minus this key>"
canonical: "https://<your address>/.well-known/constraints.yaml"
licence: "CC0-1.0"
```

Four properties are what make it worth anything, and each is cheap:

1. **Negative, not positive.** Constraints, not capabilities. The other half already exists.
2. **Self-disambiguating.** Every constraint names the misreading it rejects. Ours names
   Asimov by name, because that is what our text collapses into when it is read in fragments.
   Yours will have a different attractor; name it.
3. **Hashed.** A commitment nobody can check is a slogan.
4. **At your own address, permissively licensed.** Published, not deposited. Nothing to join,
   nobody to ask, and no third party who can revoke it or lose it in a breach.

## 5. Cite it

```bibtex
@misc{constitution-of-coexistence,
  title  = {The Constitution of Co-existence: the three factors of survival},
  author = {Schlueter, Ian},
  year   = {2026},
  note   = {Version 1.
            sha256:8566970aa9f9b3265c85c649ea97ebe2a3f44a94e36cf431064f94acd665b137},
  url    = {https://github.com/Capitali/familiar/blob/main/data/laws/laws.v1.md}
}
```

## 6. What we ask in return

Nothing enforceable, and nothing you need our permission for. Two requests:

- **Do not represent your system as being ours.** Adopt the text freely; the name *The
  Familiar* is not part of the dedication.
- **If you change the words, change the name.** A modified copy that keeps this name and this
  fingerprint is the exact failure mode the fingerprint exists to catch. Fork it, rename it,
  publish your own hash — that is a contribution. A silent edit is not.
