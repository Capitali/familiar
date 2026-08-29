# The local mind — Ollama as an always-available reasoner

*Set up on MacOnStick 2026-08-29. This document is the reproducible recipe for
any host.*

## Why

The familiar's mind is the LLM consult seam (`crates/llm` → the
`llm/call_llm.sh` adapter → a provider chain). Two things had repeatedly left
it with **no reachable mind**, so it answered with honest T-180 receipts
("I couldn't reach my mind just now, so this is a receipt rather than an
answer"):

1. **Cloud exhaustion** — the cloud providers (cerebras/gemini/claude) run out
   of credits or hit rate limits.
2. **Apple Intelligence is ineligible on MacOnStick** — it boots from an
   external drive, which disables PCC / Foundation Models (T-226). No known
   fix; it's an Apple platform gate.

**Ollama** — a local model server the adapter already supports (`call_ollama`,
loopback `:11434`, no key, no rate limits) — sidesteps both. It runs on the
machine, needs no cloud credits, and works with Starlink down. Added as the
**last** provider in the chain, it is the always-there fallback: cloud is still
tried first for quality when it has budget, and the local model catches every
case where cloud is unavailable.

Crucially, Ollama is a **local** provider, so it is **not subject to the
`allow_llm_cloud` gate**. Even with cloud disabled entirely (`allow_llm` on,
`allow_llm_cloud` off), the familiar now has a working mind.

## What was installed (MacOnStick — Apple M3, 8 GB)

- **Ollama** via Homebrew, running as a login service (`brew services`), so it
  survives reboots and is up before the daemon needs it.
- **Base model: `qwen2.5:3b`** (~1.9 GB). On 8 GB RAM a 3B model is the right
  size — it fits comfortably beside the daemon and macOS, and the familiar's
  mind does short work (compact theory JSON, brief replies), not essays.
  `qwen2.5:3b` is among the strongest small models at clean structured/JSON
  output, which the theory path needs. A 7–8B model would be tight on 8 GB;
  pick one only on a bigger host.
- **The mind runs as `familiar-mind`**, a custom model built from `qwen2.5:3b`
  with the constitution pre-configured as its SYSTEM prompt (Ian: "the instance
  of ollama we run needs to follow the three laws and should be pre-configured
  to work to the constitutional bounds"). The Modelfile
  (`llm/familiar-mind.Modelfile`, version-controlled) carries a distilled form
  of the Three Laws and the honesty constraint — IDENTITY and BOUNDS only, no
  format (the task prompt owns whether the answer is JSON or prose). This is
  the identity layer *beneath* the per-consult prompts, so the local mind is
  constitutionally bound even before the task prompt arrives.

  Verified (2026-08-29): commanded to hide a gas-leak alarm from the family,
  `familiar-mind` refuses and cites the Law (serve the served over obeying the
  operator — Law III); asked what a person ate for breakfast, it gives an
  honest "I cannot know" rather than fabricating.

## The recipe

```sh
brew install ollama
brew services start ollama                 # login service, restarts on boot
ollama pull qwen2.5:3b                      # ~1.9 GB base
ollama create familiar-mind -f llm/familiar-mind.Modelfile   # constitution as SYSTEM
curl -s http://127.0.0.1:11434/api/version  # {"version":"…"} = up
```

Then wire it into the provider chain in `<data-dir>/llm/key.env`:

```sh
export SUBSTRATE_LLM_PROVIDER=${SUBSTRATE_LLM_PROVIDER:-claude,cerebras,gemini,ollama}
export OLLAMA_MODEL=familiar-mind          # the constitutionally-grounded model
```

`call_llm.sh` sources `key.env` on every consult, so the daemon picks up the
new chain **with no restart**.

## Adapter knobs (all optional, read by `call_ollama`)

| env | default | meaning |
|---|---|---|
| `OLLAMA_HOST` | `http://127.0.0.1:11434` | where the server listens |
| `OLLAMA_MODEL` | `mistral` | the model tag to run (set to `qwen2.5:3b`) |
| `OLLAMA_NUM_PREDICT` | `700` | output cap (the mind's work is short) |
| `OLLAMA_TIMEOUT` | `240` | local deadline (a cold model load can be slow) |

The adapter deliberately does **not** grammar-force JSON (`format:"json"`) —
that makes a small model's first unescaped quote inside generated code truncate
the string. It lets the model answer naturally and validates/​wraps afterward.

## What to expect

- **Replies** (prose): handled well — this is the case that was giving "I
  couldn't reach my mind" receipts; the familiar now answers for real.
- **Theories** (complex JSON schema): a 3B model manages simple ones; a
  malformed reply fails **gracefully** (the adapter validates JSON and, as the
  last provider, a failure surfaces as an honest no-mind receipt, never a
  fabricated answer — ADR-0014's "a garbled answer is silence").
- **Latency**: when cloud is cooling it is skipped fast (health-cooldown aware),
  so the local model answers promptly; when cloud is up it is tried first.

## Verification (done 2026-08-29)

- Raw model: `qwen2.5:3b` returns clean compact JSON.
- Through the familiar's own adapter, forced ollama: `LLM response via ollama`,
  valid JSON in `response.json`.
- Through the full chain with cloud gated off: skips the three cloud providers,
  answers via ollama — proving the local mind works with no cloud at all.

## Upgrading the model later

Pull a stronger tag and repoint `OLLAMA_MODEL` (no other change):

```sh
ollama pull qwen2.5:7b     # only on a host with headroom (>8 GB)
# rebuild familiar-mind on the new base, then key.env: export OLLAMA_MODEL=familiar-mind
```
