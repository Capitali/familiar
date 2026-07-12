# TestFlight — distributing the Familiar Agent (iPhone / iPad)

The archive → export → distribution-signing pipeline is **verified working** — it produces a
signed App Store `.ipa` (Xcode auto-provisions the distribution cert + profile for team
`8GHXL328AR`). The only steps that need **your Apple account** are creating the app record and an
API key; I can't do those (I never hold your Apple ID password).

## One-time setup (you)

1. **Create the app record** — [App Store Connect](https://appstoreconnect.apple.com) → **Apps** →
   **＋** → New App:
   - Platform: iOS · Name: *Familiar Agent* · Primary language: English
   - **Bundle ID:** `io.river.familiar.ios` (register it under **Certificates, Identifiers &
     Profiles → Identifiers** first if it isn't listed) · SKU: `familiar-ios`
2. **Generate an App Store Connect API key** — App Store Connect → **Users and Access** →
   **Integrations** → **App Store Connect API** → **＋**:
   - Role: **App Manager** · Download `AuthKey_<KEYID>.p8` (**one-time download**)
   - Note the **Key ID** and the **Issuer ID** (shown at the top of the page)
   - Put the key file at `~/.appstoreconnect/private_keys/AuthKey_<KEYID>.p8`

## Each release

```sh
cd ~/Development/familiar/ios
# bump the build number first (TestFlight rejects duplicates):
#   edit project.yml → CURRENT_PROJECT_VERSION: "2"  (then 3, 4, …)
ASC_KEY_ID=XXXXXXXXXX ASC_ISSUER_ID=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx ./tools/testflight.sh
```

The build appears under your app → **TestFlight** after processing (~5–15 min). Add yourself as an
**internal tester** to install via the TestFlight app on your devices — no Xcode needed after that.

## Notes

- **Team:** signing uses `8GHXL328AR` (in `tools/ExportOptions.plist`). If the iOS app should live
  under a different team, change `teamID` there. (Your dev cert is under `X7YKPEE5DE`, but the
  archive/export succeeded provisioning under `8GHXL328AR`.)
- **The watch app** (`io.river.familiar.ios.watch`) is standalone for now, not embedded — this
  pipeline ships the iPhone/iPad app only. Embedding the watch so it rides along is a follow-up.
- **First upload** may require accepting updated agreements in App Store Connect (Business/Paid
  Apps) — a one-time click.
