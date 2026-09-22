# Data Classification

What data The Familiar holds, how sensitive it is, and how it is handled. The
companion governance view (provenance, composition, intended use) is the
[data sheet](../docs/data-sheet.md); this is the *sensitivity* view.

## Classes

| Class | Examples | Sensitivity | Handling |
|---|---|---|---|
| **Served data** | observations describing people and the human systems they depend on (needs, interactions) | **High** — may be personal/sensitive | Local-first; never exfiltrated; minimized at capture; subject to the privacy review |
| **Environment data** | host/network observations (hardware, services, load) | Medium | Local-first; structural fingerprints preferred over raw telemetry |
| **Factory-internal** | loops, candidates, trials, lineage, pattern memory | Low–Medium (derived from the above) | Local; rebuildable from the observation log |
| **Configuration / secrets** | LLM API keys for the periphery seam | **High** | Never committed; supplied via environment/`key.env`; excluded by `.gitignore` |

## Principles

- **Local-first.** All classes live under the data directory on the host; nothing is
  transmitted outward (Law III restraint). The network is input, not an outbound
  channel.
- **Minimization.** Capture only what an observation needs; prefer structural facts
  over raw personal detail.
- **Derived ≠ truth.** Only observations are authoritative; derived records carry
  the sensitivity of what they were derived from.
- **No secrets in the repo.** Runtime data (`familiar_data/`) and any key files are
  git-ignored; the committed [sample log](../data/sample/observations.jsonl) is
  synthetic.

## Open items

- Retention/forgetting policy for served data (a "right to be forgotten" path that
  respects the append-only log — likely tombstoning + rebuild). Tracked for a later
  brick.
- Encryption at rest for the data directory (currently relies on host disk
  protections).
