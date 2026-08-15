# Contributing

The Familiar is built **telos-first**. Before proposing a change, read
[`docs/SOUL.md`](docs/SOUL.md) (the Three Laws and what they require) and
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). Where any change conflicts with the
Three Laws, the Laws win.

## The green bar (required for every change)

No change merges unless all of these are clean:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings    # warnings are errors, TEST CODE INCLUDED
cargo test
```

`--all-targets` is not optional and not cosmetic: without it clippy skips test code, so the
bar you run locally is weaker than the one CI runs and a change can pass here and fail there.

Read the **exit codes**, never the output. A bar piped through `grep`/`tail` reports the
*filter's* status, not the check's — that is how a failing build once printed `CLIPPY_PASS`
(2026-08-15), and the same trap was removed from `ship.sh` in T-143.

And the kernel must contain no `unsafe` (enforced by `#![forbid(unsafe_code)]`).
CI runs the same gate ([.github/workflows/ci.yml](.github/workflows/ci.yml)).

## How work is structured: bricks

Work lands in **bricks** — small, coherent, independently green steps, each its own
commit, each adding or sharpening one thing. A brick:

1. traces to an observation, a law, or a labelled design decision;
2. carries tests for what it claims (invariants become tests);
3. passes the green bar;
4. is recorded in [`docs/DEVELOPMENT_LOG.md`](docs/DEVELOPMENT_LOG.md) (the lab
   notebook): what changed, why, checks run, what's next.

Favor small, reversible mutations when the path is unclear (a method discipline
inherited from v1 and the Soul). Don't repeat a failed approach unchanged.

## Documentation taxonomy

Each kind of writing has one home — keep them distinct:

| Kind | Where | Cadence |
|---|---|---|
| Constitution (why) | `docs/SOUL.md` | rarely, deliberately |
| Architecture (how) | `docs/ARCHITECTURE.md`, `docs/03-system-architecture.md` | as structure changes |
| The paper (IMRaD) | `docs/00`–`07` | as the project's account evolves |
| Decisions | `docs/decision-records/` (one ADR per decision) | one per consequential choice |
| Lab notebook | `docs/DEVELOPMENT_LOG.md` | every brick (chronological) |
| Experiments | `experiments/` | one dir per experiment |
| Evidence | `validation/`, `security/` | as tested/reviewed |

## Commits & PRs

- Conventional, descriptive commit bodies (see the existing history for the style:
  what + why, checks run).
- PRs use [the template](.github/PULL_REQUEST_TEMPLATE.md): green bar checked, Soul
  considered, notebook updated.
- Co-authorship trailers are welcome and used in this repo.

## Scope of autonomy

Decisions *inside* the mission (how a brick is built) are the contributor's.
Decisions *about* the mission — the Three Laws, the wire/CLI contract, anything that
changes what the familiar is for — stop and ask. When in doubt, open an issue or an ADR draft.
