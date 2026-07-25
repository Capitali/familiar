#!/usr/bin/env bash
# Runs ON a Debian/Ubuntu guest (a small VM, a revived FamTalker01, any LAN box):
# install the testworld household services on :80 and run them at boot.
#
#   ssh <guest> 'bash -s' < tools/testworld/provision-testworld.sh \
#     && scp tools/testworld/testworld.py <guest>:/opt/testworld/
#
# Or copy both files over and run locally as root. Port 80 is deliberate — the
# familiar's reach sweep probes it, so the box lands on the frontier as an http
# device the moment discovery next runs; exploration proceeds from there
# (cultivated curl sensors read /greenhouse /pantry /almanac and their
# readings become musing material).

set -euo pipefail
[ "$(id -u)" = 0 ] || { echo "run as root (systemd + :80)"; exit 1; }

command -v python3 >/dev/null || { apt-get update -qq && apt-get install -y -qq python3; }

install -d /opt/testworld /var/lib/testworld
# testworld.py is copied separately (see header) — don't clobber a newer copy here.

cat > /etc/systemd/system/testworld.service <<'EOF'
[Unit]
Description=testworld — household services for the familiar to explore
After=network.target

[Service]
ExecStart=/usr/bin/python3 /opt/testworld/testworld.py --port 80 --state-dir /var/lib/testworld
Restart=on-failure
# Nothing here needs root beyond the port; keep the surface small.
User=root
ProtectSystem=strict
ReadWritePaths=/var/lib/testworld
ProtectHome=yes
NoNewPrivileges=yes

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable testworld.service
if [ -f /opt/testworld/testworld.py ]; then
  systemctl restart testworld.service
  sleep 1
  curl -sf http://127.0.0.1/ | head -2
  echo "✓ testworld serving on :80"
else
  echo "✓ unit installed — now copy testworld.py to /opt/testworld/ and: systemctl start testworld"
fi
