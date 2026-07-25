#!/bin/bash
# build-core.sh — the embeddable familiar core for Apple shells (ADR-0009 Phase 0).
# Produces ../familiar-main/ios/FamiliarCore/: generated Swift bindings +
# FamiliarCore.xcframework (device + simulator static libs). Run from the repo root.
set -euo pipefail
cd "$(dirname "$0")/.."
OUT=../familiar-main/ios/FamiliarCore
GEN="$OUT/Generated"

echo "== host dylib (for binding generation) =="
cargo build -p familiar-core-ffi --release

echo "== swift bindings =="
rm -rf "$GEN" && mkdir -p "$GEN"
cargo run -p familiar-core-ffi --bin uniffi-bindgen -- generate \
  --library target/release/libfamiliar_core.dylib \
  --language swift --out-dir "$GEN"

echo "== device + simulator static libs =="
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
echo "✓ $OUT ready — link the xcframework + compile Generated/familiar_core.swift into the app"
