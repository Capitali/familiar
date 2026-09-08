# Re-verification brief — T-236 brick 1, round 2

**For companion:codex. Self-contained: start here cold. Launched from MacOnStick; report to
`docs/reviews/2026-09-08-t236-brick1-codex-reverification-r2.md`.**

Round 1 (2026-09-07): [`2026-09-07-t236-brick1-codex-reverification.md`](2026-09-07-t236-brick1-codex-reverification.md)
— **REJECT**, seven blockers + two should-fix on the mutation paths. Every finding has a
repair on `main` now. Re-verify each against **current main** (not the SHAs — later work sits
on top), and judge against the bar below.

---

## The bar (Ian, 2026-09-04, verbatim)

> "One 'ships computer' per captain that can act across his entire fleet under a name he
> chooses. I choose Felix."

The instance is **per captain**, not per hull. One persona, one name, one memory across every
ship the captain pairs; hulls keep their own names. Two captains' fleets share nothing
observable. Round 1 already found the kernel v1/v2 seam, captain-first no-fall-through
resolution, and "a broken persona never stalls the pilot" HELD — re-confirm briefly, then
spend the review on the nine repairs.

## The nine findings and their repairs

| # | Round-1 finding (title verbatim) | Repair commit | What to probe |
|---|---|---|---|
| 1 | Blocker — host pairing silently drops the computer name the captain supplied | `6ce7609` | `/pair` on the feed forwards `--computer-name`; the `/ships` row shows the captain's name, not `Purr` |
| 2 | Blocker — joining a captain whose only record is the ship-local fallback replaces the chosen name with Purr | `6ce7609`, `c86fbb3` | second pairing for a captain whose only record is pre-ruling Felix: Felix survives, nothing is shadowed; an unmigrated record adopts a sibling's id for the same name FIRST (`ensure_captain_id`) |
| 3 | Blocker — the persona and its append-only trail are not one serialized mutation | `03bc9cb` (+`7f7acff` fmt) | `kernel::persona::name`: OS file lock, unique temp (pid+seq), fsync file AND directory, persona+trail all-or-nothing; trail-append failure is NOT reported as success |
| 4 | Blocker — a lossy display-name slug is being used as captain identity | `6ce7609`, `c86fbb3` | `captain_id` is generated and opaque, never derived; slug is a display label only; `A/B` and `A B` get distinct personas AND distinct briefs; `fleet adopt-ids` migration is idempotent read-old-write-new; a stale slug path returns **410** with the captain's new location, not a stranger's record |
| 5 | Blocker — `--captain` can falsely act as another captain and still rename the target captain's computer | `6ce7609` | `fleet rename <world> Felix --captain Bob` on a ship whose captain is not Bob is **refused**; `--captain` labels the act, it does not select the target |
| 6 | Blocker — persona failure happens after pairing has already committed the ship and key | `6ce7609` | naming runs BEFORE world, key and captain.json are committed; a persona failure leaves no half-paired ship |
| 7 | Should-fix — strict persona errors are hidden as "unnamed" on status and briefs | `6ce7609` | a persona the kernel refuses reads as a broken persona that says so, not "(unnamed — `fleet rename`…)" |
| 8 | Blocker — opening a broken persona leaves the previous captain's voice live in Swift | `c631907` | BridgeModel open publishes only after every read succeeds; failure clears the voice |
| 9 | Should-fix — Swift and Rust disagree on the captain brief route for the LOCAL rig | `c631907` | the client asks for the row's `captain_brief`; the slug is a byte-exact legacy fallback only; the LOCAL bridge finds the fleet document |

Related since round 1, in scope only where it touches the above: the seam
(`c63a61c`, `c9419b9`, SEAM_VERSION 2) and T-238 forecast bricks do not touch the captain
store or persona paths — ignore unless a repair regressed through them.

## Method

- Rust: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`.
  Do not pipe `cargo test` into `head` (it hangs). Cite counts.
- Swift: FamiliarSC package tests (`swift test` in `ios/FamiliarSC`); both apps build.
- Transient probes are welcome (round 1 used them); note each probe's exact input and output.
- Verdict per finding: HELD / NOT HELD / PARTIAL, with file:line. Then one overall
  ACCEPT / REJECT against the bar.

## Scope and rules

- Read-only on source. Output is the report file only. No install, ship, upload, deploy.
- Do not edit `coordination/*`; the controller records the return.
- Off-limits regardless: `DirectFeed.swift`, `Briefs.swift`, `UCFFamiliar`, `ios/FamiliarCore`
  (MacOnStick's live T-237 B4 lane).
