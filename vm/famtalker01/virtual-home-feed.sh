#!/usr/bin/env bash
# Post a changed virtual-home snapshot through the daemon's ordinary loopback seam.

set -euo pipefail

STATE_DIR="${VIRTUAL_HOME_STATE_DIR:-/var/lib/familiar/virtual-home}"
PROGRAM="${VIRTUAL_HOME_PROGRAM:-/usr/local/lib/familiar/virtual_home.py}"
PORT="${FAMILIAR_LOCAL_PORT:-47101}"
URL="http://127.0.0.1:${PORT}/local/observe"
CURSOR="$STATE_DIR/feed.sha256"
mkdir -p "$STATE_DIR"
SNAPSHOT="$(mktemp "$STATE_DIR/feed.XXXXXX")"
trap 'rm -f "$SNAPSHOT"' EXIT

python3 "$PROGRAM" --state-dir "$STATE_DIR" observations > "$SNAPSHOT"
DIGEST="$(sha256sum "$SNAPSHOT" | cut -d ' ' -f 1)"
if [ -f "$CURSOR" ] && [ "$(tr -d '[:space:]' < "$CURSOR")" = "$DIGEST" ]; then
  exit 0
fi

while IFS= read -r observation; do
  [ -n "$observation" ] || continue
  CODE="$(curl -sS -o /dev/null -w '%{http_code}' \
    -H 'Content-Type: application/json' --data-binary "$observation" "$URL")"
  if [ "$CODE" != "200" ]; then
    echo "virtual-home observation rejected by $URL ($CODE)" >&2
    exit 1
  fi
done < "$SNAPSHOT"

CURSOR_TMP="$CURSOR.tmp"
printf '%s\n' "$DIGEST" > "$CURSOR_TMP"
mv "$CURSOR_TMP" "$CURSOR"
