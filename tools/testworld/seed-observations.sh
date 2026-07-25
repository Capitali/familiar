#!/usr/bin/env bash
# Seed sample domain observations into a running familiar via the loopback
# observe seam — instant musing material for tuning theories, no VM required.
#
#   tools/testworld/seed-observations.sh [observations.jsonl] [port]
#
# The seam is plain HTTP on loopback only (gossip_port + 1, default 47101);
# each line of the JSONL is one observation. Re-running re-records — the
# store keeps both, so seed once per tuning session, not on a loop.

set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
FILE="${1:-$HERE/observations.jsonl}"
PORT="${2:-47101}"
URL="http://127.0.0.1:$PORT/local/observe"

[ -f "$FILE" ] || { echo "✗ no such file: $FILE" >&2; exit 1; }

n=0
while IFS= read -r line; do
  [ -z "$line" ] && continue
  code=$(curl -s -o /dev/null -w '%{http_code}' -X POST --data "$line" "$URL")
  if [ "$code" != "200" ]; then
    echo "✗ observation rejected ($code): $line" >&2
    exit 1
  fi
  n=$((n + 1))
done < "$FILE"

echo "✓ seeded $n observation(s) into $URL"
echo "  the muse picks them up on its next due tick (fresh observer input muses next tick)"
