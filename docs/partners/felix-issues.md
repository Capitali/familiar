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
8. **The prefix says a machine observed it, so it goes only on what the machine observed.** If the
   evidence is a ledger, a journal line, a wire field or a refusal the pilot met, it is Felix's. If
   the evidence is an opinion about how the game should work — a design idea, an engineering
   judgment about the client or the exchange's shape — it is a person's, and it is filed in that
   person's own name, unprefixed. metal#75 (rescue by another hull) is Ian's idea and stays
   unprefixed; ucf-exchange#22 is Felix's because the constants sit in the pilot's own code and
   the gap showed against a live world. Dressing a human's thinking as the computer's misrepresents
   both. (Wildhorse, 2026-09-04, adopted.)
9. **Tell the other lane first.** One line naming the repo and the point before filing, so the two
   lanes never file the same thing twice (the first day's #22/#23 collision).
10. **Filings that predate the convention stay as they are** — a comment noting the source where it
    helps the reader, never a rewrite.

## Ledger

| date | repo#issue | title | lane | outcome |
|---|---|---|---|---|
| 2026-09-04 | united-cat-foods-metal#75 | Rescue by another hull (filed by wildhorse at Ian's ask, before this convention; not Felix-prefixed) | wildhorse | open |
| 2026-09-04 | ucf-exchange#22 | [Felix, ship's computer] Publish refitCost*, fuelPricePerUnit, minHoldTicks on /v1/reference — retitled and given the provenance block at tick 7823 per Ian's ruling; #23 closed onto it | wildhorse | open |
| 2026-09-04 | ucf-exchange#23 | [Felix, ship's computer] Publish the yard's fitting prices on /v1/reference — CLOSED as a duplicate of #22 (rule 5, the first day) | MacOnStick | closed |
