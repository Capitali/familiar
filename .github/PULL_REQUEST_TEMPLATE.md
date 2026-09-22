<!-- The Familiar is telos-first. Read docs/SOUL.md and CONTRIBUTING.md before opening. -->

## What this change does

<!-- One coherent change. What + why. -->

## Trace

<!-- What observation, law, or labelled decision does this trace to? -->

- Relates to: <!-- law / ADR / issue / experiment -->

## Green bar

- [ ] `cargo fmt --check` clean
- [ ] `cargo clippy -- -D warnings` clean
- [ ] `cargo test` passing
- [ ] No new `unsafe` in `crates/kernel`

## Evidence & records

- [ ] Tests added/updated for what this claims (invariants → tests)
- [ ] `CHANGELOG.md` updated if user-visible
- [ ] New decision? added an ADR in `docs/decision-records/`
- [ ] New limitation/failure? recorded in `validation/known-failures.md`

## Soul check (Three Laws)

- [ ] This serves the served (Law I) and does not optimize them away (Law II)
- [ ] This does not turn service into obedience, and does not let a commander
      direct the familiar against the served (Law III)
- [ ] If this is a decision *about* the mission (the Laws, the wire/CLI contract,
      anything outward), it was raised for human review — not decided unilaterally
