# Privacy Review

Substrate exists to serve people, so it necessarily observes them. Privacy is
therefore a constitutional concern (Laws I–III), not a compliance afterthought.

## What is collected

Observations (`actor · action · object · context · source · ts · confidence`) about
the human and technical systems the factory can reach. The served-data class may
include personal or sensitive detail. See [data-classification.md](data-classification.md)
and the [data sheet](../docs/data-sheet.md).

## Core privacy commitments

- **No exfiltration.** The factory sends no telemetry and transmits nothing
  outward. The network is an input it reads, never an outbound channel. This is
  constitutional restraint (capability is unrestricted; restraint is chosen) and
  will be enforced at the obedience guard.
- **Local-first.** All data stays on the host under the data directory.
- **Minimization.** Capture only what an observation needs; prefer structural facts
  over raw personal detail.
- **Consent before consequential outward action.** Where an action would touch a
  person's agency, the (planned) obedience guard favors the smaller, consented path
  and can refuse — service is not surveillance, and a system that drifts toward
  managing people rather than serving them is the failure to avoid.
- **No secrets committed.** Keys and runtime data are git-ignored; the sample log is
  synthetic.

## Lawful-basis / ethics stance

The factory stewards *systems* to serve the people in them; it never becomes
management or surveillance of those people (Law III). It does not grant any
operator authority to direct it against the served, which includes directing it to
surveil them.

## Open items (tracked)

- A **forgetting / retention** mechanism compatible with the append-only log
  (tombstone + rebuild), giving a person's data a removal path.
- A **privacy impact assessment** template for new observation sources before they
  are added.
- Encryption at rest for the data directory.

## Current status

At bootstrap the factory records observations and computes the service signal only;
it performs no outward action and no transmission. The commitments above are
design-level today and become code-enforced at the obedience guard (Brick 4).
