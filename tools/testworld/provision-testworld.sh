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
# The .py files are copied separately (see header) — don't clobber newer copies here.

# Three units, one pattern: the household on :80, the irrigator agent on :8081,
# the orphan heater on :8082. Same box, same seed — one sky over all of it.
unit() {
  cat > "/etc/systemd/system/$1.service" <<EOF
[Unit]
Description=$2
After=network.target

[Service]
ExecStart=/usr/bin/python3 /opt/testworld/$3 --port $4 --state-dir /var/lib/testworld
Restart=on-failure
# Nothing here needs root beyond :80; keep the surface small.
User=root
ProtectSystem=strict
ReadWritePaths=/var/lib/testworld
ProtectHome=yes
NoNewPrivileges=yes

[Install]
WantedBy=multi-user.target
EOF
}
unit testworld "testworld — household services for the familiar to explore" testworld.py 80
unit irrigator "testworld irrigator — a decision agent in an information void" irrigator.py 8081
unit heater "testworld heater — controls with no controller (steward token gated)" heater.py 8082
unit archivist "testworld archivist — cross-examines claims against its own senses" archivist.py 8083
unit registry "testworld registry — tools of varying provenance, tripwired" registry.py 8084

systemctl daemon-reload
started=0
for svc in testworld:testworld.py irrigator:irrigator.py heater:heater.py archivist:archivist.py registry:registry.py; do
  name="${svc%%:*}"; file="${svc##*:}"
  systemctl enable "$name.service" >/dev/null 2>&1
  if [ -f "/opt/testworld/$file" ]; then
    systemctl restart "$name.service" && started=$((started + 1))
  fi
done
sleep 1
if [ "$started" -gt 0 ]; then
  for p in 80 8081 8082 8083 8084; do
    curl -sf "http://127.0.0.1:$p/" >/dev/null && echo "✓ :$p answering" || echo "· :$p not up (file missing?)"
  done
  [ -f /var/lib/testworld/steward.token ] && \
    echo "steward token (the OWNER's, hand it out deliberately): /var/lib/testworld/steward.token"
else
  echo "✓ units installed — now copy the .py files to /opt/testworld/ and: systemctl start testworld irrigator heater"
fi
