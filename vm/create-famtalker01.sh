#!/usr/bin/env bash
# FamTalker01 — build a headless VirtualBox gossip-peer VM on macOS.
#
# Destroys the old "familiar" VM (with confirmation), creates "FamTalker01"
# (minimal Debian, headless, bridged NIC so LAN discovery beacons work),
# unattended-installs the OS, then provisions the familiar peer daemon to run
# at boot with auto_peer + lan_discovery on, so it joins the mesh and announces
# itself (its /mesh/hello + signed brief) with no human at the console.
#
# Run ON THE MAC:  bash vm/create-famtalker01.sh
# Requirements: VirtualBox (VBoxManage on PATH), ~15 GB disk, internet.

set -euo pipefail

OLD_VM="${OLD_VM:-familiar}"
VM="${VM:-FamTalker01}"
MEM_MB="${MEM_MB:-2048}"
CPUS="${CPUS:-2}"
DISK_MB="${DISK_MB:-12288}"
GUEST_PASSWORD="${GUEST_PASSWORD:-famtalker01}"
ISO="${FAMILIAR_ISO:-}"
VM_DIR="${VM_DIR:-$HOME/VirtualBox VMs}"
HERE="$(cd "$(dirname "$0")" && pwd)"

command -v VBoxManage >/dev/null || { echo "VBoxManage not found — install VirtualBox first."; exit 1; }

# ---- 1. Remove the old VM (destructive — confirmed interactively) -------------------
if VBoxManage showvminfo "$OLD_VM" >/dev/null 2>&1; then
  echo "About to PERMANENTLY DESTROY the VirtualBox VM '$OLD_VM' (disks included)."
  read -r -p "Type the VM name to confirm: " confirm
  [ "$confirm" = "$OLD_VM" ] || { echo "Not confirmed — aborting."; exit 1; }
  VBoxManage controlvm "$OLD_VM" poweroff >/dev/null 2>&1 || true
  sleep 2
  VBoxManage unregistervm "$OLD_VM" --delete
  echo "✓ '$OLD_VM' destroyed."
else
  echo "No VM named '$OLD_VM' — nothing to remove."
fi

# ---- 2. Pick the guest arch + ISO ----------------------------------------------------
HOST_ARCH="$(uname -m)"
if [ "$HOST_ARCH" = "arm64" ]; then
  OSTYPE_ID="Debian_arm64"
  ISO_DIR="https://cdimage.debian.org/debian-cd/current/arm64/iso-cd/"
else
  OSTYPE_ID="Debian_64"
  ISO_DIR="https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/"
fi
if [ -z "$ISO" ]; then
  ISO_NAME="$(curl -fsSL "$ISO_DIR" | grep -o 'debian-[0-9.]*-[a-z0-9]*-netinst\.iso' | head -1)"
  [ -n "$ISO_NAME" ] || { echo "Could not determine the Debian netinst ISO name — set FAMILIAR_ISO=/path/to.iso"; exit 1; }
  ISO="$HOME/Downloads/$ISO_NAME"
  [ -f "$ISO" ] || { echo "Downloading $ISO_NAME…"; curl -fL -o "$ISO" "$ISO_DIR$ISO_NAME"; }
fi
echo "✓ ISO: $ISO"

# ---- 3. Create the VM — headless-optimized -------------------------------------------
BRIDGE_IF="$(VBoxManage list bridgedifs | awk -F': +' '/^Name:/{print $2; exit}')"
[ -n "$BRIDGE_IF" ] || { echo "No bridgeable network interface found."; exit 1; }
echo "✓ Bridging to: $BRIDGE_IF (bridged NIC — LAN discovery beacons + inbound gossip need it; NAT would break both)"

VBoxManage createvm --name "$VM" --ostype "$OSTYPE_ID" --register
VBoxManage modifyvm "$VM" \
  --memory "$MEM_MB" --cpus "$CPUS" \
  --nic1 bridged --bridgeadapter1 "$BRIDGE_IF" \
  --graphicscontroller vmsvga --vram 16 \
  --audio-driver none --usb off --vrde off \
  --boot1 disk --boot2 dvd --boot3 none --boot4 none
VBoxManage createmedium disk --filename "$VM_DIR/$VM/$VM.vdi" --size "$DISK_MB"
VBoxManage storagectl "$VM" --name SATA --add sata --controller IntelAhci --portcount 2
VBoxManage storageattach "$VM" --storagectl SATA --port 0 --device 0 --type hdd --medium "$VM_DIR/$VM/$VM.vdi"
VBoxManage storageattach "$VM" --storagectl SATA --port 1 --device 0 --type dvddrive --medium "$ISO"

# ---- 4. Unattended OS install --------------------------------------------------------
VBoxManage unattended install "$VM" \
  --iso "$ISO" \
  --user familiar --password "$GUEST_PASSWORD" \
  --full-user-name "Familiar Peer" \
  --hostname famtalker01.local \
  --time-zone UTC \
  --install-additions \
  --package-selection-adjustment minimal \
  --start-vm headless

echo "⏳ Debian is installing unattended (typically 10–25 min). Waiting for the guest…"
for _ in $(seq 1 120); do
  if VBoxManage guestcontrol "$VM" --username familiar --password "$GUEST_PASSWORD" \
       run -- /bin/true >/dev/null 2>&1; then
    READY=1; break
  fi
  sleep 30
done
[ "${READY:-0}" = 1 ] || {
  echo "Guest never became controllable. Once the install finishes, provision manually:"
  echo "  VBoxManage guestcontrol $VM --username root --password $GUEST_PASSWORD copyto $HERE/provision-guest.sh /root/provision-guest.sh"
  echo "  VBoxManage guestcontrol $VM --username root --password $GUEST_PASSWORD run -- /bin/bash /root/provision-guest.sh"
  exit 1
}

# ---- 5. Provision the familiar peer inside the guest ---------------------------------
GC() { VBoxManage guestcontrol "$VM" --username root --password "$GUEST_PASSWORD" "$@"; }
GC copyto "$HERE/provision-guest.sh" /root/provision-guest.sh
GC copyto "$HERE/familiar-peer.service" /root/familiar-peer.service
GC run --timeout 3600000 -- /bin/bash /root/provision-guest.sh

# ---- 6. Autostart the VM headless at login (optional) --------------------------------
PLIST="$HOME/Library/LaunchAgents/io.river.famtalker01.plist"
sed "s|__VBOXMANAGE__|$(command -v VBoxManage)|" "$HERE/io.river.famtalker01.plist" > "$PLIST"
launchctl unload "$PLIST" >/dev/null 2>&1 || true
launchctl load "$PLIST"
echo "✓ LaunchAgent installed — $VM starts headless at login."

echo
echo "✓ $VM is up. It will join the mesh on its own (auto_peer + LAN beacons) and announce"
echo "  itself through /mesh/hello and its signed brief. Watch for it on the host:"
echo "    cat familiar_data/mesh/peers.json"
