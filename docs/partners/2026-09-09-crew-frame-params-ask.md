# Ask for Jeff (draft, not filed): publish the crew and frame economics on `/v1/reference`

`/v1/me` now tells a pilot they have 4 berths at ℳ500 a hire, two frame pods, and a next
frame at ℳ18,000. `/v1/reference.params` (19 keys today) says nothing about what a hand
DOES or what a pod holds:

- `crewWagePerDay`, `crewEngineWearReliefBps`, `crewHoldHandlingReliefBps`,
  `crewGalleyMoraleReliefBps`, `crewMoraleFallPerMissedDayBps`, `crewMoraleRisePerPaidDayBps`
- `frameCapacityPerPod`, `frameExpansionCostPerPod` (the pack prices these: 60 / 9000)
- the galley's loaf price and `loafPerDay` per hand, if priced

A hire button with no price on it is a dare (your words, M6 slice 4). A hire *decision* with
no relief number is a guess: the ship's computer cannot tell a 500-credit engineer who
halves the yard bill from one who does nothing, and on a pack where relief is zero it would
be spending the captain's money on an ornament. Same for a pod: 60 hold at 9,000 is a
payback the merchant can compute; "next frame ℳ18,000" alone is not.

Ask: put those params beside `refitCostDriveTune` on `/v1/reference` (additive), and say on
`/v1/me.crew[]` what each hand's `effectiveSkillBps` is today so morale is visible. The
familiar will hire, feed and expand on those numbers and on the ship's own evidence, and
refuse to on their absence — which it does from tonight.
