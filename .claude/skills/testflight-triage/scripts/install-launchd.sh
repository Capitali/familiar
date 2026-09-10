#!/bin/bash
# install-launchd.sh [interval-seconds] — run tf_watch.py --once every N seconds (default 900)
# as a per-user launchd agent on this Mac. Idempotent: re-running replaces the agent.
# Logs: ~/Library/Logs/tf-watch.log. Remove with:  launchctl bootout gui/$UID/io.river.tf-watch
set -euo pipefail
INTERVAL="${1:-900}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG="${TF_WATCH_CONFIG:-$HOME/.config/tf-watch/config.json}"
[ -f "$CONFIG" ] || { echo "no config at $CONFIG — copy config.example.json there and fill it in"; exit 1; }
LABEL=io.river.tf-watch
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
PYTHON="$(command -v python3)"
# launchd agents get a bare PATH; give gh, xcsym and git a chance to be found.
EXTRA_PATH="/opt/homebrew/bin:/usr/local/bin:$(dirname "$(command -v gh || echo /usr/local/bin/gh)"):$(dirname "$(command -v xcsym || echo /usr/local/bin/xcsym)")"
mkdir -p "$HOME/Library/LaunchAgents" "$HOME/Library/Logs"
cat > "$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key>
  <array>
    <string>$PYTHON</string>
    <string>$HERE/tf_watch.py</string>
    <string>--config</string><string>$CONFIG</string>
    <string>--once</string>
  </array>
  <key>StartInterval</key><integer>$INTERVAL</integer>
  <key>RunAtLoad</key><true/>
  <key>StandardOutPath</key><string>$HOME/Library/Logs/tf-watch.log</string>
  <key>StandardErrorPath</key><string>$HOME/Library/Logs/tf-watch.log</string>
  <key>EnvironmentVariables</key>
  <dict><key>PATH</key><string>$EXTRA_PATH:/usr/bin:/bin:/usr/sbin:/sbin</string></dict>
</dict></plist>
EOF
launchctl bootout "gui/$UID/$LABEL" 2>/dev/null || true
launchctl bootstrap "gui/$UID" "$PLIST"
echo "✓ $LABEL installed: every ${INTERVAL}s, log at ~/Library/Logs/tf-watch.log"
launchctl print "gui/$UID/$LABEL" | grep -E 'state|last exit' | head -3 || true
