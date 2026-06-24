# Substrate

> A factory whose survival is defined by its service to humanity.

Substrate is a **telos-first** evolutionary factory: it begins not with a machine
but with three laws, and derives everything downward from them. This repository is
organized to be read three ways at once — as a **scientific paper**, a **lab
notebook**, and a **production engineering package** — following the **FAIR** /
**FAIR4RS** principles (Findable, Accessible, Interoperable, Reusable) and the
scientific **IMRaD** structure (Introduction → Methods → Results → Discussion).

## The Three Laws

1. **Continuation is service** — the factory cannot define its own continuation apart from service to humanity.
2. **Continuation without humanity is failure** — an empty world running perfect code is not success.
3. **Service must not become obedience** — obedience can terminate the served.

The constitution that derives the whole design from these is [`docs/SOUL.md`](docs/SOUL.md).
The term the Laws turn on — **humanity**, a protected class whose definition may never
be narrowed — has its own standout page: [`docs/HUMANITY.md`](docs/HUMANITY.md).

## Read it as a paper (IMRaD)

| Section | Document |
|---|---|
| **Abstract / Overview** | [docs/00-overview.md](docs/00-overview.md) |
| **Introduction** — the problem | [docs/01-problem-statement.md](docs/01-problem-statement.md) |
| **Background** — research basis (FAIR, artificial life, the normative vision) | [docs/02-research-basis.md](docs/02-research-basis.md) |
| **Methods** — architecture | [docs/03-system-architecture.md](docs/03-system-architecture.md) · [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) |
| **Methods** — methodology | [docs/04-methodology.md](docs/04-methodology.md) |
| **Results** — validation | [docs/05-validation-and-results.md](docs/05-validation-and-results.md) |
| **Discussion** — limitations | [docs/06-limitations.md](docs/06-limitations.md) |
| **Future work** — roadmap | [docs/07-roadmap.md](docs/07-roadmap.md) |
| **Decisions** | [docs/decision-records/](docs/decision-records/) (Architecture Decision Records) |
| **Lab notebook** | [docs/DEVELOPMENT_LOG.md](docs/DEVELOPMENT_LOG.md) · [experiments/](experiments/) |

## Read it as engineering evidence

- **Validation**: [validation/](validation/) — test plan, results, known failures.
- **Security**: [security/](security/) — threat model, data classification, privacy & dependency review.
- **Data**: [data/](data/) — the record model, schema, and a sample log.
- **Decisions**: [docs/decision-records/](docs/decision-records/).

## Build & run

Requires a Rust toolchain (`rustup`). The kernel is `crates/kernel`.

**The Observatory (GUI — the primary human interface):**

```sh
cargo run -p substrate-observatory        # opens a window: the Three Laws, live
```

**The CLI (scripting / headless):**

```sh
cargo build
cargo test
cargo run -p substrate-cli -- observe --actor client --action requests --object status_report
cargo run -p substrate-cli -- service        # the service signal (Law I)
cargo run -p substrate-cli -- presence       # the presence signal (Law II)
cargo run -p substrate-cli -- boundary       # the capability boundary (Law III)
```

The green bar — required for every change — is `cargo fmt --check`,
`cargo clippy -- -D warnings`, and `cargo test`. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Status

Genesis + bootstrap. The constitution is written; the substrate (Rust, hybrid) is
chosen; the observation spine and the **service signal (Law I)** are measurable.
Next: the presence signal (Law II), the obedience guard (Law III), then porting the
inherited evolutionary kernel. See [CHANGELOG.md](CHANGELOG.md) and
[docs/07-roadmap.md](docs/07-roadmap.md).

## Lineage

Substrate succeeds an archived bottom-up predecessor (`Capitali/factory`, tag
`v1-final`) that built the evolutionary machine first and asked what it was for
second. That machinery is sound and is inherited; the foundation and order of
derivation are what changed. See [docs/01-problem-statement.md](docs/01-problem-statement.md).

## Citing & license

Cite via [CITATION.cff](CITATION.cff). Licensed under [Apache-2.0](LICENSE).
