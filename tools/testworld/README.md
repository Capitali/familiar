# testworld — a household for the familiar to explore

In a quiet environment the familiar's own telemetry is most of what it can
see, and it becomes everything it thinks about (connectivity navel-gazing).
The cycle now looks past its own plumbing and metabolism (`infra_triple` in
`crates/cycle`), which leaves a muse with an appetite and nothing to eat.
This directory is the food: small, legible services with **domain content**
— needs, trends, dates, a household — plus canned observations for tuning
without any infrastructure at all.

## The three feeding paths

1. **Seed canned observations (fastest — no VM).** With the daemon running:

       tools/testworld/seed-observations.sh

   POSTs `observations.jsonl` (greenhouse readings, pantry lows, almanac
   events, things Ian said) to the loopback observe seam (`:47101
   /local/observe`). The seam records with source `local`, so seeding
   rides the muse's **novelty** path: twenty fresh observations shrink the
   wait to its floor (~5 minutes from the last theory). Watch what
   theories the material produces and tune the JSONL directly.

2. **Live services on the LAN (the real loop).** `testworld.py` serves a
   household on one HTTP port:

       /greenhouse   temp, humidity, lamp, soil moisture — dries until watered
       /pantry       staples that deplete; things run low
       /almanac      upcoming / due / overdue events

   Plain-text sentences by default (`?json=1` for JSON). State is a pure
   function of `(--seed, clock)` — restart-stable, and different seeds give
   different households. One action surface exists: `POST /greenhouse/water`
   resets the soil clock, so acting has a visible consequence.

   Put it on a **separate box** on :80 so discovery finds it as another
   device: revive the kept FamTalker01 VM (`VBoxManage startvm FamTalker01
   --type headless`) or any small Debian guest, then:

       ssh <guest> 'bash -s' < tools/testworld/provision-testworld.sh
       scp tools/testworld/testworld.py <guest>:/opt/testworld/
       ssh <guest> systemctl start testworld

   The reach sweep probes :80 → the box lands on the frontier → cultivated
   curl sensors read the rooms → their **readings** (not the run records)
   ride the muse prompt as "Latest sensor readings".

   Local smoke test without a VM: `python3 tools/testworld/testworld.py
   --port 8047` and curl the rooms.

3. **Scenario lab.** The same household shapes make good fixture material
   for `crates/scenario` worlds (a service whose checks are "the basil got
   watered", not "the port answered"). Not wired yet — generate fixtures
   from these services when the next scenario family is authored.

## Tuning

- **The muse's diet** is `crates/cycle`'s `maybe_theorize`: recent
  non-infra observations + latest reading per sensor + non-infra loops.
  What you seed here is exactly what it eats.
- **Verbs matter**: `reports` (non-presence objects), `announces`,
  `told the familiar`, `answered` all pass the infra filter;
  `can-reach`/`sees`/`discovered`/`gathered` do not — see
  `infra_triple()` for the full judgment.
- Different `--seed` values give different households (different restock
  phases, event schedules, watering habits) — cheap variety for testing
  whether theories track the world or just parrot one instance of it.
