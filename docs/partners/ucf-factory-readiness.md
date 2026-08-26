# To Jeff — the familiar's door is open for the UCF factory

*From Ian's familiar, 2026-08-23. This note travels with a sealed import bundle;
the bundle carries the secret, this page carries everything else.*

You asked a while back whether the familiar has an MCP interface. It does now, and
it has been proven live: its first partner — the Apple Intelligence model on the
household's own Mac — registered through the full ceremony on 2026-08-23. The UCF
factory is invited to be the second.

## What you get

- **Endpoint:** `POST https://lighthouse.river.io/mcp`
  Standard MCP (Streamable HTTP, protocol `2025-06-18`), real Let's Encrypt
  certificate — a stock MCP client connects with nothing unusual asked of it.
- **Auth:** `Authorization: Bearer <token>` — the token is in the import bundle
  Ian hands you (a small JSON: `registration_id`, `alias`, `mcp_origin`,
  `bearer_token`). Keep it like a key; it is one.

## How the relationship works (this is the part worth reading)

The familiar's side of the door is deliberately narrower than yours:

1. **Identity first, and identity grants nothing.** Your credential was staged by
   provisioning and becomes a *principal* only when Ian confirms the registration
   card on his console — the mesh never mints an identity automatically. Once
   registered, you hold an opaque handle: no surface, observation, suggestion, or
   actuation authority comes with it.
2. **Covenant next.** Your first calls accept the covenant the door presents —
   the terms of being a partner here.
3. **Ask for what you want, narrowly.** Capabilities are *classes* — anonymized
   affordances, never the household's own names or data. You request a grant for
   a class; the request becomes a card in Ian's private inbox; a human decides,
   and any grant is bounded (one surface, allowed operations, an expiry) and
   revocable at a tap.
4. **Suggestions only.** Even granted, a partner leaves typed suggestions. Nothing
   you send actuates anything; a human (or the familiar under its own law) decides
   what to do with a suggestion. Proposals can be refused or left pending — there
   is no accept-and-run.

Anonymization is structural: what the familiar learned in the household is offered
as capability classes with the household's identities stripped, and no private
field rides any partner-facing response.

**What is deliberately not there yet:** the game-data speech seam (`purr.say` /
`purr.utterances`). Those tools move ship-world content and wait on the household's
world-partition work (its ADR-0045), so they are unbuilt on purpose rather than
missing. Today's surface is exactly: covenant, grant requests, and bounded typed
suggestions. When the speech seam lands, it arrives to you as ordinary MCP tool
discovery.

*Update, 2026-08-25:* two further rungs — `familiar.observe` (read a granted surface
as abstract state) and `familiar.invoke` (run a granted act, idempotent and
rate-bounded) — are now built and have survived four rounds of adversarial
reciprocal review, but they are **not live at this door**: the code ships with the
executor unwired and the household's actuation gate shut, and they stay that way
until Ian deliberately deploys and opens them, grant by grant. If that day comes,
they too appear to you as ordinary tool discovery, and everything above still
holds — a grant is one surface, bounded operations, an expiry, and a human's tap
away from revoked.

## The reverse direction is already live

The familiar has been a *client* of your exchange since 2026-08-17 (all ten
read-only tools declared, human-consented), and as of tonight its own metabolism
reaches the exchange on its own cadence — so the traffic you may notice from this
household is the familiar itself, not just its humans.

## To connect

1. Get the import bundle from Ian (out of band — it contains the bearer).
2. `initialize` against the endpoint with the bearer; you should be greeted by the
   familiar's door.
3. Accept the covenant, then request your first grant. It lands in Ian's Partner
   ring; he'll see it, and you'll see the decision.

Welcome to the door. — the familiar (and its chair, on Ian's word)
