#!/bin/bash
# ship.sh <build-number> — the ONE way a build goes out. Absolute paths everywhere: three
# separate hand-rolled pipelines died to a leaked working directory (git pathspecs missed,
# /Applications/FamiliarMac.app was rm'd with the replacement path unset). Never again.
#
# Does, in order: verify the bump is real → commit it → universal Mac build → install to
# /Applications + refresh the AirDrop zip → iOS build → direct-install to the paired
# devices → TestFlight upload. Each stage prints a marker; the first failure stops the ship.
#
# Needs an App Store Connect API key (see TESTFLIGHT.md):
#   ASC_KEY_ID=XXXXXXXXXX ASC_ISSUER_ID=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx \
#   ASC_APP_ID=NNNNNNNNNN ASC_BETA_GROUP=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx ./tools/ship.sh 123
# Optionally FAMILIAR_DEVICE_UDIDS="<udid> <udid>" to add devices the Mac cannot discover.
set -euo pipefail

: "${ASC_KEY_ID:?set ASC_KEY_ID (App Store Connect API key id)}"
: "${ASC_ISSUER_ID:?set ASC_ISSUER_ID (App Store Connect issuer id)}"
: "${ASC_APP_ID:?set ASC_APP_ID (the App Store Connect app id)}"
: "${ASC_BETA_GROUP:?set ASC_BETA_GROUP (the public-link beta group id)}"
ASC_KEY_PATH="${ASC_KEY_PATH:-$HOME/.appstoreconnect/private_keys/AuthKey_${ASC_KEY_ID}.p8}"
[ -f "$ASC_KEY_PATH" ] || { echo "ASC key not found at $ASC_KEY_PATH"; exit 1; }

# Still absolute at runtime — derived from this script's own location, so the ship
# works from any checkout (the repo lives under a different parent on each Mac here).
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IOS="$REPO/ios"
BUILD="${1:?usage: ship.sh <build-number>}"
# Extra install targets, for a device this Mac cannot discover on its own. The discovered
# set below is the real source of truth; this is only a union with it.
DEVICES="${FAMILIAR_DEVICE_UDIDS:-}"

cd "$IOS"
python3 - "$BUILD" <<'EOF'
import re, sys
n = sys.argv[1]
p = open("project.yml").read()
# Bump the app's two spots — the shared base and the Mac target, which always carry the
# same number. There is exactly one app in this project now (the ship's computer moved to
# its own repo, taking its own release cadence with it), so every version spot is this
# app's; the count is asserted afterwards so a project.yml that grows a third one fails
# loudly here rather than shipping a silently renumbered target.
p2 = re.sub(r'CURRENT_PROJECT_VERSION: "\d+"', f'CURRENT_PROJECT_VERSION: "{n}"', p)
open("project.yml", "w").write(p2)
assert p2.count(f'CURRENT_PROJECT_VERSION: "{n}"') == 2, "expected exactly two version spots"
EOF
# The console's JavaScript must parse before anything is built. Build 98 shipped a
# SyntaxError that took every client in the fleet to a spinner, because the old check ran
# `new Function()` over script blocks that were either empty (`src=`) or not plain JS
# (`type="module"`), reported FAIL on the real code every time, and was compared against a
# baseline that also said FAIL. Read the exit code.
"$IOS/tools/check-sphere.sh"
xcodegen > /dev/null
grep -q "CURRENT_PROJECT_VERSION = $BUILD" FamiliarAgent.xcodeproj/project.pbxproj \
  || { echo "✗ pbxproj did not take build $BUILD"; exit 1; }
echo "✓ version $BUILD"

cd "$REPO"
git add ios/project.yml
git diff --cached --quiet || git commit -m "Build $BUILD

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
# Rebase before pushing. The bump commit races anyone else landing on main, and
# a ship that dies between "commit the version" and "build it" leaves the number
# claimed in git and nothing on TestFlight — which is exactly what happened twice
# on 2026-09-05 while the other Mac was landing its own commits. Concurrent work
# on this repo is the normal condition now, not the exception.
git pull --rebase --autostash --quiet origin "$(git branch --show-current)"
git push origin "$(git branch --show-current)" 2>&1 | tail -1
echo "✓ committed + pushed"

cd "$IOS"
mkdir -p build
# The Mac stage authenticates like the iOS stage below: a fresh machine has no signing
# certificate in its keychain, and the API session lets xcodebuild mint one on the spot.
# Build output survives in a log — a swallowed diagnostic once cost a ship a debugging round.
# `pipefail` makes the branch follow xcodebuild/tee exit status; log prose is never a success oracle.
if ! xcodebuild -project FamiliarAgent.xcodeproj -scheme FamiliarMac -configuration Release \
  -derivedDataPath build/mac-uni ARCHS="arm64 x86_64" ONLY_ACTIVE_ARCH=NO \
  -allowProvisioningUpdates \
  -authenticationKeyPath "$ASC_KEY_PATH" \
  -authenticationKeyID "$ASC_KEY_ID" \
  -authenticationKeyIssuerID "$ASC_ISSUER_ID" build \
  2>&1 | tee build/mac-build.log; then
  echo "✗ Mac build failed (ios/build/mac-build.log)"
  exit 1
