# ADR-0014 — The device oracle: Apple Intelligence as the mesh's paid-for mind

- **Status:** accepted (implemented + validated — a device answered a queued consult via Apple Intelligence, 2026-07-27)
- **Date:** 2026-07-25
- **Relates to:** [ADR-0009](0009-sovereign-mesh-transport.md) (the signed device
  seams this extends), [ADR-0011](0011-scenario-engine.md) (the laboratory that
  will judge the results), [ADR-0013](0013-outreach-seam.md) (evidence quality —
  better consults make better-cited claims), `docs/agents.md`, the A9 campaign
  (`tools/campaigns/a9.json` — currently starved for a funded provider)

## Context

The muse and the laboratory are inference-starved: hosted providers are unfunded
or rate-limited (Anthropic $0, gemini/cerebras 429), and wildhorse is a 2019
Intel MBP whose CPU-only ollama manages ~6s/consult on a 3B model. Meanwhile the
fleet's own devices — Aphelion (iPhone 16 Pro Max) and Codex (M5 iPad Pro) — are
first-class Apple Intelligence hardware whose inference is already paid for.
iOS 26's `FoundationModels` framework (in wildhorse's Xcode 26.5 SDK) exposes
`SystemLanguageModel` with guided generation to apps; iOS 27 adds a larger
model, on-device fine-tuning, expanded context, and full tool calling; the App
Store Small Business tier adds Private Cloud Compute **at no cloud API cost**.

The shape is already ours: devices are covenant members with signed seams
(worldview reads, observe pushes). A consult is one more duty, not a new trust
relationship.

## Decision

The enrolled device becomes the mesh's **oracle** — a member that answers
consults, on its own silicon, inside the covenant.

1. **Consult queue (Rust, mesh crate).** On-disk queue `data/llm/device-queue/`
   (`<id>.prompt.json` → `<id>.answer.json`). Two endpoints on the mesh port,
   cribbed from the existing device paths:
   - `POST /mesh/consult` — a member device pulls pending prompts. Signed +
     membership-bearing + replay-protected, exactly like a worldview read.
   - `POST /mesh/consult-answer` — the device pushes `{id, json}` results,
     verified like an observe batch.

2. **Device oracle (Swift, FamiliarAgent).** On the existing `BackgroundSync`
   cadence: pull consults, run `SystemLanguageModel` with **guided generation**
   (its structured output is a natural fit for the muse's compact-JSON
   contract), push answers. Availability-checked
   (`SystemLanguageModel.default.availability`) and surfaced in the console
   when Apple Intelligence is off or the model isn't present. Fleet build 24.

3. **Adapter (`apple` provider in the call_llm.sh chain).** Enqueue → wait
   bounded (poll for the answer file) → exit 2 (RateLimited) on silence. The
   muse's existing retry/pause semantics absorb device sleep for free; the
   lab's `llm_unavailable` accounting stays honest for free.

4. **Judged in the laboratory.** Scenario-lab cells with
   `SUBSTRATE_LLM_PROVIDER=apple` vs `ollama` (same seeds, same worlds);
   campaign evidence tables + `theory_quality` give the A/B on real theories.
   Then the A9 campaign runs on the device pathway at $0.

**Privacy line.** Consult prompts carry muse material — observations about the
served. Sending them to an enrolled, covenant-signed device where the model
runs **on-device** is consistent with the worldview flow the human already
authorized. Private Cloud Compute routes prompts to Apple's servers: that is a
**separate, explicit consent decision** (its own gate or config flag, default
off) — not bundled into this ADR.

**Constraints, named.** iOS 27-only API (larger model, free PCC) needs the
Xcode 27 SDK, which likely ends Intel-Mac support — the Apple Silicon build box
(already the recommended hardware step) becomes the path. Until then the iOS 26
SDK surface is enough to ship all four pieces. Devices answer only when awake
and charged enough — the adapter's exit-2 semantics make that a pause, never a
contamination.

## Status history

- 2026-07-25 — proposed; design settled in-session (fleet verified capable:
  iPhone 16 Pro Max + M5 iPad Pro; Xcode 26.5 SDK has FoundationModels), build
  deferred to a fresh session by Ian's call.
