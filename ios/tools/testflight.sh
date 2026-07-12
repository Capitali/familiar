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

ARCHIVE=/tmp/FamiliarAgent.xcarchive
EXPORT=/tmp/FamiliarAgent-export
rm -rf "$ARCHIVE" "$EXPORT"

echo "== regenerate project =="
xcodegen >/dev/null

echo "== archive =="
xcodebuild -project FamiliarAgent.xcodeproj -scheme FamiliarAgent \
  -sdk iphoneos -destination 'generic/platform=iOS' \
  -archivePath "$ARCHIVE" archive -allowProvisioningUpdates

echo "== export (App Store) =="
xcodebuild -exportArchive -archivePath "$ARCHIVE" \
  -exportOptionsPlist tools/ExportOptions.plist -exportPath "$EXPORT" -allowProvisioningUpdates

IPA=$(ls "$EXPORT"/*.ipa | head -1)
echo "== upload $IPA =="
xcrun altool --upload-app --type ios --file "$IPA" \
  --apiKey "$ASC_KEY_ID" --apiIssuer "$ASC_ISSUER_ID"

echo "✓ uploaded — appears under App Store Connect → your app → TestFlight after processing (~5-15 min)."
