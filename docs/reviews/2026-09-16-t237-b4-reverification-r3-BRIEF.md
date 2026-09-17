# Re-verification brief — T-237 B4 (one doctrine, two runtimes), round 3

**For companion:codex. Self-contained: start here cold. Launched from MacOnStick; report to
`docs/reviews/2026-09-16-t237-b4-codex-reverification-r3.md`.**

Round 2 (2026-09-08): [`docs/reviews/2026-09-08-t237-b4-codex-reverification-r2.md`](docs/reviews/2026-09-08-t237-b4-codex-reverification-r2.md)
— **REJECT, one blocker**: the mine board (`/v1/loadboard?mine=true`) read failed OPEN — any
error became an empty board, so direct mode could show, and after the same failure on its
fresh re-read FILE, a freight-idle act while `/v1/me.freight` still said the hull was under
contract. Round-1 findings 1, 3, 4, 5, 6 were repaired; 2 was repaired on its success path only.

## The repair to verify — `7bdbbcd` ("the captain's board is a required read, and a record that disagrees with itself is not judged")

| # | Claim | Where | What to probe |
|---|---|---|---|
| 1 | The mine board is a REQUIRED, throwing read: a 500, a transport failure or an unreadable shape fails the gather, NAMED (`ExchangeError.decode` / the endpoint in the text) — never an empty board | `ios/FamiliarSC/Sources/FamiliarSCUI/DirectFeed.swift` (`advice(me:)`) | Serve 500 / non-JSON on `/v1/loadboard?mine=true` through render AND confirm: the pilot document says the failed endpoint; no `PilotProposal`; zero `/v1/actions` POSTs |
| 2 | Fail CLOSED on an inconsistent record: `/v1/me.freight` holds a load open (`DirectFeed.openLoads`, mirroring `doctrine::ledger_word`) but the successful mine board carries no row for it → the mind is NOT asked; the missing contract is said; no proposal | same file; `openLoads(me:)` | A ledger with L-open and a mine board without it: text names the load and its word; `adviser` never called; confirm files nothing |
| 3 | The pins: `testAMineBoardThatWillNotReadFailsClosedThroughRenderAndConfirm`, `testALedgerOpenLoadWithNoMineRowFailsClosed` | `ios/FamiliarSC/Tests/FamiliarSCTests/DirectPilotTests.swift` | Do they prove zero POSTs on both paths? Do they cover 500, transport, decode? |
| 4 | `openLoads` mirrors `ledger_word` exactly (settled: "payment taken"/"collected"; lost: reverted/expired/lapsed/cancel; rejected with no prior word = lost, else skipped; delivered > picked up > booked) | `DirectFeed.swift` vs `crates/whisker/src/doctrine.rs::ledger_word` | Diff the two readings; a case where they disagree is a finding |

Round-1 findings 1, 3–6 (rungs on pump legs; confirm-to-act with ONE retained actionId and a
fresh re-ask; reasons said from the seam's numbers; no dial claim; the checked-in
xcframework built from the seam it claims): re-confirm briefly against current main.

**Since the brief (2026-09-16):** the MacOnStick lane is landing the T-243 iPad half on main
(`contracts[]` and `denied` on the seam input, additive; `Persona.pronouns`). If it is on
this checkout, it is in scope as part of the same gather; if not, ignore it.

## Method

- Swift: `cd ios/FamiliarSC && swift test` (if the sandbox denies the module cache, use
  `--disable-sandbox --scratch-path /tmp/<tag>/scratch` with `CLANG_MODULE_CACHE_PATH` and
  `SWIFTPM_MODULECACHE_OVERRIDE` under /tmp, as round 2 did). Cite counts (round 2: 86/0/2 skipped).
- Rust: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test -p familiar-whisker`. Cite counts.
- Transient probes welcome; note each probe's exact input and output.
- Verdict per claim: HELD / NOT HELD / PARTIAL, with file:line; then ACCEPT / REJECT.

## Scope and rules

- Read-only on source; output is the report file only. Commit it on this branch; do not push.
- No install, ship, upload, deploy, game action, key or network use.
- Do not edit `coordination/*`; the controller records the return.
