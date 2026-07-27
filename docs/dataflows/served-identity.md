# Served identity & attribution

A node serves *many* humans, not one creator, and devices are shared and change hands.
So activity is attributed to whoever is actually present — never a baked "creator" — and
a shared worldview never becomes a shared body.

Related: [ADR-0016](../decision-records/0016-multi-human-served-identity.md),
`crates/kernel/src/identity.rs`.

```mermaid
flowchart TD
    Who["Who is present?"]
    Face["facial recognition<br/>(when available — future)"]
    Pick["a Device-menu pick<br/>(SERVING = betty)"]
    Def["the enrolled default<br/>(else 'observer')"]
    Served["servedHuman handle"]
    Actor["device actor = phone:betty / watch:betty"]
    Report["reports + answers tagged to that human"]
    Sens{"sensitive-personal?<br/>heart_rate · location · gyro · face"}
    Local["stays node-local — never federated,<br/>never shown as another human's data"]
    Shared["ordinary activity — shared worldview,<br/>attributed per human"]

    Who --> Face --> Served
    Who --> Pick --> Served
    Who --> Def --> Served
    Served --> Actor --> Report --> Sens
    Sens -- yes --> Local
    Sens -- no --> Shared
```

## Primitives

| Primitive | What it is |
|---|---|
| **The identity registry** | An append-only record of everyone the familiar has come to know — handle, name, relation, interactions, and (strongly-gated) a face signature. Names are quality data; a known name is never discarded. |
| **Present, not baked** | The device actor's human suffix comes from `servedHuman`, established by face → manual pick → enrolled default. It defaults to `observer`, never a hard-coded person. |
| **Attribution by actor** | The whole system reads the actor suffix as the human, so a shared iPad's activity is tagged to the person using it — the roster shows who's present and how it was established. |
| **Sensitive-personal scoping** | Health / precise position / biometric are attributed locally but never leave the node; a face signature never federates under any sharing setting. |
| **One shared worldview** | The mesh is a shared worldview *with per-human attribution* — everyone's ordinary activity is visible, tagged by who; only the personal body is held back. |

The attribution rides through [Observation ingest](observation-ingest.md); the redaction
happens on the way out in [Gossip & federation](gossip-federation.md).
