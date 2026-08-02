#!/usr/bin/env bash
# One-off lighthouse repair, 2026-08-01. Run from this Mac:
#
#     ./vps/cleanup-2026-08-01.sh
#
# Two independent fixes, both on the lighthouse, both safe to re-run:
#
#   1. Abandon six ghost peer records left by reinstall churn. `mesh abandon` is the
#      non-destructive path — history is kept, the node is hidden from the active roster and
#      from `guests_waiting`, and fresh contact from that node revives it automatically. So if
#      147cfa12 turns out to be Jeff's first install rather than a reinstall of Ian's phone,
#      his next launch restores him and nothing was lost.
#
#      Deliberately NOT abandoned: 83287051 (FamTalker01) — a real registered VM that happens to
#      be powered off. It is the one genuine guest, and the welcome list should still say so.
#
#   2. Set `advertise_hosts` to the lighthouse's own public address. It was empty, so the
#      lighthouse never advertised the one address it is actually reachable at; devices only had
#      it because it is baked into the client as `rendezvousHost`. Meanwhile `reachable_hosts()`
#      advertises its DigitalOcean private addresses (10.17.0.5, 10.108.0.2), which no remote
#      device can dial. An address a node asserts about ITSELF is the trustworthy kind — that is
#      the whole distinction `is_gossipable_addr` draws — so the door should assert the right one.
#
# Not included: deploying the new binary. That is `vps/deploy-lighthouse.sh`, and it is what
# stops the lighthouse GOSSIPING NAT exits to everyone. Clients on the new build already refuse
# to learn them, so this is not urgent — but any device still on build 43 keeps being poisoned
# until the lighthouse is redeployed.
set -euo pipefail

TARGET="${LIGHTHOUSE:-root@134.209.168.50}"
PUBLIC_ADDR="134.209.168.50"

# Six records, each of which lived between 3 and 48 minutes and never came back — install, get
# adopted off a single status heartbeat, uninstall. That adoption path is itself the root cause
# (see docs/reviews/2026-08-01-join-and-authorize.md §2a); this only clears the residue.
GHOSTS=(
  7361e331914fd3e3   # iPhone, 2026-07-27 05:33 → 05:37   (4 min)
  8e511e7ce5e45286   # iPhone, 2026-07-27 06:01 → 06:07   (6 min)
  fb33611b9542d166   # iPhone, 2026-07-28 17:58 → 18:03   (5 min)
  e149820b73afc1ad   # iPhone, 2026-07-28 22:39 → 22:44   (5 min)
  edf98d948c12f318   # iPhone, 2026-07-29 15:22 → 16:10  (48 min) — the anonymous TestFlight installer
  147cfa12ca86540f   # iPhone, 2026-07-31 23:30 → 23:33   (3 min)
)

# The assignments must be quoted as ONE remote word each: ssh joins its arguments and hands the
# result to a shell, so an unquoted space-separated list reads as "assign the first, then run the
# second as a command".
ssh "$TARGET" "GHOSTS='${GHOSTS[*]}' PUBLIC_ADDR='$PUBLIC_ADDR' bash -s" <<'REMOTE'
set -euo pipefail
D=/var/lib/familiar/familiar_data

STAMP=$(date +%Y%m%d-%H%M%S)
cp "$D/mesh/peers.json"  "/root/peers.backup-$STAMP.json"
cp "$D/mesh/config.json" "/root/config.backup-$STAMP.json"
echo "backups: /root/peers.backup-$STAMP.json  /root/config.backup-$STAMP.json"

echo
echo "— abandoning ghost records —"
for n in $GHOSTS; do
  # `--data-dir` is parsed as a trailing flag, not a global one — before the subcommand it is
  # silently ignored and the whole thing degrades to a usage dump.
  familiar mesh abandon "$n" --data-dir "$D" 2>&1 | sed 's/^/  /'
done

echo
echo "— asserting the lighthouse's own public address —"
python3 - "$PUBLIC_ADDR" <<'PY'
import json, sys
p = "/var/lib/familiar/familiar_data/mesh/config.json"
addr = sys.argv[1]
c = json.load(open(p))
adv = c.get("advertise_hosts") or []
if addr in adv:
    print(f"  advertise_hosts already contains {addr}")
else:
    adv.insert(0, addr)
    c["advertise_hosts"] = adv
    json.dump(c, open(p, "w"), indent=2)
    print(f"  advertise_hosts = {adv}")
PY

echo
# The unit is `familiar-peer`, not `familiar`. Restarting matters less for the config edit
# (`advertise_hosts` is re-read on every worldview build) than for the abandons: a daemon holding
# peers in memory could write its own copy back over them.
systemctl restart familiar-peer
sleep 5
echo "— active roster after cleanup —"
python3 - <<'PY'
import json
d = json.load(open("/var/lib/familiar/familiar_data/mesh/peers.json"))
ps = d if isinstance(d, list) else d.get("peers", [])
roll = set(json.load(open("/var/lib/familiar/familiar_data/standing.json"))["full"])
active = [p for p in ps if p.get("status") != "abandoned"]
print(f"  {len(active)} active of {len(ps)} records")
for p in active:
    n = p.get("node_id", "?")
    print(f"    {n[:16]}  {str(p.get('label'))[:20]:22}{'recognised' if n in roll else 'GUEST'}")
PY
REMOTE

echo
echo "Done. The welcome list should now show one guest — FamTalker01 — on every device."
