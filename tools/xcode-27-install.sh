#!/bin/bash
# xcode-27-install.sh — install the Xcode 27 GA xip beside the beta and switch to it.
#
# Why: App Store Connect refuses uploads from Xcode 27 beta 27A5237l (error 90534) since
# 2026-09-16, and the Private Cloud Compute lane only compiles with a 27 SDK. The GA
# (27A266a, 2026-09-14) is the fix — but the download needs an Apple developer sign-in and the
# switch needs sudo, so this script does everything AFTER the download.
#
# Usage:  sudo bash tools/xcode-27-install.sh ~/Downloads/Xcode_27.xip
# The beta stays at /Applications/Xcode-27-beta.app until you delete it yourself.
set -euo pipefail
XIP="${1:?path to Xcode_27.xip}"
[ -f "$XIP" ] || { echo "no such file: $XIP" >&2; exit 1; }
[ "$(id -u)" -eq 0 ] || { echo "run with sudo (xcode-select and first-launch need it)" >&2; exit 1; }
REAL_USER="${SUDO_USER:?run under sudo, so SUDO_USER names the account that owns the download}"

echo "== expanding (this takes several minutes) =="
WORK=$(mktemp -d /private/tmp/xcode27.XXXX)
sudo -u "$REAL_USER" xip --expand "$XIP" -C "$WORK"
NEW=$(find "$WORK" -maxdepth 1 -name "Xcode*.app" | head -1)
[ -n "$NEW" ] || { echo "no Xcode.app in the archive" >&2; exit 1; }
VER=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$NEW/Contents/Info.plist")
BUILD=$(/usr/libexec/PlistBuddy -c 'Print :ProductBuildVersion' "$NEW/Contents/version.plist")
echo "archive carries Xcode $VER ($BUILD)"
case "$BUILD" in 27A5*) echo "that is a BETA build; App Store Connect will still refuse it" >&2; exit 1;; esac

echo "== moving the beta aside, installing the GA =="
if [ -d /Applications/Xcode.app ]; then
  OLD=$(/usr/libexec/PlistBuddy -c 'Print :ProductBuildVersion' /Applications/Xcode.app/Contents/version.plist)
  mv /Applications/Xcode.app "/Applications/Xcode-$OLD.app"
  echo "beta kept at /Applications/Xcode-$OLD.app"
fi
mv "$NEW" /Applications/Xcode.app
chown -R "$REAL_USER":staff /Applications/Xcode.app
xattr -dr com.apple.quarantine /Applications/Xcode.app || true

echo "== selecting, licensing, first launch =="
xcode-select -s /Applications/Xcode.app
xcodebuild -license accept
xcodebuild -runFirstLaunch
rm -rf "$WORK"

echo "== verify =="
xcodebuild -version
xcrun simctl list runtimes | grep -i "iOS 27" || echo "NOTE: no iOS 27 simulator runtime yet — Xcode > Settings > Components, or: xcodebuild -downloadPlatform iOS"
echo "✓ done — next: cd ~/Projects/familiar/ios && xcodegen generate && xcodebuild -scheme FamiliarAgent -destination 'generic/platform=iOS Simulator' build"
