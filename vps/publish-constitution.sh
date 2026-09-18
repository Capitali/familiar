#!/usr/bin/env bash
# Runs ON the lighthouse as root: publish The Constitution of Co-existence as static
# files over HTTPS, and nothing else.
#
#   ssh root@<vps> 'CONSTITUTION_DOMAIN=coexist.humanhighway.net bash -s' < vps/publish-constitution.sh
#
# Re-run it to update the published copy after the constitution changes. It is
# idempotent: same inputs, same result, no duplicated state.
#
# WHAT THIS IS NOT. It is not a familiar capability and it opens no boundary gate.
# The daemon is untouched — no new route, no kernel change, nothing for the guard to
# weigh. A human is copying four files onto a web server they own. That distinction
# is why this ships today rather than waiting on ADR work: `allow_publish_card` was
# scoped for the daemon serving its own card, which is a different act with a
# different actor.
#
# WHAT IT COSTS. It adds a public HTTP surface to the box that holds the group
# secret and is the mesh's only non-transient node (ADR-0018, ADR-0027). That is a
# real increase in attack surface and is taken deliberately. The mitigations below
# are not decoration:
#
#   - the web root is OUTSIDE /var/lib/familiar, contains only files copied by an
#     explicit allowlist, and is read-only to the serving process;
#   - caddy runs as its own unprivileged user under systemd hardening, with the
#     familiar data directory inaccessible to it;
#   - static files only: no dynamic handler, no directory listing, no upload path,
#     no path from a request into anything the mesh holds;
#   - ufw gains exactly 80 and 443. 47100 and SSH are untouched.
#
# If that trade is unwanted, the alternative is a separate $5 box or any static
# host — the published bytes are identical and the fingerprint proves it, which is
# the entire point of publishing a hash. Nothing about this design requires the
# lighthouse specifically; it is simply the public address we already own.
#
# Env:
#   CONSTITUTION_DOMAIN   required. DNS A/AAAA record must already point here —
#                         Caddy obtains the certificate on first start.
#   FAMILIAR_REPO/REF     source to publish from (defaults below).

set -euo pipefail

DOMAIN="${CONSTITUTION_DOMAIN:-}"
FAMILIAR_REPO="${FAMILIAR_REPO:-https://github.com/Capitali/familiar}"
FAMILIAR_REF="${FAMILIAR_REF:-main}"
SRC=/opt/familiar-src
WEBROOT=/var/www/constitution

if [ -z "$DOMAIN" ]; then
  echo "CONSTITUTION_DOMAIN is required (e.g. CONSTITUTION_DOMAIN=coexist.humanhighway.net)" >&2
  exit 2
fi

echo "==> publishing the constitution at https://$DOMAIN"

# --- 1. Caddy, from its own apt repository ---------------------------------------
if ! command -v caddy >/dev/null; then
  export DEBIAN_FRONTEND=noninteractive
  apt-get update -qq
  apt-get install -y -qq debian-keyring debian-archive-keyring apt-transport-https curl gnupg
  curl -fsSL https://dl.cloudsmith.io/public/caddy/stable/gpg.key \
    | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
  echo "deb [signed-by=/usr/share/keyrings/caddy-stable-archive-keyring.gpg] https://dl.cloudsmith.io/public/caddy/stable/deb/debian any-version main" \
    > /etc/apt/sources.list.d/caddy-stable.list
  apt-get update -qq
  apt-get install -y -qq caddy
fi

# --- 2. Source of truth ----------------------------------------------------------
if [ -d "$SRC/.git" ]; then
  git -C "$SRC" fetch --depth 1 origin "$FAMILIAR_REF"
  git -C "$SRC" checkout -q FETCH_HEAD
else
  git clone --depth 1 --branch "$FAMILIAR_REF" "$FAMILIAR_REPO" "$SRC"
fi

# --- 3. The allowlist ------------------------------------------------------------
# Named one by one, on purpose. A `cp -r` of the repository would publish the whole
# project — including anything a future commit puts in it that was never meant to
# face the internet. Adding a file to the reading room is an edit to this list.
install -d -m 0755 /var/www "$WEBROOT"
PUBLISH=(
  "data/laws/constitution.html"
  "data/laws/implementation.html"
  "data/laws/laws.v1.json"
  "data/laws/laws.v1.md"
  "data/laws/ADOPT.md"
  "data/laws/LICENSE"
  "data/laws/charter.v1.md"
  "data/laws/charter.v1.json"
  "data/laws/RED-TEAM.md"
  "data/laws/DISSENT.md"
  "robots.txt"
)
NEW=$(mktemp -d /var/www/.constitution.XXXXXX)   # same filesystem as $WEBROOT, so the swap is a rename
for f in "${PUBLISH[@]}"; do
  [ -f "$SRC/$f" ] || { echo "missing from source: $f" >&2; exit 1; }
  install -m 0644 "$SRC/$f" "$NEW/$(basename "$f")"
