#!/usr/bin/env bash
# Refresh the pilot's copy of the exchange's dispatch deck (T-238 brick 3).
#
# The deck is Jeff's public content pack, `Content/market/events.json` in
# SpaceTrucker2196/ucf-exchange: every headline `/v1/news` can carry, with the
# effect behind it. whisker matches the feed against this copy, so a deck newer
# than ours journals its headlines as questions until this is re-run.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
src="${1:-$HOME/Development/ucf-exchange}"
from="$src/Content/market/events.json"
to="$here/crates/whisker/content/ucf-events.json"
[ -f "$from" ] || { echo "no deck at $from (pass the ucf-exchange checkout as \$1)" >&2; exit 1; }
cp "$from" "$to"
sha="$(git -C "$src" log -1 --format='%h %ad' --date=short -- Content/market/events.json 2>/dev/null || echo unknown)"
cat > "$here/crates/whisker/content/README.md" <<MD
# The dispatch deck

\`ucf-events.json\` is a verbatim copy of \`Content/market/events.json\` from
SpaceTrucker2196/ucf-exchange (last synced from commit $sha, $(date -u +%Y-%m-%d)).
\`crates/whisker/src/chain.rs\` reads \`/v1/news\` against it. Refresh with
\`tools/sync-ucf-deck.sh [path-to-ucf-exchange]\`; never edit by hand.
MD
echo "deck synced from $sha: $(python3 -c "import json;print(len(json.load(open('$to'))))") cards"
