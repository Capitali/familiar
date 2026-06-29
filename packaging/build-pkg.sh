#!/usr/bin/env bash
# build-pkg.sh — turn dist/Familiar.app into a double-click installer.
#
# Produces dist/Familiar-<version>.pkg: installs Familiar.app to /Applications and runs the
# postinstall (sets up the launchd agents + per-user data dir). Signing and notarization are
# env-gated, so this builds a working local .pkg today and a distributable one once you have
# an Apple Developer identity:
#   INSTALLER_IDENTITY="Developer ID Installer: Your Name (TEAMID)"   sign the pkg
#   NOTARY_PROFILE="familiar-notary"                                  notarytool credential profile
# (store the latter once with: xcrun notarytool store-credentials familiar-notary …)
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"
VERSION="0.1.0"
APP="$ROOT/dist/Familiar.app"
PKG_ID="io.river.familiar"
COMPONENT="$ROOT/dist/Familiar-component.pkg"
OUT="$ROOT/dist/Familiar-$VERSION.pkg"
INSTALLER_IDENTITY="${INSTALLER_IDENTITY:-}"
NOTARY_PROFILE="${NOTARY_PROFILE:-}"

[[ -d "$APP" ]] || { echo "build the app first: packaging/build-app.sh" >&2; exit 1; }
chmod +x "$ROOT/packaging/scripts/postinstall"

echo "==> pkgbuild (component, with postinstall)"
pkgbuild --component "$APP" \
  --install-location /Applications \
  --scripts "$ROOT/packaging/scripts" \
  --identifier "$PKG_ID" \
  --version "$VERSION" \
  "$COMPONENT"

echo "==> productbuild (product archive)"
PB_ARGS=(--package "$COMPONENT" --identifier "$PKG_ID" --version "$VERSION")
if [[ -n "$INSTALLER_IDENTITY" ]]; then
  PB_ARGS+=(--sign "$INSTALLER_IDENTITY")
fi
productbuild "${PB_ARGS[@]}" "$OUT"
rm -f "$COMPONENT"
[[ -z "$INSTALLER_IDENTITY" ]] && \
  echo "Note: unsigned pkg (set INSTALLER_IDENTITY='Developer ID Installer: …' to sign)."

if [[ -n "$NOTARY_PROFILE" ]]; then
  echo "==> notarizing (profile: $NOTARY_PROFILE) — this waits for Apple"
  xcrun notarytool submit "$OUT" --keychain-profile "$NOTARY_PROFILE" --wait
  echo "==> stapling"
  xcrun stapler staple "$OUT"
fi

echo
echo "Built: $OUT"
echo "Install (sets up the menu bar + boot persistence): open '$OUT'"
