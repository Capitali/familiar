#!/bin/bash
# testflight.sh — archive → export (App Store) → upload FamiliarAgent (iPhone/iPad) to TestFlight.
#
# One-time setup (needs YOUR Apple account — see TESTFLIGHT.md):
#   1. Create the app record in App Store Connect for bundle id io.river.familiar.ios.
#   2. App Store Connect → Users and Access → Integrations → App Store Connect API → generate a key
#      (role: App Manager). Download AuthKey_<KEYID>.p8 and put it in ~/.appstoreconnect/private_keys/.
#
# Then, each release:
#   ASC_KEY_ID=XXXXXXXXXX ASC_ISSUER_ID=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx ./tools/testflight.sh
#
# Signing is automatic (Xcode provisions the distribution cert + app-store profile for the team in
# tools/ExportOptions.plist). Bump CURRENT_PROJECT_VERSION in project.yml before each upload —
# TestFlight rejects a duplicate build number.
set -euo pipefail
cd "$(dirname "$0")/.."

: "${ASC_KEY_ID:?set ASC_KEY_ID (App Store Connect API key id)}"
: "${ASC_ISSUER_ID:?set ASC_ISSUER_ID (App Store Connect issuer id)}"

# Authenticate signing to the App Store Connect portal with the API key, NOT a GUI-signed-in Xcode
# account: the command-line xcodebuild doesn't see accounts signed in through Xcode's UI ("No
# Accounts"), so we hand it the .p8 directly. This lets -allowProvisioningUpdates mint the Apple
# Distribution certificate + app-store profile headlessly. The key file lives on this Mac only.
ASC_KEY_PATH="${ASC_KEY_PATH:-$HOME/.appstoreconnect/private_keys/AuthKey_${ASC_KEY_ID}.p8}"
: "${ASC_KEY_PATH:?}"
[ -f "$ASC_KEY_PATH" ] || { echo "ASC key not found at $ASC_KEY_PATH"; exit 1; }
AUTH=(-allowProvisioningUpdates \
  -authenticationKeyPath "$ASC_KEY_PATH" \
  -authenticationKeyID "$ASC_KEY_ID" \
  -authenticationKeyIssuerID "$ASC_ISSUER_ID")

ARCHIVE=/tmp/FamiliarAgent.xcarchive
EXPORT=/tmp/FamiliarAgent-export
rm -rf "$ARCHIVE" "$EXPORT"

echo "== regenerate project =="
xcodegen >/dev/null

echo "== archive =="
# No -sdk iphoneos: that flag forces EVERY target (including the embedded watchOS app) onto the
# iOS SDK, so the watch fails to build. The generic iOS destination lets each target use its own
# SDK — the app builds for iphoneos, the embedded watch for watchos.
xcodebuild -project FamiliarAgent.xcodeproj -scheme FamiliarAgent \
  -destination 'generic/platform=iOS' \
  -archivePath "$ARCHIVE" archive "${AUTH[@]}"

echo "== export (App Store) =="
# Manual signing (ExportOptions pins the Apple Distribution cert + our App Store profiles). Do NOT
# pass -allowProvisioningUpdates here — that re-triggers cloud signing, which produced a profile
# without our cert. Export reads the locally-installed profiles instead.
xcodebuild -exportArchive -archivePath "$ARCHIVE" \
  -exportOptionsPlist tools/ExportOptions.plist -exportPath "$EXPORT"

IPA=$(ls "$EXPORT"/*.ipa | head -1)
echo "== upload $IPA =="
xcrun altool --upload-app --type ios --file "$IPA" \
  --apiKey "$ASC_KEY_ID" --apiIssuer "$ASC_ISSUER_ID"

echo "✓ uploaded — appears under App Store Connect → your app → TestFlight after processing (~5-15 min)."
