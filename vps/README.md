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
  stranger can do: read `/mesh/hello`, and knock — which lands them as a guest
  reading the anonymised projection, nothing more.
- Under [ADR-0026](../docs/decision-records/0026-two-filter-admission.md) the
  `auto_accept_enrollments` switch is retired in both directions: admission is
  rules-based — a knock lands a **guest** reading the anonymised projection, and
  membership requires the human identity to be *established by evidence* (a
  rotation proof, a device voucher, an invite, or an introduction made in the
  mesh's own space — nothing a port-scanning stranger can produce). This page's
  old instinct ("auto-accept must stay off on a public node") is vindicated by
  deletion: what an internet stranger can get from this box is a guest's
  projected read, ever.

## The reading room (publishing the constitution)

The lighthouse also serves one public document over HTTPS: **The Constitution of
Co-existence**, as static files and nothing else.

```sh
# DNS A/AAAA for the name must already point at this box.
ssh root@<vps> 'CONSTITUTION_DOMAIN=laws.example.org bash -s' < vps/publish-constitution.sh
```

Re-run it to republish after the constitution changes. It refuses to publish bytes
whose fingerprint does not match the manifest — a wrong hash on a public document is
worse than no hash, because the hash is the whole reason to trust a copy.

**This is not a familiar capability and opens no boundary gate.** The daemon is
untouched: no new route, no kernel change, nothing for the guard to weigh. A human is
copying seven files onto a web server. `allow_publish_card` was scoped for the daemon
serving its own agent card — a different act, by a different actor, still unbuilt.

**What it costs.** A public HTTP surface on the box that holds the group secret and is
the mesh's only non-transient entity. Taken deliberately, with the surface kept as
small as a surface gets: static bytes, an explicit file allowlist (never `cp -r` of the
repo), a web root outside `/var/lib/familiar`, caddy as its own user under systemd
hardening with `InaccessiblePaths=/var/lib/familiar`, no directory listing, no dynamic
handler, no upload path. ufw gains 80 and 443; 47100 and SSH are untouched.

Nothing here requires the lighthouse specifically — the published bytes are identical
from any static host, and the fingerprint is what proves it. This box is simply the
public address the mesh already owns.

## Known seams (deliberate, tracked)

- **Device TLS pinning vs. failover**: enrollment payloads carry one node's
  `tlspin`; the worldview `hosts` list carries addresses without per-host pins.
  Today's device clients are pinless (encryption without endpoint proof — payload
  signatures remain the authenticity floor), so failover works. When device
  pinning lands, `hosts` needs to become `(addr, pin)` pairs or pins need to ride
  membership certs.
- **Hole punching** (lighthouse as rendezvous for direct CGNAT↔CGNAT paths) is
  Phase 3, on QUIC's UDP substrate. Until then all off-LAN traffic relays.

---

## The lighthouse-only law (ADR-0027, 2026-08-06)

The mesh must survive every device being off — this box is the only non-transient entity.
Accordingly it now carries, beyond the standard peer posture: the LLM adapter
(`familiar_data/llm/call_llm.sh` + keys, free-tier chain, `allow_llm` opened by the operator),
the full membership record via record-sync, a roster of every heartbeating device, and the
group secret (from its keyed join) so admission works with no other door alive. The
acceptance test for any mesh feature: does it work with only this box up?
