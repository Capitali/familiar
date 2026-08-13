#!/bin/bash
# ship.sh <build-number> — the ONE way a build goes out. Absolute paths everywhere: three
# separate hand-rolled pipelines died to a leaked working directory (git pathspecs missed,
# /Applications/FamiliarMac.app was rm'd with the replacement path unset). Never again.
#
# Does, in order: verify the bump is real → commit it → universal Mac build → install to
# /Applications + refresh the AirDrop zip → iOS build → direct-install to the household
# devices → TestFlight upload. Each stage prints a marker; the first failure stops the ship.
set -euo pipefail

# Still absolute at runtime — derived from this script's own location, so the ship
# works from any checkout (wildhorse kept the repo in ~/Development, this Mac in ~/Projects).
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IOS="$REPO/ios"
BUILD="${1:?usage: ship.sh <build-number>}"
DEVICES=(20369E69-19C1-5DC1-B404-72A68D3DA0E9 EE750B79-EF86-514C-A14D-F2E4AD4ACBF8)

cd "$IOS"
python3 - "$BUILD" <<'EOF'
import re, sys
n = sys.argv[1]
p = open("project.yml").read()
p2 = re.sub(r'CURRENT_PROJECT_VERSION: "\d+"', f'CURRENT_PROJECT_VERSION: "{n}"', p)
open("project.yml", "w").write(p2)
assert p2.count(f'CURRENT_PROJECT_VERSION: "{n}"') == 2, "expected exactly two version spots"
EOF
xcodegen > /dev/null
grep -q "CURRENT_PROJECT_VERSION = $BUILD" FamiliarAgent.xcodeproj/project.pbxproj \
  || { echo "✗ pbxproj did not take build $BUILD"; exit 1; }
echo "✓ version $BUILD"

cd "$REPO"
git add ios/project.yml
git diff --cached --quiet || git commit -m "Build $BUILD

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push origin "$(git branch --show-current)" 2>&1 | tail -1
echo "✓ committed + pushed"

cd "$IOS"
mkdir -p build
# The Mac stage authenticates like the iOS stage below: a fresh machine has no signing
# certificate in its keychain, and the API session lets xcodebuild mint one on the spot.
# Build output survives in a log — a swallowed diagnostic once cost a ship a debugging round.
xcodebuild -project FamiliarAgent.xcodeproj -scheme FamiliarMac -configuration Release \
  -derivedDataPath build/mac-uni ARCHS="arm64 x86_64" ONLY_ACTIVE_ARCH=NO \
  -allowProvisioningUpdates \
  -authenticationKeyPath /Users/ian/.appstoreconnect/private_keys/AuthKey_SUZJSXVS25.p8 \
  -authenticationKeyID SUZJSXVS25 \
  -authenticationKeyIssuerID 69a6de82-89e3-47e3-e053-5b8c7c11a4d1 build \
  2>&1 | tee build/mac-build.log | grep -q "BUILD SUCCEEDED" \
  || { echo "✗ Mac build failed (ios/build/mac-build.log)"; exit 1; }
MACAPP="$IOS/build/mac-uni/Build/Products/Release/FamiliarMac.app"
test -d "$MACAPP" || { echo "✗ Mac app bundle missing"; exit 1; }
osascript -e 'quit app "FamiliarMac"' 2>/dev/null || true
sleep 2
rm -rf /Applications/FamiliarMac.app
cp -R "$MACAPP" /Applications/
open /Applications/FamiliarMac.app
ditto -c -k --keepParent "$MACAPP" "$HOME/Downloads/FamiliarMac-universal.zip"
echo "✓ Mac installed + zip refreshed"

# -allowProvisioningUpdates needs an authenticated session; the CLI has no Xcode account,
# so authenticate with the App Store Connect API key (same one testflight.sh uploads with).
# Without it, any NEW entitlement (push, build 69) fails the build on a stale team profile.
xcodebuild -project FamiliarAgent.xcodeproj -scheme FamiliarAgent -configuration Release \
  -destination 'generic/platform=iOS' -allowProvisioningUpdates \
  -authenticationKeyPath /Users/ian/.appstoreconnect/private_keys/AuthKey_SUZJSXVS25.p8 \
  -authenticationKeyID SUZJSXVS25 \
  -authenticationKeyIssuerID 69a6de82-89e3-47e3-e053-5b8c7c11a4d1 \
  -derivedDataPath build/ios-rel build \
  2>&1 | tee build/ios-build.log | grep -q "BUILD SUCCEEDED" \
  || { echo "✗ iOS build failed (ios/build/ios-build.log)"; exit 1; }
IOSAPP="$IOS/build/ios-rel/Build/Products/Release-iphoneos/FamiliarAgent.app"
for D in "${DEVICES[@]}"; do
  ok=""
  for try in 1 2 3; do
    if xcrun devicectl device install app --device "$D" "$IOSAPP" >/dev/null 2>&1; then
      ok=1; echo "✓ $D installed"; break
    fi
    sleep 5
  done
  [ -n "$ok" ] || echo "⚠ $D unreachable (TestFlight will cover it)"
done

ASC_KEY_ID=SUZJSXVS25 ASC_ISSUER_ID=69a6de82-89e3-47e3-e053-5b8c7c11a4d1 \
  "$IOS/tools/testflight.sh" 2>&1 | tail -2
# Release to the EXTERNAL testers once processing lands (add to the public-link group +
# beta review submission) — uploading alone strands them on the last-reviewed build.
# Backgrounded: processing takes 5-15 min and the ship shouldn't wait on Apple.
nohup python3 "$IOS/tools/tf_release.py" "$BUILD" >> /tmp/tf_release.log 2>&1 &
echo "✓ ship $BUILD complete (external release backgrounded — /tmp/tf_release.log)"
