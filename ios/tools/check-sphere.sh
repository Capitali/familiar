#!/bin/bash
# check-sphere.sh — does the console's JavaScript actually parse?
#
# Written after shipping build 98 with a `SyntaxError: Cannot declare a const variable twice`
# that took every client in the fleet down to a spinner. The check that was supposed to catch
# it could not, and the way it failed is worth keeping:
#
#   The page has four <script> blocks. Two are `src=` (EMPTY bodies — checking them proves
#   nothing), one is `type="importmap"` (JSON, not JS), and the fourth is `type="module"` and
#   holds all 200k characters of the application. The old check ran each block through
#   `new Function(body)`, which cannot parse ES module syntax — so the real code reported FAIL
#   on every run. Compared against a baseline that also said FAIL, that read as "unchanged,
#   safe". Two failures compared to each other and called a pass.
#
# So: extract the MODULE block specifically and parse it as a module. `jsc -m` gets past syntax
# and stops at module resolution (the bare `three` specifier an importmap resolves in a browser
# but jsc cannot) — so a SyntaxError means broken and a resolution TypeError means fine.
#
# Read the exit code, never the prose (CONTRIBUTING: the green bar).
set -euo pipefail

PAGE="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/MacApp/Resources/sphere/index.html}"
JSC=/System/Library/Frameworks/JavaScriptCore.framework/Versions/A/Helpers/jsc
test -x "$JSC" || { echo "✗ jsc not found at $JSC"; exit 1; }
test -f "$PAGE" || { echo "✗ no page at $PAGE"; exit 1; }

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

python3 - "$PAGE" "$TMP/mod.mjs" <<'PY'
import re, sys
src, out = sys.argv[1], sys.argv[2]
html = open(src).read()
mods = [m.group(2) for m in re.finditer(r'<script([^>]*)>(.*?)</script>', html, re.S)
        if 'type="module"' in m.group(1)]
if len(mods) != 1:
    print(f"✗ expected exactly one module script, found {len(mods)}")
    raise SystemExit(1)
body = mods[0]
if len(body) < 1000:
    print(f"✗ the module block is only {len(body)} chars — extraction is wrong, not the page")
    raise SystemExit(1)
open(out, "w").write(body)
print(f"  module block: {len(body)} chars")
PY

# jsc -m parses, then resolves. A SyntaxError is ours; a module-resolution TypeError is
# expected here and means the syntax is clean.
set +e
"$JSC" -m "$TMP/mod.mjs" > "$TMP/out" 2>&1
set -e
if grep -qi 'SyntaxError' "$TMP/out"; then
  echo "✗ the console's module does NOT parse:"
  grep -i -m3 -A1 'SyntaxError' "$TMP/out" | sed 's/^/    /'
  exit 1
fi
echo "✓ the console's module parses"
