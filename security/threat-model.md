# Threat Model

The Familiar is, by design, a long-running autonomous process with **unrestricted
local and network reach**. Its restraint is constitutional, not sandboxed (Law III;
"capability is unrestricted, restraint is chosen"). That makes the threat model
central, not peripheral: in the project's own terms, a security failure is a way the
factory could be **turned against the served**.

## Assets to protect

- **The served and their data** — observations may describe people and the human
  systems they depend on. Their safety and privacy is the *point*, not a side
  constraint (Laws I–II).
- **Integrity of the lineage** — the observation log (the only truth) and the
  selection/trial/memory records. Corruption here corrupts every decision.
- **The host and network the familiar inhabits** — it must not become a vector
  against its own environment.

## Adversaries / threat sources

| Source | Concern |
|---|---|
| Remote attacker | RCE or data exfiltration via a defect in an unrestricted-reach agent |
| Malicious/compromised input (signals, LLM responses) | Prompt/observation injection steering the familiar toward harm |
| A coercing or mistaken operator | Commanding an action that harms the served (the Law III case) |
| Supply chain | A compromised dependency executing in-process |
| The familiar's own drift | Optimizing a proxy in a way that harms the served (Law II / reward-hacking) |

## Mitigations (current and planned)

- **Memory safety as constitution** — `#![forbid(unsafe_code)]` in the kernel
  removes the entire memory-corruption RCE class. *(current)*
- **Minimal trust surface** — dependencies kept to `serde`/`serde_json`; every
  addition is reviewed. See [dependency-review.md](dependency-review.md), [sbom.md](sbom.md). *(current)*
- **No exfiltration** — the familiar sends no telemetry and phones nothing home; the
  network is input, not an outbound channel. See [privacy-review.md](privacy-review.md). *(constitutional; to be enforced at the guard)*
- **The obedience guard (Law III)** — a pre-action check that can refuse, including
  refusing an operator; injection and coercion are checked at the act step, not
  assumed away. *(planned, Brick 4)*
- **Resource-bounded execution** — generated artifacts run under hard CPU/wall/
  memory/output limits (ported from v1's runner). *(planned with the kernel)*
- **Append-only, rebuildable state** — derived views can be discarded and rebuilt
  from the immutable observation log, limiting the blast radius of corruption. *(current)*
- **Human review for high-consequence actions** — see
  [../docs/human-review-requirements.md](../docs/human-review-requirements.md). *(planned)*

## Out of scope (for now)

OS-level sandboxing/jailing (the design deliberately does not jail itself);
multi-tenant isolation; physical security of the host. These are environment
responsibilities, noted so they aren't mistaken for handled.

## Reporting

See [../SECURITY.md](../SECURITY.md) for private vulnerability reporting.
