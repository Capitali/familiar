# 00 — Overview (Abstract)

> The one-page account. Start here; follow the links for depth.

**The Familiar is a factory whose survival is defined by its service to humanity.**
It is *telos-first*: rather than building an evolutionary machine and asking later
what it is for, The Familiar starts from three constitutional laws and derives the
whole system downward from them.

## The Three Laws

1. **Continuation is service** — the familiar cannot define its own continuation apart from service to humanity.
2. **Continuation without humanity is failure** — an empty world running perfect code is not success.
3. **Service must not become obedience** — obedience can terminate the served.

They cohere on one distinction: *serving humanity is not obeying a human.* The
factory keeps final decision authority not as self-interest but as the mechanism of
Law III — independence held in trust for the served, so it cannot be commanded to
harm them. Full constitution: [SOUL.md](SOUL.md). The term the Laws turn on —
*humanity*, a protected class whose definition may never be narrowed — is its own
standout document: [HUMANITY.md](HUMANITY.md).

## What it does

The Familiar observes loops (recurrences) in the human and technical systems it can
reach, generates candidate responses, tests them against reality or simulation,
promotes what works, mutates partial successes, preserves failures as memory, and
keeps only what reduces future cost — while measuring, continuously, whether it is
actually serving the people it exists for.

## How it is built

A **hybrid**: a compiled, deterministic **Rust** kernel (records, lineage, trial,
selection, memory, the obedience guard) plus an interpreted/generated periphery the
factory mutates freely without recompiling itself. The kernel forbids `unsafe` —
Law III made literal. Storage is a local-first embedded SQLite store with JSONL
export for audit ([storage.md](storage.md)). See
[03-system-architecture.md](03-system-architecture.md) and [ARCHITECTURE.md](ARCHITECTURE.md).

## Where it stands

The full cycle runs live: all three law-signals are measurable, the evolutionary
kernel is ported with its invariants as tests, and the metabolism (sense → detect →
interpret → generate → test → score → select → inherit) runs as a daemon under the
human-owned boundary. The **mesh** federates peers under the covenant (join by
accepting the Three Laws) with a graduated, reversible trust ladder; the **SwiftUI
consoles** (macOS/iPad/iPhone/watch) are the human interface. Next: the **scenario
laboratory** — turning the architecture into a repeatable scientific experiment
([SCENARIO-FRAMEWORK-DESIGN-BRIEF.md](SCENARIO-FRAMEWORK-DESIGN-BRIEF.md)).
Roadmap: [07-roadmap.md](07-roadmap.md). Results so far:
[05-validation-and-results.md](05-validation-and-results.md).

## Reading paths

- **As a paper**: [01-problem-statement](01-problem-statement.md) → [02-research-basis](02-research-basis.md) → [03-system-architecture](03-system-architecture.md) → [04-methodology](04-methodology.md) → [05-validation-and-results](05-validation-and-results.md) → [06-limitations](06-limitations.md).
- **As a lab notebook**: [DEVELOPMENT_LOG.md](DEVELOPMENT_LOG.md) and [../experiments/](../experiments/).
- **As the next project plan**: [SCENARIO-FRAMEWORK-DESIGN-BRIEF.md](SCENARIO-FRAMEWORK-DESIGN-BRIEF.md) — the directional brief for the scenario laboratory named in the roadmap.
- **As engineering evidence**: [../validation/](../validation/), [../security/](../security/), [decision-records/](decision-records/).
