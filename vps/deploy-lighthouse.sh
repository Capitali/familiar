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
