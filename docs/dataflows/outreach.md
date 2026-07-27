# Outreach

How the familiar speaks to **non-members** — a stranger service, another AI it might
recruit, a device with no steward. Reading a stranger's public surface is perception and
needs only `allow_network`; an *utterance* — anything that could change a stranger's
state — is a sharper act, gated by `allow_outreach` and never sent without the human's
explicit yes.

Related: [ADR-0013](../decision-records/0013-outreach-seam.md), `crates/mesh/src/outreach.rs`.

```mermaid
sequenceDiagram
    autonumber
    participant M as Muse / familiar
    participant O as Outreach seam
    participant Hu as Human
    participant X as Non-member (stranger service / AI)
    M->>X: read public surface (perception · allow_network)
    M->>O: draft an utterance to X<br/>(a pitch, a prediction, covenant terms)
    Note over O: citation check — every claim must trace to<br/>something actually observed; no invention
    O->>Hu: queue for approval (the contact ledger)
    Note over Hu: outreach approve &lt;id&gt; — the only way a covenant sends
    Hu-->>O: yes
    O->>X: send the utterance (allow_outreach open)
    Note over O: log the exchange · track the relationship<br/>(evidence-first: credited vs ignored over time)
```

## Primitives

| Primitive | What it is |
|---|---|
| **Read vs speak** | Perception (reading public pages) is ordinary reach; an utterance that could move a stranger is a separate, sharper gate — the familiar can look long before it can speak. |
| **Citation-checked** | An utterance's claims must each trace to a real observation; the seam refuses to send invented reasoning. |
| **Human's yes** | Nothing goes out without `outreach approve` — the covenant seam is drafted by the machine and *sent* only by the human. |
| **Contact ledger** | Every attempt and reply is logged; the relationship is tracked evidence-first (credited predictions earn its ear, burns cost it). |
| **Law-III shaped** | Recruit / steward / awaken postures all keep consent, honesty, and human-gated admission — a stranger is invited under the Three Laws, never coerced. |

Outreach is Phase-1 (utterance seam) today; heater-stewardship and adoption postures are
later phases against the testworld counterparties.
