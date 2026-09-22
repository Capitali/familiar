# Software Bill of Materials (SBOM)

A human-readable inventory of what ships in The Familiar. A machine-readable SBOM
(CycloneDX) will be generated and committed once the tree justifies it; until then,
this is small enough to enumerate by hand. The judgment behind these choices is in
[dependency-review.md](dependency-review.md).

## Components

### First-party

| Component | Path | License | Role |
|---|---|---|---|
| `familiar-kernel` | `crates/kernel` | Apache-2.0 | Deterministic core (`#![forbid(unsafe_code)]`) |
| `familiar-cli` (`substrate`) | `crates/cli` | Apache-2.0 | CLI shell |

### Direct third-party (crates.io)

| Crate | Purpose | License (typical) |
|---|---|---|
| `serde` (+ `serde_derive`) | (de)serialization | MIT OR Apache-2.0 |
| `serde_json` | JSONL encode/decode | MIT OR Apache-2.0 |

Transitive crates (e.g. `itoa`, `memchr`, `ryu`, `proc-macro2`, `quote`, `syn`,
`unicode-ident`) are pulled in by the above and pinned in `Cargo.lock`.

### Toolchain / build

| Item | Version (verified) |
|---|---|
| Rust (rustc/cargo) | 1.96.0 (stable, via rustup) |
| Edition | 2021 |

### External runtime (periphery seam)

The LLM is **not** linked in — it is reached by shelling out to `llm/call_llm.sh`
(planned), which calls an external provider's API. The provider and model are
configuration, not a code dependency; they are documented in the
[model card](../docs/model-card.md).

## How to regenerate

The authoritative pinned set is `Cargo.lock`. To produce a full machine SBOM:

```sh
cargo install cargo-cyclonedx   # one-time
cargo cyclonedx --format json   # emits CycloneDX SBOM
```

(Adding this to CI is an open item in [dependency-review.md](dependency-review.md).)

## Provenance & integrity

- Dependencies resolved from crates.io; versions pinned in committed `Cargo.lock`.
- First-party code provenance is the git history.
