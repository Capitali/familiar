#!/bin/sh
# Stage the Envoy's first principal ceremony without registering anything.
#
# The serving node receives a fresh bearer in its private data dir plus a secret-free pending
# card. The separate Envoy app receives the same bearer in an explicit mode-0600 import bundle.
# Only the later signed console act can turn the staging record into a PrincipalRecord.

set -eu
umask 077

if [ "$#" -lt 4 ] || [ "$#" -gt 5 ]; then
  echo "usage: $0 DATA_DIR ADDRESSED_HUMAN PUBLIC_HTTPS_MCP_ORIGIN ENVOY_IMPORT_JSON [ALIAS]" >&2
  exit 64
fi

data_dir=$1
addressed_to=$2
mcp_origin=$3
export_path=$4
# The partner's display alias on the registration card. Constrained to characters that are
# inert in the JSON both files carry — an alias is a label, never a place to smuggle syntax.
alias=${5:-"Envoy (on-device)"}
case "$alias" in
  ''|*[!A-Za-z0-9\ \(\)._-]*)
    echo "alias must be 1+ characters from: letters, digits, space, () . _ -" >&2
    exit 64
    ;;
esac

if [ ! -d "$data_dir" ]; then
  echo "data dir does not exist: $data_dir" >&2
  exit 66
fi
case "$addressed_to" in
  ''|*[!a-z0-9._-]*)
    echo "addressed human must be a lowercase established handle" >&2
    exit 64
    ;;
esac
case "$mcp_origin" in
  https://*/mcp) ;;
  *)
    echo "the Envoy production origin must be an https:// URL ending in /mcp" >&2
    exit 64
    ;;
esac
case "$mcp_origin" in
  *[!A-Za-z0-9.:/_-]*)
    echo "the MCP origin contains unsupported characters" >&2
    exit 64
    ;;
esac
if [ -e "$export_path" ]; then
  echo "refusing to overwrite Envoy import bundle: $export_path" >&2
  exit 73
fi
command -v openssl >/dev/null 2>&1 || {
  echo "openssl is required to mint a credential" >&2
  exit 69
}
command -v shasum >/dev/null 2>&1 || {
  echo "shasum is required to fingerprint a credential" >&2
  exit 69
}

random_id=$(openssl rand -hex 16)
token=$(openssl rand -hex 32)
registration_id="registration-$random_id"
credential_key=FAMILIAR_MCP_TOKEN
credential_rel="mcp/credentials/$registration_id.env"
credential_dir="$data_dir/mcp/credentials"
pending_dir="$data_dir/mcp/pending-registrations"
credential_path="$data_dir/$credential_rel"
pending_path="$pending_dir/$registration_id.json"
export_dir=$(dirname "$export_path")

mkdir -p "$credential_dir" "$pending_dir" "$export_dir"
if [ -e "$credential_path" ] || [ -e "$pending_path" ]; then
  echo "random registration id collided; run the provisioning command again" >&2
  exit 73
fi

credential_tmp=$(mktemp "$credential_dir/.envoy-credential.XXXXXX")
pending_tmp=$(mktemp "$pending_dir/.envoy-registration.XXXXXX")
export_tmp=$(mktemp "$export_dir/.envoy-import.XXXXXX")
credential_created=0
pending_created=0
export_created=0
finished=0
cleanup() {
  rm -f "$credential_tmp" "$pending_tmp" "$export_tmp"
  if [ "$finished" -eq 0 ]; then
    [ "$export_created" -eq 0 ] || rm -f "$export_path"
    [ "$pending_created" -eq 0 ] || rm -f "$pending_path"
    [ "$credential_created" -eq 0 ] || rm -f "$credential_path"
  fi
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

printf '%s=%s\n' "$credential_key" "$token" > "$credential_tmp"
fingerprint=$(
  printf '%s\000%s' 'familiar-partner-credential-v1' "$token" |
    shasum -a 256 | awk '{print $1}'
)
created_at=$(date +%s)

printf '{\n  "version": 1,\n  "id": "%s",\n  "alias": "%s",\n  "credential_file": "%s",\n  "credential_key": "%s",\n  "credential_fingerprint": "%s",\n  "addressed_to": "%s",\n  "created_at": %s\n}\n' \
  "$registration_id" "$alias" "$credential_rel" "$credential_key" "$fingerprint" \
  "$addressed_to" "$created_at" > "$pending_tmp"

# This bundle is the only exported secret. The Envoy imports it into its own Keychain; callers
# delete the file after that import. Nothing secret is printed to stdout or placed in the card.
printf '{\n  "version": 1,\n  "registration_id": "%s",\n  "alias": "%s",\n  "mcp_origin": "%s",\n  "bearer_token": "%s"\n}\n' \
  "$registration_id" "$alias" "$mcp_origin" "$token" > "$export_tmp"

# Hard-link publication is atomic and refuses an existing destination. Each temporary lives on
# the same filesystem as its target, so a crash exposes either the whole file or no file.
ln "$credential_tmp" "$credential_path"
credential_created=1
ln "$pending_tmp" "$pending_path"
pending_created=1
ln "$export_tmp" "$export_path"
export_created=1

# The door daemon, not the provisioner, must read what was staged — match the data dir's
# owner. Provisioning often runs as root while the daemon runs as a service user, and
# root-owned staging makes the fail-closed inbox read answer 500 to every console (the
# first live ceremony hit exactly this). The Envoy import bundle stays with the caller.
data_dir_owner=$(stat -c '%u:%g' "$data_dir" 2>/dev/null || stat -f '%u:%g' "$data_dir")
chown "$data_dir_owner" "$credential_dir" "$pending_dir" "$credential_path" "$pending_path"
finished=1

echo "staged $registration_id for $addressed_to"
echo "console card: $pending_path"
echo "Envoy import bundle (secret; delete after Keychain import): $export_path"
