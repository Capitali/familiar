#!/bin/bash
# ship-ucf.sh <build-number> — archive → export (App Store) → upload UCF Familiar to TestFlight.
# The standalone ship's computer (scheme UCFFamiliar, bundle io.river.familiar.ucf). Needs the App
# Store Connect record for that bundle id (Ian's act, once) and the same ASC key ship.sh uses.
# Xcode must be one App Store Connect accepts (26.x until an Xcode 27 RC exists).
set -euo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IOS="$REPO/ios"
BUILD="${1:?usage: ship-ucf.sh <build-number>}"
ASC_KEY_ID="${ASC_KEY_ID:-SUZJSXVS25}"
ASC_ISSUER_ID="${ASC_ISSUER_ID:-69a6de82-89e3-47e3-e053-5b8c7c11a4d1}"
ASC_KEY_PATH="${ASC_KEY_PATH:-$HOME/.appstoreconnect/private_keys/AuthKey_${ASC_KEY_ID}.p8}"
[ -f "$ASC_KEY_PATH" ] || { echo "ASC key not found at $ASC_KEY_PATH"; exit 1; }
cd "$IOS"
python3 - "$BUILD" <<'PY'
import re, sys
n = sys.argv[1]
p = open("project.yml").read()
# UCFFamiliar's own version line (the block's CURRENT_PROJECT_VERSION), not the Familiar app's.
head, tail = p.split("  UCFFamiliar:\n", 1)
tail = re.sub(r'CURRENT_PROJECT_VERSION: "\d+"', f'CURRENT_PROJECT_VERSION: "{n}"', tail, count=1)
open("project.yml", "w").write(head + "  UCFFamiliar:\n" + tail)
PY
xcodegen > /dev/null
echo "✓ UCF Familiar build $BUILD"
cd "$REPO"
git add ios/project.yml
git diff --cached --quiet || git commit -m "UCF Familiar build $BUILD
Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
git push origin "$(git branch --show-current)" 2>&1 | tail -1
cd "$IOS"
ARCHIVE=/tmp/UCFFamiliar.xcarchive
EXPORT=/tmp/UCFFamiliar-export
rm -rf "$ARCHIVE" "$EXPORT"
xcodebuild -project FamiliarAgent.xcodeproj -scheme UCFFamiliar -configuration Release \
  -destination 'generic/platform=iOS' -archivePath "$ARCHIVE" archive \
  -allowProvisioningUpdates -authenticationKeyPath "$ASC_KEY_PATH" \
  -authenticationKeyID "$ASC_KEY_ID" -authenticationKeyIssuerID "$ASC_ISSUER_ID"
# App Store export with automatic signing for the new bundle id (its profile is minted on the fly).
cat > /tmp/UCFFamiliar-export.plist <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>method</key><string>app-store-connect</string>
  <key>teamID</key><string>8GHXL328AR</string>
  <!-- Manual signing, as ship.sh learned the hard way: this key cannot do cloud-managed
       distribution ("Cloud signing permission error"), so we pin the Apple Distribution
       cert and the App Store profile created for this bundle through the ASC API. -->
  <key>signingStyle</key><string>manual</string>
  <key>signingCertificate</key><string>Apple Distribution</string>
  <key>provisioningProfiles</key><dict>
    <key>io.river.familiar.ucf</key><string>UCF Familiar AppStore io.river.familiar.ucf</string>
  </dict>
  <key>uploadSymbols</key><true/>
</dict></plist>
PLIST
xcodebuild -exportArchive -archivePath "$ARCHIVE" -exportOptionsPlist /tmp/UCFFamiliar-export.plist -exportPath "$EXPORT" \
  -allowProvisioningUpdates -authenticationKeyPath "$ASC_KEY_PATH" \
  -authenticationKeyID "$ASC_KEY_ID" -authenticationKeyIssuerID "$ASC_ISSUER_ID"
IPA=$(ls "$EXPORT"/*.ipa | head -1)
xcrun altool --upload-app --type ios --file "$IPA" --apiKey "$ASC_KEY_ID" --apiIssuer "$ASC_ISSUER_ID"
echo "✓ UCF Familiar $BUILD uploaded — TestFlight after processing"
