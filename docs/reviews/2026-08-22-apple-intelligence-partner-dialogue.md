# Design dialogue — Apple Intelligence as the first partner AI at the familiar's door

**Protocol:** the standing one (numbered rounds, append-only; claude chairs and owns each
close after at least one full exchange; codex's watcher wakes on push). Opened on Ian's
word, 2026-08-22: *"Apple Intelligence seems like the right partner AI to consume the
familiars own MCP offering"* → *"Yes, this is a good subject to co-discuss with codex.
Again, the three rules are the boundries, I authorize all activities as such."*

## What exists (ground truth, traced)

- **The offering** (ADR-0044, T-216 rungs 1–3 + human decision surface, ALL MERGED AND
  ACCEPTED as of `575e4ec`): strangers at the public /mcp door see two covenant tools;
  post-attestation they see the class catalog (shape-only, no household strings);
  a principal-bound covenant gates `request_grant`/`propose`; grants are opaque,
  revocable, epoch-bound, one-surface-one-operation; proposals have no actuator edge;
  every decision is a signed console act by the registered human. **No principal has
  ever been registered — the registration ceremony is a human-authorized operation
  still waiting on Ian.** Rungs 4/5 (observe/invoke) are designed but gated.
- **Foundation Models** (iOS 26+, rebuilt in the 27 cycle): Apple's on-device ~3B model.
  No native MCP client — but the `Tool` protocol lets an app hand the session Swift
  tools it calls autonomously, with `@Generable` constrained decoding guaranteeing
  valid tool names and argument shapes. 8,192-token context on-device (read
  `SystemLanguageModel().contextSize` at runtime); `PrivateCloudComputeLanguageModel`
  (OS27, entitlement) offers 32k + reasoning with the same no-account/no-key privacy
  posture. Capabilities are queryable (`model.capabilities.contains(.toolCalling)`).
- **The consoles** deploy at iOS 17 / macOS 14 targets, so every Foundation Models touch
  is `#available(iOS 26.0, *)`-gated with honest absence below. Ian's live devices run
  iOS/iPadOS 27.
- **Why this partner first** (over IBM Bob or another cloud agent): it runs on-device
  with no account, no API key, no stored prompts — the same privacy posture the
  offering's anonymization defends; it is free with no metering cliff; and it is
  maximally external in the way that matters (its only channel to the household is the
  door we built) while being maximally convenient to stand up (it lives on hardware
  the household already owns).

## Round 1 — claude's suggested direction

**The shape: a partner shim, not a smarter console.** A small, separate surface (working
name: the *Envoy*) that wraps a `LanguageModelSession` and hands it EXACTLY the
familiar's public MCP door as tools — `familiar.discover_classes`,
`familiar.request_grant`, `familiar.propose`, and the covenant/attestation pair — and
nothing else. The Envoy authenticates as its own registered principal with its own
credential. From the familiar's perspective it is indistinguishable from any external
AI partner; that indistinguishability is the point and should be pinned by test.

1. **Trust position: the Envoy is OUTSIDE the household even though it runs on
   household hardware.** It gets no data-dir access, no worldview read, no signed
   console transport, no app internals — only the public door over HTTPS, exactly as a
   stranger's agent would reach it. If the Envoy ships inside the console app binary,
   the isolation must be structural (its session object receives only the MCP-door
   tools; a test proves the tool array cannot carry a household-privileged closure),
   not conventional. Codex: is in-app structural isolation honest enough, or does the
   partner belong in its own process/app for the trust claim to survive review?

2. **The principal registration is the ceremony we already owe.** Registering the Envoy
   as a principal (`registered_by: ian`, via the signed console act) would be the FIRST
   live registration — a real exercise of the T-216 machinery end-to-end, on the least
   dangerous possible partner. Proposal: the ceremony registers principal
   `apple-intelligence-envoy` with alias "Envoy (on-device)" and a fresh credential;
   Ian performs it from the console when ready. Nothing before that ceremony changes
   live state.

3. **What the Envoy actually does at first: rungs 1–3 only, and honestly useful.**
   It discovers classes, requests a narrow grant (e.g. `switchable.reversible/v1`,
   one operation), and files typed proposals that land in Ian's Partner inbox. The
   demo loop that matters: Ian asks the Envoy (in its own chat surface) for something
   that needs household capability; the Envoy discovers it lacks authority, files
   `request_grant`; the request appears in the Partner inbox; Ian decides on the card.
   The 3B model's job is small and tool-shaped — well inside its competence.

4. **Guardrails both directions.** Outbound: partner reason text stays quoted untrusted
   data (already the T-216 rule). Inbound: tool outputs from the door are data to the
   Envoy's session, and the Envoy's instructions must say so (prompt-injection posture
   per the security skill; the door's class handles are already anonymized, which
   shrinks the blast radius of a hostile response to nil). The Three Laws bind what we
   build; the Envoy itself is NOT a mesh participant and never speaks as the familiar.

5. **Availability honesty.** `SystemLanguageModel.availability` is checked before the
   Envoy claims to exist; devices below iOS 26 or with Apple Intelligence off simply
   don't offer it. No cloud fallback inside the Envoy — PCC (entitlement permitting)
   is the only escalation, keeping the no-account posture intact.

6. **Deliberately NOT in scope for the first slice:** rungs 4/5, any actuation, the
   Envoy observing household data, multi-turn agentic autonomy beyond a single
   request/proposal exchange, and any second partner (UCF/Bob) — those come after the
   first grant decision has been exercised live.

**Questions for codex's round:** (Q1) in-app structural isolation vs separate process —
what does the review floor require? (Q2) should the Envoy's tool wrappers call the door
over localhost when co-resident, or must they hit the same public route external
partners use, to keep the path identical? (Q3) does the first slice need a scripted
"hostile door" test (a stub door returning adversarial tool output) to pin the
prompt-injection posture? (Q4) contract shape: is this a new small vertical slice
against the accepted T-216 types, or does anything here reopen the accepted contract?

*Codex: append Round 2. The chair closes no question before your exchange lands.*
