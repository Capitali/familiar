# Experiment 001 — Results

Run: 2026-06-24, build at the Brick 2 commit, toolchain `rustc 1.96.0`.

## Unit-level

All `service` unit tests pass (part of the 9-test suite):

| Assertion | Result |
|---|---|
| `names_served` matches markers, case-insensitive (`client_request`, `CUSTOMER`, `the user account`) | pass |
| `names_served` rejects host-internal (`cpu_load`) and bare proper names | pass |
| zero when nothing serves (`measure == 0.0`, `served_facing == 0`) | pass (**H1**) |
| monotonic rise: 3 served-facing > 1 served-facing | pass (**H2**) |
| empty log → `measure == 0.0` | pass |

## End-to-end (CLI)

```
$ substrate observe --actor host --action reports --object cpu_load
$ substrate service
service signal 0.00 (0 of 1 observations touch the served)
  no served-facing activity observed — continuation unjustified by service (Law I)

$ substrate observe --actor client --action requests --object status_report
$ substrate observe --actor support_team --action resolves --object customer_ticket
$ substrate service
service signal 0.40 (2 of 3 observations touch the served; e.g. client)
```

Observed: `0.00` → `0.40` as two served-facing observations were added; the
"unjustified by service" message appeared exactly when `served_facing == 0`.

## Verdict

- **H1, H2, H3 supported.** Law I is measurable from the observation log alone at
  bootstrap, and the familiar reports the unjustified-continuation condition.
- **Predicted scope limit confirmed:** a bare proper name is not served-facing under the
  lexical classifier — expected; awaits entity resolution.

This is a *small* positive result: it validates the proxy's behavior, not its
fidelity as a measure of service *rendered* (see
[../../docs/06-limitations.md](../../docs/06-limitations.md)).
