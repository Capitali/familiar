#!/usr/bin/env bash
# Runs INSIDE FamTalker01 as root. Installs the human-declared virtual home and its
# loopback observation feed. Running this script is the human/infra consent act: it
# writes actuators.json and opens the two gates the current ADR-0032 loop requires.

set -euo pipefail
[ "$(id -u)" = 0 ] || { echo "run as root" >&2; exit 1; }

HERE="$(cd "$(dirname "$0")" && pwd)"
ASSETS="$HERE/famtalker01"
DATA_DIR="${FAMILIAR_DATA_DIR:-/var/lib/familiar/familiar_data}"
HOME_STATE="${VIRTUAL_HOME_STATE_DIR:-/var/lib/familiar/virtual-home}"
BOUNDARY="$DATA_DIR/boundary.json"

id -u familiar-svc >/dev/null 2>&1 || {
  echo "familiar-svc does not exist; provision the peer first" >&2
  exit 1
}
command -v python3 >/dev/null || { apt-get update -qq && apt-get install -y -qq python3; }
command -v curl >/dev/null || { apt-get update -qq && apt-get install -y -qq curl; }

install -d /usr/local/lib/familiar
install -m 0755 "$ASSETS/virtual_home.py" /usr/local/lib/familiar/virtual_home.py
install -m 0755 "$ASSETS/virtual-home-feed.sh" /usr/local/bin/familiar-virtual-home-feed
install -d -o familiar-svc -g familiar-svc "$DATA_DIR" "$HOME_STATE"
install -m 0644 -o familiar-svc -g familiar-svc "$ASSETS/actuators.json" \
  "$DATA_DIR/actuators.json"

# Preserve every existing human choice and open only the gates required for a declared
# actuator to poll and act. The familiar itself still has no boundary write path.
python3 - "$BOUNDARY" <<'PY'
import json
import os
import sys
import tempfile

path = sys.argv[1]
try:
    with open(path) as source:
        boundary = json.load(source)
except FileNotFoundError:
    boundary = {"phase": "phase-1"}
if not isinstance(boundary, dict):
    raise ValueError(f"{path} is not a JSON object")
boundary["allow_execute"] = True
boundary["allow_actuate"] = True
directory = os.path.dirname(path)
fd, temporary = tempfile.mkstemp(prefix="boundary.", dir=directory)
with os.fdopen(fd, "w") as target:
    json.dump(boundary, target, indent=2, sort_keys=True)
    target.write("\n")
os.replace(temporary, path)
PY
chown familiar-svc:familiar-svc "$BOUNDARY"
chmod 0644 "$BOUNDARY"

install -m 0644 "$ASSETS/familiar-virtual-home-feed.service" \
  /etc/systemd/system/familiar-virtual-home-feed.service
install -m 0644 "$ASSETS/familiar-virtual-home-feed.timer" \
  /etc/systemd/system/familiar-virtual-home-feed.timer
systemctl daemon-reload
systemctl enable --now familiar-virtual-home-feed.timer
systemctl start familiar-virtual-home-feed.service

echo "✓ FamTalker01 virtual home declared: living-room-lights, greenhouse-lights"
echo "✓ observation feed active; changed snapshots post to /local/observe"
echo "  allow_execute + allow_actuate are open by this human-run provision step"
