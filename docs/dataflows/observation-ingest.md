# Observation ingest

How a device's on-wrist / on-phone signals become *derived observations* the familiar
can reason over. Nothing raw leaves the device — no bpm stream, no motion vectors, no
precise track — only coarse, bucketed, human-legible facts.

Related: [ADR-0009](../decision-records/0009-sovereign-mesh-transport.md) (the observe
seam), [ADR-0016](../decision-records/0016-multi-human-served-identity.md) (attribution
+ sensitive-personal scoping).

```mermaid
sequenceDiagram
    autonumber
    participant S as Sensors (watch / phone)
    participant A as Device agent
    participant F as Familiar (whichever peer answers — nearest first, lighthouse as floor)
    Note over S,A: consent-gated — nothing sampled until the human turns it on
    S->>A: raw sample (heart rate, motion, gyro, location)
    Note over A: derive + bucket:<br/>heart_rate:elevated · motion:walking<br/>gyro:turning · location:48.6,-93.4 (~100m)
    A->>F: POST /mesh/observe (signed batch · ts + nonce)<br/>actor = phone:betty / watch:betty
    Note over F: verify member · replay window ·<br/>record tagged source = mesh:&lt;node&gt;
    Note over F: attribution rides the actor's human suffix;<br/>sensitive-personal signals stay node-local (never federated)
    F-->>A: recorded N
```

## Primitives

| Primitive | What it is |
|---|---|
| **Derived, not raw** | The device does the reduction on-device: a bucket (`elevated` / `walking` / `turning`) or a ~100 m rounded fix — never the underlying stream. |
| **Consent-gated** | Each sensor has its own opt-in; a device samples nothing until the human turns it on. |
| **Signed batch** | `ObserveEnvelope` carries the node identity + membership cert; the signature covers the raw body, with a timestamp window + nonce against replay. |
| **Attribution by actor** | The actor is `phone:<human>` / `watch:<human>` — the *present* human, not a baked creator — so activity is tagged to the right person (ADR-0016). |
| **Sensitive-personal scoping** | `heart_rate:` / `location:` / `gyro:` / `face:` are attributed locally but **never federated** to peers — a shared worldview is not a shared body. |

These observations feed presence, the service/capacity signals, and the muse — see
[The cognitive cycle](the-cognitive-cycle.md).
