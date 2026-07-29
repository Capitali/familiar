#!/bin/bash
# vendor-sphere-assets.sh — re-download everything the Metal Sphere console loads, into
# MacApp/Resources/sphere/vendor/, so the page has NO runtime network dependency.
#
# Why this exists: the page used to pull three.js, the earth textures, qrcode-generator and the
# Google fonts from unpkg/gstatic at runtime. With no network there was no three.js at all, so
# the console rendered as a black window rather than a degraded one — and a shipping App Store
# app carried a hard dependency on two third-party CDNs. That is a plausible reading of the
# macOS build 41 review question ("how do we verify application functionality?").
#
# The sphere folder is a FOLDER REFERENCE in project.yml, so anything written here is bundled
# into both the iOS and macOS targets automatically — no project change needed.
#
# Run from anywhere:  ios/tools/vendor-sphere-assets.sh
set -euo pipefail
cd "$(dirname "$0")/../MacApp/Resources/sphere"

THREE_VER="0.160.0"
QRCODE_VER="1.4.4"
FONTS_CSS="https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@400;500;600;700&family=IBM+Plex+Mono:wght@400;500;600&display=swap"
# Google Fonts serves woff2 only to a UA it believes supports it; curl's default UA gets ttf.
UA="Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36"

rm -rf vendor
mkdir -p vendor/three vendor/textures vendor/fonts

echo "== three.js $THREE_VER =="
curl -fsSL "https://unpkg.com/three@${THREE_VER}/build/three.module.js" -o vendor/three/three.module.js
# The only addon the page imports. It resolves the bare specifier 'three', which the page's
# importmap points at the local build above.
curl -fsSL "https://unpkg.com/three@${THREE_VER}/examples/jsm/controls/OrbitControls.js" -o vendor/three/OrbitControls.js

echo "== qrcode-generator $QRCODE_VER =="
curl -fsSL "https://unpkg.com/qrcode-generator@${QRCODE_VER}/qrcode.js" -o vendor/qrcode.js

echo "== earth textures =="
for f in earth-blue-marble.jpg earth-topology.png earth-water.png earth-night.jpg; do
  curl -fsSL "https://unpkg.com/three-globe/example/img/$f" -o "vendor/textures/$f"
done

echo "== fonts =="
curl -fsSL -A "$UA" "$FONTS_CSS" -o vendor/fonts/fonts.css
(
  cd vendor/fonts
  for u in $(grep -oE 'https://fonts.gstatic.com/[^)]+' fonts.css | sort -u); do
    curl -fsSL "$u" -o "$(basename "$u")"
  done
  # Rewrite the absolute gstatic URLs to bare filenames alongside this stylesheet.
  python3 - <<'PY'
import re, pathlib
p = pathlib.Path('fonts.css')
css = re.sub(r'https://fonts\.gstatic\.com/\S*?/([^/)]+\.woff2)', r'\1', p.read_text())
p.write_text(css)
PY
)

echo
echo "== verify: nothing reaches the network =="
if grep -rqoE 'https?://' index.html vendor/fonts/fonts.css; then
  echo "FAIL — external references remain:"
  grep -rnoE 'https?://[a-zA-Z0-9./_@-]+' index.html vendor/fonts/fonts.css
  exit 1
fi
echo "ok — no external references in index.html or fonts.css"
du -sh vendor
