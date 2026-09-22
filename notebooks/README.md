# notebooks/

**Reserved — not yet used.**

Analysis notebooks (e.g. exploring an experiment's observation log, plotting how a
law-signal behaves over a run) will live here once there is enough runtime data to
analyze. They are for *analysis and communication*, never part of the deterministic
kernel or the familiar's decision path — the kernel stays clock-free, dependency-light
Rust ([../docs/ARCHITECTURE.md](../docs/ARCHITECTURE.md)).

When added, each notebook should:

- read from an exported/sample data dir, never mutate live state;
- be reproducible (pinned environment; inputs referenced, not embedded);
- pair with the experiment it supports under [../experiments/](../experiments/).

Until then, the reproducible analysis is the experiment records and the CLI walkthrough
in [../examples/](../examples/).
