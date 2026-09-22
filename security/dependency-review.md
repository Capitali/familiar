# Dependency Review

A small, legible dependency set is part of the Law III commitment: every dependency
runs in-process in an unrestricted-reach agent, so each is part of the trust
surface. The machine-readable inventory is [sbom.md](sbom.md); this is the
*judgment* behind it.

## Policy

- **Minimal by default.** Add a dependency only when it removes materially more risk
  or effort than it adds. Hand-rolling small, legible code (e.g. the CLI arg parser)
  is preferred over a crate when the crate's surface exceeds the need.
- **Audited additions.** Each new dependency is justified in an ADR or the dev log,
  with a note on its maintenance health and transitive footprint.
- **Pinned.** `Cargo.lock` is committed; builds are reproducible.
- **No `unsafe` in our kernel.** We cannot forbid `unsafe` in dependencies, which is
  one reason to keep them few and well-known.

## Current direct dependencies

| Crate | Why | Notes |
|---|---|---|
| `serde` (derive) | Record (de)serialization | De-facto standard; removes v1's hand-rolled JSON parser and its bug surface |
| `serde_json` | JSONL encode/decode | Pairs with `serde`; the only serialization format used |

No networking, async-runtime, or CLI-framework dependencies are pulled in — the CLI
and (future) periphery seam shell out rather than embed.

## Review cadence

- On every dependency change (PR-gated).
- Periodically via `cargo update` review + advisory check (`cargo audit` to be added
  to CI as the tree grows).

## Open items

- Add `cargo audit` (RustSec advisory DB) to CI once justified by tree size.
- Generate the SBOM mechanically (e.g. `cargo cyclonedx`) and commit it alongside
  the human summary in [sbom.md](sbom.md).
