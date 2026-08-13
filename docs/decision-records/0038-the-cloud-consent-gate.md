# ADR-0038 — The cloud consent gate: off-device consult is its own boundary

- **Status:** accepted — implemented 2026-08-13
- **Relates to:** `docs/SOUL.md` ("permission does not compose"),
  [ADR-0005](0005-capability-boundary.md) (the boundary this extends),
  [ADR-0014](0014-device-oracle.md) (whose deferral — *"PCC is a separate, explicit
  consent decision, its own gate, default off"* — this resolves),
  [ADR-0037](0037-one-soul-many-voices.md) (proposed: the in-process provider that
  will call the same gate), `docs/boundaries.md`, README's "a prompt need never
  leave your hardware".

## Context

`allow_llm` gates whether the familiar may consult at all; **where** the prompt goes has
been provider config (`SUBSTRATE_LLM_PROVIDER`), not policy. `guard.rs` names this gap
honestly: the guard enforces per-capability gates but does not confine data-flow within a
granted one. So README's "a prompt need never leave your hardware" was a property of how
one machine happened to be configured — true until someone edits an env file, enforced by
nothing. OS 27 sharpens the question with two new off-device paths on Apple platforms
(Private Cloud Compute from FoundationModels, and from the macOS `fm` CLI) and one new
on-device path (`fm --model system`) — a provider catalog where "local" and "cloud" no
longer track vendor names.

## Decision

1. **One gate, class-of-act:** `Boundary.allow_llm_cloud` (serde-default **false**;
   fail-closed; pre-existing boundary files stay closed). It governs every consult that
   leaves hardware the covenant controls. **Local** — needing only `allow_llm`: `ollama`
   (loopback), `apple` (the device oracle: an enrolled covenant device answering
   on-device, ADR-0014), `apple_local` (`fm --model system` on the host's own silicon).
   **Cloud** — additionally needing this gate: `claude`/`anthropic`, `gemini`,
   `cerebras`, `apple_pcc`, and PCC chosen by a device honoring a prompt's `cloud_ok`.
2. **Enforcement, three layers.** The kernel guard gains `ActionKind::LlmCloud` (the
   canonical gate any in-process provider must call — ADR-0037's seam included). The
   Rust seam exports `FAMILIAR_ALLOW_LLM_CLOUD=0|1` into the adapter's environment,
   unconditionally both ways, so a stale inherited value can never lie; the adapter
   filters the provider chain by it (unset = `0`, fail-closed for humans running the
   script by hand). The boundary decision travels the mesh as `ConsultPrompt.cloud_ok`
   so an answering device honors the hub's boundary, not its own convenience.
3. **Consents stack on the device.** A device chooses PCC only when the prompt says
   `cloud_ok` AND its own local consent toggle is on AND the OS reports the model
   available. Permission does not compose; absence of any one degrades to on-device.
4. **Degradation is always toward the device.** Old peer, old broker, missing
   entitlement, PCC outage, closed gate — every failure lands on the on-device model or
   on silence, never on a wider path.

## Consequences

**Good.** The README's privacy line becomes enforced mechanism, not configuration
folklore. A future provider slots into a *class* (local or cloud), not a policy
renegotiation. ADR-0014's deferred consent question has its answer, in the constitution
where it belonged.

**Bad, and accepted.**
- **The named behavior change:** a deployment with `allow_llm: true` and a hosted chain
  stops consulting cloud until its human writes `"allow_llm_cloud": true` — one line of
  boundary.json, flipped deliberately at deploy on the nodes that want hosted providers.
  Fail-closed in the right direction, and Ian chose it knowingly over a PCC-only flag.
- The gate is still capability-level: nothing yet inspects *what* a cloud-bound prompt
  contains. Content-aware routing (what may go where, by sensitivity) remains the named,
  unbuilt hardening in `boundaries.md`.

## Rejected

- **`allow_pcc` (vendor-scoped):** leaves hosted APIs unenforced and forces the general
  flag anyway at the next cloud provider; wrong altitude for a constitutional gate.
- **Enforcing provider choice in Rust:** provider selection deliberately lives in the
  human-editable adapter (`llm/call_llm.sh`); the seam exporting the boundary preserves
  that seam instead of hollowing it.
- **`fm serve` as the integration:** a second long-lived process to manage;
  subprocess-per-consult matches the adapter's stateless temperament at the muse's cadence.