done
# Atomic-ish swap: the reading room is never half-written.
rm -rf "$WEBROOT.old"
if [ -d "$WEBROOT" ]; then mv "$WEBROOT" "$WEBROOT.old"; fi
mv "$NEW" "$WEBROOT"
chmod 0755 "$WEBROOT"
rm -rf "$WEBROOT.old"
chown -R root:root "$WEBROOT"

# --- 4. Verify what we are about to serve ----------------------------------------
# Publishing a fingerprint that does not match the bytes under it is worse than
# publishing no fingerprint, so check before the server is allowed to start.
python3 - "$WEBROOT/laws.v1.json" <<'PY'
import hashlib, json, sys
d = json.load(open(sys.argv[1], encoding="utf-8"))
claimed = d.pop("fingerprint")
canon = json.dumps(d, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
actual = "sha256:" + hashlib.sha256(canon.encode()).hexdigest()
if actual != claimed:
    sys.exit(f"REFUSING TO PUBLISH: fingerprint mismatch\n  claimed {claimed}\n  actual  {actual}")
print(f"    fingerprint verified: {claimed}")
PY
# The charter is fingerprinted by the same rule and refused on the same terms.
python3 - "$WEBROOT/charter.v1.json" <<'PY'
import hashlib, json, sys
d = json.load(open(sys.argv[1], encoding="utf-8"))
claimed = d.pop("fingerprint")
canon = json.dumps(d, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
actual = "sha256:" + hashlib.sha256(canon.encode()).hexdigest()
if actual != claimed:
    sys.exit(f"REFUSING TO PUBLISH: charter fingerprint mismatch\n  claimed {claimed}\n  actual  {actual}")
print(f"    charter fingerprint verified: {claimed}")
PY

# --- 5. Caddy config + hardening -------------------------------------------------
install -m 0644 "$SRC/vps/Caddyfile" /etc/caddy/Caddyfile
install -d -m 0755 /var/log/caddy && chown caddy:caddy /var/log/caddy
echo "CONSTITUTION_DOMAIN=$DOMAIN" > /etc/default/caddy-constitution

install -d -m 0755 /etc/systemd/system/caddy.service.d
cat > /etc/systemd/system/caddy.service.d/10-constitution.conf <<EOF
[Service]
EnvironmentFile=/etc/default/caddy-constitution
NoNewPrivileges=true
PrivateTmp=true
PrivateDevices=true
ProtectSystem=strict
ProtectHome=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictSUIDSGID=true
RestrictNamespaces=true
LockPersonality=true
MemoryDenyWriteExecute=true
SystemCallArchitectures=native
ReadWritePaths=/var/lib/caddy /var/log/caddy
ReadOnlyPaths=$WEBROOT
# The mesh's data — the group secret included — is not reachable from the web server.
InaccessiblePaths=/var/lib/familiar
EOF

# --- 6. The firewall gains exactly two ports -------------------------------------
if command -v ufw >/dev/null; then
  ufw allow 80/tcp  >/dev/null
  ufw allow 443/tcp >/dev/null
fi

systemctl daemon-reload
CONSTITUTION_DOMAIN="$DOMAIN" caddy validate --config /etc/caddy/Caddyfile
# `caddy validate` runs here as root and provisions the site's log writer, which
# creates /var/log/caddy/constitution.log owned by root, mode 0600. The service runs
# as the caddy user and cannot open that file, so the reload fails and the restart
# that follows it fails too (2026-09-18: both names were down for two minutes).
chown -R caddy:caddy /var/log/caddy
systemctl enable --now caddy
systemctl reload caddy 2>/dev/null || systemctl restart caddy
# Say "published" only over a server that is actually up.
sleep 2
if ! systemctl is-active --quiet caddy; then
  echo "caddy is not running after the reload — nothing is published:" >&2
  journalctl -u caddy -n 20 --no-pager >&2
  exit 1
fi

cat <<EOF

==> published.

    https://$DOMAIN/                       the constitution, rendered
    https://$DOMAIN/implementation         how one system enforces it
    https://$DOMAIN/.well-known/laws.json  machine-readable, fingerprinted
    https://$DOMAIN/laws.md                the text, written to survive chunking
    https://$DOMAIN/adopt                  the adoption kit
    https://$DOMAIN/license                CC0
    https://$DOMAIN/charter                The Service Charter, downstream of the constitution
    https://$DOMAIN/charter.json           machine-readable, fingerprinted
    https://$DOMAIN/red-team               the questions the charter leads with
    https://$DOMAIN/dissent                the room kept for disagreement
    https://$DOMAIN/robots.txt             AI crawlers explicitly welcome

    Check it from anywhere:
      curl -s https://$DOMAIN/.well-known/laws.json | head -3

    Re-run this script to republish after the constitution changes. It refuses to
    publish bytes whose fingerprint does not match.
EOF
