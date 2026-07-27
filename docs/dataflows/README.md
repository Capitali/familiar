# Dataflows

One page per part of the app, each in the same shape: a short framing, one or two
**mermaid** sequence diagrams (they render on GitHub), and the primitives that make the
flow work. The through-line across all of them: **the device transmits proofs, not
secrets; status flows through the always-on lighthouse; data takes the best confirmed
path.**

| Part | What it traces | Status |
|---|---|---|
| [Authentication & mesh membership](auth-and-membership.md) | Identity, the covenant, the membership cert, per-request verification | ✅ |
| [Finding & joining a mesh](finding-and-joining.md) | Rendezvous → the door → auto-enroll → grant (no QR) | ✅ |
| [Worldview read](worldview-read.md) | A client reading the mesh's live state for its console | ✅ |
| [Observation ingest](observation-ingest.md) | Device sensing → derived observations → the familiar's store | ✅ |
| [Gossip & federation](gossip-federation.md) | Peer briefs exchange tools / patterns / observations; the mesh converges | ✅ |
| [Status & connectivity](status-connectivity.md) | Heartbeat → lighthouse → pull → roster; Tailscale probe / switch / fallback | ✅ |
| [The cognitive cycle](the-cognitive-cycle.md) | Sense → theorize → test → learn, always inside the boundary | ✅ |
| The capability boundary | How a capability request is checked against the human-owned gate | ○ planned |
| Served identity & attribution | Device → present human → attribution + sensitive-personal scoping | ○ planned |
| Outreach | Speaking to non-members over the covenant seam | ○ planned |
| Device oracle / consult | The on-device LLM consult pathway | ○ planned |
| The console (Glass) | UI ↔ daemon: render worldview, send actions / consent | ○ planned |

Each page links to the decision records it implements
([`../decision-records/`](../decision-records/)).
