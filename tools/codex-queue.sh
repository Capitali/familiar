#!/bin/zsh
# Reset-aware codex review queue (one review at a time — parallel launches burn the window).
# queue.txt lines: <worktree>|<report path relative to worktree>|<tag>
# Skips entries whose report exists; on the exact usage-limit line, parses "try again at …",
# sleeps until then + 90 s, and retries the same review. Lives under $HOME so it survives a reboot.
set -u
ROOT="${CODEX_RUNS:-$HOME/.codex-runs}"
CODEX="${CODEX:-/Applications/ChatGPT.app/Contents/Resources/codex}"   # on an Intel Mac, the VS Code extension's binary under ~/.vscode/extensions/openai.chatgpt-*/bin/macos-x86_64/codex
Q="$ROOT/queue.txt"; STATUS="$ROOT/queue.status"
say() { echo "$(date -u +%FT%TZ) $*" | tee -a "$STATUS"; }
while IFS='|' read -r wt report tag; do
  [ -z "${wt:-}" ] && continue
  case "$wt" in \#*) continue;; esac
  if [ -f "$wt/$report" ]; then say "skip $tag (report exists)"; continue; fi
  while :; do
    say "launch $tag in $wt"
    ( cd "$wt" && "$CODEX" exec --sandbox workspace-write \
        "Read REVIEW_REQUEST.md in this checkout and carry out the review it describes. Write the report to the path it names, commit it on this branch, and do not push." \
        < /dev/null > "$ROOT/$tag.log" 2>&1 )
    rc=$?
    if grep -q "ERROR: You've hit your usage limit" "$ROOT/$tag.log"; then
      when=$(grep -oE "try again at [^.]*" "$ROOT/$tag.log" | head -1 | sed 's/try again at //')
      say "limit hit on $tag; try again at: ${when:-unknown}"
      target=$(date -j -f "%b %dth, %Y %I:%M %p" "$when" +%s 2>/dev/null || date -j -f "%b %dst, %Y %I:%M %p" "$when" +%s 2>/dev/null || date -j -f "%b %dnd, %Y %I:%M %p" "$when" +%s 2>/dev/null || date -j -f "%b %drd, %Y %I:%M %p" "$when" +%s 2>/dev/null || date -j -f "%I:%M %p" "$when" +%s 2>/dev/null || echo "")
      now=$(date +%s)
      if [ -z "$target" ]; then sleep 3600; continue; fi
      [ "$target" -le "$now" ] && target=$((target + 86400))
      say "sleeping $((target - now + 90)) s"
      sleep $((target - now + 90)); continue
    fi
    say "done $tag rc=$rc report=$( [ -f "$wt/$report" ] && echo present || echo MISSING )"
    break
  done
done < "$Q"
say "queue drained"
