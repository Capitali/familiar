# ADR-0017 — Federated status & connectivity: status via the lighthouse, data via the best path

- **Status:** accepted (building — Phase A landed)
- **Date:** 2026-07-27
- **Relates to:** [ADR-0012](0012-lighthouse-rendezvous.md) (the rendezvous directory this extends),
  [ADR-0009](0009-sovereign-mesh-transport.md) (covenant transport), the Tailscale peer enumeration
  already in `crates/mesh/src/transport.rs`

## Context

With the lighthouse as the primary door (ADR-0012), enrolled devices also *read their worldview*
from the lighthouse, so only the lighthouse knew they were alive — the home node's roster went stale
("away 6h") because a member's `last_seen` only updates on the node it actually contacts. Brief
signatures re-serialize the body (`verify_brief`), so federating liveness as a new brief field would
break every older peer's signature check. Device liveness needs a different channel.

## Decision (Ian's model)

**Status flows through the always-on lighthouse; data flows over the best confirmed path (Tailscale
when it works).** Concretely:

1. **Join** → client reaches the lighthouse and auto-joins (ADR-0012, built).
2. **Hydrate** → on join, pull the other members' metadata + connectivity from the lighthouse.
3. **Every member heartbeats its status** to the lighthouse — online, present-human, connectivity
   mode, and (later) its tailnet address + Tailscale availability.
4. **Prefer Tailscale when proven** → a member probes a direct tailnet path to a peer; if the probe
   passes, data routes peer-to-peer over Tailscale.
5. **Always keep heartbeating status to the lighthouse**, even on Tailscale — so non-Tailscale
   members still see it, *and see that it's on Tailscale*. The lighthouse is the complete, always-
   fresh status authority; no member is ever invisible to another, whatever path each is on.
6. **Fallback** → if Tailscale is disabled or a probe stops passing, the member falls back to
   lighthouse federation (as at first connect) and updates its reported mode. macOS, iOS, iPadOS,
   watchOS alike.
7. **Badge** → the roster shows a via-Tailscale / via-lighthouse / local badge per member.

This makes the lighthouse the **status authority** (presence, who's-present, connectivity) — the
right role for the always-on public node, and what makes non-Tailscale visibility work. Data
(worldview reads, gossip, observations) still takes the best path; only *status* is centralized.
Presence data therefore lives on the same VPS that holds the group key — accepted; status is not in
the sensitive-personal set that ADR-0016 keeps node-local.

## Phases

- **A — Status hub (landed).** `crates/mesh/src/status.rs`: `MemberStatus` (with connectivity fields
  schema-forward) + a per-member directory with a 5-min TTL, signed like the rendezvous. Endpoints
  `POST /mesh/status` (heartbeat, own-node-only) and `GET /mesh/status` (the live directory, which
  also surfaces a node's own fresh peers so a device is visible before it heartbeats explicitly).
  Every daemon heartbeats + pulls each gossip round (`heartbeat_status`/`pull_status`), bumping known
  peers' `last_seen` forward from the directory (`apply_status_freshness`). Fixes the "away" bug.
- **B — Connectivity + badge.** Heartbeat carries mode + tailnet addr + availability (iOS/watch
  `StatusClient`); the sphere roster renders the connectivity badge.
- **C — Tailscale probe/switch/fallback.** Probe a peer's `100.64/10` path (`/mesh/hello`), switch
  data to it when it passes, report `mode=tailscale`; monitor and fall back to the lighthouse when
  Tailscale drops — on every client platform.

## Consequences

- No brief-schema change → older peers (FamTalker) keep verifying; the status channel is separate
  from the signed gossip brief.
- The directory is soft state: a member that stops heartbeating expires in 5 min.
- A member may only place *its own* status (node_id must match the membership) — no spoofing another.
