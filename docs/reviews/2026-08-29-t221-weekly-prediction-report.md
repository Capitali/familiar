# T-221 · Following-week prediction calibration report

The observed-vocabulary fix (`049fac1`) went live fleet-wide on 2026-08-21. This is the
report its acceptance criterion promised: miss rate beside prediction-instance coverage
and settlement latency, so a better-looking rate cannot be bought by predicting less or
waiting longer.

## Cohort and method

The cohort is the first complete calendar week after deployment:
**2026-08-22 00:00 CDT inclusive → 2026-08-29 00:00 CDT exclusive** (unix
`1787374800..1787979600`). Both MacOnStick and lighthouse were read after the interval
closed.

- An instance belongs to the cohort when `opened_at` is inside the interval.
- It is covered when its result has `final_at < 1787979600`; a later result or a still-live
  pending row counts as pending at the interval end.
- Miss rate is `(missed + absent_violated) / settled_by_end` for that same cohort.
- Settlement latency is `final_at - opened_at` among the covered instances.
- Prediction class and window come from the standing prediction joined by
  `prediction_id`; class existence is checked against each store's observation table.

The SQLite databases were opened with URI `mode=ro`; `predictions.json` was read as a
plain snapshot. No Familiar store helper was called (some helpers ensure tables/indexes),
and neither database, daemon, boundary, nor record was changed.

## Result

| store | opened | settled by end | pending at end | coverage | favorable | unfavorable | miss rate | median latency | p95 latency |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| lighthouse | 69 | 61 | 8 | 88.4% | 1 | 60 | 98.4% | 4,263s (71m03s) | 87,399s (24h16m39s) |
| MacOnStick | 31 | 26 | 5 | 83.9% | 0 | 26 | 100.0% | 4,224s (70m24s) | 7,979s (2h12m59s) |
| **combined** | **100** | **87** | **13** | **87.0%** | **1** | **86** | **98.9%** | **4,263s (71m03s)** | **87,399s (24h16m39s)** |

The baseline study found **121/121 misses (100%)**. The following week produced
**86/87 unfavorable settlements (98.9%)**, an improvement of only 1.1 percentage points.
That is **not material improvement**. The promised claim therefore fails honestly; it is
reported, not promoted into success.

The denominator did not vanish: 100 prediction instances opened during the week. Coverage
was 87%, with 13 still pending at the cutoff, so the rate is not presented without its
waiting cost. The historical study did not preserve a same-duration opening count, so this
report does not claim whether prediction volume rose or fell.

## What changed, and what did not

The vocabulary repair itself held: every one of the 10 lighthouse and 7 MacOnStick
predicted `actor|action` classes in the settled cohort exists in that store's observation
record. There are **zero invented event classes**, versus the baseline's unanimous
121/121 invented-class diagnosis.

But existence was not enough to make the calls good. The dominant remaining misses were:

| predicted class | misses |
|---|---:|
| `ian|answered` | 37 |
| `ian|told the familiar` | 14 |
| `host|reports` | 13 |

Those three observed-but-sparse classes account for **64/86 misses (74.4%)**. A single
`familiar|gathered` prediction confirmed; its window was seven days. Of the 87 settled
instances, 57 used a one-hour window, so the failure is not explained solely by long
windows delaying settlement.

The inference is narrow: T-221 repaired observability, not calibration. The engine stopped
predicting events that cannot exist, then continued calling low-base-rate events at poor
times. T-230 now feeds the factual result record and observed class counts back into
theorizing, and brick 2 records class×polarity results for future feedback. This report
does not claim those newer repairs are effective before another post-deploy cohort exists.

## Disposition

T-221's report obligation is complete; its material-improvement criterion is **not met**.
The next evaluation should reuse this fixed-cohort method after T-230 is separately
deployed and has accumulated a complete week. No deployment, ship, gate, human record,
live record, or fleet state changed while producing this report.
