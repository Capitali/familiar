#!/usr/bin/env bash
# Runs INSIDE the FamTalker01 guest as root: build the familiar peer daemon,
# open exactly the mesh gate, and wire it to run at boot.

set -euo pipefail

# Until the mesh auto-join/auto-form work merges to the default branch, build from its branch.
FAMILIAR_REPO="${FAMILIAR_REPO:-https://github.com/Capitali/familiar}"
FAMILIAR_REF="${FAMILIAR_REF:-claude/session-8nlpbv}"

export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq build-essential curl git ca-certificates

# Rust toolchain (minimal profile — this VM only builds, never develops).
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

# The boundary: the human act of provisioning this VM opens exactly ONE gate — the mesh.
# Everything else stays fail-closed (serde defaults fill the rest of the file closed).
cat > /var/lib/familiar/familiar_data/boundary.json <<'EOF'
{
  "phase": "phase-1",
  "allow_mesh": true
}
EOF

# Mesh tunables: seek/form a covenant on its own, beacon on the LAN, and mark itself
# headless so human-gated acts route to human-facing peers instead of waiting forever.
cat > /var/lib/familiar/familiar_data/mesh/config.json <<'EOF'
{
  "auto_peer": true,
  "lan_discovery": true,
  "headless": true
}
EOF
chown -R familiar-svc:familiar-svc /var/lib/familiar

# Run at boot.
install -m 0644 /root/familiar-peer.service /etc/systemd/system/familiar-peer.service
systemctl daemon-reload
systemctl enable --now familiar-peer.service

echo "✓ familiar peer provisioned — daemon running, mesh gate open, auto-peering."
systemctl --no-pager --lines 5 status familiar-peer.service || true
