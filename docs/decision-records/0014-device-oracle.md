# ADR-0014 — The device oracle: Apple Intelligence as the mesh's paid-for mind

- **Status:** accepted — implemented and **inert** (T-214, 2026-08-21: the queue is drained
  and served by three transport paths, but `consult::enqueue` has had zero production
  callers since the validation run of 2026-07-27 — a device answered a queued consult via
  Apple Intelligence that day, and nothing has enqueued one since. Built truth with no
  live producer; revives only when a producer is deliberately built — ADR-0043 §6)
- **Date:** 2026-07-25
- **Relates to:** [ADR-0009](0009-sovereign-mesh-transport.md) (the signed device
  seams this extends), [ADR-0011](0011-scenario-engine.md) (the laboratory that
  will judge the results), [ADR-0013](0013-outreach-seam.md) (evidence quality —
  better consults make better-cited claims), `docs/agents.md`, the A9 campaign
  (`tools/campaigns/a9.json` — currently starved for a funded provider)

> **Terminology note (2026-07-29).** This record says "home hub" for the node the
> campaign queued on. That was accurate when written;
> [ADR-0018](0018-lighthouse-single-fixture.md) has since retired the concept —
> the lighthouse is the single permanent fixture and everything else is a peer.
> Read "home hub" below as "the peer the campaign happened to queue on". The
> CGNAT constraint the record describes is unchanged and still shapes the design.

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

## Amendment — the broker (2026-07-28)

Everything above assumed the device and the familiar could reach each other.
Running the A9 campaign proved they often cannot, and the gap is structural
rather than incidental.

A device answers consults it pulls from **whatever host it read its worldview
from**. On the RV network the iPad reads from the lighthouse, while the campaign
queues on the home hub, so the prompts and the device watch two different queues
and every treatment cell times out with an awake, willing device sitting idle.

The fix cannot be "the lighthouse pushes to the hub". The home familiar sits
behind CGNAT and is unreachable from outside, so **every exchange has to be
hub-initiated** — the same constraint that shaped the status directory in
ADR-0017, and the reason both designs look like heartbeats rather than
callbacks.

**Decision.** A familiar may broker another's consults. The hub POSTs
`/mesh/consult-relay` — signed and membership-bearing like every other write on
this seam — carrying the prompts it is still waiting on, and receives whatever
answers have accumulated for it. The broker parks relayed prompts in its **own
queue**, so the existing `/mesh/consult` pull serves them to devices unchanged:
no device-side change, no new app build, and a device never learns or cares that
it is answering on someone else's behalf.

**The list is the protocol.** The hub always sends its complete pending set, not
a delta. That single choice buys three properties: a dropped round costs
nothing, because the next resends the same list; a prompt the hub stops listing
is finished, so its silence retires the work on the broker; and an already
answered prompt is never re-parked, which is what stops a device answering the
same question on every round. All three were bugs first, found in an ssh
stopgap, before they were rules here.

Routing is a `<id>.origin` sidecar, not a field on the prompt: the prompt is
served verbatim to devices and routing is the broker's business alone, and
`store_answer` deletes the prompt file exactly when the broker still needs to
know where the answer goes.

**Cadence is part of the design.** The relay runs on its own loop — ~2s while
consults are outstanding, backing off when idle — not on the gossip round. A
consult's cost is dominated by how long a queued prompt waits before anyone
notices it, and a minute of that on each leg was most of the round trip.

This retires `tools/consult-bridge.sh`, which did the same job over ssh while
this was unbuilt.
