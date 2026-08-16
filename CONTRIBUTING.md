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

## Absence is not a negative

**No data means we do not know. It never means "no", "bad", "failed", or "absent".**

Ian, 2026-08-16, after this bit us six times in one session: *"no more assuming that no data
equals a negative state."* Every one of these shipped, and every one was invisible because a
confident wrong answer looks exactly like a right one:

| The absence | What it was read as | What it actually meant |
|---|---|---|
| a provider with no health record | unhealthy, ranked below every working one | never tried — the user's configured order was silently overridden |
| a watch with no rendered error | a watch that never attempted to join | it tried, failed, and captured the reason nobody displayed |
| `UIDevice.name` withholding the user's name | the device is called "iPhone" | iOS refused to tell us, and we published the refusal as a fact |
| a join with no `linkErr` set | a failure with no cause | the cause was in the join's own detail, never read |
| an introduction with no grant yet | discard it silently | the handshake had not finished; the human's act was still valid |
| a reply the model could not be reached for | "I couldn't reach my mind" | it was reached every time; the answer was thrown away as malformed |

The discipline:

- **Distinguish the three states.** Known-true, known-false, and *not established* are three
  values, not two. If a bool cannot carry that, use an `Option`, an enum, or an explicit
  `unknown` — `status in ("ok", None)` rather than `status == "ok"`.
- **Choose the safe default deliberately, and say why in a comment.** An unset device posture
  reads as `carried` because misreading a person's phone as a fixed station would *suppress*
  real presence; that choice is defensible and documented. An undefensible default is a bug.
- **Never render absence as a value.** "unknown", "not established yet", "nobody identified
  here yet" — a blank, a zero, or a plausible placeholder is worse than the honest word.
- **Say why you don't know, where you can.** A failure mark with no reason is unactionable;
  the reason is usually already in hand somewhere and simply never read.

This is the engineering form of the rule in [SOUL.md](docs/SOUL.md): *the familiar must never
be able to make not-knowing serve it, because not-knowing is the one failure that cannot be
corrected.* Code that treats absence as a negative is the familiar lying to itself, one layer
below where anyone thinks to look.

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
