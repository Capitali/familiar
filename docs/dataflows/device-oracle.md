# Device oracle / consult

How the mesh borrows a device's **on-device Apple Intelligence** as its reasoning engine
when hosted providers are unfunded or rate-limited. The fleet's own iPhone/iPad already
pay for that inference; the consult seam lets the familiar ask it a question and get a
structured answer back — nothing leaves the device except the answer.

Related: [ADR-0014](../decision-records/0014-device-oracle.md) (design of record).
Status: **shipped + validated** (smoke test: a device answered a queued consult via Apple Intelligence).

```mermaid
sequenceDiagram
    autonumber
    participant Mu as Muse / lab
    participant F as Familiar (queue)
    participant D as Device (Apple Intelligence)
    Mu->>F: needs a consult (theory / direction / prose)
    Note over F: write prompt into llm/device-queue/<id>.prompt.json
    D->>F: POST /mesh/consult (signed) — pull pending prompts
    F-->>D: { id, prompt }
    Note over D: SystemLanguageModel guided generation<br/>(@Generable struct matching the muse's contract)<br/>on-device · Private Cloud off by default
    D->>F: POST /mesh/consult-answer (signed) — push result
    Note over F: write <id>.answer.json · the apple adapter<br/>emits it to the muse; silence → exit 2 (retry later)
    F-->>Mu: answer (or llm_unavailable — never contaminated)
```

## Primitives

| Primitive | What it is |
|---|---|
| **A signed queue** | Prompt files in / answer files out under the data dir; the device pulls and pushes over the same signed, membership-bearing seam as observations — no new trust. |
| **Guided generation** | The device runs `SystemLanguageModel` with a `@Generable` struct that matches the muse's `{question, theory, direction}` contract, so the answer is structured, not free text to parse. |
| **On-device by default** | Inference stays on the device; Private Cloud Compute is a separate consent decision, default off. |
| **Fails clean** | A sleeping device is silence, not garbage — the adapter exits `RateLimited`, the lab records `llm_unavailable` and pauses, and template output never contaminates evidence. |
| **Opaque answers** | The seam moves an opaque JSON string; the adapter (not the transport) judges the content. |

This feeds the theory pipeline of [The cognitive cycle](the-cognitive-cycle.md); it's an
LLM *provider*, swappable with the hosted or local-model adapters.
