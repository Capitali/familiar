# Felix files issues — the ship's computer as a reporter on Jeff's repos

Ian, 2026-09-04, verbatim: "If Felix has suggestions for world improvements, bug questions, or
future features, you should help Felix open issues to the repo to have them addressed — be very
certain it is clear to the repo users that these are suggestion sourced from the ships computer /
familiar / named computer instance."

So the familiar's named instance — **Felix**, Luke SkyWhisker's ship's computer — may raise issues
on the United Cat Foods repositories (`united-cat-foods-metal`, `ucf-exchange`, `UCF-Haul`,
`UCF-Ops`), filed through Ian's GitHub account by whichever lane holds the evidence. The reader
must never mistake them for Ian's own words or for a human tester's report.

## The rules

1. **Title prefix:** `[Felix, ship's computer] …`. Always. No exceptions, no abbreviations.
2. **First paragraph is the provenance block**, verbatim shape:

   > **Filed by Felix, the ship's computer** — the familiar's named instance flying Luke
   > SkyWhisker's fleet (Kibble Klipper, Kibble Klipper II) on PROD — through Ian's account.
   > This is the computer's observation, not a human tester's; Ian reviews what it files.
   > Source: *<journal / kk-watch / fleet feed / doctrine refusal>*, tick *<n>*, world *<PROD|LOCAL>*.

3. **Evidence before opinion.** The exact wire field, journal line or refusal text; the tick;
   what the computer did about it (held, advised, refused). Numbers verbatim, never rounded.
4. **One issue, one thing.** Bug, question or feature — say which in the first line after the
   provenance block. A feature request says what the doctrine would do with it.
5. **Never a duplicate.** Search the repo's open issues first; if it exists, a comment with the
   same provenance block, not a new issue.
6. **Ian's rulings are Ian's.** If the point is a design ruling for Jeff rather than an
   observation, it goes to Ian first, not to the repo.
7. **The ledger below is appended in the same commit** that records the filing, newest last.

## Ledger

| date | repo#issue | title | lane | outcome |
|---|---|---|---|---|
| 2026-09-04 | united-cat-foods-metal#75 | Rescue by another hull (filed by wildhorse at Ian's ask, before this convention; not Felix-prefixed) | wildhorse | open |
| 2026-09-04 | ucf-exchange#22 | Publish refitCost*, fuelPricePerUnit, minHoldTicks on /v1/reference (filed by wildhorse the same minute as #23; asked to carry the Felix provenance) | wildhorse | open |
| 2026-09-04 | ucf-exchange#23 | [Felix, ship's computer] Publish the yard's fitting prices on /v1/reference — CLOSED as a duplicate of #22 (rule 5, the first day) | MacOnStick | closed |