fi
MACAPP="$IOS/build/mac-uni/Build/Products/Release/FamiliarMac.app"
test -d "$MACAPP" || { echo "✗ Mac app bundle missing"; exit 1; }
osascript -e 'quit app "FamiliarMac"' 2>/dev/null || true
sleep 2
rm -rf /Applications/FamiliarMac.app
cp -R "$MACAPP" /Applications/
# Relaunching the console is a courtesy, not a stage of the ship. `open` returns -600
# (procNotFound) when there is no GUI session to launch into — a closed lid was enough — and
# under `set -e` that aborted build 93 AFTER the Mac app was already installed but BEFORE the
# iOS build and the TestFlight upload. A cosmetic step must never be able to kill a ship.
open /Applications/FamiliarMac.app || echo "⚠ could not relaunch the console (no GUI session?) — install still good"
ditto -c -k --keepParent "$MACAPP" "$HOME/Downloads/FamiliarMac-universal.zip"
echo "✓ Mac installed + zip refreshed"

# -allowProvisioningUpdates needs an authenticated session; the CLI has no Xcode account,
# so authenticate with the App Store Connect API key (same one testflight.sh uploads with).
# Without it, any NEW entitlement (push, build 69) fails the build on a stale team profile.
if ! xcodebuild -project FamiliarAgent.xcodeproj -scheme FamiliarAgent -configuration Release \
  -destination 'generic/platform=iOS' -allowProvisioningUpdates \
  -authenticationKeyPath "$ASC_KEY_PATH" \
  -authenticationKeyID "$ASC_KEY_ID" \
  -authenticationKeyIssuerID "$ASC_ISSUER_ID" \
  -derivedDataPath build/ios-rel build \
  2>&1 | tee build/ios-build.log; then
  echo "✗ iOS build failed (ios/build/ios-build.log)"
  exit 1
fi
IOSAPP="$IOS/build/ios-rel/Build/Products/Release-iphoneos/FamiliarAgent.app"

# Direct install to the devices paired with this Mac, so a build is usable before Apple has
# finished beta review. Two things used to make this stage useless in silence:
#
#   1. The device list was HARDCODED. UDIDs port between Macs in this script; the PAIRING
#      does not — it lives in the Mac's trust store. On a fresh Mac every UDID is a stranger,
#      and a newly added device is never installed to at all.
#   2. The failure was sent to /dev/null, so ten consecutive builds printed "unreachable"
#      and never once said whether the device was asleep, unpaired, untrusted, or simply not
#      there. "Unreachable" that cannot say why is not a diagnosis.
#
# So: DISCOVER what is actually paired, union it with the configured list, and when an
# install fails, print what the tool said.
# shellcheck disable=SC2016  # the single quotes are deliberate: the shell must not touch the Python
DISCOVERED=$(xcrun devicectl list devices --json-output /tmp/familiar-devices.json >/dev/null 2>&1 \
  && python3 -c '
import json
try:
    devs = json.load(open("/tmp/familiar-devices.json"))["result"]["devices"]
except Exception:
    raise SystemExit
for d in devs:
    # `sameMachine` is how a simulator presents; a real handset is wired or on the network.
    if d.get("connectionProperties", {}).get("transportType") == "sameMachine":
        continue
    print(d.get("hardwareProperties", {}).get("udid", ""))
' 2>/dev/null || true)

TARGETS=$(printf '%s\n%s\n' "$DEVICES" "$DISCOVERED" | tr ' ' '\n' | sed '/^$/d' | sort -u)
if [ -z "$(printf '%s' "$DISCOVERED")" ]; then
  echo "⚠ no physical device is paired with this Mac (devicectl sees simulators only)."
  echo "  Pair once per device: connect by USB, tap Trust on the device, enter its passcode."
  echo "  Then Xcode → Window → Devices and Simulators → ‘Connect via network’ to keep it."
fi
for D in $TARGETS; do
  ok=""; why=""
  for try in 1 2 3; do
    if why=$(xcrun devicectl device install app --device "$D" "$IOSAPP" 2>&1); then
      ok=1; echo "✓ $D installed"; break
    fi
    sleep 5
  done
  if [ -z "$ok" ]; then
    # The last line of devicectl's complaint is the actionable one.
    echo "⚠ $D not installed — $(printf '%s' "$why" | tail -3 | tr '\n' ' ' | cut -c1-200)"
    echo "  (TestFlight will still cover it)"
  fi
done

ASC_KEY_ID="$ASC_KEY_ID" ASC_ISSUER_ID="$ASC_ISSUER_ID" ASC_KEY_PATH="$ASC_KEY_PATH" \
  "$IOS/tools/testflight.sh" 2>&1 | tail -2
# Release to the EXTERNAL testers once processing lands (add to the public-link group +
# beta review submission) — uploading alone strands them on the last-reviewed build.
# Backgrounded: processing takes 5-15 min and the ship shouldn't wait on Apple.
ASC_KEY_ID="$ASC_KEY_ID" ASC_ISSUER_ID="$ASC_ISSUER_ID" ASC_KEY_PATH="$ASC_KEY_PATH" \
  ASC_APP_ID="$ASC_APP_ID" ASC_BETA_GROUP="$ASC_BETA_GROUP" \
  nohup python3 "$IOS/tools/tf_release.py" "$BUILD" >> /tmp/tf_release.log 2>&1 &
echo "✓ ship $BUILD complete (external release backgrounded — /tmp/tf_release.log)"
