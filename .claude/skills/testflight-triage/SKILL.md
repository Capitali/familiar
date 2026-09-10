---
name: testflight-triage
description: Watch TestFlight for tester feedback and crashes, file each as a GitHub issue automatically, then triage it to a shipped fix. Use when a TestFlight report arrives, when asked to check TestFlight feedback, to set up the watcher for a new app, or to symbolicate a crash from a beta build.
---

# TestFlight triage

One loop, end to end: a tester shakes the device or the app crashes → App Store Connect
holds the report → `tf_watch.py` files a GitHub issue with everything Apple knows →
you (Claude) triage it with the steps below → the fix ships → the issue closes with the
build number that carries it.

Apple exposes TestFlight feedback through the App Store Connect API 4.0+
(`betaFeedbackScreenshotSubmissions`, `betaFeedbackCrashSubmissions`, and
`…/crashLog`). Text, screenshots and crash logs are all in the API now; the old advice
that "feedback is only in Xcode Organizer" is out of date. Screenshot URLs expire after
about a week, which is why the watcher downloads them.

## Files

| Path | What |
|---|---|
| `scripts/tf_watch.py` | The watcher. Python 3 + system `openssl` only. Needs `gh` logged in; uses `xcsym` if present. |
| `scripts/config.example.json` | Copy to `~/.config/tf-watch/config.json`. One entry per app. |
| `scripts/install-launchd.sh` | Installs a per-user launchd agent that runs one pass every 15 min. |

State lives in `~/Library/Application Support/tf-watch/`: `seen.json` (submission id → issue URL)
and `reports/<submission-id>/` (screenshots, `crash.ips`, `xcsym.json`, `submission.json`).

## Setup for a new app or a new Mac

1. **App Store Connect API key** with at least the Developer role: App Store Connect →
   Users and Access → Integrations → Team Keys. Put the `.p8` in
   `~/.appstoreconnect/private_keys/AuthKey_<KEY_ID>.p8`. The ship scripts use the same key.
2. **Config**: `mkdir -p ~/.config/tf-watch && cp scripts/config.example.json ~/.config/tf-watch/config.json`,
   then fill `key_id`, `issuer_id`, and the `apps` list. `build_commit_grep` is the commit
   message your ship script writes when it claims a build number (`"UCF Familiar build {build}"`);
   it is how an issue gets its commit hash. `labels` go on every report, `crash_labels` only on
   crashes (default: `testflight`, plus `bug` for crashes). An optional `since` date (per app or
   top level) records older reports as seen without filing them.
3. **Dry run**: `python3 scripts/tf_watch.py --once --dry-run`. It prints the issues it would file.
4. **Install**: `scripts/install-launchd.sh` (optional interval in seconds). Check with
   `tail ~/Library/Logs/tf-watch.log`. Remove with `launchctl bootout gui/$UID/io.river.tf-watch`.
5. **Keep dSYMs**: the ship scripts copy each archive into
   `~/Library/Developer/Xcode/Archives/<date>/`. Without that, a crash from an old build cannot
   be symbolicated locally. If you add a new ship script, keep that block. The watcher also
   asks App Store Connect for the build's `dSYMUrl`, but Apple only fills that for bitcode
   builds; for ours it is `null` even with `includesSymbols: true`, so the local archive is the
   only copy. `tf_watch.py --resym <submission-id>` retries symbolication once an archive is found.

Share the skill by copying this directory into another repo's `.claude/skills/` (or into
`~/.claude/skills/` for every repo on one Mac). Nothing in it is specific to the familiar
repo except `config.example.json`.

## Commands

```bash
python3 scripts/tf_watch.py --once                # one pass now (what launchd runs)
python3 scripts/tf_watch.py --once --dry-run      # show, file nothing
python3 scripts/tf_watch.py --list                # everything seen, with issue URLs
xcsym crash "~/Library/Application Support/tf-watch/reports/<id>/crash.ips" --human
xcsym verify "<same>"                             # which images still lack a dSYM
```

## Triage: what to do when an issue lands

The issue body already carries device, OS, build, commit, the tester's words, screenshot
links and (for crashes) the xcsym summary. Work down its checklist in order.

1. **Pin the build.** The `Build` row gives the commit. Read `git log <prev-build-commit>..<commit>`
   for what changed in that build; most regressions are in that diff.
2. **Classify.** Crash with an exception → step 3. `0x8badf00d` watchdog or a jetsam
   report → main-thread block or memory, not a crash; profile, don't grep. No crash, just a
   screenshot and words → behaviour bug; the screenshot is the spec.
3. **Symbolicate and read.** `xcsym crash <crash.ips> --human`. If frames are still hex,
   `xcsym verify` says which image lacks a dSYM; the archive for that build is under
   `~/Library/Developer/Xcode/Archives`. The crashed thread's first frame in our module is the
   line to open.
4. **Reproduce** on the same device class and OS (simulator for layout, the household iPad
   for anything Metal, networking or lifecycle). A tester's comment names the room they were
   in; start there.
5. **Fix on main** with a failing test first when the bug is logic (`swift test --filter` on
   the package target). Pure layout fixes get a screenshot before and after.
6. **Ship** with the app's ship script (`ios/tools/ship-ucf.sh <n>` / `ios/tools/testflight.sh`).
   Comment on the issue with the build number that carries the fix and close it. If the
   tester left an email, they see the fix note through the TestFlight build notes; put one
   line there too.

### Patterns worth knowing

| Symptom in report | Usual cause here |
|---|---|
| Crash in first seconds, uptime under 5 s | Launch path: missing entitlement, keychain access group, or a `UNUserNotificationCenter` call before the bundle exists |
| `EXC_BAD_ACCESS` in Metal renderer frames | Buffer written after the command buffer was committed; check the frame's `MTLBuffer` lifetime |
| "Screen went black then app closed", no crash log | Jetsam. Check the Bridge's texture budget; `xcsym` reports `pageOuts` when a jetsam log is present |
| Overlay controls drawn over content on iPad | Safe-area or `ignoresSafeArea` on a full-bleed view; screenshot pixel size vs. `screenWidthInPoints` tells the scale |
| Feedback with no screenshot and no crash | Tester used Share → TestFlight from the app; treat as behaviour, ask for a screenshot on the issue |

## Guardrails

- The watcher **never deletes** anything in App Store Connect and files at most one issue per
  submission id. Re-running is safe.
- Don't file fixes for a report you can't classify. Ask the tester on the issue instead;
  the `email` field says who.
- Rate limits: App Store Connect returns 429 under bursts. The watcher backs off; don't
  run several copies.
- The `.p8` key is a secret. It stays in `~/.appstoreconnect/private_keys/`, never in the repo.
