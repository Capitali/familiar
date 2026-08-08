#!/usr/bin/env bash
# Redeploy the lighthouse: push main, rebuild ON the box, restart the peer.
#
#   vps/deploy-lighthouse.sh [user@host]
#
# Runs from the Mac. The box builds its own binary (same posture as
# provision-lighthouse.sh — /opt/familiar-src is a shallow clone of origin/main),
# so nothing is cross-compiled and the deployed rev is provably what main says.
#
# Env:
#   LIGHTHOUSE   override the target (default root@134.209.168.50)

set -euo pipefail

TARGET="${1:-${LIGHTHOUSE:-root@134.209.168.50}}"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# Guard: deploy exactly what main is — no drift, no surprise WIP.
BRANCH=$(git rev-parse --abbrev-ref HEAD)
[ "$BRANCH" = "main" ] || { echo "✗ on '$BRANCH' — the lighthouse deploys main only" >&2; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "✗ working tree not clean — commit or stash first" >&2; exit 1; }

git push origin main
REV=$(git rev-parse HEAD)
echo "→ deploying $REV to $TARGET"

# Seed the standing roll (ADR-0020) if a local copy exists. standing.json is read by the
# SERVING node, and the lighthouse serves every off-LAN read — an unseeded roll there means
# every household device that reads via the lighthouse goes anonymous (default is deny).
# The file is untracked (it names this household's nodes); the deploy carries it so the roll
# on the box is always the one reviewed here. Never overwrites a hand-edited box copy silently:
# it is pushed to a staging path and installed only if different, with the old one kept.
ROLL="$REPO_ROOT/vps/standing.lighthouse.json"
if [ -f "$ROLL" ]; then
  python3 -c "import json;json.load(open('$ROLL'))" || { echo "✗ $ROLL is not valid JSON" >&2; exit 1; }
  scp -q "$ROLL" "$TARGET":/var/lib/familiar/familiar_data/standing.json.staged
  echo "→ standing roll staged"
fi

# shellcheck disable=SC2087  # REV expands locally on purpose
ssh "$TARGET" REV="$REV" 'bash -s' <<'REMOTE'
set -euo pipefail
. "$HOME/.cargo/env"

cd /opt/familiar-src
git fetch --depth 1 origin main
git reset --hard "$REV" 2>/dev/null || git reset --hard FETCH_HEAD
DEPLOYED=$(git rev-parse HEAD)
[ "$DEPLOYED" = "$REV" ] || { echo "✗ box checked out $DEPLOYED, expected $REV" >&2; exit 1; }

cargo build --release -p familiar-cli
install -m 0755 target/release/familiar /usr/local/bin/familiar

# Install the staged standing roll, if the deploy carried one (see above).
DATA=/var/lib/familiar/familiar_data
if [ -f "$DATA/standing.json.staged" ]; then
  if [ -f "$DATA/standing.json" ] && cmp -s "$DATA/standing.json.staged" "$DATA/standing.json"; then
    rm -f "$DATA/standing.json.staged"
  else
    [ -f "$DATA/standing.json" ] && cp "$DATA/standing.json" "$DATA/standing.json.prev"
    mv "$DATA/standing.json.staged" "$DATA/standing.json"
    chown familiar:familiar "$DATA/standing.json" 2>/dev/null || true
    echo "→ standing roll installed (previous kept as standing.json.prev)"
  fi
fi

# The lighthouse stands at the North Pole. A VPS has no honest place on the household's
# map — and without an asserted position it INHERITS one: `self_geo` falls back to the
# freshest device fix, so a visitor knocking from Beijing once dragged the lighthouse
# (and every unlocated node clustered on it) to Beijing. geo.json always wins over
# device fixes, so the pole is where it stays. Asserted here, not hand-edited on the box.
mkdir -p "$DATA/mesh"
printf '{"lat": 90.0, "lon": 0.0}\n' > "$DATA/mesh/geo.json"
chown familiar:familiar "$DATA/mesh/geo.json" 2>/dev/null || true

systemctl restart familiar-peer.service
sleep 3
systemctl is-active --quiet familiar-peer.service || {
  echo "✗ familiar-peer failed to come back:" >&2
  systemctl --no-pager --lines 20 status familiar-peer.service >&2
  exit 1
}

# The mesh port answering hello is the whole point of this box.
curl -ksf --max-time 5 https://127.0.0.1:47100/mesh/hello >/dev/null \
  || { echo "✗ service up but /mesh/hello not answering on :47100" >&2; exit 1; }

echo "✓ lighthouse on $DEPLOYED — familiar-peer active, /mesh/hello answering"
REMOTE
