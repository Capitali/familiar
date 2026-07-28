#!/usr/bin/env bash
# Relay device-oracle consults between this hub's queue and the lighthouse's (ADR-0014).
#
#   tools/consult-bridge.sh [user@host]
#
# Why this exists: a device answers consults it pulls from whatever host it read its
# worldview from. Off-LAN — an RV network, cellular — that host is the lighthouse, while
# the lab campaign queues to the hub it runs on. The prompts and the device end up looking
# at two different queues and every treatment cell times out.
#
# This is the stopgap, not the design. The real fix is the lighthouse brokering consults
# over the mesh seam so no out-of-band copy is needed; until that ships, this keeps the two
# queues in sync over ssh so a campaign on the hub can reach a device on the lighthouse.
#
# Transfers go over tar rather than rsync: the lighthouse has no rsync, and these are a
# handful of small JSON files on one multiplexed connection, so there is nothing for a
# delta protocol to save.
#
# Env:
#   LIGHTHOUSE     target (default root@134.209.168.50)
#   LOCAL_QUEUE    hub queue dir (default the daemon's)
#   REMOTE_QUEUE   lighthouse queue dir
#   INTERVAL       poll seconds (default 2)

set -uo pipefail

TARGET="${1:-${LIGHTHOUSE:-root@134.209.168.50}}"
LOCAL_QUEUE="${LOCAL_QUEUE:-$HOME/Library/Application Support/Familiar/data/llm/device-queue}"
REMOTE_QUEUE="${REMOTE_QUEUE:-/var/lib/familiar/familiar_data/llm/device-queue}"
INTERVAL="${INTERVAL:-2}"

mkdir -p "$LOCAL_QUEUE"

# Exactly one bridge at a time. Two instances race: both push the same prompt, and one can
# pull an answer home while the other still has the prompt listed as un-sent and re-sends it.
# mkdir is the atomic test-and-set that works everywhere. (Note for anyone restarting this:
# `pkill -f consult-bridge` also matches the shell running that very command — kill by pid.)
LOCK="${TMPDIR:-/tmp}/consult-bridge.lock"
if ! mkdir "$LOCK" 2>/dev/null; then
  if [ -f "$LOCK/pid" ] && kill -0 "$(cat "$LOCK/pid")" 2>/dev/null; then
    echo "✗ already bridging (pid $(cat "$LOCK/pid"))" >&2; exit 1
  fi
  rm -f "$LOCK/pid"; rmdir "$LOCK" 2>/dev/null; mkdir "$LOCK" 2>/dev/null || {
    echo "✗ cannot take lock $LOCK" >&2; exit 1; }
fi
echo $$ > "$LOCK/pid"

# One multiplexed ssh connection for the whole run — a new handshake every 2s would cost
# more than the relay itself and would look like a brute-force to fail2ban.
CTL="${TMPDIR:-/tmp}/consult-bridge-$$.sock"
cleanup() {
  ssh -o ControlPath="$CTL" -O exit "$TARGET" 2>/dev/null
  rm -f "$LOCK/pid"; rmdir "$LOCK" 2>/dev/null
}
trap cleanup EXIT INT TERM

SSH=(ssh -o ControlMaster=auto -o ControlPath="$CTL" -o ControlPersist=60
     -o ConnectTimeout=10 -o BatchMode=yes)

"${SSH[@]}" "$TARGET" "mkdir -p '$REMOTE_QUEUE'" || {
  echo "✗ cannot reach $TARGET" >&2; exit 1
}
echo "→ bridging '$LOCAL_QUEUE' ⇄ $TARGET:$REMOTE_QUEUE every ${INTERVAL}s"

# Names only, one per line, empty when the glob matches nothing.
local_names() {
  local pat="$1" f b
  for f in "$LOCAL_QUEUE"/$pat; do
    [ -e "$f" ] || continue
    b=$(basename "$f")
    case "$b" in ._*) continue ;; esac   # AppleDouble sidecars are not consults
    printf '%s\n' "$b"
  done
}

while :; do
  # What the lighthouse currently holds, in one round trip.
  remote=$("${SSH[@]}" "$TARGET" "cd '$REMOTE_QUEUE' 2>/dev/null || exit 0
    ls *.prompt.json 2>/dev/null
    echo '--'
    ls *.answer.json 2>/dev/null" 2>/dev/null)
  remote_prompts=$(printf '%s\n' "$remote" | sed -n '1,/^--$/p' | grep -v '^--$')
  remote_answers=$(printf '%s\n' "$remote" | sed -n '/^--$/,$p' | grep -v '^--$')

  # A prompt that already has an answer here is finished, whoever cleared it upstream. The
  # answering daemon clears the prompt from *its* queue in store_answer, so without this the
  # hub's copy would look un-sent on the next cycle and be pushed — and answered — forever.
  for a in $(local_names '*.answer.json'); do
    rm -f "$LOCAL_QUEUE/${a%.answer.json}.prompt.json"
  done

  mine=$(local_names '*.prompt.json')

  # 1. Push prompts the lighthouse has not seen. Never re-send one it already has: rewriting
  #    a prompt the device is chewing on resets its mtime and hides it from the TTL sweep.
  new=$(comm -23 <(printf '%s\n' "$mine" | sort) <(printf '%s\n' "$remote_prompts" | sort) | grep -v '^$')
  # Sent one at a time over `cat` rather than in a tar stream: macOS tar ships each file's
  # xattrs as a sidecar "._name" (COPYFILE_DISABLE does not suppress it here), and those
  # sidecars land in the remote queue, match the daemon's *.prompt.json glob, and fail to
  # parse as consults. A prompt is a few hundred bytes on an already-open connection, so
  # there is nothing to batch for.
  for f in $new; do
    "${SSH[@]}" "$TARGET" \
      "cat > '$REMOTE_QUEUE/$f.part' && chown familiar-svc:familiar-svc '$REMOTE_QUEUE/$f.part' && mv '$REMOTE_QUEUE/$f.part' '$REMOTE_QUEUE/$f'" \
      < "$LOCAL_QUEUE/$f" >/dev/null 2>&1
  done

  # 2. Bring answers home and clear them there in the same command, so an answer is
  #    delivered exactly once — the adapter deletes its local copy after reading it, and a
  #    lingering remote copy would resurrect it on the next cycle.
  if [ -n "$remote_answers" ]; then
    list=$(printf '%s\n' "$remote_answers" | tr '\n' ' ')
    "${SSH[@]}" "$TARGET" "cd '$REMOTE_QUEUE' && tar cf - $list && rm -f $list" 2>/dev/null \
      | tar xf - -C "$LOCAL_QUEUE" 2>/dev/null
  fi

  # 3. Retract prompts the hub has given up on. call_apple removes its prompt file on
  #    timeout; leaving the remote copy would let the device answer it an hour later, against
  #    a consult nobody is waiting for.
  stale=$(comm -13 <(printf '%s\n' "$mine" | sort) <(printf '%s\n' "$remote_prompts" | sort) | grep -v '^$')
  if [ -n "$stale" ]; then
    list=$(printf '%s\n' "$stale" | tr '\n' ' ')
    "${SSH[@]}" "$TARGET" "cd '$REMOTE_QUEUE' && rm -f $list" >/dev/null 2>&1
  fi

  sleep "$INTERVAL"
done
