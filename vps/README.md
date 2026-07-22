# The lighthouse — a headless peer the network granted a public address

There is no lighthouse mode, role, or flag anywhere in the code, and that is the
design (ADR-0009, Phase 2). Every headless peer already runs the full lighthouse
posture: binds `0.0.0.0`, serves TLS on the mesh port, relays gossip
store-and-forward, serves the verified worldview read seam to any member device,
and routes human-gated acts to human-facing peers. What it cannot grant itself is
**reachability** — that comes from where it sits. Deploy the identical peer on a
box with a public address and it *is* the lighthouse; the fleet's other headless
peers (FamTalker01 behind the boat's CGNAT) run the same posture and simply wait
for the network to be kinder.

## Deploy

On a fresh Debian/Ubuntu VPS (any provider; 1 vCPU / 1 GB is plenty — the peer
builds in release once and idles):

```sh
# On any minting member (the boat host):
familiar mesh key            # prints the group secret — trusted channel only

# From the Mac:
ssh root@<vps> 'JOIN_KEY=<key> bash -s' < vps/provision-lighthouse.sh
```

Set `ADVERTISE_HOST=<ip-or-dns>` only if the provider NATs the public IP off the
interface (AWS/GCP style) or you want a stable DNS name advertised — on
Hetzner/DigitalOcean/Vultr the interface IP *is* the public IP and the existing
advertisement (`reachable_hosts()`) already tells devices the truth.

## Wire the fleet to it

Each fleet node dials **out** (CGNAT means the lighthouse can never dial in):

```sh
familiar mesh peer <vps-addr>     # adds to static_peers; gossip does the rest
```

From there convergence is automatic: the boat nodes gossip with the lighthouse
every round, the lighthouse appears in their peer rosters, and every worldview
they serve advertises its address in `hosts` — so enrolled devices learn the
lighthouse without re-enrollment and fail over to it when they leave the LAN.

## Security posture

- The mesh port (47100/tcp) is the only listener exposed; `/local/*` seams bind
  loopback. The provisioning script sets ufw default-deny + SSH + 47100.
- Everything served is covenant-gated: briefs, worldview reads, and observation
  batches are signature- and membership-verified after TLS. What an internet
  stranger can do: read `/mesh/hello`, and file an enrollment request that sits
  pending until a human at a human-facing peer approves or denies it.
- `auto_accept_enrollments` must stay **off** on any public node — admitting a
  member is a human act, doubly so on a port the whole internet can reach.

## Known seams (deliberate, tracked)

- **Device TLS pinning vs. failover**: enrollment payloads carry one node's
  `tlspin`; the worldview `hosts` list carries addresses without per-host pins.
  Today's device clients are pinless (encryption without endpoint proof — payload
  signatures remain the authenticity floor), so failover works. When device
  pinning lands, `hosts` needs to become `(addr, pin)` pairs or pins need to ride
  membership certs.
- **Hole punching** (lighthouse as rendezvous for direct CGNAT↔CGNAT paths) is
  Phase 3, on QUIC's UDP substrate. Until then all off-LAN traffic relays.
