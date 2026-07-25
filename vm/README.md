# FamTalker01 — a headless VirtualBox gossip peer

Provisioning for a minimal, headless Debian VM that runs the familiar peer daemon at
boot and joins the mesh on its own — a worker node with no human at the console.

## What it does

`create-famtalker01.sh` (run **on the Mac**):

1. **Destroys the old `familiar` VM** — disks included. It asks you to type the VM name
   to confirm first; this is irreversible.
2. Creates **FamTalker01**, optimized for a headless peer: 2 vCPU / 2 GB RAM / 12 GB
   disk, no audio/USB/remote-display, minimal graphics, and — importantly — a **bridged
   NIC**: the mesh's LAN discovery beacons (UDP broadcast) and inbound gossip both need
   the VM to sit on the real LAN; NAT would break them.
3. Unattended-installs Debian (netinst, minimal package selection; arm64 on Apple
   Silicon, amd64 on Intel).
4. Provisions the guest (`provision-guest.sh`): builds `familiar-cli` in release, opens
   **exactly one** boundary gate (`allow_mesh`) — everything else stays fail-closed —
   and writes `mesh/config.json` with `auto_peer`, `lan_discovery`, and `headless` on.
5. Installs `familiar-peer.service` (systemd) so the daemon runs at boot, and a macOS
   LaunchAgent so the VM itself starts headless at login.

## How it joins and announces

On boot the daemon's mesh transport (see `docs/mesh.md`):

- discovers peers by **LAN beacon** and tailnet enumeration;
- with `auto_peer` on, **asks any reachable member to admit it by covenant** (any member
  can admit — non-minting members relay), or **auto-forms** a group if no group exists
  anywhere in reach;
- once enrolled, its **signed brief** — identity, capabilities, offered tools/patterns —
  is its announcement of readiness to work, re-gossiped every round, and it appears in
  every peer's roster (`familiar_data/mesh/peers.json`) and Glass mesh panel.

`headless: true` marks that no human is at this node, so human-gated acts route to
human-facing peers rather than waiting forever.

## Run it

```sh
bash vm/create-famtalker01.sh
```

Tunables (env): `MEM_MB`, `CPUS`, `DISK_MB`, `GUEST_PASSWORD`, `FAMILIAR_ISO`
(pre-downloaded ISO path), `OLD_VM` / `VM` (names). `provision-guest.sh` honors
`FAMILIAR_REPO` / `FAMILIAR_REF` — the ref defaults to the branch carrying the mesh
auto-join work; point it at the default branch once that merges.

## Notes & limits

- Written for VirtualBox ≥ 7.0 (`unattended install`, guestcontrol). On Apple Silicon,
  VirtualBox's ARM support is younger — if `unattended install` misbehaves there, install
  Debian arm64 manually into the created VM, then run steps from `provision-guest.sh`
  inside it.
- This script set was authored off-host and not executed against a live VirtualBox —
  treat the first run as supervised, not fire-and-forget.
- The guest user is `familiar` / `$GUEST_PASSWORD` (default `famtalker01`); change it
  (`passwd`) once the VM is up if the LAN isn't fully trusted.
