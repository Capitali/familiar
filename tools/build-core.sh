#!/bin/bash
# build-core.sh — the embeddable familiar core for Apple shells (ADR-0009 Phase 0).
# Produces ios/FamiliarCore/: generated Swift bindings + FamiliarCore.xcframework
# (device + simulator static libs). Run from the repo root.
set -euo pipefail
cd "$(dirname "$0")/.."
OUT=ios/FamiliarCore
GEN="$OUT/Generated"

# The core's minimum OS is the app's declared floor (ADR-0046) — pinned here so the
# archive can never silently carry whatever the building machine's SDK defaulted to
# (the 2026-08-25 review found 26.5- and 14.0-min objects inside one archive). Both
# rustc's Apple targets and cc-built C objects honor this variable.
FLOOR=26.0
export IPHONEOS_DEPLOYMENT_TARGET="$FLOOR"

echo "== host dylib (for binding generation) =="
cargo build -p familiar-core-ffi --release

echo "== swift bindings =="
rm -rf "$GEN" && mkdir -p "$GEN"
cargo run -p familiar-core-ffi --bin uniffi-bindgen -- generate \
  --library target/release/libfamiliar_core.dylib \
  --language swift --out-dir "$GEN"

echo "== device + simulator static libs (min iOS $FLOOR) =="
cargo build -p familiar-core-ffi --release --target aarch64-apple-ios
cargo build -p familiar-core-ffi --release --target aarch64-apple-ios-sim

echo "== xcframework =="
HDR=/tmp/familiar-core-headers
rm -rf "$HDR" && mkdir -p "$HDR"
cp "$GEN"/familiar_coreFFI.h "$HDR"/
cp "$GEN"/familiar_coreFFI.modulemap "$HDR"/module.modulemap
rm -rf "$OUT/FamiliarCore.xcframework"
xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libfamiliar_core.a -headers "$HDR" \
  -library target/aarch64-apple-ios-sim/release/libfamiliar_core.a -headers "$HDR" \
  -output "$OUT/FamiliarCore.xcframework"

echo "== verify: no object in either slice requires newer than iOS $FLOOR =="
# Objects OLDER than the floor are fine (Rust ships its precompiled std at the
# toolchain's own minimum — those objects load anywhere at or above it). What must
# never happen is an object NEWER than the floor: that is the 26.5-in-a-26.0-app
# defect the 2026-08-25 review caught, and the linker only warns instead of failing.
for lib in "$OUT"/FamiliarCore.xcframework/ios-arm64/libfamiliar_core.a \
           "$OUT"/FamiliarCore.xcframework/ios-arm64-simulator/libfamiliar_core.a; do
  stray=$(otool -l "$lib" | awk -v floor="$FLOOR" \
    '/minos/ { if ($2 + 0 > floor + 0) print $2 }' | sort -u)
  if [ -n "$stray" ]; then
    echo "✗ $lib carries objects requiring newer than iOS $FLOOR: $stray" >&2
    exit 1
  fi
done
echo "✓ $OUT ready — link the xcframework + compile Generated/familiar_core.swift into the app"
