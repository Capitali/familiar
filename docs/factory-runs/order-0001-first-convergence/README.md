# Order #1 — first convergence (the familiar writes its own driver)

**2026-08-28, MacOnStick.** The first production order the familiar's factory
manufactured and proved. This directory captures the result — the data dir is
not version-controlled, so the milestone is recorded here.

## What happened

`familiar-factory-run` opened order #1 (a scoped-small first deliverable: the
SP548E state-query frame builder + decode, per `docs/research/sp548e-protocol.md`),
built the prompt from the order and its sourced research, and handed it to the
familiar's own reasoner via the consult seam. The reasoner wrote
[`driver.py`](driver.py) and [`test_driver.py`](test_driver.py). The factory
materialized them (digest-verified), ran the self-test **inside the containment
jail** (no radio, no household), and it passed. Converged on iteration 1.

The `ledger.jsonl` here is the append-only proof — its replay is the order's
sole truth:

```
#1 opened
#2 generation_returned  iter 1  refused=false
#3 rung_verdict         bench   pass=true
```

## What the familiar wrote, and why it is correct

`build_state_query()` returns the frame `53 02 00 01 00 00` — header `0x53`,
type `0x02` (state query), key `0x00` (unencrypted), one fragment, empty
payload — exactly the framing the research documents. `decode_state()` reads
mode at byte[30] and brightness at byte[33] with a length guard. Its self-test
verifies the frame byte by byte. All of this the familiar wrote itself from the
order and the research; the factory only judged it against the oracle.

## What this is NOT yet

This is the **bench** rung only — offline, against the framing, in the jail. The
driver has not touched the real device. The remaining rungs before the lights
move under the familiar's own hand:

- **read** — transmit the query to the real SP548E and decode its reply (needs
  the broker with notify support + the daemon's Bluetooth TCC);
- **act** — transmit a power/brightness command and read back;
- **witness** — a human confirms what the device never echoes (colour);
- then the factory **proposes** an exact `actuators.json`, Ian **declares** it,
  and the daemon manages the surface on its own.

The scope was kept small because the available reasoner provider caps at 2048
tokens; the rest of the surface (power, brightness, colour) follows as order
#1 continues.
