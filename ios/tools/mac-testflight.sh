#!/bin/bash
# mac-testflight.sh — archive → export (App Store) → upload FamiliarMac (macOS console) to TestFlight.
#
# ONE-TIME setup before the first run (the parts the API can't do):
#   1. Register the bundle id io.river.familiar.mac (macOS) — DONE via the ASC API (bundleIds).
#   2. Create the app record in App Store Connect for io.river.familiar.mac:
#        App Store Connect → Apps → (+) New App → Platform: macOS,
#        Name: "Familiar Console", Bundle ID: io.river.familiar.mac, SKU: familiar-mac-001.
#      (Apple forbids creating apps via the API; this step is manual, ~2 min.)
#   3. The ASC API key at ~/.appstoreconnect/private_keys/AuthKey_<KEYID>.p8 (shared with the iOS
#      pipeline).
#
# Then, each release:
#   ASC_KEY_ID=XXXXXXXXXX ASC_ISSUER_ID=xxxxxxxx-... ./tools/mac-testflight.sh
#
# Bump CURRENT_PROJECT_VERSION for the FamiliarMac target in project.yml before each upload —
# TestFlight rejects a duplicate build number.
set -euo pipefail
cd "$(dirname "$0")/.."

: "${ASC_KEY_ID:?set ASC_KEY_ID (App Store Connect API key id)}"
: "${ASC_ISSUER_ID:?set ASC_ISSUER_ID (App Store Connect issuer id)}"
ASC_KEY_PATH="${ASC_KEY_PATH:-$HOME/.appstoreconnect/private_keys/AuthKey_${ASC_KEY_ID}.p8}"
[ -f "$ASC_KEY_PATH" ] || { echo "ASC key not found at $ASC_KEY_PATH"; exit 1; }
AUTH=(-allowProvisioningUpdates \
  -authenticationKeyPath "$ASC_KEY_PATH" \
  -authenticationKeyID "$ASC_KEY_ID" \
  -authenticationKeyIssuerID "$ASC_ISSUER_ID")

ARCHIVE=/tmp/FamiliarMac.xcarchive
EXPORT=/tmp/FamiliarMac-export
rm -rf "$ARCHIVE" "$EXPORT"

echo "== regenerate project =="
xcodegen >/dev/null

echo "== archive (macOS) =="
xcodebuild -project FamiliarAgent.xcodeproj -scheme FamiliarMac \
  -destination 'generic/platform=macOS' \
  -archivePath "$ARCHIVE" archive "${AUTH[@]}"

echo "== export (App Store) =="
# Manual signing (MacExportOptions pins Apple Distribution + the 3rd Party Mac Developer Installer
# cert + the Mac App Store profile). Do NOT pass the auth key here — that re-triggers cloud signing,
# which fails with a permission error; export reads the locally-installed profile instead.
xcodebuild -exportArchive -archivePath "$ARCHIVE" \
  -exportOptionsPlist tools/MacExportOptions.plist -exportPath "$EXPORT"

PKG=$(ls "$EXPORT"/*.pkg 2>/dev/null | head -1)
[ -n "$PKG" ] || { echo "no .pkg produced — export failed"; exit 1; }
echo "== upload $PKG =="
xcrun altool --upload-app --type macos --file "$PKG" \
  --apiKey "$ASC_KEY_ID" --apiIssuer "$ASC_ISSUER_ID"

echo "✓ uploaded — appears under App Store Connect → Familiar Console → TestFlight after processing."
