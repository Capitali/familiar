# Re-verification brief — T-237 B4 (one doctrine, two runtimes), round 4

**For companion:codex. Self-contained: start here cold. Launched from MacOnStick; report to
`docs/reviews/2026-09-18-t237-b4-codex-reverification-r4.md`.**

Round 3 (2026-09-18): [`docs/reviews/2026-09-16-t237-b4-codex-reverification-r3.md`](docs/reviews/2026-09-16-t237-b4-codex-reverification-r3.md)
— **REJECT, two blockers.** Both repaired in merge `75b32ae` (log: DEVELOPMENT_LOG 2026-09-18
"T-237 B4 codex round 3 repaired"). Re-verify each against current main.

## The repairs to verify

| # | Round-3 blocker | Repair | What to probe |
|---|---|---|---|
| 1 | Valid JSON of the wrong shape on `/v1/loadboard?mine=true` failed open (HTTP-200 object / `null` / scalar → empty board → judged → filed); transport failures unnamed | `ios/FamiliarSC/Sources/FamiliarSCUI/DirectFeed.swift` (`advice(me:)`: decode `[JSONValue]`, else `ExchangeError.decode(endpoint, "expected the captain's rows as an array, got …")`); `ios/FamiliarSC/Sources/FamiliarSC/Exchange.swift` (`get`/`file` wrap transport as `"<path>: <message>"`) | Serve `{"error":…}`, `null`, `"[]"`, `7`, and a `URLProtocol` transport failure through render AND confirm: endpoint named, no proposal, the mind's call count unchanged, zero POSTs. The pin: `DirectPilotTests.testAMineBoardThatWillNotReadFailsClosedThroughRenderAndConfirm` (+ `MockExchange.fail`). Does it prove all of that? Any shape still admitted? |
| 2 | The checked-in `FamiliarCore.xcframework` predated T-243's `contracts[]`/`denied` under an unmoved seam 2 (archive: `repair`; source: `hold`) | `crates/whisker/src/wire.rs` `SEAM_VERSION = 3` (rule written on the constant); `DirectFeed.seamVersion = 3`; archive rebuilt by `tools/build-core.sh`; **the artifact pin**: `ios/FamiliarSC/Tests/FamiliarSCTests/Fixtures/contract/seam-{denied-repair,bay-full}.json` read at compile time by `wire::seam_parity_tests::the_checked_in_core_fixtures_say_what_current_source_says`, and run against the CHECKED-IN archive by `ios/UCFFamiliarTests/CorePinTests.swift` (XcodeGen target `UCFFamiliarTests`, app-hosted, simulator) | Your r3 probe (titled hull at `a`, wear 6000, credits 100000, fuel 590/600, empty board, pump at `a`, `denied:["repair"]`) against the rebuilt archive: seam 3, `hold`. Does the fixture pair cover what r3 found (denied AND multiple contracts)? Is the compile-time include honest (edit the fixture → Rust test sees it)? Is `CorePinTests` reachable by `xcodebuild test -scheme UCFFamiliar` (scheme `testTargets`)? Is a seam-2 core now REFUSED by the seam-3 shell (`testASeamThisShellWasNotBuiltForIsRefusedAndOffersNoAct`)? |

Round-1 findings 3, 4, 5 (confirm path; reasons; no dial claim) and r3 claims 2 and 4 (ledger-open
guard; `openLoads` mirrors `ledger_word`): re-confirm briefly.

## Method

- Swift: `cd ios/FamiliarSC && swift test` (sandbox note: `--disable-sandbox --scratch-path /tmp/<tag>/scratch` with `CLANG_MODULE_CACHE_PATH` and `SWIFTPM_MODULECACHE_OVERRIDE` under /tmp if needed). Cite counts (r3: 95; now 100 with T-241's 5).
- Rust: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test -p familiar-whisker -p familiar-core-ffi`. Cite counts.
- The archive: probe it as you did in r3 if you can; the simulator target is documented in DEVELOPMENT_LOG (build first, then `xcodebuild test-without-building`; the simulator on this Mac hangs on a combined `test`). Not required — say what you ran.
- Verdict per repair: HELD / NOT HELD / PARTIAL, with file:line; then ACCEPT / REJECT.

## Scope and rules

- Read-only on source; output is the report file only. Commit it on this branch; do not push.
- No approval, dial write, game action, deploy, ship, gate, key or network use.
- Do not edit `coordination/*`; the controller records the return.
