#!/usr/bin/env bash
# Runs ON the VPS as root: build the familiar peer daemon, open exactly the mesh
# gate, join the group by key, and wire it to run at boot.
#
#   ssh root@<vps> 'JOIN_KEY=<key> bash -s' < vps/provision-lighthouse.sh
#
# There is no lighthouse mode. This provisions the same headless peer FamTalker01
# runs (vm/) — deployed on a machine the network grants a public address. That
# address is the entire difference.
#
# Env:
#   JOIN_KEY        the group secret (`familiar mesh key` on any minting member).
#                   Optional — without it the node comes up ungrouped; join later
#                   with `familiar mesh join --key <K>` as familiar-svc.
#   ADVERTISE_HOST  address to assert in `advertise_hosts` — only needed when the
#                   public IP is NOT on the interface (cloud 1:1 NAT, e.g. AWS),
#                   or to advertise a stable DNS name. Optional.
#   FAMILIAR_REPO / FAMILIAR_REF   source to build (defaults below).

set -euo pipefail

FAMILIAR_REPO="${FAMILIAR_REPO:-https://github.com/Capitali/familiar}"
FAMILIAR_REF="${FAMILIAR_REF:-main}"
JOIN_KEY="${JOIN_KEY:-}"
ADVERTISE_HOST="${ADVERTISE_HOST:-}"

export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq build-essential curl git ca-certificates

# Rust toolchain (minimal profile — this box only builds, never develops).
if ! command -v cargo >/dev/null; then
  curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
. "$HOME/.cargo/env"

# Build the peer daemon.
if [ ! -d /opt/familiar-src ]; then
  git clone --depth 1 --branch "$FAMILIAR_REF" "$FAMILIAR_REPO" /opt/familiar-src
fi
cargo build --release -p familiar-cli --manifest-path /opt/familiar-src/Cargo.toml
install -m 0755 /opt/familiar-src/target/release/familiar /usr/local/bin/familiar

# Service account + data dir.
id -u familiar-svc >/dev/null 2>&1 || useradd -r -m -d /var/lib/familiar -s /usr/sbin/nologin familiar-svc
install -d -o familiar-svc -g familiar-svc /var/lib/familiar/familiar_data/mesh

# The boundary: the human act of provisioning this box opens exactly ONE gate — the
# mesh. Everything else stays fail-closed (serde defaults fill the rest closed).
cat > /var/lib/familiar/familiar_data/boundary.json <<'EOF'
{
  "phase": "phase-1",
  "allow_mesh": true
}
EOF

# Mesh tunables. Headless (no human at this node); LAN discovery off — there is no
# LAN here worth beaconing, and a public segment is nowhere to broadcast; auto_peer
# off — nothing is reachable outbound to seek (the fleet is behind CGNAT), and
# enrollment happens by key below. auto_accept_enrollments stays default-off: this
# port faces the whole internet, and admitting a member is a human act.
ADV_JSON=""
if [ -n "$ADVERTISE_HOST" ]; then
  ADV_JSON=",
  \"advertise_hosts\": [\"$ADVERTISE_HOST\"]"
fi
cat > /var/lib/familiar/familiar_data/mesh/config.json <<EOF
{
  "headless": true,
  "lan_discovery": false,
  "auto_peer": false$ADV_JSON
}
EOF
chown -R familiar-svc:familiar-svc /var/lib/familiar

# Join the covenant by key — offline: the key IS the group secret; holding it mints
# membership with no dial back to the (CGNAT-unreachable) fleet.
if [ -n "$JOIN_KEY" ]; then
  runuser -u familiar-svc -- env HOME=/var/lib/familiar \
    /usr/local/bin/familiar mesh join --key "$JOIN_KEY" --label "$(hostname -s)"
fi

# Firewall: the mesh port only. The /local console seam binds loopback and the
# daemon opens nothing else; still, default-deny is what a public box deserves.
if command -v ufw >/dev/null; then
  ufw allow OpenSSH >/dev/null
  ufw allow 47100/tcp comment 'familiar mesh (TLS)' >/dev/null
  ufw --force enable >/dev/null
fi

# Run at boot — same unit the VM peer uses.
install -m 0644 /opt/familiar-src/vm/familiar-peer.service /etc/systemd/system/familiar-peer.service
systemctl daemon-reload
systemctl enable --now familiar-peer.service

# Name the node after its host — the daemon mints the node identity with a generic
# "familiar" label on first start; the roster needs a real name.
sleep 3
NODE_JSON=/var/lib/familiar/familiar_data/mesh/node.json
if grep -q '"label": "familiar"' "$NODE_JSON" 2>/dev/null; then
  sed -i "s/\"label\": \"familiar\"/\"label\": \"$(hostname -s)\"/" "$NODE_JSON"
  chown familiar-svc:familiar-svc "$NODE_JSON"
  systemctl restart familiar-peer.service
fi

echo "✓ lighthouse provisioned — headless peer running, mesh gate open."
echo "  From each fleet node:  familiar mesh peer $(curl -4fsS --max-time 5 https://icanhazip.com 2>/dev/null || echo '<this-box-public-ip>')"
systemctl --no-pager --lines 5 status familiar-peer.service || true
