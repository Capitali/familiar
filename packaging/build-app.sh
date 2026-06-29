#!/usr/bin/env bash
# build-app.sh — assemble (and sign) Familiar.app from a release build.
#
# Produces dist/Familiar.app, a menu-bar accessory bundle whose entry point is the marble.
# All four binaries live in Contents/MacOS together, so the marble's sibling-resolution finds
# the Glass, the familiar daemon, and the familiar-eye camera helper inside the bundle — and,
# crucially, the macOS camera grant (TCC) attaches to *this bundle's* identity rather than to
# whatever terminal launched a build.
#
# Signing: pass a Developer ID Application identity as $APP_IDENTITY for a stable,
# notarizable signature; otherwise it falls back to an ad-hoc signature ("-"), which is fine
# for running locally but gives the bundle a new identity on each rebuild (so the camera grant
# must be re-approved after a rebuild — see packaging/README.md). Either way the bundle is
# signed with the hardened runtime + camera entitlement, so the same artifact is what gets
# notarized once a real identity is supplied.
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"
APP="$ROOT/dist/Familiar.app"
MACOS="$APP/Contents/MacOS"
ENTITLEMENTS="$ROOT/packaging/entitlements.plist"
BINS=(marble glass familiar familiar-eye)
APP_IDENTITY="${APP_IDENTITY:-${SIGN_IDENTITY:--}}"  # default: ad-hoc ("-")

echo "==> building release binaries"
cargo build --release

# familiar-eye is compiled by the vision crate's build.rs during the release build; verify it.
if [[ ! -x "$ROOT/target/release/familiar-eye" ]]; then
  echo "!! target/release/familiar-eye missing — is the Swift toolchain (swiftc) installed?" >&2
  echo "   The app will build without camera capture." >&2
fi

echo "==> assembling $APP"
rm -rf "$APP"
mkdir -p "$MACOS"
cp "$ROOT/packaging/Info.plist" "$APP/Contents/Info.plist"
for b in "${BINS[@]}"; do
  if [[ -x "$ROOT/target/release/$b" ]]; then
    cp "$ROOT/target/release/$b" "$MACOS/$b"
  else
    echo "   (skipping $b — not built)" >&2
  fi
done

# Strip extended attributes (quarantine, Finder info) so they don't become AppleDouble "._"
# files in the pkg payload or trip up signing/notarization.
xattr -cr "$APP"

echo "==> signing (identity: $APP_IDENTITY)"
# Hardened runtime + camera entitlement so the bundle is notarizable as-is. --deep seals the
# helper executables beside the main one in Contents/MacOS (glass, familiar, familiar-eye).
# A real identity gets a secure timestamp; ad-hoc ("-") cannot timestamp, so we skip it there.
SIGN_ARGS=(--force --deep --options runtime --entitlements "$ENTITLEMENTS")
if [[ "$APP_IDENTITY" != "-" ]]; then
  SIGN_ARGS+=(--timestamp)
fi
codesign "${SIGN_ARGS[@]}" --sign "$APP_IDENTITY" "$APP"

echo "==> verifying"
codesign --verify --deep --strict --verbose=2 "$APP"
echo
echo "Built: $APP"
if [[ "$APP_IDENTITY" == "-" ]]; then
  echo "Note:  ad-hoc signed — runs locally; set APP_IDENTITY='Developer ID Application: …' to notarize."
fi
