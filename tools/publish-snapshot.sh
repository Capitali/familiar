#!/usr/bin/env bash
# Publish this repository to its PUBLIC remote as a single-commit snapshot.
#
#   sh tools/publish-snapshot.sh            # dry run: build it, show it, touch nothing
#   sh tools/publish-snapshot.sh --push     # force-push it as the public `main`
#
# WHY A SNAPSHOT RATHER THAN A PUSH. The working repository keeps a full history
# and a working record — the board, the development log, the partner drafts, the
# review transcripts. Those are how the work gets done; they are not the project.
# They also carry a decade of incidental detail about the people doing the work,
# which belongs to them and not to a reader. So the public sees the code and the
# architecture, whole and current, and nothing else.
#
# WHAT THIS IS NOT. It is not a way to hide changes. The published tree is exactly
# the working tree minus the paths named below, and anyone can diff two published
# snapshots. What is withheld is the process, never the product.
#
# Env:
#   PUBLIC_REMOTE   git remote or URL to publish to (default: the `public` remote)
#   BRANCH          branch to publish as (default: main)
set -euo pipefail

PUBLIC_REMOTE="${PUBLIC_REMOTE:-public}"
BRANCH="${BRANCH:-main}"
PUSH=no
[ "${1:-}" = "--push" ] && PUSH=yes

# Paths that never leave this repository. Adding one here is the ONLY way a path
# stays private; everything else in the tree is published.
WITHHELD=(
  coordination              # the board and the controller's log
  docs/DEVELOPMENT_LOG.md   # the narrative handoff trail
  docs/partners             # drafts written for other people's projects
  docs/reviews              # external review transcripts
  docs/factory-runs         # run records, content-digested, naming their requester
  docs/handoff              # machine-to-machine handoffs and their eval records
  .claude                   # assistant configuration, not project code
  FACTORY.md                # its argument is a walk through coordination/
  tasks                     # scratch planning
)

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
# Only TRACKED changes matter: the snapshot is built from the commit, so an
# untracked scratch file cannot leak into it — but an uncommitted edit would mean
# publishing something other than what HEAD says.
git diff --quiet && git diff --cached --quiet || {
  echo "tracked files have uncommitted changes; commit or stash first" >&2; exit 1; }

SRC_SHA="$(git rev-parse HEAD)"
STAGE="$(mktemp -d "${TMPDIR:-/tmp}/familiar-public.XXXXXX")"
trap 'rm -rf "$STAGE"' EXIT

git archive HEAD | (cd "$STAGE" && tar xf -)
for p in "${WITHHELD[@]}"; do rm -rf "${STAGE:?}/$p"; done

# The snapshot must not carry anything personal. This is a backstop, not the
# cleanup itself: the tree is kept clean in place, and this refuses to publish if
# that ever stops being true.
# (This script is excluded from its own scan: it necessarily spells the patterns
# it looks for, and publishing what it screens out is no secret.)
PRIVATE_RE="ian@|@river\.io|wildhorse|MacOnStick|Giiweo|Motorhorse"
HITS="$(grep -rIlE "$PRIVATE_RE" "$STAGE" --exclude=publish-snapshot.sh 2>/dev/null || true)"
if [ -n "$HITS" ]; then
  echo "REFUSING TO PUBLISH: the snapshot still carries private identifiers:" >&2
  echo "$HITS" >&2
  exit 1
fi

cd "$STAGE"
git init -q -b "$BRANCH"
git add -A
git -c user.name="$(git -C "$ROOT" config user.name)" \
    -c user.email="$(git -C "$ROOT" config user.email)" \
    commit -q -m "The Familiar

A self-hosted AI companion that runs on hardware its owner already has.

Published as a snapshot of the working repository: the code and the
architecture, current and whole. See docs/ARCHITECTURE.md to start, and
docs/SOUL.md for the constitution the guard enforces."

echo "staged $(git ls-files | wc -l | tr -d ' ') files from ${SRC_SHA:0:12}"
echo "withheld: ${WITHHELD[*]}"

if [ "$PUSH" = yes ]; then
  URL="$(git -C "$ROOT" remote get-url "$PUBLIC_REMOTE" 2>/dev/null || echo "$PUBLIC_REMOTE")"
  git push --force "$URL" "$BRANCH:$BRANCH"
  echo "published to $URL ($BRANCH)"
else
  echo "dry run; re-run with --push to publish. Staged at: $STAGE"
  trap - EXIT
fi
